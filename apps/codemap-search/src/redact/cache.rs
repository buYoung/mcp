//! Bounded reuse of detection metadata, never source buffers or rendered responses.
//! Content digests catch same-size/same-mtime edits; the pinned config Arc separates
//! policy generations. File scans and context-free pattern scans must not be mixed.
use super::detection::{Detection, SourceScan};
use crate::config::ResolvedConfig;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MIN_PATTERN_BYTES: usize = 1024;
const MAX_ENTRIES: usize = 64;
const MAX_RETAINED_BYTES: usize = 8 * 1024 * 1024;

#[derive(PartialEq, Eq)]
enum InputKind {
    File(PathBuf),
    Patterns { can_use_labels: bool },
}

#[derive(PartialEq, Eq)]
struct Key {
    kind: InputKind,
    digest: blake3::Hash,
    source_bytes: usize,
}

impl Key {
    fn new(kind: InputKind, source: &str) -> Self {
        Self {
            kind,
            digest: blake3::hash(source.as_bytes()),
            source_bytes: source.len(),
        }
    }
}

#[derive(Clone)]
enum Value {
    Source(SourceScan),
    Patterns(Arc<[Detection]>),
}

struct Entry {
    key: Key,
    value: Value,
    retained_bytes: usize,
}

#[derive(Default)]
struct ScanCache {
    config: Option<Arc<ResolvedConfig>>,
    entries: VecDeque<Entry>,
    retained_bytes: usize,
}

thread_local! {
    // Redaction is synchronous and request-local, like the pinned config and activation
    // guards. No lock or retained source is shared with the background indexer.
    static CACHE: RefCell<ScanCache> = RefCell::new(ScanCache::default());
}

fn with_cache<T>(action: impl FnOnce(&mut ScanCache) -> T) -> T {
    let config = crate::config::get();
    CACHE.with(|slot| {
        let mut cache = slot.borrow_mut();
        if !cache
            .config
            .as_ref()
            .is_some_and(|previous| Arc::ptr_eq(previous, &config))
        {
            cache.entries.clear();
            cache.retained_bytes = 0;
            cache.config = Some(config);
        }
        action(&mut cache)
    })
}

fn get(key: &Key) -> Option<Value> {
    with_cache(|cache| {
        let index = cache.entries.iter().position(|entry| entry.key == *key)?;
        let entry = cache.entries.remove(index)?;
        let value = entry.value.clone();
        cache.entries.push_back(entry);
        Some(value)
    })
}

fn insert(key: Key, value: Value) {
    let (detections, literal_bytes) = match &value {
        Value::Source(scan) => (
            scan.detections.as_ref(),
            std::mem::size_of_val(scan.literals.as_ref()),
        ),
        Value::Patterns(detections) => (detections.as_ref(), 0),
    };
    let path_bytes = match &key.kind {
        InputKind::File(path) => path.as_os_str().len(),
        InputKind::Patterns { .. } => 0,
    };
    let retained_bytes = std::mem::size_of::<Entry>()
        + path_bytes
        + literal_bytes
        + std::mem::size_of_val(detections)
        + detections
            .iter()
            .map(|detection| detection.rule_id.capacity())
            .sum::<usize>();
    if retained_bytes > MAX_RETAINED_BYTES {
        return;
    }
    with_cache(|cache| {
        while cache.entries.len() >= MAX_ENTRIES
            || cache.retained_bytes + retained_bytes > MAX_RETAINED_BYTES
        {
            if let Some(entry) = cache.entries.pop_front() {
                cache.retained_bytes -= entry.retained_bytes;
            }
        }
        cache.retained_bytes += retained_bytes;
        cache.entries.push_back(Entry {
            key,
            value,
            retained_bytes,
        });
    });
}

pub(super) fn source_scan(
    path: &Path,
    source: &str,
    build: impl FnOnce() -> Option<SourceScan>,
) -> Option<SourceScan> {
    let key = Key::new(InputKind::File(path.to_owned()), source);
    if let Some(Value::Source(scan)) = get(&key) {
        return Some(scan);
    }
    // In particular, do not retain a timed-out/failed parse's fallback as a successful
    // syntax scan. The next request must still get its normal chance to parse.
    let scan = build()?;
    insert(key, Value::Source(scan.clone()));
    Some(scan)
}

pub(super) fn patterns(
    source: &str,
    can_use_labels: bool,
    build: impl FnOnce() -> Vec<Detection>,
) -> Vec<Detection> {
    // Small metadata strings are cheap and would evict reusable file scans.
    if source.len() < MIN_PATTERN_BYTES {
        return build();
    }
    let key = Key::new(InputKind::Patterns { can_use_labels }, source);
    if let Some(Value::Patterns(detections)) = get(&key) {
        return detections.to_vec();
    }
    let detections = build();
    insert(key, Value::Patterns(detections.clone().into()));
    detections
}
