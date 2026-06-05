use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use predicates::prelude::*;
use std::fs;

#[tokio::test]
async fn test_cross_bm25_mcp_branching() {
    // 1. Setup repo with 6 matching files
    let temp = create_mock_repo(&[
        ("src/a.rs", "fn query_func() {}"),
        ("src/b.rs", "fn query_func() {}"),
        ("src/c.rs", "fn query_func() {}"),
        ("src/d.rs", "fn query_func() {}"),
        ("src/e.rs", "fn query_func() {}"),
        ("src/f.rs", "fn query_func() {}"),
    ]).unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // 2. Call search via MCP (>= 5 matches -> list view)
    let res_large = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": { "query": "query_func" }
    })).await.unwrap();

    let text_large = res_large["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text_large.contains("a.rs"));
    assert!(!text_large.contains("fn query_func"));

    // 3. Remove files to make < 5 matches
    fs::remove_file(temp.path().join("src/e.rs")).unwrap();
    fs::remove_file(temp.path().join("src/f.rs")).unwrap();

    // 4. Call search again via MCP (< 5 matches -> scope view)
    let res_small = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": { "query": "query_func" }
    })).await.unwrap();

    let text_small = res_small["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text_small.contains("fn query_func"));
}

#[test]
fn test_cross_extraction_codemaps() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn original() {}"),
    ]).unwrap();

    // Verify original codemap
    let assert_1 = run_cli(&["codemap", "--path", "src/lib.rs"], temp.path());
    assert_1.success()
        .stdout(predicates::str::contains("original"));

    // Modify file to introduce comment and TODO
    fs::write(temp.path().join("src/lib.rs"), "/// Updated doc\n// TODO: verify\npub fn updated() {}").unwrap();

    // Verify dynamic codemap updates
    let assert_2 = run_cli(&["codemap", "--path", "src/lib.rs"], temp.path());
    assert_2.success()
        .stdout(predicates::str::contains("Updated doc"))
        .stdout(predicates::str::contains("hasTodo"))
        .stdout(predicates::str::contains("updated"));
}

#[tokio::test]
async fn test_cross_indexing_mcp_realtime() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
    ]).unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Search via MCP first
    let res_1 = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": { "query": "find_me" }
    })).await.unwrap();
    assert!(res_1["result"]["content"][0]["text"].as_str().unwrap().contains("lib.rs"));

    // Modify file
    fs::write(temp.path().join("src/lib.rs"), "pub fn find_something_else() {}").unwrap();

    // Search new query, should index/reflect modifications immediately
    let res_2 = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": { "query": "find_something_else" }
    })).await.unwrap();
    assert!(res_2["result"]["content"][0]["text"].as_str().unwrap().contains("lib.rs"));
}

#[test]
fn test_cross_benchmark_indexing() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn query() {}"),
        ("queries.json", r#"[{"query": "query", "expected": ["src/lib.rs"]}]"#)
    ]).unwrap();

    // 1. Run benchmark
    let assert_1 = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert_1.success();

    // 2. Clear index or force rebuilding index
    let index_dir = temp.path().join(".codemap-index");
    if index_dir.exists() {
        fs::remove_dir_all(index_dir).unwrap();
    }

    // 3. Re-run benchmark, should still execute successfully (building the index or falling back)
    let assert_2 = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert_2.success();
}

#[tokio::test]
async fn test_cross_mcp_codemaps_dynamic() {
    let temp = create_mock_repo(&[
        ("src/nested/file.rs", "pub fn old_symbol() {}"),
    ]).unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // 1. Get folder level codemap via MCP
    let res_1 = client.send_request("tools/call", serde_json::json!({
        "name": "get_codemap",
        "arguments": { "path": "src/nested" }
    })).await.unwrap();
    assert!(res_1["result"]["content"][0]["text"].as_str().unwrap().contains("file.rs"));

    // 2. Add files inside folder
    fs::write(temp.path().join("src/nested/other.rs"), "pub fn new_symbol() {}").unwrap();

    // 3. Get folder level codemap again, verify dynamic update
    let res_2 = client.send_request("tools/call", serde_json::json!({
        "name": "get_codemap",
        "arguments": { "path": "src/nested" }
    })).await.unwrap();
    let text = res_2["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("other.rs"));
}
