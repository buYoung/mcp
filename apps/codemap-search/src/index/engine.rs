use crate::parser::{
    ExtractedFile, ExtractedLiteral, ExtractedSymbol, MacroInputStamp, StaticCollectionEdge,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use tantivy::directory::{Directory, INDEX_WRITER_LOCK};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexSettings, ReloadPolicy, TantivyDocument, Term};

fn static_collection_edges_are_empty(edges: &&[StaticCollectionEdge]) -> bool {
    edges.is_empty()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredExtractedFileRef<'a> {
    #[serde(flatten)]
    extracted: &'a ExtractedFile,
    #[serde(skip_serializing_if = "static_collection_edges_are_empty")]
    static_collection_edges: &'a [StaticCollectionEdge],
    #[serde(skip_serializing_if = "Option::is_none")]
    event_input: Option<StoredEventSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow_digest: Option<&'a str>,
}

/// The stored form of an event input. The source text is not stored: publication reads the
/// file and uses it only while its digest still matches the text that was indexed.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEventSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unavailable_reason: Option<String>,
}

impl StoredEventSource {
    fn new(input: &crate::events::EventInput) -> Self {
        Self {
            source_length: input.source.as_ref().map(String::len),
            source_digest: input
                .source
                .as_deref()
                .map(|source| crate::implementations::digest(source.as_bytes())),
            unavailable_reason: input.unavailable_reason.clone(),
        }
    }

    /// The indexed source of `path` under `root`, or `None` once the file no longer holds it.
    fn read(&self, root: &Path, path: &str) -> Option<String> {
        let digest = self.source_digest.as_deref()?;
        std::fs::read_to_string(root.join(path))
            .ok()
            .filter(|source| crate::implementations::digest(source.as_bytes()) == digest)
    }
}

/// The first eight bytes of the BLAKE3 hash, kept in fast fields to identify a file's content
/// or a stored payload without reading the document store.
fn short_hash(bytes: &[u8]) -> u64 {
    let hash = blake3::hash(bytes);
    u64::from_le_bytes(hash.as_bytes()[..8].try_into().expect("eight hash bytes"))
}

/// An indexed file's modification stamp and the hash of the content it was extracted from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IndexedStamp {
    mtime: u64,
    content_hash: Option<u64>,
}

impl IndexedStamp {
    /// Whether `disk_path` still holds the indexed content although its modification time
    /// changed, as after a touch or a checkout that rewrote identical content.
    fn matches_content(&self, disk_path: &Path) -> bool {
        self.content_hash.is_some_and(|hash| {
            std::fs::read(disk_path).is_ok_and(|bytes| short_hash(&bytes) == hash)
        })
    }
}

/// The recorded macro-expansion inputs of a stored document, decoded without the rest of it.
#[derive(Deserialize)]
struct StoredMacroInputs {
    #[serde(default)]
    navigation: Option<StoredNavigationInputs>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredNavigationInputs {
    #[serde(default)]
    macro_expansion: Option<StoredExpansionInputs>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredExpansionInputs {
    #[serde(default)]
    inputs: Vec<MacroInputStamp>,
    #[serde(default)]
    active_lines: Option<serde::de::IgnoredAny>,
}

/// What one native file's last macro expansion depended on.
struct RecordedExpansion {
    inputs: Vec<MacroInputStamp>,
    is_expanded: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredExtractedFile {
    #[serde(flatten)]
    extracted: ExtractedFile,
    #[serde(default)]
    static_collection_edges: Vec<StaticCollectionEdge>,
    #[serde(default)]
    event_input: Option<StoredEventSource>,
    #[serde(default)]
    flow_digest: Option<String>,
}

/// One stored document as decoded for a published generation. Generations share it while the
/// document's payload and mtime are unchanged, so a publication decodes only changed documents.
pub(super) struct PublishedEntry {
    payload: Option<u64>,
    mtime: Option<u64>,
    file: std::sync::Arc<ExtractedFile>,
    /// The stored event input. The source text itself lives in the published event index.
    event_input: Option<StoredEventSource>,
    flow_digest: Option<String>,
}

impl PublishedEntry {
    fn new(
        payload: Option<u64>,
        mtime: Option<u64>,
        extracted: ExtractedFile,
        event_input: Option<StoredEventSource>,
        flow_digest: Option<String>,
    ) -> Self {
        Self {
            payload,
            mtime,
            file: std::sync::Arc::new(extracted),
            event_input,
            flow_digest,
        }
    }

    fn is_current(&self, payload: Option<u64>, mtime: Option<u64>) -> bool {
        payload.is_some() && self.payload == payload && self.mtime == mtime
    }
}

/// A stored document decoded by the first state read, held for the first publication so it is
/// not decoded twice. Its event source is not held: publication reads only the sources that fit
/// the snapshot budget.
pub(super) struct DecodedEntry {
    entry: PublishedEntry,
    static_collection_edges: Vec<StaticCollectionEdge>,
}

impl DecodedEntry {
    fn decoded(payload: Option<u64>, mtime: Option<u64>, stored: StoredExtractedFile) -> Self {
        let entry = PublishedEntry::new(
            payload,
            mtime,
            stored.extracted,
            stored.event_input,
            stored.flow_digest,
        );
        Self {
            entry,
            static_collection_edges: stored.static_collection_edges,
        }
    }

    fn publish(self) -> (std::sync::Arc<PublishedEntry>, Vec<StaticCollectionEdge>) {
        (
            std::sync::Arc::new(self.entry),
            self.static_collection_edges,
        )
    }
}

/// Decoded entries per segment, indexed by document id. Segments are immutable, so a live
/// document at a recorded location is the document that was decoded there.
type EntriesBySegment<T> = HashMap<tantivy::index::SegmentId, Vec<Option<T>>>;

/// A segment's fast-field columns, which identify its documents without reading the store.
struct DocumentColumns {
    path: Option<tantivy::columnar::StrColumn>,
    mtime: Option<tantivy::columnar::Column<u64>>,
    content_hash: Option<tantivy::columnar::Column<u64>>,
    payload_hash: Option<tantivy::columnar::Column<u64>>,
}

impl DocumentColumns {
    fn open(segment: &tantivy::SegmentReader) -> Result<Self, String> {
        let fields = segment.fast_fields();
        let error =
            |error: tantivy::TantivyError| format!("indexed document columns open failed: {error}");
        Ok(Self {
            path: fields.str("file_path").map_err(error)?,
            mtime: fields.column_opt("mtime").map_err(error)?,
            content_hash: fields.column_opt("content_hash").map_err(error)?,
            payload_hash: fields.column_opt("payload_hash").map_err(error)?,
        })
    }

    fn path(&self, doc_id: tantivy::DocId) -> Option<String> {
        let column = self.path.as_ref()?;
        let ord = column.term_ords(doc_id).next()?;
        let mut path = String::new();
        column.ord_to_str(ord, &mut path).ok()?.then_some(path)
    }

    fn mtime(&self, doc_id: tantivy::DocId) -> Option<u64> {
        self.mtime.as_ref()?.first(doc_id)
    }

    fn content_hash(&self, doc_id: tantivy::DocId) -> Option<u64> {
        self.content_hash.as_ref()?.first(doc_id)
    }

    fn payload_hash(&self, doc_id: tantivy::DocId) -> Option<u64> {
        self.payload_hash.as_ref()?.first(doc_id)
    }
}

/// The entries of one published generation.
pub(super) struct PublishedEntries {
    generation: u64,
    by_path: HashMap<String, std::sync::Arc<PublishedEntry>>,
    by_segment: EntriesBySegment<std::sync::Arc<PublishedEntry>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRankingSignal {
    pub symbol_name: String,
    pub symbol_owner: Option<String>,
    pub exact_name_hit: bool,
    pub exact_boost_applied: bool,
    pub matched_token_count: usize,
    pub query_token_count: usize,
    pub owner_match_count: usize,
    pub path_match_count: usize,
    pub same_name_candidate_count: usize,
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
    /// Compact explanation data for the strongest matched symbol. Rendering turns this into
    /// bounded `match_reason` / `ambiguity` hints without exposing score internals.
    pub ranking_signal: Option<SearchRankingSignal>,
    /// The qualified-name string literal (`encoding::base64::decode`) this file matched when a
    /// qualified query word exactly equals one of its string literals. Carries the dispatch/lookup
    /// literal so the renderer can label the file in the tail / `match_reason` instead of
    /// mislabeling it by a weak module symbol — a legacy dispatch table reads as itself, not as
    /// "a plain mod.rs". Computed independently of the (currently held) qualified-literal score
    /// lift (see `ENABLE_QUALIFIED_LITERAL_BOOST`), so this label survives with the lift off.
    /// `None` when no qualified literal matched.
    pub qualified_literal_hit: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQueryContext {
    pub language_hint: Option<String>,
    pub extension_hint: Option<String>,
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
    pub(super) index: Index,
    pub(super) reader: IndexReader,
    pub(super) file_path_field: Field,
    pub(super) file_path_parts_field: Field,
    pub(super) symbol_field: Field,
    pub(super) docstring_field: Field,
    pub(super) literal_field: Field,
    pub(super) definition_body_field: Field,
    pub(super) reference_field: Field,
    pub(super) format_text_field: Field,
    pub(super) extracted_json_field: Field,
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
    pub definition_body_field: Field,
    pub reference_field: Field,
    pub format_text_field: Field,
    pub extracted_json_field: Field,
    pub mtime_field: Field,
    content_hash_field: Field,
    payload_hash_field: Field,

    // In-memory snapshot of the on-disk path→mtime map. Lazily populated once from the
    // index (one `AllQuery`) and then maintained incrementally, so a long-lived engine
    // (MCP mode) no longer runs an `AllQuery` over the whole index before every search
    // (Child 04). Reflects only what is actually committed to the index.
    indexed_mtimes_cache: Option<HashMap<String, IndexedStamp>>,
    // Recorded macro-expansion inputs (the file, its headers, build settings) per native file,
    // loaded and maintained with `indexed_mtimes_cache`, so a full refresh reprocesses only
    // the native files whose inputs changed.
    macro_inputs_cache: Option<HashMap<String, RecordedExpansion>>,
    pub(super) is_waiting_for_writer: bool,
    // Full-refresh inputs, retained across reopen so dependency/config refreshes never
    // widen a caller's explicit file or directory selection to the process cwd.
    indexed_roots: Vec<String>,
    macro_config_stamp: Option<String>,
    event_config_stamp: Option<String>,
    // Entries of the last published generation, reused by the next publication for documents
    // that are unchanged.
    published_entries: Option<PublishedEntries>,
    // Entries of the generation being published, kept once the caller confirms publication.
    next_published_entries: Option<PublishedEntries>,
    // Whether this engine publishes generations. Only the background indexer does, so other
    // users (the CLI) do not decode or hold entries.
    is_publishing: bool,
    // Documents decoded by the first read of the stored state, for the first publication.
    loaded_entries: EntriesBySegment<DecodedEntry>,
}

/// Filename of the sidecar that stamps the extraction-format version of the stored
/// `extracted_json` documents. Lives alongside tantivy's own files in the index dir.
const EXTRACTION_FORMAT_FILE: &str = "codemap.format";
const INDEXED_ROOTS_FILE: &str = "indexed-roots.json";

/// Per-literal cap on INDEXED characters (the full value stays in `extracted_json` for
/// the detail view): a long SQL/template/fixture string would bloat the term dictionary
/// without adding lookup value beyond its leading words.
pub(super) const INDEXED_LITERAL_MAX_CHARS: usize = 256;

/// Share of deleted documents above which a segment is rewritten. tantivy's default never
/// merges for deletions alone, so files reindexed in place would keep their old copies.
const DELETED_RATIO_BEFORE_MERGE: f32 = 0.1;

fn merge_policy() -> Box<dyn tantivy::indexer::MergePolicy> {
    let mut policy = tantivy::indexer::LogMergePolicy::default();
    policy.set_del_docs_ratio_before_merge(DELETED_RATIO_BEFORE_MERGE);
    Box::new(policy)
}

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
/// the one-time reindex that populates them. `v9` adds two indexed auxiliary fields
/// (`definition_body`, `reference`) populated from optional per-language `tags.scm` hooks,
/// while keeping the stored `ExtractedFile` JSON contract unchanged. `v10` stores bounded
/// static TypeScript/JavaScript collection write/read endpoints used by search relation hints.
/// `v11` extends those endpoints to every supported high-level language with a safe collection
/// identity model. `v12` preserves qualified owner identity and republishes bounded relation
/// indexes alongside the codemap snapshot. Each bump rebuilds exactly once.
// Composite source masking and SQL declarations change what a file contributes to a persisted
// document, so existing local indexes must be rebuilt once. The schema/JSON shape is unchanged.
// `v17` adds Swift, Dart, Scala, Groovy/Gradle, and PowerShell source extensions plus their
// persisted symbols/navigation metadata. Existing local indexes must therefore be rebuilt once
// even though the Tantivy schema and MCP JSON contracts are unchanged.
// `v18` adds Nix symbols, static build relationships, and a new source extension under the same
// persisted contract, requiring one re-extraction of existing workspaces.
// v19 decodes Rust use groups and Go imports from syntax instead of shared text heuristics.
// v20 fixes PowerShell function kinds/assignment ranges and records static member call names.
// v21 indexes complete Dart member declarations instead of their signatures alone.
// v22 adds immutable bindings, operators/accessors, and assigned Lua functions.
// v23 also retains external Dart function declarations found during the replay.
// v24 drops fake Groovy constructors produced when quoted method parsing recovers.
// v25 bundles Groovy quoted declarations and the Zsh scanner termination fix.
// v35 persists TypeScript/JavaScript/C/C++ data-member names for related-terms evidence;
// they are not symbols, so search ranking and codemap views stay unchanged.
// v36 types more call receivers (fields, static types, bases, C++ and PowerShell parameters)
// in the persisted implementation facts used for caller attribution.
// v39 persists upper-snake-case constant names by line for the read trailer of shared keys.
// v40 names the declaration of TypeScript `{ type Name }` import specifiers and persists
// TypeScript/JavaScript exports that rename a declaration, for the read trailer of importers.
// v43 compresses stored payloads with zstd, stores event source digests instead of their text,
// drops stored flow summaries and unread stored fields, and adds fast fields for paths, mtimes
// and content and payload hashes.
const EXTRACTION_FORMAT_VERSION: &str = "v43-compact-store";

/// Stored payloads are JSON, which zstd keeps at about half of lz4's compressed size.
fn index_settings() -> IndexSettings {
    IndexSettings {
        docstore_compression: tantivy::store::Compressor::Zstd(
            tantivy::store::ZstdCompressor::default(),
        ),
        ..IndexSettings::default()
    }
}

/// Serializes the destructive format-upgrade branch across MCP server processes. The owner PID
/// lets a later process reclaim a lock left by a crash, while live owners are never replaced.
struct FormatUpgradeLock {
    path: PathBuf,
    token: String,
}

impl FormatUpgradeLock {
    fn acquire(index_path: &Path) -> Result<Self, String> {
        let parent = index_path.parent().unwrap_or_else(|| Path::new("."));
        let name = index_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("codemap-index");
        // Keep this outside the index directory: the upgrade itself removes that directory.
        let path = parent.join(format!(".{name}.codemap-format-upgrade.lock"));
        let token = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("failed to create index upgrade lock token: {error}"))?
                .as_nanos()
        );
        let candidate_path =
            parent.join(format!(".{name}.codemap-format-upgrade.candidate-{token}"));
        let mut candidate = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate_path)
            .map_err(|error| format!("failed to create index upgrade lock candidate: {error}"))?;
        if let Err(error) = write!(candidate, "pid={}\ntoken={token}\n", std::process::id())
            .and_then(|()| candidate.sync_all())
        {
            let _ = std::fs::remove_file(&candidate_path);
            return Err(format!("failed to prepare index upgrade lock: {error}"));
        }
        drop(candidate);
        for _ in 0..600 {
            match std::fs::hard_link(&candidate_path, &path) {
                Ok(()) => {
                    let _ = std::fs::remove_file(&candidate_path);
                    return Ok(Self { path, token });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if let Err(error) = Self::reclaim_stale(&path) {
                        let _ = std::fs::remove_file(&candidate_path);
                        return Err(error);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(error) => {
                    let _ = std::fs::remove_file(&candidate_path);
                    return Err(format!("failed to acquire index upgrade lock: {error}"));
                }
            }
        }
        let _ = std::fs::remove_file(&candidate_path);
        Err("timed out waiting for a concurrent index format upgrade".to_string())
    }

    fn reclaim_stale(path: &Path) -> Result<(), String> {
        let contents = match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("failed to read index upgrade lock: {error}")),
        };
        let owner_pid = contents
            .lines()
            .find_map(|line| line.strip_prefix("pid="))
            .and_then(|pid| pid.parse::<u32>().ok());
        if let Some(owner_pid) = owner_pid {
            if Self::process_is_alive(owner_pid) {
                return Ok(());
            }
        } else {
            let is_old_enough = std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= std::time::Duration::from_secs(2));
            if !is_old_enough {
                return Ok(());
            }
        }

        // Rename, rather than unlink, so a contender can never remove a freshly acquired
        // lock after observing an old crashed owner's file.
        let stale_path = path.with_extension(format!(
            "stale-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("failed to reclaim index upgrade lock: {error}"))?
                .as_nanos()
        ));
        match std::fs::rename(path, &stale_path) {
            Ok(()) => {
                let _ = std::fs::remove_file(stale_path);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "failed to reclaim stale index upgrade lock: {error}"
            )),
        }
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        let output = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output();
        match output {
            Ok(output) if output.status.success() => true,
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
                // `EPERM` still proves the process exists. Only an explicit ESRCH-style
                // response permits reclaim; all other command failures fail closed.
                !(stderr.contains("no such process") || stderr.contains("not found"))
            }
            Err(_) => true,
        }
    }

    #[cfg(not(unix))]
    fn process_is_alive(_pid: u32) -> bool {
        // Without a portable PID liveness primitive, fail closed rather than risk removing a
        // live sibling's lock. Unix hosts (the supported deployment environment) recover it.
        true
    }
}

impl Drop for FormatUpgradeLock {
    fn drop(&mut self) {
        let is_ours = std::fs::read_to_string(&self.path)
            .ok()
            .is_some_and(|contents| {
                contents
                    .lines()
                    .any(|line| line == format!("token={}", self.token))
            });
        if is_ours {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl TantivySearchEngine {
    pub fn new(index_path: &str) -> Result<Self, String> {
        let mut schema_builder = Schema::builder();
        // Paths, mtimes and hashes are also fast fields, so refresh decisions and publication
        // identify documents without reading the store.
        let file_path_field = schema_builder.add_text_field("file_path", STRING | STORED | FAST);
        let file_path_parts_field = schema_builder.add_text_field("file_path_parts", TEXT);
        // Symbol names and docstrings are read from `extracted_json`; these fields only search.
        let symbol_field = schema_builder.add_text_field("symbol", TEXT);
        let docstring_field = schema_builder.add_text_field("docstring", TEXT);
        // String literals ARE indexed (low boost): config defaults ("8000") and error
        // messages live only in literals, and agents search those words. The full values
        // stay in `extracted_json` for the detail view; `grep` remains the exact-match
        // tool (no index lag, regex).
        let literal_field = schema_builder.add_text_field("literal", TEXT);
        let definition_body_field = schema_builder.add_text_field("definition_body", TEXT);
        let reference_field = schema_builder.add_text_field("reference", TEXT);
        // Raw data/markup text is searchable at a deliberately lower boost and is not stored.
        let format_text_field = schema_builder.add_text_field("format_text", TEXT);
        let extracted_json_field = schema_builder.add_text_field("extracted_json", STORED);
        let mtime_field = schema_builder.add_u64_field("mtime", STORED | FAST);
        let content_hash_field = schema_builder.add_u64_field("content_hash", FAST);
        let payload_hash_field = schema_builder.add_u64_field("payload_hash", FAST);
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
        let mut version_mismatch = stored_version.as_deref() != Some(EXTRACTION_FORMAT_VERSION);
        // The lock covers the full destructive upgrade transaction. A waiting process must
        // re-read the sidecar after it wins the lock, because the prior process may already
        // have rebuilt and stamped the directory while this one waited.
        let upgrade_lock = if version_mismatch {
            Some(FormatUpgradeLock::acquire(path)?)
        } else {
            None
        };
        if upgrade_lock.is_some() {
            version_mismatch = std::fs::read_to_string(&version_path)
                .ok()
                .map(|version| version.trim().to_string())
                .as_deref()
                != Some(EXTRACTION_FORMAT_VERSION);
        }
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
                    Index::builder()
                        .schema(schema.clone())
                        .settings(index_settings())
                        .open_or_create(dir)
                        .map_err(|e| e.to_string())
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
                if path.exists() {
                    std::fs::remove_dir_all(path).map_err(|error| error.to_string())?;
                }
                std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
                let directory =
                    tantivy::directory::MmapDirectory::open(path).map_err(|e| e.to_string())?;
                Index::create(directory, schema.clone(), index_settings())
                    .map_err(|e| e.to_string())?
            }
        };

        let reader = index
            .reader_builder()
            // The indexer publishes BM25 and derived codemap data under one gate.
            // Automatic reload could expose a new commit before its codemap is ready.
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| e.to_string())?;

        // Stamp the current format version only after the reader is built (a failed reader
        // build must not leave a stale-but-current version that would skip the next rebuild).
        // Written once: after this run the sidecar matches, so `version_mismatch` is false
        // on every subsequent run and neither the rebuild nor this write fires again.
        if version_mismatch {
            std::fs::write(&version_path, EXTRACTION_FORMAT_VERSION).map_err(|error| {
                format!(
                    "failed to write index extraction format stamp {}: {error}",
                    version_path.display()
                )
            })?;
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
            definition_body_field,
            reference_field,
            format_text_field,
            extracted_json_field,
            mtime_field,
            content_hash_field,
            payload_hash_field,
            indexed_mtimes_cache: None,
            macro_inputs_cache: None,
            is_waiting_for_writer: false,
            published_entries: None,
            next_published_entries: None,
            is_publishing: false,
            loaded_entries: HashMap::new(),
            indexed_roots: std::fs::read(path.join(INDEXED_ROOTS_FILE))
                .ok()
                .and_then(|data| serde_json::from_slice(&data).ok())
                .unwrap_or_else(|| vec![".".to_string()]),
            event_config_stamp: std::fs::read_to_string(
                Path::new(path).join("event-navigation-config"),
            )
            .ok(),
            macro_config_stamp: std::fs::read_to_string(
                Path::new(index_path).join("macro-expansion-config"),
            )
            .ok(),
        })
    }

    /// A fresh handle for one write pass. Tantivy reads `.managed.json` once per opened index
    /// and rewrites the whole file from that copy. Processes sharing this directory take turns
    /// as the writer, so writing through the handle opened at startup dropped the files
    /// other processes had registered since, and garbage collection never removed them.
    fn open_for_write(&self) -> Result<Index, String> {
        let directory = tantivy::directory::MmapDirectory::open(&self.index_path)
            .map_err(|error| error.to_string())?;
        Index::open(directory).map_err(|error| error.to_string())
    }

    /// The committed path→stamp map and the recorded macro-expansion inputs of native files,
    /// read from the fast fields; only native files' stored documents are read.
    #[allow(clippy::type_complexity)]
    fn read_indexed_state(
        &self,
    ) -> Result<
        (
            HashMap<String, IndexedStamp>,
            HashMap<String, RecordedExpansion>,
        ),
        String,
    > {
        self.read_stored_state(false)
            .map(|(stamps, macro_inputs, _)| (stamps, macro_inputs))
    }

    /// [`Self::read_indexed_state`], and with `should_load_entries` every document decoded by
    /// the same pass for the first publication.
    #[allow(clippy::type_complexity)]
    fn read_stored_state(
        &self,
        should_load_entries: bool,
    ) -> Result<
        (
            HashMap<String, IndexedStamp>,
            HashMap<String, RecordedExpansion>,
            EntriesBySegment<DecodedEntry>,
        ),
        String,
    > {
        // A competing writer may have committed since this process last published.
        // Read its mtimes without advancing the request reader ahead of the codemap.
        let reader: IndexReader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|error| format!("indexed mtime reader open failed: {error}"))?;
        let searcher = reader.searcher();
        let mut map = HashMap::new();
        let mut macro_inputs = HashMap::new();
        let mut loaded = HashMap::new();

        // Every live document with no limit, so the mtime map is never silently truncated on
        // large repos (which would corrupt delete detection).
        for segment in searcher.segment_readers() {
            let columns = DocumentColumns::open(segment)?;
            let mut store = None;
            let mut segment_entries = Vec::new();
            if should_load_entries {
                segment_entries.resize_with(segment.max_doc() as usize, || None);
            }
            for doc_id in 0..segment.max_doc() {
                if segment.is_deleted(doc_id) {
                    continue;
                }
                let path = columns.path(doc_id).filter(|path| !path.is_empty());
                let is_native = path
                    .as_deref()
                    .is_some_and(|path| super::preprocess::is_eligible(Path::new(path)));
                let doc =
                    if should_load_entries || is_native {
                        let store = match &mut store {
                            Some(store) => store,
                            None => store.insert(segment.get_store_reader(1).map_err(|error| {
                                format!("indexed mtime document read failed: {error}")
                            })?),
                        };
                        Some(store.get::<TantivyDocument>(doc_id).map_err(|error| {
                            format!("indexed mtime document read failed: {error}")
                        })?)
                    } else {
                        None
                    };
                let json = doc
                    .as_ref()
                    .and_then(|doc| doc.get_first(self.extracted_json_field))
                    .and_then(|value| value.as_str());
                let stored = json
                    .filter(|_| should_load_entries)
                    .and_then(|json| serde_json::from_str::<StoredExtractedFile>(json).ok());
                let mtime = columns.mtime(doc_id);
                if let (Some(path), Some(mtime)) = (path, mtime) {
                    if is_native {
                        let inputs =
                            match &stored {
                                Some(stored) => stored.extracted.macro_expansion().map(|info| {
                                    RecordedExpansion {
                                        inputs: info.inputs.clone(),
                                        is_expanded: info.is_expanded(),
                                    }
                                }),
                                None => json
                                    .and_then(|json| {
                                        serde_json::from_str::<StoredMacroInputs>(json).ok()
                                    })
                                    .and_then(|stored| stored.navigation?.macro_expansion)
                                    .map(|stored| RecordedExpansion {
                                        inputs: stored.inputs,
                                        is_expanded: stored.active_lines.is_some(),
                                    }),
                            };
                        if let Some(inputs) = inputs {
                            macro_inputs.insert(path.clone(), inputs);
                        }
                    }
                    let content_hash = columns.content_hash(doc_id);
                    map.insert(
                        path,
                        IndexedStamp {
                            mtime,
                            content_hash,
                        },
                    );
                }
                if let Some(stored) = stored {
                    segment_entries[doc_id as usize] = Some(DecodedEntry::decoded(
                        columns.payload_hash(doc_id),
                        mtime,
                        stored,
                    ));
                }
            }
            if should_load_entries {
                loaded.insert(segment.segment_id(), segment_entries);
            }
        }
        Ok((map, macro_inputs, loaded))
    }

    /// Record the new modification times of files whose content still matched the index, so
    /// later passes compare against them without hashing the files again.
    fn record_touched(&mut self, touched: Vec<(String, u64)>) {
        if let Some(cache) = self.indexed_mtimes_cache.as_mut() {
            for (path, mtime) in touched {
                if let Some(stamp) = cache.get_mut(&path) {
                    stamp.mtime = mtime;
                }
            }
        }
    }

    fn ensure_indexed_state(&mut self) -> Result<(), String> {
        if self.indexed_mtimes_cache.is_none() {
            // Before the first publication, decode the documents while reading them anyway.
            let (mtimes, macro_inputs, loaded) =
                if self.is_publishing && self.published_entries.is_none() {
                    self.read_stored_state(true)?
                } else {
                    let (mtimes, macro_inputs) = self.read_indexed_state()?;
                    (mtimes, macro_inputs, HashMap::new())
                };
            self.indexed_mtimes_cache = Some(mtimes);
            self.macro_inputs_cache = Some(macro_inputs);
            self.loaded_entries = loaded;
            // Import the competing writer's extraction settings along with its mtimes,
            // so a lock retry does not unnecessarily reparse the entire repository.
            self.event_config_stamp = std::fs::read_to_string(
                Path::new(&self.index_path).join("event-navigation-config"),
            )
            .ok();
            self.macro_config_stamp =
                std::fs::read_to_string(Path::new(&self.index_path).join("macro-expansion-config"))
                    .ok();
        }
        Ok(())
    }

    /// Whether a changed path can affect macro expansion: a native source or header, a
    /// compilation database or flags file, or an input that an expansion recorded.
    fn has_native_input(&self, paths: &[PathBuf], index_root: &Path, abs_cwd: &Path) -> bool {
        let mut recorded: Option<HashSet<&str>> = None;
        paths.iter().any(|path| {
            if is_under_index_root(path, index_root, abs_cwd) {
                return false;
            }
            if super::preprocess::is_native_input(path) {
                return true;
            }
            let recorded = recorded.get_or_insert_with(|| {
                self.macro_inputs_cache
                    .iter()
                    .flat_map(HashMap::values)
                    .flat_map(|recorded| &recorded.inputs)
                    .map(|input| input.path.as_str())
                    .collect()
            });
            let absolute = crate::workspace::canonicalize_path_lenient(&abs_cwd.join(path));
            recorded.contains(absolute.display().to_string().as_str())
        })
    }

    /// Probe only the writer lock while a peer is indexing. Do not walk the working tree
    /// or reopen every stored document on each retry tick.
    pub(super) fn is_writer_available(&self) -> bool {
        match self.index.directory().acquire_lock(&INDEX_WRITER_LOCK) {
            Ok(_guard) => true,
            Err(tantivy::directory::error::LockError::LockBusy) => false,
            // Let the regular refresh report actionable I/O errors.
            Err(_) => true,
        }
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

/// Files a refresh reads and extracts together before adding them to the writer in order.
const EXTRACTION_BATCH_FILES: usize = 512;

/// A file's text and its extraction, or the error that kept it from being read.
type FileExtraction = std::io::Result<(
    String,
    Result<(ExtractedFile, crate::parser::IndexAuxiliary), String>,
)>;

/// `files` with their extractions, in the order given. Each batch is read and extracted on up
/// to `threads` threads, so the order of documents in the writer does not depend on timing.
fn extract_in_order<'a>(
    extractor: &'a crate::parser::TreeSitterExtractor,
    files: Vec<(String, PathBuf, u64)>,
    threads: usize,
) -> impl Iterator<Item = ((String, PathBuf, u64), FileExtraction)> + 'a {
    let mut files = files.into_iter();
    let mut extracted = std::collections::VecDeque::new();
    std::iter::from_fn(move || {
        if extracted.is_empty() {
            let batch: Vec<_> = files.by_ref().take(EXTRACTION_BATCH_FILES).collect();
            let extractions = extract_batch(extractor, &batch, threads);
            extracted.extend(batch.into_iter().zip(extractions));
        }
        extracted.pop_front()
    })
}

fn extract_batch(
    extractor: &crate::parser::TreeSitterExtractor,
    files: &[(String, PathBuf, u64)],
    threads: usize,
) -> Vec<FileExtraction> {
    crate::parallel::map_in_order(
        files,
        threads,
        || (),
        |(), (rel_path, disk_path, _)| {
            std::fs::read_to_string(disk_path).map(|content| {
                let extraction = extractor.extract_for_index(&content, rel_path);
                (content, extraction)
            })
        },
    )
}

/// The directory indexed relative paths are resolved against: the canonical process cwd.
fn indexed_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.canonicalize().unwrap_or(cwd)
}

/// Gather the index entry for a single file: enforce the source-extension allowlist
/// and the [`crate::workspace::MAX_INDEXED_FILE_BYTES`] size cap (skip oversize/minified
/// blobs before they are read+parsed, Child 04), and capture a sub-second mtime so a
/// same-second edit still reindexes. Returns `None` when the file is not to be indexed.
fn collect_index_entry(entry_path: &Path, abs_cwd: &Path) -> Option<(String, PathBuf, u64)> {
    if crate::workspace::is_explicitly_excluded_file(entry_path) {
        return None;
    }
    if !crate::workspace::is_supported_source_path(entry_path) {
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
        self.index_files_changed_with_reload(paths, true)
    }

    /// Indexer-only variant that leaves the shared reader on its prior generation until the
    /// companion published snapshot has been reconstructed successfully.
    pub(crate) fn index_files_changed_deferred(&mut self, paths: &[&str]) -> Result<bool, String> {
        self.index_files_changed_with_reload(paths, false)
    }

    fn index_files_changed_with_reload(
        &mut self,
        paths: &[&str],
        should_reload_reader: bool,
    ) -> Result<bool, String> {
        let mut files_to_process = Vec::new();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        // JSON and related formats are source inputs now, so a caller that indexes a parent of
        // the Tantivy directory must not feed Tantivy's own metadata back into the index.
        let index_root = normalized_index_root(Path::new(&self.index_path), &abs_cwd);

        for path_str in paths {
            let path = Path::new(path_str);
            if path.is_file() {
                if !is_under_index_root(path, &index_root, &abs_cwd) {
                    if let Some(entry) = collect_index_entry(path, &abs_cwd) {
                        files_to_process.push(entry);
                    }
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
                    // The event can name an ancestor of a repository-local custom index.
                    // Apply the same entry-level guard as a full walk before considering
                    // descendants, otherwise `search-index/meta.json` becomes a JSON source.
                    if is_under_index_root(entry_path, &index_root, &abs_cwd) {
                        continue;
                    }
                    if !entry_path.is_file() {
                        continue;
                    }
                    if let Some(collected) = collect_index_entry(entry_path, &abs_cwd) {
                        files_to_process.push(collected);
                    }
                }
            }
        }

        // Lazily snapshot the on-disk mtime map once, then read from the maintained
        // cache — avoids an `AllQuery` over the whole index on every call, which in MCP
        // mode runs before every search (Child 04).
        self.ensure_indexed_state()?;
        let event_config_stamp = crate::events::config_stamp();
        let should_refresh_event_files =
            self.event_config_stamp.as_deref() != Some(&event_config_stamp);
        let macro_config = crate::config::get().macro_expansion.clone();
        let macro_config_stamp = format!("{macro_config:?}");
        let has_macro_config_changed =
            self.macro_config_stamp.as_deref() != Some(&macro_config_stamp);
        let indexed_mtimes = self.indexed_mtimes_cache.as_ref().unwrap();
        let macro_inputs = self.macro_inputs_cache.as_ref();
        // A failed expansion does not know every header it needs, so a header that is new
        // to the index (for example a generated configuration header) retries the failed
        // ones. Only a readable header counts, as an unreadable one stays new forever.
        let has_new_header = files_to_process.iter().any(|(rel_path, disk_path, _)| {
            !indexed_mtimes.contains_key(rel_path)
                && super::preprocess::is_header(disk_path)
                && std::fs::read_to_string(disk_path).is_ok()
        });
        let mut current_inputs: HashMap<String, MacroInputStamp> = HashMap::new();
        // A native file is reprocessed when the settings changed or, while expansion is
        // enabled, when it has no recorded inputs yet or one of them (itself, an included
        // header, build settings) changed. Unrelated refreshes leave it alone.
        let mut needs_macro_refresh = |rel_path: &str, disk_path: &Path| {
            super::preprocess::is_eligible(disk_path)
                && (has_macro_config_changed
                    || (macro_config.is_enabled
                        && macro_inputs
                            .and_then(|inputs| inputs.get(rel_path))
                            .is_none_or(|recorded| {
                                (has_new_header && !recorded.is_expanded)
                                    || recorded.inputs.iter().any(|input| {
                                        current_inputs.entry(input.path.clone()).or_insert_with(
                                            || {
                                                super::preprocess::input_stamp(Path::new(
                                                    &input.path,
                                                ))
                                            },
                                        ) != input
                                    })
                            })))
        };

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
        let mut touched = Vec::new();
        for (rel_path, disk_path, mtime) in files_to_process {
            match indexed_mtimes.get(&rel_path) {
                Some(indexed)
                    if !(should_refresh_event_files && crate::events::eligible(&rel_path))
                        && !needs_macro_refresh(&rel_path, &disk_path)
                        && (indexed.mtime == mtime || indexed.matches_content(&disk_path)) =>
                {
                    // Skip indexing: the content has not changed since extraction.
                    if indexed.mtime != mtime {
                        touched.push((rel_path, mtime));
                    }
                }
                _ => {
                    to_index.push((rel_path, disk_path, mtime));
                }
            }
        }

        tracing::debug!(
            "index_files: files_to_process={}, to_index={}, to_delete={}, touched={}",
            files_to_process_len,
            to_index.len(),
            to_delete.len(),
            touched.len()
        );
        self.record_touched(touched);

        let changed = self.apply_index_updates(to_index, to_delete, should_reload_reader)?;
        // A settings change must also discard previously expanded declarations when disabled.
        if self.macro_config_stamp.as_deref() != Some(&macro_config_stamp)
            && (changed
                || self
                    .indexed_mtimes_cache
                    .as_ref()
                    .is_some_and(HashMap::is_empty))
        {
            std::fs::write(
                Path::new(&self.index_path).join("macro-expansion-config"),
                &macro_config_stamp,
            )
            .map_err(|e| e.to_string())?;
            self.macro_config_stamp = Some(macro_config_stamp);
        }
        if should_refresh_event_files {
            std::fs::write(
                Path::new(&self.index_path).join("event-navigation-config"),
                &event_config_stamp,
            )
            .map_err(|e| e.to_string())?;
            self.event_config_stamp = Some(event_config_stamp);
        }
        let indexed_roots: Vec<String> = paths
            .iter()
            .map(|path| abs_cwd.join(path).to_string_lossy().into_owned())
            .collect();
        if self.indexed_roots != indexed_roots {
            std::fs::write(
                Path::new(&self.index_path).join(INDEXED_ROOTS_FILE),
                serde_json::to_vec(&indexed_roots).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            self.indexed_roots = indexed_roots;
        }
        // Publish the new settings generation even when the workspace has no eligible
        // event files. Otherwise an opt-in toggle would remain permanently "not ready".
        Ok(changed || should_refresh_event_files)
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
        should_reload_reader: bool,
    ) -> Result<bool, String> {
        self.is_waiting_for_writer = false;
        // Return early if no updates (adds or deletes) to avoid touching index and triggering modification
        if to_index.is_empty() && to_delete.is_empty() {
            // A peer's commit may already satisfy the refreshed mtime cache. Direct
            // callers still need that generation; the indexer publishes it separately.
            if should_reload_reader {
                self.reader.reload().map_err(|error| error.to_string())?;
            }
            return Ok(false);
        }

        let extractor = crate::parser::TreeSitterExtractor::new();
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut macro_expander =
            super::MacroExpander::new(&root, crate::config::get().macro_expansion.clone());
        let managed_path = Path::new(&self.index_path).join(".managed.json");
        let mut attempts = 0;
        let (write_index, mut writer) = loop {
            attempts += 1;
            let managed_before = std::fs::read(&managed_path).ok();
            let write_index = self.open_for_write()?;
            let writer = match write_index.writer(50_000_000) {
                Ok(w) => w,
                Err(tantivy::TantivyError::LockFailure(e, _)) => {
                    self.indexed_mtimes_cache = None;
                    self.macro_inputs_cache = None;
                    self.is_waiting_for_writer =
                        matches!(e, tantivy::directory::error::LockError::LockBusy);
                    return Err(format!(
                        "Index writer lock unavailable; refresh deferred for retry: {e:?}"
                    ));
                }
                Err(e) => return Err(e.to_string()),
            };
            // Another writer may have registered files after the list was read and before the
            // lock was taken. With the lock held the list stays put, so one more open settles it.
            if attempts == 3 || std::fs::read(&managed_path).ok() == managed_before {
                writer.set_merge_policy(merge_policy());
                break (write_index, writer);
            }
        };

        for rel_path in &to_delete {
            let term = Term::from_field_text(self.file_path_field, rel_path);
            writer.delete_term(term);
        }

        // Track only the docs that actually get added so the cache mirrors the index:
        // a file that fails to read/parse stays out of the cache and is retried next call.
        let mut committed_updates: Vec<(String, IndexedStamp)> = Vec::new();
        let mut committed_macro_inputs: Vec<(String, Option<RecordedExpansion>)> = Vec::new();
        let mut encoding_excluded_paths = Vec::new();
        let threads = crate::parallel::threads();
        for ((rel_path, disk_path, mtime), read) in extract_in_order(&extractor, to_index, threads)
        {
            let (content, extraction) = match read {
                Ok(read) => read,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::InvalidData {
                        // A formerly valid file must disappear from both search and
                        // codemap when it no longer satisfies the UTF-8 input contract.
                        writer.delete_term(Term::from_field_text(self.file_path_field, &rel_path));
                        encoding_excluded_paths.push(rel_path);
                        eprintln!("Warning: Not indexing {}: invalid UTF-8; indexing accepts UTF-8 source only", disk_path.display());
                        continue;
                    }
                    eprintln!(
                        "Warning: Failed to read file {}: {}",
                        disk_path.display(),
                        e
                    );
                    continue;
                }
            };
            let (mut extracted, mut auxiliary) = match extraction {
                Ok(extracted_parts) => extracted_parts,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse file {}: {}",
                        disk_path.display(),
                        e
                    );
                    continue;
                }
            };

            if let Some(expanded_auxiliary) =
                macro_expander.expand_for_index(&disk_path, &content, &mut extracted)
            {
                auxiliary = expanded_auxiliary;
            }
            if extracted.is_macro_expanded() {
                auxiliary.static_collection_edges.clear();
                auxiliary.event_input = None;
                auxiliary.flow_file = None;
            }

            let term = Term::from_field_text(self.file_path_field, &rel_path);
            writer.delete_term(term);

            let mut doc = TantivyDocument::default();
            doc.add_text(self.file_path_field, &rel_path);

            let path_parts = tokenize_path(&rel_path);
            doc.add_text(self.file_path_parts_field, &path_parts);

            let stored_extracted = StoredExtractedFileRef {
                extracted: &extracted,
                static_collection_edges: &auxiliary.static_collection_edges,
                event_input: auxiliary.event_input.as_ref().map(StoredEventSource::new),
                flow_digest: auxiliary
                    .flow_file
                    .as_ref()
                    .map(|flow| flow.digest.as_str()),
            };
            let json_str = match serde_json::to_string(&stored_extracted) {
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
            let content_hash = short_hash(content.as_bytes());
            doc.add_u64(self.content_hash_field, content_hash);
            doc.add_u64(self.payload_hash_field, short_hash(json_str.as_bytes()));

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
                    let capped: String = literal
                        .text
                        .chars()
                        .take(INDEXED_LITERAL_MAX_CHARS)
                        .collect();
                    doc.add_text(self.literal_field, &capped);
                } else {
                    doc.add_text(self.literal_field, &literal.text);
                }
            }
            for definition_body in &auxiliary.definition_body {
                doc.add_text(self.definition_body_field, definition_body);
            }
            for reference in &auxiliary.reference {
                doc.add_text(self.reference_field, reference);
            }
            for format_text in &auxiliary.format_text {
                doc.add_text(self.format_text_field, format_text);
            }

            let macro_inputs = extracted.macro_expansion().map(|info| RecordedExpansion {
                inputs: info.inputs.clone(),
                is_expanded: info.is_expanded(),
            });
            match writer.add_document(doc) {
                Ok(_) => {
                    committed_macro_inputs.push((rel_path.clone(), macro_inputs));
                    let content_hash = Some(content_hash);
                    committed_updates.push((
                        rel_path,
                        IndexedStamp {
                            mtime,
                            content_hash,
                        },
                    ));
                }
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
        // Still the writer, so no other process is creating segment files.
        super::segment_files::remove_orphans(&write_index, Path::new(&self.index_path));
        // Dropping a writer cancels the merges its commit started, which would leave their
        // output unreferenced and the deleted documents in place. The commit already landed,
        // so a failed merge only postpones the cleanup to a later pass.
        if let Err(error) = writer.wait_merging_threads() {
            tracing::warn!("index segment merge did not finish: {error}");
        }
        if should_reload_reader {
            self.reader.reload().map_err(|error| error.to_string())?;
        }
        // Reconcile the cache only after the commit landed (Child 04): drop the deleted
        // paths and record the freshly-indexed mtimes. Failed commits/reloads never add
        // uncommitted files; writer contention discards the cache so a retry can import
        // the other process's committed state.
        if let Some(cache) = self.indexed_mtimes_cache.as_mut() {
            for path in to_delete.iter().chain(&encoding_excluded_paths) {
                cache.remove(path);
            }
            for (rel_path, stamp) in committed_updates {
                cache.insert(rel_path, stamp);
            }
        }
        if let Some(cache) = self.macro_inputs_cache.as_mut() {
            for path in to_delete.iter().chain(&encoding_excluded_paths) {
                cache.remove(path);
            }
            for (rel_path, recorded) in committed_macro_inputs {
                match recorded {
                    Some(recorded) => cache.insert(rel_path, recorded),
                    None => cache.remove(&rel_path),
                };
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
        self.refresh_paths_with_reload(paths, true)
    }

    /// Indexer-only watcher refresh that defers reader visibility until snapshot publication.
    pub(crate) fn refresh_paths_deferred(&mut self, paths: &[PathBuf]) -> Result<bool, String> {
        self.refresh_paths_with_reload(paths, false)
    }

    fn refresh_indexed_roots(&mut self, should_reload_reader: bool) -> Result<bool, String> {
        let roots = self.indexed_roots.clone();
        let paths: Vec<&str> = roots.iter().map(String::as_str).collect();
        self.index_files_changed_with_reload(&paths, should_reload_reader)
    }

    fn refresh_paths_with_reload(
        &mut self,
        paths: &[PathBuf],
        should_reload_reader: bool,
    ) -> Result<bool, String> {
        self.ensure_indexed_state()?;
        if self.event_config_stamp.as_deref() != Some(&crate::events::config_stamp()) {
            return self.refresh_indexed_roots(should_reload_reader);
        }
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let abs_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        let index_root = normalized_index_root(Path::new(&self.index_path), &abs_cwd);

        // Headers, included macro files, and compilation flags can affect any translation
        // unit, so a change to one reconciles the native files fully. Other changes stay
        // path-scoped even while native preprocessing is enabled.
        if crate::config::get().macro_expansion.is_enabled
            && self.has_native_input(paths, &index_root, &abs_cwd)
        {
            return self.refresh_indexed_roots(should_reload_reader);
        }

        let indexed_mtimes = self.indexed_mtimes_cache.as_ref().unwrap();

        let mut to_index: Vec<(String, PathBuf, u64)> = Vec::new();
        let mut to_delete: Vec<String> = Vec::new();
        let mut touched = Vec::new();

        for path in paths {
            // A repository-local custom index can contain supported files such as Tantivy's
            // `meta.json`. Ignore both existing and removed paths under it before deriving
            // delete keys, otherwise watcher events feed the index back into itself.
            if is_under_index_root(path, &index_root, &abs_cwd) {
                continue;
            }
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
                    Some((rel_path, disk_path, mtime)) => match indexed_mtimes.get(&rel_path) {
                        Some(indexed)
                            if indexed.mtime == mtime || indexed.matches_content(&disk_path) =>
                        {
                            if indexed.mtime != mtime {
                                touched.push((rel_path, mtime));
                            }
                        }
                        _ => to_index.push((rel_path, disk_path, mtime)),
                    },
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
                    // A watcher can receive an ancestor-directory event. Exclude every
                    // descendant of a repository-local custom index before collecting or
                    // adding it to the subtree delete-diff set.
                    if is_under_index_root(entry_path, &index_root, &abs_cwd) {
                        continue;
                    }
                    if entry_path.is_file() {
                        if let Some((rel_path, disk_path, mtime)) =
                            collect_index_entry(entry_path, &abs_cwd)
                        {
                            subtree_disk_paths.insert(rel_path.clone());
                            match indexed_mtimes.get(&rel_path) {
                                Some(indexed)
                                    if indexed.mtime == mtime
                                        || indexed.matches_content(&disk_path) =>
                                {
                                    if indexed.mtime != mtime {
                                        touched.push((rel_path, mtime));
                                    }
                                }
                                _ => to_index.push((rel_path, disk_path, mtime)),
                            }
                        }
                    }
                }
                let directory_key = crate::workspace::stored_index_key(path, &abs_cwd);
                // The workspace root has an empty stored key. `format!("{directory_key}/")`
                // would be `/`, which matches no ordinary relative document and leaves a
                // deleted exact-name format stale when FSEvents reports only its parent.
                let prefix = (!directory_key.is_empty()).then(|| format!("{directory_key}/"));
                for indexed_path in indexed_mtimes.keys() {
                    if prefix
                        .as_ref()
                        .is_none_or(|prefix| indexed_path.starts_with(prefix))
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
            "refresh_paths: event_paths={}, to_index={}, to_delete={}, touched={}",
            paths.len(),
            to_index.len(),
            to_delete.len(),
            touched.len()
        );
        self.record_touched(touched);

        self.apply_index_updates(to_index, to_delete, should_reload_reader)
    }

    /// Make the newest committed Tantivy generation visible only after its companion
    /// `PublishedIndexSnapshot` has been rebuilt successfully.
    pub(crate) fn reload_reader(&self) -> Result<(), String> {
        self.reader.reload().map_err(|error| error.to_string())
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
            definition_body_field: self.definition_body_field,
            reference_field: self.reference_field,
            format_text_field: self.format_text_field,
            extracted_json_field: self.extracted_json_field,
        }
    }

    pub fn search_with_context(
        &self,
        query: &str,
        limit: usize,
        context: &SearchQueryContext,
    ) -> Result<Vec<SearchResult>, String> {
        self.searcher_handle()
            .search_with_context(query, limit, context)
    }

    /// Mark this engine as the one that publishes generations, so its first state read decodes
    /// the documents for the first publication and later publications reuse entries.
    pub(crate) fn start_publishing(&mut self) {
        self.is_publishing = true;
    }

    /// Record that the generation from the last [`Self::load_next_published_snapshot`] is
    /// published as `generation`, so the next publication reuses its entries.
    pub(crate) fn finish_publication(&mut self, generation: u64) {
        if let Some(mut entries) = self.next_published_entries.take() {
            entries.generation = generation;
            self.published_entries = Some(entries);
        }
    }

    /// Rebuild all in-memory derived data from stored docs in one pass. This is called only by
    /// the background indexer when publishing a successful generation; search never scans
    /// Tantivy documents or deserializes stored JSON on its hot path.
    pub fn load_published_snapshot(
        &self,
    ) -> Result<super::indexer::PublishedIndexSnapshot, String> {
        self.build_published_snapshot(None, HashMap::new())
            .map(|(snapshot, _)| snapshot)
    }

    /// [`Self::load_published_snapshot`] after `previous`, the generation this engine published
    /// last. Documents unchanged since `previous`, or decoded by its first state read, are not
    /// decoded again, and the new generation shares unchanged files with `previous`.
    pub(crate) fn load_next_published_snapshot(
        &mut self,
        previous: &super::indexer::PublishedIndexSnapshot,
    ) -> Result<super::indexer::PublishedIndexSnapshot, String> {
        let loaded = std::mem::take(&mut self.loaded_entries);
        let published = self
            .published_entries
            .as_ref()
            .filter(|entries| entries.generation == previous.generation())
            .map(|entries| (previous, entries));
        let (snapshot, entries) = self.build_published_snapshot(published, loaded)?;
        self.next_published_entries = Some(entries);
        Ok(snapshot)
    }

    fn build_published_snapshot(
        &self,
        previous: Option<(&super::indexer::PublishedIndexSnapshot, &PublishedEntries)>,
        mut loaded: EntriesBySegment<DecodedEntry>,
    ) -> Result<(super::indexer::PublishedIndexSnapshot, PublishedEntries), String> {
        // This temporary reader sees the just-committed generation without advancing the
        // shared request reader. If reconstruction fails, callers keep the prior BM25 and
        // relation generation together.
        let snapshot_reader: IndexReader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|error| format!("published snapshot reader open failed: {error}"))?;
        let searcher = snapshot_reader.searcher();
        let mut entries = Vec::new();
        let mut by_segment = HashMap::new();
        // Every live document, segment by segment, so the codemap snapshot stays complete on
        // large repos. Only documents without a decoded entry are read from the store.
        for segment in searcher.segment_readers() {
            let segment_id = segment.segment_id();
            let reusable =
                previous.and_then(|(_, published)| published.by_segment.get(&segment_id));
            let mut loaded = loaded.remove(&segment_id);
            let mut located = vec![None; segment.max_doc() as usize];
            let mut documents = Vec::new();
            let mut unread = Vec::new();
            for doc_id in 0..segment.max_doc() {
                if segment.is_deleted(doc_id) {
                    continue;
                }
                let reused = reusable
                    .and_then(|entries| entries.get(doc_id as usize)?.clone())
                    .zip(previous);
                let loaded = loaded
                    .as_mut()
                    .and_then(|entries| entries.get_mut(doc_id as usize)?.take());
                let document = if let Some((entry, (previous, _))) = reused {
                    let edges = previous.static_collection_edges(&entry.file.file_path);
                    Some((entry, edges, true))
                } else if let Some(loaded) = loaded {
                    let (entry, edges) = loaded.publish();
                    Some((entry, edges, false))
                } else {
                    unread.push(doc_id);
                    None
                };
                documents.push((doc_id, document));
            }
            let mut read = read_published_entries(
                self.extracted_json_field,
                segment,
                &unread,
                previous,
                crate::parallel::threads(),
            )?
            .into_iter();
            for (doc_id, document) in documents {
                let Some((entry, edges, is_unchanged)) = document.or_else(|| read.next()) else {
                    continue;
                };
                located[doc_id as usize] = Some(std::sync::Arc::clone(&entry));
                entries.push((entry, edges, is_unchanged));
            }
            by_segment.insert(segment_id, located);
        }
        drop(loaded);
        // Event sources fill their budget in path order. An unchanged document's source comes
        // from `previous` when it was within budget there; other sources are read from disk
        // and used only while they still hold the indexed text.
        entries.sort_by(|(left, ..), (right, ..)| left.file.file_path.cmp(&right.file.file_path));
        let previous_sources = previous.map(|(previous, _)| previous.events().indexed_sources());
        let root = indexed_root();
        let mut event_inputs = crate::events::EventInputs::default();
        for (entry, _, is_unchanged) in &entries {
            let path = &entry.file.file_path;
            let input = entry.event_input.as_ref();
            let sized = input.map(|input| (input.source_length, input.unavailable_reason.clone()));
            event_inputs.insert_sized(path.clone(), sized, || {
                is_unchanged
                    .then(|| previous_sources?.get(path).cloned())
                    .flatten()
                    .or_else(|| input?.read(&root, path))
            });
        }
        let mut source_mtimes = HashMap::new();
        let mut flow_documents = std::collections::BTreeMap::new();
        let mut files_and_edges = Vec::with_capacity(entries.len());
        let mut by_path = HashMap::with_capacity(entries.len());
        for (entry, edges, _) in entries {
            let path = &entry.file.file_path;
            if let Some(mtime) = entry.mtime {
                source_mtimes.insert(path.clone(), mtime);
            }
            if let Some(digest) = &entry.flow_digest {
                flow_documents.insert(path.clone(), digest.clone());
            }
            files_and_edges.push((std::sync::Arc::clone(&entry.file), edges));
            by_path.insert(path.clone(), entry);
        }
        let snapshot = super::indexer::PublishedIndexSnapshot::from_files_and_edges_with_flow(
            files_and_edges,
            event_inputs,
            Some(crate::flow::IndexedFlowStore {
                documents: flow_documents,
            }),
            Some(Path::new(&self.index_path)),
        )
        .with_source_mtimes(source_mtimes);
        let entries = PublishedEntries {
            generation: 0,
            by_path,
            by_segment,
        };
        Ok((snapshot, entries))
    }
}

/// A document read for publication: its entry, its static collection edges and whether it is
/// unchanged since the previous generation.
type ReadEntry = (
    std::sync::Arc<PublishedEntry>,
    Vec<StaticCollectionEdge>,
    bool,
);

/// [`read_published_entry`] for each of `docs`, in order, split into contiguous runs read on up
/// to `threads` threads.
fn read_published_entries(
    extracted_json_field: Field,
    segment: &tantivy::SegmentReader,
    docs: &[tantivy::DocId],
    previous: Option<(&super::indexer::PublishedIndexSnapshot, &PublishedEntries)>,
    threads: usize,
) -> Result<Vec<ReadEntry>, String> {
    let runs: Vec<_> = docs
        .chunks(docs.len().div_ceil(threads.max(1)).max(1))
        .collect();
    let read = crate::parallel::map_in_order(
        &runs,
        threads,
        || (),
        |(), docs| {
            let columns = DocumentColumns::open(segment)?;
            let mut store = None;
            docs.iter()
                .map(|&doc_id| {
                    read_published_entry(
                        extracted_json_field,
                        segment,
                        &columns,
                        &mut store,
                        doc_id,
                        previous,
                    )
                })
                .collect::<Result<Vec<_>, String>>()
        },
    );
    let mut entries = Vec::with_capacity(docs.len());
    for run in read {
        entries.extend(run?);
    }
    Ok(entries)
}

/// Read one document for publication: a document unchanged since `previous` at another
/// location (after a merge) is matched by its fast-field payload hash and mtime; others are
/// read from the store and decoded.
fn read_published_entry(
    extracted_json_field: Field,
    segment: &tantivy::SegmentReader,
    columns: &DocumentColumns,
    store: &mut Option<tantivy::store::StoreReader>,
    doc_id: tantivy::DocId,
    previous: Option<(&super::indexer::PublishedIndexSnapshot, &PublishedEntries)>,
) -> Result<ReadEntry, String> {
    let payload = columns.payload_hash(doc_id);
    let mtime = columns.mtime(doc_id);
    if let Some((path, (previous, published))) = columns.path(doc_id).zip(previous) {
        if let Some(entry) = published
            .by_path
            .get(&path)
            .filter(|entry| entry.is_current(payload, mtime))
        {
            return Ok((
                std::sync::Arc::clone(entry),
                previous.static_collection_edges(&path),
                true,
            ));
        }
    }
    let store = match store {
        Some(store) => store,
        None => store.insert(
            segment
                .get_store_reader(1)
                .map_err(|error| format!("published snapshot document read failed: {error}"))?,
        ),
    };
    let doc = store
        .get::<TantivyDocument>(doc_id)
        .map_err(|error| format!("published snapshot document read failed: {error}"))?;
    let json = doc
        .get_first(extracted_json_field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| "published snapshot document is missing extracted_json".to_string())?;
    let stored = serde_json::from_str::<StoredExtractedFile>(json)
        .map_err(|error| format!("published snapshot JSON decode failed: {error}"))?;
    let (entry, edges) = DecodedEntry::decoded(payload, mtime, stored).publish();
    Ok((entry, edges, false))
}

fn normalized_index_root(index_path: &Path, abs_cwd: &Path) -> PathBuf {
    let absolute = if index_path.is_absolute() {
        index_path.to_path_buf()
    } else {
        abs_cwd.join(index_path)
    };
    crate::workspace::canonicalize_path_lenient(&absolute)
}

fn is_under_index_root(path: &Path, index_root: &Path, abs_cwd: &Path) -> bool {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        abs_cwd.join(path)
    };
    crate::workspace::canonicalize_path_lenient(&absolute).starts_with(index_root)
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
    fn test_non_utf8_update_removes_stale_symbols_and_recovers() {
        let directory = tempdir().unwrap();
        let file = directory.path().join("source.rs");
        fs::write(&file, "pub fn encoding_probe() {}\n").unwrap();
        let mut engine =
            TantivySearchEngine::new(directory.path().join("index").to_str().unwrap()).unwrap();
        engine.index_files(&[file.to_str().unwrap()]).unwrap();
        assert_eq!(engine.search("encoding_probe", 10).unwrap().len(), 1);
        fs::write(&file, b"// invalid \xff\npub fn encoding_probe() {}\n").unwrap();
        filetime::set_file_mtime(&file, filetime::FileTime::from_unix_time(1_000_000_001, 0))
            .unwrap();
        engine.index_files(&[file.to_str().unwrap()]).unwrap();
        assert!(engine.search("encoding_probe", 10).unwrap().is_empty());
        assert!(engine
            .load_published_snapshot()
            .unwrap()
            .codemap()
            .is_empty());
        fs::write(&file, "pub fn encoding_probe() {}\n").unwrap();
        filetime::set_file_mtime(&file, filetime::FileTime::from_unix_time(1_000_000_002, 0))
            .unwrap();
        engine.index_files(&[file.to_str().unwrap()]).unwrap();
        assert_eq!(engine.search("encoding_probe", 10).unwrap().len(), 1);
        assert_eq!(engine.load_published_snapshot().unwrap().codemap().len(), 1);
    }

    #[test]
    fn test_long_else_if_chain_keeps_the_index_publishable() {
        let directory = tempdir().unwrap();
        let file = directory.path().join("chain.js");
        let branches: String = (1..40)
            .map(|value| format!(" else if (x === {value}) {{ return {value}; }}"))
            .collect();
        fs::write(
            &file,
            format!("function chain_probe(x) {{\n  if (x === 0) {{ return 0; }}{branches}\n  return -1;\n}}\n"),
        )
        .unwrap();
        let mut engine =
            TantivySearchEngine::new(directory.path().join("index").to_str().unwrap()).unwrap();
        engine.index_files(&[file.to_str().unwrap()]).unwrap();
        assert_eq!(engine.search("chain_probe", 10).unwrap().len(), 1);
        assert_eq!(engine.load_published_snapshot().unwrap().codemap().len(), 1);
    }

    #[test]
    fn test_deferred_deletion_publishes_matching_search_and_codemap() {
        let directory = tempdir().unwrap();
        let file = directory.path().join("source.rs");
        fs::write(&file, "pub fn deletion_probe() {}\n").unwrap();
        let mut engine =
            TantivySearchEngine::new(directory.path().join("index").to_str().unwrap()).unwrap();
        engine.index_files(&[file.to_str().unwrap()]).unwrap();
        assert_eq!(engine.load_published_snapshot().unwrap().codemap().len(), 1);
        assert!(engine.index_files_changed_deferred(&[]).unwrap());
        // The request reader remains on the old generation until explicit publication.
        assert_eq!(engine.search("deletion_probe", 10).unwrap().len(), 1);
        assert!(engine
            .load_published_snapshot()
            .unwrap()
            .codemap()
            .is_empty());
        engine.reload_reader().unwrap();
        assert!(engine.search("deletion_probe", 10).unwrap().is_empty());
    }

    #[test]
    fn test_repo_local_custom_index_root_is_normalized_for_relative_walker_and_watcher_paths() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let index = workspace.join("search-index");
        fs::create_dir_all(&index).unwrap();
        let root = normalized_index_root(Path::new("search-index"), &workspace);
        assert!(is_under_index_root(
            Path::new("search-index/meta.json"),
            &root,
            &workspace
        ));
        assert!(is_under_index_root(
            &index.join("meta.json"),
            &root,
            &workspace
        ));
        assert!(!is_under_index_root(
            Path::new("src/config.json"),
            &root,
            &workspace
        ));
    }

    #[test]
    fn test_lsr_006_ancestor_directory_refresh_excludes_repo_local_index_subtree() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let index = workspace.join("search-index");
        fs::create_dir_all(&index).unwrap();
        fs::write(workspace.join("source.json"), r#"{"source":"normal"}"#).unwrap();

        let mut engine = TantivySearchEngine::new(&index.to_string_lossy()).unwrap();
        engine.index_files(&[&workspace.to_string_lossy()]).unwrap();
        fs::write(
            index.join("watcher-generated.json"),
            r#"{"needle":"self_index_needle"}"#,
        )
        .unwrap();

        // A watcher event for the custom-index ancestor walks the workspace. The generated
        // JSON must never be handed to `collect_index_entry` through that directory branch.
        engine.refresh_paths(&[workspace]).unwrap();
        assert!(engine.search("self_index_needle", 10).unwrap().is_empty());
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
                engine.read_indexed_state().map(|(mtimes, _)| mtimes)
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
    fn test_engine_reference_auxiliary_surfaces_call_site_query() {
        let temp = tempdir().unwrap();
        let index_dir = temp.path().join("index");
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let file_ts = src_dir.join("worker.ts");
        fs::write(
            &file_ts,
            "export function dispatchOrder() {\n  submitOrder();\n}\n",
        )
        .unwrap();

        let mut engine = TantivySearchEngine::new(&index_dir.to_string_lossy()).unwrap();
        engine
            .index_files(&[&temp.path().to_string_lossy()])
            .unwrap();

        let results = engine.search("submitOrder", 10).unwrap();
        assert!(
            results
                .first()
                .is_some_and(|result| result.file_path.contains("worker.ts")),
            "reference auxiliary should surface the call-site file: {:?}",
            results
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
        assert_eq!(EXTRACTION_FORMAT_VERSION, "v43-compact-store");
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
        fs::write(&version_path, "v24-groovy-recovery-guard").unwrap();

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
