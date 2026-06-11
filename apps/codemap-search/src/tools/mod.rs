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
pub const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "kt", "kts",
    // C and C++: `.h` is listed here and parsed with the C++ grammar (tolerant of plain C).
    "c", "h", "cpp", "cc", "cxx", "hpp", "hh", "hxx",
    // Assembly (GAS AT&T and Intel syntax): `S` (uppercase) is preprocessed GAS and must be
    // listed explicitly — extension comparison is case-sensitive.
    "s", "S", "asm",
];

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

// --- Shared gitignore-style glob matching (find + grep) --------------------------------

/// Strip a leading `./` (repeated) from a glob so `./src/*.rs` behaves as `src/*.rs`.
/// A leading `!` (gitignore negation) is preserved and the `./` after it is stripped too,
/// so `!./src/*.rs` normalizes to `!src/*.rs` (without this the negation would silently
/// fail to anchor and disable itself). ripgrep does not normalize `./`; codemap does as an
/// intentional ergonomic improvement.
pub(crate) fn normalize_glob_prefix(pattern: &str) -> String {
    let (bang, rest) = match pattern.strip_prefix('!') {
        Some(rest) => ("!", rest),
        None => ("", pattern),
    };
    let mut body = rest;
    while let Some(stripped) = body.strip_prefix("./") {
        body = stripped;
    }
    format!("{bang}{body}")
}

/// Split a Grep-style `glob` argument into individual gitignore patterns: whitespace
/// first, then split each brace-free whitespace token on commas (so `*.ts,*.tsx` →
/// two globs). Mirrors Claude Code Grep's `--glob` expansion. A token containing `{` is
/// kept intact so brace alternations survive. `find` does NOT use this — it passes its
/// single pattern verbatim, matching Claude Code Glob.
pub(crate) fn split_grep_globs(glob: &str) -> Vec<String> {
    let mut out = Vec::new();
    for whitespace_token in glob.split_whitespace() {
        // Keep a token intact only when it carries a complete brace alternation `{…}`
        // (Claude Code requires both `{` and `}`); otherwise comma-split it.
        if whitespace_token.contains('{') && whitespace_token.contains('}') {
            out.push(whitespace_token.to_string());
        } else {
            for comma_token in whitespace_token.split(',') {
                if !comma_token.is_empty() {
                    out.push(comma_token.to_string());
                }
            }
        }
    }
    out
}

/// A compiled gitignore-style glob matcher equivalent to `rg --glob <pattern>`: a
/// slash-less pattern matches the basename at **any depth**, `**` crosses directories,
/// `*`/`?` do not cross `/` within a segment, `{a,b}` brace-expands, and a leading `!`
/// negates (gitignore semantics, via `ignore::overrides`). When every supplied glob is a
/// negation, unmatched files are *included* (ripgrep treats `--glob '!x'` as "all but x");
/// when any positive glob exists, only matches pass. Shared by `find` and `grep` so both
/// observe identical semantics instead of re-deriving them.
pub struct GlobMatcher {
    overrides: ignore::overrides::Override,
    has_whitelist: bool,
}

impl GlobMatcher {
    /// Whether `rel_path` (relative to the matcher's base directory) matches the glob set.
    pub fn is_match(&self, rel_path: &Path) -> bool {
        match self.overrides.matched(rel_path, false) {
            ignore::Match::Ignore(_) => false,
            ignore::Match::Whitelist(_) => true,
            // No glob touched this path: included only when there is no positive glob to
            // satisfy (negation-only sets are "everything except"), excluded otherwise.
            ignore::Match::None => !self.has_whitelist,
        }
    }
}

/// Build a [`GlobMatcher`] from one or more patterns rooted at `base`. `./` prefixes are
/// normalized away first. `find` passes its single pattern; `grep` passes the
/// whitespace/comma-split set from [`split_grep_globs`]. Errors map to JSON-RPC -32602.
pub fn build_glob_matcher(base: &Path, patterns: &[String]) -> Result<GlobMatcher, (i64, String)> {
    let mut builder = ignore::overrides::OverrideBuilder::new(base);
    for pattern in patterns {
        let normalized = normalize_glob_prefix(pattern);
        if normalized.is_empty() || normalized == "!" {
            continue;
        }
        builder
            .add(&normalized)
            .map_err(|e| (-32602, format!("Invalid glob pattern '{pattern}': {e}")))?;
    }
    let overrides = builder
        .build()
        .map_err(|e| (-32602, format!("Invalid glob pattern set: {e}")))?;
    let has_whitelist = overrides.num_whitelists() > 0;
    Ok(GlobMatcher {
        overrides,
        has_whitelist,
    })
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
