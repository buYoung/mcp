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
async fn test_mcp_branching_hybrid_view() {
    // 6 matches (> configured threshold 5) => hybrid: the top 5 files render full detail
    // with source snippets; the 6th lands in the ranked tail as a one-liner without source.
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "[search]\nresult_threshold = 5\n"),
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
    assert_eq!(
        text.matches("\n### results\n").count(),
        5,
        "top-threshold files get the detail view: {text:?}"
    );
    assert!(
        text.contains("fn match_func"),
        "detail sections include source snippets: {text:?}"
    );
    assert!(
        text.contains("Other matches — 1 more files"),
        "the overflow match lands in the ranked tail: {text:?}"
    );
    assert!(
        text.contains("match_func [L1-1]"),
        "tail line carries the matched symbol with its range: {text:?}"
    );
}

#[tokio::test]
async fn test_mcp_fallback_match_no_snippets_and_clean_tail() {
    // 6 files match the query via their shared path segment ("widgets"), NOT by symbol
    // name. Hybrid view: the top 5 render as fallback detail — symbol names + ranges but
    // never source snippets — and the ranked-tail line must stay bare (no unrelated
    // symbols leaked, same all-terms criterion as the index symbol filter).
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "[search]\nresult_threshold = 5\n"),
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
        "matched files listed: {text:?}"
    );
    assert!(
        !text.contains("pub fn"),
        "fallback detail must not include source snippets: {text:?}"
    );
    let tail = text
        .split("## Other matches")
        .nth(1)
        .expect("ranked tail present for the overflow match");
    for sym in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"] {
        assert!(
            !tail.contains(sym),
            "ranked tail leaked fallback symbol '{sym}': {tail:?}"
        );
    }
}

#[tokio::test]
async fn test_index_and_codemap_exclude_junk_dirs() {
    // Detected JS/Rust projects exclude node_modules/target from the BM25 index and
    // codemap, even when they hold real source extensions. A shared symbol name proves
    // search returns only the in-tree file, not the junk-dir copies. The exact
    // "src/foo.rs" match also confirms the walker swap kept the rel_path byte-identical.
    let temp = create_mock_repo(&[
        ("package.json", "{}"),
        ("Cargo.toml", ""),
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
        codemap_text.contains("- src ("),
        "codemap should surface the in-tree src directory: {codemap_text:?}"
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
async fn test_index_excludes_minified_and_generated_bundles_case_insensitively() {
    let temp = create_mock_repo(&[
        ("src/keep.js", "function shared_asset_symbol() {}"),
        (
            "src/generated.MIN.js",
            "function excluded_minified_symbol() {}",
        ),
        (
            "src/generated.bundle.js",
            "function excluded_bundle_symbol() {}",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let kept = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search", "arguments": { "query": "shared_asset_symbol" }
            }),
        )
        .await
        .unwrap();
    let kept_text = kept["result"]["content"][0]["text"].as_str().unwrap();
    assert!(kept_text.contains("src/keep.js"), "{kept_text:?}");

    for (query, excluded_path) in [
        ("excluded_minified_symbol", "generated.MIN.js"),
        ("excluded_bundle_symbol", "generated.bundle.js"),
    ] {
        let response = client
            .send_request(
                "tools/call",
                serde_json::json!({ "name": "search", "arguments": { "query": query } }),
            )
            .await
            .unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            !text.contains(excluded_path),
            "explicitly excluded file leaked into the index: {text:?}"
        );
    }
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

// --- Opt-in caller/callee context (v1) e2e ---------------------------------------------

/// The `search` inputSchema advertises the optional `caller_context` boolean and does not
/// require it.
#[tokio::test]
async fn test_caller_context_schema_is_optional() {
    let temp = create_mock_repo(&[("src/a.rs", "pub fn x() {}")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let response = client
        .send_request("tools/list", serde_json::json!({}))
        .await
        .unwrap();
    let tools = response["result"]["tools"].as_array().unwrap();
    let search = tools
        .iter()
        .find(|t| t["name"] == "search")
        .expect("search tool present");
    let props = &search["inputSchema"]["properties"];
    assert!(
        props.get("caller_context").is_some(),
        "caller_context advertised: {props:?}"
    );
    let required = search["inputSchema"]["required"].as_array().unwrap();
    assert!(
        required.iter().all(|r| r != "caller_context"),
        "caller_context must be optional"
    );
}

#[tokio::test]
async fn test_search_hint_schema_is_optional() {
    let temp = create_mock_repo(&[("src/a.rs", "pub fn x() {}")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let response = client
        .send_request("tools/list", serde_json::json!({}))
        .await
        .unwrap();
    let tools = response["result"]["tools"].as_array().unwrap();
    let search = tools
        .iter()
        .find(|tool| tool["name"] == "search")
        .expect("search tool present");
    let props = &search["inputSchema"]["properties"];
    assert!(
        props.get("language_hint").is_some(),
        "language_hint advertised: {props:?}"
    );
    assert!(
        props.get("extension_hint").is_some(),
        "extension_hint advertised: {props:?}"
    );
    let required = search["inputSchema"]["required"].as_array().unwrap();
    assert!(
        required.iter().all(|field| field != "language_hint"),
        "language_hint must be optional"
    );
    assert!(
        required.iter().all(|field| field != "extension_hint"),
        "extension_hint must be optional"
    );
}

#[tokio::test]
async fn test_search_hint_is_applied_by_mcp() {
    let temp = create_mock_repo(&[
        (
            "src/policy.rs",
            "/// policy policy policy policy\npub fn policy() {}\npub fn policy_helper() { policy(); }\n",
        ),
        ("src/policy.ts", "export function policy() { return true; }\n"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let no_hint = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "policy" }
            }),
        )
        .await
        .unwrap();
    let no_hint_text = no_hint["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        no_hint_text.find("## 1. src/policy.rs").unwrap()
            < no_hint_text.find("## 2. src/policy.ts").unwrap(),
        "baseline fixture should put the Rust file first: {no_hint_text:?}"
    );

    let hinted = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": {
                    "query": "policy",
                    "language_hint": "typescript",
                    "extension_hint": "ts"
                }
            }),
        )
        .await
        .unwrap();
    let hinted_text = hinted["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        hinted_text.find("## 1. src/policy.ts").unwrap()
            < hinted_text.find("## 2. src/policy.rs").unwrap(),
        "MCP search should pass language/extension hints into ranking: {hinted_text:?}"
    );
}

/// The built-in default is ON: an omitted `caller_context` parameter annotates the detail
/// view without any repo config.
#[tokio::test]
async fn test_caller_context_default_on_annotates_when_omitted() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        "pub fn callee() {}\npub fn target() {\n    callee();\n}\n",
    )])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_tool_until("search", serde_json::json!({ "query": "target" }), |t| {
            t.contains("approximate") && !t.contains("warming up")
        })
        .await
        .unwrap();
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("approximate"),
        "default-on annotates with the parameter omitted: {text:?}"
    );
}

/// With the repo config flipping the default OFF: omitted vs explicitly false detail output
/// is byte-identical — and neither carries any caller annotation.
#[tokio::test]
async fn test_caller_context_repo_off_is_byte_identical() {
    let temp = create_mock_repo(&[
        (
            "src/lib.rs",
            "pub fn callee() {}\npub fn target() {\n    callee();\n}\n",
        ),
        // Repo config flips the default OFF (built-in default is on).
        (".codemap/config.toml", "caller_context_default = false\n"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let omitted = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "target", "include_seen": true }),
            |t| t.contains("target") && !t.contains("warming up"),
        )
        .await
        .unwrap();
    let explicit_false = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "target", "caller_context": false, "include_seen": true }),
            |t| t.contains("target") && !t.contains("warming up"),
        )
        .await
        .unwrap();

    let omitted_text = omitted["result"]["content"][0]["text"].as_str().unwrap();
    let false_text = explicit_false["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(
        omitted_text, false_text,
        "omitted and explicit-false must be byte-identical under repo-off"
    );
    assert!(
        !omitted_text.contains("approximate"),
        "no annotation when off: {omitted_text:?}"
    );
}

/// Flag on: a `fn` with an in-repo caller renders the enclosing caller symbol + file:line,
/// marked approximate; its callee (depth 1) is rendered; the qualified owner name appears
/// for a Rust impl method.
#[tokio::test]
async fn test_caller_context_on_renders_callers_callees_qualified() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        // free `helper`; `Engine::run` calls it; `run` also calls free `helper`.
        "pub fn helper() {}\nstruct Engine;\nimpl Engine {\n    pub fn run(&self) {\n        helper();\n    }\n}\n",
    )])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "helper", "caller_context": true }),
            |t| t.contains("Engine::run") || t.contains("approximate"),
        )
        .await
        .unwrap();
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("approximate"), "approximate label: {text}");
    assert!(
        text.contains("Engine::run"),
        "caller rendered with owner-qualified name + the call site: {text}"
    );
    assert!(text.contains("src/lib.rs:5"), "caller file:line: {text}");
}

/// Flag on with a broad match (> threshold): the hybrid view renders the top files in
/// detail and the rest as a ranked tail; fallback (path-matched) files are never annotated,
/// so no caller annotation appears anywhere in this all-fallback result.
#[tokio::test]
async fn test_caller_context_broad_match_hybrid_tail() {
    // 6 files share the path token "widgets" (> configured result_threshold 5).
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "[search]\nresult_threshold = 5\n"),
        ("widgets/w1.rs", "pub fn a1() {}"),
        ("widgets/w2.rs", "pub fn a2() {}"),
        ("widgets/w3.rs", "pub fn a3() {}"),
        ("widgets/w4.rs", "pub fn a4() {}"),
        ("widgets/w5.rs", "pub fn a5() {}"),
        ("widgets/w6.rs", "pub fn a6() {}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let response = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "widgets", "caller_context": true }),
            |t| t.contains("Other matches"),
        )
        .await
        .unwrap();
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(
        text.matches("\n### results\n").count(),
        5,
        "top-threshold files render detail sections: {text}"
    );
    assert!(
        text.contains("Other matches — 1 more files"),
        "overflow match lands in the ranked tail: {text}"
    );
    assert!(
        !text.contains("approximate"),
        "fallback (path-matched) files are never call-annotated: {text}"
    );
}

/// With the repo-level default key ON, an explicit `caller_context=false` overrides it and
/// is byte-identical to the pre-change build (no annotation). The repo-on default only
/// applies when the parameter is omitted (then annotations appear — not a byte-identical
/// case by design).
#[tokio::test]
async fn test_caller_context_explicit_false_overrides_repo_default_on() {
    let temp = create_mock_repo(&[
        (
            "src/lib.rs",
            "pub fn callee() {}\npub fn target() {\n    callee();\n}\n",
        ),
        // Repo config flips the default ON.
        (".codemap/config.toml", "caller_context_default = true\n"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    // Omitted → repo default ON → annotated.
    let omitted = client
        .send_tool_until("search", serde_json::json!({ "query": "target" }), |t| {
            t.contains("approximate") && !t.contains("warming up")
        })
        .await
        .unwrap();
    let omitted_text = omitted["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        omitted_text.contains("approximate"),
        "repo-on default annotates when the parameter is omitted: {omitted_text:?}"
    );

    // Explicit false → overrides repo-on → no annotation.
    let explicit_false = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "target", "caller_context": false }),
            |t| t.contains("target") && !t.contains("warming up"),
        )
        .await
        .unwrap();
    let false_text = explicit_false["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        !false_text.contains("approximate"),
        "explicit false overrides repo-on default — no annotation: {false_text:?}"
    );
}

/// Flag on, a `fn` with no discoverable callers: the observation-scope caveat appears,
/// never a bare "0 callers".
#[tokio::test]
async fn test_caller_context_zero_callers_caveat() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn lonely_fn() {}\n")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let response = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "lonely_fn", "caller_context": true }),
            |t| t.contains("no direct caller observed"),
        )
        .await
        .unwrap();
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("no direct caller observed"),
        "observation-scope caveat: {text}"
    );
    assert!(
        !text.contains("0 callers"),
        "never a bare 0 callers: {text}"
    );
}

/// Flag on with a small `search_detail_byte_cap`: a `fn` with many callers yields a large
/// annotation. The matched snippet alone fits under the cap, but snippet + annotation would
/// overflow it. The annotation must be dropped rather than pushed past the cap, so the detail
/// text stays within `search_detail_byte_cap` — the brief's hard "never exceeds the cap" /
/// "truncated, not over-budget" criterion. Regression guard: the annotation sub-budget is
/// reserved up front against the pre-snippet cap headroom, so without a live re-check at the
/// note-attach point the note overflowed the cap by up to the sub-budget.
#[tokio::test]
async fn test_caller_context_annotation_respects_byte_cap() {
    const CAP: usize = 200;
    let temp = create_mock_repo(&[
        (
            "src/lib.rs",
            "pub fn target() {}\n\
             pub fn user_a() { target(); }\n\
             pub fn user_b() { target(); }\n\
             pub fn user_c() { target(); }\n\
             pub fn user_d() { target(); }\n\
             pub fn user_e() { target(); }\n",
        ),
        // Small total detail budget: snippet fits, snippet + caller annotation would not.
        (".codemap/config.toml", "search_detail_byte_cap = 200\n"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let response = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "target", "caller_context": true }),
            |t| t.contains("target") && !t.contains("warming up"),
        )
        .await
        .unwrap();
    let text = response["result"]["content"][0]["text"].as_str().unwrap();

    // The matched `target` snippet renders (it fits under the cap)...
    assert!(text.contains("target"), "snippet rendered: {text}");
    // ...but the full detail output never exceeds the configured cap — the large caller
    // annotation is dropped rather than overflowing it.
    assert!(
        text.len() <= CAP,
        "detail text ({} bytes) must stay within search_detail_byte_cap ({CAP}): {text:?}",
        text.len()
    );
}

// --- Jev stages: config-gated, intent-driven, offline through an injected evaluator ---------

mod jev_stages {
    use crate::e2e::helpers::{create_mock_repo, response_text, with_in_process_server, McpClient};
    use codemap_search::jev::mock::{answers, Gate, MockEvaluator};
    use codemap_search::jev::{EvaluationRequest, JevError};
    use serde_json::json;
    use std::sync::Arc;

    const TASK: &str = "how is the output byte budget reserved before rendering?";

    const BUDGET_TS: &str =
        "export function keepMe(remainingBytes: number, footer: string, limit: number): number {
  const reserve = footer.length + 8;
  if (remainingBytes <= reserve) {
    return 0;
  }
  const allowed = Math.min(limit, remainingBytes - reserve);
  return allowed;
}

export function dropMe(input: string): string {
  const parts = input.split(\",\");
  const trimmed = parts.map((part) => part.trim());
  const joined = trimmed.join(\"|\");
  return joined.toUpperCase();
}
";

    fn search_filter_config(threshold: &str) -> String {
        format!("[analysis.jev]\nsearch_filter_enabled = true\nsearch_filter_min_unrelated_probability = {threshold}\n")
    }

    fn search_arguments() -> serde_json::Value {
        json!({ "query": "keepMe dropMe" })
    }

    /// Answers every body question from the declaration name; unknown names are related.
    fn body_judge(unrelated_by_name: &'static [(&'static str, f64)]) -> MockEvaluator {
        MockEvaluator::new(move |request: &EvaluationRequest| {
            Ok(request
                .questions()
                .iter()
                .map(|question| {
                    let name = question.instructions()["candidate"]["name"]
                        .as_str()
                        .unwrap_or("");
                    let unrelated = unrelated_by_name
                        .iter()
                        .find(|(candidate, _)| *candidate == name)
                        .map_or(0.0, |(_, probability)| *probability);
                    (question.id().clone(), answers::noul(1.0 - unrelated))
                })
                .collect())
        })
    }

    #[tokio::test]
    async fn test_jev_search_filter_uses_the_captured_threshold_for_an_in_flight_call() {
        let temp = create_mock_repo(&[
            (".codemap/config.toml", &search_filter_config("0.70")),
            ("src/budget.ts", BUDGET_TS),
        ])
        .unwrap();
        let cwd = temp.path().to_path_buf();
        let gate = Gate::new();
        let evaluator = Arc::new(
            body_judge(&[("dropMe", 0.80), ("keepMe", 0.10)]).with_gate(Arc::clone(&gate)),
        );
        let judge = Arc::clone(&evaluator);
        with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
            let missing = client.call("tools/call", json!({"name":"search","arguments":search_arguments()})).await.unwrap();
            assert_eq!(missing["error"]["code"], -32602, "missing registration is not an opt-out");
            let legacy = client.call("tools/call", json!({"name":"initial_instructions","arguments":{"task_query":TASK}})).await.unwrap();
            assert_eq!(legacy["error"]["code"], -32602, "text-only registration is rejected");
            client.register_task(TASK).await;
            let invalid = client.call("tools/call", json!({"name":"initial_instructions","arguments":{"task_query":TASK,"questions":[]}})).await.unwrap();
            assert_eq!(invalid["error"]["code"], -32602);
            let stale = client.call("tools/call", json!({"name":"search","arguments":search_arguments()})).await.unwrap();
            assert_eq!(stale["error"]["code"], -32602, "invalid replacement clears the previous registration");
            client.register_task(TASK).await;
            let response = client.plain_call("search", search_arguments()).await.unwrap();
            let plain = response_text(&response).to_string();
            assert!(!plain.contains("Jev"), "a bypass adds no inline text: {plain}");
            assert_eq!(judge.request_count(), 0, "the disabled baseline never evaluates");

            // The tools list reflects the enabled stage, including the external-access hint.
            let tools = client.call("tools/list", json!({})).await.unwrap();
            let tool = |name: &str| {
                tools["result"]["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|tool| tool["name"] == name)
                    .unwrap()
                    .clone()
            };
            let initial = tool("initial_instructions");
            assert_eq!(initial["inputSchema"]["required"], json!(["task_query","questions"]));
            assert_eq!(initial["inputSchema"]["properties"]["match"]["default"], "all");
            let search_tool = tool("search");
            assert!(initial["description"].as_str().unwrap().contains("Jev filters complete bodies"));
            assert!(search_tool["inputSchema"]["properties"].get("task_query").is_none());
            assert_eq!(search_tool["annotations"]["openWorldHint"], json!(true));
            assert_eq!(search_tool["annotations"]["readOnlyHint"], json!(true));
            let overview_tool = tool("overview");
            assert_eq!(overview_tool["annotations"]["openWorldHint"], json!(false), "the overview stage is off");
            assert!(!overview_tool["description"].as_str().unwrap().contains("is enabled"));

            // In flight: the threshold captured at 0.70 survives a config change to 0.90.
            client
                .send("tools/call", json!({ "name": "search", "arguments": search_arguments() }))
                .await
                .unwrap();
            gate.entered().await;
            std::fs::write(cwd.join(".codemap/config.toml"), search_filter_config("0.90")).unwrap();
            codemap_search::config::reload(&cwd);
            gate.release();
            let response = client.receive().await.unwrap();
            let text = response_text(&response);
            assert!(
                text.contains("- _omitted body: L10-15 (fn dropMe) did not match the task questions; read src/budget.ts offset 10 limit 6 to inspect._"),
                "{text}"
            );
            assert!(!text.contains("input.split"), "the omitted body is gone: {text}");
            assert!(text.contains("const reserve = footer.length + 8;"), "the related body stays: {text}");
            assert!(text.contains("dropMe (fn) [L10-15]"), "declaration rows stay: {text}");
            assert!(text.len() < plain.len(), "an omission only ever shrinks the output");
            assert_eq!(judge.request_count(), 1);

            // The next call reads the reloaded threshold: 0.80 is below 0.90, so nothing is
            // omitted and the output is the plain output again (all-keep adds no text).
            let response = client
                .call("tools/call", json!({ "name": "search", "arguments": search_arguments() }))
                .await
                .unwrap();
            let text = response_text(&response);
            assert_eq!(text, plain, "all-keep preserves the base output byte for byte");
            assert_eq!(judge.request_count(), 2);
            let requests = judge.requests();
            assert!(requests.iter().all(|request| request.task_query() == Some(TASK)));
            assert!(requests.iter().all(|request| request.state["search_arguments"]["query"] == json!("keepMe dropMe")));
            assert_eq!(requests[0].questions.len(), 4, "two registered Noul questions per complete body");
        })
        .await;
    }

    #[tokio::test]
    async fn test_jev_stages_are_independent_and_failures_preserve_the_base_output() {
        // Only the search filter is on: registration does not enable overview.
        let temp = create_mock_repo(&[
            (".codemap/config.toml", &search_filter_config("0.70")),
            ("src/budget.ts", BUDGET_TS),
        ])
        .unwrap();
        let evaluator = Arc::new(body_judge(&[("dropMe", 0.80)]));
        let judge = Arc::clone(&evaluator);
        with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
            let response = client
                .call_tool_until("overview", json!({ "task_query": TASK }), |text| {
                    text.contains("src/budget.ts") && !text.contains("warming up")
                })
                .await
                .unwrap();
            let text = response_text(&response);
            assert!(
                !text.contains("Jev"),
                "overview stays plain while only the filter is enabled: {text}"
            );
            assert_eq!(judge.request_count(), 0);
            client.register_task(TASK).await;
            let response = client
                .call_tool_until("search", search_arguments(), |text| {
                    text.contains("_omitted body:")
                })
                .await
                .unwrap();
            assert!(
                response_text(&response).contains("(fn dropMe) did not match the task questions")
            );
            assert_eq!(judge.request_count(), 1);
        })
        .await;

        // A retired overview flag never enables any evaluator.
        let temp = create_mock_repo(&[
            (
                ".codemap/config.toml",
                "[analysis.jev]\noverview_enabled = true\n",
            ),
            ("src/budget.ts", BUDGET_TS),
        ])
        .unwrap();
        let evaluator = Arc::new(body_judge(&[("dropMe", 0.80)]));
        let judge = Arc::clone(&evaluator);
        with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
            let response = client
                .call_tool_until("search", search_arguments(), |text| {
                    text.contains("function dropMe")
                })
                .await
                .unwrap();
            let text = response_text(&response);
            assert!(!text.contains("Jev body filter"), "{text}");
            assert!(text.contains("input.split"), "{text}");
            assert_eq!(judge.request_count(), 0);
        })
        .await;

        // A provider failure keeps every body and the whole base overview.
        let temp = create_mock_repo(&[
            (
                ".codemap/config.toml",
                "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\n",
            ),
            ("src/budget.ts", BUDGET_TS),
        ])
        .unwrap();
        let evaluator = Arc::new(MockEvaluator::failing(JevError::RateLimited {
            status: 429,
        }));
        let judge = Arc::clone(&evaluator);
        with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
            client.register_task(TASK).await;
            let plain = client
                .plain_call("search", search_arguments())
                .await
                .unwrap();
            let response = client
                .call(
                    "tools/call",
                    json!({ "name": "search", "arguments": search_arguments() }),
                )
                .await
                .unwrap();
            assert_eq!(
                response_text(&response),
                response_text(&plain),
                "a provider failure returns the plain search byte for byte"
            );
            assert_eq!(judge.request_count(), 1, "the failed request was attempted");
            let plain = client.plain_call("overview", json!({})).await.unwrap();
            let response = client
                .call(
                    "tools/call",
                    json!({ "name": "overview", "arguments": { "task_query": TASK } }),
                )
                .await
                .unwrap();
            assert_eq!(
                response_text(&response),
                response_text(&plain),
                "a provider failure returns the base overview byte for byte"
            );
            assert_eq!(judge.request_count(), 1, "overview does not contact Jev");
        })
        .await;
    }

    #[tokio::test]
    async fn test_jev_missing_credentials_bypass_without_any_request() {
        let temp = create_mock_repo(&[
            (
                ".codemap/config.toml",
                "[analysis.jev]\noverview_enabled = true\nsearch_filter_enabled = true\napi_key_env = \"CODEMAP_TEST_JEV_KEY_UNSET\"\n",
            ),
            ("src/budget.ts", BUDGET_TS),
        ])
        .unwrap();
        std::env::remove_var("CODEMAP_TEST_JEV_KEY_UNSET");
        with_in_process_server(temp.path(), None, |mut client| async move {
            client.register_task(TASK).await;
            let plain = client
                .plain_call("search", search_arguments())
                .await
                .unwrap();
            let response = client
                .call(
                    "tools/call",
                    json!({ "name": "search", "arguments": search_arguments() }),
                )
                .await
                .unwrap();
            let text = response_text(&response);
            assert_eq!(
                text,
                response_text(&plain),
                "missing credentials return the plain output: {text}"
            );
            assert!(text.contains("input.split"), "{text}");
            let plain = client.plain_call("overview", json!({})).await.unwrap();
            let response = client
                .call(
                    "tools/call",
                    json!({ "name": "overview", "arguments": { "task_query": TASK } }),
                )
                .await
                .unwrap();
            let text = response_text(&response);
            assert_eq!(
                text,
                response_text(&plain),
                "missing credentials return the base overview: {text}"
            );
            assert!(!text.contains("Jev"), "{text}");
        })
        .await;
    }

    #[tokio::test]
    async fn test_jev_task_query_is_validated_and_ignored_while_disabled() {
        let temp = create_mock_repo(&[("src/budget.ts", BUDGET_TS)]).unwrap();
        let mut client = McpClient::spawn(temp.path()).await.unwrap();

        let response = client
            .send_request(
                "tools/call",
                json!({ "name": "search", "arguments": { "query": "keepMe", "task_query": 5 } }),
            )
            .await
            .unwrap();
        assert_eq!(response["error"]["code"], json!(-32602), "{response}");
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("task_query"));
        let response = client
            .send_request(
                "tools/call",
                json!({ "name": "overview", "arguments": { "task_query": ["x"] } }),
            )
            .await
            .unwrap();
        assert!(
            response["error"].is_null(),
            "obsolete overview arguments are ignored: {response}"
        );

        let response = client
            .send_tool_until("search", search_arguments(), |text| {
                text.contains("function keepMe")
            })
            .await
            .unwrap();
        let text = response_text(&response);
        assert!(
            !text.contains("Jev"),
            "disabled stages ignore task_query: {text}"
        );
        let response = client
            .send_request(
                "tools/call",
                json!({ "name": "overview", "arguments": { "task_query": TASK } }),
            )
            .await
            .unwrap();
        assert!(!response_text(&response).contains("Jev"));

        let tools = client.send_request("tools/list", json!({})).await.unwrap();
        for name in ["overview", "search"] {
            let tool = tools["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|tool| tool["name"] == name)
                .unwrap();
            assert!(tool["inputSchema"]["properties"]
                .get("task_query")
                .is_none());
            assert!(!tool["description"].as_str().unwrap().contains("is enabled"));
            assert_eq!(
                tool["annotations"]["openWorldHint"],
                json!(false),
                "{name} is local-only while its stage is off"
            );
        }
    }
}
