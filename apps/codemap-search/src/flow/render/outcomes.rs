use super::*;
use crate::flow::outcome::{Condition, ConditionalOutcome, ReturnedValue, ValueType};

fn value(fact: &ReturnedValue, is_debug: bool) -> String {
    match fact {
        ReturnedValue::Constant(super::super::Constant::Number(number)) => number.clone(),
        ReturnedValue::Constant(constant) => match constant {
            super::super::Constant::String(string) => {
                if is_debug {
                    format!("{string:?}")
                } else {
                    "…".into()
                }
            }
            super::super::Constant::Boolean(value) => value.to_string(),
            super::super::Constant::Null => "null".into(),
            super::super::Constant::Number(_) => unreachable!(),
        },
        ReturnedValue::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    if !is_debug && i > 0 {
                        "…".into()
                    } else {
                        value(item, is_debug)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ReturnedValue::Variant { name, value: inner } => {
            format!("{name}({})", value(inner, is_debug))
        }
        ReturnedValue::Unknown => "…".into(),
    }
}

pub(super) fn append<'a>(
    query: &'a Query<'_>,
    output: &mut String,
    cap: usize,
    current_file: Option<&str>,
    mut budget: Option<&mut super::super::RequestBudget>,
    anchors: Option<&[(String, usize, usize)]>,
    should_debug: bool,
) -> Vec<&'a ConditionalOutcome> {
    let mut shown = Vec::new();
    for outcome in query
        .outcomes
        .iter()
        .filter(|outcome| anchors.is_none_or(|anchors| outcome.overlaps(anchors)))
    {
        if budget
            .as_ref()
            .is_some_and(|budget| budget.shown.contains_key(&outcome.key()))
        {
            shown.push(outcome);
            continue;
        }
        let Condition::NotType {
            subject,
            expected: ValueType::JsonObject,
        } = &outcome.condition;
        let returned = if matches!(outcome.returned_value, ReturnedValue::Unknown) {
            "반환값 미확정".into()
        } else {
            value(&outcome.returned_value, should_debug)
        };
        let status = match outcome.certainty {
            Evidence::Model => "model",
            Evidence::Source => "source",
            Evidence::Candidate => "candidate",
        };
        let start = outcome.location.range.start_line;
        let end = outcome.location.range.end_line_inclusive();
        let range = if start == end {
            format!("L{start}")
        } else {
            format!("L{start}–L{end}")
        };
        let at = if current_file == Some(outcome.location.path.as_str()) {
            range
        } else {
            format!("{}:{range}", outcome.location.path)
        };
        let limitation = outcome
            .limitation
            .as_ref()
            .map(|reason| format!(" ({reason})"))
            .unwrap_or_default();
        let action = if outcome.is_early_return {
            " 조기 반환"
        } else {
            ""
        };
        let row =
            format!("- [{status}] JSON 객체가 아닌 입력 → {returned}{action}{limitation} · {at}\n");
        if output.len() + row.len() + 100 > cap
            || budget
                .as_ref()
                .is_some_and(|budget| budget.shown.len() >= 128)
        {
            if let Some(budget) = budget.as_mut() {
                budget.has_omissions = true;
            }
            if should_debug {
                let note = "- [analysis limit] additional conditional outcomes omitted by the output cap.\n";
                if output.len() + note.len() <= cap {
                    output.push_str(note);
                }
            }
            break;
        }
        output.push_str(&row);
        shown.push(outcome);
        if let Some(budget) = budget.as_mut() {
            budget
                .shown
                .insert(outcome.key(), outcome.location.path.clone());
        }
        if should_debug {
            let note = format!("  - conditional contract for `{}`; the factory is evaluated only on the None path. This is not an execution observation.\n", subject.name);
            if output.len() + note.len() + 100 <= cap {
                output.push_str(&note);
            }
            for evidence in &outcome.evidence {
                let row = format!("  - proof: {}\n", display(evidence, current_file));
                if output.len() + row.len() + 100 > cap {
                    break;
                }
                output.push_str(&row);
            }
        }
    }
    shown
}
