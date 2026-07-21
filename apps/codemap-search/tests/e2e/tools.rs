//! E2E coverage for the read / find / grep MCP tools (Child 02). Exercised through
//! the real stdio JSON-RPC server so the tool registration, argument parsing, path
//! containment, ignore semantics, and output contracts are all verified end to end.

use crate::e2e::helpers::{create_mock_repo, McpClient};
use serde_json::Value;

fn call(client_id_name: &str, args: Value) -> Value {
    serde_json::json!({ "name": client_id_name, "arguments": args })
}

fn text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn is_error(resp: &Value) -> bool {
    resp.get("error").is_some()
}

fn sample_repo() -> tempfile::TempDir {
    create_mock_repo(&[
        (
            "src/core.rs",
            "pub fn run_engine() {\n    let cmd = \"rm -rf tmp\";\n}\n",
        ),
        ("src/util.rs", "pub fn helper() {}\n// TODO: cleanup\n"),
        ("README.md", "# readme\nTODO in docs\n"),
        (".gitignore", "ignored/\n"),
        ("ignored/secret.rs", "pub fn ignored_fn() {}\n"),
        ("node_modules/dep.rs", "pub fn dep_fn() {}\n"),
        ("empty.rs", ""),
    ])
    .unwrap()
}

#[tokio::test]
async fn test_tools_list_includes_read_find_grep() {
    let temp = create_mock_repo(&[]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request("tools/list", serde_json::json!({}))
        .await
        .unwrap();
    let names: Vec<&str> = resp["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in ["overview", "search", "read", "find", "grep"] {
        assert!(
            names.contains(&expected),
            "tools/list missing '{expected}': {names:?}"
        );
    }
}

// ---- read ----------------------------------------------------------------

#[tokio::test]
async fn test_read_basic_arrow_format() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "src/core.rs" })),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert!(
        out.contains('\u{2192}'),
        "expected arrow line numbers: {out:?}"
    );
    assert!(out.contains("run_engine"), "expected file content: {out:?}");
    assert!(
        out.lines().next().unwrap().contains("1\u{2192}"),
        "first line should be '1\u{2192}…': {out:?}"
    );

    let backslash_resp = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "src\\core.rs" })),
        )
        .await
        .unwrap();
    assert_eq!(
        text(&backslash_resp),
        out,
        "backslash and forward-slash file paths should read the same file"
    );
}

#[tokio::test]
async fn test_read_offset_and_limit() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({ "file_path": "src/core.rs", "offset": 2, "limit": 1 }),
            ),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert_eq!(
        out.matches('\u{2192}').count(),
        1,
        "exactly one line expected: {out:?}"
    );
    assert!(
        out.contains("2\u{2192}"),
        "line number should be 2: {out:?}"
    );
    assert!(
        out.contains("rm -rf"),
        "should be the second line content: {out:?}"
    );

    // The 1-based inclusive start_line/end_line aliases must produce the same window
    // (offset = start_line, limit = end_line - start_line + 1).
    let aliased = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({ "path": "src/core.rs", "start_line": 2, "end_line": 2 }),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        text(&aliased),
        out,
        "start_line/end_line + path aliases should match offset/limit + file_path output"
    );

    // Agents idiomatically send numerics as JSON strings; string-typed start_line/end_line
    // must coerce (not silently drop and render the whole file).
    let string_typed = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({ "file_path": "src/core.rs", "start_line": "2", "end_line": "2" }),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        text(&string_typed),
        out,
        "string-typed start_line/end_line must coerce to the same window"
    );

    // The shorter 'start'/'end' aliases (also string-typed) resolve identically.
    let start_end = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({ "file_path": "src/core.rs", "start": "2", "end": "2" }),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        text(&start_end),
        out,
        "string-typed start/end aliases must coerce to the same window"
    );
}

#[tokio::test]
async fn test_read_directory_is_rejected() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "src" })),
        )
        .await
        .unwrap();
    assert!(is_error(&resp), "reading a directory must error");
}

#[tokio::test]
async fn test_read_empty_file_warns() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "empty.rs" })),
        )
        .await
        .unwrap();
    assert!(!is_error(&resp));
    assert!(
        text(&resp).contains("empty"),
        "empty file should warn: {:?}",
        text(&resp)
    );
}

#[tokio::test]
async fn test_read_path_traversal_is_rejected() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({ "file_path": "../../../etc/passwd" }),
            ),
        )
        .await
        .unwrap();
    assert!(is_error(&resp), "path escaping the workspace must error");
}

#[tokio::test]
async fn test_read_binary_by_extension_is_rejected() {
    // Binary is gated by EXTENSION only (Claude Code parity): a known-binary extension is
    // rejected outright regardless of content.
    let temp = create_mock_repo(&[("blob.bin", "anything")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "blob.bin" })),
        )
        .await
        .unwrap();
    assert!(is_error(&resp), "a known-binary extension must error");
}

#[tokio::test]
async fn test_read_non_utf8_content_decodes_lossily() {
    // Unknown extension with non-UTF-8 / NUL bytes: NO content-based hard reject anymore
    // (Claude Code parity). The file is read with lossy decoding instead of erroring; the
    // NUL hard-reject and the invalid-UTF-8 hard-reject were removed by design.
    let temp = create_mock_repo(&[("data.qqq", "text\u{0}binary")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "data.qqq" })),
        )
        .await
        .unwrap();
    assert!(
        !is_error(&resp),
        "non-UTF-8 content must decode lossily, not error"
    );
    let out = text(&resp);
    assert!(
        out.contains("text") && out.contains("binary"),
        "content surfaced: {out}"
    );
}

// ---- find ----------------------------------------------------------------

#[tokio::test]
async fn test_find_respects_gitignore_and_excludes_node_modules() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("find", serde_json::json!({ "pattern": "**/*.rs" })),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert!(out.contains("src/core.rs"), "{out:?}");
    assert!(out.contains("src/util.rs"), "{out:?}");
    assert!(
        !out.contains("ignored/secret.rs"),
        "gitignore should be respected: {out:?}"
    );
    assert!(
        !out.contains("node_modules"),
        "node_modules should be excluded: {out:?}"
    );
}

#[tokio::test]
async fn test_find_include_ignored_bypass() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "find",
                serde_json::json!({ "pattern": "**/*.rs", "include_ignored": true }),
            ),
        )
        .await
        .unwrap();
    assert!(
        text(&resp).contains("ignored/secret.rs"),
        "include_ignored should reveal ignored files"
    );
}

#[tokio::test]
async fn test_find_and_grep_exclude_generated_files_unless_explicitly_bypassed() {
    let temp = create_mock_repo(&[
        ("src/keep.js", "const exclusion_probe = 'keep';\n"),
        ("src/app.min.js", "const exclusion_probe = 'minified';\n"),
        ("src/app.bundle.js", "const exclusion_probe = 'bundle';\n"),
        ("package-lock.json", "{\"exclusion_probe\": \"lock\"}\n"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let default_find = client
        .send_request(
            "tools/call",
            call("find", serde_json::json!({ "pattern": "**/*" })),
        )
        .await
        .unwrap();
    let default_find_text = text(&default_find);
    assert!(default_find_text.contains("src/keep.js"));
    for excluded in ["app.min.js", "app.bundle.js", "package-lock.json"] {
        assert!(
            !default_find_text.contains(excluded),
            "default find leaked {excluded}: {default_find_text:?}"
        );
    }

    let bypass_find = client
        .send_request(
            "tools/call",
            call(
                "find",
                serde_json::json!({ "pattern": "**/*", "include_ignored": true }),
            ),
        )
        .await
        .unwrap();
    let bypass_find_text = text(&bypass_find);
    for excluded in ["app.min.js", "app.bundle.js", "package-lock.json"] {
        assert!(
            bypass_find_text.contains(excluded),
            "include_ignored should reveal {excluded}: {bypass_find_text:?}"
        );
    }

    let default_grep = client
        .send_request(
            "tools/call",
            call("grep", serde_json::json!({ "pattern": "exclusion_probe" })),
        )
        .await
        .unwrap();
    let default_grep_text = text(&default_grep);
    assert!(default_grep_text.contains("src/keep.js"));
    assert!(!default_grep_text.contains("app.min.js"));
    assert!(!default_grep_text.contains("app.bundle.js"));
    assert!(!default_grep_text.contains("package-lock.json"));

    let bypass_grep = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({
                    "pattern": "exclusion_probe",
                    "include_ignored": true
                }),
            ),
        )
        .await
        .unwrap();
    let bypass_grep_text = text(&bypass_grep);
    assert!(bypass_grep_text.contains("app.min.js"));
    assert!(bypass_grep_text.contains("app.bundle.js"));
    assert!(bypass_grep_text.contains("package-lock.json"));

    let direct_read = client
        .send_request(
            "tools/call",
            call("read", serde_json::json!({ "file_path": "src/app.min.js" })),
        )
        .await
        .unwrap();
    assert!(text(&direct_read).contains("exclusion_probe"));
}

#[tokio::test]
async fn test_find_accepts_windows_style_relative_pattern() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("find", serde_json::json!({ "pattern": "src\\*.rs" })),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert!(out.contains("src/core.rs"), "{out:?}");
    assert!(out.contains("src/util.rs"), "{out:?}");
}

#[tokio::test]
async fn test_find_accepts_workspace_internal_absolute_backslash_pattern() {
    let temp = sample_repo();
    let absolute_pattern = temp
        .path()
        .join("src")
        .join("*.rs")
        .to_string_lossy()
        .replace('/', "\\");
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("find", serde_json::json!({ "pattern": absolute_pattern })),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert!(out.contains("src/core.rs"), "{out:?}");
    assert!(out.contains("src/util.rs"), "{out:?}");
}

#[tokio::test]
async fn test_find_path_param_escape_is_rejected() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "find",
                serde_json::json!({ "pattern": "*.rs", "path": "../.." }),
            ),
        )
        .await
        .unwrap();
    assert!(
        is_error(&resp),
        "a path param escaping the workspace must error"
    );
}

// ---- grep ----------------------------------------------------------------

#[tokio::test]
async fn test_grep_content_is_default_mode() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call("grep", serde_json::json!({ "pattern": "TODO" })),
        )
        .await
        .unwrap();
    let out = text(&resp);
    // Default is now content mode: lines render as `file:line:text` with line numbers.
    assert!(
        out.lines()
            .any(|l| l.starts_with("src/util.rs:") && l.contains("TODO")),
        "content line format `file:line:text` expected by default: {out:?}"
    );
    assert!(
        !out.contains("ignored/secret.rs"),
        "ignored files must not be searched: {out:?}"
    );
    assert!(
        !out.contains("node_modules"),
        "node_modules must not be searched: {out:?}"
    );
}

#[tokio::test]
async fn test_grep_content_mode_line_format() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "run_engine", "output_mode": "content" }),
            ),
        )
        .await
        .unwrap();
    let out = text(&resp);
    // Expect `path:line:text`
    assert!(
        out.lines()
            .any(|l| l.starts_with("src/core.rs:") && l.contains("run_engine")),
        "content line format `file:line:text` expected: {out:?}"
    );
}

#[tokio::test]
async fn test_grep_count_mode() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "TODO", "output_mode": "count" }),
            ),
        )
        .await
        .unwrap();
    assert!(
        text(&resp).contains("total occurrence"),
        "{:?}",
        text(&resp)
    );
}

#[tokio::test]
async fn test_grep_case_insensitive() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "todo", "-i": true, "output_mode": "count" }),
            ),
        )
        .await
        .unwrap();
    assert!(
        text(&resp).contains("total occurrence"),
        "case-insensitive should match TODO: {:?}",
        text(&resp)
    );
}

#[tokio::test]
async fn test_grep_type_filter() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    // 'TODO' appears in README.md and src/util.rs; type=rust restricts to .rs only.
    // Use files_with_matches here so the cheap enumeration mode stays covered after the
    // default flipped to content.
    let resp = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "TODO", "type": "rust", "output_mode": "files_with_matches" }),
            ),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert!(
        out.starts_with("Found "),
        "files_with_matches header expected: {out:?}"
    );
    assert!(out.contains("src/util.rs"), "{out:?}");
    assert!(
        !out.contains("README.md"),
        "type=rust must exclude markdown: {out:?}"
    );

    // `include` is an alias for `glob` (agents send both); a `*.rs` filter must restrict the
    // same way `type=rust` did, never silently degrading to a whole-repo search.
    let via_include = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "TODO", "include": "*.rs", "output_mode": "files_with_matches" }),
            ),
        )
        .await
        .unwrap();
    let include_out = text(&via_include);
    assert!(include_out.contains("src/util.rs"), "{include_out:?}");
    assert!(
        !include_out.contains("README.md"),
        "include='*.rs' must exclude markdown: {include_out:?}"
    );
}

#[tokio::test]
async fn test_grep_pattern_starting_with_dash_is_literal() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "-rf", "output_mode": "content" }),
            ),
        )
        .await
        .unwrap();
    assert!(
        text(&resp).contains("rm -rf"),
        "a `-`-leading pattern must be searched literally: {:?}",
        text(&resp)
    );
}

#[tokio::test]
async fn test_grep_path_param_escape_is_rejected() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({ "pattern": "x", "path": "../../.." }),
            ),
        )
        .await
        .unwrap();
    assert!(
        is_error(&resp),
        "a path param escaping the workspace must error"
    );
}
