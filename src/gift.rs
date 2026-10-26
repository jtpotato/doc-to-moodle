use anyhow::{bail, Result};

/// A piece of question content, either styled text or an embedded image.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    /// A text fragment with the Word styling captured for its source run.
    Text {
        /// Literal text content for this fragment.
        value: String,
        /// Whether the source Word run was bold.
        bold: bool,
        /// Whether the source Word run was italic.
        italic: bool,
    },
    /// An image placeholder, replaced with a media filename after DOCX parsing.
    Image(String),
}

impl Inline {
    /// Constructs a text inline while accepting any value convertible to `String`.
    ///
    /// The parser uses this helper for Word text and tabs so the style flags
    /// captured from the current Word run travel with each text fragment.
    pub(crate) fn text(value: impl Into<String>, bold: bool, italic: bool) -> Self {
        Self::Text {
            value: value.into(),
            bold,
            italic,
        }
    }
}

/// A Word paragraph represented as ordered inline content.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paragraph {
    /// Text and image fragments in the order they appeared in the Word paragraph.
    pub inlines: Vec<Inline>,
}

impl Paragraph {
    /// Returns whether the paragraph contains no visible text or images.
    ///
    /// Text is considered empty after trimming whitespace. An image always
    /// makes a paragraph non-empty, even when its filename is blank, because
    /// the image occupies visible content in the source document.
    pub fn is_empty(&self) -> bool {
        self.inlines.iter().all(|inline| match inline {
            Inline::Text { value, .. } => value.trim().is_empty(),
            Inline::Image(_) => false,
        })
    }

    /// Concatenates text in order, excluding image placeholders.
    ///
    /// Styles are not represented in this plain-text view. The parser uses it
    /// to recognize marker paragraphs and answer suffixes without changing the
    /// original styled inline sequence used later for rendering.
    pub fn plain_text(&self) -> String {
        self.inlines
            .iter()
            .filter_map(|inline| match inline {
                Inline::Text { value, .. } => Some(value.as_str()),
                Inline::Image(_) => None,
            })
            .collect()
    }
}

/// An answer and whether it was marked with the `(*)` correctness suffix.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// Styled text and images that make up the answer body.
    pub inlines: Vec<Inline>,
    /// Whether the author marked this answer with the `(*)` suffix.
    pub correct: bool,
}

/// A parsed multiple-choice question before Moodle XML rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    /// Identifier extracted from the question marker; used as the Moodle name.
    pub name: String,
    /// Prompt paragraphs in their original order.
    pub prompt: Vec<Paragraph>,
    /// Candidate answers in source order.
    pub answers: Vec<Answer>,
}

/// Parses marker-delimited paragraphs into validated questions.
///
/// The explicit `[Answers]` token takes precedence over the legacy
/// question-sentence heuristic.
///
/// Each recognized marker closes the previous question and starts a fresh
/// one. Before `[Answers]`, paragraphs are treated as prompt content until
/// the last prompt looks complete (ends in `?` or `answers)`) or a paragraph
/// carries an answer marker; images keep otherwise ambiguous paragraphs in
/// the prompt. After `[Answers]`, non-empty paragraphs are parsed as choices.
/// Empty paragraphs and `_` separator paragraphs are skipped. A marker before
/// any question is ignored, but a document with no recognized markers and a
/// question without any choices returns an error.
pub fn parse_questions(paragraphs: Vec<Paragraph>) -> Result<Vec<Question>> {
    let mut questions = Vec::new();
    let mut current = None;
    let mut answers_started = false;
    for paragraph in paragraphs {
        if let Some(name) = question_marker(&paragraph.plain_text()) {
            finish_question(&mut questions, current.take())?;
            current = Some(Question {
                name,
                prompt: Vec::new(),
                answers: Vec::new(),
            });
            answers_started = false;
            continue;
        }
        let Some(question) = current.as_mut() else {
            continue;
        };
        let text = paragraph.plain_text();
        if text.trim() == "[Answers]" {
            answers_started = true;
            continue;
        }
        if text.trim() == "_" || (text.trim().is_empty() && paragraph.is_empty()) {
            continue;
        }
        if answers_started {
            if let Some(answer) = parse_answer(&paragraph) {
                question.answers.push(answer);
            }
            continue;
        }
        let explicitly_marked = text.trim().ends_with("(*)") || text.trim().starts_with("~%");
        let prompt_finished = question
            .prompt
            .last()
            .map(|p| {
                let text = p.plain_text();
                text.trim().ends_with('?') || text.trim().ends_with("answers)")
            })
            .unwrap_or(false);
        let contains_image = paragraph
            .inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Image(_)));
        if question.answers.is_empty() && !explicitly_marked && (!prompt_finished || contains_image)
        {
            question.prompt.push(paragraph);
        } else if let Some(answer) = parse_answer(&paragraph) {
            question.answers.push(answer);
        } else if question.answers.is_empty() {
            question.prompt.push(paragraph);
        }
    }
    finish_question(&mut questions, current)?;
    if questions.is_empty() {
        bail!("no question markers found; add [MCQ id=...] or ::id:: markers");
    }
    Ok(questions)
}

/// Validates a completed question and appends it to the output list.
///
/// `None` means no question is currently open and requires no work. A present
/// question must contain at least one answer; this check runs both when the
/// next marker appears and once more at end of input so the final question is
/// held to the same rule.
fn finish_question(questions: &mut Vec<Question>, question: Option<Question>) -> Result<()> {
    if let Some(question) = question {
        if question.answers.is_empty() {
            bail!("question {} has no answers", question.name);
        }
        questions.push(question);
    }
    Ok(())
}

/// Recognizes a supported question marker and returns its trimmed identifier.
///
/// The bracket form accepts `[MCQ ...]` and removes an optional leading
/// `id=`. A legacy `2026_D..._Q...` identifier is also accepted, followed by
/// the GIFT form `::identifier::`. Unrecognized text and empty GIFT identifiers
/// return `None`.
fn question_marker(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(id) = text
        .strip_prefix("[MCQ")
        .and_then(|value| value.strip_suffix(']'))
    {
        return Some(id.trim().trim_start_matches("id=").trim().to_string());
    }
    if text.starts_with("2026_D") && text.contains("_Q") {
        return Some(text.to_string());
    }
    let id = text.strip_prefix("::")?.strip_suffix("::")?;
    (!id.trim().is_empty()).then(|| id.trim().to_string())
}

/// Converts a paragraph to an answer when it contains visible inline content.
///
/// A trailing `(*)` marks the answer correct and is removed from its final
/// text inline. A leading `~%0%` GIFT token is removed but does not mark the
/// answer correct. The remaining inline fragments, including styles and
/// images, are preserved in source order.
fn parse_answer(paragraph: &Paragraph) -> Option<Answer> {
    let text = paragraph.plain_text();
    let correct = text.trim().ends_with("(*)");
    let mut inlines = paragraph.inlines.clone();
    if correct {
        trim_text_suffix(&mut inlines, "(*)");
    }
    if text.trim_start().starts_with("~%0%") {
        trim_text_prefix(&mut inlines, "~%0%");
    }
    (!inlines.is_empty()).then_some(Answer { inlines, correct })
}

/// Removes the last occurrence of a marker from the final text inline.
///
/// This targets the final inline because correctness markers are suffixes;
/// image inlines or non-text content are left untouched. If the suffix is not
/// present in that text fragment, the slice is unchanged.
fn trim_text_suffix(inlines: &mut [Inline], suffix: &str) {
    if let Some(Inline::Text { value, .. }) = inlines.last_mut() {
        if let Some(index) = value.rfind(suffix) {
            value.truncate(index);
        }
    }
}

/// Removes the first occurrence of a marker from the first text inline.
///
/// Earlier image inlines are skipped, allowing a prefix token to be removed
/// from the first textual fragment while retaining all other inline content.
/// If no text fragment contains the prefix, nothing changes.
fn trim_text_prefix(inlines: &mut [Inline], prefix: &str) {
    if let Some(Inline::Text { value, .. }) = inlines
        .iter_mut()
        .find(|inline| matches!(inline, Inline::Text { .. }))
    {
        if let Some(index) = value.find(prefix) {
            value.replace_range(index..index + prefix.len(), "");
        }
    }
}
