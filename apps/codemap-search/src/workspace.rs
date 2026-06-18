//! Shared filesystem / path infrastructure used across every layer: path
//! canonicalization, workspace-root containment, the ignore-aware walker, the
//! source-extension predicate, the walk/limit constants, and the index path-key
//! helpers. Centralizing them here removes the former reverse dependencies
//! (`tools → mcp`, `index → mcp`) and the path-logic duplication.
//!
//! Dependency direction: the walk/limit constants (excluded dirs, max indexed
//! bytes) are owned HERE; `config.rs` references them (`config → workspace`,
//! one-way). The source-extension allowlist is NOT owned here — it is derived from
//! the language registry, so [`is_source_extension`] delegates to
//! [`crate::lang::source_extensions`] (`workspace → lang`, one-way). [`build_walker`]
//! reads `config::get()` at runtime for the configurable exclude set / git-exclude
//! toggle — an accepted dependency on the global config singleton.

use std::path::{Path, PathBuf};

/// Per-repo custom ignore file using gitignore syntax/semantics. Name is still
/// pending final confirmation in Child 05; keep it a single constant so the rename
/// is one line.
pub const CODEMAP_IGNORE_FILENAME: &str = ".codemapignore";

/// Files larger than this are skipped before parsing/indexing (Child 04). Hand-written
/// source virtually never exceeds 1 MiB; this guards against minified bundles and
/// generated blobs being read+parsed wholesale. Child 05 makes it configurable.
pub const MAX_INDEXED_FILE_BYTES: u64 = 1_048_576;

/// Junk / VCS / generated directories that no tool ever walks, independent of any
/// ignore file. Child 04 centralizes this and Child 05 makes it configurable.
pub const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    // Yarn Berry's PnP cache/release dir: holds zipped dependency archives and the
    // generated `.yarn/releases` bundle — the same dependency-blob class as
    // `node_modules`, never hand-written source. Bypassable via `include_ignored`.
    ".yarn",
    "target",
    "dist",
    "build",
    "vendor",
    ".git",
    ".svn",
    ".hg",
    ".bzr",
    ".jj",
    ".sl",
    ".codemap",
    ".codemap-index",
];

/// VCS internals and our own index directory: skipped by EVERY walk, even when
/// `include_ignored` is set. Walking `.git`/`.codemap` only yields churn and noise,
/// never the source a user wants. The rest of [`EXCLUDED_DIRS`] (`node_modules`,
/// `build`, `vendor`, …) stays bypassable via `include_ignored`, because those names
/// can legitimately hold real source in some repos (Go vendoring, hand-written
/// `build/` scripts) and silently hiding them reads as "search is broken".
pub const ALWAYS_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    ".bzr",
    ".jj",
    ".sl",
    ".codemap",
    ".codemap-index",
];

/// Whether `ext` is a source extension codemap-search understands. Single predicate over
/// the registry-derived allowlist [`crate::lang::source_extensions`] (the union of every
/// [`crate::lang::LanguageSpec`]'s extensions) — so adding a language never edits this file.
/// Signature unchanged: the four consumer sites (`index`, `callers`, `main.rs`, `benchmark`)
/// are untouched.
pub fn is_source_extension(ext: &str) -> bool {
    crate::lang::source_extensions().contains(ext)
}

/// Minified web bundle filename suffixes excluded as default `grep` noise (the
/// file-level analogue of the [`EXCLUDED_DIRS`] junk-dir set). A generated `*.min.js` /
/// `*.min.css` bundle is single-line and machine-produced — its matches are noise, never
/// the hand-written source a `grep` is after. Same bypass semantics as the junk dirs: this
/// applies only while ignore rules are respected, so `include_ignored` reaches the file.
const MINIFIED_BUNDLE_SUFFIXES: &[&str] = &[".min.js", ".min.css", ".min.cjs", ".min.mjs"];

/// Whether `file_name` is a minified web bundle (see [`MINIFIED_BUNDLE_SUFFIXES`]). The
/// match is on the literal basename suffix (case-sensitive, matching the source-extension
/// comparison), so `app.min.js` is a bundle but `app.js` is not.
pub fn is_minified_bundle(file_name: &str) -> bool {
    MINIFIED_BUNDLE_SUFFIXES
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
}

/// Leniently canonicalize a path that may not fully exist on disk: resolve the deepest
/// existing ancestor with `canonicalize()` (collapsing symlinks like macOS `/var` →
/// `/private/var`), then re-attach the not-yet-existing suffix. Falls back to the input
/// unchanged when even the root cannot be canonicalized.
pub(crate) fn canonicalize_path_lenient(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    let mut suffix = PathBuf::new();
    while !current.exists() {
        if let Some(parent) = current.parent() {
            if let Some(file_name) = current.file_name() {
                let mut new_suffix = PathBuf::from(file_name);
                new_suffix.push(suffix);
                suffix = new_suffix;
                current = parent.to_path_buf();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    if let Ok(canonical) = current.canonicalize() {
        // Joining an empty suffix would append a trailing separator (`/file/`),
        // which makes a later `metadata()` on a regular file fail with ENOTDIR.
        if suffix.as_os_str().is_empty() {
            canonical
        } else {
            canonical.join(suffix)
        }
    } else {
        path.to_path_buf()
    }
}

/// Resolve the current working directory, mapping failure to a JSON-RPC error.
pub(crate) fn current_dir() -> Result<PathBuf, (i64, String)> {
    std::env::current_dir()
        .map_err(|e| (-32603, format!("Cannot determine current directory: {e}")))
}

/// Resolve `rel` against the current working directory and assert the result stays
/// within it. Returns the (leniently canonicalized) absolute path. Rejects paths
/// that escape the workspace root via `..` or symlinks. The returned path may or may
/// not exist on disk — existence is the caller's concern. This is the single
/// safe-path implementation in the crate (it absorbed the former `mcp::is_safe_path`,
/// whose only caller now checks `resolve_within_cwd(p).is_err()`).
pub(crate) fn resolve_within_cwd(rel: &str) -> Result<PathBuf, (i64, String)> {
    let cwd = current_dir()?;
    let target = cwd.join(path_from_workspace_input(rel));

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

    let resolved_canonical = canonicalize_path_lenient(&resolved);
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    if resolved_canonical.starts_with(&cwd_canonical) {
        Ok(resolved_canonical)
    } else {
        Err((-32602, format!("Path escapes the workspace root: {rel}")))
    }
}

/// Build an `ignore::WalkBuilder` rooted at `root`. By default it respects
/// `.gitignore` (and global/exclude), the repo-local `.codemapignore`, and parent
/// ignore files, and skips [`EXCLUDED_DIRS`]. When `include_ignored` is true the ignore
/// files AND the configurable junk-dir excludes are bypassed — only [`ALWAYS_EXCLUDED_DIRS`]
/// (VCS internals + our own index dir) stays skipped, so `node_modules`/`build`/`vendor`
/// become reachable in repos where those names hold real source. Hidden files are
/// included for Claude Code `--hidden` parity. The returned builder can be further
/// configured (e.g. `.types(...)`) before `.build()`.
pub fn build_walker(root: &Path, include_ignored: bool) -> ignore::WalkBuilder {
    // `include_ignored` (per-call, find/grep) bypasses EVERY ignore source. Distinct from
    // that, `use_git_exclude` is a dedicated config toggle for `.git/info/exclude`
    // alone — when false, that one source is ignored while `.gitignore`/global/
    // `.codemapignore` stay honored (Child 05).
    let respect = !include_ignored;
    let use_git_exclude = respect && crate::config::get().use_git_exclude;
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(respect)
        .git_global(respect)
        .git_exclude(use_git_exclude)
        .ignore(respect)
        .parents(respect)
        // Honor `.gitignore` even outside an initialized git repo (no `.git` needed).
        .require_git(false);
    if respect {
        builder.add_custom_ignore_filename(CODEMAP_IGNORE_FILENAME);
    }
    builder.filter_entry(move |entry| {
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            if let Some(name) = entry.file_name().to_str() {
                // VCS/index dirs are skipped unconditionally — even with include_ignored.
                if ALWAYS_EXCLUDED_DIRS.contains(&name) {
                    return false;
                }
                // The configurable junk-dir set (built-ins unioned with config, Child 05)
                // applies only while ignore rules are respected; include_ignored bypasses
                // it so those names stay reachable when they hold real source.
                if respect {
                    return !crate::config::get()
                        .excluded_directories
                        .iter()
                        .any(|d| d == name);
                }
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
/// `overview` walk (`mcp.rs`) and the CLI `codemap` walk (`main.rs`).
pub fn read_source_for_parse(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > crate::config::get().max_file_size {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Convert a user-supplied workspace path to a [`PathBuf`], accepting Windows-style
/// separators even on non-Windows hosts. Filesystem APIs still receive paths, while
/// workspace-key comparisons stay slash-normalized at string boundaries.
pub fn path_from_workspace_input(path: &str) -> PathBuf {
    PathBuf::from(path.replace('\\', "/"))
}

/// Normalize a path string to the stored workspace-key form: cwd-relative,
/// forward-slash separated, with no leading `./` or `/`.
pub fn normalize_workspace_key(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let mut trimmed = replaced.as_str();
    while trimmed.starts_with("./") {
        trimmed = &trimmed[2..];
    }
    while trimmed.starts_with('/') {
        trimmed = &trimmed[1..];
    }
    trimmed.to_string()
}

/// Normalize a relative path to the stored index-key form.
pub(crate) fn normalize_relative_path(path: &Path) -> String {
    normalize_workspace_key(&path.to_string_lossy())
}

/// Compute a workspace-relative key for a user or walker path. This mirrors the
/// indexer's key contract for in-workspace files: canonicalize the path and root,
/// strip the root, then normalize to forward slashes. If the path cannot be
/// relativized to the workspace root, fall back to the input path's normalized form.
pub fn workspace_relative_key(path: &Path, workspace_root: &Path) -> String {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    let canonical_path = canonicalize_path_lenient(&absolute_path);
    let canonical_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let relative = canonical_path
        .strip_prefix(&canonical_root)
        .or_else(|_| absolute_path.strip_prefix(workspace_root))
        .unwrap_or(path);
    normalize_relative_path(relative)
}

/// Compute the stored index key for `entry_path`: the path relative to the
/// (canonicalized) current working directory, normalized to forward slashes. Falls
/// back to the leading-slash-stripped absolute path when the file is outside the cwd
/// (e.g. an absolute walk root in tests) — byte-identical to the pre-Child-04 logic,
/// so incremental delete-detection (which keys on this string) is preserved.
pub(crate) fn relative_index_path(entry_path: &Path, abs_cwd: &Path) -> String {
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
pub(crate) fn stored_index_key(path: &Path, abs_cwd: &Path) -> String {
    let abs_path = canonicalize_path_lenient(path);
    let rel = abs_path.strip_prefix(abs_cwd).unwrap_or(path);
    normalize_relative_path(rel)
}

/// Produce a workspace-relative display path that matches the codemap snapshot's
/// `file_path` keys for an entry yielded by [`build_walker`] rooted at `root`. The
/// indexer keys each file by `canonicalize().strip_prefix(canon cwd)`
/// ([`relative_index_path`]), so mirror that: strip the canonical root from the
/// canonical entry; fall back to stripping the raw root (the walker yields entries
/// rooted at `root` as passed) so the keys line up whether or not the cwd is a symlink.
/// Distinct from [`relative_index_path`]: this keeps the two-step (canonical-then-raw)
/// fallback and does only the backslash→slash replacement (no leading `./`/`/` strip),
/// so the caller-scan display path stays byte-identical.
pub(crate) fn workspace_display_path(
    entry_path: &Path,
    canonical_root: &Path,
    raw_root: &Path,
) -> String {
    let canonical_path = entry_path
        .canonicalize()
        .unwrap_or_else(|_| entry_path.to_path_buf());
    let relative = canonical_path
        .strip_prefix(canonical_root)
        .or_else(|_| entry_path.strip_prefix(raw_root))
        .unwrap_or(entry_path);
    relative.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_workspace_relative_key_normalizes_user_path_spellings() {
        let root = std::env::current_dir().unwrap();
        assert_eq!(
            workspace_relative_key(Path::new("./src/lib.rs"), &root),
            "src/lib.rs"
        );
        assert_eq!(
            workspace_relative_key(Path::new(".\\src\\lib.rs"), &root),
            "src/lib.rs"
        );
    }
}
