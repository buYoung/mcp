use super::structure::{callable, Outline};
use crate::callers::{AnnotationRequest, CallerConfig, DetailAnnotations};
use crate::parser::ExtractedFile;
use crate::tools::live_options::{LiveOptions, LiveView};
use std::collections::{BTreeMap, BTreeSet};

/// One workspace scan, interleaving requested callables across files so a large
/// first class cannot reserve every annotation before another file is considered.
pub(super) fn annotate(
    outlines: &[Outline],
    snapshot: &[ExtractedFile],
    cap: usize,
    options: LiveOptions,
    root: &std::path::Path,
    resolver: &crate::callers::resolution::SourceResolver<'_>,
) -> Option<DetailAnnotations> {
    if !options.should_include_relations() || cap < 128 {
        return None;
    }
    let symbols: Vec<Vec<_>> = outlines
        .iter()
        .map(|outline| {
            outline
                .order
                .iter()
                .copied()
                .filter(|i| outline.focused.contains(i) && callable(&outline.file.symbols[*i]))
                .map(|i| outline.file.symbols[i].clone())
                .collect()
        })
        .collect();
    let mut requests = Vec::new();
    for position in 0..symbols.iter().map(Vec::len).max().unwrap_or(0) {
        for (outline, symbols) in outlines.iter().zip(&symbols) {
            if let Some(symbol) = symbols.get(position) {
                requests.push(AnnotationRequest {
                    file_path: &outline.file.file_path,
                    symbols: std::slice::from_ref(symbol),
                    is_fallback: false,
                });
            }
        }
    }
    if requests.is_empty() {
        return None;
    }
    let cfg = crate::config::get();
    let caller_cfg = CallerConfig {
        scan_cap: cfg.scan_cap,
        caller_list_cap: cfg.caller_list_cap,
        callee_list_cap: cfg.callee_list_cap,
        annotation_sub_budget: cfg.annotation_sub_budget,
        common_name_threshold: cfg.common_name_threshold,
        caller_omit_def_threshold: cfg.caller_omit_def_threshold,
        max_file_size: cfg.max_file_size,
        navigation_context_default: cfg.navigation_context_default,
        navigation_callsite_budget: cfg.navigation_callsite_budget,
        navigation_store_references: cfg.navigation_store_references,
    };
    crate::callers::annotate_live_results(
        &requests,
        snapshot,
        &caller_cfg,
        cap,
        root,
        options.should_list_unresolved,
        resolver,
    )
}

fn select_chain(
    outline: &Outline,
    i: usize,
    chosen: &mut BTreeSet<usize>,
    used: &mut usize,
    cap: usize,
) -> bool {
    let chain = outline.chain(i);
    let cost: usize = chain
        .iter()
        .filter(|j| !chosen.contains(j))
        .map(|&j| outline.rows[j].len())
        .sum();
    if *used + cost > cap {
        return false;
    }
    chosen.extend(chain);
    *used += cost;
    true
}

fn bounded_detail(text: &str, cap: usize) -> String {
    let mut output = String::new();
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let required = line.len()
            + if line.trim_end().ends_with(":_") {
                lines.peek().map_or(0, |next| next.len())
            } else {
                0
            };
        if output.len() + required > cap {
            break;
        }
        output.push_str(line);
    }
    output
}

pub(super) fn render(
    outline: &Outline,
    annotations: Option<&DetailAnnotations>,
    cap: usize,
    options: LiveOptions,
    hints: &mut super::ReadHints<'_>,
) -> String {
    if outline.selected.is_empty() {
        return super::bounded_notice(
            &super::diagnostics::no_declaration(
                &outline.file,
                outline.anchor_line,
                options.view == LiveView::Relations,
            ),
            cap,
        );
    }
    let path = &outline.file.file_path;
    let mut chosen = BTreeSet::new();
    let mut used = 0;
    let mut details = BTreeMap::new();
    let mut omitted_symbols = 0;
    let mut omitted_relations = 0;
    let mut next_line = None;
    // Keep omission counts, with an optional unseen declaration as continuation.
    let reserve = (path.len() + 220).min(cap);
    let content_cap = cap.saturating_sub(reserve);
    let focused: Vec<_> = outline
        .order
        .iter()
        .copied()
        .filter(|i| outline.selected.contains(i) && outline.focused.contains(i))
        .collect();
    for &i in &focused {
        if !select_chain(outline, i, &mut chosen, &mut used, content_cap) {
            omitted_symbols += 1;
            let line = outline.file.symbols[i].range.start_line;
            if hints.next(path, line).is_some() {
                next_line.get_or_insert(line);
            }
        }
    }
    let callables: Vec<_> = focused
        .iter()
        .copied()
        .filter(|i| chosen.contains(i) && callable(&outline.file.symbols[*i]))
        .collect();
    if options.should_include_relations() {
        for (position, &i) in callables.iter().enumerate() {
            let symbol = &outline.file.symbols[i];
            let indent = "  ".repeat(outline.chain(i).len().saturating_sub(1));
            let detail = if outline.file.macro_expansion().is_some() {
                "  - [Call and constant-reference attribution unresolved after preprocessing.]\n"
                    .into()
            } else {
                annotations
                    .and_then(|annotations| {
                        annotations.render_live_relations(
                            path,
                            symbol.range.start_line,
                            Some(symbol.range.start_col),
                        )
                    })
                    .unwrap_or_else(|| {
                        "  - [Caller/callee unavailable or omitted by scan/output budget.]\n".into()
                    })
            };
            let detail = super::locations::calls(&detail, path);
            let mut detail = detail
                .lines()
                .map(|line| format!("{indent}{line}\n"))
                .collect::<String>();
            if let Some(rows) = outline
                .references
                .get(&i)
                .filter(|_| outline.file.macro_expansion().is_none())
            {
                detail.push_str(&format!(
                    "{indent}  - _references (constants; same-file or explicit source import):_\n"
                ));
                for row in rows {
                    detail.push_str(&indent);
                    detail.push_str(&super::locations::reference(row, path));
                }
            }
            let share = content_cap.saturating_sub(used) / (callables.len() - position);
            let bounded = bounded_detail(&detail, share);
            if bounded.len() < detail.len() {
                omitted_relations += 1;
            }
            used += bounded.len();
            details.insert(i, bounded);
        }
    }
    // Unmatched siblings provide compact owner/member context only. Their bodies,
    // call graphs and references do not compete with directly requested symbols.
    if options.view != LiveView::Relations {
        for &i in &outline.order {
            if !outline.selected.contains(&i) || outline.focused.contains(&i) || chosen.contains(&i)
            {
                continue;
            }
            if !select_chain(outline, i, &mut chosen, &mut used, content_cap) {
                omitted_symbols += 1;
                let line = outline.file.symbols[i].range.start_line;
                if hints.next(path, line).is_some() {
                    next_line.get_or_insert(line);
                }
            }
        }
    }
    let mut output = String::new();
    for &i in &outline.order {
        if chosen.contains(&i) {
            output.push_str(&outline.rows[i]);
            if let Some(detail) = details.get(&i) {
                output.push_str(detail);
            }
        }
    }
    if omitted_symbols > 0 || omitted_relations > 0 {
        let mut notice = format!("[Symbol context budget: {omitted_symbols} declarations omitted; {omitted_relations} relation groups omitted or partial.");
        if let Some((line, next)) =
            next_line.and_then(|line| hints.next(path, line).map(|next| (line, next)))
        {
            if output.len() + notice.len() + next.len() + 10 <= cap {
                notice.push_str(&format!(" Next: {next}"));
                hints.record(path, line);
            }
        }
        notice.push_str("]\n");
        output.push_str(&super::bounded_notice(
            &notice,
            cap.saturating_sub(output.len()),
        ));
    }
    output
}
