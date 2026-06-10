use crate::index::SearcherHandle;
use crate::indexer::IndexerHandle;
use crate::watcher::{WatcherHandle, WatcherStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    // A JSON-RPC response carries either `result` or `error`, never both — omit the
    // unused member so success frames don't ship `"error": null` (and vice versa).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

pub struct McpServer {
    // Read-only search handle over the committed index (cloned Arc-backed reader). Indexing
    // happens off-thread, so the request loop never blocks on it.
    searcher: SearcherHandle,
    // Filesystem watcher (None when `watch = false` or the watch failed to start). Field
    // order is load-bearing: struct fields drop in declaration order, and the watcher
    // thread holds a sender clone into the indexer channel — `watcher` MUST be declared
    // (and thus dropped) BEFORE `indexer`, whose drop joins a recv loop that only ends
    // once ALL senders are gone. The opposite order deadlocks on shutdown.
    watcher: Option<WatcherHandle>,
    // Background indexer: fire-and-forget refresh trigger, warming/error status, and the
    // current codemap snapshot consumed by `overview`.
    indexer: IndexerHandle,
    // Watcher health flag: while healthy, the watcher keeps the index current and the
    // request-triggered refresh below is suppressed entirely. Stays unhealthy forever
    // when `watch = false` or the watch failed to start, which preserves the lazy path.
    watcher_status: Arc<WatcherStatus>,
    // Instant of the last refresh trigger. Within `config::index_staleness_ms` we skip
    // re-triggering so a burst of search/overview calls enqueues at most one refresh; the
    // indexer's own mtime diff keeps each pass incremental. A single field now suffices
    // because search and overview share one background refresh.
    last_refresh_trigger: Option<Instant>,
    // Automatic indexer restarts performed so far (see `maybe_restart_indexer`).
    indexer_restart_attempts: u32,
}

pub(crate) fn canonicalize_path_lenient(path: &std::path::Path) -> PathBuf {
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

fn is_safe_path(p: &str) -> bool {
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let target = cwd.join(p);

    let mut resolved = PathBuf::new();
    for component in target.components() {
        match component {
            std::path::Component::ParentDir => {
                resolved.pop();
            }
            std::path::Component::CurDir => {}
            _ => {
                resolved.push(component.as_os_str());
            }
        }
    }

    let resolved_canonical = canonicalize_path_lenient(&resolved);
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    resolved_canonical.starts_with(&cwd_canonical)
}

/// Cap a code snippet at `max_lines` AND `max_bytes`, appending an elision marker when
/// truncated, so the `search` detail view never emits a 1,000-line symbol body — nor a
/// single multi-hundred-KB minified line (the line cap alone leaves byte size unbounded).
fn cap_snippet(snippet: &str, max_lines: usize, max_bytes: usize) -> String {
    let lines: Vec<&str> = snippet.lines().collect();
    let mut out = if lines.len() > max_lines {
        let shown = lines[..max_lines].join("\n");
        format!("{shown}\n… ({} more lines)", lines.len() - max_lines)
    } else {
        snippet.to_string()
    };
    if max_bytes > 0 && out.len() > max_bytes {
        // Truncate at a UTF-8 char boundary, then mark the cut.
        let mut end = max_bytes.min(out.len());
        while end > 0 && !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("\n… (truncated)");
    }
    out
}

/// Truncate a matched literal to `max_len` characters with an ellipsis, so a long
/// SQL/template literal can't bloat the `search` detail view.
fn truncate_literal(literal: &str, max_len: usize) -> String {
    if literal.chars().count() > max_len {
        let truncated: String = literal.chars().take(max_len).collect();
        format!("{truncated}…")
    } else {
        literal.to_string()
    }
}

fn get_code_snippet(file_path: &str, range: &crate::parser::CodeRange) -> String {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(range.end_line, lines.len());
            if start < end {
                return lines[start..end].join("\n");
            }
        }
    }
    String::new()
}

impl McpServer {
    pub fn new(
        searcher: SearcherHandle,
        watcher: Option<WatcherHandle>,
        indexer: IndexerHandle,
        watcher_status: Arc<WatcherStatus>,
    ) -> Self {
        Self {
            searcher,
            watcher,
            indexer,
            watcher_status,
            last_refresh_trigger: None,
            indexer_restart_attempts: 0,
        }
    }
}

struct LimitedLineReader<R> {
    reader: R,
    buffer: Vec<u8>,
    max_line_length: usize,
}

impl<R: tokio::io::AsyncRead + Unpin> LimitedLineReader<R> {
    fn new(reader: R, max_line_length: usize) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            max_line_length,
        }
    }

    async fn next_line(&mut self) -> Result<Option<String>, String> {
        let mut byte_buf = [0u8; 1024];
        loop {
            if let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes = self.buffer.drain(..=pos).collect::<Vec<u8>>();
                let mut len = line_bytes.len();
                if len > 0 && line_bytes[len - 1] == b'\n' {
                    len -= 1;
                }
                if len > 0 && line_bytes[len - 1] == b'\r' {
                    len -= 1;
                }
                let line_str = String::from_utf8(line_bytes[..len].to_vec())
                    .map_err(|e| format!("Invalid UTF-8: {}", e))?;
                return Ok(Some(line_str));
            }

            use tokio::io::AsyncReadExt;
            let n = self
                .reader
                .read(&mut byte_buf)
                .await
                .map_err(|e| format!("Read error: {}", e))?;
            if n == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    let line_bytes = std::mem::take(&mut self.buffer);
                    let mut len = line_bytes.len();
                    if len > 0 && line_bytes[len - 1] == b'\n' {
                        len -= 1;
                    }
                    if len > 0 && line_bytes[len - 1] == b'\r' {
                        len -= 1;
                    }
                    let line_str = String::from_utf8(line_bytes[..len].to_vec())
                        .map_err(|e| format!("Invalid UTF-8: {}", e))?;
                    return Ok(Some(line_str));
                }
            }
            self.buffer.extend_from_slice(&byte_buf[..n]);
            if self.buffer.len() > self.max_line_length {
                return Err("Max line length exceeded".to_string());
            }
        }
    }
}

/// Cap on automatic indexer restarts per server process: enough to absorb sporadic
/// failures, finite so a deterministically-crashing pass (e.g. a parser bug tripped by
/// one specific file on every walk) cannot respawn-loop forever. Past the cap the
/// existing "results are frozen" notice stands and a server restart is required.
const MAX_INDEXER_RESTART_ATTEMPTS: u32 = 5;

impl McpServer {
    /// Auto-recover a dead indexer thread (config `indexer_auto_restart`, default true):
    /// rebuild the engine, respawn the indexer, and re-attach the watcher, so one panic
    /// does not freeze results for the rest of the session. Runs at most once per
    /// request, only when death is actually observed, and never past
    /// [`MAX_INDEXER_RESTART_ATTEMPTS`]. `TantivySearchEngine::new` rebuilds a corrupt
    /// index directory, so a crash caused by index corruption heals instead of recurring.
    fn maybe_restart_indexer(&mut self) {
        if !crate::config::get().indexer_auto_restart || !self.indexer.is_dead() {
            return;
        }
        if self.indexer_restart_attempts >= MAX_INDEXER_RESTART_ATTEMPTS {
            return; // exhausted — the dead-indexer notice keeps reporting frozen results
        }
        self.indexer_restart_attempts += 1;
        tracing::warn!(
            "indexer thread died — auto-restarting ({}/{})",
            self.indexer_restart_attempts,
            MAX_INDEXER_RESTART_ATTEMPTS
        );

        // Tear the watcher down FIRST: its thread holds a sender clone into the dead
        // channel, and its drop flips the shared health flag off — done before the
        // respawn below so it cannot clobber the new watcher's healthy=true.
        self.watcher = None;

        let engine =
            match crate::index::TantivySearchEngine::new(&crate::config::get().index_path) {
                Ok(engine) => engine,
                Err(e) => {
                    tracing::warn!("indexer auto-restart failed to reopen the index: {e}");
                    return; // next request retries (attempt already counted)
                }
            };
        self.searcher = engine.searcher_handle();
        // Old IndexerHandle drops on assignment: its thread is already dead, so the
        // join returns immediately — no shutdown-order hazard here.
        self.indexer = crate::indexer::spawn_indexer(engine);

        if crate::config::get().watch {
            if let Ok(cwd) = std::env::current_dir() {
                self.watcher = crate::watcher::spawn_watcher(
                    &cwd,
                    self.indexer.command_sender(),
                    Arc::clone(&self.watcher_status),
                );
            }
        }
    }

    /// Enqueue a background index refresh unless one was already triggered within the
    /// staleness window. Fire-and-forget — never blocks the request on indexing. Shared by
    /// search and overview, which both serve the indexer's published snapshot.
    ///
    /// While the filesystem watcher is healthy this is a no-op: the watcher already keeps
    /// the index current, so suppressing the request-triggered full walk here is what
    /// actually removes the per-request O(repo) walk during active use. The fallback below
    /// runs only when the watcher is absent (`watch = false`), failed to start, or died.
    fn maybe_trigger_refresh(&mut self) {
        // A dead indexer disarms the suppression: falling through lets `trigger_refresh`
        // observe the disconnected channel and raise the "results are frozen" notice,
        // which a healthy-looking watcher would otherwise mask indefinitely.
        if self.watcher_status.is_healthy() && !self.indexer.is_dead() {
            return;
        }
        let staleness = Duration::from_millis(crate::config::get().index_staleness_ms);
        let is_fresh = self
            .last_refresh_trigger
            .is_some_and(|t| t.elapsed() < staleness);
        if !is_fresh {
            self.indexer.trigger_refresh();
            self.last_refresh_trigger = Some(Instant::now());
        }
    }

    pub async fn run(&mut self) -> Result<(), String> {
        let stdin = tokio::io::stdin();
        let mut reader = LimitedLineReader::new(stdin, 10 * 1024 * 1024 + 100 * 1024);
        let mut stdout = tokio::io::stdout();

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    let req: JsonRpcRequest = match serde_json::from_str(&line) {
                        Ok(r) => r,
                        Err(e) => {
                            let err_resp = JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                result: None,
                                error: Some(serde_json::json!({
                                    "code": -32700,
                                    "message": format!("Parse error: {}", e)
                                })),
                                id: None,
                            };
                            if let Ok(resp_str) = serde_json::to_string(&err_resp) {
                                let _ =
                                    stdout.write_all(format!("{}\n", resp_str).as_bytes()).await;
                                let _ = stdout.flush().await;
                            }
                            continue;
                        }
                    };

                    // JSON-RPC notifications carry no `id` and MUST receive no response.
                    if req.id.is_none() {
                        continue;
                    }

                    let response_result = self.handle_request(&req.method, req.params.as_ref());

                    let resp = match response_result {
                        Ok(res_val) => JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: Some(res_val),
                            error: None,
                            id: req.id,
                        },
                        Err((code, msg)) => JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: None,
                            error: Some(serde_json::json!({
                                "code": code,
                                "message": msg
                            })),
                            id: req.id,
                        },
                    };

                    if let Ok(resp_str) = serde_json::to_string(&resp) {
                        let _ = stdout.write_all(format!("{}\n", resp_str).as_bytes()).await;
                        let _ = stdout.flush().await;
                    }
                }
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn handle_request(
        &mut self,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                // Echo the client's requested protocolVersion when we support it,
                // otherwise fall back to our newest supported version (MCP negotiation).
                const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
                    &["2025-06-18", "2025-03-26", "2024-11-05"];
                let protocol_version = params
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .filter(|v| SUPPORTED_PROTOCOL_VERSIONS.contains(v))
                    .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
                Ok(serde_json::json!({
                    "protocolVersion": protocol_version,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "codemap-search-server",
                        "version": "0.1.0"
                    },
                    "instructions": "Five code-navigation tools; pick by what you already know — no fixed order.\n- grep: first move when you know the exact identifier, string, or error message (not a last resort). Always current; sees comments and non-code files.\n- search: first move when you only know the concept, can't write a reliable pattern, or grep returned zero hits or noise. Once it names the symbol, grep its uses.\n- overview: orient in unfamiliar code; before reading a large file, overview it to get the exact line range.\n- read / find: file contents / glob lookup.\nIterating grep -> read is a normal, effective loop."
                }))
            }
            "ping" => Ok(serde_json::json!({})),
            "tools/list" => Ok(serde_json::json!({
                "tools": [
                    {
                        "name": "overview",
                        "description": "Hierarchical codemap. No path: repo-root map with file/symbol counts; folder path: narrows; file path: that file's symbols with line ranges.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Empty/omitted = repo root overview; a folder path narrows; a file path shows that file's symbol details." },
                                "format": { "type": "string", "description": "Optional output format (e.g. 'llms-txt')." }
                            }
                        }
                    },
                    {
                        "name": "search",
                        "description": "BM25 keyword search over indexed symbols and docstrings; identifier splitting and ranking recover what exact grep matching misses. Returns a codemap when many files match, per-file symbols with line ranges when few.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "read",
                        "description": "Read one file's contents as '   N\u{2192}content' lines; offset/limit pages large files.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Workspace-relative path to the file." },
                                "offset": { "type": "integer", "description": "1-indexed start line (default 1)." },
                                "limit": { "type": "integer", "description": "Max lines to read from offset." }
                            },
                            "required": ["file_path"]
                        }
                    },
                    {
                        "name": "find",
                        "description": "Locate files by glob (e.g. '**/*.rs') to confirm exactly which files exist. mtime-sorted, capped. Respects .gitignore and .codemapignore; set include_ignored to bypass.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Glob pattern; '**' crosses directories, '*'/'?' do not." },
                                "path": { "type": "string", "description": "Base directory to search (default '.')." },
                                "include_ignored": { "type": "boolean", "description": "Bypass .gitignore/.codemapignore (default false)." }
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "grep",
                        "description": "Exact literal/regex match over files on disk; parameters mirror Claude Code's Grep. Respects .gitignore/.codemapignore; set include_ignored to bypass.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Regex (or literal) to search for." },
                                "path": { "type": "string", "description": "Base directory to search (default '.')." },
                                "glob": { "type": "string", "description": "Filter files by glob (e.g. '*.rs')." },
                                "type": { "type": "string", "description": "Filter by ripgrep file type (e.g. 'rust', 'py', 'ts')." },
                                "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"], "description": "Default 'files_with_matches'." },
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
            })),
            "tools/call" => {
                let params = params.ok_or_else(|| (-32602, "Missing params".to_string()))?;
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (-32602, "Missing tool name".to_string()))?;
                let default_args = serde_json::Value::Object(serde_json::Map::new());
                let arguments = params.get("arguments").unwrap_or(&default_args);

                match name {
                    "search" => {
                        let query = arguments
                            .get("query")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| (-32602, "Missing query parameter".to_string()))?;

                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced by the staleness
                        // window), then search the current committed snapshot immediately
                        // — the response never blocks on indexing. A queued trigger
                        // coalesces bursts; the indexer's mtime diff keeps each pass
                        // incremental.
                        self.maybe_restart_indexer();
                        self.maybe_trigger_refresh();

                        let results = self
                            .searcher
                            .search(query, 100)
                            .map_err(|e| (-32603, format!("Search error: {}", e)))?;

                        // Result-branch threshold: at or below it, return file details;
                        // above it, return a codemap overview. Config-driven (Child 05),
                        // default 5.
                        let result_branch_threshold = crate::config::get().result_threshold;

                        let mut text = String::new();
                        // While the initial background index builds, results can be empty or
                        // partial — say so, and point at the always-live tools meanwhile.
                        if self.indexer.is_dead() {
                            text.push_str(
                                "_Background indexer stopped — search results are frozen at the last index and may be stale; restart the server to recover. read/find/grep stay live._\n\n",
                            );
                        } else if self.indexer.is_warming() {
                            text.push_str(
                                "_Index is warming up (initial background indexing) — results may be empty or partial; retry shortly, or use grep/find for live results._\n\n",
                            );
                        } else if let Some(err) = self.indexer.last_error() {
                            text.push_str(&format!(
                                "_Last background index refresh failed: {err} — results may be stale._\n\n"
                            ));
                        }
                        if results.len() > result_branch_threshold {
                            // Codemap overview: matched files + the symbols that match the
                            // query by name (name + kind, no source). The details-branch
                            // fallback (all-symbols when the strict filter is empty) must NOT
                            // leak here — otherwise a path/docstring match would dump every
                            // symbol and defeat the overview's context-efficiency purpose.
                            let query_terms: Vec<String> = query
                                .to_lowercase()
                                .split_whitespace()
                                .map(|s| s.to_string())
                                .collect();
                            // Cap the overview file headers so a broad query (a directory
                            // name, a common token) can't emit ~100 headers and blow the
                            // agent's context budget. Config-driven (default 50).
                            let search_overview_file_limit =
                                crate::config::get().search_overview_file_limit;
                            text.push_str(&format!(
                                "## Codemap overview — {} matches\n",
                                results.len()
                            ));
                            for res in results.iter().take(search_overview_file_limit) {
                                text.push_str(&format!(
                                    "### {} ({} lines)\n",
                                    res.file_path, res.total_lines
                                ));
                                for sym in &res.matched_symbols {
                                    let name_lower = sym.name.to_lowercase();
                                    // Use the SAME criterion as the index symbol filter and
                                    // the detail-view fallback (all query terms, over name ∪
                                    // split-identifier ∪ docstring) so the overview and detail
                                    // branches agree on which symbols count as matched.
                                    let name_matches = query_terms.iter().all(|t| {
                                        name_lower.contains(t.as_str())
                                            || sym.docstring.as_ref().is_some_and(|d| {
                                                d.to_lowercase().contains(t.as_str())
                                            })
                                            || crate::parser::split_identifier(&sym.name)
                                                .iter()
                                                .any(|sub| sub.to_lowercase().contains(t.as_str()))
                                    });
                                    if name_matches {
                                        text.push_str(&format!(
                                            "- {} ({}) [L{}-{}]\n",
                                            sym.name,
                                            sym.kind,
                                            sym.range.start_line,
                                            sym.range.end_line
                                        ));
                                    }
                                }
                            }
                            if results.len() > search_overview_file_limit {
                                text.push_str(&format!(
                                    "\n_… {} more files not shown; refine the query or use overview/find to narrow._\n",
                                    results.len() - search_overview_file_limit
                                ));
                            }
                        } else {
                            // Detail view: enclosing code scopes for the pinpointed files,
                            // bounded by config caps so a few large or fallback-matched files
                            // can't dump the whole tree into the agent's context.
                            let cfg = crate::config::get();
                            let snippet_max_lines = cfg.search_detail_snippet_max_lines;
                            let symbol_limit = cfg.search_detail_symbol_limit;
                            let byte_cap = cfg.search_detail_byte_cap;
                            let literal_max_len = cfg.search_literal_max_len;
                            let literal_limit = cfg.search_literal_limit;

                            let mut budget_hit = false;
                            'files: for res in &results {
                                if text.len() >= byte_cap {
                                    budget_hit = true;
                                    break;
                                }
                                text.push_str(&format!(
                                    "### File: {} ({} lines)\n",
                                    res.file_path, res.total_lines
                                ));

                                if res.symbol_fallback {
                                    // Matched via docstring/path, not a symbol name — render
                                    // symbol names + ranges ONLY (no snippets) so we never
                                    // `cat` the file. Still count-capped.
                                    for sym in res.matched_symbols.iter().take(symbol_limit) {
                                        text.push_str(&format!(
                                            "- Symbol: {} ({}) [L{}-{}]\n",
                                            sym.name,
                                            sym.kind,
                                            sym.range.start_line,
                                            sym.range.end_line
                                        ));
                                    }
                                    if res.matched_symbols.len() > symbol_limit {
                                        text.push_str(&format!(
                                            "- _… {} more symbols not shown; use overview/read to inspect._\n",
                                            res.matched_symbols.len() - symbol_limit
                                        ));
                                    }
                                } else {
                                    // Name-matched file: emit capped snippets, deduping
                                    // nested ranges so a container and its members don't
                                    // double-print the same source lines.
                                    let mut symbols: Vec<&crate::parser::ExtractedSymbol> =
                                        res.matched_symbols.iter().collect();
                                    // Outermost-first (smaller start, then larger end) so an
                                    // enclosing container is emitted before/over its members.
                                    symbols.sort_by(|a, b| {
                                        a.range
                                            .start_line
                                            .cmp(&b.range.start_line)
                                            .then(b.range.end_line.cmp(&a.range.end_line))
                                    });
                                    let mut emitted_ranges: Vec<(usize, usize)> = Vec::new();
                                    let mut shown = 0usize;
                                    let mut skipped_for_cap = 0usize;
                                    for sym in symbols {
                                        let (start, end) =
                                            (sym.range.start_line, sym.range.end_line);
                                        // Skip a symbol fully contained in an already-emitted
                                        // range — its lines were shown inside that snippet.
                                        if emitted_ranges
                                            .iter()
                                            .any(|(es, ee)| *es <= start && end <= *ee)
                                        {
                                            continue;
                                        }
                                        if shown >= symbol_limit {
                                            skipped_for_cap += 1;
                                            continue;
                                        }
                                        if text.len() >= byte_cap {
                                            budget_hit = true;
                                            break 'files;
                                        }
                                        text.push_str(&format!(
                                            "- Symbol: {} ({}) [L{}-{}]\n",
                                            sym.name,
                                            sym.kind,
                                            sym.range.start_line,
                                            sym.range.end_line
                                        ));
                                        let snippet =
                                            get_code_snippet(&res.file_path, &sym.range);
                                        // The range actually shown is bounded by the per-snippet
                                        // line cap. Record the DISPLAYED end (not the full range
                                        // end) so a matched member that falls BELOW a truncated
                                        // container's shown lines is still emitted as its own
                                        // snippet rather than wrongly skipped as "already shown".
                                        let snippet_lines = snippet.lines().count();
                                        let displayed_lines = snippet_lines.min(snippet_max_lines);
                                        let displayed_end =
                                            start + displayed_lines.saturating_sub(1);
                                        if !snippet.is_empty() {
                                            let capped = cap_snippet(
                                                &snippet,
                                                snippet_max_lines,
                                                byte_cap,
                                            );
                                            text.push_str(&format!("```\n{}\n```\n", capped));
                                        }
                                        emitted_ranges.push((start, displayed_end));
                                        shown += 1;
                                    }
                                    if skipped_for_cap > 0 {
                                        text.push_str(&format!(
                                            "- _… {skipped_for_cap} more symbols not shown; use overview/read to inspect._\n"
                                        ));
                                    }
                                }

                                // Literals: length-truncated and count-capped.
                                for lit in res.matched_literals.iter().take(literal_limit) {
                                    text.push_str(&format!(
                                        "- Literal: {:?}\n",
                                        truncate_literal(lit, literal_max_len)
                                    ));
                                }
                                if res.matched_literals.len() > literal_limit {
                                    text.push_str(&format!(
                                        "- _… {} more literals not shown._\n",
                                        res.matched_literals.len() - literal_limit
                                    ));
                                }
                            }
                            if budget_hit {
                                text.push_str(
                                    "\n_Detail view truncated at the output budget; refine the query or use overview/read to inspect specific files._\n",
                                );
                            }
                        }

                        Ok(serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ]
                        }))
                    }
                    "overview" => {
                        // An empty or "." path means the repo root overview, not a folder
                        // named "" — normalize so it renders the root view (Child 03).
                        let path = arguments
                            .get("path")
                            .and_then(|v| v.as_str())
                            .filter(|p| !p.is_empty() && *p != ".");
                        let format = arguments.get("format").and_then(|v| v.as_str());

                        if let Some(p) = path {
                            if !is_safe_path(p) {
                                return Err((-32602, "Path traversal detected".to_string()));
                            }
                        }

                        let cwd = std::env::current_dir()
                            .map_err(|e| (-32603, format!("Error getting current dir: {}", e)))?;

                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced) and read the codemap
                        // snapshot the indexer publishes — no per-call tree walk or parse.
                        // The indexer parses the working tree once for the index and reuses
                        // it here, so the former overview-only walk+parse is gone.
                        self.maybe_restart_indexer();
                        self.maybe_trigger_refresh();
                        let snapshot = self.indexer.codemap_snapshot();
                        let extracted_files: &[crate::parser::ExtractedFile] = &snapshot;

                        // Nothing to show yet because the initial index is still building (or
                        // the indexer thread died before it finished): say so rather than
                        // render an empty codemap.
                        if extracted_files.is_empty()
                            && (self.indexer.is_warming() || self.indexer.is_dead())
                        {
                            let text = if self.indexer.is_dead() {
                                "Background indexer stopped before the codemap was built; restart the server. Use find/grep/read for live results."
                            } else {
                                "Codemap is warming up (initial background indexing in progress). Retry shortly, or use find/grep/read for live results."
                            };
                            return Ok(serde_json::json!({
                                "content": [{ "type": "text", "text": text }]
                            }));
                        }

                        use crate::codemap::CodemapView;
                        let codemap_text = if let Some(p) = path {
                            let target_path = cwd.join(p);
                            let canonical_cwd = cwd.canonicalize().unwrap_or(cwd);
                            let canonical_target = canonicalize_path_lenient(&target_path);
                            if canonical_target.is_file() {
                                if let Ok(rel_path) = canonical_target.strip_prefix(&canonical_cwd)
                                {
                                    let rel_path_str = rel_path.to_string_lossy().to_string();
                                    if let Some(file) =
                                        extracted_files.iter().find(|f| f.file_path == rel_path_str)
                                    {
                                        crate::codemap::CodemapGenerator::generate_detail_view(file)
                                            .to_markdown()
                                    } else {
                                        // On disk but absent from the codemap: skipped, not
                                        // broken — non-source extension, over the size cap, or
                                        // unparseable. Say so rather than imply a failure.
                                        return Err((-32602, format!(
                                            "File '{}' is not in the codemap (not a supported source file, exceeds the size cap, or could not be parsed)",
                                            p
                                        )));
                                    }
                                } else {
                                    return Err((
                                        -32603,
                                        format!("Failed to process file '{}'", p),
                                    ));
                                }
                            } else {
                                crate::codemap::CodemapGenerator::generate_folder_view(
                                    extracted_files,
                                    p,
                                )
                                .to_markdown()
                            }
                        } else {
                            if format == Some("llms-txt") {
                                crate::codemap::CodemapGenerator::generate_llms_txt_view(
                                    extracted_files,
                                )
                            } else {
                                crate::codemap::CodemapGenerator::generate_root_view(
                                    extracted_files,
                                )
                                .to_markdown()
                            }
                        };

                        Ok(serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": codemap_text
                                }
                            ]
                        }))
                    }
                    "read" => {
                        let text = crate::tools::read::read_file(arguments)?;
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        }))
                    }
                    "find" => {
                        let text = crate::tools::find::find_files(arguments)?;
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        }))
                    }
                    "grep" => {
                        let text = crate::tools::grep::grep(arguments)?;
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        }))
                    }
                    _ => Err((-32601, "Tool not found".to_string())),
                }
            }
            _ => Err((-32601, "Method not found".to_string())),
        }
    }
}
