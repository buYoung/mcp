use codemap_search::{
    config,
    index::{spawn_indexer, EngineSupervisor, TantivySearchEngine, WatcherStatus},
    jev::*,
    mcp::McpServer,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::time::{Duration, Instant};

const PRIMARY: &str =
    "pub fn discard() -> u32 {\n    let unrelated_marker = 17;\n    unrelated_marker\n}\n";
const SUPPORT: &str =
    "pub fn retained() -> u32 {\n    let supporting_marker = 42;\n    supporting_marker\n}\n";
const NESTED:&str="export class Widget {\n    run() {\n        return this.finish();\n    }\n    finish() {\n        return 42;\n    }\n}\n";

fn write_config(overview: bool, filter: bool, threshold: f64) {
    std::fs::create_dir_all(".codemap").unwrap();
    std::fs::write(".codemap/config.toml",format!("[update]\nconfig_auto_update=false\n[index.refresh]\nwatch=false\n[output.overview]\nis_stats_enabled=false\n[analysis.jev]\noverview_enabled={overview}\nsearch_filter_enabled={filter}\nsearch_filter_min_unrelated_probability={threshold}\napi_key_env='CODEMAP_JEV_ABSENT_KEY'\n")).unwrap();
    config::init(std::path::Path::new("."));
}

#[derive(Default)]
struct Mock {
    calls: Mutex<Vec<Value>>,
    mode: AtomicUsize,
    should_delay: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl Transport for Mock {
    fn send(&self, bytes: Vec<u8>) -> DecisionFuture<'_, Result<Vec<u8>, FailureKind>> {
        Box::pin(async move {
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            self.calls.lock().unwrap().push(request.clone());
            if self.should_delay.swap(false, Ordering::SeqCst) {
                self.started.notify_one();
                self.release.notified().await;
            }
            if self.mode.load(Ordering::SeqCst) == 1 {
                return Ok(b"{}".to_vec());
            }
            let no_match = self.mode.load(Ordering::SeqCst) == 2;
            let answers=request["questions"].as_object().unwrap().iter().map(|(id,question)|{
                let answer=match question["type"].as_str().unwrap() {
                    "score"=>{
                        let criteria=question["criteria"].as_array().unwrap();let score=if no_match{0}else{2.min(criteria.len()-1)};
                        let probabilities=criteria.iter().enumerate().map(|(i,_)|(i.to_string(),json!(if i==score{1.0}else{0.0}))).collect::<serde_json::Map<_,_>>();
                        let legend=criteria.iter().enumerate().map(|(i,description)|(i.to_string(),description.clone())).collect::<serde_json::Map<_,_>>();
                        json!({"type":"score","score":score,"probabilities":probabilities,"legend":legend,"confidence":1.0})
                    },
                    "choice"=>{
                        let criteria=question["criteria"].as_object().unwrap();let selected=criteria.keys().find(|key|key.as_str()=="implementation").unwrap();
                        let probabilities=criteria.keys().map(|key|(key.clone(),json!(if key==selected{1.0}else{0.0}))).collect::<serde_json::Map<_,_>>();
                        json!({"type":"choice","choice":selected,"probabilities":probabilities,"confidence":1.0})
                    },
                    "noul"=>json!({"type":"noul","noul":if self.mode.load(Ordering::SeqCst)==3 {1.0} else {0.8}}),
                    _=>panic!("unexpected primitive"),
                };(id.clone(),answer)
            }).collect::<serde_json::Map<_,_>>();
            Ok(serde_json::to_vec(&json!({"model":MODEL,"answers":answers,"usage":{"input_tokens":23,"output_tokens":7}})).unwrap())
        })
    }
}

async fn engine() -> EngineSupervisor {
    let engine = TantivySearchEngine::new(&config::get().index_path).unwrap();
    let searcher = engine.searcher_handle();
    let indexer = spawn_indexer(engine);
    let engine = EngineSupervisor::new(
        searcher,
        None,
        None,
        indexer,
        Arc::new(WatcherStatus::default()),
    );
    tokio::time::timeout(Duration::from_secs(15), async {
        while engine.is_warming() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        engine.published_snapshot().codemap().len() >= 3,
        "fixture must be populated"
    );
    engine
}

async fn call(server: &mut McpServer, name: &str, args: Value) -> Value {
    server
        .handle_request("tools/call", Some(&json!({"name":name,"arguments":args})))
        .await
        .unwrap()
}
fn text(value: &Value) -> &str {
    value["content"][0]["text"].as_str().unwrap()
}
fn search_args() -> Value {
    json!({"query":"discard","task_query":"Find timeout flow","caller_context":false,"include_events":false})
}

#[test]
fn test_native_jev_injected_matrix() {
    if std::env::var("CODEMAP_JEV_ISOLATED_TEST").as_deref() != Ok("1") {
        let repo = super::helpers::create_mock_repo(&[
            ("src/primary.rs", PRIMARY),
            ("src/support.rs", SUPPORT),
            ("src/nested.ts", NESTED),
        ])
        .unwrap();
        super::helpers::run_isolated_jev_test(
            "e2e::jev::test_native_jev_injected_matrix",
            repo.path(),
        );
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            write_config(false, false, 0.70);
            let fake = Arc::new(Mock::default());
            let evaluator = Arc::new(Runtime::new(fake.clone(), Policy::default()).unwrap());
            let mut server = McpServer::with_evaluator(engine().await, evaluator);
            server.set_call_logging_enabled(false);
            let baseline = call(&mut server, "search", search_args()).await;
            assert!(text(&baseline).contains("let unrelated_marker = 17"));
            assert!(baseline.get("_meta").is_none());
            assert!(fake.calls.lock().unwrap().is_empty());
            for (overview, filter) in [(false, false), (true, false), (false, true), (true, true)] {
                write_config(overview, filter, 0.70);
                let root = call(
                    &mut server,
                    "overview",
                    json!({"task_query":"Find timeout flow"}),
                )
                .await;
                let search = call(&mut server, "search", search_args()).await;
                assert_eq!(root.get("_meta").is_some(), overview);
                assert_eq!(search.get("_meta").is_some(), filter);
                if overview {
                    assert_eq!(root["_meta"]["jev"]["outcome"], "applied");
                    assert_eq!(root["_meta"]["jev"]["evaluated_count"], 3);
                }
                if filter {
                    assert_eq!(search["_meta"]["jev"]["outcome"], "applied", "{search}");
                    assert_eq!(search["_meta"]["jev"]["evaluated_count"], 1, "{search}");
                    assert!(
                        !text(&search).contains("let unrelated_marker = 17"),
                        "{search}"
                    );
                    assert!(text(&search).contains("discard (fn) [L1-4]"), "{search}");
                } else {
                    assert_eq!(search, baseline);
                }
            }
            let count = fake.calls.lock().unwrap().len();
            for args in [
                json!({"query":"discard"}),
                json!({"query":"discard","task_query":"  "}),
            ] {
                let result = call(&mut server, "search", args).await;
                assert_eq!(result["_meta"]["jev"]["reason"], "missing_task_query");
            }
            for name in ["search", "overview"] {
                let params = json!({"name":name,"arguments":{"query":"discard","task_query":4}});
                assert_eq!(
                    server
                        .handle_request("tools/call", Some(&params))
                        .await
                        .err()
                        .unwrap()
                        .0,
                    -32602
                );
            }
            let folder = call(
                &mut server,
                "overview",
                json!({"query":"src","task_query":"Find timeout flow"}),
            )
            .await;
            assert_eq!(folder["_meta"]["jev"]["reason"], "non_root_overview");
            call(
                &mut server,
                "read",
                json!({"file_path":"src/primary.rs","view":"source"}),
            )
            .await;
            call(&mut server, "find", json!({"pattern":"*.rs","path":"src"})).await;
            call(
                &mut server,
                "grep",
                json!({"pattern":"unrelated_marker","path":"src","view":"source","expand":"none"}),
            )
            .await;
            let event = call(
                &mut server,
                "search",
                json!({"query":"discard","event_key":"absent","task_query":"Find timeout flow"}),
            )
            .await;
            assert_eq!(event["_meta"]["jev"]["outcome"], "bypassed");
            assert_eq!(fake.calls.lock().unwrap().len(), count);
            fake.mode.store(2, Ordering::SeqCst);
            let no_match = call(
                &mut server,
                "overview",
                json!({"task_query":"Find timeout flow"}),
            )
            .await;
            assert_eq!(
                no_match["_meta"]["jev"]["recommendation_status"],
                "no_match"
            );
            assert_eq!(no_match["_meta"]["jev"]["selected_count"], 0);
            fake.mode.store(1, Ordering::SeqCst);
            let fallback = call(&mut server, "search", search_args()).await;
            assert_eq!(fallback["_meta"]["jev"]["outcome"], "fallback");
            assert!(text(&fallback).contains("let unrelated_marker = 17"));
            fake.mode.store(0, Ordering::SeqCst);
            fake.should_delay.store(true, Ordering::SeqCst);
            let params = json!({"name":"search","arguments":search_args()});
            let pending = server.handle_request("tools/call", Some(&params));
            let change = async {
                fake.started.notified().await;
                write_config(true, true, 0.90);
                fake.release.notify_one();
            };
            let (result, _) = tokio::join!(pending, change);
            let result = result.unwrap();
            assert_eq!(
                result["_meta"]["jev"]["search_filter_min_unrelated_probability"],
                0.70
            );
            assert!(!text(&result).contains("let unrelated_marker = 17"));
            let next = call(&mut server, "search", search_args()).await;
            assert_eq!(
                next["_meta"]["jev"]["search_filter_min_unrelated_probability"],
                0.90
            );
            assert!(text(&next).contains("let unrelated_marker = 17"));
            let options = EvaluationOptions {
                deadline: Instant::now() + Duration::from_millis(1),
                ..Default::default()
            };
            let expired = server
                .handle_request_with_options("tools/call", Some(&params), options)
                .await
                .unwrap();
            assert_eq!(expired["_meta"]["jev"]["reason"], "Deadline");
            assert!(text(&expired).contains("let unrelated_marker = 17"));
            let options = EvaluationOptions::default();
            options.cancellation.cancel();
            let cancelled = server
                .handle_request_with_options("tools/call", Some(&params), options)
                .await
                .unwrap();
            assert_eq!(cancelled["_meta"]["jev"]["reason"], "Cancelled");
            drop(server);
            let mut keyless = McpServer::new(engine().await);
            keyless.set_call_logging_enabled(false);
            let keyless_result = call(&mut keyless, "search", search_args()).await;
            assert_eq!(
                keyless_result["_meta"]["jev"]["reason"],
                "missing_credentials"
            );
            assert!(text(&keyless_result).contains("let unrelated_marker = 17"));
        });
}

#[test]
fn test_native_jev_redaction_accounting_and_caps() {
    const SENSITIVE:&str="pub fn discard() -> usize {\n    let password = \"fixture-only-credential\";\n    password.len()\n}\n";
    if std::env::var("CODEMAP_JEV_ISOLATED_TEST").as_deref() != Ok("1") {
        let repo = super::helpers::create_mock_repo(&[
            ("src/primary.rs", SENSITIVE),
            ("src/support.rs", SUPPORT),
            ("src/nested.ts", NESTED),
        ])
        .unwrap();
        super::helpers::run_isolated_jev_test(
            "e2e::jev::test_native_jev_redaction_accounting_and_caps",
            repo.path(),
        );
        return;
    }
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        write_config(true,true,0.70);
        let fake=Arc::new(Mock::default());let evaluator=Arc::new(Runtime::new(fake.clone(),Policy::default()).unwrap());
        let engine=engine().await;let snapshot=engine.published_snapshot();
        let indexed_before=serde_json::to_value(snapshot.codemap().as_ref()).unwrap();
        let mut server=McpServer::with_evaluator(engine,evaluator);
        let omitted=call(&mut server,"search",search_args()).await;
        assert_eq!(omitted["_meta"]["jev"]["outcome"],"applied");assert_eq!(omitted["_meta"]["jev"]["selected_count"],1);
        assert!(!text(&omitted).contains("password.len()"));
        let db=rusqlite::Connection::open(".codemap/analysis.sqlite3").unwrap();
        assert_eq!(db.query_row("SELECT count(*) FROM file_observations",[],|row|row.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(db.query_row("SELECT response_bytes FROM calls ORDER BY id DESC LIMIT 1",[],|row|row.get::<_,i64>(0)).unwrap(),text(&omitted).len() as i64);
        assert!(!fake.calls.lock().unwrap().last().unwrap().to_string().contains("fixture-only-credential"));
        assert!(fake.calls.lock().unwrap().last().unwrap().to_string().contains("[REDACTED]"));

        write_config(true,true,0.90);
        let retained=call(&mut server,"search",search_args()).await;
        assert!(text(&retained).contains("password.len()"));assert!(!text(&retained).contains("fixture-only-credential"));
        let observation:(String,i64)=db.query_row("SELECT path,result_bytes FROM file_observations ORDER BY call_id DESC LIMIT 1",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(observation.0,"src/primary.rs");assert!(observation.1>0 && observation.1<=text(&retained).len() as i64);
        let overview=call(&mut server,"overview",json!({"task_query":"Find timeout flow"})).await;
        assert_eq!(overview["_meta"]["jev"]["outcome"],"applied");
        assert!(!fake.calls.lock().unwrap().iter().any(|call|call.to_string().contains("fixture-only-credential")));
        assert_eq!(std::fs::read_to_string("src/primary.rs").unwrap(),SENSITIVE);
        assert_eq!(serde_json::to_value(snapshot.codemap().as_ref()).unwrap(),indexed_before);

        use std::io::Write;
        std::fs::OpenOptions::new().append(true).open(".codemap/config.toml").unwrap().write_all(b"[output]\nis_redact_enabled=false\n").unwrap();config::init(std::path::Path::new("."));
        let unmasked=call(&mut server,"search",search_args()).await;
        assert!(text(&unmasked).contains("fixture-only-credential"));
        assert!(fake.calls.lock().unwrap().last().unwrap().to_string().contains("fixture-only-credential"));

        write_config(true,true,0.70);
        std::fs::OpenOptions::new().append(true).open(".codemap/config.toml").unwrap().write_all(b"[output.search]\nmax_bytes=240\n").unwrap();
        let contents=std::fs::read_to_string(".codemap/config.toml").unwrap().replace("is_stats_enabled=false","is_stats_enabled=false\nmax_bytes=4096");
        std::fs::write(".codemap/config.toml",contents).unwrap();config::init(std::path::Path::new("."));
        let capped=call(&mut server,"search",search_args()).await;
        assert!(text(&capped).len()<=240);assert!(!text(&capped).contains("fixture-only-credential"));assert!(capped["_meta"]["jev"].is_object());
        assert_eq!(text(&capped).matches("```\n").count()%2,0);
        let read_bytes:i64=db.query_row("SELECT coalesce(sum(result_bytes),0) FROM file_observations WHERE call_id=(SELECT max(id) FROM calls)",[],|row|row.get(0)).unwrap();
        assert!(read_bytes<=text(&capped).len() as i64);
    });
}

#[test]
fn test_native_jev_snapshot_refresh_during_evaluation() {
    if std::env::var("CODEMAP_JEV_ISOLATED_TEST").as_deref() != Ok("1") {
        let repo = super::helpers::create_mock_repo(&[
            ("src/primary.rs", PRIMARY),
            ("src/support.rs", SUPPORT),
            ("src/nested.ts", NESTED),
        ])
        .unwrap();
        super::helpers::run_isolated_jev_test(
            "e2e::jev::test_native_jev_snapshot_refresh_during_evaluation",
            repo.path(),
        );
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            write_config(true, false, 0.70);
            let search_engine = TantivySearchEngine::new(&config::get().index_path).unwrap();
            let observer = search_engine.searcher_handle();
            let searcher = search_engine.searcher_handle();
            let indexer = spawn_indexer(search_engine);
            let sender = indexer.command_sender();
            tokio::time::timeout(Duration::from_secs(15), async {
                while indexer.is_warming() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(indexer.published_snapshot().codemap().len(), 3);
            let fake = Arc::new(Mock::default());
            fake.should_delay.store(true, Ordering::SeqCst);
            let evaluator = Arc::new(Runtime::new(fake.clone(), Policy::default()).unwrap());
            let engine = EngineSupervisor::new(
                searcher,
                None,
                None,
                indexer,
                Arc::new(WatcherStatus::default()),
            );
            let mut server = McpServer::with_evaluator(engine, evaluator);
            server.set_call_logging_enabled(false);
            let params = json!({"name":"overview","arguments":{"task_query":"Find timeout flow"}});
            let pending = server.handle_request("tools/call", Some(&params));
            let update = async {
                fake.started.notified().await;
                std::fs::write("src/newly_added.rs", "pub fn newly_added() -> u32 { 99 }\n")
                    .unwrap();
                while sender
                    .try_send(codemap_search::index::IndexCommand::Refresh)
                    .is_err()
                {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                tokio::time::timeout(Duration::from_secs(15), async {
                    while observer.search("newly_added", 5).unwrap().is_empty() {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
                fake.release.notify_one();
            };
            let (result, _) = tokio::join!(pending, update);
            let result = result.unwrap();
            assert_eq!(result["_meta"]["jev"]["evaluated_count"], 3);
            assert!(!text(&result).contains("newly_added"));
            let next = call(
                &mut server,
                "overview",
                json!({"task_query":"Find timeout flow"}),
            )
            .await;
            assert_eq!(next["_meta"]["jev"]["evaluated_count"], 4);
            assert!(text(&next).contains("newly_added.rs"));
            drop(sender);
        });
}

#[test]
fn test_native_jev_schema_and_no_background_evaluation() {
    if std::env::var("CODEMAP_JEV_ISOLATED_TEST").as_deref() != Ok("1") {
        let repo = super::helpers::create_mock_repo(&[
            ("src/primary.rs", PRIMARY),
            ("src/support.rs", SUPPORT),
            ("src/nested.ts", NESTED),
        ])
        .unwrap();
        super::helpers::run_isolated_jev_test(
            "e2e::jev::test_native_jev_schema_and_no_background_evaluation",
            repo.path(),
        );
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            write_config(false, false, 0.70);
            let fake = Arc::new(Mock::default());
            let evaluator = Arc::new(Runtime::new(fake.clone(), Policy::default()).unwrap());
            let mut server = McpServer::with_evaluator(engine().await, evaluator);
            for (overview, filter) in [(false, false), (true, false), (false, true), (true, true)] {
                write_config(overview, filter, 0.70);
                let tools = server.handle_request("tools/list", None).await.unwrap();
                for tool in tools["tools"].as_array().unwrap() {
                    let name = tool["name"].as_str().unwrap();
                    let is_enabled = match name {
                        "overview" => overview,
                        "search" => filter,
                        _ => false,
                    };
                    assert_eq!(tool["annotations"]["openWorldHint"], is_enabled);
                    assert_eq!(tool["annotations"]["readOnlyHint"], true);
                    if ["overview", "search"].contains(&name) {
                        assert_eq!(
                            tool["inputSchema"]["properties"]["task_query"]["type"],
                            "string"
                        );
                    }
                    if name == "search" {
                        assert_eq!(tool["inputSchema"]["required"], json!(["query"]));
                    }
                }
                call(&mut server, "initial_instructions", json!({})).await;
                server
                    .handle_request("initialize", Some(&json!({"protocolVersion":"2024-11-05"})))
                    .await
                    .unwrap();
                server.handle_request("ping", None).await.unwrap();
            }
            assert!(fake.calls.lock().unwrap().is_empty());
        });
}

#[test]
fn test_native_jev_preserves_adjacent_same_line_declarations() {
    const ADJACENT: &str = "pub fn discard() -> u32 { 17 } pub fn neighbor() -> u32 { 42 }\n";
    if std::env::var("CODEMAP_JEV_ISOLATED_TEST").as_deref() != Ok("1") {
        let repo = super::helpers::create_mock_repo(&[
            ("src/primary.rs", ADJACENT),
            ("src/support.rs", SUPPORT),
            ("src/nested.ts", NESTED),
        ])
        .unwrap();
        super::helpers::run_isolated_jev_test(
            "e2e::jev::test_native_jev_preserves_adjacent_same_line_declarations",
            repo.path(),
        );
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            write_config(false, true, 0.70);
            let fake = Arc::new(Mock::default());
            let evaluator = Arc::new(Runtime::new(fake.clone(), Policy::default()).unwrap());
            let mut server = McpServer::with_evaluator(engine().await, evaluator);
            let result = call(&mut server, "search", search_args()).await;
            assert_eq!(result["_meta"]["jev"]["outcome"], "applied");
            assert!(
                text(&result).contains(ADJACENT.trim_end()),
                "adjacent declaration source must survive: {result}"
            );
            assert_eq!(result["_meta"]["jev"]["selected_count"], 0);
        });
}

#[test]
fn test_native_jev_protects_rust_and_typescript_declarations_at_one() {
    const STRUCTURED: &str = "pub struct State { pub value: u32 }\nimpl State {\n    pub fn run(&self) -> u32 {\n        LIMIT + self.value\n    }\n}\npub const LIMIT: u32 = 9;\n";
    if std::env::var("CODEMAP_JEV_ISOLATED_TEST").as_deref() != Ok("1") {
        let repo = super::helpers::create_mock_repo(&[
            ("src/primary.rs", STRUCTURED),
            ("src/support.rs", SUPPORT),
            ("src/nested.ts", NESTED),
        ]).unwrap();
        super::helpers::run_isolated_jev_test("e2e::jev::test_native_jev_protects_rust_and_typescript_declarations_at_one", repo.path());
        return;
    }
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        write_config(false, false, 0.70);
        let fake = Arc::new(Mock::default());
        fake.mode.store(3, Ordering::SeqCst);
        let evaluator = Arc::new(Runtime::new(fake.clone(), Policy::default()).unwrap());
        let mut server = McpServer::with_evaluator(engine().await, evaluator);
        let cases = [
            (json!({"query":"State run LIMIT","language_hint":"rust","task_query":"Inspect ordering","caller_context":false,"include_events":false}), "LIMIT + self.value"),
            (json!({"query":"Widget run finish","language_hint":"typescript","task_query":"Inspect ordering","caller_context":false,"include_events":false}), "return this.finish()"),
        ];
        for (arguments, expected_source) in cases {
            write_config(false, false, 0.70);
            let baseline = call(&mut server, "search", arguments.clone()).await;
            assert!(text(&baseline).contains(expected_source), "{baseline}");
            write_config(false, true, 0.70);
            let filtered = call(&mut server, "search", arguments).await;
            assert_eq!(filtered["_meta"]["jev"]["outcome"], "applied", "{filtered}");
            assert!(filtered["_meta"]["jev"]["evaluated_count"].as_u64().unwrap() > 0);
            assert!(text(&filtered).starts_with(text(&baseline)), "{filtered}");
            assert_eq!(filtered["_meta"]["jev"]["selected_count"], 0);
        }
        assert!(fake.calls.lock().unwrap().len() >= 2);
    });
}
