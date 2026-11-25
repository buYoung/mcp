//! Implementation/override trailer for `read view=source`: source-proven implementations,
//! overrides, and Rust explicit impl blocks outside the read window. Source rows above the
//! trailer stay byte-for-byte unchanged, and lookups read only the precomputed
//! `ImplementationIndex` without reparsing.

use super::LiveOutput;
use crate::index::EngineSupervisor;

/// Bytes for the whole implementation trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1024;
pub(crate) const HEADER: &str = crate::implementations::SOURCE_TRAILER_HEADER;

/// The trailer for a read window, within `byte_cap` bytes and never above [`TRAILER_BYTE_CAP`].
pub(super) fn trailer(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    byte_cap: usize,
) -> Option<String> {
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    if !output.should_list_outside_callers || byte_cap < HEADER.len() + 48 {
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
    let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
    if filter.is_file_excluded(path) {
        return None;
    }
    let snapshot = engine.published_snapshot();
    let files = snapshot.codemap();
    let file = files.iter().find(|file| file.file_path == path)?;
    if super::diagnostics::unavailable(Some(file), anchor, &root, snapshot.flows().digest(path))
        .is_some()
    {
        return None;
    }
    snapshot
        .implementations()
        .source_trailer(path, start, end, &root, byte_cap)
}
