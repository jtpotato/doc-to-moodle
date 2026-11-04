use crate::{Answer, Inline, Question, QuestionType};

/// Formats parsed questions and their Moodle answer grades for terminal output.
pub(crate) fn render_questions(questions: &[Question], color: bool) -> String {
    questions
        .iter()
        .enumerate()
        .map(|(index, question)| render_question(index + 1, question, color))
        .collect()
}

fn render_question(index: usize, question: &Question, color: bool) -> String {
    let question_type = match question.question_type {
        QuestionType::MultiChoice => "Multiple choice",
        QuestionType::Matching => "Matching",
    };
    let prompt = question
        .prompt
        .iter()
        .map(|paragraph| format!("   {}", render_inlines(&paragraph.inlines, color)))
        .collect::<Vec<_>>()
        .join("\n");
    let answers = match question.question_type {
        QuestionType::Matching => render_matching_answers(question, color),
        QuestionType::MultiChoice => render_multiple_choice_answers(question, color),
    }
    .join("\n");

    format!(
        "\n{index}. {}  ({question_type})\n{prompt}\n{answers}\n",
        question.name
    )
}

fn render_matching_answers(question: &Question, color: bool) -> Vec<String> {
    let points_per_pair = 100.0 / question.answers.len() as f64;
    question
        .answers
        .iter()
        .map(|answer| render_matching_answer(answer, points_per_pair, color))
        .collect()
}

fn render_matching_answer(answer: &Answer, points_per_pair: f64, color: bool) -> String {
    let label = answer
        .matching_value
        .as_deref()
        .map(|value| format!(" → {}", render_inlines(value, color)))
        .unwrap_or_default();

    format!(
        "   • {}{label}{}",
        render_inlines(&answer.inlines, color),
        styled(
            &format!("  [correct, {}%]", format_percentage(points_per_pair)),
            "\x1b[32m",
            color,
        )
    )
}

fn render_multiple_choice_answers(question: &Question, color: bool) -> Vec<String> {
    let explicit_total = question
        .answers
        .iter()
        .filter_map(|answer| answer.percentage)
        .sum::<f64>();
    let unscored_correct = question
        .answers
        .iter()
        .filter(|answer| answer.correct && answer.percentage.is_none())
        .count();
    let distributed = match unscored_correct {
        0 => 0.0,
        count => (100.0 - explicit_total) / count as f64,
    };

    question
        .answers
        .iter()
        .map(|answer| render_multiple_choice_answer(answer, distributed, color))
        .collect()
}

fn render_multiple_choice_answer(answer: &Answer, distributed: f64, color: bool) -> String {
    let (marker, grade) = if answer.correct {
        let percentage = answer.percentage.unwrap_or(distributed);
        (
            "✓",
            styled(
                &format!("  [{}%]", format_percentage(percentage)),
                "\x1b[32m",
                color,
            ),
        )
    } else {
        ("•", styled("  [incorrect]", "\x1b[90m", color))
    };

    format!(
        "   {marker} {}{grade}",
        render_inlines(&answer.inlines, color)
    )
}

fn styled(value: &str, color_code: &str, color: bool) -> String {
    if color {
        format!("{color_code}{value}\x1b[0m")
    } else {
        value.to_owned()
    }
}

fn format_percentage(value: f64) -> String {
    format!("{value:.4}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn render_inlines(inlines: &[Inline], color: bool) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            Inline::Text {
                value,
                bold,
                italic,
            } => {
                let bold_start = if color && *bold { "\x1b[1m" } else { "" };
                let italic_start = if color && *italic { "\x1b[3m" } else { "" };
                let reset = if color && (*bold || *italic) {
                    "\x1b[0m"
                } else {
                    ""
                };
                format!("{bold_start}{italic_start}{value}{reset}")
            }
            Inline::Image(_) => "[image]".to_owned(),
        })
        .collect()
}
