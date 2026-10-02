//! The MCP JSON-RPC contract: the stdio run loop, request dispatch, and `ToolContext`
//! construction. Tool business logic lives under [`crate::tools`]; this module only speaks
//! protocol — it parses frames, routes `tools/call` to the right tool, runs the engine
//! lifecycle (`ensure_alive`/`trigger_refresh`) on the snapshot-backed tools, and wraps tool
//! output and tool failures in the JSON-RPC `result` envelope; protocol failures use `error`.

mod jev;
pub mod protocol;
mod source_history;

use crate::index::EngineSupervisor;
use crate::tools::ToolContext;
use protocol::{JsonRpcRequest, JsonRpcResponse, LimitedLineReader};
use serde_json::Value;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// JSON-RPC code for an unknown tool name, the only protocol error raised after tool dispatch.
const TOOL_NOT_FOUND_CODE: i64 = -32601;

/// A failed tool call as a result the model can read and correct. Clients such as Codex do
/// not forward `isError`, so the text itself states the failure.
fn tool_error_result(message: &str) -> Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": format!("Error: {message}") }],
        "isError": true
    })
}

fn response_texts(response: &Value) -> impl Iterator<Item = &str> {
    response
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
}

fn response_text_bytes(response: &Value) -> usize {
    response_texts(response).fold(0usize, |total, text| total.saturating_add(text.len()))
}

/// Search/read already construct bounded output. Other tools reject an oversized
/// response only when a new common or per-tool response budget was explicitly set.
fn enforce_response_cap(name: &str, response: &Value) -> Result<(), (i64, String)> {
    let config = crate::config::get();
    let cap = match name {
        "overview" => config.overview_output_byte_cap,
        "grep" => config.grep_response_byte_cap,
        "find" | "initial_instructions" | "register_task" | "analyze" => config.output_byte_cap,
        _ => None,
    };
    let Some(cap) = cap else { return Ok(()) };
    let bytes = response_text_bytes(response);
    if bytes > cap {
        return Err((-32602, format!("{name} output ({bytes} bytes) exceeds the configured maximum of {cap} bytes. Narrow the path/query or adjust output.max_bytes or the tool's output section.")));
    }
    Ok(())
}

/// `output.client` bounds the final text after Jev, masking and duplicate folding, however
/// large the earlier `max_bytes` budgets are. Search defers bodies to fit before this check;
/// every other tool returns a narrowing error rather than a result the client would clip.
fn enforce_client_delivery_cap(
    name: &str,
    arguments: Option<&Value>,
    response: &Value,
) -> Result<(), (i64, String)> {
    let Some(cap) = crate::config::get().client_output.delivery_byte_cap() else {
        return Ok(());
    };
    let bytes = response_text_bytes(response);
    if bytes <= cap {
        return Ok(());
    }
    let narrowing = match name {
        "read" => read_narrowing(arguments, response, bytes, cap),
        "grep" => "Retry with a smaller head_limit and continue later pages with offset, or narrow the path/glob/pattern.".into(),
        "search" => "Narrow the query.".into(),
        _ => "Narrow the path/query.".into(),
    };
    Err((-32602, format!("{name} output ({bytes} bytes) exceeds the output.client delivery limit of {cap} bytes. {narrowing}")))
}

/// Scale the returned numbered source rows to a window that fits the delivery limit.
fn read_narrowing(arguments: Option<&Value>, response: &Value, bytes: usize, cap: usize) -> String {
    let expansion_hint = if arguments
        .and_then(|arguments| arguments.get("expand"))
        .and_then(Value::as_str)
        == Some("callable")
    {
        "expand=none and "
    } else {
        ""
    };
    let mut line_numbers = response_texts(response)
        .flat_map(str::lines)
        .filter_map(|line| line.trim_start().split_once('→')?.0.parse::<usize>().ok());
    let Some(first_line) = line_numbers.next() else {
        return format!("Continue with {expansion_hint}a narrower window.");
    };
    let last_line = line_numbers.last().unwrap_or(first_line).max(first_line);
    let suggested_limit = (last_line - first_line + 1)
        .saturating_mul(cap)
        .checked_div(bytes)
        .unwrap_or(1)
        .saturating_sub(1)
        .max(1);
    format!("Continue with {expansion_hint}a narrower window such as offset={first_line}, limit={suggested_limit}.")
}
pub struct McpServer {
    // The live index subsystem: read-only searcher handle, background indexer, optional
    // filesystem watcher, and the supervision state (auto-restart + refresh fallback). The
    // server calls `ensure_alive()`/`trigger_refresh()` on it at the search/overview
    // dispatch sites and reads the committed snapshot through its accessors.
    engine: EngineSupervisor,
    active_workspace_scope: Option<String>,
    call_recorder: crate::analyze::CallRecorder,
    pending_source_files: Vec<crate::analyze::FileObservation>,
    source_history: source_history::SourceHistory,
    source_history_config: Option<std::sync::Arc<crate::config::ResolvedConfig>>,
    // Task context is connection-local, registered separately through register_task.
    // Credentials/evaluator remain separate and are resolved only for enabled stages.
    registered_task: Option<crate::tools::task::RegisteredTask>,
    jev: jev::JevHost,
}

impl McpServer {
    pub fn new(engine: EngineSupervisor) -> Self {
        Self {
            engine,
            active_workspace_scope: None,
            call_recorder: crate::analyze::CallRecorder::new(),
            pending_source_files: Vec::new(),
            source_history: source_history::SourceHistory::default(),
            source_history_config: None,
            registered_task: None,
            jev: jev::JevHost::default(),
        }
    }

    /// Build the index subsystem for `cwd` (searcher, background indexer, config and
    /// filesystem watchers) from the current in-memory config and wrap it in a server. The
    /// `mcp` command and in-process tests share this; call `config::reload(cwd)` first.
    pub fn bootstrap(cwd: &Path) -> Result<Self, String> {
        use crate::index;
        let engine = index::TantivySearchEngine::new(&crate::config::get().index_path)?;
        // Read-only search handle for the request loop; the engine (the single tantivy
        // writer) moves into the background indexer, which starts the initial index pass
        // immediately so the first request need not block on it.
        let searcher = engine.searcher_handle();
        let indexer = index::spawn_indexer(engine);
        let config_watcher = crate::config::spawn_config_watcher(cwd, indexer.command_sender());
        // Health gate shared with the server: stays unhealthy when `watch = false` or the
        // watch fails to start, which keeps the request-triggered fallback active.
        let watcher_status = std::sync::Arc::new(index::WatcherStatus::default());
        let watcher = crate::config::get().watch.then(|| {
            index::spawn_watcher(
                cwd,
                indexer.command_sender(),
                std::sync::Arc::clone(&watcher_status),
            )
        });
        // The supervisor owns all the handles so it can rebuild them when the indexer dies
        // (`indexer_auto_restart`); its field order guarantees the shutdown sequence —
        // config/filesystem watchers drop first (threads joined, their command-sender clones
        // released) before IndexerHandle::drop closes the channel and joins the indexer,
        // whose recv loop ends only when ALL senders are gone.
        let supervisor = index::EngineSupervisor::new(
            searcher,
            config_watcher,
            watcher.flatten(),
            indexer,
            watcher_status,
        );
        Ok(Self::new(supervisor))
    }

    /// Use `evaluator` for every Jev stage instead of building the HTTPS evaluator from the
    /// configured credentials. Regression suites inject `jev::mock::MockEvaluator` here so
    /// the whole MCP path runs offline; production never calls this.
    pub fn with_evaluator(mut self, evaluator: std::sync::Arc<dyn crate::jev::Evaluator>) -> Self {
        self.jev = jev::JevHost::injected(evaluator);
        self
    }

    pub fn set_call_logging_enabled(&mut self, is_enabled: bool) {
        self.call_recorder.set_enabled(is_enabled);
    }

    fn overview_path_argument(arguments: &Value) -> Option<&str> {
        ["path", "file_path", "file", "query"]
            .iter()
            .find_map(|key| crate::tools::get_arg(arguments, key).and_then(|value| value.as_str()))
    }

    fn update_active_workspace_scope_from_overview(&mut self, arguments: &Value) {
        let Some(path) = Self::overview_path_argument(arguments) else {
            self.active_workspace_scope = None;
            return;
        };
        if crate::codemap::is_all_workspace_scope_input(path) {
            self.active_workspace_scope = None;
            return;
        }
        let snapshot = self.engine.published_snapshot();
        self.active_workspace_scope = snapshot.workspace_catalog().search_scope_for_input(path);
    }

    pub async fn run(&mut self) -> Result<(), String> {
        self.run_with_io(tokio::io::stdin(), tokio::io::stdout())
            .await
    }

    /// The stdio loop over arbitrary line-framed streams (stdin/stdout in production,
    /// in-memory pipes in tests). Requests are handled one at a time; a Jev stage awaits
    /// inside the request, so the loop stays sequential.
    pub async fn run_with_io<R, W>(&mut self, reader: R, mut stdout: W) -> Result<(), String>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut reader = LimitedLineReader::new(reader, 10 * 1024 * 1024 + 100 * 1024);
        self.call_recorder.maintain();
        let period = std::time::Duration::from_secs(60);
        let mut retention = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        retention.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let next = {
                // Preserve a partially read frame while the idle retention timer runs.
                let next_line = reader.next_line();
                tokio::pin!(next_line);
                loop {
                    tokio::select! {
                        result = &mut next_line => break result,
                        _ = retention.tick() => self.call_recorder.maintain(),
                    }
                }
            };
            match next {
                Ok(Some(line)) => {
                    let req: JsonRpcRequest = match serde_json::from_str(&line) {
                        Ok(r) => r,
                        Err(e) => {
                            let _redact_scope = crate::redact::begin_request();
                            let err_resp = JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                result: None,
                                error: Some(serde_json::json!({
                                    "code": -32700,
                                    "message": crate::redact::source(&format!("Parse error: {}", e))
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

                    let response_result =
                        self.handle_request(&req.method, req.params.as_ref()).await;

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
                        if stdout
                            .write_all(format!("{}\n", resp_str).as_bytes())
                            .await
                            .is_ok()
                            && stdout.flush().await.is_ok()
                        {
                            self.source_history.commit_written();
                        } else {
                            self.source_history.discard_pending();
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn handle_request(
        &mut self,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, (i64, String)> {
        // Both scopes are thread-local and stay pinned across the awaits inside one request:
        // the loop is sequential and the runtime is single-threaded, so no other request can
        // observe or replace them while a Jev stage is in flight.
        let _config_scope = crate::config::pin_request();
        let _redact_scope = crate::redact::begin_request();
        let started = std::time::Instant::now();
        self.pending_source_files.clear();
        let config = crate::config::get();
        // A new output/permission/masking configuration starts a fresh delivery
        // history. Compare the pinned snapshot, including across in-flight reloads.
        if self
            .source_history_config
            .as_ref()
            .is_none_or(|previous| !std::sync::Arc::ptr_eq(previous, &config))
        {
            self.source_history.clear();
            self.source_history_config = Some(config);
        }
        self.source_history.begin_request();
        let tool_name = (method == "tools/call")
            .then(|| {
                params
                    .and_then(|params| params.get("name"))
                    .and_then(Value::as_str)
            })
            .flatten();
        let result = match self.handle_request_inner(method, params).await {
            // Negotiation and tool definitions are control metadata. PII rules must
            // not rewrite protocol versions, tool names or schema/enum values.
            Ok(value) if matches!(method, "initialize" | "tools/list") => Ok(value),
            Ok(mut value) => {
                // analyze masks its structured values before serializing compact JSON.
                if tool_name != Some("analyze") {
                    crate::redact::response(&mut value);
                }
                tool_name
                    .map_or(Ok(()), |name| enforce_response_cap(name, &value))
                    .map(|()| {
                        if let Some(name) = tool_name {
                            let empty = serde_json::json!({});
                            let arguments =
                                params.and_then(|p| p.get("arguments")).unwrap_or(&empty);
                            let folded = self.source_history.prepare(name, arguments, &mut value);
                            for file in &mut self.pending_source_files {
                                if let Some(bytes) = folded.removed_source_bytes.get(&file.path) {
                                    file.result_bytes =
                                        file.result_bytes.saturating_sub(*bytes as u64);
                                }
                            }
                            self.pending_source_files
                                .retain(|file| file.result_bytes > 0);
                            if matches!(name, "search" | "read" | "grep") {
                                tracing::info!(
                                    tool = name,
                                    folded_spans = folded.spans,
                                    folded_lines = folded.lines,
                                    saved_bytes = folded.saved_bytes,
                                    "source delivery history"
                                );
                            }
                        }
                        value
                    })
            }
            Err((code, message)) => Err((code, crate::redact::source(&message).into_owned())),
        };
        // Removing a very short repeat may add a slightly longer marker. Check
        // the final response too, and never commit evidence from a rejected one.
        let result = result.and_then(|value| {
            if let Some(name) = tool_name {
                enforce_response_cap(name, &value)?;
                let arguments = params.and_then(|params| params.get("arguments"));
                enforce_client_delivery_cap(name, arguments, &value)?;
            }
            Ok(value)
        });
        if result.is_err() {
            self.source_history.discard_pending();
        }
        if let Some(name) = tool_name {
            let response_bytes = match &result {
                Ok(value) => value
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.get("text").and_then(Value::as_str))
                    .map(str::len)
                    .sum(),
                Err((_, message)) => message.len(),
            };
            tracing::info!(
                tool = name,
                has_meta_call_id = params.and_then(|p| p.pointer("/_meta/callId")).is_some(),
                exec_call_id = protocol::exec_call_id(params),
                response_bytes,
                codex_output_token_limit =
                    crate::config::get().client_output.codex_output_token_limit,
                "MCP client delivery context"
            );
            self.call_recorder.record(
                name,
                &self.pending_source_files,
                result.is_err(),
                response_bytes,
                started.elapsed().as_secs_f64() * 1000.0,
            );
        }
        // Tool execution failures, argument validation included, become tool results after
        // recording. Malformed calls without a tool name and unknown tools stay protocol errors.
        match result {
            Err((code, message)) if tool_name.is_some() && code != TOOL_NOT_FOUND_CODE => {
                Ok(tool_error_result(&message))
            }
            result => result,
        }
    }

    async fn handle_request_inner(
        &mut self,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                self.registered_task = None;
                self.source_history.clear();
                // Echo the client's requested protocolVersion when we support it,
                // otherwise fall back to our newest supported version (MCP negotiation).
                const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
                    &["2025-06-18", "2025-03-26", "2024-11-05"];
                let protocol_version = params
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .filter(|v| SUPPORTED_PROTOCOL_VERSIONS.contains(v))
                    .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
                let mut instructions = crate::tools::server_instructions();
                // Codex sends this clientInfo.name. An output setting or an arbitrary
                // client name containing "codex" does not identify a Codex connection.
                let client_name = params
                    .and_then(|p| p.pointer("/clientInfo/name"))
                    .and_then(Value::as_str);
                if client_name == Some("codex-mcp-client") {
                    instructions.push_str("\n\n");
                    instructions.push_str(&crate::tools::codex_exec_instructions());
                }
                Ok(serde_json::json!({
                    "protocolVersion": protocol_version,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "codemap-search-server",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": instructions
                }))
            }
            "ping" => Ok(serde_json::json!({})),
            "tools/list" => Ok(crate::tools::list_tools()),
            "tools/call" => {
                let params = params.ok_or_else(|| (-32602, "Missing params".to_string()))?;
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| (-32602, "Missing tool name".to_string()))?;
                let default_args = serde_json::Value::Object(serde_json::Map::new());
                let arguments = params.get("arguments").unwrap_or(&default_args);
                source_history::SourceHistory::validate_arguments(name, arguments)?;
                let tool_arguments = source_history::SourceHistory::tool_arguments(name, arguments);
                let arguments = tool_arguments.as_ref();

                match name {
                    "analyze" => {
                        let text = crate::tools::analyze::run(arguments, &self.engine)?;
                        Ok(serde_json::json!({"content": [{"type": "text", "text": text}]}))
                    }
                    "search" => {
                        crate::tools::search::validate_arguments(arguments)?;
                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced by the staleness
                        // window), then search the current committed snapshot immediately
                        // — the response never blocks on indexing. A queued trigger
                        // coalesces bursts; the indexer's mtime diff keeps each pass
                        // incremental. Lifecycle fires only here and on `overview`, never
                        // on the live-filesystem tools (read/find/grep).
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();
                        let config = crate::config::get();
                        let ctx = ToolContext {
                            engine: &self.engine,
                            arguments,
                            active_workspace_scope: self.active_workspace_scope.as_deref(),
                        };
                        let output = if config.jev.is_enabled_for("search") {
                            jev::search(&mut self.jev, &ctx, self.registered_task.as_ref(), &config)
                                .await?
                        } else {
                            crate::tools::search::run_with_metadata(&ctx)?
                        };
                        self.pending_source_files = output.source_files;
                        Ok(serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": output.text
                                }
                            ]
                        }))
                    }
                    "overview" => {
                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced) and read the codemap
                        // snapshot the indexer publishes — no per-call tree walk or parse.
                        // The indexer parses the working tree once for the index and reuses
                        // it here, so the former overview-only walk+parse is gone. Lifecycle
                        // fires only here and on `search`, never on read/find/grep.
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();
                        let ctx = ToolContext {
                            engine: &self.engine,
                            arguments,
                            active_workspace_scope: self.active_workspace_scope.as_deref(),
                        };
                        let text = jev::overview(
                            &mut self.jev,
                            &ctx,
                            self.registered_task.as_ref(),
                            &crate::config::get(),
                        )
                        .await?;
                        self.update_active_workspace_scope_from_overview(arguments);
                        Ok(serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ]
                        }))
                    }
                    "read" | "grep" => {
                        let output = jev::live(
                            &mut self.jev,
                            &self.engine,
                            if name == "read" { "read" } else { "grep" },
                            arguments,
                            self.registered_task.as_ref(),
                            &crate::config::get(),
                        )
                        .await?;
                        self.pending_source_files = output.source_files;
                        Ok(serde_json::json!({"content": [{"type": "text", "text": output.text}]}))
                    }
                    "find" => {
                        let text = crate::tools::find::find_files(arguments)?;
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        }))
                    }
                    "register_task" => {
                        // A new registration replaces the old task; an invalid registration
                        // must not leave a previous task active for subsequent filtering.
                        self.registered_task = None;
                        self.source_history.clear();
                        self.registered_task = Some(crate::tools::task::parse(arguments)?);
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": "Jev task registered. Enabled filters reuse these criteria; register again when the user's task changes." }]
                        }))
                    }
                    "initial_instructions" => {
                        if !arguments.as_object().is_some_and(|args| args.is_empty()) {
                            return Err((-32602, "initial_instructions takes no arguments. Call it with {}. Use register_task for Jev task context.".into()));
                        }
                        let text = if crate::codemap::looks_like_monorepo_workspace() {
                            // Match `overview` lifecycle behavior so the initial response can
                            // include the same root scope selection without a second MCP call.
                            self.engine.ensure_alive();
                            self.engine.trigger_refresh();
                            let ctx = ToolContext {
                                engine: &self.engine,
                                arguments,
                                active_workspace_scope: self.active_workspace_scope.as_deref(),
                            };
                            let root_overview = crate::tools::overview::run(&ctx)?;
                            crate::tools::instructions_with_root_overview(&root_overview)
                        } else {
                            crate::tools::instructions()
                        };
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        }))
                    }
                    _ => Err((TOOL_NOT_FOUND_CODE, "Tool not found".to_string())),
                }
            }
            _ => Err((-32601, "Method not found".to_string())),
        }
    }
}
