//! An isolated child process keeps its cwd and config globals away from other e2e cases.
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use codemap_search::jev::{
    Answer, Cancellation, Evaluation, Evaluator, JevError, Policy, Question, Request, Usage,
};
use codemap_search::{config, index, mcp};
use serde_json::{json, Value};

use super::helpers::{create_mock_repo, McpClient};

struct FakeEvaluator;

impl Evaluator for FakeEvaluator {
    fn evaluate<'a>(
        &'a self,
        request: Request,
        _policy: Policy,
        _cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Evaluation, JevError>> + Send + 'a>> {
        Box::pin(async move {
            if request.state["task_query"] == "force_failure" {
                return Err(JevError::InvalidResponse);
            }
            let answers = request
                .questions
                .into_iter()
                .map(|(id, question)| {
                    let answer = match question {
                        Question::Score { .. } => Answer::Score {
                            score: 3.0,
                            confidence: 1.0,
                            probabilities: BTreeMap::from([
                                ("0".into(), 0.0),
                                ("1".into(), 0.0),
                                ("2".into(), 0.0),
                                ("3".into(), 1.0),
                            ]),
                        },
                        Question::Choice { criteria, .. } => Answer::Choice {
                            choice: "implementation".into(),
                            confidence: 1.0,
                            probabilities: criteria
                                .keys()
                                .map(|key| {
                                    (key.clone(), if key == "implementation" { 1.0 } else { 0.0 })
                                })
                                .collect(),
                        },
                        Question::Noul { .. } => Answer::Noul { probability: 0.8 },
                    };
                    (id, answer)
                })
                .collect();
            Ok(Evaluation {
                model: codemap_search::jev::MODEL.into(),
                answers,
                usage: Usage {
                    input_tokens: 12,
                    output_tokens: 2,
                },
                elapsed: Duration::ZERO,
                batches: Vec::new(),
            })
        })
    }
}

#[tokio::test]
async fn enabled_modes_without_credentials_keep_base_results() {
    let temp = create_mock_repo(&[
        (
            ".codemap/config.toml",
            "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\n",
        ),
        (
            "src/route.rs",
            "pub fn requested_route() { let visible = true; }\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let overview = client
        .send_request(
            "tools/call",
            json!({
                "name":"overview","arguments":{"task_query":"Find route"}
            }),
        )
        .await
        .unwrap();
    assert_eq!(overview["result"]["_meta"]["jev"]["outcome"], "bypassed");
    assert_eq!(
        overview["result"]["_meta"]["jev"]["reason"],
        "missing_api_key"
    );
    let search = client
        .send_request(
            "tools/call",
            json!({
                "name":"search","arguments":{"query":"requested_route","task_query":"Find route"}
            }),
        )
        .await
        .unwrap();
    assert_eq!(search["result"]["_meta"]["jev"]["outcome"], "bypassed");
    assert_eq!(
        search["result"]["_meta"]["jev"]["reason"],
        "missing_api_key"
    );
    assert!(search["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("visible = true"));
    for name in ["overview", "search"] {
        let arguments = if name == "search" {
            json!({"query":"requested_route","task_query":17})
        } else {
            json!({"task_query":17})
        };
        let invalid = client
            .send_request("tools/call", json!({"name":name,"arguments":arguments}))
            .await
            .unwrap();
        assert_eq!(invalid["error"]["code"], -32602);
    }
}

#[test]
fn native_jev_applies_to_both_tools_without_provider_traffic() {
    let temp = create_mock_repo(&[
        (
            ".codemap/config.toml",
            "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\n",
        ),
        (
            "src/route.rs",
            "pub fn requested_route() -> &'static str {\n    \"ok\"\n}\npub fn route_secondary() {\n    let third = 3;\n}\n",
        ),
        (
            "src/helper.rs",
            "pub fn route_helper() {\n    let second = false;\n}\n",
        ),
    ])
    .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("e2e::jev::injected_pipeline_harness")
        .arg("--nocapture")
        .current_dir(temp.path())
        .env("JEV_TEST_REPO", temp.path())
        .env("CODEMAP_HOME", temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("JEV_PIPELINE_OK"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn injected_pipeline_harness() {
    let Ok(repo) = std::env::var("JEV_TEST_REPO") else {
        return;
    };
    config::init(std::path::Path::new(&repo));
    let engine = index::TantivySearchEngine::new(&config::get().index_path).unwrap();
    let searcher = engine.searcher_handle();
    let indexer = index::spawn_indexer(engine);
    let supervisor = index::EngineSupervisor::new(
        searcher,
        None,
        None,
        indexer,
        Arc::new(index::WatcherStatus::default()),
    );
    let mut server = mcp::McpServer::with_jev_evaluator(supervisor, Arc::new(FakeEvaluator));
    let overview_call =
        json!({"name":"overview","arguments":{"task_query":"Find requested route"}});
    let mut overview = Value::Null;
    for _ in 0..100 {
        overview = server
            .handle_request("tools/call", Some(&overview_call))
            .await
            .unwrap();
        if overview["_meta"]["jev"]["outcome"] == "applied" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(overview["_meta"]["jev"]["outcome"], "applied", "{overview}");
    assert_eq!(overview["_meta"]["jev"]["recommendation_status"], "matched");
    assert!(overview["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("src/route.rs"));

    let search = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"search","arguments":{
                    "query":"route","task_query":"Find requested route"
                }
            })),
        )
        .await
        .unwrap();
    assert_eq!(search["_meta"]["jev"]["outcome"], "applied", "{search}");
    assert!(search["_meta"]["jev"]["evaluated_bodies"].as_u64().unwrap() > 0);
    assert!(search["_meta"]["jev"]["omitted_bodies"].as_u64().unwrap() > 0);
    let filtered = search["content"][0]["text"].as_str().unwrap();
    assert!(filtered.contains("src/route.rs"));
    assert!(filtered.contains("src/helper.rs"));
    assert!(filtered.contains("requested_route"));
    assert!(!filtered.contains("\"ok\""), "{filtered}");
    assert!(!filtered.contains("third = 3"), "{filtered}");
    assert!(!filtered.contains("second = false"), "{filtered}");
    assert_eq!(
        filtered
            .lines()
            .filter(|line| {
                line.starts_with("## ")
                    && (line.contains("src/route.rs") || line.contains("src/helper.rs"))
            })
            .count(),
        2,
        "{filtered}"
    );
    assert_eq!(filtered.matches("```").count() % 2, 0, "{filtered}");
    let reads = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"analyze","arguments":{
                    "target":"reads","tool":"search","view":"summary","compare":false
                }
            })),
        )
        .await
        .unwrap();
    let reads: Value = serde_json::from_str(reads["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(reads["summary"]["files"], 0, "{reads}");

    let failed_search = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"search","arguments":{
                    "query":"route","task_query":"force_failure"
                }
            })),
        )
        .await
        .unwrap();
    assert_eq!(failed_search["_meta"]["jev"]["outcome"], "fallback");
    assert_eq!(failed_search["_meta"]["jev"]["input_tokens"], Value::Null);
    assert!(failed_search["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("third = 3"));
    let failed_overview = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"overview","arguments":{"task_query":"force_failure"}
            })),
        )
        .await
        .unwrap();
    assert_eq!(failed_overview["_meta"]["jev"]["outcome"], "fallback");
    assert!(failed_overview["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("src/route.rs"));

    std::fs::write(
        std::path::Path::new(&repo).join(".codemap/config.toml"),
        "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.90\n",
    ).unwrap();
    config::reload(std::path::Path::new(&repo));
    let retained = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"search","arguments":{
                    "query":"route","task_query":"Find requested route"
                }
            })),
        )
        .await
        .unwrap();
    assert_eq!(retained["_meta"]["jev"]["outcome"], "applied");
    assert_eq!(retained["_meta"]["jev"]["threshold"], 0.9);
    assert_eq!(retained["_meta"]["jev"]["omitted_bodies"], 0);
    let retained_text = retained["content"][0]["text"].as_str().unwrap();
    let route_heading = retained_text.find("src/route.rs").unwrap();
    let helper_heading = retained_text.find("src/helper.rs").unwrap();
    let route_body = retained_text.find("\"ok\"").unwrap();
    let secondary_body = retained_text.find("third = 3").unwrap();
    let helper_body = retained_text.find("second = false").unwrap();
    if route_heading < helper_heading {
        assert!(route_heading < route_body && route_body < helper_heading);
        assert!(route_heading < secondary_body && secondary_body < helper_heading);
        assert!(helper_heading < helper_body);
    } else {
        assert!(helper_heading < helper_body && helper_body < route_heading);
        assert!(route_heading < route_body);
        assert!(route_heading < secondary_body);
    }
    let reads_after_keep = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"analyze","arguments":{
                    "target":"reads","tool":"search","view":"summary","compare":false
                }
            })),
        )
        .await
        .unwrap();
    let reads_after_keep: Value =
        serde_json::from_str(reads_after_keep["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        reads_after_keep["summary"]["files"], 2,
        "{reads_after_keep}"
    );

    std::fs::write(
        std::path::Path::new(&repo).join(".codemap/config.toml"),
        "[analysis.jev]\noverview_enabled = false\nsearch_filter_enabled = true\n",
    )
    .unwrap();
    config::reload(std::path::Path::new(&repo));
    let disabled_overview = server
        .handle_request("tools/call", Some(&overview_call))
        .await
        .unwrap();
    assert!(disabled_overview.get("_meta").is_none());
    let active_search = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"search","arguments":{
                    "query":"route","task_query":"Find requested route"
                }
            })),
        )
        .await
        .unwrap();
    assert_eq!(active_search["_meta"]["jev"]["outcome"], "applied");

    std::fs::write(
        std::path::Path::new(&repo).join(".codemap/config.toml"),
        "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = false\n",
    )
    .unwrap();
    config::reload(std::path::Path::new(&repo));
    let active_overview = server
        .handle_request("tools/call", Some(&overview_call))
        .await
        .unwrap();
    assert_eq!(active_overview["_meta"]["jev"]["outcome"], "applied");
    let disabled_search = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"search","arguments":{
                    "query":"route","task_query":"Find requested route"
                }
            })),
        )
        .await
        .unwrap();
    assert!(disabled_search.get("_meta").is_none());

    let search_base = server
        .handle_request(
            "tools/call",
            Some(&json!({
                "name":"search","arguments":{"query":"route"}
            })),
        )
        .await
        .unwrap();
    assert!(search_base.get("_meta").is_none());
    let base_text = search_base["content"][0]["text"].as_str().unwrap();
    assert!(base_text.contains("requested_route"));
    assert!(base_text.contains("\"ok\""));
    println!("JEV_PIPELINE_OK");
}
