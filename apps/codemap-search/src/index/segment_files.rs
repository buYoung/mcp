//! Segment files that neither the live index nor tantivy's garbage-collection list refers to.
//!
//! Server processes sharing one index directory take turns as the writer. Releases that wrote
//! through the index handle opened at startup rewrote `.managed.json` from a stale copy and
//! dropped files other processes had registered, so tantivy never deleted them once unused.
//! They are removed here while the caller holds the index writer lock, when no other writer
//! can be creating segment files.

use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

use tantivy::directory::{Directory, META_LOCK};
use tantivy::Index;

/// Unreferenced files younger than this are left alone.
const MIN_ORPHAN_AGE: Duration = Duration::from_secs(10 * 60);

/// Delete unreferenced segment files of the index at `path`; failures only skip the cleanup.
pub(super) fn remove_orphans(index: &Index, path: &Path) {
    match remove_unreferenced(index, path) {
        Ok((0, _)) => {}
        Ok((files, bytes)) => {
            tracing::info!(files, bytes, "removed unreferenced index segment files")
        }
        Err(error) => tracing::debug!("skipped unreferenced segment file cleanup: {error}"),
    }
}

fn remove_unreferenced(index: &Index, path: &Path) -> Result<(usize, u64), String> {
    // Readers hold this lock while opening segments, so none is opening one being deleted.
    let _meta_lock = index
        .directory()
        .acquire_lock(&META_LOCK)
        .map_err(|error| error.to_string())?;
    let live: HashSet<String> = index
        .load_metas()
        .map_err(|error| error.to_string())?
        .list_segment_files()
        .into_iter()
        .filter_map(|file| file.to_str().map(str::to_owned))
        .collect();
    // Files tantivy still tracks are its own to collect.
    let managed: HashSet<String> = std::fs::read(path.join(".managed.json"))
        .map_err(|error| error.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))?;
    let now = SystemTime::now();
    let (mut files, mut bytes) = (0, 0);
    for entry in std::fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_segment_file(name) || live.contains(name) || managed.contains(name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let is_old = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= MIN_ORPHAN_AGE);
        if is_old && std::fs::remove_file(entry.path()).is_ok() {
            files += 1;
            bytes += metadata.len();
        }
    }
    Ok((files, bytes))
}

/// `<32 hex digit segment id>.<component>`, including `<id>.<opstamp>.del`.
fn is_segment_file(name: &str) -> bool {
    name.len() > 33
        && name.as_bytes()[32] == b'.'
        && name.as_bytes()[..32]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
}
