//! Modification times of files whose content still matched their indexed document.
//!
//! A touch, a checkout that rewrites identical content, or an index started from another
//! worktree's index leaves a document whose recorded mtime differs from the file although its
//! content is the same. Refresh passes keep such documents and record the file's current mtime
//! here, so later passes, publications and other server processes treat them as current without
//! hashing the files again. A record applies only while the document holds the recorded content.

use std::collections::HashMap;
use std::path::Path;

const FILE_NAME: &str = "content-mtimes.json";

#[derive(Default)]
pub(crate) struct ContentMtimes {
    /// Stored path → (content hash of its document, current file mtime).
    by_path: HashMap<String, (u64, u64)>,
}

impl ContentMtimes {
    /// The records of the index at `index`; an absent or unreadable file records nothing.
    pub(crate) fn load(index: &Path) -> Self {
        let by_path = std::fs::read(index.join(FILE_NAME))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { by_path }
    }

    /// The current mtime of `path` while its document holds `content_hash`.
    pub(crate) fn mtime(&self, path: &str, content_hash: Option<u64>) -> Option<u64> {
        self.by_path
            .get(path)
            .filter(|(hash, _)| Some(*hash) == content_hash)
            .map(|(_, mtime)| *mtime)
    }

    pub(super) fn record(&mut self, path: String, content_hash: u64, mtime: u64) {
        self.by_path.insert(path, (content_hash, mtime));
    }

    /// Drop the record of a path whose document was rewritten or deleted.
    pub(super) fn remove(&mut self, path: &str) -> bool {
        self.by_path.remove(path).is_some()
    }

    /// Keep only the records `is_current` accepts.
    pub(super) fn retain(&mut self, mut is_current: impl FnMut(&str, u64) -> bool) {
        self.by_path
            .retain(|path, (content_hash, _)| is_current(path, *content_hash));
    }

    /// Replace the file in `index` with these records, or remove it when there are none.
    /// Processes sharing the index may overwrite each other's records; a lost record only
    /// makes a later pass hash that file again.
    pub(super) fn save(&self, index: &Path) -> Result<(), String> {
        let path = index.join(FILE_NAME);
        if self.by_path.is_empty() {
            return match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    Err(error.to_string())
                }
                _ => Ok(()),
            };
        }
        let bytes = serde_json::to_vec(&self.by_path).map_err(|error| error.to_string())?;
        let staged = index.join(format!(".{FILE_NAME}.{}", std::process::id()));
        std::fs::write(&staged, bytes)
            .and_then(|()| std::fs::rename(&staged, &path))
            .map_err(|error| {
                let _ = std::fs::remove_file(&staged);
                error.to_string()
            })
    }
}
