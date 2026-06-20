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
    // 6 matches (> threshold 5) => hybrid: the top 5 files render full detail (with source
    // snippets), the 6th lands in the ranked tail as a one-liner without source.
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
    assert_eq!(
        text.matches("### File:").count(),
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
        .send_tool_until("search", serde_json::json!({ "query": "target" }), |t| {
            t.contains("target") && !t.contains("warming up")
        })
        .await
        .unwrap();
    let explicit_false = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "target", "caller_context": false }),
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
    // 6 files share the path token "widgets" (> result_threshold default 5).
    let temp = create_mock_repo(&[
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
        text.matches("### File:").count(),
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
