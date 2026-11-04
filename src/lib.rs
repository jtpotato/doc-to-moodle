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
mod preview;
mod text;

use anyhow::Result;
use std::io::{Read, Seek};

pub use gift::{parse_questions, Answer, Inline, Paragraph, Question, QuestionType};
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
    moodle::questions_to_xml(questions, &std::collections::HashMap::new())
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
    convert_docx_with_preview(input, false).map(|(xml, _)| xml)
}

/// Converts a DOCX package and returns both Moodle XML and a readable question preview.
pub fn convert_docx_with_preview<R: Read + Seek>(
    input: R,
    color: bool,
) -> Result<(String, String)> {
    let document = docx::Document::read(input)?;
    let questions = parse_questions(document.paragraphs)?;
    let preview = preview::render_questions(&questions, color);
    let xml = moodle::questions_to_xml(&questions, &document.images);
    Ok((xml, preview))
}
