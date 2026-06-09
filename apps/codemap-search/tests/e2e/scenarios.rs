use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use predicates::prelude::*;

#[tokio::test]
async fn test_realworld_small_repo() {
    // 5 files, typical structure
    let temp = create_mock_repo(&[
        ("Cargo.toml", "[package]\nname = \"small-repo\""),
        ("src/main.rs", "fn main() {}"),
        ("src/lib.rs", "pub fn helper() {}"),
        ("src/utils.rs", "pub fn calculate() {}"),
        ("src/db.rs", "pub fn connect() {}"),
        (
            "queries.json",
            r#"[{"query": "calculate", "expected": ["src/utils.rs"]}]"#,
        ),
    ])
    .unwrap();

    // Index
    let assert_index = run_cli(&["index"], temp.path());
    assert_index.success();

    // Search
    let assert_search = run_cli(&["search", "calculate"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/utils.rs"));

    // Benchmark
    let assert_bench = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert_bench.success();
}

#[tokio::test]
async fn test_realworld_medium_repo() {
    // Simulate a medium repository with 50 files
    let mut files = Vec::new();
    for i in 1..=50 {
        let rel_path = format!("src/module_{}/file.rs", i);
        let content = format!("pub fn func_in_{}() {{}}", i);
        files.push((rel_path, content));
    }

    // Add cargo project files and queries
    let cargo_content = "[package]\nname = \"med-repo\"";
    let query_content = r#"[{"query": "func_in_25", "expected": ["src/module_25/file.rs"]}]"#;

    let mut mock_files = Vec::new();
    mock_files.push(("Cargo.toml".to_string(), cargo_content.to_string()));
    mock_files.push(("queries.json".to_string(), query_content.to_string()));
    for (path, content) in &files {
        mock_files.push((path.clone(), content.clone()));
    }

    let mock_files_refs: Vec<(&str, &str)> = mock_files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();

    let temp = create_mock_repo(&mock_files_refs).unwrap();

    // Run full pipeline
    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "func_in_25"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/module_25/file.rs"));
}

#[tokio::test]
async fn test_realworld_todo_finder() {
    let temp = create_mock_repo(&[
        (
            "src/auth.rs",
            "// TODO: setup OAuth2 provider\nfn auth() {}",
        ),
        (
            "src/db.rs",
            "// FIXME: connection leak under load\nfn connect() {}",
        ),
        ("src/ok.rs", "fn normal() {}"),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // TODO/FIXME discovery is handled by `grep` (raw file contents), not `search`
    // (Child 03 — comments are no longer promoted into the BM25 index).
    let res = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "grep",
                "arguments": {
                    "pattern": "TODO",
                    "output_mode": "content"
                }
            }),
        )
        .await
        .unwrap();

    let text = res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("auth.rs"),
        "grep should find the TODO comment file: {text:?}"
    );
    assert!(
        text.contains("TODO"),
        "grep content mode should surface the TODO text: {text:?}"
    );
}

#[test]
fn test_realworld_multilang_workspace() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn code() {}"),
        ("README.md", "# Documentation\nSome details here"),
        ("package.json", "{\"name\": \"pkg\"}"),
        ("src/assets/logo.png", "png-binary"),
    ])
    .unwrap();

    let assert_codemap = run_cli(&["codemap"], temp.path());
    assert_codemap
        .success()
        .stdout(predicates::str::contains("src/lib.rs"))
        .stdout(predicates::str::contains("package.json").not())
        .stdout(predicates::str::contains("README.md").not());
}

#[tokio::test]
async fn test_realworld_ai_agent_simulation() {
    let temp = create_mock_repo(&[
        ("src/main.rs", "fn main() {}"),
        ("src/core/mod.rs", "pub fn run_engine() {}"),
        (
            "queries.json",
            r#"[{"query": "run_engine", "expected": ["src/core/mod.rs"]}]"#,
        ),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // 1. Initial handshake
    let init_res = client
        .send_request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "ai-agent", "version": "1.0" }
            }),
        )
        .await
        .unwrap();
    assert!(init_res.get("result").is_some());

    // 2. Listing root codemap
    let root_res = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "" }
            }),
        )
        .await
        .unwrap();
    assert!(root_res["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("src/main.rs"));

    // 3. Search for a symbol
    let search_res = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "run_engine" }
            }),
        )
        .await
        .unwrap();
    assert!(search_res["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("src/core/mod.rs"));

    // 4. Get file detail
    let detail_res = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "src/core/mod.rs" }
            }),
        )
        .await
        .unwrap();
    assert!(detail_res["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("run_engine"));
}
