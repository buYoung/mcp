use super::structure::{callable, Outline};
use crate::callers::{AnnotationRequest, CallerConfig};
use crate::parser::ExtractedFile;
use std::collections::{BTreeMap, BTreeSet};

struct Selection {
    symbols: BTreeSet<usize>,
    relation_headers: BTreeSet<usize>,
    relations: BTreeMap<usize, String>,
    references: BTreeMap<usize, String>,
}

fn add_chain(
    outline: &Outline,
    i: usize,
    chosen: &mut BTreeSet<usize>,
    used: &mut usize,
    cap: usize,
    extra: usize,
) -> bool {
    let chain = outline.chain(i);
    let cost = chain
        .iter()
        .filter(|j| !chosen.contains(j))
        .map(|&j| outline.rows[j].len())
        .sum::<usize>()
        + extra;
    if *used + cost > cap {
        return false;
    }
    chosen.extend(chain);
    *used += cost;
    true
}

pub(super) fn render(outlines: &[Outline], snapshot: &[ExtractedFile], cap: usize) -> String {
    let mut selections = Vec::new();
    let mut symbol_used = 0;
    let mut symbol_omitted = 0;
    for outline in outlines {
        let mut chosen = BTreeSet::new();
        for &i in &outline.order {
            if outline.selected.contains(&i)
                && !add_chain(outline, i, &mut chosen, &mut symbol_used, cap, 0)
            {
                symbol_omitted += 1;
            }
        }
        selections.push(Selection {
            symbols: chosen,
            relation_headers: BTreeSet::new(),
            relations: BTreeMap::new(),
            references: BTreeMap::new(),
        });
    }
    let cfg = crate::config::get();
    let selected_symbols: Vec<Vec<_>> = outlines
        .iter()
        .map(|o| {
            o.order
                .iter()
                .filter(|i| o.selected.contains(i) && callable(&o.file.symbols[**i]))
                .map(|&i| o.file.symbols[i].clone())
                .collect()
        })
        .collect();
    let requests: Vec<_> = outlines
        .iter()
        .zip(&selected_symbols)
        .map(|(o, s)| AnnotationRequest {
            file_path: &o.file.file_path,
            symbols: s,
            is_fallback: false,
        })
        .collect();
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
    let root = std::env::current_dir().unwrap_or_default();
    let annotations =
        crate::callers::annotate_results(&requests, snapshot, &caller_cfg, cap, &root);
    let mut relation_used = 0;
    let mut relation_omitted = 0;
    let mut relation_targets = 0;
    let mut reference_omitted = 0;
    {
        for (outline, selection) in outlines.iter().zip(&mut selections) {
            for &i in &outline.order {
                if !outline.selected.contains(&i) || !callable(&outline.file.symbols[i]) {
                    continue;
                }
                let symbol = &outline.file.symbols[i];
                relation_targets += 1;
                let detail = annotations
                    .as_ref()
                    .and_then(|a| {
                        a.render_live_relations(&outline.file.file_path, symbol.range.start_line)
                    })
                    .unwrap_or_else(|| {
                        "  - [Caller/callee unavailable or omitted by scan/output budget.]\n"
                            .to_string()
                    });
                let indent = "  ".repeat(outline.chain(i).len() - 1);
                let indented = detail
                    .lines()
                    .map(|line| format!("{indent}{line}\n"))
                    .collect::<String>();
                if add_chain(
                    outline,
                    i,
                    &mut selection.relation_headers,
                    &mut relation_used,
                    cap,
                    indented.len(),
                ) {
                    selection.relations.insert(i, indented);
                } else {
                    relation_omitted += 1;
                }
                if let Some(rows) = outline.references.get(&i) {
                    let header =
                        format!("{indent}  - _references (constants; same-file or explicit source import):_\n");
                    let mut references = String::new();
                    for row in rows {
                        let row = format!("{indent}{row}");
                        let extra = row.len()
                            + if references.is_empty() {
                                header.len()
                            } else {
                                0
                            };
                        if add_chain(
                            outline,
                            i,
                            &mut selection.relation_headers,
                            &mut relation_used,
                            cap,
                            extra,
                        ) {
                            if references.is_empty() {
                                references.push_str(&header);
                            }
                            references.push_str(&row);
                        } else {
                            reference_omitted += 1;
                        }
                    }
                    selection.references.insert(i, references);
                }
            }
        }
    }
    let mut out="Scope: enclosing declarations and members. Declaration locations and access are shown below. Verify behavior in # results.\n\n".to_string();
    let mut emitted = 0;
    for (outline, selection) in outlines.iter().zip(&selections) {
        for &i in &outline.order {
            let show = selection.symbols.contains(&i) || selection.relation_headers.contains(&i);
            if !show {
                continue;
            }
            out.push_str(&outline.rows[i]);
            emitted += 1;
            if let Some(detail) = selection.relations.get(&i) {
                out.push_str(detail);
            }
            if let Some(references) = selection.references.get(&i) {
                out.push_str(references);
            }
        }
    }
    if emitted == 0 {
        out.push_str("[No indexed declaration or callable identified for this region.]\n");
    }
    if symbol_omitted > 0 {
        out.push_str(&format!(
            "[Symbol output cap: {symbol_omitted} entries not shown.]\n"
        ));
    }
    if relation_targets == 0 {
        out.push_str("[No callable identified for caller/callee lookup.]\n");
    }
    if relation_omitted > 0 {
        out.push_str(&format!(
            "[Relation output cap: {relation_omitted} targets not shown.]\n"
        ));
    }
    if reference_omitted > 0 {
        out.push_str(&format!(
            "[Reference output cap: {reference_omitted} entries not shown.]\n"
        ));
    }
    out
}
