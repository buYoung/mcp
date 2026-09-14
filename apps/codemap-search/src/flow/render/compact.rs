use super::*;
use std::collections::BTreeMap;

fn name(value: &str) -> String {
    value
        .chars()
        .take(120)
        .collect::<String>()
        .replace(['\n', '\r'], " ")
}

fn passing_target(value: &str) -> String {
    let value = value.split_once(" of ").map_or(value, |(_, target)| target);
    name(
        value
            .trim_start_matches("call ")
            .trim_start_matches("call ")
            .rsplit(['.', ':'])
            .next()
            .unwrap_or(value),
    )
}

struct Row {
    text: String,
    lines: BTreeSet<usize>,
    keys: Vec<String>,
}

pub(super) fn render(
    query: &Query<'_>,
    cap: usize,
    current_file: Option<&str>,
    mut budget: Option<&mut super::super::RequestBudget>,
    anchors: Option<&[(String, usize, usize)]>,
) -> String {
    if cap < 256 {
        return String::new();
    }
    let mut groups: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    let mut is_partial = budget.is_none() && query.work_limit.is_some();
    let mut unresolved = 0usize;
    let mut steps = query
        .steps
        .iter()
        .filter(|step| {
            query.is_relevant
                && step.is_interesting
                && step.relation != "opaque result → member use"
                && (step.relation != "returned value → call result"
                    || step.to.name.starts_with("call "))
                && anchors.is_none_or(|anchors| {
                    super::super::index::overlaps(&step.from, anchors)
                        || super::super::index::overlaps(&step.to, anchors)
                })
        })
        .collect::<Vec<_>>();
    steps.sort_by_key(|step| {
        (
            step.relation != "possible callback invocation",
            !(super::super::index::overlaps(&step.from, &query.anchors)
                || super::super::index::overlaps(&step.to, &query.anchors)),
        )
    });
    is_partial |= steps.len() > 128;
    for step in steps.into_iter().take(128) {
        if step.evidence == Evidence::Candidate && !query.should_list_unresolved {
            unresolved += 1;
            continue;
        }
        let key = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            step.relation,
            step.from.path,
            step.from.range.start_line,
            step.from.range.start_col,
            step.to.path,
            step.to.range.start_line,
            step.to.range.start_col
        );
        if budget
            .as_ref()
            .is_some_and(|budget| budget.shown.contains_key(&key))
        {
            continue;
        }
        let candidate = if step.evidence == Evidence::Candidate {
            "[candidate] "
        } else if step.evidence == Evidence::Model {
            "[model] "
        } else {
            ""
        };
        let from = if step.from.path == step.to.path {
            name(&step.from.name)
        } else {
            display(&step.from, Some(&step.to.path))
        };
        let text = if step.relation == "function value passed" {
            let from = if step.from.name == "<closure>" && step.from.path == step.to.path {
                "익명 함수".into()
            } else {
                from
            };
            format!(
                "{candidate}{from} → {} 인자로 전달",
                passing_target(&step.to.name)
            )
        } else {
            format!(
                "{candidate}{}: {from}{} → {}",
                step.relation,
                if step.from.path == step.to.path {
                    format!(" · L{}", step.from.range.start_line)
                } else {
                    String::new()
                },
                name(&step.to.name)
            )
        };
        let rows = groups.entry(step.to.path.clone()).or_default();
        let row = if let Some(position) = rows.iter().position(|row| row.text == text) {
            &mut rows[position]
        } else {
            rows.push(Row {
                text,
                lines: BTreeSet::new(),
                keys: Vec::new(),
            });
            rows.last_mut().unwrap()
        };
        row.lines.insert(step.to.range.start_line);
        if step.relation == "function value passed" && step.from.path == step.to.path {
            row.lines.insert(step.from.range.start_line);
        }
        row.keys.push(key);
    }
    for diagnostic in &query.diagnostics {
        if anchors
            .is_some_and(|anchors| !super::super::index::overlaps(&diagnostic.location, anchors))
        {
            continue;
        }
        if diagnostic.is_analysis_limit {
            is_partial = true;
        } else if diagnostic.reason.contains("stale") || diagnostic.reason.contains("changed since")
        {
            let row = Row {
                text: format!(
                    "[stale] {}: source changed since indexing",
                    name(&diagnostic.location.name)
                ),
                lines: BTreeSet::from([diagnostic.location.range.start_line]),
                keys: Vec::new(),
            };
            groups
                .entry(diagnostic.location.path.clone())
                .or_default()
                .push(row);
        } else if diagnostic.reason.contains("call target")
            || diagnostic.reason.contains("function parameter")
        {
            unresolved += 1;
        }
    }
    if groups.is_empty() && unresolved == 0 {
        return String::new();
    }
    let mut output = "## Value relationships\n".to_string();
    for (path, rows) in groups {
        let header = if current_file == Some(path.as_str()) {
            String::new()
        } else {
            format!("\n파일: {path}\n")
        };
        let mut has_header = false;
        for row in rows {
            let lines = row
                .lines
                .iter()
                .map(|line| format!("L{line}"))
                .collect::<Vec<_>>()
                .join(", ");
            let line = format!("- {} · {lines}\n", row.text);
            if output.len() + line.len() + if has_header { 0 } else { header.len() } + 160 > cap
                || budget
                    .as_ref()
                    .is_some_and(|budget| budget.shown.len() + row.keys.len() > 128)
            {
                is_partial = true;
                break;
            }
            if !has_header {
                output.push_str(&header);
                has_header = true;
            }
            output.push_str(&line);
            if let Some(budget) = budget.as_mut() {
                budget
                    .shown
                    .extend(row.keys.into_iter().map(|key| (key, path.clone())));
            }
        }
    }
    if unresolved > 0 {
        let note = format!("- 호출 대상 미해결: {unresolved}건.\n");
        if output.len() + note.len() + 100 <= cap {
            output.push_str(&note);
        } else {
            is_partial = true;
        }
    }
    if is_partial {
        if let Some(budget) = budget.as_mut() {
            budget.has_omissions = true;
            return output;
        }
        let note = "\n[분석 제한: 보조 관계 일부 생략. 상세: debug=true.]\n";
        if output.len() + note.len() <= cap {
            output.push_str(note);
        }
    }
    output
}
