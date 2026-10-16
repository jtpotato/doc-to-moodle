use anyhow::{bail, Result};

/// A piece of question content, either styled text or an embedded image.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text {
        value: String,
        bold: bool,
        italic: bool,
    },
    Image(String),
}

impl Inline {
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
    pub inlines: Vec<Inline>,
}

impl Paragraph {
    /// Returns whether the paragraph contains no visible text or images.
    pub fn is_empty(&self) -> bool {
        self.inlines.iter().all(|inline| match inline {
            Inline::Text { value, .. } => value.trim().is_empty(),
            Inline::Image(_) => false,
        })
    }

    /// Concatenates text in the paragraph, excluding image placeholders.
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
    pub inlines: Vec<Inline>,
    pub correct: bool,
}

/// A parsed multiple-choice question before Moodle XML rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub name: String,
    pub prompt: Vec<Paragraph>,
    pub answers: Vec<Answer>,
}

/// Parses marker-delimited paragraphs into validated questions.
///
/// The explicit `[Answers]` token takes precedence over the legacy
/// question-sentence heuristic.
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

fn finish_question(questions: &mut Vec<Question>, question: Option<Question>) -> Result<()> {
    if let Some(question) = question {
        if question.answers.is_empty() {
            bail!("question {} has no answers", question.name);
        }
        questions.push(question);
    }
    Ok(())
}

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

fn trim_text_suffix(inlines: &mut [Inline], suffix: &str) {
    if let Some(Inline::Text { value, .. }) = inlines.last_mut() {
        if let Some(index) = value.rfind(suffix) {
            value.truncate(index);
        }
    }
}

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
