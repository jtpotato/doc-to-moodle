use crate::{docx::Image, Inline, Paragraph, Question};
use base64::Engine;
use std::collections::HashMap;

pub fn render_questions(questions: &[Question], images: &HashMap<String, Image>) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<quiz>\n");
    for question in questions {
        render_question(&mut output, question, images);
    }
    output.push_str("</quiz>\n");
    output
}

fn render_question(output: &mut String, question: &Question, images: &HashMap<String, Image>) {
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
    let fraction = if correct_count == 0 {
        0.0
    } else {
        100.0 / correct_count as f64
    };
    for answer in &question.answers {
        output.push_str(&format!(
            "    <answer fraction=\"{}\"><text><![CDATA[{}]]></text></answer>\n",
            if answer.correct {
                format_fraction(fraction)
            } else {
                "0".into()
            },
            render_inlines(&answer.inlines, images)
        ));
    }
    output.push_str("  </question>\n");
}

fn render_paragraph(paragraph: &Paragraph, images: &HashMap<String, Image>) -> String {
    render_inlines(&paragraph.inlines, images)
}

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

fn format_fraction(value: f64) -> String {
    // Moodle compares this value with its generated grade options. Preserve
    // enough precision for 1/3 and other repeating fractions.
    format!("{value:.10}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
