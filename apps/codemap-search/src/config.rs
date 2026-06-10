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
/// behavior exactly until the user uncomments a line. Mirrors the README "Configuration"
/// section; keep the two aligned when adding or renaming a key.
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
