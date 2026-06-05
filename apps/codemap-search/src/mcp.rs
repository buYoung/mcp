use crate::index::SearchEngine;
use crate::parser::CodeExtractor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

pub struct McpServer<S: SearchEngine, E: CodeExtractor> {
    pub search_engine: S,
    pub extractor: E,
}

fn canonicalize_path_lenient(path: &std::path::Path) -> PathBuf {
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
        canonical.join(suffix)
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

impl<S: SearchEngine, E: CodeExtractor> McpServer<S, E> {
    pub fn new(search_engine: S, extractor: E) -> Self {
        Self {
            search_engine,
            extractor,
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
            let n = self.reader.read(&mut byte_buf).await
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

impl<S: SearchEngine, E: CodeExtractor> McpServer<S, E> {
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
                                let _ = stdout.write_all(format!("{}\n", resp_str).as_bytes()).await;
                                let _ = stdout.flush().await;
                            }
                            continue;
                        }
                    };

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

    fn handle_request(&mut self, method: &str, params: Option<&Value>) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                Ok(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "serverInfo": {
                        "name": "codemap-search-server",
                        "version": "0.1.0"
                    }
                }))
            }
            "tools/list" => {
                Ok(serde_json::json!({
                    "tools": [
                        {
                            "name": "search",
                            "description": "Search code using BM25 query",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string" }
                                },
                                "required": ["query"]
                            }
                        },
                        {
                            "name": "get_codemap",
                            "description": "Get codemap view of directory or file",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "path": { "type": "string" },
                                    "format": { "type": "string" }
                                }
                            }
                        }
                    ]
                }))
            }
            "tools/call" => {
                let params = params.ok_or_else(|| (-32602, "Missing params".to_string()))?;
                let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                    (-32602, "Missing tool name".to_string())
                })?;
                let default_args = serde_json::Value::Object(serde_json::Map::new());
                let arguments = params.get("arguments").unwrap_or(&default_args);

                match name {
                    "search" => {
                        let query = arguments.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
                            (-32602, "Missing query parameter".to_string())
                        })?;

                        // Run indexing dynamically prior to searching to ensure real-time updates
                        if let Err(e) = self.search_engine.index_files(&["."]) {
                            return Err((-32603, format!("Indexing error: {}", e)));
                        }

                        let results = self.search_engine.search(query, 100).map_err(|e| {
                            (-32603, format!("Search error: {}", e))
                        })?;

                        let mut text = String::new();
                        if results.len() >= 5 {
                            // List view: file path list
                            for res in &results {
                                text.push_str(&format!("- {}\n", res.file_path));
                            }
                        } else {
                            // Scope/Spans view: detailed code scopes
                            for res in &results {
                                text.push_str(&format!("### File: {}\n", res.file_path));
                                for sym in &res.matched_symbols {
                                    text.push_str(&format!("- Symbol: {} ({})\n", sym.name, sym.kind));
                                    let snippet = get_code_snippet(&res.file_path, &sym.range);
                                    if !snippet.is_empty() {
                                        text.push_str(&format!("```\n{}\n```\n", snippet));
                                    }
                                }
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
                    "get_codemap" => {
                        let path = arguments.get("path").and_then(|v| v.as_str());
                        let format = arguments.get("format").and_then(|v| v.as_str());

                        if let Some(p) = path {
                            if !is_safe_path(p) {
                                return Err((-32602, "Path traversal detected".to_string()));
                            }
                        }

                        let cwd = std::env::current_dir().map_err(|e| {
                            (-32603, format!("Error getting current dir: {}", e))
                        })?;

                        let mut extracted_files = Vec::new();
                        for entry in walkdir::WalkDir::new(&cwd)
                            .into_iter()
                            .filter_entry(|e| {
                                if e.depth() == 0 {
                                    true
                                } else {
                                    let name = e.file_name().to_string_lossy();
                                    if e.file_type().is_dir() {
                                        !name.starts_with('.')
                                    } else {
                                        true
                                    }
                                }
                            })
                            .filter_map(|e| e.ok())
                        {
                            let file_path = entry.path();
                            if file_path.is_file() {
                                if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                                    if matches!(ext, "rs" | "py" | "ts" | "tsx" | "js" | "jsx") {
                                        if let Ok(rel_path) = file_path.strip_prefix(&cwd) {
                                            let rel_path_str = rel_path.to_string_lossy().to_string();
                                            if let Ok(content) = std::fs::read_to_string(file_path) {
                                                if let Ok(extracted) = self.extractor.extract(&content, &rel_path_str) {
                                                    extracted_files.push(extracted);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        use crate::codemap::CodemapView;
                        let codemap_text = if let Some(p) = path {
                            let target_path = cwd.join(p);
                            let canonical_cwd = cwd.canonicalize().unwrap_or(cwd);
                            let canonical_target = canonicalize_path_lenient(&target_path);
                            if canonical_target.is_file() {
                                if let Ok(rel_path) = canonical_target.strip_prefix(&canonical_cwd) {
                                    let rel_path_str = rel_path.to_string_lossy().to_string();
                                    if let Some(file) = extracted_files.iter().find(|f| f.file_path == rel_path_str) {
                                        crate::codemap::CodemapGenerator::generate_detail_view(file).to_markdown()
                                    } else {
                                        return Err((-32603, format!("Failed to process file '{}'", p)));
                                    }
                                } else {
                                    return Err((-32603, format!("Failed to process file '{}'", p)));
                                }
                            } else {
                                crate::codemap::CodemapGenerator::generate_folder_view(&extracted_files, p).to_markdown()
                            }
                        } else {
                            if format == Some("llms-txt") {
                                crate::codemap::CodemapGenerator::generate_llms_txt_view(&extracted_files)
                            } else {
                                crate::codemap::CodemapGenerator::generate_root_view(&extracted_files).to_markdown()
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
                    _ => Err((-32601, "Tool not found".to_string())),
                }
            }
            _ => Err((-32601, "Method not found".to_string())),
        }
    }
}
