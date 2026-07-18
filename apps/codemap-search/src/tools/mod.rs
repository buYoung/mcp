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
    pub active_workspace_scope: Option<&'a str>,
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
    let replaced = rest.replace('\\', "/");
    let mut body = replaced.as_str();
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

/// The MCP `initialize` `instructions` string: deliberately minimal. It only points the
/// client at the `initial_instructions` tool, which returns the full navigation guidance
/// ([`instructions`]). This keeps the server-level string short while still reaching
/// clients that hide or compress `initialize` instructions (e.g. Codex), since they read
/// tool descriptions and call the tool. The prose lives in `instructions/server.md`,
/// embedded at compile time so the binary stays self-contained; `trim_end` drops the
/// file's trailing newline.
pub fn server_instructions() -> &'static str {
    include_str!("instructions/server.md").trim_end()
}

/// The full navigation guidance returned by the `initial_instructions` tool. The prose
/// lives in `instructions/navigation.md` (kept beside the per-tool description files under
/// `instructions/`), embedded with `include_str!` so prose and tool schemas stay in sync
/// in one directory; `trim_end` drops the file's trailing newline.
pub fn instructions() -> String {
    let text = if crate::codemap::looks_like_monorepo_workspace() {
        include_str!("instructions/navigation.monorepo.md")
    } else {
        include_str!("instructions/navigation.md")
    };
    text.trim_end().to_string()
}

/// Compose the monorepo bootstrap response from the existing navigation guidance and the root
/// overview rendered from the current index snapshot. The caller owns lifecycle handling and
/// invokes this only for monorepos, preserving the non-monorepo response unchanged.
pub fn instructions_with_root_overview(root_overview: &str) -> String {
    format!("{}\n\n{}", instructions(), root_overview)
}

fn filesystem_tool_description(
    base_description: &str,
    policy: crate::config::FilesystemPermissionPolicy,
    allowed_roots: &[std::path::PathBuf],
) -> String {
    let mut description = format!(
        "{base_description}\n\nCurrent permission: {}.",
        policy.as_str()
    );
    if matches!(
        policy,
        crate::config::FilesystemPermissionPolicy::AllowedRoots
    ) && !allowed_roots.is_empty()
    {
        let allowed_roots_list = allowed_roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        description.push_str(" Allowed roots: ");
        description.push_str(&allowed_roots_list);
        description.push('.');
    }
    description
}

/// The MCP `tools/list` result: the six tool schemas (name, description, read-only
/// annotations, and input schema), including `initial_instructions`. Base tool
/// `description` prose is embedded from `instructions/tools/<name>.md` via `include_str!`;
/// live filesystem tools append their currently configured permission policy. The
/// input-schema property descriptions stay inline beside their property. Tool descriptions
/// intentionally carry imperative usage guidance; the cross-tool flow lives in
/// [`instructions`] and the `initial_instructions` tool result.
pub fn list_tools() -> Value {
    let config = crate::config::get();
    let permissions = &config.filesystem_permissions;
    let is_monorepo = crate::codemap::looks_like_monorepo_workspace();
    let initial_instructions_description = if is_monorepo {
        include_str!("instructions/tools/initial_instructions.monorepo.md").trim_end()
    } else {
        include_str!("instructions/tools/initial_instructions.md").trim_end()
    };
    let overview_description = if is_monorepo {
        include_str!("instructions/tools/overview.monorepo.md").trim_end()
    } else {
        include_str!("instructions/tools/overview.md").trim_end()
    };
    let search_description = if is_monorepo {
        include_str!("instructions/tools/search.monorepo.md").trim_end()
    } else {
        include_str!("instructions/tools/search.md").trim_end()
    };
    let read_description = filesystem_tool_description(
        include_str!("instructions/tools/read.md").trim_end(),
        permissions.read,
        &permissions.allowed_roots,
    );
    let find_description = filesystem_tool_description(
        include_str!("instructions/tools/find.md").trim_end(),
        permissions.find,
        &permissions.allowed_roots,
    );
    let grep_description = filesystem_tool_description(
        include_str!("instructions/tools/grep.md").trim_end(),
        permissions.grep,
        &permissions.allowed_roots,
    );
    let mut search_properties = serde_json::json!({
        "query": { "type": "string" },
        "caller_context": { "type": "boolean", "description": "Annotate each matched function's detail snippet with its depth-1 callers/callees. Attribution is approximate unless explicitly marked tree-sitter precise. Detail view only; on by default (config caller_context_default) — pass false to disable." },
        "language_hint": { "type": "string", "description": "Optional query-language hint for cross-language ranking priors (examples: 'typescript', 'rust'). Omit to keep existing language-agnostic behavior." },
        "extension_hint": { "type": "string", "description": "Optional query-extension hint for same-extension ranking prior (examples: 'ts', '.rs'). Omit to keep existing behavior." }
    });
    if is_monorepo {
        if let Some(properties) = search_properties.as_object_mut() {
            properties.insert(
                "workspace_scope".to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": "Optional monorepo scope named by root overview, such as conventional workspace 'apps/api' or top-level source root 'api'/'app'/'sdk'; a unique basename is accepted. Omit to use the active scope selected by overview. Use 'all' or '전체' for repo-wide search."
                }),
            );
        }
    }
    serde_json::json!({
                "tools": [
                    {
                        "name": "initial_instructions",
                        "description": initial_instructions_description,
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "overview",
                        "description": overview_description,
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
                        "description": search_description,
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": search_properties,
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "read",
                        "description": read_description,
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Workspace-relative path by default; configured filesystem permissions may allow absolute paths. Aliases 'path'/'file'/'query' are also accepted." },
                                "offset": { "type": "integer", "description": "1-indexed start line (default 1). Aliases: 'start_line'/'start'." },
                                "limit": { "type": "integer", "description": "Max lines to read from offset. The 1-based inclusive 'end_line'/'end' aliases derive limit relative to the effective offset. String-typed numerics (e.g. \"228\") are accepted." }
                            },
                            "required": ["file_path"]
                        }
                    },
                    {
                        "name": "find",
                        "description": find_description,
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Glob pattern, ripgrep -g style: a slash-less glob like '*.rs' matches the basename at any depth; '**' crosses directories, '*'/'?' do not; '{a,b}' expands and '!' negates." },
                                "path": { "type": "string", "description": "Base directory to search (default '.'); configured filesystem permissions may allow absolute paths." },
                                "include_ignored": { "type": "boolean", "description": "Bypass .gitignore/.codemapignore (default false)." }
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "grep",
                        "description": grep_description,
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Regex (or literal) to search for." },
                                "path": { "type": "string", "description": "Base directory to search (default '.'); configured filesystem permissions may allow absolute paths." },
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

#[cfg(test)]
mod tests {
    use super::normalize_glob_prefix;

    #[test]
    fn test_normalize_glob_prefix_accepts_windows_separators() {
        assert_eq!(normalize_glob_prefix(".\\src\\*.rs"), "src/*.rs");
        assert_eq!(normalize_glob_prefix("!.\\src\\*.rs"), "!src/*.rs");
    }
}
