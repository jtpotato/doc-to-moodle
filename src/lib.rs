//! Convert a Word question manuscript into a Moodle multiple-choice question bank.
//!
//! The conversion pipeline reads the DOCX ZIP package, maps Word paragraphs and
//! embedded images into an intermediate model, detects question and answer
//! markers, then serializes the model as Moodle XML. See [`convert_docx`] for
//! the end-to-end entry point and [`parse_questions`] for the authoring marker
//! rules. [`render_moodle_text`] can turn generated XML into a readable
//! diagnostic transcript.
//!
//! # Generating the documentation
//!
//! Run `cargo doc --no-deps --document-private-items` to build this API
//! documentation with the internal parsing and rendering helpers included.

mod docx;
mod gift;
mod moodle;
mod text;

use anyhow::Result;
use std::io::{Cursor, Read, Seek};
use wasm_bindgen::prelude::*;

pub use gift::{parse_questions, Answer, Inline, Paragraph, Question};
pub use text::{render_file as render_moodle_text_file, render_xml as render_moodle_text};

/// Renders questions without a DOCX image archive.
///
/// This helper is primarily useful for tests and callers that construct the
/// intermediate model themselves. Images are rendered without dimensions and
/// no image file payloads are emitted.
///
/// The function supplies an empty image map to the same Moodle renderer used
/// by [`convert_docx`]. Text and question settings therefore follow the normal
/// rendering rules, while image references have no dimensions or associated
/// base64 file payloads.
pub fn moodle_xml_for_test(questions: &[Question]) -> String {
    moodle::render_questions(questions, &std::collections::HashMap::new())
}

/// Converts a DOCX package to a Moodle XML question bank.
///
/// The reader must be seekable because DOCX is a ZIP archive. The pipeline
/// reads Word paragraphs and runs, resolves embedded media, parses question
/// markers, and renders Moodle `multichoice` XML.
///
/// The stages run in order: the DOCX reader opens the ZIP and resolves
/// supported Word content; [`parse_questions`] groups paragraphs by markers
/// and validates that each question has answers; the Moodle renderer embeds
/// image data and assigns answer fractions. Any failure in ZIP/XML reading,
/// image resolution, or question validation is returned with context.
pub fn convert_docx<R: Read + Seek>(input: R) -> Result<String> {
    let document = docx::Document::read(input)?;
    let questions = parse_questions(document.paragraphs)?;
    Ok(moodle::render_questions(&questions, &document.images))
}

/// Converts a DOCX in the browser and returns both a structured preview and
/// its Moodle XML. The input and all generated content remain in memory.
#[wasm_bindgen]
pub fn convert_docx_preview(bytes: &[u8]) -> std::result::Result<String, JsValue> {
    let document = docx::Document::read(Cursor::new(bytes))
        .map_err(|error| JsValue::from_str(&format!("Could not read this DOCX: {error:#}")))?;
    let questions = parse_questions(document.paragraphs)
        .map_err(|error| JsValue::from_str(&format!("Could not parse questions: {error:#}")))?;
    let xml = moodle::render_questions(&questions, &document.images);
    let image_data: std::collections::HashMap<_, _> = document
        .images
        .iter()
        .map(|(name, image)| {
            let mime = if image.bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                "image/png"
            } else {
                "image/jpeg"
            };
            (
                name.clone(),
                format!(
                    "data:{mime};base64,{}",
                    base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &image.bytes
                    )
                ),
            )
        })
        .collect();
    let inline_json = |inline: &Inline| match inline {
        Inline::Text {
            value,
            bold,
            italic,
        } => serde_json::json!({"type":"text", "value":value, "bold":bold, "italic":italic}),
        Inline::Image(name) => {
            serde_json::json!({"type":"image", "name":name, "src":image_data.get(name)})
        }
    };
    let preview: Vec<_> = questions.iter().map(|question| serde_json::json!({
        "name": question.name,
        "prompt": question.prompt.iter().map(|paragraph| paragraph.inlines.iter().map(inline_json).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "answers": question.answers.iter().map(|answer| serde_json::json!({
            "correct": answer.correct,
            "content": answer.inlines.iter().map(inline_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })).collect();
    serde_json::to_string(&serde_json::json!({"questions":preview,"xml":xml}))
        .map_err(|error| JsValue::from_str(&format!("Could not prepare preview: {error}")))
}
