//! Actual MCP regressions for task-based read/grep filtering;
//! SQLite assertions inspect the real final response accounting, not a copied policy.
use super::helpers::{
    create_mock_repo, response_text, with_in_process_server_logging as run_server, InProcessClient,
};
use codemap_search::jev::mock::{answers, Gate, MockEvaluator};
use codemap_search::jev::{EvaluationRequest, JevError};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

const TASK: &str = "Find functions implementing the response output budget";
const CONFIG: &str = "[output.jev]\nenabled = true\nscope = ['read', 'grep']\nsearch_filter_min_unrelated_probability = 0.70\n[index.refresh]\nwatch = false\n";
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
                    answers::noul(if name == "keepBody" { 0.99 } else { 0.01 }),
                )
            })
            .collect())
    })
}
async fn with_in_process_server_logging<F, Fut>(
    root: &Path,
    evaluator: Option<Arc<dyn codemap_search::jev::Evaluator>>,
    should_record_calls: bool,
    script: F,
) where
    F: FnOnce(InProcessClient) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    run_server(
        root,
        evaluator,
        should_record_calls,
        |mut client| async move {
            client.register_task(TASK).await;
            script(client).await;
        },
    )
    .await;
}

fn arguments(tool: &str, view: &str) -> Value {
    // These assertions compare Jev decisions, including source delivered by earlier calls.
    if tool == "read" {
        json!({"file_path":"src/budget.ts", "view":view, "include_events":false, "include_seen":true})
    } else {
        json!({"path":"src/budget.ts", "pattern":"function", "expand":"callable", "view":view, "include_events":false, "include_seen":true})
    }
}
async fn call(client: &mut InProcessClient, tool: &str, args: Value) -> Value {
    let response = client
        .call("tools/call", json!({"name":tool,"arguments":args}))
        .await
        .unwrap();
    assert!(
        response["error"].is_null() && response["result"]["isError"] != true,
        "{response}"
    );
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
#[tokio::test]
async fn read_and_grep_omit_unrelated_bodies_and_record_delivered_bytes() {
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
                let plain = client.plain_call(tool, arguments(tool, view)).await.unwrap();
                let before = observations(&root, &plain);
                assert_eq!(before.len(), 1);
                let filtered = call(&mut client, tool, arguments(tool, view)).await;
                let text = response_text(&filtered);
                assert!(text.contains("_omitted body:"));
                assert!(!text.contains("UNRELATED_MARKER"));
                assert!(text.contains("const reserve = footer.length + 8;"));
                let after = observations(&root, &filtered);
                assert_eq!(after.len(), 1);
                assert!(after[0].1 < before[0].1);
                let (start, end) = drop_range();
                let plain_text = response_text(&plain);
                let removed_bytes: usize = plain_text.split_inclusive('\n').filter(|row| {
                    let row = row.trim_start();
                    let number = if tool == "read" {
                        row.split_once('→').map(|(number, _)| number)
                    } else {
                        let row = row.strip_prefix("src/budget.ts:").or_else(|| row.strip_prefix("src/budget.ts-")).unwrap_or(row);
                        row.split_once([':', '-']).map(|(number, _)| number)
                    };
                    number.and_then(|number| number.parse::<usize>().ok())
                        .is_some_and(|line| start <= line && line <= end)
                }).map(str::len).sum();
                assert_eq!(after[0].1, before[0].1 - removed_bytes as u64);
                assert!(recorded.request_count() > 0);
                let requests_before_restore = recorded.request_count();
                let (start, end) = drop_range();
                let restored = client.plain_call("read", json!({"file_path":"src/budget.ts","offset":start,"limit":end-start+1,"view":"source","include_seen":true})).await.unwrap();
                for (i, line) in DROP.lines().enumerate() {
                    assert!(response_text(&restored).contains(&format!("{:>6}→{line}", start+i)));
                }
                assert!(!response_text(&restored).contains("_omitted body:"));
                assert_eq!(recorded.request_count(), requests_before_restore);
                assert_eq!(observations(&root, &restored)[0].1, response_text(&restored).len() as u64);
            }).await;
        }
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
            let plain = client.plain_call(tool, args.clone()).await.unwrap();
            let response = call(&mut client, tool, args).await;
            assert_eq!(response_text(&response), response_text(&plain), "{tool}");
        }
        assert_eq!(recorded.request_count(), 0);
    }).await;
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
                    .map(|q| (q.id().clone(), answers::noul(1.0)))
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
                        let plain = client
                            .plain_call(tool, arguments(tool, view))
                            .await
                            .unwrap();
                        let before = observations(&root, &plain);
                        let filtered = call(&mut client, tool, arguments(tool, view)).await;
                        assert_eq!(response_text(&filtered), response_text(&plain));
                        assert_eq!(observations(&root, &filtered), before);
                    }
                }
                assert!(recorded.request_count() > 0);
            },
        )
        .await;
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
                let mut args = arguments(tool, "source");
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
async fn complete_bodies_in_unexpanded_grep_context_can_be_omitted() {
    let source = source();
    let temp =
        create_mock_repo(&[(".codemap/config.toml", CONFIG), ("src/budget.ts", &source)]).unwrap();
    let root = temp.path().to_path_buf();
    let evaluator = Arc::new(judge());
    let recorded = evaluator.clone();
    with_in_process_server_logging(temp.path(), Some(evaluator), true, |mut client| async move {
        let response = call(&mut client, "grep", json!({"path":"src/budget.ts","pattern":"function dropBody","expand":"none","-A":7,"view":"source"})).await;
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
            let plain = client.plain_call("grep", args.clone()).await.unwrap();
            let footer = response_text(&plain)
                .lines()
                .find(|line| line.contains("next_offset=1"))
                .unwrap()
                .to_string();
            let response = call(&mut client, "grep", args).await;
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
            assert_eq!(recorded.requests()[0].questions.len(), 2);
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
                let response = call(&mut client, tool, arguments(tool, "source")).await;
                let text = response_text(&response);
                assert!(
                    text.contains("function helperBudget"),
                    "linked helper must stay: {text}"
                );
                assert!(
                    text.contains("(fn dropBody) Jev found no task match"),
                    "{text}"
                );
                assert!(!text.contains("(fn helperBudget) Jev found no task match"));
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
            let plain = client
                .plain_call("read", arguments("read", "source"))
                .await
                .unwrap();
            let filtered = call(&mut client, "read", arguments("read", "source")).await;
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
                client.register_task(&format!("{TASK} {secret}")).await;
                let args = arguments(tool, "source");
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
                assert!(!requests[0].state["candidates"].as_object().unwrap().is_empty());
                assert!(requests[0].state.get("search_arguments").is_some());
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
                let restored = client.plain_call("read", json!({"file_path":"src/budget.ts","view":"source"})).await.unwrap();
                assert!(response_text(&restored).contains("NEW_SOURCE"));
                assert_eq!(recorded.request_count(), 1);
            },
        )
        .await;
    }
}

#[tokio::test]
async fn enabled_scope_schema_reload_and_threshold_reach_live_consumers() {
    let source = source();
    let config =
        "[output.jev]\nenabled=true\nscope=['read']\nsearch_filter_min_unrelated_probability=1.0\n";
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
                assert!(entry["inputSchema"]["properties"]
                    .get("task_query")
                    .is_none());
            }
            let plain = client
                .plain_call("read", arguments("read", "source"))
                .await
                .unwrap();
            let kept = call(&mut client, "read", arguments("read", "source")).await;
            assert_eq!(
                response_text(&kept),
                response_text(&plain),
                "threshold 1.0 retains nonzero relevance"
            );
            let unfiltered = call(&mut client, "grep", arguments("grep", "source")).await;
            assert!(response_text(&unfiltered).contains("UNRELATED_MARKER"));
            assert_eq!(recorded.request_count(), 1);
            std::fs::write(root.join(".codemap/config.toml"), CONFIG).unwrap();
            codemap_search::config::reload(&root);
            for tool in ["read", "grep"] {
                let response = call(&mut client, tool, arguments(tool, "source")).await;
                assert!(
                    response_text(&response).contains("_omitted body:"),
                    "0.70 applies after reload: {response}"
                );
            }
            assert_eq!(recorded.request_count(), 3);
            client.register_task("inspect the updated task").await;
            call(&mut client, "read", arguments("read", "source")).await;
            assert_eq!(
                recorded.requests().last().unwrap().task_query(),
                Some("inspect the updated task")
            );
            let missing = client
                .call(
                    "tools/call",
                    json!({"name":"initial_instructions","arguments":{"task_query":" "}}),
                )
                .await
                .unwrap();
            assert_eq!(missing["result"]["isError"], true, "{missing}");
            let missing = client
                .call(
                    "tools/call",
                    json!({"name":"read","arguments":arguments("read","source")}),
                )
                .await
                .unwrap();
            assert_eq!(
                missing["result"]["isError"], true,
                "a failed new registration clears the old task"
            );
            assert_eq!(
                recorded.request_count(),
                4,
                "missing context cannot evaluate or silently bypass"
            );
            let invalid = client
                .call(
                    "tools/call",
                    json!({"name":"read","arguments":{"file_path":"src/budget.ts","task_query":7}}),
                )
                .await
                .unwrap();
            assert_eq!(invalid["result"]["isError"], true, "{invalid}");
        },
    )
    .await;
}
