//! Native Jev activation through the stdio MCP server, answered offline by the debug-build
//! scripted transport. No case reaches the provider.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

use crate::e2e::helpers::{create_mock_repo, JevScript, McpClient, JEV_TEST_KEY_ENV};

const UPLOAD_RS: &str = "pub struct RetryPolicy {
    max_attempts: u32,
}

impl RetryPolicy {
    pub fn allows(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

pub fn retry_upload(policy: &RetryPolicy, chunk: &[u8]) -> bool {
    let mut attempt = 0;
    while policy.allows(attempt) {
        if send_chunk(chunk) {
            return true;
        }
        attempt += 1;
    }
    false
}

fn send_chunk(chunk: &[u8]) -> bool {
    !chunk.is_empty()
}

pub fn format_retry_size(bytes: usize) -> String {
    format!(\"{bytes} bytes\")
}
";
const TASK_QUERY: &str = "How does upload retry decide to stop?";
const SEARCH_QUERY: &str = "retry upload size";
/// A displayed line of the `format_retry_size` body.
const UNRELATED_BODY_LINE: &str = "format!(\"{bytes} bytes\")";
/// A displayed line of the `retry_upload` body.
const RELATED_BODY_LINE: &str = "while policy.allows(attempt)";

struct JevServer {
    client: McpClient,
    script: JevScript,
    global: TempDir,
    _scripts: TempDir,
    _repo: TempDir,
}

impl JevServer {
    /// A server over `src/upload.rs` whose global config holds `[analysis.jev]` `settings`
    /// (plus the test key variable and no request spacing) and whose Jev answers follow
    /// `script`.
    async fn start(settings: &str, script: Value, has_key: bool) -> Self {
        let repo = create_mock_repo(&[("src/upload.rs", UPLOAD_RS)]).unwrap();
        let global = tempfile::tempdir().unwrap();
        write_global_config(global.path(), settings);
        let scripts = tempfile::tempdir().unwrap();
        let script = JevScript::new(scripts.path(), script);
        let client = McpClient::spawn_with_jev(repo.path(), global.path(), &script, has_key)
            .await
            .unwrap();
        Self {
            client,
            script,
            global,
            _scripts: scripts,
            _repo: repo,
        }
    }

    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.client
            .send_request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await
            .unwrap()
    }

    async fn text(&mut self, name: &str, arguments: Value) -> String {
        text_of(&self.call(name, arguments).await)
    }
}

fn write_global_config(global: &Path, settings: &str) {
    std::fs::write(
        global.join("config.toml"),
        format!(
            "[analysis.jev]\napi_key_env = \"{JEV_TEST_KEY_ENV}\"\nrequest_spacing_ms = 0\n{settings}\n"
        ),
    )
    .unwrap();
}

fn text_of(response: &Value) -> String {
    assert_eq!(response["jsonrpc"], "2.0", "{response}");
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("tool text expected: {response}"))
        .to_string()
}

/// The unrelated body judged 0.8, everything else 0.1.
fn search_answers() -> Value {
    json!({"rules": [
        {"type": "noul", "contains": "format_retry_size", "noul": 0.8},
        {"type": "noul", "noul": 0.1}
    ]})
}

/// `upload.rs` directly relevant, other files not, and roles for its declarations.
fn overview_answers() -> Value {
    json!({"rules": [
        {"type": "score", "contains": "upload.rs", "probabilities": [0.0, 0.0, 0.2, 0.8]},
        {"type": "score", "probabilities": [1.0, 0.0, 0.0, 0.0]},
        {"type": "choice", "choice": "entry"}
    ]})
}

fn search_arguments(task_query: Option<&str>) -> Value {
    let mut arguments = json!({ "query": SEARCH_QUERY });
    if let Some(task_query) = task_query {
        arguments["task_query"] = json!(task_query);
    }
    arguments
}

/// The output before an appended Jev note.
fn without_note<'a>(text: &'a str, tool: &str) -> &'a str {
    text.split(&format!("\n\n[jev {tool}:")).next().unwrap()
}

async fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "condition not reached in time");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn test_jev_threshold_is_captured_per_call_across_a_config_change() {
    let mut server = JevServer::start(
        "is_search_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.70",
        search_answers(),
        true,
    )
    .await;
    // Polls through the warm-up; a call without intent never evaluates.
    let regular = server.text("search", search_arguments(None)).await;
    assert!(
        regular.ends_with("[jev search: bypassed (no_task_query) · original output unchanged]"),
        "{regular}"
    );
    assert!(regular.contains(UNRELATED_BODY_LINE) && regular.contains(RELATED_BODY_LINE));
    assert!(server.script.requests().is_empty());

    let filtered = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert!(
        filtered.contains("[jev search: applied · omitted=1/2 bodies · judged=2 · threshold=0.70"),
        "{filtered}"
    );
    assert!(!filtered.contains(UNRELATED_BODY_LINE), "{filtered}");
    assert!(filtered.contains(RELATED_BODY_LINE));
    assert!(
        filtered.contains("format_retry_size `src/upload.rs` L26-28"),
        "{filtered}"
    );
    assert!(
        filtered.contains("- Symbol: format_retry_size (fn) [L26-28]"),
        "{filtered}"
    );
    let requests = server.script.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["state"]["task_query"], TASK_QUERY);
    assert_eq!(
        requests[0]["state"]["search_arguments"]["query"],
        SEARCH_QUERY
    );

    // Change the threshold while an evaluation is in flight.
    server
        .script
        .update(json!({"delay_ms": 3000, "rules": search_answers()["rules"]}));
    let id = server
        .client
        .start_request(
            "tools/call",
            json!({ "name": "search", "arguments": search_arguments(Some(TASK_QUERY)) }),
        )
        .await
        .unwrap();
    let script = &server.script;
    wait_for(|| script.requests().len() == 2).await;
    write_global_config(
        server.global.path(),
        "is_search_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.90",
    );
    // The config watcher debounces for one second; the evaluation is still sleeping.
    tokio::time::sleep(Duration::from_millis(2200)).await;
    let in_flight = server.client.read_response().await.unwrap();
    assert_eq!(in_flight["id"], id);
    let in_flight = text_of(&in_flight);
    assert!(
        in_flight.contains("omitted=1/2 bodies · judged=2 · threshold=0.70"),
        "{in_flight}"
    );

    server.script.update(search_answers());
    let params = search_arguments(Some(TASK_QUERY));
    let next = server
        .client
        .send_tool_until("search", params, |text| text.contains("threshold=0.90"))
        .await
        .unwrap();
    let next = text_of(&next);
    assert!(
        next.contains("omitted=0/2 bodies · judged=2 · threshold=0.90"),
        "{next}"
    );
    assert!(
        next.contains(UNRELATED_BODY_LINE),
        "0.8 stays below 0.90: {next}"
    );
    assert_eq!(
        without_note(&next, "search"),
        without_note(&regular, "search")
    );
}

#[tokio::test]
async fn test_jev_modes_are_independent_and_off_by_default() {
    // Both modes off: task_query is accepted and ignored, nothing is sent.
    let mut server = JevServer::start("", overview_answers(), true).await;
    let regular_search = server.text("search", search_arguments(None)).await;
    let with_intent = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert_eq!(with_intent, regular_search);
    assert!(!with_intent.contains("[jev"));
    let regular_overview = server.text("overview", json!({})).await;
    let overview_with_intent = server
        .text("overview", json!({ "task_query": TASK_QUERY }))
        .await;
    assert_eq!(overview_with_intent, regular_overview);
    let tools = server
        .client
        .send_request("tools/list", json!({}))
        .await
        .unwrap();
    for tool in tools["result"]["tools"].as_array().unwrap() {
        assert_eq!(tool["annotations"]["openWorldHint"], false, "{tool}");
    }
    assert!(server.script.requests().is_empty());

    // Overview only: search stays regular and silent.
    let mut server = JevServer::start("is_overview_enabled = true", overview_answers(), true).await;
    let search = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert_eq!(search, regular_search);
    let overview = server
        .text("overview", json!({ "task_query": TASK_QUERY }))
        .await;
    assert!(overview.starts_with(&regular_overview), "{overview}");
    assert!(
        overview.contains("## Indexed file recommendations"),
        "{overview}"
    );
    assert!(overview.contains("### 1. src/upload.rs"), "{overview}");
    assert!(
        overview.contains("[jev overview: applied · status=matched · requests=2"),
        "{overview}"
    );
    let requests = server.script.requests();
    assert_eq!(requests.len(), 2, "file scores, then declaration roles");
    assert!(requests
        .iter()
        .all(|request| request["state"]["task_query"] == TASK_QUERY));
    let tools = server
        .client
        .send_request("tools/list", json!({}))
        .await
        .unwrap();
    let hints: Vec<(String, Value)> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            (
                tool["name"].as_str().unwrap().to_string(),
                tool["annotations"]["openWorldHint"].clone(),
            )
        })
        .collect();
    for (name, hint) in hints {
        assert_eq!(hint, json!(name == "overview"), "{name}");
    }

    // Search only: overview stays regular and silent.
    let mut server =
        JevServer::start("is_search_filter_enabled = true", search_answers(), true).await;
    let overview = server
        .text("overview", json!({ "task_query": TASK_QUERY }))
        .await;
    assert_eq!(overview, regular_overview);
    let search = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert!(
        search.contains("[jev search: applied · omitted=1/2 bodies"),
        "{search}"
    );
    assert_eq!(server.script.requests().len(), 1);
}

#[tokio::test]
async fn test_jev_zero_match_overview_keeps_the_base_overview() {
    let answers = json!({"rules": [
        {"type": "score", "probabilities": [0.7, 0.3, 0.0, 0.0]},
        {"type": "choice", "choice": "entry"}
    ]});
    let mut server = JevServer::start("is_overview_enabled = true", answers, true).await;
    let base = server.text("overview", json!({})).await;
    assert!(
        base.ends_with("[jev overview: bypassed (no_task_query) · original output unchanged]"),
        "{base}"
    );
    let base = without_note(&base, "overview").to_string();
    let overview = server
        .text("overview", json!({ "task_query": TASK_QUERY }))
        .await;
    assert!(overview.starts_with(&base), "{overview}");
    assert!(overview.contains("Status: no_match"), "{overview}");
    assert!(
        overview.contains("this does not show that the implementation is absent"),
        "{overview}"
    );
    assert!(
        !overview.contains("### 1."),
        "no forced recommendation: {overview}"
    );
    assert!(
        overview.contains("[jev overview: applied · status=no_match · requests=1"),
        "{overview}"
    );
    assert_eq!(
        server.script.requests().len(),
        1,
        "no role stage without a selected file"
    );

    // A folder view never evaluates; the intent is reported as unused.
    let folder = server
        .text(
            "overview",
            json!({ "path": "src", "task_query": TASK_QUERY }),
        )
        .await;
    assert!(
        folder.ends_with(
            "[jev overview: bypassed (not_repository_root) · original output unchanged]"
        ),
        "{folder}"
    );
    assert_eq!(server.script.requests().len(), 1);
}

#[tokio::test]
async fn test_jev_missing_key_intent_and_failures_preserve_the_base_output() {
    let settings = "is_search_filter_enabled = true\nis_overview_enabled = true\ntimeout_ms = 1000";
    let mut keyless = JevServer::start(settings, search_answers(), false).await;
    let search = keyless
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert!(
        search.ends_with("[jev search: bypassed (missing_credential) · original output unchanged]"),
        "{search}"
    );
    let overview = keyless
        .text("overview", json!({ "task_query": TASK_QUERY }))
        .await;
    assert!(
        overview
            .ends_with("[jev overview: bypassed (missing_credential) · original output unchanged]"),
        "{overview}"
    );
    assert!(keyless.script.requests().is_empty());

    let mut server = JevServer::start(settings, search_answers(), true).await;
    let regular = server.text("search", search_arguments(Some("   "))).await;
    assert!(
        regular.ends_with("[jev search: bypassed (no_task_query) · original output unchanged]"),
        "{regular}"
    );
    let base = without_note(&regular, "search").to_string();
    for (tool, arguments) in [
        ("search", json!({ "query": SEARCH_QUERY, "task_query": 7 })),
        ("overview", json!({ "task_query": ["retry"] })),
    ] {
        let response = server.call(tool, arguments).await;
        assert_eq!(response["error"]["code"], -32602, "{response}");
    }
    assert!(server.script.requests().is_empty());

    server
        .script
        .update(json!({"status": 529, "rules": search_answers()["rules"]}));
    let overloaded = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert!(
        overloaded.contains(
            "[jev search: fallback (http_status) · original output preserved · requests=1"
        ),
        "{overloaded}"
    );
    assert_eq!(without_note(&overloaded, "search"), base);

    server
        .script
        .update(json!({"delay_ms": 3000, "rules": search_answers()["rules"]}));
    let late = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert!(
        late.contains("[jev search: fallback (deadline_exceeded) · original output preserved"),
        "{late}"
    );
    assert_eq!(without_note(&late, "search"), base);

    server
        .script
        .update(json!({"rules": [{"type": "noul", "contains": "retry_upload", "noul": 0.9}]}));
    let unanswered = server
        .text("search", search_arguments(Some(TASK_QUERY)))
        .await;
    assert!(
        unanswered
            .contains("[jev search: fallback (answer_set_mismatch) · original output preserved"),
        "{unanswered}"
    );
    assert_eq!(without_note(&unanswered, "search"), base);

    server.script.update(json!({"status": 529}));
    let overview = server
        .text("overview", json!({ "task_query": TASK_QUERY }))
        .await;
    assert!(
        overview.contains(
            "[jev overview: fallback (http_status) · original output preserved · requests=1"
        ),
        "{overview}"
    );
    assert!(
        !overview.contains("## Indexed file recommendations"),
        "{overview}"
    );
}

#[tokio::test]
async fn test_jev_live_tools_and_other_methods_never_evaluate() {
    let settings = "is_search_filter_enabled = true\nis_overview_enabled = true";
    let mut server = JevServer::start(settings, search_answers(), true).await;
    server.text("search", search_arguments(None)).await;
    let instructions = server.text("initial_instructions", json!({})).await;
    assert!(
        instructions.contains(
            "Optional Jev judgments are enabled for overview recommendations and search filtering."
        ),
        "{instructions}"
    );
    let read = server
        .text(
            "read",
            json!({ "file_path": "src/upload.rs", "view": "source" }),
        )
        .await;
    assert!(read.contains(UNRELATED_BODY_LINE));
    let grep = server
        .text("grep", json!({ "pattern": "format_retry_size" }))
        .await;
    assert!(grep.contains("format_retry_size"));
    let find = server.text("find", json!({ "pattern": "*.rs" })).await;
    assert!(find.contains("upload.rs"));
    let ping = server.client.send_request("ping", json!({})).await.unwrap();
    assert_eq!(ping["result"], json!({}));
    assert!(server.script.requests().is_empty());
}
