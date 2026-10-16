use moodle_doc_converter::{convert_docx, parse_questions, render_moodle_text, Inline, Paragraph};
use std::io::Cursor;

fn paragraph(text: &str) -> Paragraph {
    Paragraph {
        inlines: vec![Inline::Text {
            value: text.into(),
            bold: false,
            italic: false,
        }],
    }
}

#[test]
fn splits_marks_evenly_for_multiple_correct_answers() {
    let questions = parse_questions(vec![
        paragraph("[MCQ multi]"),
        paragraph("Choose all"),
        paragraph("One (*)"),
        paragraph("Two (*)"),
        paragraph("Three"),
    ])
    .unwrap();
    let xml = moodle_doc_converter::moodle_xml_for_test(&questions);
    assert!(xml.contains("<single>false</single>"));
    assert_eq!(xml.matches("fraction=\"50\"").count(), 2);
    assert!(xml.contains("fraction=\"0\""));
}

#[test]
fn uses_moodle_compatible_precision_for_three_correct_answers() {
    let questions = parse_questions(vec![
        paragraph("[MCQ thirds]"),
        paragraph("Choose all"),
        paragraph("One (*)"),
        paragraph("Two (*)"),
        paragraph("Three (*)"),
    ])
    .unwrap();
    let xml = moodle_doc_converter::moodle_xml_for_test(&questions);
    assert_eq!(xml.matches("fraction=\"33.3333333333\"").count(), 3);
}

#[test]
fn answers_token_unambiguously_starts_answers() {
    let questions = parse_questions(vec![
        paragraph("[MCQ token]"),
        paragraph("This body does not end in a question mark."),
        paragraph("[Answers]"),
        paragraph("Correct (*)"),
        paragraph("A distractor"),
    ])
    .unwrap();
    assert_eq!(
        questions[0].prompt[0].plain_text(),
        "This body does not end in a question mark."
    );
    assert_eq!(questions[0].answers.len(), 2);
}

#[test]
fn text_subcommand_rendering_keeps_readable_structure_and_styles() {
    let xml = br#"<quiz><question type="multichoice">
        <name><text>styles</text></name>
        <questiontext format="html"><text><![CDATA[Choose <strong>bold</strong><br/>now]]></text></questiontext>
        <answer fraction="100"><text><![CDATA[<i>yes</i>]]></text></answer>
        <answer fraction="0"><text>no</text></answer>
    </question></quiz>"#;
    let text = render_moodle_text(xml).unwrap();
    assert!(text.contains("1. styles"));
    assert!(text.contains("Choose \u{1b}[1mbold"));
    assert!(text.contains("1. \u{1b}[3myes"));
}

#[test]
fn preserves_inline_styles_in_moodle_html() {
    let questions = parse_questions(vec![
        paragraph("[MCQ styles]"),
        paragraph("Which?"),
        Paragraph {
            inlines: vec![
                Inline::Text {
                    value: "bold".into(),
                    bold: true,
                    italic: false,
                },
                Inline::Text {
                    value: " italic".into(),
                    bold: false,
                    italic: true,
                },
            ],
        },
        paragraph("No"),
    ])
    .unwrap();
    let xml = moodle_doc_converter::moodle_xml_for_test(&questions);
    assert!(xml.contains("<strong>bold</strong><i> italic</i>"));
}

#[test]
fn converts_the_example_docx() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("2026 Daily Quizzes - Questions.docx");
    let bytes = std::fs::read(path).unwrap();
    let xml = convert_docx(Cursor::new(bytes)).unwrap();
    assert_eq!(xml.matches("<question type=\"multichoice\">").count(), 13);
    assert_eq!(xml.matches("<file ").count(), 11);
    assert!(xml.contains("width=\""));
    assert!(xml.contains("height=\""));
    assert!(xml.contains("fraction=\"50\""));
}
