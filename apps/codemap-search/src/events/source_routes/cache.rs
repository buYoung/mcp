//! Reuse the bounded, render-ready snapshot when its analysis inputs are unchanged.
use super::snapshot::Snapshot;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tantivy::directory::{Directory, Lock, MmapDirectory};

const CACHE_FILE: &str = "source-routes.cache";
const CACHE_BYTE_CAP: usize = 64 * 1024 * 1024;

struct CacheBuffer(Vec<u8>);

impl Write for CacheBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > CACHE_BYTE_CAP.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other(
                "source route snapshot exceeds cache byte cap",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) struct SnapshotCache {
    directory: MmapDirectory,
    path: PathBuf,
    key: blake3::Hash,
}

impl SnapshotCache {
    pub(super) fn open(directory: &Path, inputs: &impl Serialize) -> Option<Self> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(env!("CODEMAP_ANALYSIS_BUILD_ID").as_bytes());
        serde_json::to_writer(&mut hasher, inputs).ok()?;
        Some(Self {
            directory: MmapDirectory::open(directory).ok()?,
            path: directory.join(CACHE_FILE),
            key: hasher.finalize(),
        })
    }

    fn load(&self) -> Option<Snapshot> {
        let file = std::fs::File::open(&self.path).ok()?;
        if file.metadata().ok()?.len() > CACHE_BYTE_CAP as u64 {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(CACHE_BYTE_CAP as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        // Two fixed-length hashes precede the JSON: input identity, then payload checksum.
        if bytes.len() < 64 || bytes.len() > CACHE_BYTE_CAP || bytes[..32] != *self.key.as_bytes() {
            return None;
        }
        if bytes[32..64] != *blake3::hash(&bytes[64..]).as_bytes() {
            return None;
        }
        let mut snapshot: Snapshot = serde_json::from_slice(&bytes[64..]).ok()?;
        snapshot.restore_indexes();
        Some(snapshot)
    }

    fn store(&self, snapshot: &Snapshot) -> Result<(), String> {
        let mut buffer = CacheBuffer(vec![0; 64]);
        serde_json::to_writer(&mut buffer, snapshot).map_err(|error| error.to_string())?;
        let mut bytes = buffer.0;
        bytes[..32].copy_from_slice(self.key.as_bytes());
        let checksum = blake3::hash(&bytes[64..]);
        bytes[32..64].copy_from_slice(checksum.as_bytes());
        self.directory
            .atomic_write(Path::new(CACHE_FILE), &bytes)
            .map_err(|error| error.to_string())
    }
}

pub(super) fn load_or_build(
    cache: Option<SnapshotCache>,
    build: impl FnOnce() -> Snapshot,
) -> Snapshot {
    let started = std::time::Instant::now();
    let Some(cache) = cache else {
        return build();
    };
    if let Some(snapshot) = cache.load() {
        tracing::debug!("reused source route snapshot");
        return snapshot;
    }
    // Independent of the Tantivy writer: one process derives a generation and peers
    // reuse it. OS locks are released on exit, including crashes.
    let _guard = match cache.directory.acquire_lock(&Lock {
        filepath: PathBuf::from(".source-routes.lock"),
        is_blocking: true,
    }) {
        Ok(guard) => Some(guard),
        Err(error) => {
            tracing::debug!("source route cache lock unavailable: {error}");
            None
        }
    };
    if let Some(snapshot) = cache.load() {
        tracing::debug!("reused source route snapshot after peer analysis");
        return snapshot;
    }
    let snapshot = build();
    if let Err(error) = cache.store(&snapshot) {
        tracing::debug!("source route cache write skipped: {error}");
    }
    tracing::debug!(
        elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
        route_count = snapshot.routes.len(),
        "built source route snapshot"
    );
    snapshot
}
