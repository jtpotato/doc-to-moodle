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

/// An answer, whether it is correct, and its optional explicit score.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// Styled text and images that make up the answer body.
    pub inlines: Vec<Inline>,
    /// Whether the author marked this answer with the `(*)` suffix.
    pub correct: bool,
    /// Explicit percentage for a correct answer, if supplied in the guide format.
    pub percentage: Option<f64>,
    /// Matching value for a labeled matching row.
    pub matching_value: Option<Vec<Inline>>,
}

/// A question type, either multi-choice or drop-down.

#[derive(Debug, Clone, PartialEq)]
pub enum QuestionType {
    /// A multiple-choice question with radio buttons or checkboxes.
    MultiChoice,
    /// A drop-down question with a single correct choice.
    Matching,
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
    /// The type of the question.
    pub question_type: QuestionType,
}

/// Parses marker-delimited paragraphs into validated questions.
///
/// Each recognized marker closes the previous question and starts a fresh
/// one. Paragraphs before `[Answers]` are prompt content; paragraphs after it
/// are choices.
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
                question_type: QuestionType::MultiChoice,
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
            let is_matching_row = text.contains("->");
            if question.answers.is_empty() {
                question.question_type = if is_matching_row {
                    QuestionType::Matching
                } else {
                    QuestionType::MultiChoice
                };
            } else if is_matching_row != (question.question_type == QuestionType::Matching) {
                bail!(
                    "question {} mixes matching and multiple-choice answer formats",
                    question.name
                );
            }
            if let Some(answer) = if is_matching_row {
                parse_matching_answer(&paragraph)?
            } else {
                parse_answer(&paragraph)?
            } {
                question.answers.push(answer);
            }
            continue;
        }
        question.prompt.push(paragraph);
    }
    finish_question(&mut questions, current)?;
    if questions.is_empty() {
        bail!("no question markers found; add ::id:: markers");
    }
    Ok(questions)
}

/// Parses a matching row in `label -> value` form.
fn parse_matching_answer(paragraph: &Paragraph) -> Result<Option<Answer>> {
    let text = paragraph.plain_text();
    let Some((label, value)) = text.split_once("->") else {
        if text.trim().is_empty() {
            return Ok(None);
        }
        bail!("matching answers must use `label -> value`: {text}");
    };
    let label = label.trim();
    let value = value.trim();
    if label.is_empty() || value.is_empty() {
        bail!("matching answers must have a label and value: {text}");
    }
    let mut inlines = paragraph.inlines.clone();
    truncate_inlines_at_text_offset(&mut inlines, text.find("->").unwrap_or(text.len()));
    if let Some(Inline::Text { value: text, .. }) = inlines.last_mut() {
        *text = text.trim_end().to_string();
    }
    Ok(Some(Answer {
        inlines,
        correct: true,
        percentage: None,
        matching_value: Some(vec![Inline::text(value, false, false)]),
    }))
}

/// Keeps inline content before a byte offset in the paragraph's plain text.
fn truncate_inlines_at_text_offset(inlines: &mut Vec<Inline>, offset: usize) {
    let mut text_bytes = 0;
    let mut keep_through = inlines.len();
    for (index, inline) in inlines.iter_mut().enumerate() {
        if let Inline::Text { value, .. } = inline {
            if text_bytes + value.len() >= offset {
                value.truncate(offset.saturating_sub(text_bytes));
                keep_through = index + 1;
                break;
            }
            text_bytes += value.len();
        }
    }
    inlines.truncate(keep_through);
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
        let explicit_total: f64 = question
            .answers
            .iter()
            .filter_map(|answer| answer.percentage)
            .sum();
        if explicit_total > 100.0 {
            bail!(
                "question {} has answer percentages totaling more than 100",
                question.name
            );
        }
        questions.push(question);
    }
    Ok(())
}

/// Recognizes a question marker and returns its trimmed identifier.
/// Trims ``::id::`` to `id`.
fn question_marker(text: &str) -> Option<String> {
    let text = text.trim();
    let id = text.strip_prefix("::")?.strip_suffix("::")?;
    (!id.trim().is_empty()).then(|| id.trim().to_string())
}

/// Converts a paragraph to an answer when it contains visible inline content.
///
/// A trailing `(*)` marks the answer correct and is removed from its final
/// text inline. The remaining inline fragments, including styles and images,
/// are preserved in source order.
fn parse_answer(paragraph: &Paragraph) -> Result<Option<Answer>> {
    let mut answer_text = paragraph.plain_text().trim_end().to_string();
    let score = find_score_if_exists(&answer_text)?;
    if let Some(score) = &score {
        answer_text.truncate(score.start);
    }
    let correct = is_correct(&answer_text);
    if score.is_some() && !correct {
        bail!("an answer percentage requires the answer to be marked (*)");
    }

    let mut inlines = paragraph.inlines.clone();
    if let Some(score) = &score {
        trim_text_suffix(&mut inlines, &score.suffix);
    }
    if correct {
        trim_text_suffix(&mut inlines, "(*)");
    }
    Ok((!inlines.is_empty()).then_some(Answer {
        inlines,
        correct,
        percentage: score.map(|score| score.percentage),
        matching_value: None,
    }))
}

struct AnswerScore {
    start: usize,
    /// The exact trailing score marker, including its leading space (for example, ` (75%)`).
    /// The parser removes this from the answer text before exporting it.
    suffix: String,
    percentage: f64,
}

/// Finds and validates a trailing `(percentage%)` score marker.
fn find_score_if_exists(answer_text: &str) -> Result<Option<AnswerScore>> {
    let Some(start) = answer_text.rfind(" (") else {
        return Ok(None);
    };
    let Some(number) = answer_text[start + 2..].strip_suffix(')') else {
        return Ok(None);
    };
    let Some(number) = number.strip_suffix('%') else {
        return Ok(None);
    };
    let Ok(percentage) = number.parse::<f64>() else {
        return Ok(None);
    };
    if !(0.0..=100.0).contains(&percentage) {
        bail!("answer percentage must be between 0 and 100: {percentage}");
    }
    Ok(Some(AnswerScore {
        start,
        suffix: answer_text[start..].to_string(),
        percentage,
    }))
}

/// Reports whether an answer ends with the guide's correctness marker.
fn is_correct(answer_text: &str) -> bool {
    answer_text.trim_end().ends_with("(*)")
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
