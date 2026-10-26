use anyhow::{Context, Result};
use quick_xml::{events::Event, Reader};
use std::{fs, path::Path};

/// Turns Moodle XML into a writer-friendly plain-text transcript.
///
/// ANSI escapes are used for bold and italics when stdout is a terminal.
/// The returned string deliberately keeps the escapes so callers can choose
/// whether to print them or remove them.
///
/// The file is read as bytes with path context on I/O errors, then passed to
/// [`render_xml`] for parsing. File payload text is intentionally suppressed
/// by the XML renderer so image base64 does not flood the transcript.
pub fn render_file(path: impl AsRef<Path>) -> Result<String> {
    let data =
        fs::read(path.as_ref()).with_context(|| format!("reading {}", path.as_ref().display()))?;
    render_xml(&data)
}

/// Turns Moodle XML bytes into a writer-friendly plain-text transcript.
///
/// The parser intentionally ignores file payloads and represents embedded
/// images as `[image]` rather than printing base64 data.
///
/// A streaming XML reader tracks whether it is inside a question name, prompt,
/// answer, or image file. It numbers questions and answers as those elements
/// begin, appends only text in the relevant regions, and delegates HTML text
/// fragments for inline formatting. `strong`/`b` and `i`/`em` tags produce
/// ANSI escapes, `<br>` adds a newline, and `<img>` becomes `[image]`. The
/// result is trimmed and terminated with exactly one newline.
pub fn render_xml(data: &[u8]) -> Result<String> {
    let mut reader = Reader::from_reader(data);
    let mut buffer = Vec::new();
    let mut output = String::new();
    let mut question_number = 0;
    let mut in_name = false;
    let mut in_question_text = false;
    let mut in_answer = false;
    let mut in_answer_text = false;
    let mut in_file = false;
    let mut bold = false;
    let mut italic = false;
    let mut answer_number = 0;

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) => match event.name().as_ref() {
                b"question" => {
                    question_number += 1;
                    answer_number = 0;
                    output.push_str(&format!("\n{}. ", question_number));
                }
                b"name" => in_name = true,
                b"questiontext" => in_question_text = true,
                b"answer" => {
                    answer_number += 1;
                    in_answer = true;
                    output.push_str(&format!("\n  {}. ", answer_number));
                }
                b"text" if in_name || in_question_text || in_answer => {
                    if in_question_text {
                        output.push_str("\n  ");
                    }
                    in_answer_text = in_answer;
                }
                b"file" => in_file = true,
                b"strong" | b"b" => {
                    bold = true;
                    output.push_str("\x1b[1m");
                }
                b"i" | b"em" => {
                    italic = true;
                    output.push_str("\x1b[3m");
                }
                b"br" => output.push('\n'),
                b"img" => output.push_str("[image]"),
                _ => {}
            },
            Event::End(event) => match event.name().as_ref() {
                b"question" => {
                    output.push('\n');
                }
                b"name" => in_name = false,
                b"questiontext" => in_question_text = false,
                b"answer" => {
                    in_answer = false;
                    in_answer_text = false;
                }
                b"text" => {
                    if bold || italic {
                        output.push_str("\x1b[0m");
                        bold = false;
                        italic = false;
                    }
                }
                b"file" => in_file = false,
                _ => {}
            },
            Event::Text(event) if !in_file => {
                let value = event.unescape()?.into_owned();
                if in_name || in_question_text || in_answer_text {
                    append_html_fragment(&value, &mut output)?;
                }
            }
            Event::CData(event) if !in_file => {
                let value = String::from_utf8_lossy(event.as_ref());
                if in_name || in_question_text || in_answer_text {
                    append_html_fragment(&value, &mut output)?;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(output.trim().to_string() + "\n")
}

/// Appends one HTML fragment from a Moodle text node to the transcript.
///
/// A second XML reader handles the markup embedded inside Moodle's XML text
/// element. It preserves text and CDATA, maps bold and italic tags to ANSI
/// styling, and maps empty break and image tags to transcript equivalents.
/// Unsupported markup is skipped while its text children remain visible.
fn append_html_fragment(value: &str, output: &mut String) -> Result<()> {
    let mut reader = Reader::from_str(value);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) => match event.name().as_ref() {
                b"strong" | b"b" => output.push_str("\x1b[1m"),
                b"i" | b"em" => output.push_str("\x1b[3m"),
                _ => {}
            },
            Event::End(event)
                if matches!(event.name().as_ref(), b"strong" | b"b" | b"i" | b"em") =>
            {
                output.push_str("\x1b[0m");
            }
            Event::Empty(event) if event.name().as_ref() == b"br" => output.push('\n'),
            Event::Empty(event) if event.name().as_ref() == b"img" => output.push_str("[image]"),
            Event::Text(event) => output.push_str(&event.unescape()?),
            Event::CData(event) => output.push_str(&String::from_utf8_lossy(event.as_ref())),
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}
