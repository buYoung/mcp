//! Read one indexed callable's source for a follow-up judgment.

use super::*;

/// The complete, identity-checked and masked source of one indexed callable, if it fits.
/// Reuses the ordinary read path's scope checks and full-file secret masking.
pub(super) fn callable_source(path: &str, symbol: &BlockSymbol) -> Option<String> {
    let first = symbol.start_line;
    let last = symbol.end_line;
    let output = crate::tools::read::read_file_with_metadata(&json!({
        "file_path": path, "offset": first,
        "limit": last.saturating_sub(first) + 1, "view": "source", "expand": "none",
    }))
    .ok()?;
    if output.text.len() > MAX_COMPLETE_BODY_BYTES
        || !identity_verified(&output.text, symbol)
        || is_masked_unavailable(&output.text)
    {
        return None;
    }
    let last_returned = output
        .text
        .lines()
        .filter_map(|line| line.split_once('→')?.0.trim().parse::<usize>().ok())
        .next_back()?;
    (last_returned == last).then_some(output.text)
}
