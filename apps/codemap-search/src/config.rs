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
//! the `mcp` command calls [`ensure_repo_template`] once at startup to scaffold a commented,
//! behavior-preserving `<repo>/.codemap/config.toml` when none exists (for discoverability).
//! That writer never overwrites an existing file, never touches any git file, and warns
//! rather than crashing on failure. Keeping `.codemap/` out of `git status` is the user's
//! `.gitignore` choice.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Repo-local / global config directory name. Kept in [`crate::tools::EXCLUDED_DIRS`] so
/// the tool never walks (indexes) it.
pub const CODEMAP_DIR_NAME: &str = ".codemap";
/// Config file name, shared by the repo and global layers.
const CONFIG_FILE_NAME: &str = "config.toml";
/// Env var overriding the global config home (default `~/.codemap`). Tests set it for
/// hermetic isolation; users may set it to relocate global config.
const HOME_ENV: &str = "CODEMAP_HOME";

/// Commented, no-op scaffold written to a fresh repo on `mcp` start (see
/// [`ensure_repo_template`]). Every key is commented out at its compiled-in default, so the
/// file parses to an empty layer — zero keys, zero warnings — and reproduces the built-in
/// behavior exactly until the user uncomments a line. Mirrors the key reference in
/// `docs/configuration.md`; keep the two aligned when adding or renaming a key.
const CONFIG_TEMPLATE: &str = "\
# codemap-search repo-local config. Optional: with every key commented out (the state
# below), this file reproduces the built-in behavior exactly. Uncomment and edit a line to
# override its default. Resolution precedence is per key: repo > global > built-in default.

# Where the tantivy index lives (relative to the repo root).
# index_path = \".codemap/index\"

# `search` returns file details at or below this many matches, a codemap overview above.
# result_threshold = 5

# Files larger than this many bytes are skipped before parse/index (minified/generated blobs).
# max_file_size = 1048576   # 1 MiB

# Directory names to exclude, ADDED to the built-ins (node_modules, target, dist, build,
# vendor, .git, …). Built-ins can't be removed — this augments, it does not replace.
# excluded_directories = [\"__pycache__\", \".next\", \"coverage\"]

# Dedicated toggle for `.git/info/exclude` ONLY. Set false to let index/codemap/find/grep
# see files hidden solely by `.git/info/exclude` (e.g. local personal excludes) while
# `.gitignore`, the global gitignore, and `.codemapignore` stay honored.
# use_git_exclude = true

# Debounce window (milliseconds) for background index/codemap refreshes triggered by
# search/overview: within this window repeated calls enqueue at most one background refresh,
# and each call answers immediately from the committed snapshot. read/find/grep always read
# live disk, so brief search staleness is corrected by the follow-up read. (default 5000)
# index_staleness_ms = 5000

# Max file headers `search` emits in its codemap-overview branch (when matches exceed
# result_threshold). Caps the context a broad query can spend. (default 50)
# search_overview_file_limit = 50

# Filesystem watcher: when true (the default), file changes refresh the index in the
# background on their own and search/overview never trigger a tree walk. Set false to
# fall back to the request-triggered lazy refresh (debounced by index_staleness_ms).
# watch = true

# Debounce window (milliseconds) for watcher events: changes arriving within this window
# are batched into one incremental refresh. (default 500)
# watch_debounce_ms = 500

# Automatic recovery when the background indexer thread dies: the next search/overview
# rebuilds the index engine, respawns the indexer, and re-attaches the watcher (capped
# per server run so a deterministic crash cannot respawn-loop). Set false to serve
# results frozen at the last commit until the server is restarted instead.
# indexer_auto_restart = true

# Allow `find` absolute-path patterns whose static prefix resolves OUTSIDE the workspace
# root (Claude Code's Glob accepts any absolute base). Default false keeps the sandbox:
# absolute/`..` patterns escaping the root are rejected. Set true only to opt into
# searching arbitrary on-disk locations via `find`.
# allow_absolute_path_outside_root = false

# `grep` content-mode column cap: a matched line longer than this many columns is replaced
# with `[Omitted long matching line]` instead of being dumped in full (Claude Code passes
# `--max-columns 500`). 0 disables the cap. (default 500)
# grep_max_columns = 500

# `read` always-applied output ceiling (bytes): even with `offset`/`limit` set, a `read`
# whose rendered output would exceed this throws rather than emitting an unbounded blob
# (approximates Claude Code's ~25,000-token cap). Measured on the RENDERED output including
# the `     N→` line-number prefixes (~7 bytes/line). Separate from the 256 KiB whole-file
# cap that applies only when `limit` is omitted. (default 102400 ≈ 100 KiB)
# read_output_byte_cap = 102400

# `search` detail-view caps (the ≤ result_threshold branch that emits code snippets). These
# bound the detail response so a query matching a few large files can't dump tens of
# thousands of lines. Output-size only.
# search_detail_snippet_max_lines = 80   # per-symbol snippet line cap; over-long bodies elide
# search_detail_symbol_limit = 20        # max symbols rendered per file; rest summarized
# search_detail_byte_cap = 32768         # total detail-view byte budget (32 KiB) before cutoff
# search_literal_max_len = 200           # matched-literal truncation length (chars)
# search_literal_limit = 10              # max matched literals rendered per file

# Caller/callee context for the `search` detail view. When the `caller_context` request
# parameter is omitted, this key decides the default; an explicit parameter always wins.
# The feature annotates each matched `fn` symbol's snippet with its depth-1 callers and
# callees (name-match only, approximate). Default on; set false (or pass
# caller_context=false per call) to disable.
# caller_context_default = true

# Caps for the caller/callee annotation (all output-size / cost bounds, tunable).
# scan_cap = 500               # hit budget for the combined-regex walk, split across scanned names (floor 25/name)
# caller_list_cap = 5          # max callers (or non-call references) rendered per symbol
# callee_list_cap = 5          # max callees rendered per symbol
# annotation_sub_budget = 8192 # annotation byte budget WITHIN search_detail_byte_cap (not added on top)
# common_name_threshold = 2    # a name with ≥ this many fn defs gets its callers/callees labeled ambiguous
";

/// Fully-resolved configuration. Every field carries a compiled-in default that
/// reproduces the post-Child-04 behavior exactly when no config file is present.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Tantivy index location (default `.codemap/index`).
    pub index_path: String,
    /// `search` overview/detail branch threshold — at or below it `search` returns file
    /// details, above it a codemap overview (default 5).
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
    /// Max file headers `search` emits in its codemap-overview branch (default 50). Caps the
    /// context a broad query can spend; pairs with `result_threshold`, which picks the
    /// overview-vs-detail branch. Output-size only — safe to tune.
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
    /// Allow `find` absolute-path patterns whose static prefix resolves outside the
    /// workspace root (default false). When false, absolute/`..` patterns escaping the
    /// root are rejected as today; when true, `find` bypasses the within-root assertion
    /// so a Claude Code-style absolute glob can search anywhere on disk. The default
    /// preserves the sandbox; this is the opt-in escape hatch.
    pub allow_absolute_path_outside_root: bool,
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
    /// `search` detail-view total output byte budget (default 32768 ≈ 32 KiB). Once the
    /// rendered detail view reaches this, emission stops with a truncation note. Output-size
    /// only.
    pub search_detail_byte_cap: usize,
    /// `search` matched-literal truncation length in characters (default 200). A longer
    /// literal is cut with an ellipsis. Output-size only.
    pub search_literal_max_len: usize,
    /// `search` per-file matched-literal count cap (default 10). Output-size only.
    pub search_literal_limit: usize,
    /// Repo-level default for the caller/callee context (default true). The per-call
    /// `caller_context` parameter, when supplied, always overrides this; the key only
    /// decides the default when the parameter is omitted.
    pub caller_context_default: bool,
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
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            index_path: format!("{CODEMAP_DIR_NAME}/index"),
            result_threshold: 5,
            max_file_size: crate::tools::MAX_INDEXED_FILE_BYTES,
            excluded_directories: crate::tools::EXCLUDED_DIRS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            use_git_exclude: true,
            index_staleness_ms: 5_000,
            search_overview_file_limit: 50,
            watch: true,
            watch_debounce_ms: 500,
            indexer_auto_restart: true,
            allow_absolute_path_outside_root: false,
            grep_max_columns: 500,
            read_output_byte_cap: 102_400,
            search_detail_snippet_max_lines: 80,
            search_detail_symbol_limit: 20,
            search_detail_byte_cap: 32_768,
            search_literal_max_len: 200,
            search_literal_limit: 10,
            caller_context_default: true,
            scan_cap: 500,
            caller_list_cap: 5,
            callee_list_cap: 5,
            annotation_sub_budget: 8192,
            common_name_threshold: 2,
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
    allow_absolute_path_outside_root: Option<bool>,
    grep_max_columns: Option<usize>,
    read_output_byte_cap: Option<usize>,
    search_detail_snippet_max_lines: Option<usize>,
    search_detail_symbol_limit: Option<usize>,
    search_detail_byte_cap: Option<usize>,
    search_literal_max_len: Option<usize>,
    search_literal_limit: Option<usize>,
    caller_context_default: Option<bool>,
    scan_cap: Option<usize>,
    caller_list_cap: Option<usize>,
    callee_list_cap: Option<usize>,
    annotation_sub_budget: Option<usize>,
    common_name_threshold: Option<usize>,
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
    for (key, v) in table {
        match key.as_str() {
            "index_path" => layer.index_path = as_nonempty_string(&v, &key, path),
            "result_threshold" => layer.result_threshold = as_positive_usize(&v, &key, path),
            "max_file_size" => layer.max_file_size = as_positive_u64(&v, &key, path),
            "excluded_directories" => layer.excluded_directories = as_string_array(&v, &key, path),
            "use_git_exclude" => layer.use_git_exclude = as_bool(&v, &key, path),
            "index_staleness_ms" => layer.index_staleness_ms = as_positive_u64(&v, &key, path),
            "search_overview_file_limit" => {
                layer.search_overview_file_limit = as_positive_usize(&v, &key, path)
            }
            "watch" => layer.watch = as_bool(&v, &key, path),
            "watch_debounce_ms" => layer.watch_debounce_ms = as_positive_u64(&v, &key, path),
            "indexer_auto_restart" => layer.indexer_auto_restart = as_bool(&v, &key, path),
            "allow_absolute_path_outside_root" => {
                layer.allow_absolute_path_outside_root = as_bool(&v, &key, path)
            }
            "grep_max_columns" => layer.grep_max_columns = as_nonneg_usize(&v, &key, path),
            "read_output_byte_cap" => layer.read_output_byte_cap = as_positive_usize(&v, &key, path),
            "search_detail_snippet_max_lines" => {
                layer.search_detail_snippet_max_lines = as_positive_usize(&v, &key, path)
            }
            "search_detail_symbol_limit" => {
                layer.search_detail_symbol_limit = as_positive_usize(&v, &key, path)
            }
            "search_detail_byte_cap" => {
                layer.search_detail_byte_cap = as_positive_usize(&v, &key, path)
            }
            "search_literal_max_len" => {
                layer.search_literal_max_len = as_positive_usize(&v, &key, path)
            }
            "search_literal_limit" => {
                layer.search_literal_limit = as_positive_usize(&v, &key, path)
            }
            "caller_context_default" => {
                layer.caller_context_default = as_bool(&v, &key, path)
            }
            "scan_cap" => layer.scan_cap = as_positive_usize(&v, &key, path),
            "caller_list_cap" => layer.caller_list_cap = as_positive_usize(&v, &key, path),
            "callee_list_cap" => layer.callee_list_cap = as_positive_usize(&v, &key, path),
            "annotation_sub_budget" => {
                layer.annotation_sub_budget = as_positive_usize(&v, &key, path)
            }
            "common_name_threshold" => {
                layer.common_name_threshold = as_positive_usize(&v, &key, path)
            }
            other => warn(&format!(
                "unknown config key '{other}': {} — ignored",
                path.display()
            )),
        }
    }
    layer
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
        allow_absolute_path_outside_root: repo
            .allow_absolute_path_outside_root
            .or(global.allow_absolute_path_outside_root)
            .unwrap_or(defaults.allow_absolute_path_outside_root),
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
        caller_context_default: repo
            .caller_context_default
            .or(global.caller_context_default)
            .unwrap_or(defaults.caller_context_default),
        scan_cap: repo.scan_cap.or(global.scan_cap).unwrap_or(defaults.scan_cap),
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

/// Scaffold a commented, no-op `<repo_root>/.codemap/config.toml` from [`CONFIG_TEMPLATE`]
/// when none exists, so a fresh repo gets a discoverable, self-documenting config on `mcp`
/// start. Never-exit: an existing file is left untouched (protects user edits), and a
/// directory-create or write failure warns to stderr and returns rather than crashing the
/// server. The path matches exactly what [`load`] reads, so an uncommented key takes effect
/// on the next run.
pub fn ensure_repo_template(repo_root: &Path) {
    let dir = repo_root.join(CODEMAP_DIR_NAME);
    let path = dir.join(CONFIG_FILE_NAME);
    if path.exists() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn(&format!(
            "config template skipped: create {}: {e}",
            dir.display()
        ));
        return;
    }
    if let Err(e) = std::fs::write(&path, CONFIG_TEMPLATE) {
        warn(&format!(
            "config template skipped: write {}: {e}",
            path.display()
        ));
        return;
    }
    warn(&format!("created default config: {}", path.display()));
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
        assert_eq!(cfg.max_file_size, crate::tools::MAX_INDEXED_FILE_BYTES);
        assert!(cfg.use_git_exclude);
        assert!(cfg.excluded_directories.iter().any(|d| d == "node_modules"));
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
}
