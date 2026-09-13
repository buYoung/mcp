//! Indexed member and call context above untouched live filesystem results.
pub(crate) mod callable;
mod references;
mod render;
mod structure;

use super::live_options::{LiveOptions, LiveView};
use crate::index::EngineSupervisor;
use std::collections::BTreeMap;

pub(super) const PAYLOAD_BYTE_CAP: usize = 8192;
const FRAMING_BYTE_BUDGET: usize = 512;
const OUTLINED_FILE_LIMIT: usize = 8;
const TEST_CONTEXT_EXCLUDED_NOTICE: &str = "[Test code excluded from automatic context; set exclude.should_include_test_code=true to include it.]\n";

pub(crate) struct LiveAnchor {
    pub file_path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

#[derive(Default)]
pub(crate) struct LiveOutput {
    pub text: String,
    pub anchors: Vec<LiveAnchor>,
    pub notices: Vec<String>,
}

/// Read and grep remain live filesystem operations. Missing or warming index
/// context never triggers a synchronous rebuild or replaces their source results.
pub(crate) fn append(
    engine: &EngineSupervisor,
    output: LiveOutput,
    output_byte_cap: Option<usize>,
    options: LiveOptions,
) -> Result<String, (i64, String)> {
    // This return precedes snapshots, test filters, resolvers and relation scans.
    if options.view == LiveView::Source {
        let text = if output.notices.is_empty() {
            output.text
        } else {
            format!("{}\n{}", output.text, output.notices.join("\n"))
        };
        if output_byte_cap.is_some_and(|cap| text.len() > cap) {
            return Err((
                -32602,
                "Source output plus expansion notices exceeds the output cap; narrow the request."
                    .into(),
            ));
        }
        return Ok(text);
    }
    let raw = if options.view == LiveView::Full {
        output.text
    } else {
        String::new()
    };
    let notices = if output.notices.is_empty() {
        String::new()
    } else {
        format!("{}\n", output.notices.join("\n"))
    };
    let frame = |content: &str| match options.view {
        LiveView::Full => format!("# symbols\n\n{notices}{content}\n# results\n{raw}"),
        LiveView::Relations => format!("# relations\n\n{notices}{content}"),
        _ => format!("# symbols\n\n{notices}{content}"),
    };
    let limit = output_byte_cap.unwrap_or(usize::MAX);
    let empty = frame("");
    if empty.len() > limit {
        return Err((
            -32602,
            "Read window leaves no room for section headers; retry with a smaller limit."
                .to_string(),
        ));
    }
    let remaining = output_byte_cap
        .map(|limit| limit.saturating_sub(raw.len() + notices.len()))
        .unwrap_or(PAYLOAD_BYTE_CAP * 2 + FRAMING_BYTE_BUDGET);
    let cap = PAYLOAD_BYTE_CAP.min(remaining.saturating_sub(FRAMING_BYTE_BUDGET) / 2);
    let event_cap = if options.should_include_events && options.should_include_relations() {
        cap
    } else {
        0
    };
    let cap = if event_cap > 0 { cap / 2 } else { cap };
    let content = if remaining < FRAMING_BYTE_BUDGET {
        String::new()
    } else if cap < 128 {
        "[No room for symbol context; narrow the read window.]\n".to_string()
    } else if output.anchors.is_empty() {
        "[No returned source location for symbol lookup.]\n".to_string()
    } else if engine.is_warming() || engine.is_dead() || engine.last_error().is_some() {
        "[Symbol index unavailable or stale; live results remain available below.]\n".to_string()
    } else {
        let root = std::env::current_dir().unwrap_or_default();
        let test_filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
        if output
            .anchors
            .iter()
            .all(|anchor| test_filter.is_file_excluded(&anchor.file_path))
        {
            TEST_CONTEXT_EXCLUDED_NOTICE.to_string()
        } else {
            let snapshot = engine.published_snapshot();
            let source_files = snapshot.codemap();
            let mut grouped: BTreeMap<&str, Vec<&LiveAnchor>> = BTreeMap::new();
            for anchor in &output.anchors {
                grouped.entry(&anchor.file_path).or_default().push(anchor);
            }
            // Only outlines need a filtered copy. Relations keep the shared snapshot
            // and check test exclusions lazily for matching definitions/call sites.
            let filtered_files = test_filter.filter_snapshot(&source_files, |file| {
                grouped.contains_key(file.file_path.as_str())
            });
            let files = filtered_files.as_ref();
            let resolver = crate::callers::resolution::SourceResolver::new(&source_files, &root);
            let mut outlines = Vec::new();
            let mut has_excluded_test_context = false;
            let mut encoding_notices = Vec::new();
            for (path, anchors) in grouped.iter().take(OUTLINED_FILE_LIMIT) {
                if let Some(file) = source_files.iter().find(|file| file.file_path == *path) {
                    has_excluded_test_context |= file.symbols.iter().any(|symbol| {
                        anchors.iter().any(|anchor| {
                            anchor
                                .start_line
                                .zip(anchor.end_line)
                                .is_none_or(|(start, end)| {
                                    symbol.range.start_line <= end
                                        && start <= symbol.range.end_line_inclusive()
                                })
                        }) && test_filter.is_excluded(path, &symbol.range)
                    });
                }
                if let Some(file) = files.iter().find(|file| file.file_path == *path) {
                    if let Some(info) = file
                        .navigation
                        .as_ref()
                        .and_then(|navigation| navigation.macro_expansion.as_ref())
                    {
                        encoding_notices.push(format!("[{path}: {}]\n", info.notice));
                    }
                    outlines.push(structure::Outline::new(file, anchors, &resolver, options));
                } else if let Some(reason) =
                    crate::workspace::source_encoding_exclusion(&root.join(path))
                {
                    encoding_notices.push(format!("[{path}: {reason}]\n"));
                }
            }
            let mut encoding_notices = encoding_notices.concat();
            if encoding_notices.len() > cap / 2 {
                let mut end = cap / 2;
                while !encoding_notices.is_char_boundary(end) {
                    end = end.saturating_sub(1);
                }
                encoding_notices.truncate(end);
                encoding_notices.push_str("…\n");
            }
            let render_cap = cap.saturating_sub(encoding_notices.len());
            let mut rendered = if has_excluded_test_context {
                let notice = TEST_CONTEXT_EXCLUDED_NOTICE;
                if outlines.iter().all(|outline| outline.selected.is_empty()) {
                    notice.to_string()
                } else {
                    format!(
                        "{notice}\n{}",
                        render::render(&outlines, &source_files, render_cap, options)
                    )
                }
            } else {
                render::render(&outlines, &source_files, render_cap, options)
            };
            if !encoding_notices.is_empty() {
                let notices = encoding_notices;
                rendered = if outlines.is_empty() {
                    notices
                } else {
                    format!("{notices}\n{rendered}")
                };
            }
            if grouped.len() > OUTLINED_FILE_LIMIT {
                rendered.push_str(&format!(
                    "[File limit: {} returned files not outlined.]\n",
                    grouped.len() - OUTLINED_FILE_LIMIT
                ));
            }
            if event_cap > 0 {
                let anchors = output
                    .anchors
                    .iter()
                    .map(|anchor| {
                        (
                            anchor.file_path.clone(),
                            anchor.start_line.unwrap_or(1),
                            anchor.end_line.unwrap_or(usize::MAX),
                        )
                    })
                    .collect::<Vec<_>>();
                rendered.push('\n');
                rendered.push_str(
                    &snapshot
                        .events()
                        .for_paths(&anchors, None, event_cap, &root),
                );
            }
            rendered
        }
    };
    let text = frame(&content);
    if text.len() > limit {
        return Err((
            -32602,
            "Read window plus symbol context exceeds the output cap; retry with a smaller limit."
                .to_string(),
        ));
    }
    Ok(text)
}
