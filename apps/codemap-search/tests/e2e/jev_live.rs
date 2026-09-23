//! Actual MCP regressions for opt-in live body filters. Every judge is injected;
//! SQLite assertions inspect the real final response accounting, not a copied policy.
use super::helpers::{
    create_mock_repo, response_text, with_in_process_server_logging, InProcessClient,
};
use codemap_search::jev::mock::{answers, Gate, MockEvaluator};
use codemap_search::jev::{EvaluationRequest, JevError};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

const TASK: &str = "locate the output budget calculation";
const CONFIG: &str = "[analysis.jev]\nread_filter_enabled = true\ngrep_filter_enabled = true\n[index.refresh]\nwatch = false\n";
const KEEP: &str =
    "export function keepBody(remainingBytes: number, footer: string, limit: number): number {
  const reserve = footer.length + 8;
  if (remainingBytes <= reserve) {
    return 0;
  }
  const allowed = Math.min(limit, remainingBytes - reserve);
  return allowed;
}";
const DROP: &str = "export function dropBody(input: string): string {
  const trimmed = input.trim();
  const uppercase = trimmed.toUpperCase();
  const separated = uppercase.replaceAll(\",\", \"|\");
  const prefixed = \"UNRELATED_MARKER:\" + separated;
  const padded = prefixed.padEnd(160, \"-\");
  return padded + \"--finished--\";
}";
fn source() -> String {
    format!("{KEEP}\n\n{DROP}\n")
}
fn drop_range() -> (usize, usize) {
    let start = KEEP.lines().count() + 2;
    (start, start + DROP.lines().count() - 1)
}
fn judge() -> MockEvaluator {
    MockEvaluator::new(|request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|q| {
                let name = q.instructions()["candidate"]["name"].as_str().unwrap();
                (
                    q.id().clone(),
                    answers::noul(if name == "keepBody" { 0.01 } else { 0.99 }),
                )
            })
            .collect())
    })
}
fn arguments(tool: &str, view: &str, intent: bool) -> Value {
    let mut args = if tool == "read" {
        json!({"file_path":"src/budget.ts", "view":view, "include_events":false})
    } else {
        json!({"path":"src/budget.ts", "pattern":"function", "expand":"callable", "view":view, "include_events":false})
    };
    if intent {
        args["task_query"] = json!(TASK);
    }
    args
}
async fn call(client: &mut InProcessClient, tool: &str, args: Value) -> Value {
    let response = client
        .call("tools/call", json!({"name":tool,"arguments":args}))
        .await
        .unwrap();
    assert!(response["error"].is_null(), "{response}");
    response
}
async fn warm(client: &mut InProcessClient) {
    let response = client
        .call_tool_until("overview", json!({"path":"src/budget.ts"}), |text| {
            text.contains("keepBody")
        })
        .await
        .unwrap();
    assert!(response_text(&response).contains("keepBody"), "{response}");
}
fn observations(root: &Path, response: &Value) -> Vec<(String, u64)> {
    let path = root.join(".codemap/analysis.sqlite3");
    assert!(path.exists(), "real usage recording must be enabled");
    let connection = rusqlite::Connection::open(path).unwrap();
    let (id, response_bytes, is_error): (i64, i64, bool) = connection
        .query_row(
            "SELECT id, response_bytes, is_error FROM calls ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        response_bytes,
        response_text(response).len() as i64,
        "final response bytes include markers, after masking and caps"
    );
    assert!(!is_error);
    let mut statement = connection
        .prepare("SELECT path,result_bytes FROM file_observations WHERE call_id=?1 ORDER BY path")
        .unwrap();
    statement
        .query_map([id], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}
fn source_block<'a>(text: &'a str, view: &str) -> &'a str {
    if view == "source" {
        text
    } else {
        text.split_once("\n### results\n").unwrap().1
    }
}
fn row_number(row: &str) -> Option<usize> {
    if let Some((number, _)) = row.split_once('→') {
        return number.trim().parse().ok();
    }
    let row = row
        .strip_prefix("src/budget.ts")
        .map(|s| &s[1..])
        .unwrap_or(row);
    row.split([':', '-']).next()?.parse().ok()
}

#[tokio::test]
async fn read_and_grep_omit_only_unrelated_bodies_and_record_delivered_bytes() {
    for tool in ["read", "grep"] {
        for view in ["source", "full"] {
            let source = source();
            let temp =
                create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)])
                    .unwrap();
            let root = temp.path().to_path_buf();
            let evaluator = Arc::new(judge());
            let recorded = evaluator.clone();
            with_in_process_server_logging(temp.path(), Some(evaluator), true, |mut client| async move {
                warm(&mut client).await;
                let plain = call(&mut client, tool, arguments(tool, view, false)).await;
                let before = observations(&root, &plain);
                assert_eq!(before.len(), 1);
                let filtered = call(&mut client, tool, arguments(tool, view, true)).await;
                let text = response_text(&filtered);
                assert!(text.contains("_omitted body:") && text.contains("fn dropBody"), "{tool}/{view}: {text}");
                assert!(text.contains("const reserve = footer.length + 8;"));
                assert!(!text.contains("UNRELATED_MARKER"));
                assert!(text.len() < response_text(&plain).len());
                let (start, end) = drop_range();
                let original = source_block(response_text(&plain), view);
                let removed_bytes: usize = original.split_inclusive('\n').filter(|row| row_number(row).is_some_and(|line| start <= line && line <= end)).map(str::len).sum();
                assert!(removed_bytes > 0);
                let after = observations(&root, &filtered);
                assert_eq!(after, vec![("src/budget.ts".into(), before[0].1 - removed_bytes as u64)], "omission notes are not delivered source");
                for row in original.lines().filter(|row| row_number(row).is_some_and(|line| line < start || line > end)) {
                    assert!(text.lines().any(|returned| returned == row), "retained file/line/content lost: {row}");
                }
                assert_eq!(recorded.request_count(), 1);
                let requests = recorded.requests();
                assert_eq!(requests[0].state["tool"], tool);
                assert!(requests[0].state.get("search_arguments").is_none());
                assert_eq!(requests[0].task_query(), Some(TASK));
                assert!(requests[0].questions.values().all(|q| q["type"] == "noul"));
                // The marker's range restores original lines without inference, even though
                // the read filter remains enabled in configuration.
                assert!(text.contains(&format!("offset {start} limit {}", end - start + 1)));
                let restored = call(&mut client, "read", json!({"file_path":"src/budget.ts","offset":start,"limit":end-start+1,"view":"source"})).await;
                for (i, line) in DROP.lines().enumerate() {
                    assert!(response_text(&restored).contains(&format!("{:>6}→{line}", start+i)));
                }
                assert!(!response_text(&restored).contains("_omitted body:"));
                assert_eq!(recorded.request_count(), 1);
                assert_eq!(observations(&root, &restored)[0].1, response_text(&restored).len() as u64);
            }).await;
        }
    }
}

#[tokio::test]
async fn marker_only_files_are_not_recorded_as_read_source() {
    for tool in ["read", "grep"] {
        let source = source();
        let temp =
            create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)])
                .unwrap();
        let root = temp.path().to_path_buf();
        let evaluator = Arc::new(judge());
        let recorded = evaluator.clone();
        with_in_process_server_logging(
            temp.path(),
            Some(evaluator),
            true,
            |mut client| async move {
                let (start, end) = drop_range();
                let mut args = arguments(tool, "source", true);
                if tool == "read" {
                    args["offset"] = json!(start);
                    args["limit"] = json!(end - start + 1);
                } else {
                    args["pattern"] = json!("function dropBody");
                }
                let response = call(&mut client, tool, args).await;
                assert!(
                    response_text(&response).contains("_omitted body:"),
                    "{response}"
                );
                assert!(!response_text(&response).contains("UNRELATED_MARKER"));
                assert!(
                    observations(&root, &response).is_empty(),
                    "markers do not count as source"
                );
                assert_eq!(recorded.request_count(), 1);
            },
        )
        .await;
    }
}

#[tokio::test]
async fn partial_windows_rows_and_metadata_modes_never_send_body_questions() {
    let source = source();
    let temp =
        create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)]).unwrap();
    let evaluator = Arc::new(judge());
    let recorded = evaluator.clone();
    with_in_process_server_logging(temp.path(), Some(evaluator), false, |mut client| async move {
        warm(&mut client).await;
        let (start, _) = drop_range();
        for (tool, args) in [
            ("read", json!({"file_path":"src/budget.ts","offset":start,"limit":3,"view":"source"})),
            ("grep", json!({"path":"src/budget.ts","pattern":"function","expand":"none","view":"source"})),
            ("read", json!({"file_path":"src/budget.ts","view":"definitions","include_events":false})),
            ("grep", json!({"path":"src/budget.ts","pattern":"function","view":"relations","include_events":false})),
            ("grep", json!({"path":"src/budget.ts","pattern":"function","output_mode":"count"})),
            ("grep", json!({"path":"src/budget.ts","pattern":"function","output_mode":"files_with_matches"})),
        ] {
            let plain = call(&mut client, tool, args.clone()).await;
            let mut filtered = args; filtered["task_query"] = json!(TASK);
            let response = call(&mut client, tool, filtered).await;
            assert_eq!(response_text(&response), response_text(&plain), "{tool}");
        }
        assert_eq!(recorded.request_count(), 0);
    }).await;
}

#[tokio::test]
async fn complete_bodies_in_unexpanded_grep_context_can_be_omitted() {
    let source = source();
    let temp =
        create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)]).unwrap();
    let root = temp.path().to_path_buf();
    let evaluator = Arc::new(judge());
    let recorded = evaluator.clone();
    with_in_process_server_logging(temp.path(), Some(evaluator), true, |mut client| async move {
        let response = call(&mut client, "grep", json!({"path":"src/budget.ts","pattern":"function dropBody","expand":"none","-A":7,"view":"source","task_query":TASK})).await;
        assert!(response_text(&response).contains("_omitted body:"), "{response}");
        assert!(!response_text(&response).contains("UNRELATED_MARKER"));
        assert!(observations(&root, &response).is_empty());
        assert_eq!(recorded.request_count(), 1);
    }).await;
}

#[tokio::test]
async fn an_omitted_grep_page_keeps_its_footer_and_does_not_refill_from_the_next_page() {
    let source = format!("{DROP}\n\n{KEEP}\n");
    let temp =
        create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)]).unwrap();
    let root = temp.path().to_path_buf();
    let evaluator = Arc::new(judge());
    let recorded = evaluator.clone();
    with_in_process_server_logging(
        temp.path(),
        Some(evaluator),
        true,
        |mut client| async move {
            let args =
                json!({"path":"src/budget.ts","pattern":"function","head_limit":1,"view":"source"});
            let plain = call(&mut client, "grep", args.clone()).await;
            let footer = response_text(&plain)
                .lines()
                .find(|line| line.contains("next_offset=1"))
                .unwrap()
                .to_string();
            let mut filtered = args;
            filtered["task_query"] = json!(TASK);
            let response = call(&mut client, "grep", filtered).await;
            let text = response_text(&response);
            assert!(
                text.contains("_omitted body:") && text.contains(&footer),
                "{text}"
            );
            assert!(
                !text.contains("keepBody") && !text.contains("const reserve"),
                "the next page must not refill freed space"
            );
            assert!(observations(&root, &response).is_empty());
            assert_eq!(recorded.request_count(), 1);
            assert_eq!(recorded.requests()[0].questions.len(), 1);
        },
    )
    .await;
}

#[tokio::test]
async fn retained_callers_and_nested_declarations_protect_their_bodies() {
    let linked = format!(
        "{}\n\n{}\n\n{}\n",
        KEEP.replace("return allowed;", "return helperBudget(allowed);"),
        DROP.replace(
            "dropBody(input: string): string",
            "helperBudget(input: number): string"
        )
        .replace("input.trim()", "String(input).trim()"),
        DROP
    );
    let temp =
        create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &linked)]).unwrap();
    let evaluator = Arc::new(judge());
    with_in_process_server_logging(
        temp.path(),
        Some(evaluator),
        false,
        |mut client| async move {
            for tool in ["read", "grep"] {
                let response = call(&mut client, tool, arguments(tool, "source", true)).await;
                let text = response_text(&response);
                assert!(
                    text.contains("function helperBudget"),
                    "linked helper must stay: {text}"
                );
                assert!(text.contains("(fn dropBody) judged unrelated"), "{text}");
                assert!(!text.contains("(fn helperBudget) judged unrelated"));
            }
        },
    )
    .await;
    // A retained nested callable must not disappear inside an unrelated outer one.
    let nested = format!(
        "export function outerBody() {{\n{}\n{}\n  return keepBody(10, '', 20);\n}}",
        KEEP, DROP
    );
    let temp =
        create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &nested)]).unwrap();
    let evaluator = Arc::new(judge());
    with_in_process_server_logging(
        temp.path(),
        Some(evaluator),
        false,
        |mut client| async move {
            let plain = call(&mut client, "read", arguments("read", "source", false)).await;
            let filtered = call(&mut client, "read", arguments("read", "source", true)).await;
            assert_eq!(
                response_text(&filtered),
                response_text(&plain),
                "retained nested source protects its enclosing body"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn all_keep_and_provider_failure_preserve_text_and_source_observations() {
    for should_fail in [false, true] {
        let evaluator = if should_fail {
            MockEvaluator::failing(JevError::RateLimited { status: 429 })
        } else {
            MockEvaluator::new(|request: &EvaluationRequest| {
                Ok(request
                    .questions()
                    .iter()
                    .map(|q| (q.id().clone(), answers::noul(0.0)))
                    .collect())
            })
        };
        let evaluator = Arc::new(evaluator);
        let recorded = evaluator.clone();
        let source = source();
        let temp =
            create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)])
                .unwrap();
        let root = temp.path().to_path_buf();
        with_in_process_server_logging(
            temp.path(),
            Some(evaluator),
            true,
            |mut client| async move {
                warm(&mut client).await;
                for tool in ["read", "grep"] {
                    for view in ["source", "full"] {
                        let plain = call(&mut client, tool, arguments(tool, view, false)).await;
                        let before = observations(&root, &plain);
                        let filtered = call(&mut client, tool, arguments(tool, view, true)).await;
                        assert_eq!(response_text(&filtered), response_text(&plain));
                        assert_eq!(observations(&root, &filtered), before);
                    }
                }
                assert_eq!(recorded.request_count(), 4);
            },
        )
        .await;
    }
}

#[tokio::test]
async fn captures_masked_payload_and_keeps_the_snapshot_during_inference() {
    for tool in ["read", "grep"] {
        let secret = format!("AKIA{}", "IOSFODNN7EXAMPLE");
        let source = source().replace(
            "const trimmed = input.trim();",
            &format!("const api_key = '{secret}';\n  const trimmed = input.trim();"),
        );
        let temp =
            create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)])
                .unwrap();
        let path = temp.path().join("src/budget.ts");
        let gate = Gate::new();
        let release = gate.clone();
        let evaluator = Arc::new(judge().with_gate(gate));
        let recorded = evaluator.clone();
        with_in_process_server_logging(
            temp.path(),
            Some(evaluator),
            false,
            |mut client| async move {
                let mut args = arguments(tool, "source", true);
                args["task_query"] = json!(format!("{TASK} {secret}"));
                client
                    .send("tools/call", json!({"name":tool,"arguments":args}))
                    .await
                    .unwrap();
                release.entered().await;
                let requests = recorded.requests();
                let wire = serde_json::to_string(
                    &json!({"state":requests[0].state,"questions":requests[0].questions.values().collect::<Vec<_>>()}),
                )
                .unwrap();
                assert!(!wire.contains(&secret), "outbound payload must be masked");
                assert!(wire.contains("[REDACTED]"));
                assert!(wire.contains("candidate.body") && wire.contains("tool_arguments"));
                std::fs::write(
                    &path,
                    "export function afterCapture() { return 'NEW_SOURCE'; }\n",
                )
                .unwrap();
                release.release();
                let response = client.receive().await.unwrap();
                let text = response_text(&response);
                assert!(
                    text.contains("const reserve"),
                    "old kept source must remain: {text}"
                );
                assert!(text.contains("_omitted body:"));
                assert!(!text.contains("NEW_SOURCE"));
                let restored = call(
                    &mut client,
                    "read",
                    json!({"file_path":"src/budget.ts","view":"source"}),
                )
                .await;
                assert!(response_text(&restored).contains("NEW_SOURCE"));
                assert_eq!(recorded.request_count(), 1);
            },
        )
        .await;
    }
}

#[tokio::test]
async fn tool_flags_schema_reload_and_threshold_reach_live_consumers() {
    let source = source();
    let config = "[analysis.jev]\nread_filter_enabled=true\ngrep_filter_enabled=false\nsearch_filter_min_unrelated_probability=1.0\n";
    let temp =
        create_mock_repo(&[(".codemap/config.toml", config), ("src/budget.ts", &source)]).unwrap();
    let root = temp.path().to_path_buf();
    let evaluator = Arc::new(judge());
    let recorded = evaluator.clone();
    with_in_process_server_logging(
        temp.path(),
        Some(evaluator),
        false,
        |mut client| async move {
            let listed = client.call("tools/list", json!({})).await.unwrap();
            let tools = listed["result"]["tools"].as_array().unwrap();
            for (tool, enabled) in [("read", true), ("grep", false), ("search", false)] {
                let entry = tools.iter().find(|entry| entry["name"] == tool).unwrap();
                assert_eq!(entry["annotations"]["openWorldHint"], enabled);
                assert_eq!(
                    entry["inputSchema"]["properties"]["task_query"]["type"],
                    "string"
                );
            }
            let plain = call(&mut client, "read", arguments("read", "source", false)).await;
            let kept = call(&mut client, "read", arguments("read", "source", true)).await;
            assert_eq!(
                response_text(&kept),
                response_text(&plain),
                "1.0 retains a 0.99 judgment"
            );
            let unfiltered = call(&mut client, "grep", arguments("grep", "source", true)).await;
            assert!(response_text(&unfiltered).contains("UNRELATED_MARKER"));
            assert_eq!(recorded.request_count(), 1);
            std::fs::write(root.join(".codemap/config.toml"), CONFIG).unwrap();
            codemap_search::config::reload(&root);
            for tool in ["read", "grep"] {
                let response = call(&mut client, tool, arguments(tool, "source", true)).await;
                assert!(
                    response_text(&response).contains("_omitted body:"),
                    "0.70 applies after reload: {response}"
                );
            }
            assert_eq!(recorded.request_count(), 3);
            let invalid = client
                .call(
                    "tools/call",
                    json!({"name":"read","arguments":{"file_path":"src/budget.ts","task_query":7}}),
                )
                .await
                .unwrap();
            assert_eq!(invalid["error"]["code"], -32602);
        },
    )
    .await;
}
