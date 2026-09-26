//! Whole-feature regression matrix for the optional Jev stages (child 05). Every scenario
//! drives the real MCP pipeline in-process with an offline evaluator: an injected
//! `MockEvaluator`, or the real `JevEvaluator` over a scripted `MockTransport`. No scenario
//! reads credentials or opens a connection, and no scenario duplicates the retention policy:
//! expectations are checked on the delivered tool text, on what the evaluator received and
//! on the `jev stage` diagnostic lines (the canonical record of every outcome).

use crate::e2e::helpers::{
    create_mock_repo, response_text, with_in_process_server as run_server, InProcessClient,
};
use codemap_search::jev::mock::{answers, MockEvaluator, MockTransport, ScriptedResponse};
use codemap_search::jev::{
    EvaluationRequest, Evaluator, EvaluatorConfig, JevEvaluator, QuestionKind, Transport, Usage,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TASK: &str = "how is the output byte budget reserved before rendering?";
const TASK_KO: &str = "출력 바이트 예산은 렌더링 전에 어디에서 확보되나요?";

const BUDGET_TS: &str =
    "export function keepMe(remainingBytes: number, footer: string, limit: number): number {
  const reserve = footer.length + 8;
  if (remainingBytes <= reserve) {
    return 0;
  }
  const allowed = Math.min(limit, remainingBytes - reserve);
  return allowed;
}

export function dropMe(input: string): string {
  const parts = input.split(\",\");
  const trimmed = parts.map((part) => part.trim());
  const joined = trimmed.join(\"|\");
  return joined.toUpperCase();
}
";

/// `keepMe` (eight lines) plus a three-line callable whose long lines make its body larger
/// than its omission note.
const WINDOW_TS: &str = "export function keepMe(remainingBytes: number, footer: string, limit: number): number {
  const reserve = footer.length + 8;
  if (remainingBytes <= reserve) {
    return 0;
  }
  const allowed = Math.min(limit, remainingBytes - reserve);
  return allowed;
}

export function wideDrop(input: string): string {
  return input.split(\",\").map((part) => part.trim()).filter((part) => part.length > 0).map((part) => part.replace(/[^a-z0-9]/gi, \"_\")).join(\"|\").toUpperCase().padEnd(160, \"-\").concat(\"--end--\");
}
";

/// Rust evidence with a constant, two structs, an impl with a related method and a method
/// that calls it, and a free function without any link.
const CHECKOUT_RS: &str = "pub const MAX_LINES: usize = 12;

pub struct OrderLine {
    pub price: i64,
    pub quantity: i64,
}

pub struct Checkout {
    pub lines: Vec<OrderLine>,
}

impl Checkout {
    pub fn total(&self) -> i64 {
        self.lines.iter().map(|line| line.price * line.quantity).sum()
    }

    pub fn submit(&self) -> Result<i64, String> {
        if self.lines.len() > MAX_LINES {
            return Err(\"too many lines\".into());
        }
        Ok(self.total())
    }
}

pub fn banner(name: &str) -> String {
    let underline = \"=\".repeat(name.len() + 6);
    let mut rendered = String::with_capacity(name.len() * 2 + underline.len() + 8);
    rendered.push_str(\"== \");
    rendered.push_str(name);
    rendered.push_str(\" ==\\n\");
    rendered.push_str(&underline);
    rendered
}
";

const BOTH_STAGES: &str = "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\n";
const FILTER_ONLY: &str = "[analysis.jev]\nsearch_filter_enabled = true\n";

async fn with_in_process_server<F, Fut>(
    root: &std::path::Path,
    evaluator: Option<Arc<dyn Evaluator>>,
    script: F,
) where
    F: FnOnce(InProcessClient) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    run_server(root, evaluator, |mut client| async move {
        client.register_task(TASK).await;
        script(client).await;
    })
    .await;
}

fn search_arguments(query: &str) -> Value {
    // These assertions isolate Jev selection from independent delivery deduplication.
    json!({ "query": query, "include_seen": true })
}

/// One judge for both stages: files under `qualifying_prefix` qualify with a clear margin,
/// files under `tied_prefix` return equal useful/unhelpful mass, everything else is
/// unhelpful; bodies are judged unrelated by declaration name. Overview never requests roles.
fn stage_judge(
    qualifying_prefix: &'static str,
    tied_prefix: &'static str,
    unrelated_by_name: &'static [(&'static str, f64)],
) -> MockEvaluator {
    MockEvaluator::new(move |request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|question| {
                let answer = match question.kind() {
                    QuestionKind::Score { .. } => {
                        let path = question.instructions()["candidate"]["file_path"]
                            .as_str()
                            .unwrap_or("");
                        if !qualifying_prefix.is_empty() && path.starts_with(qualifying_prefix) {
                            answers::score(&[0.45, 0.0, 0.55, 0.0])
                        } else if !tied_prefix.is_empty() && path.starts_with(tied_prefix) {
                            answers::score(&[0.5, 0.0, 0.0, 0.5])
                        } else {
                            answers::score(&[0.8, 0.1, 0.05, 0.05])
                        }
                    }
                    QuestionKind::Choice { .. } => {
                        panic!("overview must not infer roles from names")
                    }
                    QuestionKind::Noul { .. } => {
                        let name = question.instructions()["candidate"]["name"]
                            .as_str()
                            .unwrap_or("");
                        let unrelated = unrelated_by_name
                            .iter()
                            .find(|(candidate, _)| *candidate == name)
                            .map_or(0.0, |(_, probability)| *probability);
                        answers::noul(1.0 - unrelated)
                    }
                };
                (question.id().clone(), answer)
            })
            .collect())
    })
}

fn body_judge(unrelated_by_name: &'static [(&'static str, f64)]) -> MockEvaluator {
    stage_judge("", "", unrelated_by_name)
}

/// Name, kind and owner of every Noul question the evaluator received, per request.
fn judged_names(evaluator: &MockEvaluator) -> Vec<Vec<(String, String, Option<String>)>> {
    evaluator
        .requests()
        .iter()
        .map(|request| {
            request.state["candidates"]
                .as_object()
                .unwrap()
                .values()
                .map(|candidate| {
                    (
                        candidate["name"].as_str().unwrap().to_string(),
                        candidate["kind"].as_str().unwrap().to_string(),
                        candidate["owner"].as_str().map(str::to_string),
                    )
                })
                .collect()
        })
        .collect()
}

// --- Diagnostic capture -----------------------------------------------------------------

#[derive(Clone, Default)]
struct LogCapture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogCapture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl LogCapture {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }

    /// Every `jev stage` line, parsed into `field -> value` (quotes stripped).
    fn stages(&self) -> Vec<BTreeMap<String, String>> {
        self.text()
            .lines()
            .filter(|line| line.contains("jev stage"))
            .map(parse_fields)
            .collect()
    }
}

/// Parse `key=value` and `key="value"` pairs of one tracing line; quoted values may contain
/// spaces and `=`.
fn parse_fields(line: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut rest = line;
    while let Some(position) = rest.find('=') {
        let key = rest[..position]
            .rsplit(|character: char| character.is_whitespace())
            .next()
            .unwrap_or("")
            .to_string();
        rest = &rest[position + 1..];
        let value = if let Some(stripped) = rest.strip_prefix('"') {
            let end = stripped.find('"').unwrap_or(stripped.len());
            let value = stripped[..end].to_string();
            rest = &stripped[end.min(stripped.len())..];
            rest = rest.strip_prefix('"').unwrap_or(rest);
            value
        } else {
            let end = rest
                .find(|character: char| character.is_whitespace())
                .unwrap_or(rest.len());
            let value = rest[..end].to_string();
            rest = &rest[end..];
            value
        };
        if !key.is_empty() {
            fields.insert(key, value);
        }
    }
    fields
}

/// Install a thread-scoped tracing subscriber that captures `jev stage` lines.
fn capture_logs() -> (LogCapture, tracing::subscriber::DefaultGuard) {
    let capture = LogCapture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    (capture, guard)
}

/// The `detail` field of an applied search stage, as `name -> number`.
fn detail_counts(stage: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    stage["detail"]
        .split_whitespace()
        .filter_map(|token| token.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[tokio::test]
async fn test_jev_disabled_stages_ignore_intent_and_an_injected_evaluator() {
    let temp = create_mock_repo(&[("src/budget.ts", BUDGET_TS)]).unwrap();
    let evaluator = Arc::new(body_judge(&[("dropMe", 0.99), ("keepMe", 0.99)]));
    let judge = Arc::clone(&evaluator);
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        let plain = client
            .plain_call("search", search_arguments("keepMe dropMe"))
            .await
            .unwrap();
        client.register_task(TASK_KO).await;
        let with_intent = client
            .call(
                "tools/call",
                json!({ "name": "search", "arguments": search_arguments("keepMe dropMe") }),
            )
            .await
            .unwrap();
        assert_eq!(
            response_text(&plain),
            response_text(&with_intent),
            "task_query is inert while both stages are off"
        );
        assert!(!response_text(&plain).contains("Jev"));

        let overview = client
            .call_tool_until("overview", json!({ "task_query": TASK }), |text| {
                text.contains("src/budget.ts") && !text.contains("warming up")
            })
            .await
            .unwrap();
        assert!(
            !response_text(&overview).contains("Jev"),
            "{}",
            response_text(&overview)
        );
        assert_eq!(
            judge.request_count(),
            0,
            "an injected evaluator is never consulted while both stages are off"
        );

        let instructions = client
            .call(
                "tools/call",
                json!({ "name": "initial_instructions", "arguments": {} }),
            )
            .await
            .unwrap();
        assert!(
            !response_text(&instructions).contains("are enabled"),
            "no Jev guidance while disabled"
        );
        let tools = client.call("tools/list", json!({})).await.unwrap();
        for tool in tools["result"]["tools"].as_array().unwrap() {
            let description = tool["description"].as_str().unwrap();
            assert!(
                !description.contains("is enabled"),
                "{}: {description}",
                tool["name"]
            );
            assert_eq!(
                tool["annotations"]["openWorldHint"],
                json!(false),
                "{}",
                tool["name"]
            );
            assert_eq!(
                tool["annotations"]["readOnlyHint"],
                json!(true),
                "{}",
                tool["name"]
            );
        }
    })
    .await;
}

#[tokio::test]
async fn test_jev_search_protects_partial_windows_and_omits_only_complete_bodies() {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "[analysis.jev]\nsearch_filter_enabled = true\n[output.search]\nsnippet_max_lines = 3\n"),
        ("src/window.ts", WINDOW_TS),
    ])
    .unwrap();
    let (capture, _guard) = capture_logs();
    let evaluator = Arc::new(body_judge(&[("keepMe", 0.99), ("wideDrop", 0.99)]));
    let judge = Arc::clone(&evaluator);
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        tracing::callsite::rebuild_interest_cache();
        let response = client
            .call_tool_until("search", search_arguments("keepMe wideDrop"), |text| text.contains("_omitted body:"))
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(text.contains("const reserve = footer.length + 8;"), "the partial window of keepMe stays: {text}");
        assert!(text.contains("more lines)"), "the window keeps its elision marker: {text}");
        assert!(!text.contains("padEnd(160"), "the complete unrelated body is omitted: {text}");
        assert!(text.contains("- _omitted body: L10-12 (fn wideDrop) did not match the task questions; read src/window.ts offset 10 limit 3 to inspect._"), "{text}");
        assert_eq!(judged_names(&judge), vec![vec![("wideDrop".to_string(), "fn".to_string(), None)]], "only the complete body is judged");
    })
    .await;
    let stages = capture.stages();
    let applied = stages
        .iter()
        .find(|stage| stage["outcome"] == "applied" && stage["tool"] == "search")
        .unwrap_or_else(|| panic!("no applied search stage in:\n{}", capture.text()));
    let counts = detail_counts(applied);
    assert_eq!(counts["bodies"], "2", "{applied:?}");
    assert_eq!(counts["judged"], "1", "{applied:?}");
    assert_eq!(counts["omitted"], "1", "{applied:?}");
    assert_eq!(counts["rendered_omissions"], "1", "{applied:?}");
    assert_eq!(counts["protected"], "1", "{applied:?}");
    assert_eq!(counts["linked"], "0", "{applied:?}");
}

#[tokio::test]
async fn test_jev_search_keeps_linked_rust_methods_and_never_judges_data_declarations() {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", FILTER_ONLY),
        ("src/checkout.rs", CHECKOUT_RS),
    ])
    .unwrap();
    let (capture, _guard) = capture_logs();
    let evaluator = Arc::new(body_judge(&[
        ("banner", 0.95),
        ("submit", 0.90),
        ("total", 0.10),
    ]));
    let judge = Arc::clone(&evaluator);
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        tracing::callsite::rebuild_interest_cache();
        client.register_task("how is the order total computed at checkout?").await;
        let response = client
            .call_tool_until(
                "search",
                search_arguments("banner submit total MAX_LINES"),
                |text| text.contains("_omitted body:"),
            )
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(text.contains("- _omitted body: L25-33 (fn banner) did not match the task questions; read src/checkout.rs offset 25 limit 9 to inspect._"), "{text}");
        assert!(!text.contains("rendered.push_str"), "{text}");
        assert!(text.contains("Ok(self.total())"), "submit stays through its call to the related total: {text}");
        assert!(text.contains("line.price * line.quantity"), "{text}");
        assert!(text.contains("pub const MAX_LINES: usize = 12;"), "data declarations keep their source: {text}");
        assert!(text.contains("Checkout::submit"), "methods keep their owner: {text}");
        let mut names = judged_names(&judge).remove(0);
        names.sort();
        assert_eq!(
            names,
            vec![
                ("banner".to_string(), "fn".to_string(), None),
                ("submit".to_string(), "fn".to_string(), Some("Checkout".to_string())),
                ("total".to_string(), "fn".to_string(), Some("Checkout".to_string())),
            ],
            "only callable bodies are judged; the constant and the structs are never asked"
        );
    })
    .await;
    // Four displayed bodies: three callables (judged) and the constant (protected, never asked).
    let stages = capture.stages();
    let applied = stages
        .iter()
        .find(|stage| stage["outcome"] == "applied")
        .unwrap_or_else(|| panic!("no applied stage in:\n{}", capture.text()));
    let counts = detail_counts(applied);
    assert_eq!(counts["bodies"], "4", "{applied:?}");
    assert_eq!(counts["judged"], "3", "{applied:?}");
    assert_eq!(counts["omitted"], "1", "{applied:?}");
    assert_eq!(counts["protected"], "1", "{applied:?}");
    assert_eq!(counts["linked"], "1", "{applied:?}");
}

#[tokio::test]
async fn test_jev_configured_deadline_wins_over_the_runtime_default_through_mcp() {
    // 1.5 s leaves room for the synchronous preparation (ranking and rendering in a debug
    // build) so the request is actually dispatched; the transport then outlasts the deadline.
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\ntimeout_ms = 1500\n"),
        ("src/budget.ts", BUDGET_TS),
    ])
    .unwrap();
    let (capture, _guard) = capture_logs();
    let transport = Arc::new(MockTransport::with_handler(|_| {
        ScriptedResponse::ok("{}").after(Duration::from_millis(6_000))
    }));
    let evaluator = JevEvaluator::new(
        Arc::clone(&transport) as Arc<dyn Transport>,
        EvaluatorConfig::default(),
    )
    .expect("runtime defaults validate");
    let posts = Arc::clone(&transport);
    with_in_process_server(
        temp.path(),
        Some(Arc::new(evaluator) as Arc<dyn Evaluator>),
        |mut client| async move {
            tracing::callsite::rebuild_interest_cache();
            let plain = client
                .plain_call("search", search_arguments("keepMe dropMe"))
                .await
                .unwrap();
            let started = std::time::Instant::now();
            let response = client
                .call(
                    "tools/call",
                    json!({ "name": "search", "arguments": search_arguments("keepMe dropMe") }),
                )
                .await
                .unwrap();
            assert_eq!(
                response_text(&response),
                response_text(&plain),
                "a deadline fallback returns the base output byte for byte"
            );
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the 1.5 s deadline is enforced, not the 45 s runtime default"
            );
            assert_eq!(posts.post_count(), 1, "one attempt, no retry");

            let plain = client.plain_call("overview", json!({})).await.unwrap();
            let response = client
                .call(
                    "tools/call",
                    json!({ "name": "overview", "arguments": { "task_query": TASK } }),
                )
                .await
                .unwrap();
            assert_eq!(
                response_text(&response),
                response_text(&plain),
                "the base overview is unchanged on fallback"
            );
            assert_eq!(posts.post_count(), 1, "overview never evaluates");
        },
    )
    .await;
    let stages = capture.stages();
    let fallbacks: Vec<&BTreeMap<String, String>> = stages
        .iter()
        .filter(|stage| stage["outcome"] == "fallback")
        .collect();
    assert_eq!(fallbacks.len(), 1, "{}", capture.text());
    assert!(
        fallbacks
            .iter()
            .all(|stage| stage["status"] == "fallback:deadline_exceeded"),
        "{fallbacks:?}"
    );
    assert!(
        fallbacks.iter().all(|stage| stage["attempts"] == "1"),
        "{fallbacks:?}"
    );
    assert!(
        fallbacks
            .iter()
            .all(|stage| stage["input_tokens"] == "None" && stage["unreported_responses"] == "0"),
        "an unanswered request has unknown, not zero, usage: {fallbacks:?}"
    );
}

#[tokio::test]
async fn test_jev_context_and_batch_limits_fail_explicitly_without_sending() {
    let mut bulky =
        String::from("export function bulky(): string[] {\n  const rows: string[] = [];\n");
    for index in 0..24 {
        bulky.push_str(&format!(
            "  rows.push(\"row-{index:02}-{}\");\n",
            "x".repeat(64)
        ));
    }
    bulky.push_str("  return rows;\n}\n");
    let temp = create_mock_repo(&[
        (".codemap/config.toml", FILTER_ONLY),
        ("src/bulky.ts", bulky.as_str()),
    ])
    .unwrap();

    let cases: [(&str, EvaluatorConfig); 3] = [
        (
            "estimated_token_limit",
            EvaluatorConfig {
                max_estimated_tokens: 1,
                ..EvaluatorConfig::default()
            },
        ),
        (
            "estimated_token_limit",
            EvaluatorConfig {
                max_estimated_state_plus_longest_question_tokens: 1,
                ..EvaluatorConfig::default()
            },
        ),
        (
            "question_too_large",
            EvaluatorConfig {
                max_batch_bytes: 1,
                ..EvaluatorConfig::default()
            },
        ),
    ];
    for (expected_kind, config) in cases {
        let (capture, _guard) = capture_logs();
        let transport = Arc::new(MockTransport::scripted(Vec::new()));
        let evaluator = JevEvaluator::new(Arc::clone(&transport) as Arc<dyn Transport>, config)
            .expect("limits validate");
        let posts = Arc::clone(&transport);
        with_in_process_server(
            temp.path(),
            Some(Arc::new(evaluator) as Arc<dyn Evaluator>),
            |mut client| async move {
                tracing::callsite::rebuild_interest_cache();
                let plain = client
                    .plain_call("search", search_arguments("bulky"))
                    .await
                    .unwrap();
                let response = client
                    .call(
                        "tools/call",
                        json!({ "name": "search", "arguments": search_arguments("bulky") }),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    response_text(&response),
                    response_text(&plain),
                    "{expected_kind}: the full body is preserved byte for byte"
                );
                assert_eq!(
                    posts.post_count(),
                    0,
                    "{expected_kind}: nothing is sent when a limit fails"
                );
            },
        )
        .await;
        let stages = capture.stages();
        let fallback = stages
            .iter()
            .find(|stage| stage["outcome"] == "fallback")
            .unwrap_or_else(|| panic!("{expected_kind}: no fallback line in:\n{}", capture.text()));
        assert_eq!(fallback["status"], format!("fallback:{expected_kind}"));
        assert_eq!(fallback["attempts"], "0");
    }
}

#[tokio::test]
async fn test_jev_stage_logs_report_usage_and_outcome_without_evidence() {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", FILTER_ONLY),
        ("src/budget.ts", BUDGET_TS),
    ])
    .unwrap();
    let (capture, _guard) = capture_logs();
    let evaluator = Arc::new(body_judge(&[("dropMe", 0.80)]).with_usage(Usage::reported(296, 20)));
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        // Tests share one process: a callsite first hit by another test thread caches its
        // interest against that thread's (absent) subscriber, and a scoped subscriber does
        // not revisit it. Rebuild the cache on this thread once the server is ours.
        tracing::callsite::rebuild_interest_cache();
        client
            .call_tool_until(
                "search",
                search_arguments("keepMe dropMe"),
                |text| text.contains("_omitted body:"),
            )
            .await
            .unwrap();
        client
            .call(
                "tools/call",
                json!({ "name": "search", "arguments": search_arguments("keepMe dropMe") }),
            )
            .await
            .unwrap();
        // An inherited read filter logs a bypass for an incomplete body without inference.
        client
            .call(
                "tools/call",
                json!({ "name": "read", "arguments": { "file_path": "src/budget.ts", "limit": 2 } }),
            )
            .await
            .unwrap();
    })
    .await;
    let log = capture.text();
    let stages = capture.stages();
    let applied = stages
        .iter()
        .find(|stage| stage["outcome"] == "applied")
        .unwrap_or_else(|| panic!("no applied stage line in:\n{log}"));
    assert_eq!(applied["tool"], "search");
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["model"], "jev-1.13.0");
    assert_eq!(
        applied["versions"],
        "search-task-evidence/7 search-task-questions/5 search-selection-policy/4-experimental"
    );
    let counts = detail_counts(applied);
    assert_eq!(counts["bodies"], "2");
    assert_eq!(counts["judged"], "2");
    assert_eq!(counts["omitted"], "1");
    assert_eq!(counts["rendered_omissions"], "1");
    assert_eq!(counts["protected"], "0");
    assert_eq!(counts["linked"], "0");
    assert_eq!(counts["unverified"], "0");
    assert_eq!(counts["masked_unavailable"], "0");
    assert_eq!(counts["threshold"], "0.70");
    assert_eq!(counts["note_inline"], "false");
    assert_eq!(applied["input_tokens"], "Some(296)");
    assert_eq!(applied["output_tokens"], "Some(20)");
    assert_eq!(applied["reported_responses"], "1");
    assert_eq!(applied["unreported_responses"], "0");
    assert_eq!(applied["attempts"], "1");
    assert!(
        applied.contains_key("elapsed_ms")
            && applied.contains_key("http_ms")
            && applied.contains_key("queue_ms"),
        "{applied:?}"
    );
    assert!(
        stages.iter().all(|stage| (stage["tool"] == "search"
            && (stage["outcome"] == "applied"
                || (stage["outcome"] == "bypassed" && stage["status"] == "no_ranked_candidates")))
            || (stage["tool"] == "read"
                && stage["outcome"] == "bypassed"
                && stage["status"] == "bypassed:no_complete_bodies")),
        "startup searches and partial read bodies bypass evaluation: {log}"
    );
    assert_eq!(
        stages
            .iter()
            .filter(|stage| stage["outcome"] == "applied")
            .count(),
        2,
        "both populated searches run after one registration; the partial read sends no questions:\n{log}"
    );
    assert!(
        !log.contains("const reserve") && !log.contains("input.split"),
        "no source in logs:\n{log}"
    );
    assert!(!log.contains(TASK), "no intent in logs:\n{log}");
}

#[tokio::test]
async fn test_jev_korean_intent_reaches_search_verbatim() {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", BOTH_STAGES),
        ("src/budget.ts", BUDGET_TS),
    ])
    .unwrap();
    let evaluator = Arc::new(stage_judge("src/budget.ts", "", &[("dropMe", 0.80)]));
    let judge = Arc::clone(&evaluator);
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        client.register_task(TASK_KO).await;
        let response = client
            .call_tool_until("overview", json!({}), |text| text.contains("src/budget.ts"))
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(
            !text.contains("Recommended files"),
            "{text}"
        );
        let response = client
            .call_tool_until("search", search_arguments("keepMe dropMe"), |text| {
                text.contains("_omitted body:")
            })
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(text.contains("(fn dropMe) did not match the task questions"), "{text}");
        let requests = judge.requests();
        assert_eq!(requests[0].questions.len(), 4);
        for (id, question) in &requests[0].questions {
            assert!(question["instructions"].get("criterion_id").is_none());
            let expected = if id.as_str().ends_with(".q0") {
                "Does this function implement or concretely support the requested output-budget behavior?"
            } else {
                "Does the same function participate in the requested output-budget flow?"
            };
            assert_eq!(question["instructions"]["question"], expected);
            assert!(question["instructions"]["candidate"].get("body").is_none());
        }
        eprintln!("JEV_TASK_TRACE {}", json!({"state":requests[0].state,
            "questions":requests[0].questions.iter().map(|(id,wire)|json!({"id":id.as_str(),"wire":wire})).collect::<Vec<_>>(),
            "output":text}));
        assert_eq!(requests.len(), 1, "only search Noul; overview stays local");
        assert!(
            requests
                .iter()
                .all(|request| request.task_query() == Some(TASK_KO)),
            "the intent is transmitted verbatim (nothing in it needed masking)"
        );
    })
    .await;
}

#[tokio::test]
async fn test_jev_omitted_bodies_restore_through_read_and_retained_lines_are_verbatim() {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", FILTER_ONLY),
        ("src/budget.ts", BUDGET_TS),
    ])
    .unwrap();
    let evaluator = Arc::new(body_judge(&[("dropMe", 0.80)]));
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        let response = client
            .call_tool_until("search", search_arguments("keepMe dropMe"), |text| text.contains("_omitted body:"))
            .await
            .unwrap();
        let text = response_text(&response);
        for line in BUDGET_TS.lines().take(8) {
            assert!(text.contains(line), "retained line missing verbatim: {line:?}\n{text}");
        }
        for line in BUDGET_TS.lines().skip(10).take(4) {
            assert!(!text.contains(line), "omitted line still present: {line:?}\n{text}");
        }
        assert!(text.contains("dropMe (fn) [L10-15]"), "declaration rows stay: {text}");
        let restored = client
            .plain_call("read", json!({ "file_path": "src/budget.ts", "offset": 10, "limit": 6, "include_seen": true }))
            .await
            .unwrap();
        let restored = response_text(&restored);
        assert!(restored.contains("10→export function dropMe(input: string): string {"), "{restored}");
        for line in BUDGET_TS.lines().skip(10).take(5) {
            assert!(restored.contains(line), "{line:?} not restored:\n{restored}");
        }
        assert!(restored.contains("15→}"), "{restored}");
        assert!(!restored.contains("keepMe"), "the note's window covers exactly the omitted body: {restored}");
    })
    .await;
}

#[tokio::test]
async fn test_jev_output_caps_bound_stage_output() {
    // Search: the filter never grows the output beyond the configured detail cap, and a
    // capped base is not reduced to make room for filter text.
    let temp = create_mock_repo(&[
        (
            ".codemap/config.toml",
            "[analysis.jev]\nsearch_filter_enabled = true\n[output.search]\nmax_bytes = 1600\n",
        ),
        ("src/budget.ts", BUDGET_TS),
    ])
    .unwrap();
    let evaluator = Arc::new(body_judge(&[("dropMe", 0.80)]));
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        let plain = client
            .plain_call("search", search_arguments("keepMe dropMe"))
            .await
            .unwrap();
        let plain = response_text(&plain).to_string();
        assert!(plain.len() <= 1600);
        let response = client
            .call(
                "tools/call",
                json!({ "name": "search", "arguments": search_arguments("keepMe dropMe") }),
            )
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(
            response["error"].is_null() && response["result"]["isError"] != true,
            "{response}"
        );
        assert!(
            text.len() <= 1600,
            "{} bytes exceed the 1600-byte cap: {text}",
            text.len()
        );
        assert!(
            text.len() <= plain.len(),
            "the filter only ever shrinks the output"
        );
        assert!(text.contains("function keepMe"), "{text}");
    })
    .await;
}

#[tokio::test]
async fn test_jev_stale_files_are_protected_and_metadata_only_files_are_not_read_observations() {
    // The index sees `dropMe` at L10-15; before the evaluated search the file is rewritten
    // so that another declaration occupies those lines. The displayed buffer no longer
    // names `dropMe` where the index put it: the body is protected as unverified and never
    // judged, while the unchanged `keepMe` is still judged.
    let temp = create_mock_repo(&[
        (".codemap/config.toml", FILTER_ONLY),
        ("src/budget.ts", BUDGET_TS),
    ])
    .unwrap();
    let path = temp.path().join("src/budget.ts");
    let (capture, _guard) = capture_logs();
    let evaluator = Arc::new(body_judge(&[
        ("dropMe", 0.99),
        ("keepMe", 0.99),
        ("shifted", 0.99),
    ]));
    let judge = Arc::clone(&evaluator);
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        tracing::callsite::rebuild_interest_cache();
        client
            .plain_call("search", search_arguments("keepMe dropMe"))
            .await
            .unwrap();
        // Same line count, different declaration on the indexed lines of `dropMe`.
        let rewritten = BUDGET_TS.replace(
            "export function dropMe(input: string): string {",
            "export function shifted(input: string): string {",
        );
        std::fs::write(&path, rewritten).unwrap();
        let response = client
            .call(
                "tools/call",
                json!({ "name": "search", "arguments": search_arguments("keepMe dropMe") }),
            )
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(
            text.contains("function shifted"),
            "the live buffer is displayed: {text}"
        );
        assert!(
            !text.contains("(fn dropMe) did not match the task questions"),
            "a stale body is never omitted: {text}"
        );
        let judged: Vec<String> = judged_names(&judge)
            .into_iter()
            .flatten()
            .map(|(name, _, _)| name)
            .collect();
        assert_eq!(
            judged,
            vec!["keepMe".to_string()],
            "only the verified body is judged"
        );
    })
    .await;
    let stages = capture.stages();
    let applied = stages
        .iter()
        .find(|stage| stage["outcome"] == "applied")
        .unwrap_or_else(|| panic!("no applied stage in:\n{}", capture.text()));
    let counts = detail_counts(applied);
    assert_eq!(counts["unverified"], "1", "{applied:?}");
    assert_eq!(counts["judged"], "1", "{applied:?}");
    assert_eq!(
        counts["omitted"], "1",
        "keepMe (verified, judged 0.99) is omitted: {applied:?}"
    );
}
