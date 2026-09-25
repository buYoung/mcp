//! Actual MCP regressions for task-based read/grep filtering;
//! SQLite assertions inspect the real final response accounting, not a copied policy.
use super::helpers::{
    create_mock_repo, response_text, with_in_process_server_logging as run_server, InProcessClient,
};
use codemap_search::jev::mock::{answers, MockEvaluator};
use codemap_search::jev::{EvaluationRequest, JevError};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

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
            client
                .register_task("Find functions implementing the response output budget")
                .await;
            script(client).await;
        },
    )
    .await;
}

fn arguments(tool: &str, view: &str) -> Value {
    if tool == "read" {
        json!({"file_path":"src/budget.ts", "view":view, "include_events":false})
    } else {
        json!({"path":"src/budget.ts", "pattern":"function", "expand":"callable", "view":view, "include_events":false})
    }
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
