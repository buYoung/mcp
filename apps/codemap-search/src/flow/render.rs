mod compact;
mod outcomes;
use super::evaluate::{Evidence, Query};
use super::index::Location;
use std::collections::BTreeSet;

fn display(location: &Location, current_file: Option<&str>) -> String {
    let name = location
        .name
        .chars()
        .take(120)
        .collect::<String>()
        .replace(['\n', '\r'], " ");
    format!(
        "{name} — {}",
        crate::locations::display(&location.path, location.range.start_line, current_file)
    )
}
fn diagnostic_key(diagnostic: &super::evaluate::Diagnostic) -> String {
    let location = &diagnostic.location;
    format!(
        "{}:{}:{}:{}:{}:{}",
        location.path,
        location.range.start_line,
        location.range.start_col,
        location.range.end_line,
        location.range.end_col,
        diagnostic.reason
    )
}

#[cfg(test)]
pub(super) fn render(query: &Query<'_>, cap: usize) -> String {
    render_with_context(query, cap, None, None, None, true)
}

pub(super) fn render_with_context(
    query: &Query<'_>,
    cap: usize,
    current_file: Option<&str>,
    mut budget: Option<&mut super::RequestBudget>,
    section_anchors: Option<&[(String, usize, usize)]>,
    should_debug: bool,
) -> String {
    if !should_debug {
        return compact::render(query, cap, current_file, budget, section_anchors);
    }
    let mut diagnostics = query
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            section_anchors
                .is_none_or(|anchors| super::index::overlaps(&diagnostic.location, anchors))
        })
        .filter(|diagnostic| {
            !budget.as_ref().is_some_and(|budget| {
                budget
                    .shown_diagnostics
                    .contains(&diagnostic_key(diagnostic))
            })
        })
        .collect::<Vec<_>>();
    let work_limit = query.work_limit.as_ref().filter(|_| budget.is_none());
    if cap < 256 || !query.is_relevant && diagnostics.is_empty() && work_limit.is_none() {
        return String::new();
    }
    let mut output = if query.is_relevant && cap < 768 {
        "## Value relationships\n".to_string()
    } else if query.is_relevant {
        "## Value relationships\n\n_Static source/model relationships and unresolved candidates. Function passing is separate from invocation; runtime delivery/order is not guaranteed._\n".to_string()
    } else {
        "## Analysis diagnostics\n\n".into()
    };
    let header_len = output.len();
    outcomes::append(
        query,
        &mut output,
        cap,
        current_file,
        budget.as_deref_mut(),
        section_anchors,
        true,
    );
    let mut seen = BTreeSet::new();
    let mut omitted = None;
    let mut capped_rows = None;
    let mut unresolved_rows = diagnostics.len();
    let mut shared_files = BTreeSet::new();
    let mut append = |row: String, location: &Location, reserve: usize| {
        if !seen.insert(row.clone()) {
            return true;
        }
        if output.len() + row.len() + reserve > cap {
            omitted.get_or_insert_with(|| location.clone());
            return false;
        }
        output.push_str(&row);
        true
    };
    if query.is_relevant {
        let mut steps = query
            .steps
            .iter()
            .filter(|step| {
                section_anchors.is_none_or(|anchors| {
                    super::index::overlaps(&step.from, anchors)
                        || super::index::overlaps(&step.to, anchors)
                })
            })
            .filter(|step| {
                step.is_interesting
                    || (step.relation == "source-resolved call"
                        && super::index::overlaps(&step.to, &query.anchors))
            })
            .filter(|step| {
                step.relation != "opaque result → member use"
                    || super::index::overlaps(&step.to, &query.anchors)
            })
            .filter(|step| {
                let unresolved = step.evidence == Evidence::Candidate
                    || matches!(
                        step.relation.as_str(),
                        "opaque result → member use" | "argument passed" | "function value passed"
                    );
                if unresolved && !query.should_list_unresolved {
                    unresolved_rows += 1;
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>();
        steps.sort_by_key(|step| {
            (
                step.relation != "possible callback invocation",
                !(super::index::overlaps(&step.from, &query.anchors)
                    || super::index::overlaps(&step.to, &query.anchors)),
                !step.is_interesting,
            )
        });
        capped_rows = steps.get(128).map(|step| step.to.clone());
        for step in steps.into_iter().take(128) {
            let state = match step.evidence {
                Evidence::Source => "source",
                Evidence::Model => "model",
                Evidence::Candidate => "candidate",
            };
            let detail = step
                .detail
                .as_ref()
                .map(|detail| format!(" ({detail})"))
                .unwrap_or_default();
            let row = format!(
                "- [{state}] {}: {} → {}{detail}\n",
                step.relation,
                display(&step.from, current_file),
                display(&step.to, current_file)
            );
            let canonical = format!(
                "[{state}] {}: {} → {}{detail}",
                step.relation,
                display(&step.from, None),
                display(&step.to, None)
            );
            if let Some(previous_file) = budget
                .as_ref()
                .and_then(|budget| budget.shown.get(&canonical))
            {
                if Some(previous_file.as_str()) != current_file {
                    shared_files.insert(previous_file.clone());
                }
                continue;
            }
            if budget
                .as_ref()
                .is_some_and(|budget| budget.shown.len() >= 128)
            {
                capped_rows = Some(step.to.clone());
                break;
            }
            // Keep diagnostic/continuation space even when relationships are plentiful.
            let reserve = 256
                + if query.should_list_unresolved && !diagnostics.is_empty() {
                    (cap / 3).min(900)
                } else {
                    0
                };
            if !append(row, &step.to, reserve) {
                break;
            }
            if let (Some(budget), Some(path)) = (budget.as_mut(), current_file) {
                budget.shown.insert(canonical, path.into());
            }
        }
    }
    if let Some(location) = query.steps.first().map(|step| &step.to) {
        for path in shared_files {
            if !append(
                format!("- shared value relationships: see the `{path}` file section above.\n"),
                location,
                256,
            ) {
                break;
            }
        }
    }
    diagnostics
        .sort_by_key(|diagnostic| !super::index::overlaps(&diagnostic.location, &query.anchors));
    if query.should_list_unresolved && diagnostics.len() > 3 {
        capped_rows.get_or_insert_with(|| diagnostics[3].location.clone());
    }
    for diagnostic in diagnostics
        .iter()
        .filter(|_| query.should_list_unresolved)
        .take(3)
    {
        let status = if diagnostic.is_analysis_limit {
            "analysis limit"
        } else {
            "unresolved"
        };
        let row = format!(
            "- [{status}] {}: {}.\n",
            display(&diagnostic.location, current_file),
            diagnostic.reason,
        );
        if !append(row, &diagnostic.location, 256) {
            break;
        }
        if let Some(budget) = budget.as_mut() {
            budget.shown_diagnostics.insert(diagnostic_key(diagnostic));
        }
    }
    if !query.should_list_unresolved && unresolved_rows > 0 {
        if let Some(location) = query
            .diagnostics
            .first()
            .map(|diagnostic| &diagnostic.location)
            .or_else(|| query.steps.first().map(|step| &step.to))
        {
            if append(format!("- {unresolved_rows} unresolved value relationships/diagnostics (names omitted).\n"), location, 256) {
                if let Some(budget) = budget.as_mut() {
                    budget.shown_diagnostics.extend(diagnostics.iter().map(|diagnostic| diagnostic_key(diagnostic)));
                }
            }
        }
    }
    if let Some((reason, location)) = work_limit {
        let at = if query.should_list_unresolved {
            format!(" — {}", display(location, current_file))
        } else {
            String::new()
        };
        let note = format!(
            "\n[Partial value analysis: {reason}{at}; additional relationships may be missing.]\n"
        );
        if output.len() + note.len() <= cap {
            output.push_str(&note);
        } else {
            let note = format!("\n[Partial value analysis: {reason}.]\n");
            if output.len() + note.len() <= cap {
                output.push_str(&note);
            }
        }
    }
    if omitted.or(capped_rows).is_some() {
        let note = "\n[Value-flow output budget reached; additional relationships/diagnostics omitted. Narrow the source window.]\n";
        if output.len() + note.len() <= cap {
            output.push_str(note);
        }
    }
    if output.len() == header_len {
        String::new()
    } else {
        output
    }
}
