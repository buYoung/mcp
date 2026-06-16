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

/// The MCP `initialize` `instructions` string, also returned verbatim by the
/// `initial_instructions` tool so the guidance reaches clients that do not surface
/// server-level instructions (e.g. Codex, which omits the `initialize` instructions but
/// reads tool descriptions and calls the tool). Single source; co-located with
/// [`list_tools`] so prose and tool schemas stay in sync.
pub fn instructions() -> &'static str {
    "Five code-navigation tools. Pick by intent, not order.\n* `search`: use first for symbols, definitions, concepts, quoted strings, errors, config defaults, or 'where is X done?' questions. BM25 over indexed symbols, docstrings, and string literals; identifier splitting + ranking find what exact grep misses. Top files include compact `match_reason`, ambiguity hints, `read_suggestion`, line snippets, and matched function depth-1 callers/callees. Set `caller_context=false` to disable. More matches appear as ranked one-line list. Output capped by `search_detail_byte_cap`; if partial, narrow query or read listed ranges. Snippet lines and caller `file:line` exact — cite directly, do not re-read to confirm. Only caller→definition attribution is name-match approximate.\n* `grep`: use first for exact enumeration: confirmed names, regex, comments, non-code files, or just-edited files with no index lag. More pages include `next_offset`.\n* `overview`: use to orient in unfamiliar code or get symbol line ranges before `read`. Root gives bounded repo map with compact file/symbol groups. Folder path narrows. File path gives exact symbol ranges.\n* `read`: read file contents. Prefer `offset`/`limit` windows from `search.read_suggestion` or `overview`, especially for large files.\n* `find`: locate files by glob and confirm exact paths.\nTypical flow: `search` finds symbol + call context → `read` exact range. Use `grep` when every literal occurrence matters or file was just edited."
}

/// The MCP `tools/list` result: the six tool schemas (name, description, read-only
/// annotations, and input schema), including `initial_instructions`. Co-located with
/// [`instructions`] so schema and prose stay in sync. Tool descriptions intentionally
/// carry imperative usage guidance; the cross-tool flow lives in [`instructions`] and the
/// `initial_instructions` tool result.
pub fn list_tools() -> Value {
    serde_json::json!({
                "tools": [
                    {
                        "name": "initial_instructions",
                        "description": "REQUIRED FIRST CALL: before using search/grep/read/find/overview, call this once with no arguments to load the codemap-search usage rules and the recommended navigation flow.",
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "overview",
                        "description": "Use for repo orientation or to get a symbol's line range before read. No path returns a bounded repo-root map with recursive file/symbol counts per directory plus top files with grouped significant symbol names, but no line ranges. Folder path narrows to that folder. File path lists symbols with exact line ranges. To get a symbol range before reading, run overview, then read that window. For a named symbol or concept, prefer search.",
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
                        "description": "Default first tool for symbols, definitions, concepts, quoted strings, errors, config defaults, or 'where is X done?' questions. BM25 over indexed symbols, docstrings, and string literals; identifier splitting finds what exact patterns miss. Top files show line-numbered snippets, symbol ranges, read_suggestion, and matched function depth-1 callers/callees. Set caller_context=false to skip call context. Remaining matches are ranked one-line results. Snippet lines and caller file:line are exact; caller→definition attribution is name-match approximate. Cite returned lines, then read the suggested range. Do not re-read to confirm or re-search returned info. If partial via search_detail_byte_cap, narrow the query or read listed ranges.",
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
                        "description": "Read one file as N\u{2192}content lines. Prefer offset/limit when a line range is known, the file is large, or the file is unfamiliar. Get ranges from search read_suggestion or overview, then read that window. No-limit reads of large files are refused with a narrower-window error. Do not read a whole large file just to find one symbol.",
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
                        "description": "Locate files by glob, e.g. '**/*.rs', to confirm exact files exist. Results are mtime-sorted and capped. Respects .gitignore and .codemapignore; set include_ignored to bypass. A pattern without slash, e.g. '*rpc*', matches filenames only, never directory segments. To match a path component, use '**/*rpc*' or '**/rpc/**'.",
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
                        "description": "Use for exhaustive exact literal/regex matches on disk: confirmed names, regexes, comments, non-code files, or just-edited files with no index lag. Params mirror Claude Code Grep. If a scoped pattern returns 0, do not rerun unchanged. Check path, glob, type, -i, multiline, or include_ignored; or switch to search for semantic/identifier-split lookup. Broaden regex only when intended. Partial output gives total count and next_offset; page with offset. Respects .gitignore and .codemapignore; set include_ignored to bypass.",
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
