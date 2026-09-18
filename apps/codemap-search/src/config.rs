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
//! `<repo>/.codemap/config.toml` exists it scaffolds an explicit-default file
//! (stamped with the current schema [`CONFIG_VERSION`]) for discoverability. When one
//! already exists it **incrementally syncs** it: for every key introduced since the file's
//! stamped version it appends that key's commented block (presence-guarded so an existing
//! key — set or commented — is never duplicated) and re-stamps the version marker. The sync
//! normally adds commented keys. The one-time v6 transition also materializes directory
//! exclusions; from v6 onward that array is never automatically changed. Sync never
//! rewrites a file already at the current version, never touches any git file, and
//! warns rather than crashing on failure. Keeping `.codemap/` out of `git status` is the
//! user's `.gitignore` choice.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config_locale::{config_comment_language, ConfigCommentLanguage};
use crate::workspace::exclusions::DirectoryExclusions;

mod event_navigation;
mod exclude;
pub use event_navigation::EventNavigationConfig;
mod macro_expansion;
pub(crate) mod redact;
pub use macro_expansion::MacroExpansionConfig;
pub use redact::RedactConfig;
mod scaffold;
mod test_code;
pub use test_code::TestCodeRules;

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

/// Current config-template schema version. Stamped in generated config templates and used by
/// [`ensure_repo_config`] to decide whether an existing file needs an incremental sync. Bump
/// this whenever the templates grow a key, and add the matching [`MIGRATIONS`] entry so
/// pre-existing repo files pick the key up (as a localized commented block) on their next `mcp`
/// start. Comment-only localization does not bump this version.
const CONFIG_VERSION: u32 = 16;
/// Version assumed for a file that carries no [`VERSION_MARKER_PREFIX`] line — i.e. a file
/// written before versioning existed. Such a file is run through every [`MIGRATIONS`] entry
/// (each presence-guarded) so it converges to the current schema without duplicating any key
/// it already holds. It stays the lowest version forever, so the floor never drifts.
const CONFIG_BASELINE_VERSION: u32 = 1;
/// Leading text of the comment line that stamps a config file's schema version, e.g.
/// `# codemap-config-version: 1`. Deliberately a comment, not a TOML key: the loader and
/// [`normalize`] never see it, and the migrator reads it with a plain string scan since the
/// TOML parser drops comments.
const VERSION_MARKER_PREFIX: &str = "# codemap-config-version:";

/// English scaffold written to a fresh repo on `mcp` start (see
/// [`ensure_repo_config`]). The first line is the [`VERSION_MARKER_PREFIX`] schema marker,
/// and every key is
/// live at its default except project-discovered directory exclusions; repo values override a
/// global config until the user deletes or comments out a key. Mirrors the key reference in
/// `docs/configuration.md`; keep the two aligned when adding or renaming a key. When adding a
/// key, update every config template, bump [`CONFIG_VERSION`], and add localized commented
/// [`MIGRATIONS`] entries so existing repo files pick it up incrementally without behavior
/// changes.
const CONFIG_TEMPLATE: &str = include_str!("config_template.toml");
const CONFIG_TEMPLATE_KO: &str = include_str!("config_template.ko.toml");

fn config_template(language: ConfigCommentLanguage) -> &'static str {
    language.select(CONFIG_TEMPLATE, CONFIG_TEMPLATE_KO)
}

/// Fully-resolved configuration. Every field carries a compiled-in default that
/// reproduces the post-Child-04 behavior exactly when no config file is present.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Mask detected credentials in MCP output; source files and indexes stay unchanged.
    pub is_redact_enabled: bool,
    pub redact: RedactConfig,
    pub macro_expansion: MacroExpansionConfig,
    pub event_navigation: EventNavigationConfig,
    /// Explicit Rust analysis target; never inferred from the running host.
    pub analysis_target_os: Option<String>,
    /// Whether `mcp` may create/sync the repo-local `.codemap/config.toml` file.
    pub config_auto_update: bool,
    /// Tantivy index location (default `.codemap/index`).
    pub index_path: String,
    /// Number of top-ranked files `search` renders as details before remaining matches
    /// become compact ranked-tail rows (default 24).
    pub result_threshold: usize,
    /// Files larger than this (bytes) are skipped before read/parse (default 1 MiB).
    pub max_file_size: u64,
    /// Complete optional directory rules. Explicit arrays replace lower layers/defaults;
    /// VCS and index internals remain mandatory exclusions.
    pub excluded_directories: Vec<String>,
    pub(crate) directory_exclusions: DirectoryExclusions,
    pub(crate) index_root: PathBuf,
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
    /// Max file headers `search` emits in the compact ranked tail (default 80). Caps
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
    /// Whether Markdown documents (`.md`, `.mdx`) participate in index-backed discovery.
    /// Direct `read`/`find`/`grep`/`parse` remain available.
    pub is_document_support_enabled: bool,
    /// Whether shell scripts (`.sh`, `.bash`, `.zsh`) participate in index-backed discovery.
    pub is_shell_support_enabled: bool,
    /// Whether infrastructure definitions (`.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix`)
    /// participate in index-backed discovery.
    pub is_infrastructure_support_enabled: bool,
    /// Whether interface definitions (`.proto`, `.graphql`, `.gql`) participate in indexing and
    /// index-backed discovery.
    pub is_interface_support_enabled: bool,
    /// Whether build definitions (`Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`,
    /// `BUILD.bazel`, `.bzl`) participate in index-backed discovery.
    pub is_build_support_enabled: bool,
    /// Filesystem permissions for live disk tools (`find`, `grep`, `read`). Defaults keep
    /// every tool workspace-confined unless configured otherwise.
    pub filesystem_permissions: FilesystemPermissions,
    /// `grep` content-mode column cap (default 0, unlimited). A matched line wider
    /// than a positive cap is replaced with `[Omitted long matching line]`.
    pub grep_max_columns: usize,
    /// `read` and callable-expanded `grep` output ceiling in bytes (default 5 MiB).
    /// Oversized reads return a narrowing error; expanded grep returns bounded pages.
    /// Distinct from the 256 KiB whole-file read cap when `limit` is omitted.
    pub read_output_byte_cap: usize,
    /// `search` detail-view per-symbol snippet line cap (default 500). A symbol body longer
    /// than this is truncated with an elision marker. Output-size only.
    pub search_detail_snippet_max_lines: usize,
    /// `search` detail-view per-file symbol cap (default 100). Beyond it, a "more symbols
    /// not shown" note replaces the remaining symbols. Output-size only.
    pub search_detail_symbol_limit: usize,
    /// Hard byte ceiling for one `search` response (default 1 MiB), including
    /// detail files, ranked tail, and the partial-output footer. Output-size only.
    pub search_detail_byte_cap: usize,
    /// `search` matched-literal truncation length in characters (default 1200). A longer
    /// literal is cut with an ellipsis. Output-size only.
    pub search_literal_max_len: usize,
    /// `search` per-file matched-literal count cap (default 60). Output-size only.
    pub search_literal_limit: usize,
    /// `search` detail-view per-file anchor full-snippet cap (default 20). At most this many
    /// anchor symbols in one detail file render a FULL snippet; anchors ranked beyond the cap
    /// degrade to a ≤3-line signature (the Tier-2 abbreviation), not a one-line stub. A file
    /// whose anchor count is at or below the cap is unaffected. Output-size only — it bounds
    /// the snippet flood a broad query with a common name (`save`/`send`) can trigger.
    pub search_anchor_snippet_limit: usize,
    /// Repo-level default for the caller/callee context (default true). The per-call
    /// `caller_context` parameter, when supplied, always overrides this; the key only
    /// decides the default when the parameter is omitted.
    pub caller_context_default: bool,
    /// Include test regions in automatic symbol and caller/callee context.
    pub should_include_test_code: bool,
    pub test_code_rules: TestCodeRules,
    /// Repo-level default for navigation-based precise attribution (default false). Caller
    /// context still renders via the existing name-match fallback when this is off.
    pub navigation_context_default: bool,
    /// Max navigation call sites resolved in one annotation pass before falling back
    /// (default 1000).
    pub navigation_callsite_budget: usize,
    /// Whether extraction stores reference sites in `NavigationFile` (default false).
    pub navigation_store_references: bool,
    /// Max call sites collected across the single combined-regex caller scan (default 16000).
    /// Shared by all matched names in one scan; reaching it marks the caller list truncated.
    pub scan_cap: usize,
    /// Per-symbol caller-list (or non-call-reference) cap (default 1000). Output-size only.
    pub caller_list_cap: usize,
    /// Per-symbol callee-list cap (default 1000). Output-size only.
    pub callee_list_cap: usize,
    /// Annotation byte sub-budget WITHIN `search_detail_byte_cap` (default 128 KiB). A
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

impl ResolvedConfig {
    fn language_support_settings(&self) -> [bool; 5] {
        [
            self.is_document_support_enabled,
            self.is_shell_support_enabled,
            self.is_infrastructure_support_enabled,
            self.is_interface_support_enabled,
            self.is_build_support_enabled,
        ]
    }
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            is_redact_enabled: true,
            redact: RedactConfig::default(),
            macro_expansion: MacroExpansionConfig::default(),
            event_navigation: EventNavigationConfig::default(),
            analysis_target_os: None,
            config_auto_update: true,
            index_path: format!("{CODEMAP_DIR_NAME}/index"),
            index_root: PathBuf::from(format!("{CODEMAP_DIR_NAME}/index")),
            result_threshold: 24,
            max_file_size: crate::workspace::MAX_INDEXED_FILE_BYTES,
            excluded_directories: crate::workspace::EXCLUDED_DIRS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            directory_exclusions: DirectoryExclusions::new(
                &crate::workspace::EXCLUDED_DIRS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            )
            .expect("built-in directory patterns are valid"),
            use_git_exclude: true,
            index_staleness_ms: 5_000,
            search_overview_file_limit: 80,
            watch: true,
            watch_debounce_ms: 500,
            indexer_auto_restart: true,
            is_document_support_enabled: false,
            is_shell_support_enabled: false,
            is_infrastructure_support_enabled: false,
            is_interface_support_enabled: false,
            is_build_support_enabled: false,
            filesystem_permissions: FilesystemPermissions::default(),
            grep_max_columns: 0,
            read_output_byte_cap: 5 * 1024 * 1024,
            search_detail_snippet_max_lines: 500,
            search_detail_symbol_limit: 100,
            search_detail_byte_cap: 1024 * 1024,
            search_literal_max_len: 1200,
            search_literal_limit: 60,
            search_anchor_snippet_limit: 20,
            caller_context_default: true,
            should_include_test_code: false,
            test_code_rules: TestCodeRules::default(),
            navigation_context_default: false,
            navigation_callsite_budget: 1000,
            navigation_store_references: false,
            scan_cap: 16_000,
            caller_list_cap: 1000,
            callee_list_cap: 1000,
            annotation_sub_budget: 128 * 1024,
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
    is_redact_enabled: Option<bool>,
    redact: redact::RedactLayer,
    macro_expansion: macro_expansion::MacroExpansionLayer,
    event_navigation: event_navigation::EventNavigationLayer,
    analysis_target_os: Option<Option<String>>,
    config_auto_update: Option<bool>,
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
    is_document_support_enabled: Option<bool>,
    is_shell_support_enabled: Option<bool>,
    is_infrastructure_support_enabled: Option<bool>,
    is_interface_support_enabled: Option<bool>,
    is_build_support_enabled: Option<bool>,
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
    should_include_test_code: Option<bool>,
    test_code_rules: test_code::TestCodeLayer,
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
    let mut resolved = merge(repo_layer, global_layer);
    resolved.index_root =
        crate::workspace::canonicalize_path_lenient(&repo_root.join(&resolved.index_path));
    resolved
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
    let mut exclude_value = None;
    for (key, value) in table {
        match key.as_str() {
            "event_navigation" => {
                layer.event_navigation = event_navigation::normalize(&value, path)
            }
            "redact" => layer.redact = redact::normalize(&value, path),
            "macro_expansion" => layer.macro_expansion = macro_expansion::normalize(&value, path),
            "exclude" => exclude_value = Some(value),
            "update" | "index" | "refresh" | "search" | "tool_output" | "caller_context"
            | "language_support" | "analysis" => {
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
    if let Some(value) = exclude_value {
        exclude::normalize_section(&mut layer, &value, path);
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
        "analysis" => key == "target_os",
        "update" => matches!(key, "config_auto_update"),
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
        "tool_output" => matches!(
            key,
            "grep_max_columns" | "read_output_byte_cap" | "is_redact_enabled"
        ),
        "exclude" => exclude::TEST_KEYS.contains(&key) || exclude::WORKSPACE_KEYS.contains(&key),
        "caller_context" => matches!(
            key,
            "caller_context_default"
                | "should_include_test_code"
                | "test_file_patterns"
                | "test_attributes"
                | "test_decorators"
                | "test_calls"
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
        "language_support" => matches!(
            key,
            "is_document_support_enabled"
                | "is_shell_support_enabled"
                | "is_infrastructure_support_enabled"
                | "is_interface_support_enabled"
                | "is_build_support_enabled"
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
        "target_os" => {
            layer.analysis_target_os = match value.as_str() {
                Some("") => Some(None),
                Some(value)
                    if value
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') =>
                {
                    Some(Some(value.to_string()))
                }
                _ => {
                    warn(&format!("config '{key_display}' must be an OS identifier or empty string: {} — ignored", path.display()));
                    None
                }
            };
        }
        "is_redact_enabled" => layer.is_redact_enabled = as_bool(value, key_display, path),
        "config_auto_update" => layer.config_auto_update = as_bool(value, key_display, path),
        "index_path" => layer.index_path = as_nonempty_string(value, key_display, path),
        "result_threshold" => layer.result_threshold = as_positive_usize(value, key_display, path),
        "max_file_size" => layer.max_file_size = as_positive_byte_size(value, key_display, path),
        "excluded_directories" => {
            layer.excluded_directories = as_directory_patterns(value, key_display, path)
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
        "is_document_support_enabled" => {
            layer.is_document_support_enabled = as_bool(value, key_display, path)
        }
        "is_shell_support_enabled" => {
            layer.is_shell_support_enabled = as_bool(value, key_display, path)
        }
        "is_infrastructure_support_enabled" => {
            layer.is_infrastructure_support_enabled = as_bool(value, key_display, path)
        }
        "is_interface_support_enabled" => {
            layer.is_interface_support_enabled = as_bool(value, key_display, path)
        }
        "is_build_support_enabled" => {
            layer.is_build_support_enabled = as_bool(value, key_display, path)
        }
        "grep_max_columns" => layer.grep_max_columns = as_nonneg_usize(value, key_display, path),
        "read_output_byte_cap" => {
            layer.read_output_byte_cap = as_positive_byte_size(value, key_display, path)
        }
        "search_detail_snippet_max_lines" => {
            layer.search_detail_snippet_max_lines = as_positive_usize(value, key_display, path)
        }
        "search_detail_symbol_limit" => {
            layer.search_detail_symbol_limit = as_positive_usize(value, key_display, path)
        }
        "search_detail_byte_cap" => {
            layer.search_detail_byte_cap = as_positive_byte_size(value, key_display, path)
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
        "should_include_test_code" => {
            layer.should_include_test_code = as_bool(value, key_display, path)
        }
        "test_file_patterns" => {
            layer.test_code_rules.file_patterns =
                test_code::parse_patterns(value, key_display, path, true)
        }
        "test_attributes" => {
            layer.test_code_rules.attributes = test_code::parse_languages(value, key_display, path)
        }
        "test_decorators" => {
            layer.test_code_rules.decorators = test_code::parse_languages(value, key_display, path)
        }
        "test_calls" => {
            layer.test_code_rules.calls = test_code::parse_languages(value, key_display, path)
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
            layer.annotation_sub_budget = as_positive_byte_size(value, key_display, path)
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

/// Per-key `repo > global > default` merge, including complete directory arrays.
fn merge(repo: ConfigLayer, global: ConfigLayer) -> ResolvedConfig {
    let defaults = ResolvedConfig::default();
    let excluded_directories = repo
        .excluded_directories
        .or(global.excluded_directories)
        .unwrap_or(defaults.excluded_directories);
    let directory_exclusions = DirectoryExclusions::new(&excluded_directories)
        .expect("directory patterns were validated during config normalization");
    ResolvedConfig {
        is_redact_enabled: repo
            .is_redact_enabled
            .or(global.is_redact_enabled)
            .unwrap_or(defaults.is_redact_enabled),
        redact: redact::merge(repo.redact, global.redact),
        macro_expansion: macro_expansion::merge(repo.macro_expansion, global.macro_expansion),
        event_navigation: event_navigation::merge(repo.event_navigation, global.event_navigation),
        analysis_target_os: repo
            .analysis_target_os
            .or(global.analysis_target_os)
            .flatten(),
        config_auto_update: repo
            .config_auto_update
            .or(global.config_auto_update)
            .unwrap_or(defaults.config_auto_update),
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
        directory_exclusions,
        index_root: defaults.index_root,
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
        is_document_support_enabled: repo
            .is_document_support_enabled
            .or(global.is_document_support_enabled)
            .unwrap_or(defaults.is_document_support_enabled),
        is_shell_support_enabled: repo
            .is_shell_support_enabled
            .or(global.is_shell_support_enabled)
            .unwrap_or(defaults.is_shell_support_enabled),
        is_infrastructure_support_enabled: repo
            .is_infrastructure_support_enabled
            .or(global.is_infrastructure_support_enabled)
            .unwrap_or(defaults.is_infrastructure_support_enabled),
        is_interface_support_enabled: repo
            .is_interface_support_enabled
            .or(global.is_interface_support_enabled)
            .unwrap_or(defaults.is_interface_support_enabled),
        is_build_support_enabled: repo
            .is_build_support_enabled
            .or(global.is_build_support_enabled)
            .unwrap_or(defaults.is_build_support_enabled),
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
        should_include_test_code: repo
            .should_include_test_code
            .or(global.should_include_test_code)
            .unwrap_or(defaults.should_include_test_code),
        test_code_rules: test_code::merge_test_code_rules(
            repo.test_code_rules,
            global.test_code_rules,
        ),
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
/// Call at startup, before any [`get`].
pub fn init(repo_root: &Path) {
    reload(repo_root);
}

/// The resolved in-memory config snapshot. This never reads disk; [`reload`] and the
/// config watcher replace the snapshot only after `config.toml` changes.
pub fn get() -> Arc<ResolvedConfig> {
    if let Some(config) = REQUEST_CONFIG.with(|slot| slot.borrow().clone()) {
        return config;
    }
    match config_lock().read() {
        Ok(config) => Arc::clone(&config),
        Err(poisoned) => {
            warn("config lock poisoned while reading — using latest in-memory value");
            Arc::clone(&poisoned.into_inner())
        }
    }
}

thread_local! {
    static REQUEST_CONFIG: std::cell::RefCell<Option<Arc<ResolvedConfig>>> = const { std::cell::RefCell::new(None) };
}

/// A synchronous MCP request uses one config generation across all consumers.
/// Reload/indexer threads remain independent; the next request sees their update.
pub(crate) struct RequestConfigScope {
    previous: Option<Arc<ResolvedConfig>>,
    same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

pub(crate) fn pin_request() -> RequestConfigScope {
    let config = get();
    RequestConfigScope {
        previous: REQUEST_CONFIG.with(|slot| slot.replace(Some(config))),
        same_thread: std::marker::PhantomData,
    }
}

#[cfg(test)]
pub(crate) fn pin_test_config(config: ResolvedConfig) -> RequestConfigScope {
    RequestConfigScope {
        previous: REQUEST_CONFIG.with(|slot| slot.replace(Some(Arc::new(config)))),
        same_thread: std::marker::PhantomData,
    }
}

impl Drop for RequestConfigScope {
    fn drop(&mut self) {
        REQUEST_CONFIG.with(|slot| slot.replace(self.previous.take()));
    }
}

/// Re-read repo/global config and replace the process-wide in-memory snapshot.
pub fn reload(repo_root: &Path) {
    let global = global_dir();
    reload_from_paths(repo_root, &global);
}

fn reload_from_paths(repo_root: &Path, global: &Path) {
    let resolved = load(repo_root, global);
    match config_lock().write() {
        Ok(mut config) => *config = Arc::new(resolved),
        Err(poisoned) => {
            warn("config lock poisoned while reloading — replacing latest in-memory value");
            *poisoned.into_inner() = Arc::new(resolved);
        }
    }
}

fn config_lock() -> &'static RwLock<Arc<ResolvedConfig>> {
    CONFIG.get_or_init(|| RwLock::new(Arc::new(ResolvedConfig::default())))
}

static CONFIG: OnceLock<RwLock<Arc<ResolvedConfig>>> = OnceLock::new();

const CONFIG_WATCH_DEBOUNCE_MS: u64 = 1_000;

/// Owner of the config file watcher. Dropping it stops notify events, then joins the
/// debounce thread before MCP shutdown continues.
pub struct ConfigWatcherHandle {
    watcher: Option<RecommendedWatcher>,
    join_handle: Option<JoinHandle<()>>,
}

impl Drop for ConfigWatcherHandle {
    fn drop(&mut self) {
        drop(self.watcher.take());
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Watch repo/global `config.toml` files and refresh the in-memory config after a fixed
/// one-second debounce window. This is independent of the index watcher and runs even when
/// `[refresh].watch` is disabled.
pub fn spawn_config_watcher(
    repo_root: &Path,
    index_command_sender: SyncSender<crate::index::IndexCommand>,
) -> Option<ConfigWatcherHandle> {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let global = global_dir();
    let config_paths = watched_config_paths(&repo_root, &global);
    tracing::debug!(?config_paths, "starting config watcher");
    let watch_dirs = config_paths
        .iter()
        .filter_map(|path| {
            path.parent()
                .map(crate::workspace::canonicalize_path_lenient)
        })
        .filter(|dir| dir.is_dir())
        .collect::<BTreeSet<_>>();
    if watch_dirs.is_empty() {
        tracing::warn!("config watcher skipped: no config directories exist");
        return None;
    }

    let (event_sender, event_receiver) = channel();
    let mut watcher = match notify::recommended_watcher(event_sender) {
        Ok(watcher) => watcher,
        Err(e) => {
            tracing::warn!("config watcher creation failed: {e}");
            return None;
        }
    };
    let mut registered_watch_count = 0;
    for dir in &watch_dirs {
        if let Err(e) = watcher.watch(dir, RecursiveMode::NonRecursive) {
            tracing::warn!(
                "config watch registration failed for {}: {e}",
                dir.display()
            );
        } else {
            registered_watch_count += 1;
        }
    }
    if registered_watch_count == 0 {
        tracing::warn!("config watcher skipped: no config directories could be watched");
        return None;
    }

    let debounce = Duration::from_millis(CONFIG_WATCH_DEBOUNCE_MS);
    let join_handle = match std::thread::Builder::new()
        .name("codemap-config-watcher".to_string())
        .spawn(move || {
            run_config_watch_loop(
                event_receiver,
                repo_root,
                global,
                config_paths,
                debounce,
                index_command_sender,
            )
        }) {
        Ok(handle) => handle,
        Err(e) => {
            tracing::warn!("config watcher thread spawn failed: {e}");
            return None;
        }
    };

    Some(ConfigWatcherHandle {
        watcher: Some(watcher),
        join_handle: Some(join_handle),
    })
}

fn watched_config_paths(repo_root: &Path, global: &Path) -> BTreeSet<PathBuf> {
    [
        repo_root.join(CODEMAP_DIR_NAME).join(CONFIG_FILE_NAME),
        global.join(CONFIG_FILE_NAME),
    ]
    .into_iter()
    .map(|path| crate::workspace::canonicalize_path_lenient(&path))
    .collect()
}

fn run_config_watch_loop(
    events: Receiver<Result<notify::Event, notify::Error>>,
    repo_root: PathBuf,
    global: PathBuf,
    config_paths: BTreeSet<PathBuf>,
    debounce: Duration,
    index_command_sender: SyncSender<crate::index::IndexCommand>,
) {
    loop {
        let first = match events.recv() {
            Ok(event) => event,
            Err(_) => return,
        };
        let mut should_reload = is_config_event(first, &config_paths);
        let deadline = Instant::now() + debounce;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match events.recv_timeout(remaining) {
                Ok(event) => should_reload |= is_config_event(event, &config_paths),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        if should_reload {
            let previous = get();
            reload_from_paths(&repo_root, &global);
            tracing::debug!(excluded_directories = ?get().excluded_directories, "reloaded config");
            request_refresh_if_index_scope_changed(&previous, &get(), &index_command_sender);
        }
    }
}

fn request_refresh_if_index_scope_changed(
    previous: &ResolvedConfig,
    current: &ResolvedConfig,
    index_command_sender: &SyncSender<crate::index::IndexCommand>,
) {
    if previous.language_support_settings() == current.language_support_settings()
        && previous.excluded_directories == current.excluded_directories
        && previous.use_git_exclude == current.use_git_exclude
        && previous.macro_expansion == current.macro_expansion
        && previous.event_navigation == current.event_navigation
        && previous.analysis_target_os == current.analysis_target_os
        && previous.should_include_test_code == current.should_include_test_code
        && previous.test_code_rules == current.test_code_rules
    {
        return;
    }
    // This is the dedicated config thread, not the request loop. Waiting for capacity
    // ensures a queued RefreshPaths cannot swallow the required full reconciliation.
    if index_command_sender
        .send(crate::index::IndexCommand::Refresh)
        .is_err()
    {
        tracing::warn!("index scope changed, but the indexer is unavailable; search results remain stale until recovery");
    }
}

fn is_config_event(
    event: Result<notify::Event, notify::Error>,
    config_paths: &BTreeSet<PathBuf>,
) -> bool {
    let event = match event {
        Ok(event) => event,
        Err(e) => {
            tracing::warn!("config watcher backend error: {e} — reloading config");
            return true;
        }
    };
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    tracing::debug!(kind = ?event.kind, paths = ?event.paths, "config watcher event");
    event.paths.iter().any(|path| {
        let path = crate::workspace::canonicalize_path_lenient(path);
        config_paths.contains(&path)
    })
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
    /// Section-aware insertion point for the migration block.
    placement: KeyPlacement,
    /// The English commented migration block for the key (doc comment line(s) then a
    /// `# key = default` line, no surrounding blank lines — the inserter spaces it).
    english_block: &'static str,
    /// The Korean commented migration block for the same key. TOML section, key, and value text
    /// must match the English block; only comment prose may differ.
    korean_block: &'static str,
}

impl Migration {
    fn block(&self, language: ConfigCommentLanguage) -> &'static str {
        language.select(self.english_block, self.korean_block)
    }
}

/// Ordered, additive migrations. v1 is the baseline; later entries add commented blocks for
/// keys introduced after that baseline. To introduce a key in a later release: add its commented
/// block to each config template at its logical position, bump [`CONFIG_VERSION`] to N, then
/// append a localized `Migration` entry here.
///
/// Existing repo files then gain the key (commented, before the first table header) and a
/// refreshed version marker on their next `mcp` start, with their own edits untouched.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 16,
        key: "pii_entities",
        placement: KeyPlacement::Subtable("redact"),
        english_block: "# Opt-in PII entity types, e.g. [\"CREDIT_CARD\", \"EMAIL_ADDRESS\"]. Empty keeps credential-only masking.\n# pii_entities = []",
        korean_block: "# 선택 활성화할 PII 종류입니다. 예: [\"CREDIT_CARD\", \"EMAIL_ADDRESS\"]. 비어 있으면 인증정보만 가립니다.\n# pii_entities = []",
    },
    Migration {
        version: 15,
        key: "exceptions",
        placement: KeyPlacement::Subtable("redact"),
        english_block: "# Exact rule_id + detected value exceptions; no path-wide bypass.\n# exceptions = []",
        korean_block: "# 규칙 식별자와 탐지 값이 모두 정확히 일치하는 예외입니다. 경로 전체를 제외하지 않습니다.\n# exceptions = []",
    },
    Migration {
        version: 15,
        key: "rules",
        placement: KeyPlacement::Subtable("redact"),
        english_block: "# Additional regex rules with custom.* IDs. See docs/configuration.md.\n# rules = []",
        korean_block: "# custom.* 식별자를 가진 추가 정규식 규칙입니다. docs/configuration.ko.md를 참고하세요.\n# rules = []",
    },
    Migration {
        version: 15,
        key: "sensitive_fields",
        placement: KeyPlacement::Subtable("redact"),
        english_block: "# Additional sensitive field names; normalized exact matches.\n# sensitive_fields = []",
        korean_block: "# 추가 민감 필드명입니다. 정규화한 이름이 정확히 일치할 때 적용합니다.\n# sensitive_fields = []",
    },
    Migration {
        version: 14,
        key: "is_redact_enabled",
        placement: KeyPlacement::Subtable("tool_output"),
        english_block: "# Mask detected credentials in MCP output. Files, indexes and matching keep original data.\n# is_redact_enabled = true",
        korean_block: "# MCP 응답에서 탐지된 인증정보를 가립니다. 파일·인덱스·검색 일치는 원문을 유지합니다.\n# is_redact_enabled = true",
    },
    Migration {
        version: 13,
        key: "use_builtin_rules",
        placement: KeyPlacement::Subtable("event_navigation"),
        english_block: "# Event analysis and relevant navigation output are enabled by default.\n# Set is_enabled to false only to disable event analysis; explicit values are preserved.\n# use_builtin_rules = true",
        korean_block: "# 이벤트 분석과 관련 탐색 결과를 기본으로 제공합니다.\n# 이벤트 분석을 끌 때만 is_enabled를 false로 설정하세요. 명시한 값은 유지됩니다.\n# use_builtin_rules = true",
    },
    Migration {
        version: 12,
        key: "is_enabled",
        placement: KeyPlacement::Subtable("event_navigation"),
        english_block: "# Indexed event routes are enabled by default. See docs/configuration.md for API rules and limits.\n# is_enabled = true",
        korean_block: "# 이벤트 관계 색인은 기본으로 켜져 있습니다. API 규칙과 한계는 docs/configuration.ko.md를 참고하세요.\n# is_enabled = true",
    },
    Migration {
        version: 11,
        key: "target_os",
        placement: KeyPlacement::Subtable("analysis"),
        english_block: "# Explicit Rust target OS. Empty means unknown; never uses the host OS.\n# target_os = \"\"",
        korean_block: "# Rust 분석 대상 OS. 빈 값은 미지정이며 실행 컴퓨터의 OS를 추정하지 않습니다.\n# target_os = \"\"",
    },
    Migration {
        version: 10,
        key: "is_enabled",
        placement: KeyPlacement::Subtable("macro_expansion"),
        english_block: "# Clang/NASM preprocessing is enabled by default for native files. See docs/configuration.md.\n# is_enabled = true",
        korean_block: "# C/C++/ASM 파일의 Clang/NASM 전처리는 기본으로 켜집니다. docs/configuration.ko.md를 참고하세요.\n# is_enabled = true",
    },
    Migration {
        version: 2,
        key: "navigation_store_references",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# navigation_store_references = false",
        korean_block: "# navigation_store_references = false",
    },
    Migration {
        version: 2,
        key: "navigation_callsite_budget",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# navigation_callsite_budget = 1000",
        korean_block: "# navigation_callsite_budget = 1000",
    },
    Migration {
        version: 2,
        key: "navigation_context_default",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# Precise caller/callee attribution. When false, annotations use conservative name matching.\n# Set true to allow tree-sitter navigation data to mark unambiguous lines as precise.\n# navigation_context_default = false",
        korean_block: "# 호출자/호출 대상 귀속을 정밀하게 표시합니다. false이면 주석은 보수적인 이름 매칭을 사용합니다.\n# tree-sitter navigation 데이터가 모호하지 않은 줄을 precise로 표시할 수 있게 하려면 true로 설정하세요.\n# navigation_context_default = false",
    },
    Migration {
        version: 3,
        key: "config_auto_update",
        placement: KeyPlacement::TopLevel,
        english_block: "# Automatic repo config file creation and schema sync on `mcp` startup.\n# true: create missing `.codemap/config.toml` and append commented blocks for new settings.\n# false: never writes `.codemap/config.toml` automatically; existing config is still read.\n# Existing-file schema sync adds new settings as commented blocks, not active values.\n# [update]\n# config_auto_update = true",
        korean_block: "# `mcp` 시작 시 저장소 설정 파일 생성과 스키마 동기화를 자동으로 수행합니다.\n# true: 누락된 `.codemap/config.toml`을 만들고 새 설정의 주석 블록을 추가합니다.\n# false: `.codemap/config.toml`을 자동으로 쓰지 않습니다. 기존 설정은 계속 읽습니다.\n# 기존 파일의 스키마 동기화는 새 설정을 활성 값이 아닌 주석 블록으로 추가합니다.\n# [update]\n# config_auto_update = true",
    },
    Migration {
        version: 4,
        key: "is_document_support_enabled",
        placement: KeyPlacement::TopLevel,
        english_block: "# Include Markdown documents (`.md`, `.mdx`) in index-backed discovery.\n# false excludes them from search/overview/codemap; direct read/parse/find/grep remain available.\n# [language_support]\n# is_document_support_enabled = false",
        korean_block: "# Markdown 문서(`.md`, `.mdx`)를 색인 기반 탐색에 포함합니다.\n# false이면 search/overview/codemap에서 제외하지만 직접 read/parse/find/grep은 유지됩니다.\n# [language_support]\n# is_document_support_enabled = false",
    },
    // Insert in reverse display order because every subtable migration is placed immediately
    // after the same header. The resulting config reads shell → infrastructure → interface → build.
    Migration {
        version: 5,
        key: "is_build_support_enabled",
        placement: KeyPlacement::Subtable("language_support"),
        english_block: "# Build definitions\n# Includes Makefile and `.mk`; CMakeLists.txt and `.cmake`; BUILD, BUILD.bazel, and `.bzl`.\n# Direct read/parse/find/grep remain available when false.\n# is_build_support_enabled = false",
        korean_block: "# 빌드 정의\n# Makefile과 `.mk`, CMakeLists.txt와 `.cmake`, BUILD·BUILD.bazel과 `.bzl`을 포함합니다.\n# false여도 직접 read/parse/find/grep은 계속 허용합니다.\n# is_build_support_enabled = false",
    },
    Migration {
        version: 5,
        key: "is_interface_support_enabled",
        placement: KeyPlacement::Subtable("language_support"),
        english_block: "# Interface definitions\n# Includes Protocol Buffers (`.proto`) and GraphQL (`.graphql`, `.gql`).\n# Direct read/parse/find/grep remain available when false.\n# is_interface_support_enabled = false",
        korean_block: "# 인터페이스 정의\n# Protocol Buffers(`.proto`)와 GraphQL(`.graphql`, `.gql`)을 포함합니다.\n# false여도 직접 read/parse/find/grep은 계속 허용합니다.\n# is_interface_support_enabled = false",
    },
    Migration {
        version: 5,
        key: "is_infrastructure_support_enabled",
        placement: KeyPlacement::Subtable("language_support"),
        english_block: "# Infrastructure definitions\n# Includes HCL/Terraform (`.hcl`, `.tf`, `.tfvars`), Dockerfile, and Nix (`.nix`).\n# Direct read/parse/find/grep remain available when false.\n# is_infrastructure_support_enabled = false",
        korean_block: "# 인프라 정의\n# HCL/Terraform(`.hcl`, `.tf`, `.tfvars`), Dockerfile, Nix(`.nix`)를 포함합니다.\n# false여도 직접 read/parse/find/grep은 계속 허용합니다.\n# is_infrastructure_support_enabled = false",
    },
    Migration {
        version: 5,
        key: "is_shell_support_enabled",
        placement: KeyPlacement::Subtable("language_support"),
        english_block: "# Shell scripts\n# Includes `.sh`, `.bash`, and `.zsh`.\n# Direct read/parse/find/grep remain available when false.\n# is_shell_support_enabled = false",
        korean_block: "# 셸 스크립트\n# `.sh`, `.bash`, `.zsh`를 포함합니다.\n# false여도 직접 read/parse/find/grep은 계속 허용합니다.\n# is_shell_support_enabled = false",
    },
    Migration {
        version: 7,
        key: "test_calls",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# Per-language test rules: replace a list to customize it; [] disables it.\n# test_calls = { javascript = [\"describe\", \"describe.*\", \"it\", \"it.*\", \"test\", \"test.*\", \"suite\", \"suite.*\"], typescript = [\"describe\", \"describe.*\", \"it\", \"it.*\", \"test\", \"test.*\", \"suite\", \"suite.*\"], dart = [\"test\", \"group\", \"testWidgets\"], ruby = [\"describe\", \"context\", \"it\", \"specify\"], powershell = [\"Describe\", \"Context\", \"It\"] }",
        korean_block: "# 언어별 테스트 규칙: 목록을 바꾸면 사용자 설정으로 대체하고 []로 비활성화합니다.\n# test_calls = { javascript = [\"describe\", \"describe.*\", \"it\", \"it.*\", \"test\", \"test.*\", \"suite\", \"suite.*\"], typescript = [\"describe\", \"describe.*\", \"it\", \"it.*\", \"test\", \"test.*\", \"suite\", \"suite.*\"], dart = [\"test\", \"group\", \"testWidgets\"], ruby = [\"describe\", \"context\", \"it\", \"specify\"], powershell = [\"Describe\", \"Context\", \"It\"] }",
    },
    Migration {
        version: 7,
        key: "test_decorators",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# Per-language test rules: replace a list to customize it; [] disables it.\n# test_decorators = { python = [\"pytest.fixture\", \"pytest.mark.*\", \"unittest.skip\", \"unittest.skipIf\", \"unittest.skipUnless\", \"unittest.expectedFailure\"] }",
        korean_block: "# 언어별 테스트 규칙: 목록을 바꾸면 사용자 설정으로 대체하고 []로 비활성화합니다.\n# test_decorators = { python = [\"pytest.fixture\", \"pytest.mark.*\", \"unittest.skip\", \"unittest.skipIf\", \"unittest.skipUnless\", \"unittest.expectedFailure\"] }",
    },
    Migration {
        version: 7,
        key: "test_attributes",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# Per-language test rules: replace a list to customize it; [] disables it.\n# test_attributes = { rust = [\"test\", \"tokio::test\", \"async_std::test\", \"rstest\", \"rstest::rstest\", \"cfg(test)\"], java = [\"Test\", \"ParameterizedTest\", \"RepeatedTest\", \"TestFactory\", \"TestTemplate\", \"Nested\", \"BeforeEach\", \"AfterEach\", \"BeforeAll\", \"AfterAll\"], kotlin = [\"Test\", \"ParameterizedTest\", \"RepeatedTest\", \"BeforeTest\", \"AfterTest\", \"BeforeEach\", \"AfterEach\"], csharp = [\"Fact\", \"Theory\", \"Test\", \"TestCase\", \"TestCaseSource\", \"TestFixture\", \"SetUp\", \"TearDown\", \"OneTimeSetUp\", \"OneTimeTearDown\"], swift = [\"Test\", \"Suite\"], php = [\"Test\"] }",
        korean_block: "# 언어별 테스트 규칙: 목록을 바꾸면 사용자 설정으로 대체하고 []로 비활성화합니다.\n# test_attributes = { rust = [\"test\", \"tokio::test\", \"async_std::test\", \"rstest\", \"rstest::rstest\", \"cfg(test)\"], java = [\"Test\", \"ParameterizedTest\", \"RepeatedTest\", \"TestFactory\", \"TestTemplate\", \"Nested\", \"BeforeEach\", \"AfterEach\", \"BeforeAll\", \"AfterAll\"], kotlin = [\"Test\", \"ParameterizedTest\", \"RepeatedTest\", \"BeforeTest\", \"AfterTest\", \"BeforeEach\", \"AfterEach\"], csharp = [\"Fact\", \"Theory\", \"Test\", \"TestCase\", \"TestCaseSource\", \"TestFixture\", \"SetUp\", \"TearDown\", \"OneTimeSetUp\", \"OneTimeTearDown\"], swift = [\"Test\", \"Suite\"], php = [\"Test\"] }",
    },
    Migration {
        version: 7,
        key: "test_file_patterns",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# Workspace-relative test file globs; slash-less patterns match basenames. [] disables path detection.\n# test_file_patterns = [\"**/tests/**\", \"**/test/**\", \"**/__tests__/**\", \"test_*.py\", \"*_test.*\", \"*.test.*\", \"*_spec.*\", \"*.spec.*\", \"*Test.java\", \"*Tests.java\", \"*IT.java\"]",
        korean_block: "# 작업공간 기준 테스트 파일 glob입니다. /가 없으면 파일 이름과 비교하며 []로 경로 판별을 끕니다.\n# test_file_patterns = [\"**/tests/**\", \"**/test/**\", \"**/__tests__/**\", \"test_*.py\", \"*_test.*\", \"*.test.*\", \"*_spec.*\", \"*.spec.*\", \"*Test.java\", \"*Tests.java\", \"*IT.java\"]",
    },
    Migration {
        version: 7,
        key: "should_include_test_code",
        placement: KeyPlacement::Subtable("caller_context"),
        english_block: "# Include test regions in automatic symbol/call context. Live read/grep source is unchanged.\n# should_include_test_code = false",
        korean_block: "# 자동 심볼·호출 관계에 테스트 영역을 포함합니다. 직접 read/grep한 원문은 유지됩니다.\n# should_include_test_code = false",
    },
];

/// `# codemap-config-version: <version>` — the stamp line written into every managed file.
fn version_marker_line(version: u32) -> String {
    format!("{VERSION_MARKER_PREFIX} {version}")
}

/// Scaffold or incrementally sync `<repo_root>/.codemap/config.toml` on `mcp` start.
///
/// - Absent → write the localized explicit-default template, whose first line is the
///   [`CONFIG_VERSION`] marker, so a fresh repo gets a discoverable config whose keys are live.
/// - Present → run migration sync: append only the commented blocks for keys introduced
///   since the file's stamped version (presence-guarded), re-stamp the marker, and rewrite.
///   A file already at the current version is left byte-for-byte untouched.
///
/// Never-exit: a directory-create, read, or write failure warns to stderr and returns rather
/// than crashing the server. The path matches exactly what [`load`] reads. Incrementally added
/// keys are still commented; v6 materializes directory exclusions once. v8/v9 relocate
/// test-code and workspace exclusions into `[exclude]` without changing effective values.
pub fn ensure_repo_config(repo_root: &Path) {
    ensure_repo_config_with_auto_update(repo_root, get().config_auto_update);
}

fn ensure_repo_config_with_auto_update(repo_root: &Path, config_auto_update: bool) {
    if !config_auto_update {
        return;
    }
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

/// Write the version-stamped localized template for a repo that has no config file yet.
fn scaffold_fresh(dir: &Path, path: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        warn(&format!(
            "config template skipped: create {}: {e}",
            dir.display()
        ));
        return;
    }
    let repo_root = dir.parent().unwrap_or(dir);
    let template =
        match scaffold::fresh_config(config_template(config_comment_language()), repo_root) {
            Ok(template) => template,
            Err(error) => {
                warn(&format!("config template skipped: {error}"));
                return;
            }
        };
    if let Err(e) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(template.as_bytes()))
    {
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
    let original = existing;
    let file_version = parse_version_marker(existing).unwrap_or(CONFIG_BASELINE_VERSION);
    if file_version >= CONFIG_VERSION {
        return; // already current — never touch the user's file
    }
    let existing = if file_version < 6 {
        let repo_root = path
            .parent()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."));
        match scaffold::migrate_exclusions(existing, repo_root, &get().excluded_directories) {
            Ok(updated) => updated,
            Err(error) => {
                warn(&format!(
                    "config v6 migration skipped for {}: {error}",
                    path.display()
                ));
                return;
            }
        }
    } else {
        existing.to_string()
    };
    let Some(mut updated) = apply_migrations_with_language(
        &existing,
        file_version,
        CONFIG_VERSION,
        MIGRATIONS,
        config_comment_language(),
    ) else {
        return; // already current — never touch the user's file
    };
    if file_version < 9 {
        updated = match exclude::migrate(&updated, &existing, path) {
            Ok(updated) => updated,
            Err(error) => {
                warn(&format!(
                    "config v9 migration skipped for {}: {error}",
                    path.display()
                ));
                return;
            }
        };
    }
    if let Err(e) = scaffold::write_migration(path, original, &updated) {
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
    let ranges = config_value_ranges(contents);
    let mut offset = 0;
    contents.split_inclusive('\n').find_map(|line| {
        let start = offset;
        offset += line.len();
        if ranges.iter().any(|range| range.contains(&start)) {
            return None;
        }
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
    let ranges = config_value_ranges(contents);
    let mut offset = 0;
    contents.split_inclusive('\n').any(|line| {
        let start = offset;
        offset += line.len();
        if ranges.iter().any(|range| range.contains(&start)) {
            return false;
        }
        let body = line.trim_start();
        let body = body.strip_prefix('#').map(str::trim_start).unwrap_or(body);
        body.strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
            || body
                .strip_prefix('[')
                .and_then(|table| table.split_once(']'))
                .is_some_and(|(table, _)| table.trim().rsplit('.').next() == Some(key))
    })
}

/// Apply the additive migrations newer than `file_version` (up to `target_version`) to
/// `contents`. Each unseen key's block is inserted section-aware; the version marker is then
/// stamped to `target_version`. Returns `Some(new_contents)` when a sync is due, or `None`
/// when the file is already at/ahead of the target (so the caller writes nothing).
#[cfg(test)]
fn apply_migrations(
    contents: &str,
    file_version: u32,
    target_version: u32,
    migrations: &[Migration],
) -> Option<String> {
    apply_migrations_with_language(
        contents,
        file_version,
        target_version,
        migrations,
        ConfigCommentLanguage::English,
    )
}

fn apply_migrations_with_language(
    contents: &str,
    file_version: u32,
    target_version: u32,
    migrations: &[Migration],
    language: ConfigCommentLanguage,
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
        let block = migration.block(language);
        out = match migration.placement {
            KeyPlacement::TopLevel => insert_top_level(&out, block),
            KeyPlacement::Subtable(table) => insert_subtable(&out, table, block),
        };
    }
    // `file_version < target_version` here, so the marker always advances → always a change.
    Some(set_version_marker(&out, target_version))
}

/// Byte offset of the first line that opens a TOML table (`[...]`), honoring an optional
/// leading `# ` so it also anchors before a *commented* `# [filesystem_permissions]`. `None`
/// when the file has no table header.
fn first_table_header_offset(contents: &str) -> Option<usize> {
    let value_ranges = config_value_ranges(contents);
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        let body = line.trim_start();
        let body = body.strip_prefix('#').map(str::trim_start).unwrap_or(body);
        if body.starts_with('[') && !value_ranges.iter().any(|range| range.contains(&offset)) {
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
    let value_ranges = config_value_ranges(contents);
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        let body = line.trim_start();
        let body = body.strip_prefix('#').map(str::trim_start).unwrap_or(body);
        if body.starts_with(&header) && !value_ranges.iter().any(|range| range.contains(&offset)) {
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
    insert_top_level(contents, &format!("# {header}\n{block}"))
}

/// Immutable TOML documents retain the physical spans discarded by DocumentMut.
/// Value ranges keep migration text out of multiline strings and array contents.
fn config_value_ranges(contents: &str) -> Vec<std::ops::Range<usize>> {
    fn collect(table: &toml_edit::Table, ranges: &mut Vec<std::ops::Range<usize>>) {
        for (_, item) in table.iter() {
            if let Some(value) = item.as_value() {
                if let Some(range) = value.span() {
                    ranges.push(range);
                }
            } else if let Some(table) = item.as_table() {
                collect(table, ranges);
            } else if let Some(tables) = item.as_array_of_tables() {
                for table in tables.iter() {
                    collect(table, ranges);
                }
            }
        }
    }
    let mut ranges = Vec::new();
    if let Ok(document) = toml_edit::Document::parse(contents) {
        collect(document.as_table(), &mut ranges);
    }
    ranges
}

/// Replace the existing [`VERSION_MARKER_PREFIX`] line's value with `version`, or prepend a
/// fresh marker line when the file has none.
fn set_version_marker(contents: &str, version: u32) -> String {
    let marker = version_marker_line(version);
    let ranges = config_value_ranges(contents);
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        if line.trim_start().starts_with(VERSION_MARKER_PREFIX)
            && !ranges.iter().any(|range| range.contains(&offset))
        {
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

/// Byte sizes retain integer-byte compatibility; quoted units use powers of 1024.
fn parse_byte_size(value: &toml::Value) -> Option<u64> {
    let bytes = match value {
        toml::Value::Integer(bytes) => u64::try_from(*bytes).ok()?,
        toml::Value::String(size) => {
            let size = size.trim();
            let unit_start = size.find(|ch: char| !ch.is_ascii_digit())?;
            let (amount, unit) = size.split_at(unit_start);
            let amount = amount.parse::<u64>().ok()?;
            let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
                "b" => 1,
                "kb" => 1024,
                "mb" => 1024 * 1024,
                "gb" => 1024 * 1024 * 1024,
                _ => return None,
            };
            amount.checked_mul(multiplier)?
        }
        _ => return None,
    };
    (bytes > 0).then_some(bytes)
}

fn as_positive_byte_size<T: TryFrom<u64>>(
    value: &toml::Value,
    key: &str,
    path: &Path,
) -> Option<T> {
    match parse_byte_size(value).and_then(|bytes| T::try_from(bytes).ok()) {
        Some(bytes) => Some(bytes),
        None => {
            warn(&format!(
                "config '{key}' must be a positive byte count or quoted size using b/kb/mb/gb within the supported range: {} — ignored",
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

fn as_directory_patterns(value: &toml::Value, key: &str, path: &Path) -> Option<Vec<String>> {
    let patterns = as_string_array(value, key, path)?;
    match DirectoryExclusions::new(&patterns) {
        Ok(_) => Some(
            patterns
                .into_iter()
                .map(|pattern| pattern.replace('\\', "/"))
                .collect(),
        ),
        Err(error) => {
            warn(&format!(
                "config '{key}': {error}: {} — ignored",
                path.display()
            ));
            None
        }
    }
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
    use std::sync::mpsc::{sync_channel, TryRecvError};
    use tempfile::tempdir;

    fn write_repo_config(repo: &Path, body: &str) {
        let dir = repo.join(CODEMAP_DIR_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONFIG_FILE_NAME), body).unwrap();
    }

    #[test]
    fn test_analysis_target_os_layers_clear_and_invalid_fallback() {
        let path = Path::new("config.toml");
        let layer = |value: &str| normalize(toml::from_str(value).unwrap(), path);
        let global = layer("[analysis]\ntarget_os='macos'\n");
        assert_eq!(
            merge(ConfigLayer::default(), global)
                .analysis_target_os
                .as_deref(),
            Some("macos")
        );
        let config = merge(
            layer("[analysis]\ntarget_os=''\n"),
            layer("[analysis]\ntarget_os='macos'\n"),
        );
        assert_eq!(config.analysis_target_os, None);
        let config = merge(
            layer("[analysis]\ntarget_os=42\n"),
            layer("[analysis]\ntarget_os='linux'\n"),
        );
        assert_eq!(config.analysis_target_os.as_deref(), Some("linux"));
    }

    #[test]
    fn test_defaults_when_no_files() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        let cfg = load(repo.path(), global.path());
        let defaults = ResolvedConfig::default();
        assert!(cfg.config_auto_update);
        assert_eq!(cfg.index_path, defaults.index_path);
        assert_eq!(cfg.result_threshold, 24);
        assert_eq!(cfg.max_file_size, crate::workspace::MAX_INDEXED_FILE_BYTES);
        assert!(cfg.use_git_exclude);
        assert!(cfg.excluded_directories.iter().any(|d| d == "node_modules"));
        assert_eq!(cfg.search_anchor_snippet_limit, 20);
        // Caller/callee context: annotation on by default, caps at their tuned values.
        assert!(cfg.caller_context_default);
        assert!(!cfg.should_include_test_code);
        assert_eq!(cfg.test_code_rules, TestCodeRules::default());
        assert_eq!(cfg.scan_cap, 16_000);
        assert_eq!(cfg.caller_list_cap, 1000);
        assert_eq!(cfg.callee_list_cap, 1000);
        assert_eq!(cfg.annotation_sub_budget, 128 * 1024);
        assert_eq!(cfg.common_name_threshold, 2);
        assert!(!cfg.is_document_support_enabled);
        assert!(!cfg.is_shell_support_enabled);
        assert!(!cfg.is_infrastructure_support_enabled);
        assert!(!cfg.is_interface_support_enabled);
        assert!(!cfg.is_build_support_enabled);
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
        assert_eq!(cfg.caller_list_cap, 1000);
        assert_eq!(cfg.annotation_sub_budget, 128 * 1024);
    }

    #[test]
    fn test_test_rules_override_per_language_and_allow_disabling_defaults() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        fs::write(
            global.path().join(CONFIG_FILE_NAME),
            r#"
[caller_context]
should_include_test_code = true
test_file_patterns = ["checks/**"]
test_attributes = { rust = ["global::check"], java = ["GlobalTest"] }
test_decorators = { python = ["project.check"] }
"#,
        )
        .unwrap();
        write_repo_config(
            repo.path(),
            r#"
[exclude]
should_include_test_code = false
test_file_patterns = []
test_attributes = { rust = [], kotlin = ["CustomTest"] }
test_calls = { typescript = [] }
"#,
        );
        let cfg = load(repo.path(), global.path());
        assert!(!cfg.should_include_test_code);
        assert!(cfg.test_code_rules.file_patterns.is_empty());
        assert!(cfg.test_code_rules.attributes["rust"].is_empty());
        assert_eq!(cfg.test_code_rules.attributes["java"], ["GlobalTest"]);
        assert_eq!(cfg.test_code_rules.attributes["kotlin"], ["CustomTest"]);
        assert_eq!(cfg.test_code_rules.decorators["python"], ["project.check"]);
        assert!(cfg.test_code_rules.calls["typescript"].is_empty());
        assert_eq!(
            cfg.test_code_rules.calls["javascript"],
            TestCodeRules::default().calls["javascript"]
        );

        write_repo_config(
            repo.path(),
            r#"
[exclude]
test_file_patterns = ["../outside"]
test_attributes = { rust = ["["], unknown_language = ["check"] }
"#,
        );
        let invalid = load(repo.path(), global.path());
        assert!(invalid.should_include_test_code);
        assert_eq!(invalid.test_code_rules.file_patterns, ["checks/**"]);
        assert_eq!(
            invalid.test_code_rules.attributes["rust"],
            ["global::check"]
        );
        assert!(!invalid
            .test_code_rules
            .attributes
            .contains_key("unknown_language"));

        write_repo_config(
            repo.path(),
            r#"
[exclude]
should_include_test_code = false
test_file_patterns = []
test_attributes = { rust = [] }
[caller_context]
should_include_test_code = true
test_file_patterns = ["legacy/**"]
test_attributes = { rust = ["legacy::test"], java = ["LegacyTest"] }
"#,
        );
        let mixed = load(repo.path(), global.path());
        assert!(!mixed.should_include_test_code);
        assert!(mixed.test_code_rules.file_patterns.is_empty());
        assert!(mixed.test_code_rules.attributes["rust"].is_empty());
        assert_eq!(mixed.test_code_rules.attributes["java"], ["LegacyTest"]);
    }

    #[test]
    fn test_update_config_auto_update_overrides() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        write_repo_config(repo.path(), "[update]\nconfig_auto_update = false\n");
        let cfg = load(repo.path(), global.path());
        assert!(
            !cfg.config_auto_update,
            "repo config can disable automatic config writes"
        );
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
    fn test_language_support_repo_overrides_global_and_bad_type_falls_back() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        fs::write(
            global.path().join(CONFIG_FILE_NAME),
            "[language_support]\nis_document_support_enabled = true\nis_shell_support_enabled = true\nis_infrastructure_support_enabled = true\nis_interface_support_enabled = true\nis_build_support_enabled = true\n",
        )
        .unwrap();
        assert_eq!(
            load(repo.path(), global.path()).language_support_settings(),
            [true; 5]
        );

        write_repo_config(
            repo.path(),
            "[language_support]\nis_document_support_enabled = false\nis_shell_support_enabled = false\nis_infrastructure_support_enabled = false\nis_interface_support_enabled = false\nis_build_support_enabled = false\n",
        );
        assert_eq!(
            load(repo.path(), global.path()).language_support_settings(),
            [false; 5]
        );

        write_repo_config(
            repo.path(),
            "[language_support]\nis_document_support_enabled = \"yes\"\nis_shell_support_enabled = \"yes\"\nis_infrastructure_support_enabled = \"yes\"\nis_interface_support_enabled = \"yes\"\nis_build_support_enabled = \"yes\"\n",
        );
        assert_eq!(
            load(repo.path(), global.path()).language_support_settings(),
            [true; 5],
            "invalid repo types must fall back to the valid global values"
        );
    }

    #[test]
    fn test_index_scope_refresh_survives_a_full_path_queue() {
        let (sender, receiver) = sync_channel(1);
        let previous = ResolvedConfig::default();
        request_refresh_if_index_scope_changed(&previous, &previous, &sender);
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
        sender
            .send(crate::index::IndexCommand::RefreshPaths(vec![]))
            .unwrap();
        let mut current = previous.clone();
        current.excluded_directories.clear();
        let task = std::thread::spawn(move || {
            request_refresh_if_index_scope_changed(&previous, &current, &sender);
        });
        assert!(matches!(
            receiver.recv().unwrap(),
            crate::index::IndexCommand::RefreshPaths(_)
        ));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            crate::index::IndexCommand::Refresh
        ));
        task.join().unwrap();
    }

    #[test]
    fn test_excluded_directories_replace_and_validate() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        std::fs::write(
            global.path().join("config.toml"),
            "[index]\nexcluded_directories = ['vendor']\n",
        )
        .unwrap();
        write_repo_config(
            repo.path(),
            "[exclude]\nexcluded_directories = ['apps/web/build']\n",
        );
        assert_eq!(
            load(repo.path(), global.path()).excluded_directories,
            vec!["apps/web/build"]
        );
        write_repo_config(repo.path(), "[exclude]\nexcluded_directories = []\n");
        assert!(load(repo.path(), global.path())
            .excluded_directories
            .is_empty());
        write_repo_config(
            repo.path(),
            "[exclude]\nexcluded_directories = ['../bad']\n",
        );
        assert_eq!(
            load(repo.path(), global.path()).excluded_directories,
            vec!["vendor"]
        );
        write_repo_config(repo.path(), "[exclude]\n");
        assert_eq!(
            load(repo.path(), global.path()).excluded_directories,
            vec!["vendor"]
        );
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
        write_repo_config(repo.path(), "[exclude]\nuse_git_exclude = false\nexcluded_directories = []\n[index]\nuse_git_exclude = true\nexcluded_directories = ['legacy']\n");
        let canonical = load(repo.path(), global.path());
        assert!(!canonical.use_git_exclude);
        assert!(canonical.excluded_directories.is_empty());
        write_repo_config(repo.path(), "[exclude]\nuse_git_exclude = 'invalid'\nexcluded_directories = ['../bad']\n[index]\nuse_git_exclude = false\nexcluded_directories = ['legacy']\n");
        let fallback = load(repo.path(), global.path());
        assert!(!fallback.use_git_exclude);
        assert_eq!(fallback.excluded_directories, ["legacy"]);
        fs::write(
            global.path().join(CONFIG_FILE_NAME),
            "[exclude]\nuse_git_exclude = false\nexcluded_directories = ['global']\n",
        )
        .unwrap();
        write_repo_config(
            repo.path(),
            "[index]\nuse_git_exclude = true\nexcluded_directories = []\n",
        );
        let legacy_repo = load(repo.path(), global.path());
        assert!(legacy_repo.use_git_exclude);
        assert!(legacy_repo.excluded_directories.is_empty());
    }

    #[test]
    fn test_malformed_config_falls_back_to_defaults() {
        let repo = tempdir().unwrap();
        let global = tempdir().unwrap();
        write_repo_config(repo.path(), "this is = = not valid toml [[[");
        let cfg = load(repo.path(), global.path());
        assert_eq!(
            cfg.result_threshold, 24,
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
            cfg.result_threshold, 24,
            "bad-typed key must fall back to default"
        );
    }

    // --- version marker + incremental sync ---------------------------------------------

    #[test]
    fn test_config_template_version_marker_matches_config_version() {
        assert_eq!(
            parse_version_marker(CONFIG_TEMPLATE),
            Some(CONFIG_VERSION),
            "config_template.toml must carry the current schema version marker"
        );
        assert_eq!(
            parse_version_marker(CONFIG_TEMPLATE_KO),
            Some(CONFIG_VERSION),
            "config_template.ko.toml must carry the current schema version marker"
        );
        assert_eq!(
            CONFIG_TEMPLATE.matches(VERSION_MARKER_PREFIX).count(),
            1,
            "config_template.toml must contain exactly one schema version marker"
        );
    }

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
            english_block: "# New key doc.\n# new_key = 7",
            korean_block: "# New key doc.\n# new_key = 7",
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
            english_block: "# dup doc.\n# already_here = 1",
            korean_block: "# dup doc.\n# already_here = 1",
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
            english_block: "# v2 key.\n# added_in_v2 = 1",
            korean_block: "# v2 key.\n# added_in_v2 = 1",
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
    fn test_v2_migrations_insert_navigation_keys_under_caller_context() {
        let body = "# codemap-config-version: 1\n\n[caller_context]\n# caller_context_default = true\n\n[tool_output]\n# grep_max_columns = 500\n";
        let out = apply_migrations(body, 1, 2, MIGRATIONS).expect("v2 sync is due");
        let header_pos = out.find("[caller_context]").unwrap();
        let navigation_context_pos = out.find("navigation_context_default").unwrap();
        let navigation_budget_pos = out.find("navigation_callsite_budget").unwrap();
        let navigation_store_pos = out.find("navigation_store_references").unwrap();
        let next_table_pos = out.find("[tool_output]").unwrap();
        assert!(
            header_pos < navigation_context_pos && navigation_store_pos < next_table_pos,
            "navigation keys must be inserted inside [caller_context]: {out:?}"
        );
        assert!(
            navigation_context_pos < navigation_budget_pos
                && navigation_budget_pos < navigation_store_pos,
            "navigation migration keys must keep template order: {out:?}"
        );
        assert_eq!(
            out.matches("navigation_context_default").count(),
            1,
            "migration must not duplicate the v2 navigation key: {out:?}"
        );
        assert!(out.contains("# codemap-config-version: 2"));
    }

    #[test]
    fn test_v3_migration_adds_commented_update_section_before_tables() {
        let body = "# codemap-config-version: 2\n\n[index]\nindex_path = \".codemap/index\"\n";
        let out = apply_migrations(body, 2, 3, MIGRATIONS).expect("v3 sync is due");
        let update_pos = out.find("# [update]").unwrap();
        let key_pos = out.find("# config_auto_update = true").unwrap();
        let index_pos = out.find("[index]").unwrap();
        assert!(
            update_pos < key_pos && key_pos < index_pos,
            "update migration block must be commented and precede the first table: {out:?}"
        );
        assert!(
            out.contains("append commented blocks for new settings"),
            "migration block should explain that auto-update adds commented settings: {out:?}"
        );
        assert!(out.contains("# codemap-config-version: 3"));
    }

    #[test]
    fn test_language_support_migrations_are_localized_and_idempotent() {
        let original = "# codemap-config-version: 3\n[index]\nindex_path = \".codemap/index\"\n";
        for (language, expected_comment) in [
            (ConfigCommentLanguage::English, "Shell scripts"),
            (ConfigCommentLanguage::Korean, "셸 스크립트"),
        ] {
            let migrated =
                apply_migrations_with_language(original, 3, CONFIG_VERSION, MIGRATIONS, language)
                    .unwrap();
            assert!(migrated.contains(&version_marker_line(CONFIG_VERSION)));
            assert!(migrated.contains("# [language_support]"));
            assert!(migrated.contains("# is_document_support_enabled = false"));
            assert!(migrated.contains("# is_shell_support_enabled = false"));
            assert!(migrated.contains("# is_infrastructure_support_enabled = false"));
            assert!(migrated.contains("# is_interface_support_enabled = false"));
            assert!(migrated.contains("# is_build_support_enabled = false"));
            assert!(migrated.contains(expected_comment));
            for key in [
                "is_document_support_enabled",
                "is_shell_support_enabled",
                "is_infrastructure_support_enabled",
                "is_interface_support_enabled",
                "is_build_support_enabled",
            ] {
                assert_eq!(migrated.matches(key).count(), 1);
            }
            assert!(apply_migrations_with_language(
                &migrated,
                CONFIG_VERSION,
                CONFIG_VERSION,
                MIGRATIONS,
                language
            )
            .is_none());
        }
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
        // Explicit scaffolded defaults still resolve to the compiled-in defaults.
        let global = tempdir().unwrap();
        assert_eq!(load(repo.path(), global.path()).result_threshold, 24);
    }

    #[test]
    fn test_ensure_repo_config_skips_when_auto_update_disabled() {
        let repo = tempdir().unwrap();
        ensure_repo_config_with_auto_update(repo.path(), false);
        let path = repo.path().join(CODEMAP_DIR_NAME).join(CONFIG_FILE_NAME);
        assert!(
            !path.exists(),
            "disabled config auto-update must not create a repo config"
        );
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
        let global = tempdir().unwrap();
        fs::write(global.path().join(CONFIG_FILE_NAME), "[exclude]\nuse_git_exclude = false\nexcluded_directories = ['global']\nshould_include_test_code = true\ntest_file_patterns = ['global_checks/**']\ntest_attributes = { rust = ['global::test'] }\n").unwrap();
        let commented = format!(
            "# codemap-config-version: 7\n[caller_context]\n# keep policy\n{}\n# keep disabled\nshould_include_test_code = true\n",
            MIGRATIONS
                .iter()
                .filter(|migration| migration.version == 7)
                .map(|migration| migration.english_block)
                .collect::<Vec<_>>()
                .join("\n\n")
        );
        for source in [
            "# codemap-config-version: 7\n[caller_context]\n# keep policy\nshould_include_test_code = true\ntest_file_patterns = [] # keep disabled\nscan_cap = 25\n[caller_context.test_attributes]\nrust = []\n[caller_context.test_decorators]\npython = ['company_test']\n[caller_context.test_calls]\ntypescript = []\n[index]\nexcluded_directories = []\n",
            "# codemap-config-version: 7\n# keep policy\ncaller_context = { should_include_test_code = true, test_file_patterns = [], test_attributes = { rust = [] }, scan_cap = 25 } # keep disabled\n",
            "# codemap-config-version: 7\n# keep policy\nshould_include_test_code = true\ntest_file_patterns = [] # keep disabled\n[caller_context.\"test_attributes\"]\nrust = []\n",
            "# codemap-config-version: 7\n[exclude]\n# keep policy\nshould_include_test_code = false\ntest_file_patterns = [] # keep disabled\ntest_attributes = { rust = [] }\n[caller_context]\nshould_include_test_code = true\ntest_attributes = { rust = ['old'], java = ['CustomTest'] }\n",
            "# codemap-config-version: 7\nindex_path = '''.cache\n[exclude]'''\n[caller_context]\n# keep policy\nshould_include_test_code = true\ntest_file_patterns = [] # keep disabled\n",
            "# codemap-config-version: 7\n# keep policy\nexclude = { should_include_test_code = false, test_file_patterns = [], test_attributes = { rust = [] } } # keep disabled\n[caller_context]\ntest_attributes = { java = ['CustomTest'] }\n",
            "# codemap-config-version: 8\n[index]\nindex_path = '.my-index'\nmax_file_size = 2048\n# keep policy\nexcluded_directories = ['apps/web/build', 'a#b'] # keep disabled\nuse_git_exclude = true\n[exclude]\nshould_include_test_code = false\n",
            "# codemap-config-version: 8\n# keep policy\nindex = { index_path = '.my-index', excluded_directories = [], use_git_exclude = false } # keep disabled\n",
            "# codemap-config-version: 8\n# keep policy\nexcluded_directories = [] # keep disabled\nuse_git_exclude = true\n",
            "# codemap-config-version: 8\n[exclude]\n# keep policy\nexcluded_directories = [] # keep disabled\nuse_git_exclude = true\n[index]\nexcluded_directories = ['legacy']\nuse_git_exclude = false\n",
            commented.as_str(),
        ] {
            fs::write(&path, source).unwrap();
            let before = load(repo.path(), global.path());
            ensure_repo_config_with_auto_update(repo.path(), true);
            let migrated = fs::read_to_string(&path).unwrap();
            assert!(migrated.starts_with(&format!("# codemap-config-version: {CONFIG_VERSION}")), "{migrated}");
            assert!(migrated.contains("# keep policy"), "{migrated}");
            assert!(migrated.contains("# keep disabled"), "{migrated}");
            let parsed: toml::Value = toml::from_str(&migrated).unwrap();
            assert!(parsed.get("exclude").and_then(toml::Value::as_table).is_some());
            if source == commented {
                let (_, section) = migrated.split_once("[exclude]").unwrap();
                assert!(section.contains("# should_include_test_code = false"), "{migrated}");
                assert!(section.contains("# test_attributes ="), "{migrated}");
            }
            for &key in exclude::TEST_KEYS {
                assert!(parsed.get(key).is_none(), "{migrated}");
                assert!(parsed.get("caller_context").and_then(|table| table.get(key)).is_none(), "{migrated}");
            }
            for &key in exclude::WORKSPACE_KEYS {
                assert!(parsed.get(key).is_none(), "{migrated}");
                assert!(parsed.get("index").and_then(|table| table.get(key)).is_none(), "{migrated}");
            }
            let after = load(repo.path(), global.path());
            assert_eq!(before.should_include_test_code, after.should_include_test_code);
            assert_eq!(before.test_code_rules, after.test_code_rules);
            assert_eq!(before.excluded_directories, after.excluded_directories);
            assert_eq!(before.use_git_exclude, after.use_git_exclude);
            assert_eq!(before.index_path, after.index_path);
            assert_eq!(before.max_file_size, after.max_file_size);
            assert_eq!(before.scan_cap, after.scan_cap);
            assert_eq!(before.macro_expansion, after.macro_expansion);
            ensure_repo_config_with_auto_update(repo.path(), true);
            assert_eq!(fs::read_to_string(&path).unwrap(), migrated);
        }
    }
}
