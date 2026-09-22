//! The MCP JSON-RPC contract: the stdio run loop, request dispatch, and `ToolContext`
//! construction. Tool business logic lives under [`crate::tools`]; this module only speaks
//! protocol — it parses frames, routes `tools/call` to the right tool, runs the engine
//! lifecycle (`ensure_alive`/`trigger_refresh`) on the snapshot-backed tools, and wraps tool
//! output in the JSON-RPC `result`/`error` envelope.

pub mod protocol;

use crate::index::EngineSupervisor;
use crate::tools::ToolContext;
use protocol::{JsonRpcRequest, JsonRpcResponse, LimitedLineReader};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

struct JevClient {
    key_digest: blake3::Hash,
    pool_idle_timeout_ms: u64,
    evaluator: Arc<dyn crate::jev::Evaluator>,
}

fn task_query(arguments: &Value) -> Result<Option<&str>, (i64, String)> {
    match arguments.get("task_query") {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|query| (!query.trim().is_empty()).then_some(query))
            .ok_or_else(|| (-32602, "Invalid 'task_query': expected a string.".into())),
    }
}

fn jev_policy(settings: &crate::config::JevConfig) -> crate::jev::Policy {
    crate::jev::Policy {
        deadline: Duration::from_millis(settings.timeout_ms),
        max_batch_bytes: settings.max_batch_bytes,
        max_in_flight_requests: settings.max_in_flight_requests,
        request_spacing: Duration::from_millis(settings.request_spacing_ms),
    }
}

fn add_jev_status(text: &mut String, cap: Option<usize>, label: &str) {
    let note = format!("\n_Jev {label}._");
    if cap.is_none_or(|cap| text.len().saturating_add(note.len()) <= cap) {
        text.push_str(&note);
    }
}

fn tool_result(text: String, jev: Option<Value>) -> Value {
    let mut result = serde_json::json!({"content":[{"type":"text","text":text}]});
    if let Some(jev) = jev {
        result["_meta"] = serde_json::json!({"jev":jev});
    }
    result
}

fn returned_source_files(
    output: &crate::tools::live_symbols::LiveOutput,
    options: crate::tools::live_options::LiveOptions,
) -> Vec<crate::analyze::FileObservation> {
    use crate::tools::live_options::LiveView;
    if !matches!(options.view, LiveView::Full | LiveView::Source) {
        return Vec::new();
    }
    let mut files = output
        .source_ranges
        .iter()
        .map(|(path, _, _)| (path.as_str(), 0u64))
        .collect::<std::collections::BTreeMap<_, _>>();
    for span in &output.files {
        if let Some(bytes) = files.get_mut(span.file_path.as_str()) {
            let mut result_bytes = span.end_byte.saturating_sub(span.start_byte);
            if options.view == LiveView::Full {
                // Full view removes the producer-written path prefixes under file headings.
                let prefix_bytes = output
                    .path_prefixes
                    .iter()
                    .filter(|prefix| span.start_byte <= prefix.start && prefix.end <= span.end_byte)
                    .map(|prefix| prefix.end - prefix.start)
                    .sum::<usize>();
                result_bytes = result_bytes.saturating_sub(prefix_bytes);
            }
            *bytes = bytes.saturating_add(result_bytes as u64);
        }
    }
    files
        .into_iter()
        .map(|(path, result_bytes)| crate::analyze::FileObservation {
            path: path.into(),
            result_bytes,
        })
        .collect()
}

/// Search/read already construct bounded output. Other tools reject an oversized
/// response only when a new common or per-tool response budget was explicitly set.
fn enforce_response_cap(name: &str, response: &Value) -> Result<(), (i64, String)> {
    let config = crate::config::get();
    let cap = match name {
        "overview" => config.overview_output_byte_cap,
        "grep" => config.grep_response_byte_cap,
        "find" | "initial_instructions" | "analyze" => config.output_byte_cap,
        _ => None,
    };
    let Some(cap) = cap else { return Ok(()) };
    let bytes = response
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .fold(0usize, |total, text| total.saturating_add(text.len()));
    if bytes > cap {
        return Err((-32602, format!("{name} output ({bytes} bytes) exceeds the configured maximum of {cap} bytes. Narrow the path/query or adjust output.max_bytes or the tool's output section.")));
    }
    Ok(())
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
    jev_client: Option<JevClient>,
    injected_jev_evaluator: Option<Arc<dyn crate::jev::Evaluator>>,
}

impl McpServer {
    pub fn new(engine: EngineSupervisor) -> Self {
        Self {
            engine,
            active_workspace_scope: None,
            call_recorder: crate::analyze::CallRecorder::new(),
            pending_source_files: Vec::new(),
            jev_client: None,
            injected_jev_evaluator: None,
        }
    }

    /// Injection boundary for offline integration tests and library embedding.
    /// Normal MCP construction uses the pinned HTTPS transport.
    pub fn with_jev_evaluator(
        engine: EngineSupervisor,
        evaluator: Arc<dyn crate::jev::Evaluator>,
    ) -> Self {
        let mut server = Self::new(engine);
        server.injected_jev_evaluator = Some(evaluator);
        server
    }

    fn jev_evaluator(
        &mut self,
        settings: &crate::config::JevConfig,
    ) -> Result<Arc<dyn crate::jev::Evaluator>, &'static str> {
        if let Some(evaluator) = &self.injected_jev_evaluator {
            return Ok(Arc::clone(evaluator));
        }
        let api_key = std::env::var(&settings.api_key_env)
            .ok()
            .filter(|key| !key.is_empty())
            .ok_or("missing_api_key")?;
        let key_digest = blake3::hash(api_key.as_bytes());
        if let Some(client) = &self.jev_client {
            if client.key_digest == key_digest
                && client.pool_idle_timeout_ms == settings.pool_idle_timeout_ms
            {
                return Ok(Arc::clone(&client.evaluator));
            }
        }
        let transport = crate::jev::HttpTransport::new(
            api_key,
            Duration::from_millis(settings.pool_idle_timeout_ms),
        )
        .map_err(|_| "transport_initialization")?;
        let evaluator: Arc<dyn crate::jev::Evaluator> =
            Arc::new(crate::jev::BatchEvaluator::new(Arc::new(transport)));
        self.jev_client = Some(JevClient {
            key_digest,
            pool_idle_timeout_ms: settings.pool_idle_timeout_ms,
            evaluator: Arc::clone(&evaluator),
        });
        Ok(evaluator)
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
        let stdin = tokio::io::stdin();
        let mut reader = LimitedLineReader::new(stdin, 10 * 1024 * 1024 + 100 * 1024);
        let mut stdout = tokio::io::stdout();
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

    /// Handle one request in the same sequential, pinned-config boundary as stdio.
    /// Library callers may inject an evaluator for offline integration.
    pub async fn handle_request(
        &mut self,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, (i64, String)> {
        let _config_scope = crate::config::pin_request();
        let _redact_scope = crate::redact::begin_request();
        let started = std::time::Instant::now();
        self.pending_source_files.clear();
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
                    .map(|()| value)
            }
            Err((code, message)) => Err((code, crate::redact::source(&message).into_owned())),
        };
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
            self.call_recorder.record(
                name,
                &self.pending_source_files,
                result.is_err(),
                response_bytes,
                started.elapsed().as_secs_f64() * 1000.0,
            );
        }
        result
    }

    async fn handle_request_inner(
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
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": crate::tools::server_instructions()
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

                match name {
                    "analyze" => {
                        let text = crate::tools::analyze::run(arguments, &self.engine)?;
                        Ok(serde_json::json!({"content": [{"type": "text", "text": text}]}))
                    }
                    "search" => {
                        crate::tools::search::validate_arguments(arguments)?;
                        let intent = task_query(arguments)?;
                        let settings = crate::config::get().jev.clone();
                        let evaluator = if settings.search_filter_enabled && intent.is_some() {
                            Some(self.jev_evaluator(&settings))
                        } else {
                            None
                        };
                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced by the staleness
                        // window), then search the current committed snapshot immediately
                        // — the response never blocks on indexing. A queued trigger
                        // coalesces bursts; the indexer's mtime diff keeps each pass
                        // incremental. Lifecycle fires only here and on `overview`, never
                        // on the live-filesystem tools (read/find/grep).
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();
                        let output = {
                            let ctx = ToolContext {
                                engine: &self.engine,
                                arguments,
                                active_workspace_scope: self.active_workspace_scope.as_deref(),
                            };
                            if evaluator.as_ref().is_some_and(Result::is_ok) {
                                crate::tools::search::prepare_for_jev(&ctx)?
                            } else {
                                crate::tools::search::run_with_metadata(&ctx)?
                            }
                        };
                        let (mut output, jev_meta) = match evaluator {
                            Some(Ok(evaluator)) => {
                                match crate::tools::search::filter_prepared(
                                    output,
                                    intent.unwrap_or_default(),
                                    arguments,
                                    evaluator.as_ref(),
                                    jev_policy(&settings),
                                    crate::jev::Cancellation::new(),
                                    settings.search_filter_min_unrelated_probability,
                                )
                                .await
                                {
                                    Ok(result) => {
                                        let meta = serde_json::json!({
                                            "mode":"search_filter","outcome":result.outcome,
                                            "evaluated_bodies":result.evaluated_bodies,
                                            "omitted_bodies":result.omitted_bodies,
                                            "threshold":result.effective_threshold,
                                            "policy_version":result.policy_version,
                                            "input_tokens":result.usage.input_tokens,
                                            "output_tokens":result.usage.output_tokens,
                                            "elapsed_ms":result.elapsed.as_millis()
                                        });
                                        (result.output, Some(meta))
                                    }
                                    Err(failure) => {
                                        let meta = serde_json::json!({
                                            "mode":"search_filter","outcome":"fallback",
                                            "reason":failure.reason,
                                            "input_tokens":failure.usage.map(|usage| usage.input_tokens),
                                            "output_tokens":failure.usage.map(|usage| usage.output_tokens),
                                            "elapsed_ms":failure.elapsed.as_millis()
                                        });
                                        (failure.output, Some(meta))
                                    }
                                }
                            }
                            Some(Err(reason)) => (
                                output,
                                Some(serde_json::json!({
                                    "mode":"search_filter","outcome":"bypassed","reason":reason,
                                    "input_tokens":0,"output_tokens":0,"elapsed_ms":0
                                })),
                            ),
                            None if settings.search_filter_enabled => (
                                output,
                                Some(serde_json::json!({
                                    "mode":"search_filter","outcome":"bypassed",
                                    "reason":"missing_task_query",
                                    "input_tokens":0,"output_tokens":0,"elapsed_ms":0
                                })),
                            ),
                            None => (output, None),
                        };
                        if let Some(meta) = &jev_meta {
                            add_jev_status(
                                &mut output.text,
                                Some(crate::config::get().search_detail_byte_cap),
                                &format!(
                                    "search_filter: {}{}",
                                    meta["outcome"].as_str().unwrap_or("bypassed"),
                                    meta["reason"]
                                        .as_str()
                                        .map(|reason| format!(" ({reason})"))
                                        .unwrap_or_default()
                                ),
                            );
                        }
                        self.pending_source_files = output.source_files;
                        Ok(tool_result(output.text, jev_meta))
                    }
                    "overview" => {
                        let intent = task_query(arguments)?;
                        let settings = crate::config::get().jev.clone();
                        let evaluator = if settings.overview_enabled && intent.is_some() {
                            Some(self.jev_evaluator(&settings))
                        } else {
                            None
                        };
                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced) and read the codemap
                        // snapshot the indexer publishes — no per-call tree walk or parse.
                        // The indexer parses the working tree once for the index and reuses
                        // it here, so the former overview-only walk+parse is gone. Lifecycle
                        // fires only here and on `search`, never on read/find/grep.
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();
                        let preparation = {
                            let ctx = ToolContext {
                                engine: &self.engine,
                                arguments,
                                active_workspace_scope: self.active_workspace_scope.as_deref(),
                            };
                            crate::tools::overview::prepare(&ctx)?
                        };
                        self.update_active_workspace_scope_from_overview(arguments);
                        let cap = crate::config::get()
                            .overview_output_byte_cap
                            .or(crate::config::get().output_byte_cap);
                        let (mut text, jev_meta) = match evaluator {
                            Some(Ok(evaluator)) if preparation.snapshot.is_some() => {
                                let base = preparation.text.clone();
                                match crate::tools::overview::recommend(
                                    preparation,
                                    intent.unwrap_or_default(),
                                    evaluator.as_ref(),
                                    jev_policy(&settings),
                                    crate::jev::Cancellation::new(),
                                    cap,
                                )
                                .await
                                {
                                    Ok(result) => (
                                        result.text,
                                        Some(serde_json::json!({
                                            "mode":"overview","outcome":"applied",
                                            "recommendation_status":result.status,
                                            "evaluated_files":result.evaluated_files,
                                            "evaluated_fragments":result.evaluated_fragments,
                                            "policy_version":result.policy_version,
                                            "input_tokens":result.usage.input_tokens,
                                            "output_tokens":result.usage.output_tokens,
                                            "elapsed_ms":result.elapsed.as_millis()
                                        })),
                                    ),
                                    Err(failure) => (
                                        base,
                                        Some(serde_json::json!({
                                            "mode":"overview","outcome":"fallback",
                                            "reason":failure.reason,
                                            "input_tokens":failure.usage.map(|usage| usage.input_tokens),
                                            "output_tokens":failure.usage.map(|usage| usage.output_tokens),
                                            "elapsed_ms":failure.elapsed.as_millis()
                                        })),
                                    ),
                                }
                            }
                            Some(Ok(_)) => (
                                preparation.text,
                                Some(serde_json::json!({
                                    "mode":"overview","outcome":"bypassed",
                                    "reason":"index_not_ready_or_non_root",
                                    "input_tokens":0,"output_tokens":0,"elapsed_ms":0
                                })),
                            ),
                            Some(Err(reason)) => (
                                preparation.text,
                                Some(serde_json::json!({
                                    "mode":"overview","outcome":"bypassed","reason":reason,
                                    "input_tokens":0,"output_tokens":0,"elapsed_ms":0
                                })),
                            ),
                            None if settings.overview_enabled => (
                                preparation.text,
                                Some(serde_json::json!({
                                    "mode":"overview","outcome":"bypassed",
                                    "reason":"missing_task_query",
                                    "input_tokens":0,"output_tokens":0,"elapsed_ms":0
                                })),
                            ),
                            None => (preparation.text, None),
                        };
                        if let Some(meta) = &jev_meta {
                            add_jev_status(
                                &mut text,
                                cap,
                                &format!(
                                    "overview: {}{}",
                                    meta["outcome"].as_str().unwrap_or("bypassed"),
                                    meta["reason"]
                                        .as_str()
                                        .map(|reason| format!(" ({reason})"))
                                        .unwrap_or_default()
                                ),
                            );
                        }
                        Ok(tool_result(text, jev_meta))
                    }
                    "read" => {
                        let options = crate::tools::live_options::LiveOptions::parse(arguments)?;
                        let output = crate::tools::read::read_file_with_metadata(arguments)?;
                        self.pending_source_files = returned_source_files(&output, options);
                        let text = crate::tools::live_symbols::append(
                            &self.engine,
                            output,
                            Some(crate::config::get().read_output_byte_cap),
                            options,
                        )?;
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
                        let options =
                            crate::tools::live_options::LiveOptions::parse_grep(arguments)?;
                        let output = crate::tools::grep::grep_with_metadata(arguments)?;
                        self.pending_source_files = returned_source_files(&output, options);
                        let output_mode = arguments
                            .get("output_mode")
                            .and_then(|value| value.as_str())
                            .unwrap_or("content");
                        let text = if output_mode == "content" {
                            crate::tools::live_symbols::append(
                                &self.engine,
                                output,
                                options
                                    .should_expand_callable
                                    .then(|| crate::config::get().grep_output_byte_cap),
                                options,
                            )?
                        } else {
                            output.text
                        };
                        Ok(serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        }))
                    }
                    "initial_instructions" => {
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
                    _ => Err((-32601, "Tool not found".to_string())),
                }
            }
            _ => Err((-32601, "Method not found".to_string())),
        }
    }
}
