use super::{
    diagnostics, render, structure, LiveOutput, OUTLINED_FILE_LIMIT, TEST_CONTEXT_EXCLUDED_NOTICE,
};
use crate::index::EngineSupervisor;
use crate::tools::live_options::LiveOptions;
use std::collections::BTreeMap;

pub(super) fn build(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    cap: usize,
    options: LiveOptions,
) -> (Vec<String>, String) {
    let mut contexts = vec![String::new(); output.files.len()];
    let count = output.files.len().min(OUTLINED_FILE_LIMIT);
    if count == 0 || cap < 128 {
        return (contexts, String::new());
    }
    let notice_cap = if options.should_include_relations() {
        cap.min(super::SHARED_NOTICE_CAP)
    } else {
        0
    };
    let cap = cap.saturating_sub(notice_cap);
    let root = std::env::current_dir().unwrap_or_default();
    let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
    let mut grouped = BTreeMap::new();
    for anchor in &output.anchors {
        grouped
            .entry(anchor.file_path.as_str())
            .or_insert_with(Vec::new)
            .push(anchor);
    }
    let is_unavailable = engine.is_warming() || engine.is_dead() || engine.last_error().is_some();
    let snapshot = engine.published_snapshot();
    let source_files = snapshot.codemap();
    let filtered = filter.filter_snapshot(&source_files, |file| {
        grouped.contains_key(file.file_path.as_str())
    });
    let resolver = crate::callers::resolution::SourceResolver::new(&source_files, &root);
    let mut outlines = Vec::new();
    for (i, file) in output.files.iter().take(count).enumerate() {
        let path = &file.file_path;
        let Some(anchors) = grouped
            .get(path.as_str())
            .filter(|anchors| !anchors.is_empty())
        else {
            contexts[i] = "[Only surrounding source lines on this page; no matched line for symbol lookup.]\n".into();
            continue;
        };
        if is_unavailable {
            contexts[i] =
                "[Symbol index unavailable or stale; live results remain available below.]\n"
                    .into();
            continue;
        }
        if filter.is_file_excluded(path) {
            contexts[i] = TEST_CONTEXT_EXCLUDED_NOTICE.into();
            continue;
        }
        if let Some(reason) = crate::workspace::source_encoding_exclusion(&root.join(path)) {
            contexts[i] = format!("[{path}: {reason}]\n");
            continue;
        }
        let indexed = source_files.iter().find(|file| &file.file_path == path);
        if let Some(notice) =
            diagnostics::unavailable(indexed, anchors[0], &root, snapshot.flows().digest(path))
        {
            contexts[i] = notice;
            continue;
        }
        let has_excluded_tests = indexed.is_some_and(|file| {
            file.symbols.iter().any(|symbol| {
                anchors.iter().any(|anchor| {
                    anchor
                        .start_line
                        .zip(anchor.end_line)
                        .is_none_or(|(start, end)| {
                            symbol.range.start_line <= end
                                && start <= symbol.range.end_line_inclusive()
                        })
                }) && filter.is_excluded(path, &symbol.range)
            })
        });
        if has_excluded_tests {
            contexts[i].push_str(TEST_CONTEXT_EXCLUDED_NOTICE);
        }
        if let Some(file) = filtered.iter().find(|file| &file.file_path == path) {
            if let Some(info) = file.macro_expansion() {
                contexts[i].push_str(&format!("[{path}: {}]\n", info.notice));
            }
            let outline = structure::Outline::new(file, anchors, &resolver, options);
            if !has_excluded_tests || !outline.selected.is_empty() {
                outlines.push(outline);
            }
        }
    }
    let annotations =
        render::annotate(&outlines, &source_files, cap / 2, options, &root, &resolver);
    let mut remaining = cap;
    let mut flow_budget = crate::flow::RequestBudget::default();
    let mut shown_routes = crate::events::ShownRoutes::default();
    let mut hints = super::ReadHints::new(output, options);
    for (i, file) in output.files.iter().take(count).enumerate() {
        // Unused space is carried forward; every remaining file first receives a share.
        let share = remaining / (count - i);
        let context = &mut contexts[i];
        *context = super::bounded_notice(context, share);
        if let Some(outline) = outlines
            .iter()
            .find(|outline| outline.file.file_path == file.file_path)
        {
            let available = share.saturating_sub(context.len());
            let symbol_cap = if options.should_include_relations() {
                available / 2
            } else {
                available
            };
            context.push_str(&render::render(
                outline,
                annotations.as_ref(),
                symbol_cap,
                options,
                &mut hints,
            ));
            if options.should_include_relations() {
                let anchors: Vec<_> = grouped
                    .get(file.file_path.as_str())
                    .into_iter()
                    .flatten()
                    .map(|anchor| {
                        (
                            anchor.file_path.clone(),
                            anchor.start_line.unwrap_or(1),
                            anchor.end_line.unwrap_or(usize::MAX),
                        )
                    })
                    .collect();
                let available = share.saturating_sub(context.len());
                if options.should_include_events() {
                    let text = snapshot.events().for_paths_with_context(
                        &anchors,
                        None,
                        available / 2,
                        &root,
                        Some(&file.file_path),
                        Some(&mut shown_routes),
                    );
                    append_relation(context, text, share);
                }
                let available = share.saturating_sub(context.len());
                let text = snapshot.implementations().for_paths_with_context(
                    &anchors,
                    None,
                    available / 2,
                    &root,
                    true,
                    Some(&file.file_path),
                );
                append_relation(context, text, share);
                let available = share.saturating_sub(context.len() + 32);
                let text = snapshot.flows().for_file(
                    &anchors,
                    available,
                    &root,
                    options.should_list_unresolved,
                    &mut flow_budget,
                );
                append_relation(context, text, share);
            }
        }
        remaining = remaining.saturating_sub(context.len());
    }
    (
        contexts,
        super::bounded_notice(&flow_budget.notice(), notice_cap),
    )
}

fn append_relation(context: &mut String, text: String, cap: usize) {
    if text.is_empty() {
        return;
    }
    let mut nested = String::from("\n");
    for line in text.split_inclusive('\n') {
        if line.starts_with("## ") || line.starts_with("### ") {
            nested.push_str("##");
        }
        nested.push_str(line);
    }
    if context.len() + nested.len() <= cap {
        context.push_str(&nested);
    } else {
        let notice =
            "[Additional relationships omitted by output budget; narrow this file/window.]\n";
        context.push_str(&super::bounded_notice(
            notice,
            cap.saturating_sub(context.len()),
        ));
    }
}
