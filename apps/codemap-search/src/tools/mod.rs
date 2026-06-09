//! Self-contained `read` / `find` / `grep` primitives backed by ripgrep's library
//! crates (`grep`, `ignore`, `globset`) — no external `rg` binary, no system `rg`.
//!
//! This module also owns the shared filesystem-walk configuration (custom-ignore
//! filename, junk-dir excludes, source-extension set, and the configured
//! `ignore::WalkBuilder`). Child 04 (indexing) is expected to adopt these wholesale
//! so every tool observes the same excludes, and Child 05 (config) will make the
//! exclude set + ignore filename user-configurable.

pub mod find;
pub mod grep;
pub mod read;

use std::path::{Path, PathBuf};

/// Per-repo custom ignore file using gitignore syntax/semantics. Name is still
/// pending final confirmation in Child 05; keep it a single constant so the rename
/// is one line.
pub const CODEMAP_IGNORE_FILENAME: &str = ".codemapignore";

/// The source-file extensions codemap-search understands. Single source of truth —
/// Child 03 collapses the previously-duplicated literal lists (`main.rs`, `mcp.rs`,
/// `index.rs`, `benchmark.rs`) onto this constant.
pub const SOURCE_EXTENSIONS: &[&str] = &["rs", "py", "ts", "tsx", "js", "jsx"];

/// Files larger than this are skipped before parsing/indexing (Child 04). Hand-written
/// source virtually never exceeds 1 MiB; this guards against minified bundles and
/// generated blobs being read+parsed wholesale. Child 05 makes it configurable.
pub const MAX_INDEXED_FILE_BYTES: u64 = 1_048_576;

/// Junk / VCS / generated directories that no tool ever walks, independent of any
/// ignore file. Child 04 centralizes this and Child 05 makes it configurable.
pub const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    ".git",
    ".svn",
    ".hg",
    ".codemap",
    ".codemap-index",
];

/// Whether `ext` is a source extension codemap-search understands. Single predicate
/// over [`SOURCE_EXTENSIONS`] — replaces the `matches!(ext, "rs" | "py" | …)` literals
/// that were duplicated across `main.rs`, `mcp.rs`, `index.rs`, and `benchmark.rs`.
pub fn is_source_extension(ext: &str) -> bool {
    SOURCE_EXTENSIONS.contains(&ext)
}

/// Resolve the current working directory, mapping failure to a JSON-RPC error.
pub(crate) fn current_dir() -> Result<PathBuf, (i64, String)> {
    std::env::current_dir()
        .map_err(|e| (-32603, format!("Cannot determine current directory: {e}")))
}

/// Resolve `rel` against the current working directory and assert the result stays
/// within it. Returns the (leniently canonicalized) absolute path. Rejects paths
/// that escape the workspace root via `..` or symlinks. The returned path may or may
/// not exist on disk — existence is the caller's concern.
pub(crate) fn resolve_within_cwd(rel: &str) -> Result<PathBuf, (i64, String)> {
    let cwd = current_dir()?;
    let target = cwd.join(rel);

    // Lexically collapse `.`/`..` before canonicalizing so traversal can't escape.
    let mut resolved = PathBuf::new();
    for component in target.components() {
        match component {
            std::path::Component::ParentDir => {
                resolved.pop();
            }
            std::path::Component::CurDir => {}
            other => resolved.push(other.as_os_str()),
        }
    }

    let resolved_canonical = crate::mcp::canonicalize_path_lenient(&resolved);
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    if resolved_canonical.starts_with(&cwd_canonical) {
        Ok(resolved_canonical)
    } else {
        Err((-32602, format!("Path escapes the workspace root: {rel}")))
    }
}

/// Build an `ignore::WalkBuilder` rooted at `root`. By default it respects
/// `.gitignore` (and global/exclude), the repo-local `.codemapignore`, and parent
/// ignore files, and always skips [`EXCLUDED_DIRS`]. When `include_ignored` is true
/// the ignore files are bypassed (the explicit override from the brief) while the
/// junk-dir excludes still apply. Hidden files are included for Claude Code `--hidden`
/// parity. The returned builder can be further configured (e.g. `.types(...)`) before
/// `.build()`.
pub fn build_walker(root: &Path, include_ignored: bool) -> ignore::WalkBuilder {
    // `include_ignored` (per-call, find/grep) bypasses EVERY ignore source. Distinct from
    // that, `respect_git_exclude` is a dedicated config toggle for `.git/info/exclude`
    // alone — when false, that one source is ignored while `.gitignore`/global/
    // `.codemapignore` stay respected (Child 05).
    let respect = !include_ignored;
    let respect_git_exclude = respect && crate::config::get().respect_git_exclude;
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(respect)
        .git_global(respect)
        .git_exclude(respect_git_exclude)
        .ignore(respect)
        .parents(respect)
        // Honor `.gitignore` even outside an initialized git repo (no `.git` needed).
        .require_git(false);
    if respect {
        builder.add_custom_ignore_filename(CODEMAP_IGNORE_FILENAME);
    }
    builder.filter_entry(|entry| {
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            if let Some(name) = entry.file_name().to_str() {
                // The resolved exclude set is the built-in junk dirs unioned with any
                // configured ones (Child 05); defaults to EXCLUDED_DIRS when no config.
                return !crate::config::get()
                    .excluded_directories
                    .iter()
                    .any(|d| d == name);
            }
        }
        true
    });
    builder
}

/// Read a source file's contents for codemap parsing, enforcing the
/// [`MAX_INDEXED_FILE_BYTES`] size cap before reading. Returns `None` when the file is
/// oversize, unreadable, or non-UTF8 (`read_to_string` rejects invalid UTF-8) — so
/// minified/binary blobs are never parsed wholesale (Child 04). Shared by the
/// `get_codemap` walk (`mcp.rs`) and the CLI `codemap` walk (`main.rs`).
pub fn read_source_for_parse(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > crate::config::get().max_file_size {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Read a boolean argument with a default.
pub(crate) fn arg_bool(args: &serde_json::Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// Read an unsigned-integer argument with a default.
pub(crate) fn arg_usize(args: &serde_json::Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(default)
}

/// Read a required string argument, erroring with -32602 when absent/non-string.
pub(crate) fn arg_required_str<'a>(
    args: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, (i64, String)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, format!("Missing required '{key}' parameter")))
}
