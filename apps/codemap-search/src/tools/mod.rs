//! Self-contained `read` / `find` / `grep` primitives backed by ripgrep's library
//! crates (`grep`, `ignore`, `globset`) — no external `rg` binary, no system `rg`.
//!
//! Shared filesystem-walk infrastructure (the ignore-aware walker, junk-dir excludes,
//! source-extension set, path canonicalization/containment, and index path keys) now
//! lives in [`crate::workspace`]. This module keeps only the tool-local glob matching
//! and MCP argument-coercion helpers.

pub mod find;
pub mod grep;
pub mod overview;
pub mod read;
pub mod search;

use crate::index::EngineSupervisor;
use serde_json::Value;
use std::path::Path;

/// Borrowed request context for the snapshot-backed tools (`search`/`overview`). The MCP
/// dispatch arm runs the `EngineSupervisor` lifecycle (`ensure_alive`/`trigger_refresh`)
/// itself, then hands the tool body an immutable engine borrow plus the parsed arguments;
/// the tool reads the committed snapshot through `engine` and never needs `&mut`. The
/// live-filesystem tools (`read`/`find`/`grep`) keep their plain `fn(&Value)` shape.
pub struct ToolContext<'a> {
    pub engine: &'a EngineSupervisor,
    pub arguments: &'a Value,
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

/// Normalize an argument key for tolerant alias matching: lowercase, then drop every `_`
/// and `-`. So `startLine`, `StartLine`, `start-line`, and `start_line` all collapse to
/// `startline`, and `filePath`/`file_path` to `filepath`. This is the comparison form used
/// by [`get_arg`] when an exact key spelling is absent — it lets a camel/kebab/Pascal variant
/// resolve to its canonical snake_case parameter instead of being silently dropped (the live
/// benchmark defect where a camel `startLine` was ignored and the whole file rendered).
fn normalize_arg_key(key: &str) -> String {
    key.chars()
        .filter(|c| *c != '_' && *c != '-')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Look up an argument by `key`, tolerating case/`_`/`-` spelling variants. **An exact key
/// match always wins** (the canonical-name rule): only when no key equals `key` verbatim does
/// it fall back to the first arg whose [`normalize_arg_key`] form matches `key`'s. So when both
/// `start_line` (canonical) and a normalized-equal variant are present, the canonical wins; an
/// unambiguous lone variant (`startLine`) still resolves. Among multiple normalized matches the
/// object's first key wins — deterministic, though such a collision is not expected in practice.
pub(crate) fn get_arg<'a>(args: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if let Some(value) = args.get(key) {
        return Some(value);
    }
    let table = args.as_object()?;
    let target = normalize_arg_key(key);
    table
        .iter()
        .find(|(k, _)| normalize_arg_key(k) == target)
        .map(|(_, v)| v)
}

/// Read a boolean argument with a default, tolerating alias spellings via [`get_arg`].
pub(crate) fn arg_bool(args: &serde_json::Value, key: &str, default: bool) -> bool {
    get_arg(args, key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Coerce a JSON value to `usize`, accepting both a real `Value::Number` (`as_u64`) and a
/// string-typed numeric like `"228"` (trim, then `parse::<u64>()`). Agents routinely send
/// numerics as JSON strings — observed live in benchmark transcripts, where a string-typed
/// `start_line`/`offset` was silently dropped and the whole file rendered. Anything that is
/// neither a number nor a parseable numeric string yields `None`.
pub(crate) fn lenient_usize(value: &serde_json::Value) -> Option<usize> {
    if let Some(n) = value.as_u64() {
        return Some(n as usize);
    }
    value
        .as_str()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|n| n as usize)
}

/// Read an unsigned-integer argument with a default. Uses [`get_arg`] (alias-tolerant) and
/// [`lenient_usize`] so a string-typed numeric (e.g. grep's `head_limit: "10"`) or a camel/
/// kebab alias coerces instead of falling back to the default.
pub(crate) fn arg_usize(args: &serde_json::Value, key: &str, default: usize) -> usize {
    get_arg(args, key)
        .and_then(lenient_usize)
        .unwrap_or(default)
}

/// Read a required string argument, erroring with -32602 when absent/non-string. Alias-tolerant
/// via [`get_arg`] so a camel/kebab spelling resolves to its canonical parameter.
pub(crate) fn arg_required_str<'a>(
    args: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, (i64, String)> {
    get_arg(args, key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, format!("Missing required '{key}' parameter")))
}

/// The MCP `initialize` `instructions` string. Co-located with [`list_tools`] so the
/// product-surface prose and the tool schemas cannot drift apart. Byte-identical to the
/// value the server has always returned — do not edit without intending a behavior change.
pub fn instructions() -> &'static str {
    "Five code-navigation tools; split by purpose, not by order.\n- search: first move for symbols, definitions, concepts, and quoted strings ('where is the auth token refreshed?', an error message, a config default). BM25 over indexed symbols/docstrings/string literals — identifier splitting and ranking find what exact grep matching misses. Top-ranked files render detail with compact match_reason/ambiguity/read_suggestion hints, line-numbered snippets, and each matched function's depth-1 callers/callees (on by default; caller_context=false to disable); further matches follow as a ranked one-line list. Output is capped by search_detail_byte_cap; when partial, narrow the query or read the listed ranges. Snippet line numbers and caller file:line positions are exact — cite them directly instead of re-reading to confirm; only caller→definition attribution is name-match approximate.\n- grep: first move for exact-pattern enumeration — every occurrence of a name you already confirmed, regex matches, comments, non-code files, just-edited files (no index lag). Pages with more results include next_offset.\n- overview: orient in unfamiliar code; root is bounded and includes compact file-symbol groups, folder path narrows, file path gives exact line ranges.\n- read / find: file contents / glob lookup; read uses offset/limit windows for large files.\nTypical flow: search to locate a symbol and see its call context, then read the exact range; grep when you need every literal occurrence or just-edited files."
}

/// The MCP `tools/list` result: the five tool schemas (name, description, read-only
/// annotations, and input schema). Co-located with [`instructions`] so schema and prose
/// stay in sync. Byte-identical to the value the server has always returned.
pub fn list_tools() -> Value {
    serde_json::json!({
                "tools": [
                    {
                        "name": "overview",
                        "description": "Hierarchical codemap. No path: bounded repo-root map with compact file-symbol groups; folder path narrows; file path shows that file's symbols with line ranges.",
                        // All five tools are read-only over the local workspace. Declaring it
                        // matters: clients gate approval on these hints (Codex auto-cancels
                        // un-annotated tools in non-interactive runs, and prompts per call in
                        // interactive ones).
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Empty/omitted = repo root overview; a folder path narrows; a file path shows that file's symbol details. Aliases 'file_path'/'file'/'query' are also accepted." },
                                "format": { "type": "string", "description": "Optional output format (e.g. 'llms-txt'); root llms-txt output is bounded." }
                            }
                        }
                    },
                    {
                        "name": "search",
                        "description": "Symbol, definition, concept, and quoted-string lookup: BM25 keyword search over indexed symbols, docstrings, and string literals (error messages, config defaults); identifier splitting and ranking recover what exact grep matching misses. Top-ranked files render in detail — compact match_reason/ambiguity/read_suggestion hints, symbols with line ranges and line-numbered snippets, and each matched function annotated with its depth-1 callers/callees (on by default; positions exact, attribution name-match approximate) — and remaining matches follow as a ranked one-line list. Output is capped by search_detail_byte_cap; when partial, narrow the query or read the listed ranges. Cite line numbers directly from the response.",
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "caller_context": { "type": "boolean", "description": "Annotate each matched function's detail snippet with its depth-1 callers/callees (approximate, name-match only). Detail view only; on by default (config caller_context_default) — pass false to disable." }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "read",
                        "description": "Read one file's contents as '   N\u{2192}content' lines; offset/limit pages large files and oversized errors suggest a narrower window.",
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Workspace-relative path to the file. Aliases 'path'/'file'/'query' are also accepted." },
                                "offset": { "type": "integer", "description": "1-indexed start line (default 1). Aliases: 'start_line'/'start'." },
                                "limit": { "type": "integer", "description": "Max lines to read from offset. The 1-based inclusive 'end_line'/'end' aliases derive limit relative to the effective offset. String-typed numerics (e.g. \"228\") are accepted." }
                            },
                            "required": ["file_path"]
                        }
                    },
                    {
                        "name": "find",
                        "description": "Locate files by glob (e.g. '**/*.rs') to confirm exactly which files exist. mtime-sorted, capped. Respects .gitignore and .codemapignore; set include_ignored to bypass. A pattern without a slash (e.g. '*rpc*') matches only the filename, never a directory segment — to match a path component use '**/*rpc*' or '**/rpc/**'.",
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Glob pattern, ripgrep -g style: a slash-less glob like '*.rs' matches the basename at any depth; '**' crosses directories, '*'/'?' do not; '{a,b}' expands and '!' negates." },
                                "path": { "type": "string", "description": "Base directory to search (default '.')." },
                                "include_ignored": { "type": "boolean", "description": "Bypass .gitignore/.codemapignore (default false)." }
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "grep",
                        "description": "Exact literal/regex match over files on disk; parameters mirror Claude Code's Grep. Partial output includes total count and next_offset. Respects .gitignore/.codemapignore; set include_ignored to bypass.",
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Regex (or literal) to search for." },
                                "path": { "type": "string", "description": "Base directory to search (default '.')." },
                                "glob": { "type": "string", "description": "Filter files by glob, ripgrep -g style: a slash-less glob like '*.rs' matches at any depth; a glob with a slash is matched relative to path; multiple globs split on whitespace/comma; '!' negates and '{a,b}' expands. Aliases 'include'/'file_pattern' are also accepted." },
                                "type": { "type": "string", "description": "Filter by ripgrep file type (e.g. 'rust', 'py', 'ts')." },
                                "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"], "description": "Default 'content' — matching lines as 'file:line:text' with line numbers (via -n). Use 'files_with_matches' for a cheap file-list enumeration, or 'count' for per-file match counts." },
                                "-i": { "type": "boolean", "description": "Case-insensitive (default false)." },
                                "-n": { "type": "boolean", "description": "Show line numbers in content mode (default true)." },
                                "-A": { "type": "integer", "description": "Lines of context after each match." },
                                "-B": { "type": "integer", "description": "Lines of context before each match." },
                                "-C": { "type": "integer", "description": "Lines of context before and after (overrides -A/-B)." },
                                "multiline": { "type": "boolean", "description": "Allow matches to span lines (default false)." },
                                "head_limit": { "type": "integer", "description": "Max results (default 250; 0 = unlimited)." },
                                "offset": { "type": "integer", "description": "0-indexed result offset for pagination (default 0)." },
                                "include_ignored": { "type": "boolean", "description": "Bypass .gitignore/.codemapignore (default false)." }
                            },
                            "required": ["pattern"]
                        }
                    }
                ]
    })
}
