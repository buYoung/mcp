//! TOML configuration surface for codemap-search (Child 05).
//!
//! Resolution precedence is per-key `repo > global > default` (the scout pattern): a
//! repo-local `<repo>/.codemap/config.toml` overrides a global `<global>/config.toml`
//! overrides the compiled-in defaults. The loader is **never-exit**: a missing file,
//! parse error, unknown key, or type mismatch warns to stderr and falls back to the
//! default for that key — it never panics or exits the process.
//!
//! The global directory is injected (`CODEMAP_HOME`, else `~/.codemap`) so the loader is
//! pure/unit-testable and tests stay hermetic (they never read the developer's real home).
//! The loader ([`load`]) is itself pure and side-effect-free — it only reads. Separately,
//! the `mcp` command calls [`ensure_repo_config`] once at startup. When no
//! `<repo>/.codemap/config.toml` exists it scaffolds a commented, behavior-preserving file
//! (stamped with the current schema [`CONFIG_VERSION`]) for discoverability. When one
//! already exists it **incrementally syncs** it: for every key introduced since the file's
//! stamped version it appends that key's commented block (presence-guarded so an existing
//! key — set or commented — is never duplicated) and re-stamps the version marker. The sync
//! is strictly additive — it never edits, reorders, or removes a user's existing lines,
//! never rewrites a file already at the current version, never touches any git file, and
//! warns rather than crashing on failure. Keeping `.codemap/` out of `git status` is the
//! user's `.gitignore` choice.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Permission policy for a live filesystem tool (`find`, `grep`, `read`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemPermissionPolicy {
    Workspace,
    AllowedRoots,
    Anywhere,
}

impl FilesystemPermissionPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::AllowedRoots => "allowed_roots",
            Self::Anywhere => "anywhere",
        }
    }
}

/// Resolved filesystem permission settings for live disk tools.
#[derive(Debug, Clone)]
pub struct FilesystemPermissions {
    pub find: FilesystemPermissionPolicy,
    pub grep: FilesystemPermissionPolicy,
    pub read: FilesystemPermissionPolicy,
    pub allowed_roots: Vec<PathBuf>,
}

impl Default for FilesystemPermissions {
    fn default() -> Self {
        Self {
            find: FilesystemPermissionPolicy::Workspace,
            grep: FilesystemPermissionPolicy::Workspace,
            read: FilesystemPermissionPolicy::Workspace,
            allowed_roots: Vec::new(),
        }
    }
}

/// Repo-local / global config directory name. Kept in [`crate::workspace::EXCLUDED_DIRS`] so
/// the tool never walks (indexes) it.
pub const CODEMAP_DIR_NAME: &str = ".codemap";
/// Config file name, shared by the repo and global layers.
const CONFIG_FILE_NAME: &str = "config.toml";
/// Env var overriding the global config home (default `~/.codemap`). Tests set it for
/// hermetic isolation; users may set it to relocate global config.
const HOME_ENV: &str = "CODEMAP_HOME";

/// Current config-template schema version. Stamped into every scaffolded file and used by
/// [`ensure_repo_config`] to decide whether an existing file needs an incremental sync. Bump
/// this whenever [`CONFIG_TEMPLATE`] grows a key, and add the matching [`MIGRATIONS`] entry so
/// pre-existing repo files pick the key up (as a commented block) on their next `mcp` start.
const CONFIG_VERSION: u32 = 2;
/// Version assumed for a file that carries no [`VERSION_MARKER_PREFIX`] line — i.e. a file
/// written before versioning existed. Such a file is run through every [`MIGRATIONS`] entry
/// (each presence-guarded) so it converges to the current schema without duplicating any key
/// it already holds. It stays the lowest version forever, so the floor never drifts.
const CONFIG_BASELINE_VERSION: u32 = 1;
/// Leading text of the comment line that stamps a config file's schema version, e.g.
/// `# codemap-config-version: 1`. Deliberately a comment, not a TOML key: the loader and
/// [`normalize`] never see it (preserving the "every key commented → empty layer → zero
/// warnings" invariant), and the migrator reads it with a plain string scan since the TOML
/// parser drops comments.
const VERSION_MARKER_PREFIX: &str = "# codemap-config-version:";

/// Commented, no-op scaffold written to a fresh repo on `mcp` start (see
/// [`ensure_repo_config`], which prepends the [`VERSION_MARKER_PREFIX`] line). Every key is
/// commented out at its compiled-in default. Section headers are live TOML tables, but they
/// are inert while every setting line stays commented, so the file resolves to defaults with
/// zero warnings until the user uncomments a setting. Mirrors the key reference in
/// `docs/configuration.md`; keep the two aligned when adding or renaming a key. When adding a
/// key, update `config_template.toml`, bump [`CONFIG_VERSION`], and add a [`MIGRATIONS`] entry
/// so existing repo files pick it up incrementally.
const CONFIG_TEMPLATE: &str = include_str!("config_template.toml");

/// Fully-resolved configuration. Every field carries a compiled-in default that
/// reproduces the post-Child-04 behavior exactly when no config file is present.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Tantivy index location (default `.codemap/index`).
    pub index_path: String,
    /// Number of top-ranked files `search` renders as details before remaining matches
    /// become compact ranked-tail rows (default 5).
    pub result_threshold: usize,
    /// Files larger than this (bytes) are skipped before read/parse (default 1 MiB).
    pub max_file_size: u64,
    /// Directory names never walked: the built-in junk dirs UNIONED with any configured
    /// names (augment, not replace — built-ins can't be un-excluded).
    pub excluded_directories: Vec<String>,
    /// Whether the walkers honor **`.git/info/exclude`** specifically (default true). This
    /// is a dedicated toggle for that one source only — `.gitignore`, the global gitignore,
    /// and `.codemapignore` are unaffected and stay honored. Set false to let
    /// index/codemap/find/grep see files hidden solely by `.git/info/exclude` (e.g. local
    /// personal excludes) while still honoring `.gitignore`. A per-call `include_ignored`
    /// on find/grep is a broader override that bypasses every ignore source.
    pub use_git_exclude: bool,
    /// Debounce window (milliseconds) for background index/codemap refreshes triggered by
    /// `search`/`overview` (default 5000). Within this window repeated calls enqueue at most
    /// one background refresh; each call still answers immediately from the committed
    /// snapshot. `read`/`find`/`grep` always read live disk, so any brief search staleness
    /// is corrected by the follow-up read/grep.
    pub index_staleness_ms: u64,
    /// Max file headers `search` emits in the compact ranked tail (default 12). Caps
    /// the context a broad query can spend after the top `result_threshold` detail files.
    /// Output-size only — safe to tune.
    pub search_overview_file_limit: usize,
    /// Filesystem watcher toggle (default true). When the watcher runs and is healthy,
    /// changes refresh the index autonomously and `search`/`overview` never trigger a
    /// tree walk; when false (or the watcher fails), the request-triggered lazy refresh
    /// (`index_staleness_ms`) is the fallback.
    pub watch: bool,
    /// Debounce window (milliseconds) for watcher events (default 500): events arriving
    /// within the window are batched into one incremental refresh, and a git HEAD change
    /// joins the same window so a half-written tree mid-checkout is not walked twice.
    pub watch_debounce_ms: u64,
    /// Automatic indexer recovery (default true). When the background indexer thread
    /// dies, the next `search`/`overview` rebuilds the engine, respawns the indexer, and
    /// re-attaches the watcher — bounded by a per-process attempt cap (see
    /// `mcp::MAX_INDEXER_RESTART_ATTEMPTS`) so a deterministic crash cannot respawn-loop.
    /// Set false to keep the frozen-results behavior until the server restarts.
    pub indexer_auto_restart: bool,
    /// Filesystem permissions for live disk tools (`find`, `grep`, `read`). Defaults keep
    /// every tool workspace-confined unless configured otherwise.
    pub filesystem_permissions: FilesystemPermissions,
    /// `grep` content-mode column cap (default 500, matching Claude Code's `--max-columns
    /// 500`). A matched line wider than this is replaced with `[Omitted long matching
    /// line]`; `0` disables the cap. Output-size only.
    pub grep_max_columns: usize,
    /// `read` always-applied output ceiling in bytes (default 102400 ≈ 100 KiB). Even with
    /// `offset`/`limit` set, a `read` whose rendered output exceeds this throws instead of
    /// emitting an unbounded blob (approximates Claude Code's ~25,000-token cap). Distinct
    /// from the 256 KiB whole-file cap that applies only when `limit` is omitted.
    pub read_output_byte_cap: usize,
    /// `search` detail-view per-symbol snippet line cap (default 80). A symbol body longer
    /// than this is truncated with an elision marker. Output-size only.
    pub search_detail_snippet_max_lines: usize,
    /// `search` detail-view per-file symbol cap (default 20). Beyond it, a "more symbols
    /// not shown" note replaces the remaining symbols. Output-size only.
    pub search_detail_symbol_limit: usize,
    /// Hard byte ceiling for one `search` response (default 32768 ≈ 32 KiB), including
    /// detail files, ranked tail, and the partial-output footer. Output-size only.
    pub search_detail_byte_cap: usize,
    /// `search` matched-literal truncation length in characters (default 200). A longer
    /// literal is cut with an ellipsis. Output-size only.
    pub search_literal_max_len: usize,
    /// `search` per-file matched-literal count cap (default 10). Output-size only.
    pub search_literal_limit: usize,
    /// `search` detail-view per-file anchor full-snippet cap (default 3). At most this many
    /// anchor symbols in one detail file render a FULL snippet; anchors ranked beyond the cap
    /// degrade to a ≤3-line signature (the Tier-2 abbreviation), not a one-line stub. A file
    /// whose anchor count is at or below the cap is unaffected. Output-size only — it bounds
    /// the snippet flood a broad query with a common name (`save`/`send`) can trigger.
    pub search_anchor_snippet_limit: usize,
    /// Repo-level default for the caller/callee context (default true). The per-call
    /// `caller_context` parameter, when supplied, always overrides this; the key only
    /// decides the default when the parameter is omitted.
    pub caller_context_default: bool,
    /// Repo-level default for navigation-based precise attribution (default false). Caller
    /// context still renders via the existing name-match fallback when this is off.
    pub navigation_context_default: bool,
    /// Max navigation call sites resolved in one annotation pass before falling back
    /// (default 1000).
    pub navigation_callsite_budget: usize,
    /// Whether extraction stores reference sites in `NavigationFile` (default false).
    pub navigation_store_references: bool,
    /// Max call sites collected across the single combined-regex caller scan (default 500).
    /// Shared by all matched names in one scan; reaching it marks the caller list truncated.
    pub scan_cap: usize,
    /// Per-symbol caller-list (or non-call-reference) cap (default 5). Output-size only.
    pub caller_list_cap: usize,
    /// Per-symbol callee-list cap (default 5). Output-size only.
    pub callee_list_cap: usize,
    /// Annotation byte sub-budget WITHIN `search_detail_byte_cap` (default 8192). A
    /// sub-limit, not an allowance added on top — snippets keep priority; annotations stop
    /// when either this or the remaining overall cap is exhausted.
    pub annotation_sub_budget: usize,
    /// Common-name threshold (default 2): a name with this many `fn` definitions in the
    /// snapshot has its caller list and callee occurrences labeled attribution-ambiguous
    /// (still rendered, never suppressed).
    pub common_name_threshold: usize,
    /// Caller-omission threshold (default 5): a matched `fn` name with this many `fn`
    /// definitions in the snapshot has its caller list replaced by a one-line omission note
    /// (attribution is unresolvable by a name-match scan; a `grep "name("` pointer is given
    /// instead). Stricter than `common_name_threshold`, which only labels the list — this
    /// suppresses it. Callees are unaffected. Output-size / signal-quality only.
    pub caller_omit_def_threshold: usize,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            index_path: format!("{CODEMAP_DIR_NAME}/index"),
            result_threshold: 5,
            max_file_size: crate::workspace::MAX_INDEXED_FILE_BYTES,
            excluded_directories: crate::workspace::EXCLUDED_DIRS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            use_git_exclude: true,
            index_staleness_ms: 5_000,
            search_overview_file_limit: 12,
            watch: true,
            watch_debounce_ms: 500,
            indexer_auto_restart: true,
            filesystem_permissions: FilesystemPermissions::default(),
            grep_max_columns: 500,
            read_output_byte_cap: 102_400,
            search_detail_snippet_max_lines: 80,
            search_detail_symbol_limit: 20,
            search_detail_byte_cap: 32_768,
            search_literal_max_len: 200,
            search_literal_limit: 10,
            search_anchor_snippet_limit: 3,
            caller_context_default: true,
            navigation_context_default: false,
            navigation_callsite_budget: 1000,
            navigation_store_references: false,
            scan_cap: 500,
            caller_list_cap: 5,
            callee_list_cap: 5,
            annotation_sub_budget: 8192,
            common_name_threshold: 2,
            caller_omit_def_threshold: 5,
        }
    }
}

/// One config file's parsed-and-validated contribution. Every key is optional so a
/// missing key delegates to the lower-precedence layer; invalid values are dropped
/// (warn + ignore) during normalization so they also delegate.
#[derive(Default)]
struct ConfigLayer {
    index_path: Option<String>,
    result_threshold: Option<usize>,
    max_file_size: Option<u64>,
    excluded_directories: Option<Vec<String>>,
    use_git_exclude: Option<bool>,
    index_staleness_ms: Option<u64>,
    search_overview_file_limit: Option<usize>,
    watch: Option<bool>,
    watch_debounce_ms: Option<u64>,
    indexer_auto_restart: Option<bool>,
    filesystem_permissions: FilesystemPermissionsLayer,
    grep_max_columns: Option<usize>,
    read_output_byte_cap: Option<usize>,
    search_detail_snippet_max_lines: Option<usize>,
    search_detail_symbol_limit: Option<usize>,
    search_detail_byte_cap: Option<usize>,
    search_literal_max_len: Option<usize>,
    search_literal_limit: Option<usize>,
    search_anchor_snippet_limit: Option<usize>,
    caller_context_default: Option<bool>,
    navigation_context_default: Option<bool>,
    navigation_callsite_budget: Option<usize>,
    navigation_store_references: Option<bool>,
    scan_cap: Option<usize>,
    caller_list_cap: Option<usize>,
    callee_list_cap: Option<usize>,
    annotation_sub_budget: Option<usize>,
    common_name_threshold: Option<usize>,
    caller_omit_def_threshold: Option<usize>,
}

#[derive(Default)]
struct FilesystemPermissionsLayer {
    find: Option<FilesystemPermissionPolicy>,
    grep: Option<FilesystemPermissionPolicy>,
    read: Option<FilesystemPermissionPolicy>,
    allowed_roots: Option<Vec<PathBuf>>,
}

/// Load and resolve config from `repo_root` and an explicitly-injected `global_dir`.
/// Pure (no globals, no env reads) so it is unit-testable with temp directories.
pub fn load(repo_root: &Path, global_dir: &Path) -> ResolvedConfig {
    let repo_layer = read_layer(&repo_root.join(CODEMAP_DIR_NAME).join(CONFIG_FILE_NAME));
    let global_layer = read_layer(&global_dir.join(CONFIG_FILE_NAME));
    merge(repo_layer, global_layer)
}

/// Read one config file into a normalized layer. Missing file → empty layer (silent);
/// read/parse failure → empty layer + stderr warning. Never returns an error.
fn read_layer(path: &Path) -> ConfigLayer {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ConfigLayer::default(),
        Err(e) => {
            warn(&format!(
                "config read failed: {}: {e} — using defaults",
                path.display()
            ));
            return ConfigLayer::default();
        }
    };
    let value: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            warn(&format!(
                "config parse failed: {}: {e} — using defaults",
                path.display()
            ));
            return ConfigLayer::default();
        }
    };
    normalize(value, path)
}

/// Validate a parsed TOML table into a [`ConfigLayer`]. Unknown keys and per-key type
/// mismatches warn and are skipped (the key delegates to a lower layer / default).
fn normalize(value: toml::Value, path: &Path) -> ConfigLayer {
    let mut layer = ConfigLayer::default();
    let table = match value {
        toml::Value::Table(t) => t,
        _ => {
            warn(&format!(
                "config top-level is not a table: {} — using defaults",
                path.display()
            ));
            return layer;
        }
    };
    let mut section_values = Vec::new();
    for (key, value) in table {
        match key.as_str() {
            "index" | "refresh" | "search" | "tool_output" | "caller_context" => {
                section_values.push((key, value));
            }
            "filesystem_permissions" => {
                layer.filesystem_permissions = normalize_filesystem_permissions(&value, path)
            }
            other => {
                if !assign_config_key(&mut layer, other, &value, other, path) {
                    warn(&format!(
                        "unknown config key '{other}': {} — ignored",
                        path.display()
                    ));
                }
            }
        }
    }
    for (section, value) in section_values {
        normalize_config_section(&mut layer, &section, &value, path);
    }
    layer
}

fn normalize_config_section(
    layer: &mut ConfigLayer,
    section: &str,
    value: &toml::Value,
    path: &Path,
) {
    let table = match value.as_table() {
        Some(table) => table,
        None => {
            warn(&format!(
                "config '{section}' must be a table: {} — ignored",
                path.display()
            ));
            return;
        }
    };

    for (key, value) in table {
        let key_display = format!("{section}.{key}");
        if section_accepts_key(section, key)
            && assign_config_key(layer, key, value, &key_display, path)
        {
            continue;
        }
        warn(&format!(
            "unknown config key '{key_display}': {} — ignored",
            path.display()
        ));
    }
}

fn section_accepts_key(section: &str, key: &str) -> bool {
    match section {
        "index" => matches!(
            key,
            "index_path" | "max_file_size" | "excluded_directories" | "use_git_exclude"
        ),
        "refresh" => matches!(
            key,
            "watch" | "watch_debounce_ms" | "index_staleness_ms" | "indexer_auto_restart"
        ),
        "search" => matches!(
            key,
            "result_threshold"
                | "search_overview_file_limit"
                | "search_detail_snippet_max_lines"
                | "search_detail_symbol_limit"
                | "search_detail_byte_cap"
                | "search_literal_max_len"
                | "search_literal_limit"
                | "search_anchor_snippet_limit"
        ),
        "tool_output" => matches!(key, "grep_max_columns" | "read_output_byte_cap"),
        "caller_context" => matches!(
            key,
            "caller_context_default"
                | "navigation_context_default"
                | "navigation_callsite_budget"
                | "navigation_store_references"
                | "scan_cap"
                | "caller_list_cap"
                | "callee_list_cap"
                | "annotation_sub_budget"
                | "common_name_threshold"
                | "caller_omit_def_threshold"
        ),
        _ => false,
    }
}

fn assign_config_key(
    layer: &mut ConfigLayer,
    key: &str,
    value: &toml::Value,
    key_display: &str,
    path: &Path,
) -> bool {
    match key {
        "index_path" => layer.index_path = as_nonempty_string(value, key_display, path),
        "result_threshold" => layer.result_threshold = as_positive_usize(value, key_display, path),
        "max_file_size" => layer.max_file_size = as_positive_u64(value, key_display, path),
        "excluded_directories" => {
            layer.excluded_directories = as_string_array(value, key_display, path)
        }
        "use_git_exclude" => layer.use_git_exclude = as_bool(value, key_display, path),
        "index_staleness_ms" => {
            layer.index_staleness_ms = as_positive_u64(value, key_display, path)
        }
        "search_overview_file_limit" => {
            layer.search_overview_file_limit = as_positive_usize(value, key_display, path)
        }
        "watch" => layer.watch = as_bool(value, key_display, path),
        "watch_debounce_ms" => layer.watch_debounce_ms = as_positive_u64(value, key_display, path),
        "indexer_auto_restart" => layer.indexer_auto_restart = as_bool(value, key_display, path),
        "grep_max_columns" => layer.grep_max_columns = as_nonneg_usize(value, key_display, path),
        "read_output_byte_cap" => {
            layer.read_output_byte_cap = as_positive_usize(value, key_display, path)
        }
        "search_detail_snippet_max_lines" => {
            layer.search_detail_snippet_max_lines = as_positive_usize(value, key_display, path)
        }
        "search_detail_symbol_limit" => {
            layer.search_detail_symbol_limit = as_positive_usize(value, key_display, path)
        }
        "search_detail_byte_cap" => {
            layer.search_detail_byte_cap = as_positive_usize(value, key_display, path)
        }
        "search_literal_max_len" => {
            layer.search_literal_max_len = as_positive_usize(value, key_display, path)
        }
        "search_literal_limit" => {
            layer.search_literal_limit = as_positive_usize(value, key_display, path)
        }
        "search_anchor_snippet_limit" => {
            layer.search_anchor_snippet_limit = as_positive_usize(value, key_display, path)
        }
        "caller_context_default" => {
            layer.caller_context_default = as_bool(value, key_display, path)
        }
        "navigation_context_default" => {
            layer.navigation_context_default = as_bool(value, key_display, path)
        }
        "navigation_callsite_budget" => {
            layer.navigation_callsite_budget = as_positive_usize(value, key_display, path)
        }
        "navigation_store_references" => {
            layer.navigation_store_references = as_bool(value, key_display, path)
        }
        "scan_cap" => layer.scan_cap = as_positive_usize(value, key_display, path),
        "caller_list_cap" => layer.caller_list_cap = as_positive_usize(value, key_display, path),
        "callee_list_cap" => layer.callee_list_cap = as_positive_usize(value, key_display, path),
        "annotation_sub_budget" => {
            layer.annotation_sub_budget = as_positive_usize(value, key_display, path)
        }
        "common_name_threshold" => {
            layer.common_name_threshold = as_positive_usize(value, key_display, path)
        }
        "caller_omit_def_threshold" => {
            layer.caller_omit_def_threshold = as_positive_usize(value, key_display, path)
        }
        _ => return false,
    }
    true
}

/// Per-key `repo > global > default` merge. Array keys take the winning layer's list and
/// UNION it with the built-in defaults (augment — built-ins are never dropped).
fn merge(repo: ConfigLayer, global: ConfigLayer) -> ResolvedConfig {
    let defaults = ResolvedConfig::default();
    let excluded_directories = match repo.excluded_directories.or(global.excluded_directories) {
        Some(extra) => union_excludes(defaults.excluded_directories, extra),
        None => defaults.excluded_directories,
    };
    ResolvedConfig {
        index_path: repo
            .index_path
            .or(global.index_path)
            .unwrap_or(defaults.index_path),
        result_threshold: repo
            .result_threshold
            .or(global.result_threshold)
            .unwrap_or(defaults.result_threshold),
        max_file_size: repo
            .max_file_size
            .or(global.max_file_size)
            .unwrap_or(defaults.max_file_size),
        excluded_directories,
        use_git_exclude: repo
            .use_git_exclude
            .or(global.use_git_exclude)
            .unwrap_or(defaults.use_git_exclude),
        index_staleness_ms: repo
            .index_staleness_ms
            .or(global.index_staleness_ms)
            .unwrap_or(defaults.index_staleness_ms),
        search_overview_file_limit: repo
            .search_overview_file_limit
            .or(global.search_overview_file_limit)
            .unwrap_or(defaults.search_overview_file_limit),
        watch: repo.watch.or(global.watch).unwrap_or(defaults.watch),
        watch_debounce_ms: repo
            .watch_debounce_ms
            .or(global.watch_debounce_ms)
            .unwrap_or(defaults.watch_debounce_ms),
        indexer_auto_restart: repo
            .indexer_auto_restart
            .or(global.indexer_auto_restart)
            .unwrap_or(defaults.indexer_auto_restart),
        filesystem_permissions: merge_filesystem_permissions(
            repo.filesystem_permissions,
            global.filesystem_permissions,
            defaults.filesystem_permissions,
        ),
        grep_max_columns: repo
            .grep_max_columns
            .or(global.grep_max_columns)
            .unwrap_or(defaults.grep_max_columns),
        read_output_byte_cap: repo
            .read_output_byte_cap
            .or(global.read_output_byte_cap)
            .unwrap_or(defaults.read_output_byte_cap),
        search_detail_snippet_max_lines: repo
            .search_detail_snippet_max_lines
            .or(global.search_detail_snippet_max_lines)
            .unwrap_or(defaults.search_detail_snippet_max_lines),
        search_detail_symbol_limit: repo
            .search_detail_symbol_limit
            .or(global.search_detail_symbol_limit)
            .unwrap_or(defaults.search_detail_symbol_limit),
        search_detail_byte_cap: repo
            .search_detail_byte_cap
            .or(global.search_detail_byte_cap)
            .unwrap_or(defaults.search_detail_byte_cap),
        search_literal_max_len: repo
            .search_literal_max_len
            .or(global.search_literal_max_len)
            .unwrap_or(defaults.search_literal_max_len),
        search_literal_limit: repo
            .search_literal_limit
            .or(global.search_literal_limit)
            .unwrap_or(defaults.search_literal_limit),
        search_anchor_snippet_limit: repo
            .search_anchor_snippet_limit
            .or(global.search_anchor_snippet_limit)
            .unwrap_or(defaults.search_anchor_snippet_limit),
        caller_context_default: repo
            .caller_context_default
            .or(global.caller_context_default)
            .unwrap_or(defaults.caller_context_default),
        navigation_context_default: repo
            .navigation_context_default
            .or(global.navigation_context_default)
            .unwrap_or(defaults.navigation_context_default),
        navigation_callsite_budget: repo
            .navigation_callsite_budget
            .or(global.navigation_callsite_budget)
            .unwrap_or(defaults.navigation_callsite_budget),
        navigation_store_references: repo
            .navigation_store_references
            .or(global.navigation_store_references)
            .unwrap_or(defaults.navigation_store_references),
        scan_cap: repo
            .scan_cap
            .or(global.scan_cap)
            .unwrap_or(defaults.scan_cap),
        caller_list_cap: repo
            .caller_list_cap
            .or(global.caller_list_cap)
            .unwrap_or(defaults.caller_list_cap),
        callee_list_cap: repo
            .callee_list_cap
            .or(global.callee_list_cap)
            .unwrap_or(defaults.callee_list_cap),
        annotation_sub_budget: repo
            .annotation_sub_budget
            .or(global.annotation_sub_budget)
            .unwrap_or(defaults.annotation_sub_budget),
        common_name_threshold: repo
            .common_name_threshold
            .or(global.common_name_threshold)
            .unwrap_or(defaults.common_name_threshold),
        caller_omit_def_threshold: repo
            .caller_omit_def_threshold
            .or(global.caller_omit_def_threshold)
            .unwrap_or(defaults.caller_omit_def_threshold),
    }
}

/// Append `extra` directory names to `base`, skipping duplicates (case-sensitive).
fn union_excludes(mut base: Vec<String>, extra: Vec<String>) -> Vec<String> {
    for name in extra {
        if !base.contains(&name) {
            base.push(name);
        }
    }
    base
}

fn normalize_filesystem_permissions(
    value: &toml::Value,
    path: &Path,
) -> FilesystemPermissionsLayer {
    let mut layer = FilesystemPermissionsLayer::default();
    let table = match value.as_table() {
        Some(table) => table,
        None => {
            warn(&format!(
                "config 'filesystem_permissions' must be a table: {} — ignored",
                path.display()
            ));
            return layer;
        }
    };

    for (key, value) in table {
        match key.as_str() {
            "find" => layer.find = as_permission_policy(value, "filesystem_permissions.find", path),
            "grep" => layer.grep = as_permission_policy(value, "filesystem_permissions.grep", path),
            "read" => layer.read = as_permission_policy(value, "filesystem_permissions.read", path),
            "allowed_roots" => {
                layer.allowed_roots =
                    as_allowed_roots(value, "filesystem_permissions.allowed_roots", path)
            }
            other => warn(&format!(
                "unknown config key 'filesystem_permissions.{other}': {} — ignored",
                path.display()
            )),
        }
    }
    layer
}

fn merge_filesystem_permissions(
    repo: FilesystemPermissionsLayer,
    global: FilesystemPermissionsLayer,
    defaults: FilesystemPermissions,
) -> FilesystemPermissions {
    FilesystemPermissions {
        find: repo.find.or(global.find).unwrap_or(defaults.find),
        grep: repo.grep.or(global.grep).unwrap_or(defaults.grep),
        read: repo.read.or(global.read).unwrap_or(defaults.read),
        allowed_roots: repo
            .allowed_roots
            .or(global.allowed_roots)
            .unwrap_or(defaults.allowed_roots),
    }
}

// --- Process-wide resolved config ------------------------------------------------------

static CONFIG: OnceLock<ResolvedConfig> = OnceLock::new();

/// Resolve the global config directory: `$CODEMAP_HOME`, else `~/.codemap`
/// (`$HOME`/`$USERPROFILE`), else a bare `.codemap` as a last resort.
fn global_dir() -> PathBuf {
    if let Some(home) = std::env::var_os(HOME_ENV) {
        return PathBuf::from(home);
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return PathBuf::from(home).join(CODEMAP_DIR_NAME);
    }
    PathBuf::from(CODEMAP_DIR_NAME)
}

/// Load config from `repo_root` + the resolved global dir and store it process-wide.
/// Call once at startup, before any [`get`]. A second call is a no-op.
pub fn init(repo_root: &Path) {
    let resolved = load(repo_root, &global_dir());
    let _ = CONFIG.set(resolved);
}

/// The resolved config. Falls back to defaults if [`init`] was never called (e.g. unit
/// tests that exercise the walker directly without booting the server).
pub fn get() -> &'static ResolvedConfig {
    CONFIG.get_or_init(ResolvedConfig::default)
}

/// Where a key's commented block is inserted during an incremental sync. Placement is
/// section-aware because TOML scopes every key after a `[table]` header to that table: a new
/// top-level key dropped at end-of-file would silently fall under `[filesystem_permissions]`
/// the moment a user uncomments both, so it must land *before* the first table header.
///
/// `allow(dead_code)`: the variants are only constructed by [`MIGRATIONS`] entries (none at
/// the v1 baseline) and by the migration unit tests, so a release with an empty registry has
/// no non-test constructor. They become live the moment the first key is added.
#[derive(Clone, Copy)]
#[allow(dead_code)]
enum KeyPlacement {
    /// A bare top-level key — inserted before the first `[table]` header (commented or not).
    TopLevel,
    /// A key under the named sub-table — inserted right after that table's header line.
    Subtable(&'static str),
}

/// One additive schema change: the commented block for a key introduced at `version`.
/// [`ensure_repo_config`] applies every entry newer than a file's stamped version, skipping
/// any whose `key` the file already mentions (presence guard), then re-stamps the marker.
///
/// `allow(dead_code)`: like [`KeyPlacement`], instances exist only in [`MIGRATIONS`] (empty at
/// v1) and the tests, so the fields have no non-test reader until the first migration ships.
#[allow(dead_code)]
struct Migration {
    /// Schema version that introduced `key`. Applied to files stamped older than this.
    version: u32,
    /// Key name the presence guard scans for (top-level name, or the sub-table leaf key) so a
    /// key the user already added or uncommented is never duplicated.
    key: &'static str,
    /// Section-aware insertion point for `block`.
    placement: KeyPlacement,
    /// The commented template block for the key, in [`CONFIG_TEMPLATE`]'s style (doc comment
    /// line(s) then a `# key = default` line, no surrounding blank lines — the inserter spaces
    /// it).
    block: &'static str,
}

/// Ordered, additive migrations. v1 is the baseline; later entries add commented blocks for
/// keys introduced after that baseline. To introduce a key in a later release: add its commented
/// block to [`CONFIG_TEMPLATE`] at its logical position, bump [`CONFIG_VERSION`] to N, then
/// append a `Migration { version: N, key: "...", placement: ..., block: "..." }` entry here.
///
/// Existing repo files then gain the key (commented, before the first table header) and a
/// refreshed version marker on their next `mcp` start, with their own edits untouched.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 2,
        key: "navigation_context_default",
        placement: KeyPlacement::TopLevel,
        block: "# Precise caller/callee attribution. When false, annotations use conservative name matching.\n# Set true to allow tree-sitter navigation data to mark unambiguous lines as precise.\n# navigation_context_default = false",
    },
    Migration {
        version: 2,
        key: "navigation_callsite_budget",
        placement: KeyPlacement::TopLevel,
        block: "# navigation_callsite_budget = 1000",
    },
    Migration {
        version: 2,
        key: "navigation_store_references",
        placement: KeyPlacement::TopLevel,
        block: "# navigation_store_references = false",
    },
];

/// `# codemap-config-version: <version>` — the stamp line written into every managed file.
fn version_marker_line(version: u32) -> String {
    format!("{VERSION_MARKER_PREFIX} {version}")
}

/// Scaffold or incrementally sync `<repo_root>/.codemap/config.toml` on `mcp` start.
///
/// - Absent → write the commented, no-op [`CONFIG_TEMPLATE`] stamped with [`CONFIG_VERSION`],
///   so a fresh repo gets a discoverable, self-documenting, behavior-preserving config.
/// - Present → run [`apply_migrations`]: append only the commented blocks for keys introduced
///   since the file's stamped version (presence-guarded), re-stamp the marker, and rewrite.
///   A file already at the current version is left byte-for-byte untouched.
///
/// Never-exit: a directory-create, read, or write failure warns to stderr and returns rather
/// than crashing the server. The path matches exactly what [`load`] reads, so a newly added
/// (commented) key is inert until uncommented and takes effect on a later run.
pub fn ensure_repo_config(repo_root: &Path) {
    let dir = repo_root.join(CODEMAP_DIR_NAME);
    let path = dir.join(CONFIG_FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(existing) => migrate_existing(&path, &existing),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => scaffold_fresh(&dir, &path),
        Err(e) => warn(&format!(
            "config sync skipped: read {}: {e}",
            path.display()
        )),
    }
}

/// Write the version-stamped [`CONFIG_TEMPLATE`] for a repo that has no config file yet.
fn scaffold_fresh(dir: &Path, path: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        warn(&format!(
            "config template skipped: create {}: {e}",
            dir.display()
        ));
        return;
    }
    let body = format!("{}\n{CONFIG_TEMPLATE}", version_marker_line(CONFIG_VERSION));
    if let Err(e) = std::fs::write(path, body) {
        warn(&format!(
            "config template skipped: write {}: {e}",
            path.display()
        ));
        return;
    }
    warn(&format!("created default config: {}", path.display()));
}

/// Incrementally sync an existing config file: stamp-gate, presence-guarded additive insert,
/// re-stamp, rewrite. A no-op (no write) when the file is already at [`CONFIG_VERSION`].
fn migrate_existing(path: &Path, existing: &str) {
    let file_version = parse_version_marker(existing).unwrap_or(CONFIG_BASELINE_VERSION);
    let Some(updated) = apply_migrations(existing, file_version, CONFIG_VERSION, MIGRATIONS) else {
        return; // already current — never touch the user's file
    };
    if let Err(e) = std::fs::write(path, updated) {
        warn(&format!(
            "config sync skipped: write {}: {e}",
            path.display()
        ));
        return;
    }
    warn(&format!(
        "synced config to schema v{CONFIG_VERSION}: {}",
        path.display()
    ));
}

/// Read the schema version stamped by [`VERSION_MARKER_PREFIX`]. `None` when absent or the
/// trailing token is not an integer (the caller then assumes [`CONFIG_BASELINE_VERSION`]).
fn parse_version_marker(contents: &str) -> Option<u32> {
    contents.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix(VERSION_MARKER_PREFIX)
            .and_then(|rest| rest.trim().parse::<u32>().ok())
    })
}

/// Whether `contents` already assigns `key` (commented or live). Matches a line that, after an
/// optional leading `#`, begins with `key` followed by `=` — so it accepts `key = x` and
/// `# key = x` but not a longer name that merely starts with `key` (e.g. `watch` vs
/// `watch_debounce_ms`). The presence guard: errs toward NOT inserting, never duplicating.
fn file_mentions_key(contents: &str, key: &str) -> bool {
    contents.lines().any(|line| {
        let body = line.trim_start();
        let body = body.strip_prefix('#').map(str::trim_start).unwrap_or(body);
        body.strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
}

/// Apply the additive migrations newer than `file_version` (up to `target_version`) to
/// `contents`. Each unseen key's block is inserted section-aware; the version marker is then
/// stamped to `target_version`. Returns `Some(new_contents)` when a sync is due, or `None`
/// when the file is already at/ahead of the target (so the caller writes nothing).
fn apply_migrations(
    contents: &str,
    file_version: u32,
    target_version: u32,
    migrations: &[Migration],
) -> Option<String> {
    if file_version >= target_version {
        return None;
    }
    let mut out = contents.to_string();
    for migration in migrations
        .iter()
        .filter(|m| m.version > file_version && m.version <= target_version)
    {
        if file_mentions_key(&out, migration.key) {
            continue; // presence guard — already there, never duplicate
        }
        out = match migration.placement {
            KeyPlacement::TopLevel => insert_top_level(&out, migration.block),
            KeyPlacement::Subtable(table) => insert_subtable(&out, table, migration.block),
        };
    }
    // `file_version < target_version` here, so the marker always advances → always a change.
    Some(set_version_marker(&out, target_version))
}

/// Byte offset of the first line that opens a TOML table (`[...]`), honoring an optional
/// leading `# ` so it also anchors before a *commented* `# [filesystem_permissions]`. `None`
/// when the file has no table header.
fn first_table_header_offset(contents: &str) -> Option<usize> {
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        let body = line.trim_start();
        let body = body.strip_prefix('#').map(str::trim_start).unwrap_or(body);
        if body.starts_with('[') {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

/// Insert a top-level key's `block` before the first table header (a blank line after it), or
/// append it at end-of-file when there is no table. Keeps top-level keys out of any table's
/// scope no matter what the user later uncomments.
fn insert_top_level(contents: &str, block: &str) -> String {
    match first_table_header_offset(contents) {
        Some(at) => {
            let mut out = String::with_capacity(contents.len() + block.len() + 2);
            out.push_str(&contents[..at]);
            out.push_str(block);
            out.push_str("\n\n");
            out.push_str(&contents[at..]);
            out
        }
        None => {
            let mut out = contents.to_string();
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
            out.push_str(block);
            out.push('\n');
            out
        }
    }
}

/// Insert a sub-table key's `block` right after the `[table]` header line. Falls back to
/// top-level placement when the header is absent (degraded but never destructive).
fn insert_subtable(contents: &str, table: &str, block: &str) -> String {
    let header = format!("[{table}]");
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        let body = line.trim_start();
        let body = body.strip_prefix('#').map(str::trim_start).unwrap_or(body);
        if body.starts_with(&header) {
            let at = offset + line.len(); // immediately after the header line
            let mut out = String::with_capacity(contents.len() + block.len() + 1);
            out.push_str(&contents[..at]);
            out.push_str(block);
            out.push('\n');
            out.push_str(&contents[at..]);
            return out;
        }
        offset += line.len();
    }
    insert_top_level(contents, block)
}

/// Replace the existing [`VERSION_MARKER_PREFIX`] line's value with `version`, or prepend a
/// fresh marker line when the file has none.
fn set_version_marker(contents: &str, version: u32) -> String {
    let marker = version_marker_line(version);
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        if line.trim_start().starts_with(VERSION_MARKER_PREFIX) {
            let line_end = offset + line.len();
            let mut out = String::with_capacity(contents.len() + marker.len());
            out.push_str(&contents[..offset]);
            out.push_str(&marker);
            if line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&contents[line_end..]);
            return out;
        }
        offset += line.len();
    }
    format!("{marker}\n{contents}")
}

// --- value validators (warn + drop on mismatch) ----------------------------------------

fn as_nonempty_string(value: &toml::Value, key: &str, path: &Path) -> Option<String> {
    match value.as_str() {
        Some(s) if !s.trim().is_empty() => Some(s.to_string()),
        _ => {
            warn(&format!(
                "config '{key}' must be a non-empty string: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_positive_usize(value: &toml::Value, key: &str, path: &Path) -> Option<usize> {
    match value.as_integer() {
        Some(n) if n > 0 => Some(n as usize),
        _ => {
            warn(&format!(
                "config '{key}' must be a positive integer: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_nonneg_usize(value: &toml::Value, key: &str, path: &Path) -> Option<usize> {
    match value.as_integer() {
        Some(n) if n >= 0 => Some(n as usize),
        _ => {
            warn(&format!(
                "config '{key}' must be a non-negative integer: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_positive_u64(value: &toml::Value, key: &str, path: &Path) -> Option<u64> {
    match value.as_integer() {
        Some(n) if n > 0 => Some(n as u64),
        _ => {
            warn(&format!(
                "config '{key}' must be a positive integer: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_bool(value: &toml::Value, key: &str, path: &Path) -> Option<bool> {
    match value.as_bool() {
        Some(b) => Some(b),
        None => {
            warn(&format!(
                "config '{key}' must be true/false: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_permission_policy(
    value: &toml::Value,
    key: &str,
    path: &Path,
) -> Option<FilesystemPermissionPolicy> {
    match value.as_str() {
        Some("workspace") => Some(FilesystemPermissionPolicy::Workspace),
        Some("allowed_roots") => Some(FilesystemPermissionPolicy::AllowedRoots),
        Some("anywhere") => Some(FilesystemPermissionPolicy::Anywhere),
        _ => {
            warn(&format!(
                "config '{key}' must be one of 'workspace', 'allowed_roots', or 'anywhere': {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_string_array(value: &toml::Value, key: &str, path: &Path) -> Option<Vec<String>> {
    let array = match value.as_array() {
        Some(a) => a,
        None => {
            warn(&format!(
                "config '{key}' must be an array of strings: {} — ignored",
                path.display()
            ));
            return None;
        }
    };
    let mut out = Vec::with_capacity(array.len());
    for element in array {
        match element.as_str() {
            Some(s) => out.push(s.to_string()),
            None => {
                warn(&format!(
                    "config '{key}' must contain only strings: {} — ignored",
                    path.display()
                ));
                return None;
            }
        }
    }
    Some(out)
}

fn as_allowed_roots(value: &toml::Value, key: &str, path: &Path) -> Option<Vec<PathBuf>> {
    let raw_roots = as_string_array(value, key, path)?;
    let mut roots = Vec::with_capacity(raw_roots.len());
    for raw_root in raw_roots {
        if raw_root.trim().is_empty() {
            warn(&format!(
                "config '{key}' must not contain empty paths: {} — ignored",
                path.display()
            ));
            return None;
        }
        let root_path = crate::workspace::path_from_workspace_input(&raw_root);
        roots.push(crate::workspace::canonicalize_path_lenient(&root_path));
    }
    Some(roots)
}

/// Diagnostics go to stderr only — stdout is reserved for the MCP JSON-RPC stream (the
/// never-exit philosophy's surface: warn, never throw/exit).
fn warn(message: &str) {
    eprintln!("[codemap-search] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_repo_config(repo: &Path, body: &str) {
        let dir = repo.join(CODEMAP_DIR_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONFIG_FILE_NAME), body).unwrap();
    }

    #[test]
    fn test_defaults_when_no_files() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        let cfg = load(repo.path(), global.path());
        let defaults = ResolvedConfig::default();
        assert_eq!(cfg.index_path, defaults.index_path);
        assert_eq!(cfg.result_threshold, 5);
        assert_eq!(cfg.max_file_size, crate::workspace::MAX_INDEXED_FILE_BYTES);
        assert!(cfg.use_git_exclude);
        assert!(cfg.excluded_directories.iter().any(|d| d == "node_modules"));
        assert_eq!(cfg.search_anchor_snippet_limit, 3);
        // Caller/callee context: annotation on by default, caps at their tuned values.
        assert!(cfg.caller_context_default);
        assert_eq!(cfg.scan_cap, 500);
        assert_eq!(cfg.caller_list_cap, 5);
        assert_eq!(cfg.callee_list_cap, 5);
        assert_eq!(cfg.annotation_sub_budget, 8192);
        assert_eq!(cfg.common_name_threshold, 2);
    }

    #[test]
    fn test_caller_context_keys_override() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        write_repo_config(
            repo.path(),
            "caller_context_default = false\nscan_cap = 100\ncommon_name_threshold = 3\n",
        );
        let cfg = load(repo.path(), global.path());
        assert!(
            !cfg.caller_context_default,
            "repo disables the on-by-default annotation"
        );
        assert_eq!(cfg.scan_cap, 100);
        assert_eq!(cfg.common_name_threshold, 3);
        // Untouched keys keep their defaults.
        assert_eq!(cfg.caller_list_cap, 5);
        assert_eq!(cfg.annotation_sub_budget, 8192);
    }

    #[test]
    fn test_repo_overrides_global_overrides_default() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        // global sets threshold=20 and max_file_size=10; repo overrides threshold=3 only.
        fs::write(
            global.path().join(CONFIG_FILE_NAME),
            "result_threshold = 20\nmax_file_size = 10\n",
        )
        .unwrap();
        write_repo_config(repo.path(), "result_threshold = 3\n");
        let cfg = load(repo.path(), global.path());
        assert_eq!(cfg.result_threshold, 3, "repo wins for threshold");
        assert_eq!(cfg.max_file_size, 10, "global wins where repo is silent");
    }

    #[test]
    fn test_excluded_directories_augment_not_replace() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        write_repo_config(
            repo.path(),
            "excluded_directories = [\"__pycache__\", \"coverage\"]\n",
        );
        let cfg = load(repo.path(), global.path());
        // configured names are present...
        assert!(cfg.excluded_directories.iter().any(|d| d == "__pycache__"));
        assert!(cfg.excluded_directories.iter().any(|d| d == "coverage"));
        // ...and the built-ins are NOT dropped.
        assert!(cfg.excluded_directories.iter().any(|d| d == "node_modules"));
        assert!(cfg.excluded_directories.iter().any(|d| d == "target"));
    }

    #[test]
    fn test_use_git_exclude_default_and_override() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        assert!(
            load(repo.path(), global.path()).use_git_exclude,
            "defaults to true"
        );
        write_repo_config(repo.path(), "use_git_exclude = false\n");
        assert!(
            !load(repo.path(), global.path()).use_git_exclude,
            "repo override to false"
        );
    }

    #[test]
    fn test_malformed_config_falls_back_to_defaults() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        write_repo_config(repo.path(), "this is = = not valid toml [[[");
        let cfg = load(repo.path(), global.path());
        assert_eq!(
            cfg.result_threshold, 5,
            "malformed config must degrade to defaults, not crash"
        );
    }

    #[test]
    fn test_unknown_key_and_bad_type_ignored() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        // unknown key + a wrong-typed known key (string where int expected) → both ignored.
        write_repo_config(
            repo.path(),
            "totally_unknown = 1\nresult_threshold = \"five\"\n",
        );
        let cfg = load(repo.path(), global.path());
        assert_eq!(
            cfg.result_threshold, 5,
            "bad-typed key must fall back to default"
        );
    }

    // --- version marker + incremental sync ---------------------------------------------

    #[test]
    fn test_parse_version_marker() {
        assert_eq!(
            parse_version_marker("# codemap-config-version: 3\nfoo = 1\n"),
            Some(3)
        );
        // the marker need not be the first line
        assert_eq!(
            parse_version_marker("foo = 1\n# codemap-config-version: 7"),
            Some(7)
        );
        // absent → None (caller assumes the baseline)
        assert_eq!(parse_version_marker("foo = 1\n"), None);
        // non-integer trailing token → None
        assert_eq!(
            parse_version_marker("# codemap-config-version: vNext\n"),
            None
        );
    }

    #[test]
    fn test_file_mentions_key_guards_prefix_collisions() {
        // commented, live, and tightly-spaced forms all count as present
        assert!(file_mentions_key(
            "# result_threshold = 5\n",
            "result_threshold"
        ));
        assert!(file_mentions_key(
            "result_threshold = 3\n",
            "result_threshold"
        ));
        assert!(file_mentions_key(
            "#result_threshold=3\n",
            "result_threshold"
        ));
        // a longer key that merely starts with the name must NOT match
        assert!(!file_mentions_key("watch_debounce_ms = 500\n", "watch"));
        assert!(!file_mentions_key(
            "nothing relevant here\n",
            "result_threshold"
        ));
    }

    #[test]
    fn test_apply_migrations_noop_when_already_current() {
        let body = "# codemap-config-version: 2\nfoo = 1\n";
        // equal version → no sync due
        assert!(apply_migrations(body, 2, 2, &[]).is_none());
        // file ahead of target → also a no-op (never downgrade)
        assert!(apply_migrations(body, 3, 2, &[]).is_none());
    }

    #[test]
    fn test_apply_migrations_inserts_top_level_before_table() {
        let migrations = &[Migration {
            version: 2,
            key: "new_key",
            placement: KeyPlacement::TopLevel,
            block: "# New key doc.\n# new_key = 7",
        }];
        let body = "# codemap-config-version: 1\n# index doc\n# index_path = \".codemap/index\"\n\n# [filesystem_permissions]\n# find = \"workspace\"\n";
        let out = apply_migrations(body, 1, 2, migrations).expect("a sync is due");
        // section-aware: the new top-level key precedes the table header so it can never be
        // captured by the table when both are later uncommented.
        let key_pos = out.find("new_key").unwrap();
        let table_pos = out.find("[filesystem_permissions]").unwrap();
        assert!(
            key_pos < table_pos,
            "new top-level key must precede the table header: {out:?}"
        );
        // marker advanced, user content preserved
        assert!(out.contains("# codemap-config-version: 2"));
        assert!(!out.contains("version: 1"));
        assert!(out.contains("# index_path = \".codemap/index\""));
    }

    #[test]
    fn test_apply_migrations_presence_guard_skips_existing_key() {
        let migrations = &[Migration {
            version: 2,
            key: "already_here",
            placement: KeyPlacement::TopLevel,
            block: "# dup doc.\n# already_here = 1",
        }];
        // the user already has the key (commented). It must not be duplicated, but the marker
        // still advances so the file is not re-scanned every run.
        let body = "# codemap-config-version: 1\n# already_here = 99\n";
        let out = apply_migrations(body, 1, 2, migrations).expect("marker still advances");
        assert_eq!(
            out.matches("already_here").count(),
            1,
            "presence guard must not duplicate an existing key: {out:?}"
        );
        assert!(out.contains("# codemap-config-version: 2"));
    }

    #[test]
    fn test_apply_migrations_premarker_file_runs_from_baseline() {
        // A file with no marker is treated as CONFIG_BASELINE_VERSION, so a later migration
        // applies and the user's existing content is preserved verbatim.
        let migrations = &[Migration {
            version: 2,
            key: "added_in_v2",
            placement: KeyPlacement::TopLevel,
            block: "# v2 key.\n# added_in_v2 = 1",
        }];
        let body = "result_threshold = 3\n";
        assert_eq!(parse_version_marker(body), None, "fixture has no marker");
        let out = apply_migrations(body, CONFIG_BASELINE_VERSION, 2, migrations).unwrap();
        assert!(out.contains("added_in_v2"), "v2 key inserted: {out:?}");
        assert!(
            out.contains("# codemap-config-version: 2"),
            "marker stamped: {out:?}"
        );
        assert!(
            out.contains("result_threshold = 3"),
            "user content preserved: {out:?}"
        );
    }

    #[test]
    fn test_insert_subtable_places_key_under_header() {
        let body =
            "# codemap-config-version: 1\n# [filesystem_permissions]\n# find = \"workspace\"\n";
        let out = insert_subtable(body, "filesystem_permissions", "# new_perm = \"x\"");
        let header_pos = out.find("[filesystem_permissions]").unwrap();
        let key_pos = out.find("new_perm").unwrap();
        assert!(
            header_pos < key_pos,
            "a sub-table key must follow its header: {out:?}"
        );
    }

    #[test]
    fn test_set_version_marker_replaces_or_prepends() {
        // replace an existing marker in place
        let replaced = set_version_marker("# codemap-config-version: 1\nfoo = 1\n", 5);
        assert!(replaced.contains("# codemap-config-version: 5"));
        assert!(!replaced.contains("version: 1"));
        assert!(replaced.contains("foo = 1"));
        // prepend when none exists
        let prepended = set_version_marker("foo = 1\n", 3);
        assert!(prepended.starts_with("# codemap-config-version: 3\n"));
        assert!(prepended.contains("foo = 1"));
    }

    #[test]
    fn test_ensure_repo_config_scaffolds_with_version_marker() {
        let repo = tempdir().unwrap();
        ensure_repo_config(repo.path());
        let path = repo.path().join(CODEMAP_DIR_NAME).join(CONFIG_FILE_NAME);
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.starts_with(&format!("# codemap-config-version: {CONFIG_VERSION}\n")),
            "scaffold must be stamped with the current schema version: {body:?}"
        );
        // still a no-op layer: every key commented → parsing yields the defaults
        let global = tempdir().unwrap();
        assert_eq!(load(repo.path(), global.path()).result_threshold, 5);
    }

    #[test]
    fn test_ensure_repo_config_is_idempotent() {
        // A freshly scaffolded file is already current, so the next run must not rewrite it
        // (preserves the "never touch a current file" guarantee end-to-end with the real
        // registry, independent of CONFIG_VERSION's value).
        let repo = tempdir().unwrap();
        ensure_repo_config(repo.path());
        let path = repo.path().join(CODEMAP_DIR_NAME).join(CONFIG_FILE_NAME);
        let after_scaffold = fs::read_to_string(&path).unwrap();
        ensure_repo_config(repo.path());
        let after_second = fs::read_to_string(&path).unwrap();
        assert_eq!(
            after_scaffold, after_second,
            "a current config file must not be rewritten on a later run"
        );
    }
}
