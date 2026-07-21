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
        // This test checks the search renderer's branching threshold, not watcher
        // behavior. Keep request-triggered refreshes active so delete reflection is
        // deterministic.
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // 2. Call search via MCP (6 matches > threshold 5 -> hybrid: 5 detail + 1 ranked tail).
    //    Poll until the initial index covers all 6 files (the tail line appears).
    let res_large = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "query_func" }),
            |t| t.contains("Other matches — 1 more files"),
        )
        .await
        .unwrap();

    let text_large = res_large["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text_large.contains("a.rs"));
    assert_eq!(text_large.matches("### File:").count(), 5);
    assert!(text_large.contains("fn query_func")); // detail sections carry source

    // 3. Remove files to make < 5 matches.
    fs::remove_file(temp.path().join("src/e.rs")).unwrap();
    fs::remove_file(temp.path().join("src/f.rs")).unwrap();

    // 4. Call search again via MCP (4 matches ≤ threshold -> all-detail, no tail). Poll
    //    until both deletions are reflected by a request-triggered refresh.
    let res_small = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "query_func" }),
            |t| {
                !t.contains("Other matches")
                    && t.contains("fn query_func")
                    && t.matches("### File:").count() == 4
            },
        )
        .await
        .unwrap();

    let text_small = res_small["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text_small.contains("fn query_func"));
    assert_eq!(text_small.matches("### File:").count(), 4);
}

#[test]
fn test_cross_extraction_codemaps() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn original() {}")]).unwrap();

    // Verify original codemap
    let assert_1 = run_cli(&["codemap", "--path", "src/lib.rs"], temp.path());
    assert_1
        .success()
        .stdout(predicates::str::contains("original"));

    // Modify file to introduce comment and TODO
    fs::write(
        temp.path().join("src/lib.rs"),
        "/// Updated doc\n// TODO: verify\npub fn updated() {}",
    )
    .unwrap();

    // Verify dynamic codemap updates: the trimmed outline reflects the renamed
    // symbol and drops the stale one (docstring/flag dumps are no longer emitted).
    let assert_2 = run_cli(&["codemap", "--path", "src/lib.rs"], temp.path());
    assert_2
        .success()
        .stdout(predicates::str::contains("updated"))
        .stdout(predicates::str::contains("original").not());
}

#[tokio::test]
async fn test_cross_indexing_mcp_realtime() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Search via MCP first
    let res_1 = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "find_me" }
            }),
        )
        .await
        .unwrap();
    assert!(res_1["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("lib.rs"));

    // Modify file
    fs::write(
        temp.path().join("src/lib.rs"),
        "pub fn find_something_else() {}",
    )
    .unwrap();

    // Search the new query; poll until the background refresh reflects the edit.
    let res_2 = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "find_something_else" }),
            |t| t.contains("lib.rs"),
        )
        .await
        .unwrap();
    assert!(res_2["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("lib.rs"));
}

#[tokio::test]
async fn test_cross_mcp_codemap_reflects_modify() {
    // A content edit must be reflected by the background indexer: the modified file is
    // re-parsed and the published codemap snapshot shows the new symbol while the stale one
    // is gone. (Sub-second mtime resolution ensures a same-second edit still reindexes.)
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn before_symbol() {}"),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let res_1 = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "src/lib.rs" }
            }),
        )
        .await
        .unwrap();
    assert!(res_1["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("before_symbol"));

    // Same-second edit (no sleep) — exercises the sub-second fingerprint path.
    fs::write(temp.path().join("src/lib.rs"), "pub fn after_symbol() {}").unwrap();

    let res_2 = client
        .send_tool_until(
            "overview",
            serde_json::json!({ "path": "src/lib.rs" }),
            |t| t.contains("after_symbol") && !t.contains("before_symbol"),
        )
        .await
        .unwrap();
    let text = res_2["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("after_symbol"),
        "codemap must reflect modify: {text:?}"
    );
    assert!(
        !text.contains("before_symbol"),
        "stale symbol leaked from codemap: {text:?}"
    );
}

#[tokio::test]
async fn test_cross_mcp_overview_accepts_slash_and_backslash_file_paths() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn overview_path_symbol() {}"),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let forward = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "src/lib.rs" }
            }),
        )
        .await
        .unwrap();
    let forward_text = forward["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        forward_text.contains("overview_path_symbol"),
        "forward-slash overview should show file details: {forward_text:?}"
    );

    let backslash = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "src\\lib.rs" }
            }),
        )
        .await
        .unwrap();
    let backslash_text = backslash["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        backslash_text.contains("overview_path_symbol"),
        "backslash overview should show file details: {backslash_text:?}"
    );
}

#[tokio::test]
async fn test_cross_mcp_search_read_suggestion_path_is_readable() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn searchable_path_symbol() {}\n"),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let search = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "searchable_path_symbol" }),
            |t| t.contains("read src/lib.rs:"),
        )
        .await
        .unwrap();
    let search_text = search["result"]["content"][0]["text"].as_str().unwrap();
    let suggestion_path = search_text
        .lines()
        .find_map(|line| {
            line.strip_prefix("- read ")
                .and_then(|rest| rest.split_once(':').map(|(path, _)| path))
        })
        .expect("search should render a read suggestion");
    assert_eq!(suggestion_path, "src/lib.rs");

    let read = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "read",
                "arguments": { "file_path": suggestion_path }
            }),
        )
        .await
        .unwrap();
    let read_text = read["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        read_text.contains("searchable_path_symbol"),
        "search read suggestion path should be readable: {read_text:?}"
    );
}

#[tokio::test]
async fn test_cross_mcp_search_and_overview_consume_priority_format_results() {
    let temp = create_mock_repo(&[
        ("config.json", r#"{"services":[{"port":8080}]}"#),
        ("settings.toml", "root = { nested = { leaf = 1 } }\n"),
        ("config.yaml", "server:\n  port: priority_format_needle\n"),
        ("page.html", r#"<main class="shell responsive"></main>"#),
        ("page.xml", r#"<root><child id="leaf"/></root>"#),
        ("site.css", "a[href]:hover::before { --gap: 1rem; }\n"),
        ("site.scss", "@mixin surface($color) { color: $color; }\n"),
        ("site.less", ".surface(@color) { color: @color; }\n"),
        ("deploy.sh", "function deploy { :; }\nREGION=kr\n"),
        ("deploy.zsh", "function prepare { :; }\nREGION=kr\n"),
        (
            "main.tf",
            "resource \"aws_s3_bucket\" \"assets\" { lifecycle { prevent_destroy = true } }\n",
        ),
        ("Dockerfile", "ARG VERSION\nFROM rust:${VERSION} AS build\n"),
        (
            "api.proto",
            "message User { oneof identity { string email = 1; } }\n",
        ),
        (
            "schema.graphql",
            "query PriorityOperation { service { id } }\nextend type Query { other: String }\n",
        ),
        ("Makefile", "all package: compile\n"),
        ("CMakeLists.txt", "add_test(NAME unit COMMAND app)\n"),
        (
            "BUILD",
            "first, second = (1, 2)\ncc_library(name = \"core\")\n",
        ),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let search = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "priority_format_needle" }),
            |text| text.contains("config.yaml"),
        )
        .await
        .unwrap();
    assert!(search["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("config.yaml"));
    let overview = client
        .send_tool_until(
            "overview",
            serde_json::json!({ "path": "schema.graphql" }),
            |text| text.contains("PriorityOperation"),
        )
        .await
        .unwrap();
    assert!(overview["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("PriorityOperation"));

    for (path, symbol) in [
        ("config.json", "services.port"),
        ("settings.toml", "root.nested.leaf"),
        ("config.yaml", "server.port"),
        ("page.html", "responsive"),
        ("page.xml", "leaf"),
        ("site.css", "a[href]:hover::before"),
        ("site.scss", "surface"),
        ("site.less", ".surface"),
        ("deploy.sh", "deploy"),
        ("deploy.zsh", "prepare"),
        ("main.tf", "prevent_destroy"),
        ("Dockerfile", "build"),
        ("api.proto", "identity"),
        ("schema.graphql", "other"),
        ("Makefile", "package"),
        ("CMakeLists.txt", "unit"),
        ("BUILD", "second"),
    ] {
        let result = client
            .send_tool_until("overview", serde_json::json!({ "path": path }), |text| {
                text.contains(symbol)
            })
            .await
            .unwrap();
        let text = result["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains(symbol),
            "overview omitted {symbol} from {path}: {text:?}"
        );
    }
}

#[test]
fn test_cross_benchmark_indexing() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn query() {}"),
        (
            "queries.json",
            r#"[{"query": "query", "expected": ["src/lib.rs"]}]"#,
        ),
    ])
    .unwrap();

    // 1. Run benchmark
    let assert_1 = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert_1.success();

    // 2. Clear index or force rebuilding index
    let index_dir = temp.path().join(".codemap/index");
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
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n",
        ),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // 1. Get folder level codemap via MCP
    let res_1 = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "src/nested" }
            }),
        )
        .await
        .unwrap();
    assert!(res_1["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("file.rs"));

    // 2. Add files inside folder
    fs::write(
        temp.path().join("src/nested/other.rs"),
        "pub fn new_symbol() {}",
    )
    .unwrap();

    // 3. Get folder level codemap again; poll until the added file is reflected.
    let res_2 = client
        .send_tool_until(
            "overview",
            serde_json::json!({ "path": "src/nested" }),
            |t| t.contains("other.rs"),
        )
        .await
        .unwrap();
    let text = res_2["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("other.rs"));
}
