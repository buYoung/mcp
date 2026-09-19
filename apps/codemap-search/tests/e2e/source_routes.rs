//! Product checks use the real MCP binary; the target programs are never run.
use super::helpers::{create_mock_repo, McpClient};
use codemap_search::parser::{CodeExtractor, TreeSitterExtractor};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::BufReader;
use tokio::process::Command;

fn text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .into()
}
async fn tool(client: &mut McpClient, name: &str, args: Value) -> String {
    let response = client
        .send_request("tools/call", json!({"name":name,"arguments":args}))
        .await
        .unwrap();
    assert!(
        response.get("error").is_none() && response["result"]["isError"] != true,
        "{response}"
    );
    text(&response)
}
async fn isolated_client(root: &Path) -> McpClient {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin("codemap-search"))
        .arg("mcp")
        .current_dir(root)
        .env("CODEMAP_HOME", root)
        .env("PATH", root.join("no-external-tools"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    McpClient {
        stdin: child.stdin.take().unwrap(),
        stdout_reader: BufReader::new(child.stdout.take().unwrap()),
        child,
        request_id: 1,
    }
}
fn binary_digest() -> String {
    let mut file = std::fs::File::open(assert_cmd::cargo::cargo_bin("codemap-search")).unwrap();
    let mut hash = blake3::Hasher::new();
    let mut bytes = [0; 65536];
    loop {
        let read = file.read(&mut bytes).unwrap();
        if read == 0 {
            break;
        }
        hash.update(&bytes[..read]);
    }
    hash.finalize().to_hex().to_string()
}
fn report(name: &str, value: Value) {
    if let Ok(root) = std::env::var("CODEMAP_SOURCE_ROUTES_LIVE_REPORT_DIR") {
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            Path::new(&root).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}
const CONFIG:&str="[update]\nconfig_auto_update=false\n[event_navigation]\nis_enabled=true\nuse_builtin_rules=false\n[macro_expansion]\nis_enabled=false\n[refresh]\nwatch=false\nindex_staleness_ms=600000\n";

#[tokio::test]
async fn test_events_source_routes_all_languages_live_and_restart() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("experiments/event-navigation-poc/language-corpus");
    let manifest = std::fs::read(root.join("fixture-cases.json")).unwrap();
    let cases: Value = serde_json::from_slice(&manifest).unwrap();
    let cases: Vec<_> = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["id"].as_str().unwrap().ends_with("fixture-primary"))
        .cloned()
        .collect();
    assert_eq!(cases.len(), 18);
    let mut inputs = Vec::new();
    for case in &cases {
        let original = case["storage"][0].as_str().unwrap();
        let file = Path::new(original).file_name().unwrap().to_str().unwrap();
        inputs.push((
            format!("src/{}/{file}", case["language"].as_str().unwrap()),
            std::fs::read_to_string(root.join(original)).unwrap(),
        ));
    }
    let mut entries: Vec<_> = inputs
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    entries.push((".codemap/config.toml", CONFIG));
    let repo = create_mock_repo(&entries).unwrap();
    let binary = binary_digest();
    let started = Instant::now();
    let mut client = isolated_client(repo.path()).await;
    let ready = client
        .send_tool_until(
            "read",
            json!({"file_path":inputs[0].0,"offset":cases[0]["storage"][1],"limit":1}),
            |out| out.contains("Source routes"),
        )
        .await
        .unwrap();
    assert!(text(&ready).contains("Source routes"), "{}", text(&ready));
    let cold_ready_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut rows = Vec::new();
    for (case, (path, source)) in cases.iter().zip(&inputs) {
        println!("LIVE {}", case["language"]);
        let started = Instant::now();
        let mut sizes = Vec::new();
        for side in ["storage", "invocation"] {
            let out = tool(
                &mut client,
                "read",
                json!({"file_path":path,"offset":case[side][1],"limit":1}),
            )
            .await;
            assert!(
                out.contains("Source routes") && out.contains('→'),
                "{} {side}: {out}",
                case["language"]
            );
            assert!(
                out.contains(&format!("L{}", case["storage"][1]))
                    && out.contains(&format!("L{}", case["invocation"][1])),
                "{out}"
            );
            assert!(!out.contains("Value relationships"), "{out}");
            assert!(out.len() <= 16384, "{out}");
            sizes.push(out.len());
        }
        let out = tool(
            &mut client,
            "grep",
            json!({"path":path,"pattern":"call_primary"}),
        )
        .await;
        assert!(
            out.contains("Source routes\n") && out.contains('→'),
            "grep {}: {out}",
            case["language"]
        );
        assert!(out.contains("call_primary"), "{out}");
        sizes.push(out.len());
        let extracted = TreeSitterExtractor::new().extract(source, path).unwrap();
        let line = case["invocation"][1].as_u64().unwrap() as usize;
        let symbol = extracted
            .symbols
            .iter()
            .filter(|s| s.range.start_line <= line && line <= s.range.end_line_inclusive())
            .min_by_key(|s| s.range.end_line - s.range.start_line)
            .or_else(|| {
                extracted
                    .symbols
                    .iter()
                    .filter(|s| s.range.start_line <= line)
                    .max_by_key(|s| (s.range.start_line, s.range.start_col))
            })
            .unwrap_or_else(|| {
                panic!(
                    "fixture consumer symbol {}: {:?}",
                    case["language"], extracted.symbols
                )
            });
        let out = tool(
            &mut client,
            "search",
            json!({"query":format!("{} +{path}",symbol.name)}),
        )
        .await;
        assert!(
            out.contains("Source routes\n") && out.contains('→'),
            "search {}: {out}",
            case["language"]
        );
        sizes.push(out.len());
        rows.push(json!({"language":case["language"],"case":case["id"],"read_storage":true,"read_consumer":true,"grep":true,"search":true,"output_bytes":sizes,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}));
    }
    for (name, args) in [
        (
            "read",
            json!({"file_path":inputs[0].0,"offset":3,"limit":1,"include_events":false}),
        ),
        (
            "read",
            json!({"file_path":inputs[0].0,"offset":3,"limit":1,"view":"source"}),
        ),
        (
            "read",
            json!({"file_path":inputs[0].0,"offset":3,"limit":1,"view":"definitions"}),
        ),
        (
            "grep",
            json!({"path":inputs[0].0,"pattern":"call_primary","include_events":false}),
        ),
        (
            "search",
            json!({"query":format!("callbacks +{}",inputs[0].0),"include_events":false}),
        ),
    ] {
        let out = tool(&mut client, name, args).await;
        assert!(
            !out.contains("Source routes") && !out.contains("Value relationships"),
            "{out}"
        );
    }
    client.kill().await.unwrap();
    let mut client = isolated_client(repo.path()).await;
    let restarted = client
        .send_tool_until(
            "read",
            json!({"file_path":inputs[0].0,"offset":3,"limit":1}),
            |out| out.contains("Source routes"),
        )
        .await
        .unwrap();
    assert!(
        text(&restarted).contains("Source routes"),
        "{}",
        text(&restarted)
    );
    client.kill().await.unwrap();
    report(
        "live-languages",
        json!({"passed":true,"binary_blake3":binary,"case_count":rows.len(),"cases":rows,"cold_ready_ms":cold_ready_ms,"restart_passed":true,"opt_out_checks":5,"target_programs_executed":false,"external_tools_unavailable":true,"input_bytes":inputs.iter().map(|(_,s)|s.len()).sum::<usize>(),"fixture_manifest_blake3":blake3::hash(&manifest).to_hex().to_string()}),
    );
}

#[tokio::test]
async fn test_events_source_routes_support_freshness_and_delete() {
    let entries=[("src/registry.ts","export const routes = new Map<string, () => void>();\n"),("src/barrel.ts","export {routes} from './registry';\n"),("src/store.ts","import {routes} from './barrel';\nexport function register(handler: () => void) { routes.set('tick', handler); }\n"),("src/consumer.ts","import {routes} from './barrel';\nexport function dispatch() { routes.get('tick')?.(); }\n"),(".codemap/config.toml",CONFIG)];
    let repo = create_mock_repo(&entries).unwrap();
    let mut client = isolated_client(repo.path()).await;
    let args = json!({"file_path":"src/consumer.ts","offset":2,"limit":1});
    let ready = client
        .send_tool_until("read", args.clone(), |out| out.contains("Source routes"))
        .await
        .unwrap();
    assert!(text(&ready).contains("Source routes"), "{}", text(&ready));
    std::fs::write(
        repo.path().join("src/barrel.ts"),
        "export {routes} from './registry'; // changed support\n",
    )
    .unwrap();
    let changed = tool(&mut client, "read", args.clone()).await;
    assert!(
        !changed.contains("callback invocation candidate")
            && changed.contains("changed/unavailable evidence"),
        "{changed}"
    );
    std::fs::remove_file(repo.path().join("src/barrel.ts")).unwrap();
    let deleted = tool(&mut client, "read", args.clone()).await;
    assert!(
        !deleted.contains("callback invocation candidate"),
        "{deleted}"
    );
    client.kill().await.unwrap();
    std::fs::write(repo.path().join("src/barrel.ts"), entries[1].1).unwrap();
    let mut client = isolated_client(repo.path()).await;
    let restored = client
        .send_tool_until("read", args, |out| out.contains("Source routes"))
        .await
        .unwrap();
    assert!(
        text(&restored).contains("Source routes"),
        "{}",
        text(&restored)
    );
    client.kill().await.unwrap();
    report(
        "live-freshness",
        json!({"passed":true,"binary_blake3":binary_digest(),"non_endpoint_support_changed":true,"support_deleted":true,"restart_restored":true,"target_programs_executed":false}),
    );
}

#[tokio::test]
async fn test_events_source_routes_input_limit_and_disabled() {
    let body="const routes=new Map();\nexport function register(handler) { routes.set('tick',handler); }\nexport function dispatch() { routes.get('tick')(); }\n";
    let large = format!("{body}\n/*{}*/\n", "x".repeat(524288));
    let repo = create_mock_repo(&[
        ("src/small.js", body),
        ("src/large.js", &large),
        (".codemap/config.toml", CONFIG),
    ])
    .unwrap();
    let mut client = isolated_client(repo.path()).await;
    let ready = client
        .send_tool_until(
            "read",
            json!({"file_path":"src/small.js","offset":2,"limit":1}),
            |out| out.contains("Source routes"),
        )
        .await
        .unwrap();
    assert!(text(&ready).contains("Source routes"), "{}", text(&ready));
    let capped = tool(
        &mut client,
        "read",
        json!({"file_path":"src/large.js","offset":2,"limit":1}),
    )
    .await;
    assert!(
        !capped.contains("callback invocation candidate")
            && capped.contains("524288")
            && capped.contains("input unavailable"),
        "{capped}"
    );
    client.kill().await.unwrap();
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        CONFIG.replace("is_enabled=true", "is_enabled=false"),
    )
    .unwrap();
    let mut client = isolated_client(repo.path()).await;
    let out = client
        .send_tool_until(
            "read",
            json!({"file_path":"src/small.js","offset":2,"limit":1}),
            |out| out.contains("register [function"),
        )
        .await
        .unwrap();
    assert!(
        !text(&out).contains("Source routes") && !text(&out).contains("Value relationships"),
        "{}",
        text(&out)
    );
    client.kill().await.unwrap();
    report(
        "live-limits",
        json!({"passed":true,"binary_blake3":binary_digest(),"default_file_limit_bytes":524288,"oversize_file_bytes":large.len(),"small_positive_control":true,"disabled_configuration":true,"target_programs_executed":false}),
    );
}

#[tokio::test]
async fn test_events_source_routes_exclusions_and_target_reload() {
    let javascript = "const routes=new Map();\nexport function register(handler) { routes.set('tick',handler); }\nexport function dispatch() { routes.get('tick')(); }\n";
    let rust = "struct Linux { callback: fn() }\n#[cfg(target_os=\"linux\")]\nimpl Linux { fn store(&mut self, callback: fn()) { self.callback=callback; } fn run(&self) { (self.callback)(); } }\nstruct Mac { callback: fn() }\n#[cfg(target_os=\"macos\")]\nimpl Mac { fn store(&mut self, callback: fn()) { self.callback=callback; } fn run(&self) { (self.callback)(); } }\n";
    let config = format!(
        "{CONFIG}[exclude]\nshould_include_test_code=false\n[analysis]\ntarget_os=\"linux\"\n"
    );
    let repo = create_mock_repo(&[
        ("src/normal.js", javascript),
        ("tests/callbacks.js", javascript),
        ("src/lib.rs", rust),
        (
            "Cargo.toml",
            "[package]\nname=\"source-proof\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        ),
        (".codemap/config.toml", &config),
    ])
    .unwrap();
    let mut client = isolated_client(repo.path()).await;
    for (path, line) in [("src/normal.js", 2), ("src/lib.rs", 3)] {
        let ready = client
            .send_tool_until(
                "read",
                json!({"file_path":path,"offset":line,"limit":1}),
                |out| out.contains("Source routes\n") && out.contains('→'),
            )
            .await
            .unwrap();
        assert!(
            text(&ready).contains("Source routes\n") && text(&ready).contains('→'),
            "{}",
            text(&ready)
        );
    }
    for (path, line) in [("tests/callbacks.js", 2), ("src/lib.rs", 6)] {
        let out = tool(
            &mut client,
            "read",
            json!({"file_path":path,"offset":line,"limit":1}),
        )
        .await;
        assert!(!out.contains("callback invocation candidate"), "{out}");
    }
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        config
            .replace(
                "should_include_test_code=false",
                "should_include_test_code=true",
            )
            .replace("target_os=\"linux\"", "target_os=\"macos\""),
    )
    .unwrap();
    for (path, line) in [("tests/callbacks.js", 2), ("src/lib.rs", 6)] {
        let ready = client
            .send_tool_until(
                "read",
                json!({"file_path":path,"offset":line,"limit":1}),
                |out| out.contains("Source routes\n") && out.contains('→'),
            )
            .await
            .unwrap();
        assert!(
            text(&ready).contains("Source routes\n") && text(&ready).contains('→'),
            "{}",
            text(&ready)
        );
    }
    let inactive = tool(
        &mut client,
        "read",
        json!({"file_path":"src/lib.rs","offset":3,"limit":1}),
    )
    .await;
    assert!(
        !inactive.contains("callback invocation candidate"),
        "{inactive}"
    );
    client.kill().await.unwrap();
    report(
        "live-exclusions",
        json!({"passed":true,"binary_blake3":binary_digest(),"test_exclusion_and_inclusion_reload":true,"rust_target_exclusion_and_reload":true,"ordinary_source_positive_control":true,"target_programs_executed":false}),
    );
}
