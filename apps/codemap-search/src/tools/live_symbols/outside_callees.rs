//! Downstream callee trailer for `read view=source`: source-resolved definitions called by
//! functions in the window that live outside the window (preferring cross-file jump targets).
//! Source rows above the trailer stay byte-for-byte unchanged.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::structure::callable;
use super::LiveOutput;
use crate::index::EngineSupervisor;
use crate::parser::ExtractedSymbol;

/// Functions overlapping one window whose outgoing calls are resolved.
const FUNCTION_LOOKUP_LIMIT: usize = 12;
/// Outgoing definitions listed per function before the rest are counted.
const CALLEES_PER_FUNCTION: usize = 4;
/// Bytes for the whole callee trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1024;
/// Time cap so callee resolution stays imperceptible even on large files.
const TRAILER_TIME_BUDGET: Duration = Duration::from_millis(150);
pub(crate) const HEADER: &str = "[Calls to definitions outside this window (static analysis)]";

/// Compact 1-line-per-function trailer listing source-confirmed definitions called inside
/// `(path, start..=end)` whose definitions live outside `(path, start..=end)`.
pub(super) fn trailer(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    byte_cap: usize,
) -> Option<String> {
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    if !output.should_list_outside_callers || byte_cap < HEADER.len() + 64 {
        return None;
    }
    let [anchor] = output.anchors.as_slice() else {
        return None;
    };
    let (start, end) = anchor.start_line.zip(anchor.end_line)?;
    if !engine.is_active()
        || engine.is_warming()
        || engine.is_dead()
        || engine.last_error().is_some()
    {
        return None;
    }
    let root = crate::workspace::root_dir().ok()?;
    let path = anchor.file_path.as_str();
    let source_path = root.join(path);
    if !crate::callers::resolution::supports(path)
        || crate::lang::spec_for_path(&source_path).is_none()
        || !crate::workspace::walk_root_is_visible(&source_path, true)
    {
        return None;
    }
    let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
    if filter.is_file_excluded(path)
        || crate::workspace::source_encoding_exclusion(&source_path).is_some()
    {
        return None;
    }
    let snapshot = engine.published_snapshot();
    let source_files = snapshot.codemap();
    let call_lookup = snapshot.call_lookup();
    let file = call_lookup
        .file_index(path)
        .and_then(|pos| source_files.get(pos))
        .or_else(|| source_files.iter().find(|file| file.file_path == path))
        .map(|file| &**file)?;
    if super::diagnostics::unavailable_reason(
        Some(file),
        &source_path,
        snapshot.flows().digest(path),
    )
    .is_some()
    {
        return None;
    }
    let navigation = file.navigation.as_ref()?;
    if navigation.calls.is_empty() {
        return None;
    }
    let functions: Vec<&ExtractedSymbol> = file
        .symbols
        .iter()
        .filter(|symbol| {
            callable(symbol)
                && symbol.range.start_line <= end
                && start <= symbol.range.end_line_inclusive()
                && !filter.is_excluded(path, &symbol.range)
        })
        .take(FUNCTION_LOOKUP_LIMIT)
        .collect();
    if functions.is_empty() {
        return None;
    }
    let deadline = Instant::now() + TRAILER_TIME_BUDGET;
    let resolver =
        crate::callers::resolution::SourceResolver::with_lookup(&source_files, &root, call_lookup)
            .with_implementations(snapshot.implementations());

    let mut rows: Vec<String> = Vec::new();
    for function in functions {
        if Instant::now() >= deadline {
            break;
        }
        let mut seen_targets: HashSet<(&str, usize)> = HashSet::new();
        let mut targets: Vec<(bool, String, usize, String)> = Vec::new();
        for call in &navigation.calls {
            if call.range.start_line < start || call.range.start_line > end {
                continue;
            }
            if (call.range.start_line, call.range.start_col)
                < (function.range.start_line, function.range.start_col)
                || (call.range.end_line, call.range.end_col)
                    > (function.range.end_line, function.range.end_col)
            {
                continue;
            }
            if call.name == function.name {
                continue;
            }
            if Instant::now() >= deadline {
                break;
            }
            let Some(target) = resolver
                .resolve_call(file, call)
                .filter(|target| target.is_precise)
            else {
                continue;
            };
            let target_path = target.file.file_path.as_str();
            let target_line = target.symbol.range.start_line;
            if filter.is_excluded(target_path, &target.symbol.range) {
                continue;
            }
            let is_inside_window =
                target_path == path && target_line >= start && target_line <= end;
            if is_inside_window {
                continue;
            }
            if !seen_targets.insert((target_path, target_line)) {
                continue;
            }
            let is_same_file = target_path == path;
            let label = format_target(target.symbol, target_path, path);
            targets.push((is_same_file, target_path.to_string(), target_line, label));
        }
        if targets.is_empty() {
            continue;
        }
        // Cross-file jumps first, then same-file definitions outside the read window.
        targets.sort_by(|a, b| (a.0, &a.1, a.2).cmp(&(b.0, &b.1, b.2)));
        let total = targets.len();
        let shown: Vec<&str> = targets
            .iter()
            .take(CALLEES_PER_FUNCTION)
            .map(|(_, _, _, label)| label.as_str())
            .collect();
        let fn_name = function
            .name
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let mut line = format!(
            "\n- {fn_name} L{}: {}",
            function.range.start_line,
            shown.join(", ")
        );
        if total > CALLEES_PER_FUNCTION {
            line.push_str(&format!(", +{} more", total - CALLEES_PER_FUNCTION));
        }
        rows.push(line);
    }
    if rows.is_empty() {
        return None;
    }
    let mut text = String::from(HEADER);
    let mut emitted = 0usize;
    let total_rows = rows.len();
    for (idx, row) in rows.iter().enumerate() {
        let remaining = total_rows - idx;
        let more_note = format!("\n- +{remaining} more function(s) with outside calls");
        let reserve = if idx + 1 < total_rows {
            more_note.len()
        } else {
            0
        };
        if text.len() + row.len() + reserve > byte_cap {
            if emitted == 0 {
                return None;
            }
            if text.len() + more_note.len() <= byte_cap {
                text.push_str(&more_note);
            }
            break;
        }
        text.push_str(row);
        emitted += 1;
    }
    (emitted > 0).then_some(text)
}

fn format_target(symbol: &ExtractedSymbol, target_path: &str, current_path: &str) -> String {
    let name = symbol.name.split_whitespace().collect::<Vec<_>>().join(" ");
    let qualified = match &symbol.owner {
        Some(owner) => {
            let sep = crate::lang::spec_for_path(std::path::Path::new(target_path))
                .map(|spec| spec.qualified_name_separator())
                .unwrap_or(".");
            format!("{owner}{sep}{name}")
        }
        None => name,
    };
    let loc = crate::locations::display(target_path, symbol.range.start_line, Some(current_path));
    format!("{qualified} ({loc})")
}
