use crate::parser::{CodeExtractor, ExtractedFile, ExtractedSymbol};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tantivy::collector::{DocSetCollector, TopDocs};
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexSettings, ReloadPolicy, TantivyDocument, Term};

/// Runs a query-parse attempt and converts a panic into `None`. tantivy 0.26's
/// query grammar `panic!`s instead of returning `Err` on some adversarial inputs
/// — e.g. a bare `*` hits `expect("Exist query without a field isn't allowed")`.
/// A malformed search query must degrade gracefully (never-exit contract), not
/// abort the server, so callers treat `None` like a parse failure and fall back.
///
/// We deliberately do NOT swap the process-global panic hook to mute the message:
/// this is a stdio MCP server whose diagnostics go to stderr, a channel separate
/// from the JSON-RPC stdout, so a rare caught-panic line on stderr is harmless to
/// the protocol. Muting the hook globally — even briefly — would swallow panic
/// diagnostics from the indexer/watcher threads if they panic during this window,
/// which is a worse trade than one stray stderr line.
fn parse_query_catching_panic(
    run_parse: impl FnOnce() -> Result<Box<dyn tantivy::query::Query>, tantivy::query::QueryParserError>,
) -> Option<Result<Box<dyn tantivy::query::Query>, tantivy::query::QueryParserError>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_parse)).ok()
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: String,
    pub score: f32,
    pub total_lines: usize,
    pub matched_symbols: Vec<ExtractedSymbol>,
    pub matched_literals: Vec<String>,
    /// True when `matched_symbols` is the all-symbols fallback (the file ranked in via a
    /// docstring/path token, not a symbol-name match). The detail view renders these
    /// names-only (no snippets) so a path/docstring match never dumps full file source.
    pub symbol_fallback: bool,
}

pub trait SearchEngine {
    fn index_files(&mut self, paths: &[&str]) -> Result<(), String>;
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String>;
}

/// Cheap, cloneable read-side handle over a committed tantivy index. The MCP server holds
/// one of these for searching while the indexer thread owns the writer — tantivy's `Index`
/// and `IndexReader` are Arc-backed, so clones share the same committed snapshot with no
/// lock. Commits made by the writer become visible here via the reader's reload policy.
#[derive(Clone)]
pub struct SearcherHandle {
    index: Index,
    reader: IndexReader,
    file_path_field: Field,
    file_path_parts_field: Field,
    symbol_field: Field,
    docstring_field: Field,
    extracted_json_field: Field,
}

pub struct TantivySearchEngine {
    pub index_path: String,
    pub schema: Schema,
    pub index: Index,
    pub reader: IndexReader,

    // Schema field references
    pub file_path_field: Field,
    pub file_path_parts_field: Field,
    pub symbol_field: Field,
    pub docstring_field: Field,
    pub extracted_json_field: Field,
    pub mtime_field: Field,

    // In-memory snapshot of the on-disk path→mtime map. Lazily populated once from the
    // index (one `AllQuery`) and then maintained incrementally, so a long-lived engine
    // (MCP mode) no longer runs an `AllQuery` over the whole index before every search
    // (Child 04). Reflects only what is actually committed to the index.
    indexed_mtimes_cache: Option<HashMap<String, u64>>,
}

/// Filename of the sidecar that stamps the extraction-format version of the stored
/// `extracted_json` documents. Lives alongside tantivy's own files in the index dir.
const EXTRACTION_FORMAT_FILE: &str = "codemap.format";

/// Current extraction-format version. Bump whenever the `ExtractedFile` JSON shape changes
/// in a way that requires re-extracting on-disk source (not just a serde-compatible add).
/// `v2` introduced `ExtractedSymbol.owner`: the field is serde-default-compatible (old docs
/// still deserialize), but a one-time reindex is needed so every stored symbol actually
/// carries `owner`. The version bump forces that rebuild exactly once.
const EXTRACTION_FORMAT_VERSION: &str = "v2-owner";

impl TantivySearchEngine {
    pub fn new(index_path: &str) -> Result<Self, String> {
        let mut schema_builder = Schema::builder();
        let file_path_field = schema_builder.add_text_field("file_path", STRING | STORED);
        let file_path_parts_field = schema_builder.add_text_field("file_path_parts", TEXT);
        let symbol_field = schema_builder.add_text_field("symbol", TEXT | STORED);
        let docstring_field = schema_builder.add_text_field("docstring", TEXT | STORED);
        // Literals are details-layer only (Child 03): not indexed for BM25 search.
        // Exact/literal string search is delegated to the rg-backed `grep` tool.
        let extracted_json_field = schema_builder.add_text_field("extracted_json", STORED);
        let mtime_field = schema_builder.add_u64_field("mtime", STORED);
        let schema = schema_builder.build();

        let path = Path::new(index_path);
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        }

        // Adding `owner` to `extracted_json` does NOT change the tantivy schema (it is one
        // STORED text field holding JSON), so `Index::open_or_create` would succeed against a
        // pre-upgrade index and the open-failure auto-rebuild below would never self-trigger.
        // An explicit sidecar version check forces a one-time reindex: read the stamped
        // version BEFORE any `remove_dir_all` (which would delete the sidecar), and treat a
        // mismatch exactly like the corrupt-recovery path so the rebuild logic stays unified.
        let version_path = path.join(EXTRACTION_FORMAT_FILE);
        let stored_version = std::fs::read_to_string(&version_path)
            .ok()
            .map(|s| s.trim().to_string());
        let format_outdated = stored_version.as_deref() != Some(EXTRACTION_FORMAT_VERSION);

        // Try to open or create the index directory. Rebuild from scratch if metadata is
        // corrupted OR the extraction-format version is outdated — both take the same
        // `remove_dir_all` + recreate path so the index is rebuilt exactly once.
        let opened = if format_outdated {
            Err("extraction format outdated".to_string())
        } else {
            tantivy::directory::MmapDirectory::open(path)
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    Index::open_or_create(dir, schema.clone()).map_err(|e| e.to_string())
                })
        };
        let index = match opened {
            Ok(idx) => idx,
            Err(_) => {
                let _ = std::fs::remove_dir_all(path);
                std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
                let directory =
                    tantivy::directory::MmapDirectory::open(path).map_err(|e| e.to_string())?;
                Index::create(directory, schema.clone(), IndexSettings::default())
                    .map_err(|e| e.to_string())?
            }
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| e.to_string())?;

        // Stamp the current format version only after the reader is built (a failed reader
        // build must not leave a stale-but-current version that would skip the next rebuild).
        // Written once: after this run the sidecar matches, so `format_outdated` is false on
        // every subsequent run and neither the rebuild nor this write fires again.
        if format_outdated {
            let _ = std::fs::write(&version_path, EXTRACTION_FORMAT_VERSION);
        }

        Ok(Self {
            index_path: index_path.to_string(),
            schema,
            index,
            reader,
            file_path_field,
            file_path_parts_field,
            symbol_field,
            docstring_field,
            extracted_json_field,
            mtime_field,
            indexed_mtimes_cache: None,
        })
    }

    fn get_indexed_mtimes(&self) -> HashMap<String, u64> {
        let searcher = self.reader.searcher();
        let mut map = HashMap::new();

        // DocSetCollector enumerates every matching doc with no limit, so the mtime map is
        // never silently truncated on large repos (which would corrupt delete detection).
        if let Ok(doc_addresses) = searcher.search(&AllQuery, &DocSetCollector) {
            for doc_address in doc_addresses {
                if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                    let path_val = doc.get_first(self.file_path_field);
                    let mtime_val = doc.get_first(self.mtime_field);
                    if let (Some(p_val), Some(m_val)) = (path_val, mtime_val) {
                        let path = p_val.as_str().unwrap_or("").to_string();
                        let mtime = m_val.as_u64().unwrap_or(0);
                        if !path.is_empty() {
                            map.insert(path, mtime);
                        }
                    }
                }
            }
        }
        map
    }
}

fn tokenize_path(file_path: &str) -> String {
    let mut tokens = Vec::new();
    for part in file_path.split(['/', '\\']) {
        if part.is_empty() {
            continue;
        }
        if part.contains('.') {
            let subparts: Vec<&str> = part.split('.').filter(|s| !s.is_empty()).collect();
            for sp in subparts {
                tokens.push(sp.to_string());
            }
        } else {
            tokens.push(part.to_string());
        }
    }
    tokens.join(" ")
}

fn normalize_relative_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    let replaced = s.replace('\\', "/");
    let mut trimmed = replaced.as_str();
    while trimmed.starts_with("./") {
        trimmed = &trimmed[2..];
    }
    while trimmed.starts_with('/') {
        trimmed = &trimmed[1..];
    }
    trimmed.to_string()
}

/// Compute the stored index key for `entry_path`: the path relative to the
/// (canonicalized) current working directory, normalized to forward slashes. Falls
/// back to the leading-slash-stripped absolute path when the file is outside the cwd
/// (e.g. an absolute walk root in tests) — byte-identical to the pre-Child-04 logic,
/// so incremental delete-detection (which keys on this string) is preserved.
fn relative_index_path(entry_path: &Path, abs_cwd: &Path) -> String {
    let abs_path = entry_path
        .canonicalize()
        .unwrap_or_else(|_| entry_path.to_path_buf());
    let rel = abs_path.strip_prefix(abs_cwd).unwrap_or(entry_path);
    normalize_relative_path(rel)
}

/// The stored index key for a path that may no longer exist on disk (a watcher remove
/// event): lenient canonicalization resolves the deepest existing ancestor (so a deleted
/// file under a symlinked root — e.g. macOS `/var` → `/private/var` — still strips the
/// canonical cwd prefix), yielding the same key [`relative_index_path`] stored when the
/// file existed.
fn stored_index_key(path: &Path, abs_cwd: &Path) -> String {
    let abs_path = crate::mcp::canonicalize_path_lenient(path);
    let rel = abs_path.strip_prefix(abs_cwd).unwrap_or(path);
    normalize_relative_path(rel)
}

/// Whether an event path would be reached by the shared ignore-aware walk
/// ([`crate::tools::build_walker`]) descending from the workspace root. The full walk
/// honors directory-only ignore rules (e.g. a `.gitignore` line `generated/`) by never
/// descending into the ignored directory, so a single depth-1 check of the immediate
/// parent is NOT enough — it would be rooted *inside* the ignored subtree and the
/// ancestor rule would never apply. Instead, verify every step of the ancestor chain
/// from the root: each component must be yielded by an ignore-aware depth-1 walk of its
/// parent, exactly as the full walk would descend. Cost is one `readdir` per path depth.
fn is_path_visible_to_walk(path: &Path) -> bool {
    let target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = match std::env::current_dir() {
        Ok(cwd) => cwd.canonicalize().unwrap_or(cwd),
        Err(_) => return true,
    };
    let rel = match target.strip_prefix(&root) {
        Ok(rel) => rel.to_path_buf(),
        Err(_) => return false,
    };
    let mut current = root;
    for component in rel.components() {
        let next = current.join(component);
        let is_visible = crate::tools::build_walker(&current, false)
            .max_depth(Some(1))
            .build()
            .filter_map(|e| e.ok())
            .any(|entry| {
                entry.path() == next
                    || entry
                        .path()
                        .canonicalize()
                        .map(|p| p == next)
                        .unwrap_or(false)
            });
        if !is_visible {
            return false;
        }
        current = next;
    }
    true
}

/// Gather the index entry for a single file: enforce the source-extension allowlist
/// and the [`crate::tools::MAX_INDEXED_FILE_BYTES`] size cap (skip oversize/minified
/// blobs before they are read+parsed, Child 04), and capture a sub-second mtime so a
/// same-second edit still reindexes. Returns `None` when the file is not to be indexed.
fn collect_index_entry(entry_path: &Path, abs_cwd: &Path) -> Option<(String, PathBuf, u64)> {
    let ext = entry_path.extension().and_then(|s| s.to_str())?;
    if !crate::tools::is_source_extension(ext) {
        return None;
    }
    let metadata = std::fs::metadata(entry_path).ok()?;
    if metadata.len() > crate::config::get().max_file_size {
        return None;
    }
    let duration = metadata
        .modified()
        .ok()?
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()?;
    let rel_path = relative_index_path(entry_path, abs_cwd);
    Some((
        rel_path,
        entry_path.to_path_buf(),
        duration.as_nanos() as u64,
    ))
}

impl TantivySearchEngine {
    /// Incremental index refresh. Returns `true` only when a commit actually landed (adds
    /// or deletes), so callers (the indexer thread) can skip rebuilding derived snapshots
    /// on no-op passes. The `SearchEngine::index_files` trait method delegates here.
    pub fn index_files_changed(&mut self, paths: &[&str]) -> Result<bool, String> {
        let mut files_to_process = Vec::new();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());

        for path_str in paths {
            let path = Path::new(path_str);
            if path.is_file() {
                if let Some(entry) = collect_index_entry(path, &abs_cwd) {
                    files_to_process.push(entry);
                }
            } else if path.is_dir() {
                // Shared walker: honors EXCLUDED_DIRS + .gitignore/.codemapignore so
                // node_modules/target/… never enter the BM25 index (Child 04), matching
                // find/grep. include_ignored=false keeps ignored paths out of the index.
                for entry in crate::tools::build_walker(path, false)
                    .build()
                    .filter_map(|e| e.ok())
                {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(collected) = collect_index_entry(entry_path, &abs_cwd) {
                            files_to_process.push(collected);
                        }
                    }
                }
            }
        }

        // Lazily snapshot the on-disk mtime map once, then read from the maintained
        // cache — avoids an `AllQuery` over the whole index on every call, which in MCP
        // mode runs before every search (Child 04).
        if self.indexed_mtimes_cache.is_none() {
            let snapshot = self.get_indexed_mtimes();
            self.indexed_mtimes_cache = Some(snapshot);
        }
        let indexed_mtimes = self.indexed_mtimes_cache.as_ref().unwrap();

        let disk_file_paths: std::collections::HashSet<String> = files_to_process
            .iter()
            .map(|(rel_path, _, _)| rel_path.clone())
            .collect();

        let mut to_delete = Vec::new();
        for indexed_path in indexed_mtimes.keys() {
            if !disk_file_paths.contains(indexed_path) {
                to_delete.push(indexed_path.clone());
            }
        }

        let files_to_process_len = files_to_process.len();
        let mut to_index = Vec::new();
        for (rel_path, disk_path, mtime) in files_to_process {
            match indexed_mtimes.get(&rel_path) {
                Some(&indexed_mtime) if indexed_mtime == mtime => {
                    // Skip indexing: mtime hasn't changed
                }
                _ => {
                    to_index.push((rel_path, disk_path, mtime));
                }
            }
        }

        tracing::debug!(
            "index_files: files_to_process={}, to_index={}, to_delete={}",
            files_to_process_len,
            to_index.len(),
            to_delete.len()
        );

        self.apply_index_updates(to_index, to_delete)
    }

    /// Apply a computed set of reindex/delete updates to the index: delete the stale docs,
    /// read+parse+add the changed files, commit, reload the reader, and reconcile the mtime
    /// cache. Shared by the full-walk refresh ([`Self::index_files_changed`]) and the
    /// path-scoped watcher refresh ([`Self::refresh_paths`]). Returns `true` only when a
    /// commit actually landed.
    fn apply_index_updates(
        &mut self,
        to_index: Vec<(String, PathBuf, u64)>,
        to_delete: Vec<String>,
    ) -> Result<bool, String> {
        // Return early if no updates (adds or deletes) to avoid touching index and triggering modification
        if to_index.is_empty() && to_delete.is_empty() {
            return Ok(false);
        }

        let extractor = crate::parser::TreeSitterExtractor::new();
        let mut writer = match self.index.writer(50_000_000) {
            Ok(w) => w,
            Err(tantivy::TantivyError::LockFailure(e, _)) => {
                tracing::warn!("index_files LockFailure: {:?}", e);
                return Ok(false);
            }
            Err(e) => return Err(e.to_string()),
        };

        for rel_path in &to_delete {
            let term = Term::from_field_text(self.file_path_field, rel_path);
            writer.delete_term(term);
        }

        // Track only the docs that actually get added so the cache mirrors the index:
        // a file that fails to read/parse stays out of the cache and is retried next call.
        let mut committed_updates: Vec<(String, u64)> = Vec::new();
        for (rel_path, disk_path, mtime) in to_index {
            let content = match std::fs::read_to_string(&disk_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to read file {}: {}",
                        disk_path.display(),
                        e
                    );
                    continue;
                }
            };
            let extracted = match extractor.extract(&content, &rel_path) {
                Ok(ext) => ext,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse file {}: {}",
                        disk_path.display(),
                        e
                    );
                    continue;
                }
            };

            let term = Term::from_field_text(self.file_path_field, &rel_path);
            writer.delete_term(term);

            let mut doc = TantivyDocument::default();
            doc.add_text(self.file_path_field, &rel_path);

            let path_parts = tokenize_path(&rel_path);
            doc.add_text(self.file_path_parts_field, &path_parts);

            let json_str = match serde_json::to_string(&extracted) {
                Ok(js) => js,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to serialize extracted symbols for {}: {}",
                        disk_path.display(),
                        e
                    );
                    continue;
                }
            };
            doc.add_text(self.extracted_json_field, &json_str);
            doc.add_u64(self.mtime_field, mtime);

            for sym in &extracted.symbols {
                doc.add_text(self.symbol_field, &sym.name);
                let sub_tokens = crate::parser::split_identifier(&sym.name);
                for token in sub_tokens {
                    doc.add_text(self.symbol_field, &token);
                }
                if let Some(ref docstring) = sym.docstring {
                    doc.add_text(self.docstring_field, docstring);
                }
            }

            // Literals are intentionally not indexed (Child 03) — details-layer only.

            match writer.add_document(doc) {
                Ok(_) => committed_updates.push((rel_path, mtime)),
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to add document to index for {}: {}",
                        disk_path.display(),
                        e
                    );
                    continue;
                }
            }
        }

        writer.commit().map_err(|e| e.to_string())?;
        self.reader.reload().map_err(|e| e.to_string())?;

        // Reconcile the cache only after the commit landed (Child 04): drop the deleted
        // paths and record the freshly-indexed mtimes. On any earlier return (LockFailure,
        // commit/reload error) the cache is left untouched, so it never claims a file is
        // indexed when it is not.
        if let Some(cache) = self.indexed_mtimes_cache.as_mut() {
            for path in &to_delete {
                cache.remove(path);
            }
            for (rel_path, mtime) in committed_updates {
                cache.insert(rel_path, mtime);
            }
        }

        Ok(true)
    }

    /// Path-scoped incremental refresh driven by watcher events. Disk state after the
    /// debounce window is the source of truth, which is what makes rename / atomic-save
    /// event sequences safe: a path that exists on disk is (re)indexed regardless of any
    /// transient remove event, and only paths absent on disk are deleted. Deletes are
    /// derived from the event paths themselves — never from a set difference against the
    /// whole index (that logic assumes a full walk and would treat every not-passed file
    /// as deleted). Returns `true` only when a commit actually landed.
    pub fn refresh_paths(&mut self, paths: &[PathBuf]) -> Result<bool, String> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());

        if self.indexed_mtimes_cache.is_none() {
            let snapshot = self.get_indexed_mtimes();
            self.indexed_mtimes_cache = Some(snapshot);
        }
        let indexed_mtimes = self.indexed_mtimes_cache.as_ref().unwrap();

        let mut to_index: Vec<(String, PathBuf, u64)> = Vec::new();
        let mut to_delete: Vec<String> = Vec::new();

        for path in paths {
            if path.is_file() {
                let canonical_key = stored_index_key(path, &abs_cwd);
                // Case-only renames on case-insensitive filesystems (macOS APFS default):
                // the event may arrive under the OLD spelling, which canonicalizes to the
                // new on-disk spelling — the old spelling's doc would otherwise go stale
                // forever, since no later event names it and the watcher-healthy path
                // never runs the full-walk set-difference cleanup.
                if let Ok(raw_rel) = path.strip_prefix(&abs_cwd) {
                    let raw_key = normalize_relative_path(raw_rel);
                    if raw_key != canonical_key && indexed_mtimes.contains_key(&raw_key) {
                        to_delete.push(raw_key);
                    }
                }
                // The full walk honors .gitignore/.codemapignore via build_walker; a single
                // event path bypasses the walk, so check visibility explicitly — otherwise
                // the watcher would index ignored files the next full walk then deletes
                // (flip-flopping index contents).
                if !is_path_visible_to_walk(path) {
                    if indexed_mtimes.contains_key(&canonical_key) {
                        to_delete.push(canonical_key);
                    }
                    continue;
                }
                match collect_index_entry(path, &abs_cwd) {
                    Some((rel_path, disk_path, mtime)) => {
                        match indexed_mtimes.get(&rel_path) {
                            Some(&indexed_mtime) if indexed_mtime == mtime => {}
                            _ => to_index.push((rel_path, disk_path, mtime)),
                        }
                    }
                    None => {
                        // Not indexable (extension/size): if a former source file now
                        // exceeds the cap, drop its stale doc.
                        if indexed_mtimes.contains_key(&canonical_key) {
                            to_delete.push(canonical_key);
                        }
                    }
                }
            } else if path.is_dir() {
                // An ignored directory's subtree must never enter the index through an
                // event (same flip-flop hazard as the file branch); anything indexed
                // under it is stale by the full walk's standards — drop it.
                if !is_path_visible_to_walk(path) {
                    let prefix = format!("{}/", stored_index_key(path, &abs_cwd));
                    for indexed_path in indexed_mtimes.keys() {
                        if indexed_path.starts_with(&prefix) {
                            to_delete.push(indexed_path.clone());
                        }
                    }
                    continue;
                }
                // A directory event (created/renamed-in dir): walk just that subtree. The
                // set difference here is scoped to the subtree the walk fully covered, so
                // it is NOT the full-walk-only delete logic the doc comment above forbids.
                let mut subtree_disk_paths = std::collections::HashSet::new();
                for entry in crate::tools::build_walker(path, false)
                    .build()
                    .filter_map(|e| e.ok())
                {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some((rel_path, disk_path, mtime)) =
                            collect_index_entry(entry_path, &abs_cwd)
                        {
                            subtree_disk_paths.insert(rel_path.clone());
                            match indexed_mtimes.get(&rel_path) {
                                Some(&indexed_mtime) if indexed_mtime == mtime => {}
                                _ => to_index.push((rel_path, disk_path, mtime)),
                            }
                        }
                    }
                }
                let prefix = format!("{}/", stored_index_key(path, &abs_cwd));
                for indexed_path in indexed_mtimes.keys() {
                    if indexed_path.starts_with(&prefix)
                        && !subtree_disk_paths.contains(indexed_path)
                    {
                        to_delete.push(indexed_path.clone());
                    }
                }
            } else {
                // Absent on disk: delete the exact path, and any indexed files under it in
                // case the removed path was a directory.
                let rel_path = stored_index_key(path, &abs_cwd);
                let prefix = format!("{rel_path}/");
                for indexed_path in indexed_mtimes.keys() {
                    if indexed_path == &rel_path || indexed_path.starts_with(&prefix) {
                        to_delete.push(indexed_path.clone());
                    }
                }
            }
        }

        to_index.sort_by(|a, b| a.0.cmp(&b.0));
        to_index.dedup_by(|a, b| a.0 == b.0);
        to_delete.sort();
        to_delete.dedup();

        tracing::debug!(
            "refresh_paths: event_paths={}, to_index={}, to_delete={}",
            paths.len(),
            to_index.len(),
            to_delete.len()
        );

        self.apply_index_updates(to_index, to_delete)
    }

    /// Cheap read-side handle: clones the Arc-backed index/reader so search runs off the
    /// live committed snapshot without touching the indexing path.
    pub fn searcher_handle(&self) -> SearcherHandle {
        SearcherHandle {
            index: self.index.clone(),
            reader: self.reader.clone(),
            file_path_field: self.file_path_field,
            file_path_parts_field: self.file_path_parts_field,
            symbol_field: self.symbol_field,
            docstring_field: self.docstring_field,
            extracted_json_field: self.extracted_json_field,
        }
    }

    /// Rebuild the codemap snapshot from the stored `extracted_json` docs (one AllQuery,
    /// same shape as get_indexed_mtimes). The indexer publishes this for `overview`, so the
    /// working tree is parsed once for the index instead of separately on every overview.
    pub fn load_extracted_files(&self) -> Vec<ExtractedFile> {
        let searcher = self.reader.searcher();
        let mut files = Vec::new();
        // DocSetCollector enumerates every doc (no limit), so the codemap snapshot stays
        // complete on large repos.
        if let Ok(doc_addresses) = searcher.search(&AllQuery, &DocSetCollector) {
            for doc_address in doc_addresses {
                if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
                    if let Some(json) = doc
                        .get_first(self.extracted_json_field)
                        .and_then(|v| v.as_str())
                    {
                        if let Ok(file) = serde_json::from_str::<ExtractedFile>(json) {
                            files.push(file);
                        }
                    }
                }
            }
        }
        files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        files
    }
}

impl SearcherHandle {
    /// BM25 search over the committed index snapshot. Reads index/reader/field handles
    /// only — moved verbatim from the former `TantivySearchEngine::search`.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        if query_str.len() > 10000 {
            return Err("Query too long".to_string());
        }
        let searcher = self.reader.searcher();

        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.symbol_field,
                self.docstring_field,
                self.file_path_parts_field,
            ],
        );

        query_parser.set_field_boost(self.symbol_field, 4.0);
        query_parser.set_field_boost(self.docstring_field, 2.0);
        query_parser.set_field_boost(self.file_path_parts_field, 1.0);

        if query_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let query = match parse_query_catching_panic(|| query_parser.parse_query(query_str)) {
            Some(Ok(q)) => q,
            // Primary parse failed or panicked (e.g. a bare `*` in tantivy 0.26):
            // strip special characters to spaces and retry as a plain term query.
            _ => {
                let escaped: String = query_str
                    .to_lowercase()
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c.is_whitespace() {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect();
                if escaped.trim().is_empty() {
                    return Ok(Vec::new());
                }
                match parse_query_catching_panic(|| query_parser.parse_query(&escaped)) {
                    Some(Ok(q)) => q,
                    Some(Err(e)) => return Err(e.to_string()),
                    None => return Ok(Vec::new()),
                }
            }
        };

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit).order_by_score())
            .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        let query_lower = query_str.to_lowercase();

        for (score, doc_address) in top_docs {
            let doc = searcher
                .doc::<TantivyDocument>(doc_address)
                .map_err(|e| e.to_string())?;

            let file_path = doc
                .get_first(self.file_path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let extracted_json = doc
                .get_first(self.extracted_json_field)
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            let extracted_file: ExtractedFile = serde_json::from_str(extracted_json)
                .unwrap_or_else(|_| ExtractedFile {
                    file_path: file_path.clone(),
                    total_lines: 0,
                    symbols: Vec::new(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                });

            let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

            // Capture the file's total line count before the partial moves of
            // `.symbols`/`.literals` below borrow `extracted_file` apart.
            let total_lines = extracted_file.total_lines;
            let all_symbols = extracted_file.symbols;
            let mut matched_symbols: Vec<ExtractedSymbol> = all_symbols
                .iter()
                .filter(|sym| {
                    !query_terms.is_empty()
                        && query_terms.iter().all(|&term| {
                            sym.name.to_lowercase().contains(term)
                                || sym
                                    .docstring
                                    .as_ref()
                                    .is_some_and(|d| d.to_lowercase().contains(term))
                                || crate::parser::split_identifier(&sym.name)
                                    .iter()
                                    .any(|t| t.to_lowercase().contains(term))
                        })
                })
                .cloned()
                .collect();
            // The doc ranked in via some field (symbol/docstring/path). If the symbol
            // substring filter is empty (e.g. matched via docstring or path tokens), fall
            // back to the file's own symbols so the detail view never renders an empty
            // file header (Child 03 — OR/AND render consistency).
            let symbol_fallback = matched_symbols.is_empty();
            if symbol_fallback {
                matched_symbols = all_symbols;
            }

            let matched_literals: Vec<String> = extracted_file
                .literals
                .into_iter()
                .filter(|lit| {
                    !query_terms.is_empty()
                        && query_terms
                            .iter()
                            .all(|&term| lit.to_lowercase().contains(term))
                })
                .collect();

            results.push(SearchResult {
                file_path,
                score,
                total_lines,
                matched_symbols,
                matched_literals,
                symbol_fallback,
            });
        }

        Ok(results)
    }
}

impl SearchEngine for TantivySearchEngine {
    fn index_files(&mut self, paths: &[&str]) -> Result<(), String> {
        self.index_files_changed(paths).map(|_| ())
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        self.searcher_handle().search(query, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_tokenize_path_helper() {
        assert_eq!(tokenize_path("src/lib.rs"), "src lib rs");
        assert_eq!(tokenize_path("a\\b\\c.js"), "a b c js");
        assert_eq!(tokenize_path("main.rs"), "main rs");
    }

    #[test]
    fn test_normalize_relative_path_helper() {
        assert_eq!(
            normalize_relative_path(Path::new("./src/lib.rs")),
            "src/lib.rs"
        );
        assert_eq!(
            normalize_relative_path(Path::new("src\\lib.rs")),
            "src/lib.rs"
        );
    }

    #[test]
    fn test_engine_basic_indexing_and_search() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let file1 = src_dir.join("lib.rs");
        fs::write(&file1, "pub fn calculate_prime_numbers() {}").unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();

        // Initially search should be empty
        let res = engine.search("calculate_prime_numbers", 10).unwrap();
        assert_eq!(res.len(), 0);

        // Index the files
        if let Err(e) = engine.index_files(&[&temp.path().to_string_lossy()]) {
            println!("test_engine_basic_indexing_and_search index error: {}", e);
        }

        // Search again
        let res = engine.search("calculate_prime_numbers", 10).unwrap();
        println!("basic search results len: {}", res.len());
        if res.is_empty() {
            // Let's print out what files were registered or if indexing was skipped
            println!(
                "Indexed files mtimes map: {:?}",
                engine.get_indexed_mtimes()
            );
        }
        assert_eq!(res.len(), 1);
        assert!(res[0].file_path.contains("lib.rs"));
        assert_eq!(res[0].matched_symbols[0].name, "calculate_prime_numbers");
    }

    #[test]
    fn test_engine_ranking_weights() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // File A: QueryTerm only in a string literal — literals are NOT indexed
        // (Child 03, details-layer only), so file_a must not be returned by search.
        let file_a = src_dir.join("file_a.rs");
        fs::write(&file_a, "pub fn alpha() { let x = \"QueryTerm\"; }").unwrap();

        // File B: QueryTerm in symbol name (weight = 4.0)
        let file_b = src_dir.join("file_b.rs");
        fs::write(&file_b, "pub fn QueryTerm() {}").unwrap();

        // File C: QueryTerm in docstring (weight = 2.0)
        let file_c = src_dir.join("file_c.rs");
        fs::write(&file_c, "/// QueryTerm\npub fn hello() {}").unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();

        let res = engine.search("QueryTerm", 10).unwrap();
        // Only the symbol- and docstring-matched files rank; the literal-only file is gone.
        assert_eq!(
            res.len(),
            2,
            "literal-only file must not be searchable, got: {:?}",
            res
        );
        // Best match should be File B (symbol, weight 4)
        assert!(
            res[0].file_path.contains("file_b.rs"),
            "Expected file_b.rs first, got: {:?}",
            res
        );
        // Second should be File C (docstring, weight 2)
        assert!(
            res[1].file_path.contains("file_c.rs"),
            "Expected file_c.rs second, got: {:?}",
            res
        );
        // File A (literal only) must be absent.
        assert!(
            !res.iter().any(|r| r.file_path.contains("file_a.rs")),
            "literal-only file_a must not appear, got: {:?}",
            res
        );
    }

    #[test]
    fn test_engine_incremental_indexing() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let file1 = src_dir.join("lib.rs");
        fs::write(&file1, "pub fn first_func() {}").unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();

        // Read initial modification time of index directory
        let initial_mtime = fs::metadata(&index_dir).unwrap().modified().unwrap();

        // Index again immediately with no changes
        std::thread::sleep(std::time::Duration::from_millis(50));
        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();

        let final_mtime = fs::metadata(&index_dir).unwrap().modified().unwrap();
        assert_eq!(initial_mtime, final_mtime);

        // Now modify a file
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&file1, "pub fn first_func_modified() {}").unwrap();
        // Force update the mtime of the file
        let new_mtime = filetime::FileTime::from_system_time(
            std::time::SystemTime::now() + std::time::Duration::from_secs(10),
        );
        filetime::set_file_mtime(&file1, new_mtime).unwrap();

        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();
        let after_modify_mtime = fs::metadata(&index_dir).unwrap().modified().unwrap();
        assert_ne!(initial_mtime, after_modify_mtime);

        // Search for new symbol
        let res = engine.search("first_func_modified", 10).unwrap();
        assert_eq!(res.len(), 1);
    }

    #[test]
    fn test_engine_corrupt_recovery() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        fs::create_dir_all(&index_dir).unwrap();

        // Corrupt the index directory with invalid meta.json
        fs::write(index_dir.join("meta.json"), "{invalid json}").unwrap();

        // Instantiating the engine should auto-recover
        let engine = TantivySearchEngine::new(&index_dir.to_string_lossy());
        assert!(engine.is_ok());
    }

    #[test]
    fn test_format_version_sidecar_written_and_stable() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let file1 = src_dir.join("lib.rs");
        fs::write(&file1, "pub fn alpha_symbol() {}").unwrap();

        // First instantiation: writes the sidecar and indexes content.
        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();
        let version_path = index_dir.join(EXTRACTION_FORMAT_FILE);
        assert_eq!(
            fs::read_to_string(&version_path).unwrap().trim(),
            EXTRACTION_FORMAT_VERSION
        );
        assert_eq!(engine.search("alpha_symbol", 10).unwrap().len(), 1);
        drop(engine);

        // Second instantiation with a matching sidecar must NOT rebuild: the previously
        // indexed content survives (a rebuild would wipe it).
        let engine2 = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        assert_eq!(engine2.search("alpha_symbol", 10).unwrap().len(), 1);
        assert_eq!(
            fs::read_to_string(&version_path).unwrap().trim(),
            EXTRACTION_FORMAT_VERSION
        );
    }

    #[test]
    fn test_format_version_mismatch_rebuilds_exactly_once() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let file1 = src_dir.join("lib.rs");
        fs::write(&file1, "pub fn beta_symbol() {}").unwrap();

        // Seed an index with content under the current format.
        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();
        assert_eq!(engine.search("beta_symbol", 10).unwrap().len(), 1);
        drop(engine);

        // Simulate a pre-upgrade index: stamp an outdated version.
        let version_path = index_dir.join(EXTRACTION_FORMAT_FILE);
        fs::write(&version_path, "v1-legacy").unwrap();

        // Instantiation with a stale sidecar rebuilds once: the stored docs are wiped (so the
        // search is empty until re-indexed) and the sidecar is restamped to the current
        // version.
        let mut engine2 = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        assert_eq!(
            engine2.search("beta_symbol", 10).unwrap().len(),
            0,
            "outdated format should have triggered a wipe"
        );
        assert_eq!(
            fs::read_to_string(&version_path).unwrap().trim(),
            EXTRACTION_FORMAT_VERSION
        );

        // Re-index after the rebuild, then a subsequent instantiation must be stable — no
        // second rebuild (content survives), proving the reindex fires exactly once.
        engine2
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();
        assert_eq!(engine2.search("beta_symbol", 10).unwrap().len(), 1);
        drop(engine2);

        let engine3 = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        assert_eq!(
            engine3.search("beta_symbol", 10).unwrap().len(),
            1,
            "matching sidecar must not rebuild a second time"
        );
    }

    #[test]
    fn test_query_error_handling() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");

        let engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();

        // Search with query containing syntax errors / special characters should not panic
        let res = engine.search("AND OR NOT * : ()", 10);
        if let Err(ref e) = res {
            println!("test_query_error_handling failed with error: {}", e);
        }
        assert!(res.is_ok());
    }
}
