use crate::e2e::helpers::{create_mock_repo, McpClient};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[tokio::test]
async fn test_mcp_initialize() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0"
                }
            }),
        )
        .await
        .unwrap();

    assert!(response.get("result").is_some());
    assert!(response["result"].get("protocolVersion").is_some());
    assert!(response["result"].get("capabilities").is_some());
}

#[tokio::test]
async fn test_mcp_list_tools() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_request("tools/list", serde_json::json!({}))
        .await
        .unwrap();

    assert!(response.get("result").is_some());
    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools should be an array");

    // Tools should contain 'search' and 'overview'
    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(tool_names.contains(&"search"));
    assert!(tool_names.contains(&"overview"));
}

#[tokio::test]
async fn test_mcp_branching_list_view() {
    // 6 matches (> threshold 5) => returns the codemap overview (no source spans)
    let temp = create_mock_repo(&[
        ("src/a.rs", "fn match_func() {}"),
        ("src/b.rs", "fn match_func() {}"),
        ("src/c.rs", "fn match_func() {}"),
        ("src/d.rs", "fn match_func() {}"),
        ("src/e.rs", "fn match_func() {}"),
        ("src/f.rs", "fn match_func() {}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": {
                    "query": "match_func"
                }
            }),
        )
        .await
        .unwrap();

    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    // Codemap overview: lists matched files + symbol names, but no raw source spans.
    assert!(text.contains("a.rs"));
    assert!(text.contains("f.rs"));
    assert!(text.contains("match_func")); // symbol name appears
    assert!(!text.contains("fn match_func")); // but not the raw source line
}

#[tokio::test]
async fn test_mcp_overview_excludes_fallback_symbols() {
    // 6 files match the query via their shared path segment ("widgets"), NOT by symbol
    // name. The detail-branch all-symbols fallback must not leak into the overview, so
    // the overview lists the files but none of their (unrelated) symbol names.
    let temp = create_mock_repo(&[
        ("widgets/w1.rs", "pub fn alpha() {}"),
        ("widgets/w2.rs", "pub fn beta() {}"),
        ("widgets/w3.rs", "pub fn gamma() {}"),
        ("widgets/w4.rs", "pub fn delta() {}"),
        ("widgets/w5.rs", "pub fn epsilon() {}"),
        ("widgets/w6.rs", "pub fn zeta() {}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "widgets" }
            }),
        )
        .await
        .unwrap();

    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("widgets/w1.rs"),
        "overview should list matched files: {text:?}"
    );
    // None of the unrelated symbol names should be dumped into the overview.
    for sym in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"] {
        assert!(
            !text.contains(sym),
            "overview leaked fallback symbol '{sym}': {text:?}"
        );
    }
}

#[tokio::test]
async fn test_index_and_codemap_exclude_junk_dirs() {
    // AC#1 (Child 04): node_modules/target/… must never enter the BM25 index or the
    // codemap, even when they hold real source extensions. A shared symbol name proves
    // search returns only the in-tree file, not the junk-dir copies. The exact
    // "src/foo.rs" match also confirms the walker swap kept the rel_path byte-identical.
    let temp = create_mock_repo(&[
        ("src/foo.rs", "pub fn shared_symbol_name() {}"),
        (
            "node_modules/pkg/index.js",
            "function shared_symbol_name() {}",
        ),
        (
            "target/debug/generated.rs",
            "pub fn shared_symbol_name() {}",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let search_res = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "shared_symbol_name" }
            }),
        )
        .await
        .unwrap();
    let search_text = search_res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        search_text.contains("src/foo.rs"),
        "search should find the in-tree file: {search_text:?}"
    );
    assert!(
        !search_text.contains("node_modules"),
        "search leaked node_modules: {search_text:?}"
    );
    assert!(
        !search_text.contains("target"),
        "search leaked target/: {search_text:?}"
    );

    let codemap_res = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "" }
            }),
        )
        .await
        .unwrap();
    let codemap_text = codemap_res["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        codemap_text.contains("src/foo.rs"),
        "codemap should list the in-tree file: {codemap_text:?}"
    );
    assert!(
        !codemap_text.contains("node_modules"),
        "codemap leaked node_modules: {codemap_text:?}"
    );
    assert!(
        !codemap_text.contains("target"),
        "codemap leaked target/: {codemap_text:?}"
    );
}

#[tokio::test]
async fn test_index_respects_codemapignore() {
    // Child 04: the indexer now honors .codemapignore (gitignore semantics), matching
    // find/grep — previously a find/grep-only behavior, the BM25 index ignored it.
    let temp = create_mock_repo(&[
        ("src/keep.rs", "pub fn unique_keep_token() {}"),
        ("src/secret.rs", "pub fn unique_secret_token() {}"),
        (".codemapignore", "src/secret.rs\n"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let kept = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search", "arguments": { "query": "unique_keep_token" }
            }),
        )
        .await
        .unwrap();
    assert!(kept["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("src/keep.rs"));

    let ignored = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search", "arguments": { "query": "unique_secret_token" }
            }),
        )
        .await
        .unwrap();
    let ignored_text = ignored["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        !ignored_text.contains("secret.rs"),
        "indexer must honor .codemapignore: {ignored_text:?}"
    );
}

#[tokio::test]
async fn test_mcp_branching_scope_view() {
    // 2 matches (< 5) => returns enclosing tree-sitter spans
    let temp = create_mock_repo(&[
        ("src/a.rs", "fn match_func() {\n  println!(\"hello\");\n}"),
        ("src/b.rs", "fn match_func() {\n  println!(\"world\");\n}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": {
                    "query": "match_func"
                }
            }),
        )
        .await
        .unwrap();

    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    // Branching format check: should contain actual code scopes/spans
    assert!(text.contains("fn match_func"));
    assert!(text.contains("println!"));
}

#[tokio::test]
async fn test_mcp_invalid_request() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Invalid JSON-RPC request format
    let response = client
        .send_request("non_existent_method", serde_json::json!({}))
        .await
        .unwrap();

    assert!(response.get("error").is_some());
    let code = response["error"]["code"].as_i64().unwrap();
    // Method not found code: -32601
    assert_eq!(code, -32601);
}

#[tokio::test]
async fn test_mcp_missing_arguments() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Missing query parameter
    let response = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": {}
            }),
        )
        .await
        .unwrap();

    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn test_mcp_huge_payload() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Create huge argument string (>10MB)
    let large_query = "x".repeat(10 * 1024 * 1024);
    let response = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": {
                    "query": large_query
                }
            }),
        )
        .await
        .unwrap();

    // Should return error or handle gracefully
    assert!(response.get("error").is_some() || response.get("result").is_some());
}

#[tokio::test]
async fn test_mcp_path_traversal() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Requesting folder path outside workspace root
    let response = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": {
                    "path": "../../../etc/passwd"
                }
            }),
        )
        .await
        .unwrap();

    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn test_mcp_parallel_requests() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn find_me() {}")]).unwrap();

    let mut client1 = McpClient::spawn(temp.path()).await.unwrap();
    let mut client2 = McpClient::spawn(temp.path()).await.unwrap();

    // Send multiplexed JSON-RPC requests
    let req1 = client1.send_request("tools/list", serde_json::json!({}));
    let req2 = client2.send_request("tools/list", serde_json::json!({}));

    let (res1, res2) = tokio::join!(req1, req2);
    assert!(res1.is_ok());
    assert!(res2.is_ok());
}

#[tokio::test]
async fn test_mcp_abrupt_disconnect() {
    let temp = create_mock_repo(&[]).unwrap();
    let client = McpClient::spawn(temp.path()).await.unwrap();

    // Kill client connection abruptly
    let res = client.kill().await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_mcp_oom_mitigation() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Write a huge sequence of bytes without a newline directly to the client's stdin.
    let payload = vec![b'x'; 11 * 1024 * 1024];

    let res = client.stdin.write_all(&payload).await;
    if res.is_ok() {
        let _ = client.stdin.flush().await;
    }

    // Attempt to read from stdout should fail (EOF) since the server exited/aborted due to length limit
    let mut line = String::new();
    let read_res = client.stdout_reader.read_line(&mut line).await;
    assert!(read_res.is_err() || line.is_empty());
}

#[tokio::test]
async fn test_mcp_symlink_workspace_compatibility() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn test_mcp() {}")]).unwrap();

    // Create a symlink directory
    let symlink_dir = tempfile::tempdir().unwrap();
    let link_path = symlink_dir.path().join("linked_workspace");

    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(temp.path(), &link_path).is_ok() {
            let mut client = McpClient::spawn(&link_path).await.unwrap();
            let response = client
                .send_request(
                    "tools/call",
                    serde_json::json!({
                        "name": "overview",
                        "arguments": {
                            "path": "src/lib.rs"
                        }
                    }),
                )
                .await
                .unwrap();

            assert!(
                response.get("error").is_none(),
                "Response contained error: {:?}",
                response.get("error")
            );
            let text = response["result"]["content"][0]["text"].as_str().unwrap();
            assert!(text.contains("test_mcp"));
        }
    }
}
