use crate::e2e::helpers::{create_mock_repo, McpClient};
use serde_json::Value;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt};

#[tokio::test]
async fn test_mcp_initialize() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client.send_request("initialize", serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    })).await.unwrap();

    assert!(response.get("result").is_some());
    assert!(response["result"].get("protocolVersion").is_some());
    assert!(response["result"].get("capabilities").is_some());
}

#[tokio::test]
async fn test_mcp_list_tools() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client.send_request("tools/list", serde_json::json!({})).await.unwrap();
    
    assert!(response.get("result").is_some());
    let tools = response["result"]["tools"].as_array().expect("tools should be an array");
    
    // Tools should contain 'search' and 'get_codemap'
    let tool_names: Vec<&str> = tools.iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"search"));
    assert!(tool_names.contains(&"get_codemap"));
}

#[tokio::test]
async fn test_mcp_branching_list_view() {
    // 5 matches => returns list view
    let temp = create_mock_repo(&[
        ("src/a.rs", "fn match_func() {}"),
        ("src/b.rs", "fn match_func() {}"),
        ("src/c.rs", "fn match_func() {}"),
        ("src/d.rs", "fn match_func() {}"),
        ("src/e.rs", "fn match_func() {}"),
    ]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": {
            "query": "match_func"
        }
    })).await.unwrap();

    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    // Branching format check
    assert!(text.contains("a.rs"));
    assert!(text.contains("e.rs"));
    assert!(!text.contains("fn match_func")); // Should be codemap list view (no actual code spans)
}

#[tokio::test]
async fn test_mcp_branching_scope_view() {
    // 2 matches (< 5) => returns enclosing tree-sitter spans
    let temp = create_mock_repo(&[
        ("src/a.rs", "fn match_func() {\n  println!(\"hello\");\n}"),
        ("src/b.rs", "fn match_func() {\n  println!(\"world\");\n}"),
    ]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": {
            "query": "match_func"
        }
    })).await.unwrap();

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
    let response = client.send_request("non_existent_method", serde_json::json!({})).await.unwrap();
    
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
    let response = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": {}
    })).await.unwrap();

    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn test_mcp_huge_payload() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Create huge argument string (>10MB)
    let large_query = "x".repeat(10 * 1024 * 1024);
    let response = client.send_request("tools/call", serde_json::json!({
        "name": "search",
        "arguments": {
            "query": large_query
        }
    })).await.unwrap();

    // Should return error or handle gracefully
    assert!(response.get("error").is_some() || response.get("result").is_some());
}

#[tokio::test]
async fn test_mcp_path_traversal() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Requesting folder path outside workspace root
    let response = client.send_request("tools/call", serde_json::json!({
        "name": "get_codemap",
        "arguments": {
            "path": "../../../etc/passwd"
        }
    })).await.unwrap();

    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn test_mcp_parallel_requests() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}")
    ]).unwrap();

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
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn test_mcp() {}")
    ]).unwrap();

    // Create a symlink directory
    let symlink_dir = tempfile::tempdir().unwrap();
    let link_path = symlink_dir.path().join("linked_workspace");
    
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(temp.path(), &link_path).is_ok() {
            let mut client = McpClient::spawn(&link_path).await.unwrap();
            let response = client.send_request("tools/call", serde_json::json!({
                "name": "get_codemap",
                "arguments": {
                    "path": "src/lib.rs"
                }
            })).await.unwrap();

            assert!(response.get("error").is_none(), "Response contained error: {:?}", response.get("error"));
            let text = response["result"]["content"][0]["text"].as_str().unwrap();
            assert!(text.contains("test_mcp"));
        }
    }
}
