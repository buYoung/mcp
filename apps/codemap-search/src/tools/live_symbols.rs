//! Indexed member and call context above untouched live filesystem results.
mod references;
mod render;
mod structure;

use crate::index::EngineSupervisor;
use std::collections::BTreeMap;

const PAYLOAD_BYTE_CAP: usize = 8192;
const FRAMING_BYTE_BUDGET: usize = 512;
const OUTLINED_FILE_LIMIT: usize = 8;

pub(crate) struct LiveAnchor {
    pub file_path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

pub(crate) struct LiveOutput {
    pub text: String,
    pub anchors: Vec<LiveAnchor>,
}

/// Read and grep remain live filesystem operations. Missing or warming index
/// context never triggers a synchronous rebuild or replaces their source results.
pub(crate) fn append(
    engine: &EngineSupervisor,
    output: LiveOutput,
    output_byte_cap: Option<usize>,
) -> Result<String, (i64, String)> {
    let raw = output.text;
    let limit = output_byte_cap.unwrap_or(usize::MAX);
    let empty = format!("# symbols\n\n# results\n{raw}");
    if empty.len() > limit {
        return Err((
            -32602,
            "Read window leaves no room for section headers; retry with a smaller limit."
                .to_string(),
        ));
    }
    let remaining = output_byte_cap
        .map(|limit| limit.saturating_sub(raw.len()))
        .unwrap_or(PAYLOAD_BYTE_CAP * 2 + FRAMING_BYTE_BUDGET);
    let cap = PAYLOAD_BYTE_CAP.min(remaining.saturating_sub(FRAMING_BYTE_BUDGET) / 2);
    let content = if remaining < FRAMING_BYTE_BUDGET {
        String::new()
    } else if cap < 128 {
        "[No room for symbol context; narrow the read window.]\n".to_string()
    } else if output.anchors.is_empty() {
        "[No returned source location for symbol lookup.]\n".to_string()
    } else if engine.is_warming() || engine.is_dead() || engine.last_error().is_some() {
        "[Symbol index unavailable or stale; live results remain available below.]\n".to_string()
    } else {
        let snapshot = engine.published_snapshot();
        let source_files = snapshot.codemap();
        let root = std::env::current_dir().unwrap_or_default();
        let test_filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
        let filtered_files = test_filter.filter_snapshot(&source_files);
        let files = filtered_files.as_ref();
        let resolver = crate::callers::resolution::SourceResolver::new(files, &root);
        let mut grouped: BTreeMap<&str, Vec<&LiveAnchor>> = BTreeMap::new();
        for anchor in &output.anchors {
            grouped.entry(&anchor.file_path).or_default().push(anchor);
        }
        let mut outlines = Vec::new();
        let mut has_excluded_test_context = false;
        for (path, anchors) in grouped.iter().take(OUTLINED_FILE_LIMIT) {
            if let Some(file) = source_files.iter().find(|file| file.file_path == *path) {
                has_excluded_test_context |= file.symbols.iter().any(|symbol| {
                    anchors.iter().any(|anchor| {
                        anchor
                            .start_line
                            .zip(anchor.end_line)
                            .is_none_or(|(start, end)| {
                                symbol.range.start_line <= end && start <= symbol.range.end_line
                            })
                    }) && test_filter.is_excluded(path, &symbol.range)
                });
            }
            if let Some(file) = files.iter().find(|file| file.file_path == *path) {
                outlines.push(structure::Outline::new(file, anchors, &resolver));
            }
        }
        let mut rendered = if has_excluded_test_context {
            let notice = "[Test code excluded from automatic context; set exclude.should_include_test_code=true to include it.]\n";
            if outlines.iter().all(|outline| outline.selected.is_empty()) {
                notice.to_string()
            } else {
                format!("{notice}\n{}", render::render(&outlines, files, cap))
            }
        } else {
            render::render(&outlines, files, cap)
        };
        if grouped.len() > OUTLINED_FILE_LIMIT {
            rendered.push_str(&format!(
                "[File limit: {} returned files not outlined.]\n",
                grouped.len() - OUTLINED_FILE_LIMIT
            ));
        }
        rendered
    };
    let text = format!("# symbols\n\n{content}\n# results\n{raw}");
    if text.len() > limit {
        return Err((
            -32602,
            "Read window plus symbol context exceeds the output cap; retry with a smaller limit."
                .to_string(),
        ));
    }
    Ok(text)
}
