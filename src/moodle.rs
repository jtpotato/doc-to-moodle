use crate::{docx::Image, Inline, Paragraph, Question};
use base64::Engine;
use std::collections::HashMap;

/// Serializes parsed questions as a complete Moodle XML `<quiz>` document.
///
/// Questions are rendered in input order. Each question's prompt and answers
/// become HTML fragments inside Moodle text elements, answer correctness is
/// converted to a grade fraction, and image bytes are included as base64 files
/// when referenced. The returned string always contains the XML declaration
/// and closing quiz tag.
pub fn questions_to_xml(questions: &[Question], images: &HashMap<String, Image>) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<quiz>\n");
    for question in questions {
        render_question(&mut output, question, images);
    }
    output.push_str("</quiz>\n");
    output
}

/// Appends one multichoice question, including prompt media and graded answers.
///
/// Correct answers use their explicit percentages where supplied. Unscored
/// correct answers divide the remaining percentage evenly.
/// Prompt image payloads are attached once per distinct filename, while both
/// prompt and answer inline images refer to Moodle's plugin file area.
fn render_question(output: &mut String, question: &Question, images: &HashMap<String, Image>) {
    if question.question_type == crate::gift::QuestionType::DropDown {
        render_dropdown_question(output, question, images);
        return;
    }
    let correct_count = question
        .answers
        .iter()
        .filter(|answer| answer.correct)
        .count();
    output.push_str("  <question type=\"multichoice\">\n");
    output.push_str(&format!(
        "    <name><text>{}</text></name>\n",
        xml_escape(&question.name)
    ));
    output.push_str(&format!(
        "    <questiontext format=\"html\"><text><![CDATA[{}]]></text>",
        question
            .prompt
            .iter()
            .map(|p| render_paragraph(p, images))
            .collect::<Vec<_>>()
            .join("<br/>")
    ));
    for filename in image_names(&question.prompt) {
        if let Some(image) = images.get(&filename) {
            output.push_str(&format!(
                "<file name=\"{}\" path=\"/\" encoding=\"base64\">{}</file>",
                xml_escape(&filename),
                base64::engine::general_purpose::STANDARD.encode(&image.bytes)
            ));
        }
    }
    output.push_str("</questiontext>\n");
    output.push_str(&format!(
        "    <single>{}</single>\n",
        if correct_count == 1 { "true" } else { "false" }
    ));
    output.push_str("    <shuffleanswers>true</shuffleanswers>\n");
    let explicitly_scored: f64 = question
        .answers
        .iter()
        .filter_map(|answer| answer.percentage)
        .sum();
    let unscored_correct = question
        .answers
        .iter()
        .filter(|answer| answer.correct && answer.percentage.is_none())
        .count();
    let distributed_percentage = if unscored_correct == 0 {
        0.0
    } else {
        (100.0 - explicitly_scored) / unscored_correct as f64
    };
    for answer in &question.answers {
        let fraction = if !answer.correct {
            0.0
        } else {
            answer.percentage.unwrap_or(distributed_percentage)
        };
        output.push_str(&format!(
            "    <answer fraction=\"{}\"><text><![CDATA[{}]]></text></answer>\n",
            format_fraction(fraction),
            render_inlines(&answer.inlines, images)
        ));
    }
    output.push_str("  </question>\n");
}

/// Renders each labeled dropdown row as a Moodle embedded multichoice field.
fn render_dropdown_question(
    output: &mut String,
    question: &Question,
    images: &HashMap<String, Image>,
) {
    output.push_str("  <question type=\"cloze\">\n");
    output.push_str(&format!(
        "    <name><text>{}</text></name>\n",
        xml_escape(&question.name)
    ));
    let choices: Vec<String> = question
        .answers
        .iter()
        .filter_map(|answer| answer.dropdown_value.as_ref())
        .flat_map(|value| {
            value.iter().map(|inline| match inline {
                Inline::Text { value, .. } => value.clone(),
                Inline::Image(_) => String::new(),
            })
        })
        .collect();
    let rows = question
        .answers
        .iter()
        .map(|answer| {
            let correct = answer.dropdown_value.as_deref().unwrap_or_default();
            let correct = render_inlines(correct, images);
            let alternatives = choices
                .iter()
                .filter(|choice| html_escape(choice) != correct)
                .map(|choice| html_escape(choice))
                .collect::<Vec<_>>()
                .join("~");
            let choices = if alternatives.is_empty() {
                format!("={correct}")
            } else {
                format!("{alternatives}~={correct}")
            };
            format!(
                "{} {{1:MULTICHOICE_S:{choices}}}",
                render_inlines(&answer.inlines, images),
            )
        })
        .collect::<Vec<_>>()
        .join("<br/>");
    let prompt = question
        .prompt
        .iter()
        .map(|p| render_paragraph(p, images))
        .collect::<Vec<_>>()
        .join("<br/>");
    let text = if prompt.is_empty() {
        rows
    } else {
        format!("{prompt}<br/>{rows}")
    };
    output.push_str(&format!(
        "    <questiontext format=\"html\"><text><![CDATA[{text}]]></text>"
    ));
    for filename in image_names(&question.prompt) {
        if let Some(image) = images.get(&filename) {
            output.push_str(&format!(
                "<file name=\"{}\" path=\"/\" encoding=\"base64\">{}</file>",
                xml_escape(&filename),
                base64::engine::general_purpose::STANDARD.encode(&image.bytes)
            ));
        }
    }
    output.push_str("</questiontext>\n  </question>\n");
}

/// Renders a paragraph by delegating its ordered inline fragments.
///
/// Paragraph boundaries are handled by the caller, which joins prompt
/// paragraphs with `<br/>`; this function preserves only the content and
/// ordering within one paragraph.
fn render_paragraph(paragraph: &Paragraph, images: &HashMap<String, Image>) -> String {
    render_inlines(&paragraph.inlines, images)
}

/// Converts styled text and image references into escaped HTML fragments.
///
/// Text is HTML-escaped before style tags are added, with italic nested inside
/// bold when both flags are set. Images use their media filename in the
/// `@@PLUGINFILE@@` URL and include dimensions only when a matching payload is
/// available. The input inline order is kept exactly.
fn render_inlines(inlines: &[Inline], images: &HashMap<String, Image>) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            Inline::Text {
                value,
                bold,
                italic,
            } => {
                let mut rendered = html_escape(value);
                if *italic {
                    rendered = format!("<i>{rendered}</i>");
                }
                if *bold {
                    rendered = format!("<strong>{rendered}</strong>");
                }
                rendered
            }
            Inline::Image(filename) => {
                let dimensions = images
                    .get(filename)
                    .map(|image| {
                        format!(
                            " width=\"{}\" height=\"{}\"",
                            image.width_px, image.height_px
                        )
                    })
                    .unwrap_or_default();
                format!("<img src=\"@@PLUGINFILE@@/{filename}\" alt=\"\"{dimensions}/>")
            }
        })
        .collect()
}

/// Collects image filenames from paragraphs, keeping first-seen order and uniqueness.
///
/// The renderer uses this list to attach one `<file>` payload per distinct
/// image referenced by a question prompt. Answer images are rendered as URLs
/// by [`render_inlines`], but are not gathered here for file attachment.
fn image_names(paragraphs: &[Paragraph]) -> Vec<String> {
    let mut names = Vec::new();
    for paragraph in paragraphs {
        for inline in &paragraph.inlines {
            if let Inline::Image(filename) = inline {
                if !names.contains(filename) {
                    names.push(filename.clone());
                }
            }
        }
    }
    names
}

/// Formats a grade fraction with enough decimal precision for Moodle imports.
///
/// Ten decimal places preserve repeating fractions such as one third to the
/// precision Moodle accepts. Trailing zeroes and a trailing decimal point are
/// removed so whole values such as `100` remain compact.
fn format_fraction(value: f64) -> String {
    // Moodle compares this value with its generated grade options. Preserve
    // enough precision for 1/3 and other repeating fractions.
    format!("{value:.10}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Escapes the five XML-sensitive characters in element text or attributes.
///
/// Ampersand is replaced first so the entities inserted for later characters
/// are not escaped a second time.
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escapes HTML text delimiters before text is placed inside HTML markup.
///
/// Ampersand is replaced first for the same reason as in [`xml_escape`]. Quote
/// characters are left untouched because this function handles text nodes,
/// not HTML attribute values.
fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
