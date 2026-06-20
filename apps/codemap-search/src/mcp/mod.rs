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
use tokio::io::AsyncWriteExt;

pub struct McpServer {
    // The live index subsystem: read-only searcher handle, background indexer, optional
    // filesystem watcher, and the supervision state (auto-restart + refresh fallback). The
    // server calls `ensure_alive()`/`trigger_refresh()` on it at the search/overview
    // dispatch sites and reads the committed snapshot through its accessors.
    engine: EngineSupervisor,
}

impl McpServer {
    pub fn new(engine: EngineSupervisor) -> Self {
        Self { engine }
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
                    "instructions": crate::tools::instructions()
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
                    "search" => {
                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced by the staleness
                        // window), then search the current committed snapshot immediately
                        // — the response never blocks on indexing. A queued trigger
                        // coalesces bursts; the indexer's mtime diff keeps each pass
                        // incremental. Lifecycle fires only here and on `overview`, never
                        // on the live-filesystem tools (read/find/grep).
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();
                        let ctx = ToolContext {
                            engine: &self.engine,
                            arguments,
                        };
                        let text = crate::tools::search::run(&ctx)?;
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
                        };
                        let text = crate::tools::overview::run(&ctx)?;
                        Ok(serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
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
                    "initial_instructions" => Ok(serde_json::json!({
                        "content": [{ "type": "text", "text": crate::tools::instructions() }]
                    })),
                    _ => Err((-32601, "Tool not found".to_string())),
                }
            }
            _ => Err((-32601, "Method not found".to_string())),
        }
    }
}
