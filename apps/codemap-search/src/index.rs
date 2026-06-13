use crate::parser::{CodeExtractor, ExtractedFile, ExtractedLiteral, ExtractedSymbol};
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
    pub matched_literals: Vec<ExtractedLiteral>,
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
    literal_field: Field,
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
    pub literal_field: Field,
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

/// Per-literal cap on INDEXED characters (the full value stays in `extracted_json` for
/// the detail view): a long SQL/template/fixture string would bloat the term dictionary
/// without adding lookup value beyond its leading words.
const INDEXED_LITERAL_MAX_CHARS: usize = 256;

/// Current extraction-format version. Bump whenever the `ExtractedFile` JSON shape changes
/// in a way that requires re-extracting on-disk source (not just a serde-compatible add).
/// `v2` introduced `ExtractedSymbol.owner`: the field is serde-default-compatible (old docs
/// still deserialize), but a one-time reindex is needed so every stored symbol actually
/// carries `owner`. `v3` adds Rust enum-variant symbols and BM25-indexed string literals —
/// both require re-extraction (and the new `literal` schema field forces an index rebuild
/// on its own; the version bump keeps the trigger explicit). `v4` adds line numbers to
/// `ExtractedLiteral` (literals: Vec<String> → Vec<ExtractedLiteral>) and Java/Kotlin
/// enum-constant extraction as kind `variant` — both require re-extraction. `v5` adds C,
/// C++, and Assembly extraction: new extensions (`.c`, `.h`, `.cpp`, `.cc`, `.cxx`,
/// `.hpp`, `.hh`, `.hxx`, `.s`, `.S`, `.asm`) now enter the filesystem walk and produce
/// symbols; without the bump these files would remain unindexed from a v4 index. `v6`
/// corrects C++ extraction: reference-returning functions/methods (`T& f()`, `T& operator=`,
/// `auto&& g()`) now extract a symbol (previously dropped); function-local vexing-parse
/// declarations (`std::lock_guard lock(m);`) no longer leak as `fn` symbols; and inline
/// in-class method definitions now honor the access specifier for the `exported` flag
/// (private/protected inline methods are no longer reported as exported). All three change
/// the extracted symbol set, so a re-extraction is required. `v7` makes an owned member's
/// owner (enclosing-type) name searchable: the owner name and its split sub-tokens are now
/// written into the symbol field alongside the member's own tokens, so an owner-qualified
/// query (e.g. "StorageFactory get") retrieves and selects the owned member instead of only
/// surfacing the class declaration or a same-named member of another type. This is an
/// indexed-content change with NO new tantivy schema field — `Index::open_or_create` would
/// happily reuse a v6 index without re-emitting the owner tokens — so the bump is what forces
/// the one-time reindex that populates them. Each bump rebuilds exactly once.
const EXTRACTION_FORMAT_VERSION: &str = "v7-owner-tokens-indexed";

impl TantivySearchEngine {
    pub fn new(index_path: &str) -> Result<Self, String> {
        let mut schema_builder = Schema::builder();
        let file_path_field = schema_builder.add_text_field("file_path", STRING | STORED);
        let file_path_parts_field = schema_builder.add_text_field("file_path_parts", TEXT);
        let symbol_field = schema_builder.add_text_field("symbol", TEXT | STORED);
        let docstring_field = schema_builder.add_text_field("docstring", TEXT | STORED);
        // String literals ARE indexed (low boost): config defaults ("8000") and error
        // messages live only in literals, and agents search those words. The full values
        // stay in `extracted_json` for the detail view; `grep` remains the exact-match
        // tool (no index lag, regex).
        let literal_field = schema_builder.add_text_field("literal", TEXT);
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
        let version_mismatch = stored_version.as_deref() != Some(EXTRACTION_FORMAT_VERSION);
        // Wipe only an index that actually HAS content under an outdated format. A fresh
        // or empty index dir (no tantivy meta.json — e.g. a never-indexed repo) is created
        // in place: two server processes spawning concurrently on the same repo would
        // otherwise BOTH take the wipe path and delete each other's just-created files.
        let needs_wipe = version_mismatch && path.join("meta.json").exists();

        // Try to open or create the index directory. Rebuild from scratch if metadata is
        // corrupted OR a populated index has an outdated extraction format — both take the
        // same `remove_dir_all` + recreate path so the index is rebuilt exactly once. The
        // pre-wipe retry absorbs a concurrent sibling's transient create: destructive
        // recovery must be the last resort, not the loser's reflex in a startup race.
        let open_index = || {
            tantivy::directory::MmapDirectory::open(path)
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    Index::open_or_create(dir, schema.clone()).map_err(|e| e.to_string())
                })
        };
        let opened = if needs_wipe {
            Err("extraction format outdated".to_string())
        } else {
            open_index().or_else(|_| {
                std::thread::sleep(std::time::Duration::from_millis(50));
                open_index()
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
        // Written once: after this run the sidecar matches, so `version_mismatch` is false
        // on every subsequent run and neither the rebuild nor this write fires again.
        if version_mismatch {
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
            literal_field,
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

/// Whether an event path would be reached by the shared ignore-aware walk
/// ([`crate::workspace::build_walker`]) descending from the workspace root. The full walk
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
        let is_visible = crate::workspace::build_walker(&current, false)
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
/// and the [`crate::workspace::MAX_INDEXED_FILE_BYTES`] size cap (skip oversize/minified
/// blobs before they are read+parsed, Child 04), and capture a sub-second mtime so a
/// same-second edit still reindexes. Returns `None` when the file is not to be indexed.
fn collect_index_entry(entry_path: &Path, abs_cwd: &Path) -> Option<(String, PathBuf, u64)> {
    let ext = entry_path.extension().and_then(|s| s.to_str())?;
    if !crate::workspace::is_source_extension(ext) {
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
    let rel_path = crate::workspace::relative_index_path(entry_path, abs_cwd);
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
                for entry in crate::workspace::build_walker(path, false)
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
                // Index the owner (enclosing type) name and its split sub-tokens into the
                // SAME symbol field so a query carrying owner + member terms (e.g.
                // "StorageFactory get") retrieves and selects this symbol. Short/common
                // member names (`get`, `add`) have low IDF on their own; the owner tokens
                // give the document the discriminating evidence it was missing. These are
                // ordinary symbol-field terms, so they never outrank an exact name hit on
                // another document — they only lift owner-qualified queries onto the owned
                // symbol. (Matched-symbol selection mirrors this via `term_hits_symbol_name`.)
                if let Some(ref owner) = sym.owner {
                    doc.add_text(self.symbol_field, owner);
                    for token in crate::parser::split_identifier(owner) {
                        doc.add_text(self.symbol_field, &token);
                    }
                }
                if let Some(ref docstring) = sym.docstring {
                    doc.add_text(self.docstring_field, docstring);
                }
            }

            for literal in &extracted.literals {
                if literal.text.chars().count() > INDEXED_LITERAL_MAX_CHARS {
                    let capped: String =
                        literal.text.chars().take(INDEXED_LITERAL_MAX_CHARS).collect();
                    doc.add_text(self.literal_field, &capped);
                } else {
                    doc.add_text(self.literal_field, &literal.text);
                }
            }

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
                let canonical_key = crate::workspace::stored_index_key(path, &abs_cwd);
                // Case-only renames on case-insensitive filesystems (macOS APFS default):
                // the event may arrive under the OLD spelling, which canonicalizes to the
                // new on-disk spelling — the old spelling's doc would otherwise go stale
                // forever, since no later event names it and the watcher-healthy path
                // never runs the full-walk set-difference cleanup.
                if let Ok(raw_rel) = path.strip_prefix(&abs_cwd) {
                    let raw_key = crate::workspace::normalize_relative_path(raw_rel);
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
                    let prefix = format!("{}/", crate::workspace::stored_index_key(path, &abs_cwd));
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
                for entry in crate::workspace::build_walker(path, false)
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
                let prefix = format!("{}/", crate::workspace::stored_index_key(path, &abs_cwd));
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
                let rel_path = crate::workspace::stored_index_key(path, &abs_cwd);
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
            literal_field: self.literal_field,
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

/// Post-rank multiplier for files whose path looks like test/bench scaffolding. Tests and
/// benches repeat domain terms heavily, so raw BM25 term frequency lets them crowd the
/// defining sources out of the top ranks; implementations should surface first (the test
/// files stay in the results, just lower).
const TEST_PATH_SCORE_WEIGHT: f32 = 0.3;

/// Post-rank multiplier when a query term exactly equals a discriminative symbol name
/// defined in the file. An exact identifier in the query ("TransactionReadonly", "put_tb")
/// is the strongest signal the user wants its definition, yet under plain BM25 generic
/// co-terms ("error", "enum") can outvote it via term frequency in unrelated files.
const EXACT_NAME_SCORE_BOOST: f32 = 3.0;

/// A name specific enough that exact equality with a query term means intent: multi-token
/// identifiers (snake/camel compounds) or long single tokens. Short single-word names
/// ("new", "write", "Error") are too generic to treat as a definition request.
fn is_discriminative_name(name: &str) -> bool {
    name.len() >= 8 || crate::parser::split_identifier(name).len() >= 2
}

/// One query term hits a symbol's NAME when it appears in the raw name, any split sub-token
/// of it, or — for owned members — the owner (enclosing-type) name or its sub-tokens. Name
/// evidence is what gates the partial-coverage promotion: a docstring-only partial match must
/// not unlock snippet rendering (observed: one file whose fn docstrings each grazed 3 of 5
/// query words rendered 11 snippets — 32KB — and starved the rest of the detail view).
/// Owner is folded in here (not just at index time) so an owner-qualified query like
/// "StorageFactory get" actually SELECTS the owned `get` symbol for the detail snippet,
/// matching the index-side owner tokens added to the symbol field.
fn term_hits_symbol_name(sym: &ExtractedSymbol, term: &str) -> bool {
    sym.name.to_lowercase().contains(term)
        || crate::parser::split_identifier(&sym.name)
            .iter()
            .any(|t| t.contains(term))
        || sym.owner.as_ref().is_some_and(|owner| {
            owner.to_lowercase().contains(term)
                || crate::parser::split_identifier(owner)
                    .iter()
                    .any(|t| t.contains(term))
        })
}

/// One query term hits one symbol when it appears in the name, the docstring, or any
/// split sub-token of the name. The match-count criterion behind matched-symbol selection.
fn symbol_matches_term(sym: &ExtractedSymbol, term: &str) -> bool {
    term_hits_symbol_name(sym, term)
        || sym
            .docstring
            .as_ref()
            .is_some_and(|d| d.to_lowercase().contains(term))
}

/// Minimum matched-term count for the partial-coverage promotion: half the query terms,
/// rounded up. Only consulted for 3+ term queries — at 1–2 terms it equals "all terms",
/// so the strict baseline already covers it.
fn partial_match_threshold(term_count: usize) -> usize {
    term_count.div_ceil(2)
}

/// True for paths that look like test/bench scaffolding rather than implementation.
fn is_test_like_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    let in_test_dir = ["tests/", "test/", "benches/", "bench/", "__tests__/"]
        .iter()
        .any(|dir| lower.starts_with(dir) || lower.contains(&format!("/{dir}")));
    let file_name = lower.rsplit('/').next().unwrap_or(&lower);
    let stem = file_name.split('.').next().unwrap_or(file_name);
    in_test_dir
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
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
                self.literal_field,
            ],
        );

        query_parser.set_field_boost(self.symbol_field, 4.0);
        query_parser.set_field_boost(self.docstring_field, 2.0);
        query_parser.set_field_boost(self.file_path_parts_field, 1.0);
        // Lowest tier: a literal hit ranks a file in, but never outvotes a symbol or
        // docstring match for the same terms.
        query_parser.set_field_boost(self.literal_field, 1.0);

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

            // Matched-symbol selection. All-terms is the precision baseline, but agent
            // queries carry glue words ("definition", "handler") no symbol can match,
            // which used to classify nearly every multi-word query as fallback — killing
            // snippets and caller/callee annotations. Two promotions relax it:
            //  - exact name: a query term that IS a discriminative symbol name (the same
            //    signal as EXACT_NAME_SCORE_BOOST) marks that symbol matched outright;
            //  - partial coverage: in a 3+ term query, a symbol matching at least half
            //    the terms is matched (glue words no longer veto everything).
            // Selection is ordered exact-first then by matched-term count, so the
            // renderer's symbol cap keeps the strongest evidence instead of line order.
            let mut scored_symbols: Vec<(bool, usize, &ExtractedSymbol)> = all_symbols
                .iter()
                .filter_map(|sym| {
                    if query_terms.is_empty() {
                        return None;
                    }
                    let term_match_count = query_terms
                        .iter()
                        .filter(|&&term| symbol_matches_term(sym, term))
                        .count();
                    let exact_hit = is_discriminative_name(&sym.name)
                        && query_terms.iter().any(|&t| t == sym.name.to_lowercase());
                    let all_terms_hit = term_match_count == query_terms.len();
                    // Partial coverage additionally requires NAME evidence (at least one
                    // term hitting the symbol name itself) — see `term_hits_symbol_name`.
                    let partial_hit = query_terms.len() >= 3
                        && term_match_count >= partial_match_threshold(query_terms.len())
                        && query_terms.iter().any(|&t| term_hits_symbol_name(sym, t));
                    (exact_hit || all_terms_hit || partial_hit)
                        .then_some((exact_hit, term_match_count, sym))
                })
                .collect();
            scored_symbols.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then(b.1.cmp(&a.1))
                    .then(a.2.range.start_line.cmp(&b.2.range.start_line))
            });

            // Post-rank adjustment (see the constants above): an exact discriminative
            // symbol-name hit boosts the file, a test/bench-looking path demotes it. Both
            // re-rank only within the BM25 top `limit` — the candidate set is unchanged.
            let exact_name_hit = scored_symbols.iter().any(|(exact, _, _)| *exact);
            let mut adjusted_score = score;
            if exact_name_hit {
                adjusted_score *= EXACT_NAME_SCORE_BOOST;
            }
            if is_test_like_path(&file_path) {
                adjusted_score *= TEST_PATH_SCORE_WEIGHT;
            }
            let mut matched_symbols: Vec<ExtractedSymbol> = scored_symbols
                .into_iter()
                .map(|(_, _, sym)| sym.clone())
                .collect();
            // The doc ranked in via some field (symbol/docstring/path). If the symbol
            // selection is empty (e.g. matched via docstring or path tokens), fall
            // back to the file's own symbols so the detail view never renders an empty
            // file header (Child 03 — OR/AND render consistency).
            let symbol_fallback = matched_symbols.is_empty();
            if symbol_fallback {
                matched_symbols = all_symbols;
            }

            // Matched-literal selection mirrors the symbol promotions: all-terms baseline,
            // plus an exact-value hit (a term that IS the whole literal, e.g. "8000") and
            // half-coverage for 3+ term queries (an error-message literal shouldn't be
            // vetoed by one glue word). Match decisions use `text`; `line` is carried
            // through for the detail view to render `[L<n>]`.
            let matched_literals: Vec<ExtractedLiteral> = extracted_file
                .literals
                .into_iter()
                .filter(|lit| {
                    if query_terms.is_empty() {
                        return false;
                    }
                    let lit_lower = lit.text.to_lowercase();
                    let term_match_count = query_terms
                        .iter()
                        .filter(|&&term| lit_lower.contains(term))
                        .count();
                    term_match_count == query_terms.len()
                        || query_terms.iter().any(|&t| t == lit_lower)
                        || (query_terms.len() >= 3
                            && term_match_count >= partial_match_threshold(query_terms.len()))
                })
                .collect();

            results.push(SearchResult {
                file_path,
                score: adjusted_score,
                total_lines,
                matched_symbols,
                matched_literals,
                symbol_fallback,
            });
        }

        // Re-sort by the adjusted scores (BM25 order only holds for the raw scores).
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

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

        // File A: QueryTerm only in a string literal — indexed at the lowest boost, so the
        // file ranks in (v3) but never above a symbol or docstring match.
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
        assert_eq!(res.len(), 3, "all three tiers rank in, got: {:?}", res);
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
        // File A (literal only, weight 1) ranks last, and the matched literal surfaces
        // as an exact-value hit while the symbol list stays fallback.
        assert!(
            res[2].file_path.contains("file_a.rs"),
            "Expected literal-only file_a last, got: {:?}",
            res
        );
        assert!(res[2].symbol_fallback);
        assert_eq!(res[2].matched_literals.len(), 1);
        assert_eq!(res[2].matched_literals[0].text, "QueryTerm");
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
