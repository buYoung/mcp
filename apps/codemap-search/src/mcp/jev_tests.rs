use super::*;
use crate::jev::{Answer, Evaluation, EvaluationRequest, Evaluator, Failure, Question, Usage};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::{Arc, atomic::{AtomicBool,AtomicUsize,Ordering}}};

struct Fake { calls: AtomicUsize, should_fail: AtomicBool, is_no_match: AtomicBool }
impl Evaluator for Fake {
    fn evaluate<'a>(&'a self, request: EvaluationRequest) -> Pin<Box<dyn Future<Output=Result<Evaluation,Failure>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1,Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            if self.should_fail.load(Ordering::SeqCst) {
                return Err(Failure {kind:crate::jev::FailureKind::InvalidResponse,
                    completed_batches:0,usage:Usage::default(),elapsed_ms:20,http_elapsed_ms:0});
            }
            let answers = request.questions.into_iter().map(|(id, question)| {
                let answer = match question {
                    Question::Score { .. } => Answer::Score {score:if self.is_no_match.load(Ordering::SeqCst) {0.0} else {3.0},legend:BTreeMap::new(),
                        probabilities:if self.is_no_match.load(Ordering::SeqCst) {
                            BTreeMap::from([("0".into(),1.0),("1".into(),0.0),("2".into(),0.0),("3".into(),0.0)])
                        } else {BTreeMap::from([("0".into(),0.0),("1".into(),0.0),("2".into(),0.0),("3".into(),1.0)])},confidence:1.0},
                    Question::Choice { .. } => Answer::Choice {choice:"implementation".into(),
                        probabilities:BTreeMap::from([("implementation".into(),1.0),("unrelated".into(),0.0)]),confidence:1.0},
                    Question::Noul { .. } => Answer::Noul {noul:0.8},
                };
                (id,answer)
            }).collect();
            Ok(Evaluation {model:crate::jev::MODEL.into(),answers,usage:Usage {input_tokens:11,output_tokens:2},request_ids:vec!["fake".into()],http_elapsed_ms:2,elapsed_ms:20})
        })
    }
}

fn text(value:&Value) -> &str { value["content"][0]["text"].as_str().unwrap_or("") }

async fn exercise_mcp_with_injected_evaluator() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".codemap")).unwrap();
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/lib.rs"), "pub fn target_handler() { let marker = 1; }\n").unwrap();
    std::fs::write(temp.path().join("src/widget.ts"), "export class Widget {\n  widget_method() {\n    return 7;\n  }\n}\n").unwrap();
    std::fs::write(temp.path().join("src/constants.rs"), "pub const WIDGET_CONSTANT: usize = 7;\n").unwrap();
    let config_path = temp.path().join(".codemap/config.toml");
    let settings = |threshold| format!("[update]\nconfig_auto_update=false\n[index.refresh]\nwatch=false\n[analysis.jev]\noverview_enabled=true\nsearch_filter_enabled=true\nsearch_filter_min_unrelated_probability={threshold}\n");
    std::fs::write(&config_path,settings("0.70")).unwrap();
    std::env::set_var("CODEMAP_HOME",temp.path());
    std::env::set_current_dir(temp.path()).unwrap();
    crate::config::reload(temp.path());
    let engine = crate::index::TantivySearchEngine::new(&crate::config::get().index_path).unwrap();
    let searcher = engine.searcher_handle();
    let indexer = crate::index::spawn_indexer(engine);
    let supervisor = crate::index::EngineSupervisor::new(searcher,None,None,indexer,
        Arc::new(crate::index::WatcherStatus::default()));
    let mut server = McpServer::new(supervisor);
    server.set_call_logging_enabled(false);
    let fake = Arc::new(Fake {calls:AtomicUsize::new(0),should_fail:AtomicBool::new(false),is_no_match:AtomicBool::new(false)});
    server.set_jev_evaluator(fake.clone());
    for _ in 0..200 {
        if !server.engine.is_warming() && !server.engine.published_snapshot().codemap().is_empty() {break;}
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(!server.engine.published_snapshot().codemap().is_empty(),"populated indexed fixture required");
    let base = server.handle_request("tools/call",Some(&serde_json::json!({"name":"search","arguments":{"query":"target_handler","caller_context":false}}))).await.unwrap();
    assert!(text(&base).contains("target_handler") && text(&base).contains("marker"),"{base}");
    let args = serde_json::json!({"name":"search","arguments":{"query":"target_handler","task_query":"Find the handler","caller_context":false}});
    let filtered = server.handle_request("tools/call",Some(&args)).await.unwrap();
    assert_eq!(filtered["_meta"]["jev"]["outcome"],"applied","{filtered}");
    assert!(!text(&filtered).contains("let marker"),"{filtered}");
    assert!(text(&filtered).contains("target_handler") && text(&filtered).contains("Next: read"),"{filtered}");
    assert!(server.pending_source_files.is_empty(),"omitted-only body must not be recorded as delivered source");
    assert_eq!(filtered["_meta"]["jev"]["min_unrelated_probability"],0.70);
    let ts = server.handle_request("tools/call",Some(&serde_json::json!({"name":"search","arguments":{"query":"widget_method","task_query":"Find a widget method","caller_context":false}}))).await.unwrap();
    assert!(text(&ts).contains("widget_method") && text(&ts).contains("return 7"),"nested TypeScript body must survive: {ts}");
    let constant = server.handle_request("tools/call",Some(&serde_json::json!({"name":"search","arguments":{"query":"WIDGET_CONSTANT","task_query":"Locate the constant","caller_context":false}}))).await.unwrap();
    assert!(text(&constant).contains("WIDGET_CONSTANT") && text(&constant).contains("= 7"),"Rust constant must survive: {constant}");

    let root = server.handle_request("tools/call",Some(&serde_json::json!({"name":"overview","arguments":{"task_query":"Find the handler"}}))).await.unwrap();
    assert_eq!(root["_meta"]["jev"]["outcome"],"applied","{root}");
    assert_eq!(root["_meta"]["jev"]["recommendation_status"],"matched");
    assert!(text(&root).contains("src/lib.rs") && text(&root).contains("read {"),"{root}");
    fake.is_no_match.store(true,Ordering::SeqCst);
    let no_match = server.handle_request("tools/call",Some(&serde_json::json!({"name":"overview","arguments":{"task_query":"Find the handler"}}))).await.unwrap();
    assert_eq!(no_match["_meta"]["jev"]["outcome"],"applied");
    assert_eq!(no_match["_meta"]["jev"]["recommendation_status"],"no_match");
    assert!(!text(&no_match).contains("## Jev indexed recommendations"));
    fake.is_no_match.store(false,Ordering::SeqCst);
    let before = fake.calls.load(Ordering::SeqCst);
    let (in_flight, ()) = tokio::join!(
        server.handle_request("tools/call",Some(&args)),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            std::fs::write(&config_path,settings("0.90")).unwrap();
            crate::config::reload(temp.path());
        }
    );
    let in_flight = in_flight.unwrap();
    assert_eq!(in_flight["_meta"]["jev"]["min_unrelated_probability"],0.70);
    assert!(!text(&in_flight).contains("let marker"));
    let kept = server.handle_request("tools/call",Some(&args)).await.unwrap();
    assert_eq!(kept["_meta"]["jev"]["min_unrelated_probability"],0.90,"{kept}");
    assert!(text(&kept).contains("let marker"),"{kept}");
    assert!(fake.calls.load(Ordering::SeqCst) > before);
    fake.should_fail.store(true,Ordering::SeqCst);
    let fallback = server.handle_request("tools/call",Some(&args)).await.unwrap();
    assert_eq!(fallback["_meta"]["jev"]["outcome"],"fallback");
    assert_eq!(fallback["_meta"]["jev"]["reason"],"InvalidResponse");
    assert_eq!(text(&fallback),text(&base));
    assert!(!server.pending_source_files.is_empty());
    let old = server.handle_request("tools/call",Some(&serde_json::json!({"name":"overview","arguments":{}}))).await.unwrap();
    assert!(old.get("_meta").is_some());
    assert_eq!(old["_meta"]["jev"]["reason"],"missing_task_query");
}

/// Run the cwd/config-global fixture in an isolated test process, not alongside unrelated
/// package unit tests. No production endpoint override or executable test hook is needed.
#[test]
fn test_jev_mcp_native_injected_paths() {
    if std::env::var_os("CODEMAP_JEV_ISOLATED_TEST_CHILD").is_some() {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
            .block_on(exercise_mcp_with_injected_evaluator());
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact","mcp::jev_tests::test_jev_mcp_native_injected_paths","--nocapture"])
        .env("CODEMAP_JEV_ISOLATED_TEST_CHILD","1").output().unwrap();
    assert!(output.status.success(),"isolated MCP test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
}
