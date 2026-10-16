mod docx;
mod gift;
mod moodle;
mod text;

use anyhow::Result;
use std::io::{Read, Seek};

pub use gift::{parse_questions, Answer, Inline, Paragraph, Question};
pub use text::{render_file as render_moodle_text_file, render_xml as render_moodle_text};

/// Renders questions without a DOCX image archive.
///
/// This helper is primarily useful for tests and callers that construct the
/// intermediate model themselves. Images are rendered without dimensions and
/// no image file payloads are emitted.
pub fn moodle_xml_for_test(questions: &[Question]) -> String {
    moodle::render_questions(questions, &std::collections::HashMap::new())
}

/// Converts a DOCX package to a Moodle XML question bank.
///
/// The reader must be seekable because DOCX is a ZIP archive. The pipeline
/// reads Word paragraphs and runs, resolves embedded media, parses question
/// markers, and renders Moodle `multichoice` XML.
pub fn convert_docx<R: Read + Seek>(input: R) -> Result<String> {
    let document = docx::Document::read(input)?;
    let questions = parse_questions(document.paragraphs)?;
    Ok(moodle::render_questions(&questions, &document.images))
}
