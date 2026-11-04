use crate::{Inline, Question, QuestionType};

/// Formats parsed questions and their Moodle answer grades for terminal output.
pub(crate) fn render_questions(questions: &[Question], color: bool) -> String {
    let mut output = String::new();
    for (index, question) in questions.iter().enumerate() {
        let question_type = match question.question_type {
            QuestionType::MultiChoice => "Multiple choice",
            QuestionType::DropDown => "Dropdown",
        };
        output.push_str(&format!(
            "\n{}. {}  ({question_type})\n",
            index + 1,
            question.name
        ));
        for paragraph in &question.prompt {
            output.push_str("   ");
            render_inlines(&paragraph.inlines, &mut output, color);
            output.push('\n');
        }

        if question.question_type == QuestionType::DropDown {
            for answer in &question.answers {
                output.push_str("   • ");
                render_inlines(&answer.inlines, &mut output, color);
                if let Some(value) = &answer.dropdown_value {
                    output.push_str(" → ");
                    render_inlines(value, &mut output, color);
                }
                if color {
                    output.push_str("\x1b[32m");
                }
                output.push_str("  [correct, 100%]");
                if color {
                    output.push_str("\x1b[0m");
                }
                output.push('\n');
            }
            continue;
        }

        let explicit_total: f64 = question
            .answers
            .iter()
            .filter_map(|answer| answer.percentage)
            .sum();
        let unscored_correct = question
            .answers
            .iter()
            .filter(|answer| answer.correct && answer.percentage.is_none())
            .count();
        let distributed = if unscored_correct == 0 {
            0.0
        } else {
            (100.0 - explicit_total) / unscored_correct as f64
        };
        for answer in &question.answers {
            output.push_str(if answer.correct { "   ✓ " } else { "   • " });
            render_inlines(&answer.inlines, &mut output, color);
            if answer.correct {
                let percentage = answer.percentage.unwrap_or(distributed);
                if color {
                    output.push_str("\x1b[32m");
                }
                output.push_str(&format!("  [{}%]", format_percentage(percentage)));
                if color {
                    output.push_str("\x1b[0m");
                }
            } else {
                if color {
                    output.push_str("\x1b[90m");
                }
                output.push_str("  [incorrect]");
                if color {
                    output.push_str("\x1b[0m");
                }
            }
            output.push('\n');
        }
    }
    output
}

fn format_percentage(value: f64) -> String {
    format!("{value:.4}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn render_inlines(inlines: &[Inline], output: &mut String, color: bool) {
    for inline in inlines {
        match inline {
            Inline::Text {
                value,
                bold,
                italic,
            } => {
                if color && *bold {
                    output.push_str("\x1b[1m");
                }
                if color && *italic {
                    output.push_str("\x1b[3m");
                }
                output.push_str(value);
                if color && (*bold || *italic) {
                    output.push_str("\x1b[0m");
                }
            }
            Inline::Image(_) => output.push_str("[image]"),
        }
    }
}
