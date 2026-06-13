use crate::index::EngineSupervisor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    // The live index subsystem: read-only searcher handle, background indexer, optional
    // filesystem watcher, and the supervision state (auto-restart + refresh fallback). The
    // server calls `ensure_alive()`/`trigger_refresh()` on it at the search/overview
    // dispatch sites and reads the committed snapshot through its accessors.
    engine: EngineSupervisor,
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

/// Extract a symbol's source range with `read`-style line numbers (`␠␠␠␠␠1→content`).
/// Numbered so the agent can cite exact lines straight from the detail view instead of
/// re-reading the file to confirm them (the dominant post-discovery turn cost observed).
fn get_code_snippet(file_path: &str, range: &crate::parser::CodeRange) -> String {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(range.end_line, lines.len());
            if start < end {
                return lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\u{2192}{}", start + 1 + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
    String::new()
}

/// Tokenize a query string into the SAME sub-token set the index builds, so "query-matching
/// symbol" anchoring (P1) uses identical splitting to ranking. Each whitespace-separated word
/// is run through [`crate::parser::split_identifier`] (camel/snake/kebab/acronym aware,
/// lowercasing), and the lowercased raw word is added too so a quoted-string / single-token
/// query still matches. Returns a lowercase token set.
fn query_tokens(query: &str) -> std::collections::HashSet<String> {
    let mut tokens = std::collections::HashSet::new();
    for word in query.split_whitespace() {
        for tok in crate::parser::split_identifier(word) {
            tokens.insert(tok);
        }
        let lower = word.to_lowercase();
        if !lower.is_empty() {
            tokens.insert(lower);
        }
    }
    tokens
}

/// Whether a symbol is a "query-matching symbol" (P1, Tier-2): any sub-token of its NAME, or
/// of its OWNER (the enclosing type/impl/class), intersects the query token set. The
/// owner-token path is load-bearing — it keeps a query like "StorageFactory get" anchoring the
/// `get` method whose owner is `StorageFactory`, even though `get` alone is generic.
///
/// Tier-2 is the loose tier: it powers the no-Tier-1 fallback rendering. When a file holds a
/// Tier-1 (exact-name) symbol the renderer prefers Tier-1 and demotes the merely-Tier-2 hits to
/// a one-line list, because token-intersection alone over-matches (e.g. the `select` sub-token
/// of `SELECT` crossing `get_select`, `get_extra_select`, … — the live A/B regression P1 fixed).
fn symbol_matches_query(
    sym: &crate::parser::ExtractedSymbol,
    query_tokens: &std::collections::HashSet<String>,
) -> bool {
    if crate::parser::split_identifier(&sym.name)
        .iter()
        .any(|t| query_tokens.contains(t))
    {
        return true;
    }
    if let Some(owner) = &sym.owner {
        if crate::parser::split_identifier(owner)
            .iter()
            .any(|t| query_tokens.contains(t))
        {
            return true;
        }
    }
    false
}

/// The set of "query words" (P1 Tier-1): the raw query split on whitespace AND punctuation
/// (`.`, `,`, `(`, `)`, quotes, brackets, etc.) into whole-identifier words, each lowercased.
/// Unlike [`query_tokens`], a word is NOT further sub-split — `execute_sql` stays one word — so
/// Tier-1 equality is whole-identifier (`execute_sql == execute_sql`), not sub-token. Examples:
/// "SQLCompiler execute_sql SELECT" → {sqlcompiler, execute_sql, select};
/// "class Q django.db.models" → {class, q, django, db, models}.
fn query_words(query: &str) -> std::collections::HashSet<String> {
    query
        // A query word is a maximal run of identifier characters: ASCII alphanumerics, '_',
        // '-', and any non-ASCII alphanumeric (so unicode identifiers survive). Everything
        // else — whitespace, '.', ',', parens, quotes, brackets — is a separator.
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .filter_map(|word| {
            let lower = word.to_lowercase();
            if lower.is_empty() {
                None
            } else {
                Some(lower)
            }
        })
        .collect()
}

/// Whether a symbol is a Tier-1 (exact-name) match (P1): its NAME, lowercased, equals one of the
/// query words as a whole identifier. This is strict — `get_select` is NOT Tier-1 for query word
/// `select` — so Tier-1 anchors only the symbols the agent actually named (execute_sql,
/// sqlcompiler, q, cached_property), and `StorageFactory get` makes the member `get` Tier-1 by
/// name equality while owner-token matches stay Tier-2.
fn symbol_is_tier1(
    sym: &crate::parser::ExtractedSymbol,
    query_words: &std::collections::HashSet<String>,
) -> bool {
    query_words.contains(&sym.name.to_lowercase())
}

/// A compact 2-line declaration summary for a CONTAINER whose member matched (P1 §4): the
/// container's first source lines, line-numbered like [`get_code_snippet`], so the agent sees
/// the class/impl header (and often its opening docstring) without the whole body being dumped.
/// Bounded to at most 2 physical lines of the symbol's range. Applies to any container enclosing
/// an anchor member — including a Tier-1 container that holds a Tier-1 member, which is demoted
/// to this summary so the matched member's own full snippet is what carries the detail.
fn get_summary_snippet(file_path: &str, range: &crate::parser::CodeRange) -> String {
    const SUMMARY_LINES: usize = 2;
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(
                std::cmp::min(range.end_line, lines.len()),
                start + SUMMARY_LINES,
            );
            if start < end {
                return lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\u{2192}{}", start + 1 + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
    String::new()
}

/// A signature mini-snippet for a DEMOTED symbol (P2-loop-2 C1/C3): the symbol's first
/// `max_lines` source lines, line-numbered like [`get_code_snippet`], plus a `… (N more
/// lines)` marker when its body extends past what is shown. Replaces the prior one-line stub
/// for symbols that aren't the file's full-snippet anchor — a Tier-2 hit demoted by a Tier-1
/// neighbor, an anchor demoted past the per-file full-snippet cap (`max_lines = 3`), or a
/// non-matching symbol in a name-matched file (`max_lines = 1`) — so the agent still sees the
/// declaration signature instead of reconstructing it with a follow-up whole-file read (the
/// measured regression). Returns `(snippet, more_lines)`: an empty snippet (unreadable file /
/// out-of-range) carries `more_lines = 0` so the caller falls back to the bare stub line.
fn get_signature_snippet(
    file_path: &str,
    range: &crate::parser::CodeRange,
    max_lines: usize,
) -> (String, usize) {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let symbol_end = std::cmp::min(range.end_line, lines.len());
            let shown_end = std::cmp::min(symbol_end, start + max_lines);
            if start < shown_end {
                let snippet = lines[start..shown_end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\u{2192}{}", start + 1 + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");
                let more_lines = symbol_end.saturating_sub(shown_end);
                return (snippet, more_lines);
            }
        }
    }
    (String::new(), 0)
}

/// Tunable caps threaded into [`render_anchored_symbols`] — the subset of search-detail
/// config the shared anchoring/render path needs, so both the name-matched branch and the
/// symbol-fallback branch render with one identical rule set (P2-loop-2 C1/C3 unification).
struct AnchoredRenderCaps {
    snippet_max_lines: usize,
    anchor_snippet_limit: usize,
    byte_cap: usize,
}

/// Result of [`render_anchored_symbols`]: whether the byte budget was hit mid-file (so the
/// caller emits the truncation notice and stops), plus the start lines actually emitted as a
/// snippet/summary. The fallback branch uses `emitted_starts` to skip those symbols when it
/// prints the residual name-only list, so a symbol is never both rendered AND re-listed.
struct AnchoredRenderOutcome {
    budget_hit: bool,
    emitted_starts: std::collections::HashSet<usize>,
}

/// The shared P1 2-tier anchoring + P2-loop-2 C1/C3 render path for a name-matched file's
/// symbols. Extracted so the symbol-fallback branch renders matched symbols with EXACTLY the
/// same rules as the primary name-matched branch — full snippets only for promoted anchors
/// (`anchor_snippet_limit`, Tier-1 first), a 2-line declaration summary for any container
/// enclosing an anchor member, a ≤3-line signature for over-cap/Tier-2 demotions, and a
/// 1-line signature for a non-matching symbol — instead of the prior loop-1 "5 full snippets,
/// no caps" rule that flooded a large fallback file (the `Signal`/`dispatcher.py` regression).
///
/// `symbols` is the already-selected render set (caller applies any count cap / fallback
/// selection first). Anchoring is computed over THIS set; promoted-anchor selection walks it
/// in the given order (so callers pass it in rank order before any range sort). Containers
/// enclosing an anchor are excluded from the promoted cap so a summarized container never
/// steals a full-snippet slot from a real member anchor (P2-loop-2 promoted/summary interaction).
fn render_anchored_symbols(
    text: &mut String,
    file_path: &str,
    symbols: Vec<&crate::parser::ExtractedSymbol>,
    query_word_set: &std::collections::HashSet<String>,
    query_token_set: &std::collections::HashSet<String>,
    caps: &AnchoredRenderCaps,
    caller_annotations: Option<&crate::callers::DetailAnnotations>,
) -> AnchoredRenderOutcome {
    let mut emitted_starts: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let AnchoredRenderCaps {
        snippet_max_lines,
        anchor_snippet_limit,
        byte_cap,
    } = *caps;

    // Promoted-anchor selection walks the caller's order (rank order), so the highest-ranked
    // anchors take the full-snippet slots. The render loop then walks outermost-first so a
    // container is emitted before its members.
    let ranked = symbols;
    let mut render_order: Vec<&crate::parser::ExtractedSymbol> = ranked.clone();
    render_order.sort_by(|a, b| {
        a.range
            .start_line
            .cmp(&b.range.start_line)
            .then(b.range.end_line.cmp(&a.range.end_line))
    });

    // P1 2-tier anchoring. Tier-1 = NAME exactly equals a query word (strict). Tier-2 = the
    // looser sub-token/owner match. Anchor on Tier-1 when the file holds any, else Tier-2.
    let tier1_ranges: Vec<(usize, usize)> = render_order
        .iter()
        .filter(|s| symbol_is_tier1(s, query_word_set))
        .map(|s| (s.range.start_line, s.range.end_line))
        .collect();
    let has_tier1 = !tier1_ranges.is_empty();
    let tier2_ranges: Vec<(usize, usize)> = render_order
        .iter()
        .filter(|s| symbol_matches_query(s, query_token_set))
        .map(|s| (s.range.start_line, s.range.end_line))
        .collect();
    let anchor_ranges: &[(usize, usize)] = if has_tier1 {
        &tier1_ranges
    } else {
        &tier2_ranges
    };
    let any_match = !anchor_ranges.is_empty();
    let is_anchor_sym = |sym: &crate::parser::ExtractedSymbol| {
        if has_tier1 {
            symbol_is_tier1(sym, query_word_set)
        } else {
            symbol_matches_query(sym, query_token_set)
        }
    };
    let encloses_anchor_range = |start: usize, end: usize| {
        anchor_ranges
            .iter()
            .any(|(ms, me)| start <= *ms && *me <= end && (start, end) != (*ms, *me))
    };
    // P2-loop-2 C3: per-file full-snippet cap, highest-ranked anchors first. A container
    // enclosing an anchor is summarized (not a full-snippet anchor), so it is excluded here
    // and never consumes a slot a real member anchor should get.
    let promoted_anchor_starts: std::collections::HashSet<usize> = if any_match {
        ranked
            .iter()
            .copied()
            .filter(|s| {
                is_anchor_sym(s)
                    && !encloses_anchor_range(s.range.start_line, s.range.end_line)
            })
            .take(anchor_snippet_limit)
            .map(|s| s.range.start_line)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let mut emitted_ranges: Vec<(usize, usize)> = Vec::new();
    let mut caller_block_dedup = crate::callers::CallerBlockDedup::new();
    for sym in render_order {
        let (start, end) = (sym.range.start_line, sym.range.end_line);
        if emitted_ranges
            .iter()
            .any(|(es, ee)| *es <= start && end <= *ee)
        {
            continue;
        }
        if text.len() >= byte_cap {
            return AnchoredRenderOutcome {
                budget_hit: true,
                emitted_starts,
            };
        }
        text.push_str(&format!(
            "- Symbol: {} ({}) [L{}-{}]\n",
            sym.name, sym.kind, sym.range.start_line, sym.range.end_line
        ));
        emitted_starts.insert(start);
        let is_anchor = is_anchor_sym(sym);
        let encloses_anchor = encloses_anchor_range(start, end);
        let is_summary_container = any_match && encloses_anchor && (has_tier1 || !is_anchor);
        let is_full_anchor = any_match && is_anchor && !is_summary_container;
        let is_overcap_anchor = is_full_anchor && !promoted_anchor_starts.contains(&start);
        let is_match = !any_match || (is_full_anchor && !is_overcap_anchor);
        if !is_match && !is_summary_container {
            // Demoted symbol (C1/C3): over-cap anchor or Tier-2 hit → 3 signature lines;
            // a non-matching symbol → 1 signature line. No caller/callee annotation.
            let is_tier2_hit = symbol_matches_query(sym, query_token_set);
            let sig_lines = if is_overcap_anchor || is_tier2_hit { 3 } else { 1 };
            let (sig, more_lines) = get_signature_snippet(file_path, &sym.range, sig_lines);
            if sig.is_empty() {
                emitted_ranges.push((start, start));
            } else {
                let body = if more_lines > 0 {
                    format!("{sig}\n… ({more_lines} more lines)")
                } else {
                    sig.clone()
                };
                text.push_str(&format!("```\n{body}\n```\n"));
                let shown = sig.lines().count();
                let displayed_end = start + shown.saturating_sub(1);
                emitted_ranges.push((start, displayed_end));
            }
            continue;
        }
        let snippet = if is_summary_container {
            get_summary_snippet(file_path, &sym.range)
        } else {
            get_code_snippet(file_path, &sym.range)
        };
        let snippet_lines = snippet.lines().count();
        let displayed_lines = snippet_lines.min(snippet_max_lines);
        let displayed_end = start + displayed_lines.saturating_sub(1);
        if !snippet.is_empty() {
            let capped = cap_snippet(&snippet, snippet_max_lines, byte_cap);
            text.push_str(&format!("```\n{}\n```\n", capped));
        }
        if !is_summary_container {
            if let Some(annotations) = caller_annotations {
                if let Some(prepared) = annotations.render(file_path, start, &caller_block_dedup) {
                    if text.len() + prepared.text().len() <= byte_cap {
                        text.push_str(prepared.text());
                        prepared.commit(&mut caller_block_dedup);
                    } else if text.len() + crate::callers::ANNOTATION_OMITTED_MARKER.len() <= byte_cap
                    {
                        text.push_str(crate::callers::ANNOTATION_OMITTED_MARKER);
                    }
                }
            }
        }
        emitted_ranges.push((start, displayed_end));
    }
    AnchoredRenderOutcome {
        budget_hit: false,
        emitted_starts,
    }
}

impl McpServer {
    pub fn new(engine: EngineSupervisor) -> Self {
        Self { engine }
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

impl McpServer {
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
                    "instructions": "Five code-navigation tools; split by purpose, not by order.\n- search: first move for symbols, definitions, concepts, and quoted strings ('where is the auth token refreshed?', an error message, a config default). BM25 over indexed symbols/docstrings/string literals — identifier splitting and ranking find what an exact pattern misses. Top-ranked files render in full detail — line-numbered snippets plus each matched function's depth-1 callers and callees (on by default; caller_context=false to disable) — so one call answers both 'where is it' and 'who calls it'; further matches follow as a ranked one-line list. Snippet line numbers and caller file:line positions are exact — cite them directly instead of re-reading to confirm (e.g. if search already showed `example.py:42→ def compute_total(...)`, cite example.py:42 directly — a follow-up read of the same range adds no accuracy; only caller→definition attribution is name-match approximate).\n- grep: first move for exact-pattern enumeration — every occurrence of a name you already confirmed, regex matches, comments, non-code files, just-edited files (no index lag).\n- overview: orient in unfamiliar code; before reading a large file, overview it to get the exact line range.\n- read / find: file contents / glob lookup.\nTypical flow: search to locate a symbol and see its call context, then read the exact range; grep when you need every literal occurrence or just-edited files."
                }))
            }
            "ping" => Ok(serde_json::json!({})),
            "tools/list" => Ok(serde_json::json!({
                "tools": [
                    {
                        "name": "overview",
                        "description": "Hierarchical codemap. No path: repo-root map with file/symbol counts; folder path: narrows; file path: that file's symbols with line ranges.",
                        // All five tools are read-only over the local workspace. Declaring it
                        // matters: clients gate approval on these hints (Codex auto-cancels
                        // un-annotated tools in non-interactive runs, and prompts per call in
                        // interactive ones).
                        "annotations": { "readOnlyHint": true, "openWorldHint": false },
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Empty/omitted = repo root overview; a folder path narrows; a file path shows that file's symbol details. Aliases 'file_path'/'file'/'query' are also accepted." },
                                "format": { "type": "string", "description": "Optional output format (e.g. 'llms-txt')." }
                            }
                        }
                    },
                    {
                        "name": "search",
                        "description": "Symbol, definition, concept, and quoted-string lookup: BM25 keyword search over indexed symbols, docstrings, and string literals (error messages, config defaults); identifier splitting and ranking recover what exact grep matching misses. Top-ranked files render in detail — symbols with line ranges and line-numbered snippets, each matched function annotated with its depth-1 callers and callees (on by default; positions exact, attribution name-match approximate) — and remaining matches follow as a ranked one-line list. Cite line numbers directly from the response — e.g. if search already showed `example.py:42\u{2192} def compute_total(...)`, cite example.py:42 directly; a follow-up read of the same range adds no accuracy.",
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
                        "description": "Read one file's contents as '   N\u{2192}content' lines; offset/limit pages large files.",
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
                        "description": "Exact literal/regex match over files on disk; parameters mirror Claude Code's Grep. Respects .gitignore/.codemapignore; set include_ignored to bypass.",
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

                        // Caller/callee context (default on). Precedence: the per-call
                        // parameter, when present, always wins (an explicit `false`
                        // overrides the default); the repo-level config key only decides
                        // the default when the parameter is omitted.
                        let caller_context_enabled = arguments
                            .get("caller_context")
                            .and_then(|v| v.as_bool())
                            .unwrap_or_else(|| crate::config::get().caller_context_default);

                        // Recover a dead indexer first (auto-restart, config-gated), then
                        // trigger a background refresh (debounced by the staleness
                        // window), then search the current committed snapshot immediately
                        // — the response never blocks on indexing. A queued trigger
                        // coalesces bursts; the indexer's mtime diff keeps each pass
                        // incremental.
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();

                        let results = self
                            .engine
                            .search(query, 100)
                            .map_err(|e| (-32603, format!("Search error: {}", e)))?;

                        // Result-branch threshold: at or below it, return file details;
                        // above it, return a codemap overview. Config-driven (Child 05),
                        // default 5.
                        let result_branch_threshold = crate::config::get().result_threshold;

                        let mut text = String::new();
                        // While the initial background index builds, results can be empty or
                        // partial — say so, and point at the always-live tools meanwhile.
                        if self.engine.is_dead() {
                            text.push_str(
                                "_Background indexer stopped — search results are frozen at the last index and may be stale; restart the server to recover. read/find/grep stay live._\n\n",
                            );
                        } else if self.engine.is_warming() {
                            text.push_str(
                                "_Index is warming up (initial background indexing) — results may be empty or partial; retry shortly, or use grep/find for live results._\n\n",
                            );
                        } else if let Some(err) = self.engine.last_error() {
                            text.push_str(&format!(
                                "_Last background index refresh failed: {err} — results may be stale._\n\n"
                            ));
                        }
                        // Hybrid rendering: the top-ranked files (BM25 order) always get the
                        // full detail view — snippets plus call context — and every match
                        // beyond the threshold is appended as a compact, ranked one-line
                        // list. A broad multi-word query thus still answers with usable
                        // detail for its strongest hits instead of a wall of file headers
                        // that forces a follow-up grep (the dominant agent-observed failure
                        // mode of the old overview-only branch).
                        let detail_results =
                            &results[..results.len().min(result_branch_threshold)];
                        let remaining_results = &results[detail_results.len()..];
                        {
                            // Detail view: enclosing code scopes for the pinpointed files,
                            // bounded by config caps so a few large or fallback-matched files
                            // can't dump the whole tree into the agent's context.
                            let cfg = crate::config::get();
                            let snippet_max_lines = cfg.search_detail_snippet_max_lines;
                            let symbol_limit = cfg.search_detail_symbol_limit;
                            let byte_cap = cfg.search_detail_byte_cap;
                            let literal_max_len = cfg.search_literal_max_len;
                            let literal_limit = cfg.search_literal_limit;

                            // Caller/callee context (default on): one workspace scan across
                            // every matched `fn` in the detail view, attributed off the codemap
                            // snapshot + the Phase-A owner field. Any failure → `None` → the
                            // detail view renders exactly as today (failure isolation). The
                            // annotation byte budget is what is still free under `byte_cap`
                            // at this point (snippets keep priority, two-counter inside).
                            let caller_annotations = if caller_context_enabled {
                                let snapshot = self.engine.codemap_snapshot();
                                let requests: Vec<crate::callers::AnnotationRequest<'_>> =
                                    detail_results
                                        .iter()
                                        .map(|res| crate::callers::AnnotationRequest {
                                            file_path: &res.file_path,
                                            symbols: &res.matched_symbols,
                                            is_fallback: res.symbol_fallback,
                                        })
                                        .collect();
                                let caller_cfg = crate::callers::CallerConfig {
                                    scan_cap: cfg.scan_cap,
                                    caller_list_cap: cfg.caller_list_cap,
                                    callee_list_cap: cfg.callee_list_cap,
                                    annotation_sub_budget: cfg.annotation_sub_budget,
                                    common_name_threshold: cfg.common_name_threshold,
                                    caller_omit_def_threshold: cfg.caller_omit_def_threshold,
                                    max_file_size: cfg.max_file_size,
                                };
                                let available = byte_cap.saturating_sub(text.len());
                                let root = std::env::current_dir()
                                    .unwrap_or_else(|_| std::path::PathBuf::from("."));
                                crate::callers::annotate_results(
                                    &requests,
                                    &snapshot,
                                    &caller_cfg,
                                    available,
                                    &root,
                                )
                            } else {
                                None
                            };

                            // Two query sets (P1, 2-tier anchoring). Tier-1: whole-identifier
                            // query words for exact NAME equality (strict — the primary anchor).
                            // Tier-2: sub-token set from the SAME splitter the index uses (loose
                            // — owner/sub-token match, used only when a file has no Tier-1 hit).
                            let query_word_set = query_words(query);
                            let query_token_set = query_tokens(query);

                            let mut budget_hit = false;
                            'files: for res in detail_results {
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
                                    // symbol names + ranges ONLY (no snippets) for the bulk so
                                    // we never `cat` the file. Still count-capped.
                                    //
                                    // P1 §6 exception: even in this list-only fallback, if some
                                    // of the file's symbols DO match the query, render the
                                    // matching ones in detail first — the agent ranked this file
                                    // in and a matching symbol here is the likely target — then
                                    // the count-capped name list for the rest. P1 §4 priority:
                                    // Tier-1 (exact-name) symbols are taken FIRST, then Tier-2
                                    // (sub-token/owner) symbols fill the remaining slots, so an
                                    // exactly-named symbol is never crowded out of the cap by a
                                    // loose token match — Tier-1 → Tier-2 only, never a
                                    // non-matching filler.
                                    //
                                    // P2-loop-2 C1/C3 unification: the selected matching symbols
                                    // go through the SAME shared anchoring/render path as the
                                    // name-matched branch — full snippets only for promoted
                                    // anchors (`search_anchor_snippet_limit`, Tier-1 first), a
                                    // 2-line summary for a container enclosing an anchor member
                                    // (e.g. `Signal` around its `send`), and a ≤3-line signature
                                    // for over-cap / Tier-2 demotions — instead of the prior
                                    // loop-1 "5 full snippets, no caps" rule that flooded a large
                                    // fallback file. With zero matching symbols the render set is
                                    // empty and the behavior is unchanged (P1 §5): pure
                                    // path/docstring/literal hits do not regress.
                                    const FALLBACK_SNIPPET_CAP: usize = 5;
                                    let mut matched_in_fallback: Vec<&crate::parser::ExtractedSymbol> =
                                        res.matched_symbols
                                            .iter()
                                            .filter(|s| {
                                                symbol_is_tier1(s, &query_word_set)
                                            })
                                            .collect();
                                    if matched_in_fallback.len() < FALLBACK_SNIPPET_CAP {
                                        for sym in res.matched_symbols.iter().filter(|s| {
                                            !symbol_is_tier1(s, &query_word_set)
                                                && symbol_matches_query(s, &query_token_set)
                                        }) {
                                            if matched_in_fallback.len() >= FALLBACK_SNIPPET_CAP {
                                                break;
                                            }
                                            matched_in_fallback.push(sym);
                                        }
                                    }
                                    matched_in_fallback.truncate(FALLBACK_SNIPPET_CAP);
                                    // Every symbol handed to the render path is "shown in detail"
                                    // and must be excluded from the residual name list — even one
                                    // the path deduped (a member fully inside an already-shown
                                    // range) or dropped at the byte cap — so a matching symbol is
                                    // never both rendered AND re-listed below.
                                    let mut snippet_starts: std::collections::HashSet<usize> =
                                        matched_in_fallback
                                            .iter()
                                            .map(|s| s.range.start_line)
                                            .collect();
                                    let render_caps = AnchoredRenderCaps {
                                        snippet_max_lines,
                                        anchor_snippet_limit: cfg.search_anchor_snippet_limit,
                                        byte_cap,
                                    };
                                    let outcome = render_anchored_symbols(
                                        &mut text,
                                        &res.file_path,
                                        matched_in_fallback,
                                        &query_word_set,
                                        &query_token_set,
                                        &render_caps,
                                        caller_annotations.as_ref(),
                                    );
                                    snippet_starts.extend(outcome.emitted_starts);
                                    if outcome.budget_hit {
                                        budget_hit = true;
                                        break 'files;
                                    }
                                    // Name list for the remaining symbols (those not already
                                    // shown in detail), count-capped as before.
                                    let mut listed = 0usize;
                                    for sym in res.matched_symbols.iter() {
                                        if snippet_starts.contains(&sym.range.start_line) {
                                            continue;
                                        }
                                        if listed >= symbol_limit {
                                            break;
                                        }
                                        text.push_str(&format!(
                                            "- Symbol: {} ({}) [L{}-{}]\n",
                                            sym.name,
                                            sym.kind,
                                            sym.range.start_line,
                                            sym.range.end_line
                                        ));
                                        listed += 1;
                                    }
                                    let remaining = res
                                        .matched_symbols
                                        .len()
                                        .saturating_sub(snippet_starts.len() + listed);
                                    if remaining > 0 {
                                        text.push_str(&format!(
                                            "- _… {remaining} more symbols not shown; use overview/read to inspect._\n"
                                        ));
                                    }
                                } else {
                                    // Name-matched file: emit capped snippets via the shared
                                    // anchoring/render path (P1 2-tier + P2-loop-2 C1/C3). The
                                    // symbol cap is applied on the SELECTION order (strongest
                                    // matches first — exact-name hits lead) BEFORE the function's
                                    // internal range sort, so a promoted symbol deep in a large
                                    // file is never cut in favor of earlier-but-weaker matches,
                                    // and the function's promoted-anchor pick stays rank-ordered.
                                    let skipped_for_cap = res
                                        .matched_symbols
                                        .len()
                                        .saturating_sub(symbol_limit);
                                    let symbols: Vec<&crate::parser::ExtractedSymbol> =
                                        res.matched_symbols.iter().take(symbol_limit).collect();
                                    let render_caps = AnchoredRenderCaps {
                                        snippet_max_lines,
                                        anchor_snippet_limit: cfg.search_anchor_snippet_limit,
                                        byte_cap,
                                    };
                                    let outcome = render_anchored_symbols(
                                        &mut text,
                                        &res.file_path,
                                        symbols,
                                        &query_word_set,
                                        &query_token_set,
                                        &render_caps,
                                        caller_annotations.as_ref(),
                                    );
                                    if outcome.budget_hit {
                                        budget_hit = true;
                                        break 'files;
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
                                        "- Literal: {:?} [L{}]\n",
                                        truncate_literal(&lit.text, literal_max_len),
                                        lit.line
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

                        // Ranked tail: one line per remaining match, strongest first, with
                        // up to 3 matched symbols inline. The index's matched-symbol
                        // selection (exact-name/partial promotions included, strongest
                        // first) is reused as-is; a fallback entry (path/docstring-only
                        // match) shows a bare header instead of leaking unrelated symbols.
                        // Count-capped by `search_overview_file_limit` and byte-capped like
                        // the detail view.
                        if !remaining_results.is_empty() {
                            let tail_cfg = crate::config::get();
                            let tail_file_limit = tail_cfg.search_overview_file_limit;
                            let tail_byte_cap = tail_cfg.search_detail_byte_cap;
                            text.push_str(&format!(
                                "\n## Other matches — {} more files, ranked by relevance\n",
                                remaining_results.len()
                            ));
                            let mut shown_tail = 0usize;
                            for res in remaining_results.iter().take(tail_file_limit) {
                                if text.len() >= tail_byte_cap {
                                    break;
                                }
                                let symbol_notes: Vec<String> = if res.symbol_fallback {
                                    Vec::new()
                                } else {
                                    res.matched_symbols
                                        .iter()
                                        .take(3)
                                        .map(|sym| {
                                            format!(
                                                "{} [L{}-{}]",
                                                sym.name,
                                                sym.range.start_line,
                                                sym.range.end_line
                                            )
                                        })
                                        .collect()
                                };
                                if symbol_notes.is_empty() {
                                    text.push_str(&format!(
                                        "- {} ({} lines)\n",
                                        res.file_path, res.total_lines
                                    ));
                                } else {
                                    text.push_str(&format!(
                                        "- {} ({} lines) — {}\n",
                                        res.file_path,
                                        res.total_lines,
                                        symbol_notes.join(", ")
                                    ));
                                }
                                shown_tail += 1;
                            }
                            if remaining_results.len() > shown_tail {
                                text.push_str(&format!(
                                    "- _… {} more files not shown; refine the query to narrow._\n",
                                    remaining_results.len() - shown_tail
                                ));
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
                        // Accept the same path aliases as `read` ('file_path'/'file'/'query'):
                        // an unknown param (e.g. `{"query": "file.cpp"}`) used to silently fall
                        // back to the ROOT overview, wasting agent turns. Earlier aliases win.
                        // An empty or "." path means the repo root overview, not a folder
                        // named "" — normalize so it renders the root view (Child 03).
                        let path = ["path", "file_path", "file", "query"]
                            .iter()
                            .find_map(|key| arguments.get(*key).and_then(|v| v.as_str()))
                            .filter(|p| !p.is_empty() && *p != ".");
                        let format = arguments.get("format").and_then(|v| v.as_str());

                        if let Some(p) = path {
                            if crate::workspace::resolve_within_cwd(p).is_err() {
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
                        self.engine.ensure_alive();
                        self.engine.trigger_refresh();
                        let snapshot = self.engine.codemap_snapshot();
                        let extracted_files: &[crate::parser::ExtractedFile] = &snapshot;

                        // Nothing to show yet because the initial index is still building (or
                        // the indexer thread died before it finished): say so rather than
                        // render an empty codemap.
                        if extracted_files.is_empty()
                            && (self.engine.is_warming() || self.engine.is_dead())
                        {
                            let text = if self.engine.is_dead() {
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
                            let canonical_target =
                                crate::workspace::canonicalize_path_lenient(&target_path);
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
