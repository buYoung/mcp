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
async fn test_live_context_includes_callee_locations_and_constant_values() {
    let source = concat!(
        "pub const CODEMAP_DIR_NAME: &str = \".codemap\";\n",
        "const CONFIG_FILE_NAME: &str = \"config.toml\";\n",
        "const TEXT_ONLY: &str = \"not referenced\";\n",
        "fn read_layer(path: &str) {}\n",
        "pub fn load() {\n",
        "    read_layer(CODEMAP_DIR_NAME);\n",
        "    read_layer(CONFIG_FILE_NAME);\n",
        "    let text = \"TEXT_ONLY\"; // TEXT_ONLY\n",
        "}\n",
    );
    let temp = create_mock_repo(&[
        ("src/lib.rs", "mod config; mod noise;\n"),
        ("src/config.rs", source),
        (
            "src/noise.rs",
            "fn string_only() { let text = \"load(\"; }\nfn raw_only() { let text = r#\"load(\"#; }\nfn comment_only() { /* load(); */ }\nfn method_only() { value.load(); }\nfn actual_caller() { crate::config::load(); }\n",
        ),
        ("src/foreign.py", "def foreign_caller():\n    json.load(data)\n"),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update = false\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let ready = client
        .send_tool_until(
            "read",
            serde_json::json!({"file_path": "src/config.rs", "offset": 5, "limit": 5}),
            |out| out.contains("load [function"),
        )
        .await
        .unwrap();
    let out = text(&ready);
    let (context, raw) = out.split_once("\n# results\n").unwrap();
    assert!(context.contains("read_layer — src/config.rs:4"), "{out}");
    assert!(context.contains("actual_caller (src/noise.rs:5)"), "{out}");
    for false_caller in [
        "string_only",
        "raw_only",
        "comment_only",
        "method_only",
        "foreign_caller",
    ] {
        assert!(
            !context.contains(false_caller),
            "false caller {false_caller}: {out}"
        );
    }
    assert!(
        context.contains("CODEMAP_DIR_NAME — src/config.rs:1 = \".codemap\""),
        "{out}"
    );
    assert!(
        context.contains("CONFIG_FILE_NAME — src/config.rs:2 = \"config.toml\""),
        "{out}"
    );
    assert!(
        !context.contains("TEXT_ONLY"),
        "comments and strings are not references: {out}"
    );
    assert_eq!(
        raw.lines().count(),
        5,
        "source window must stay unchanged: {raw}"
    );
    assert!(
        raw.contains("8→    let text = \"TEXT_ONLY\"; // TEXT_ONLY"),
        "{raw}"
    );

    let grep = client
        .send_request(
            "tools/call",
            call(
                "grep",
                serde_json::json!({
                    "path": "src/config.rs", "pattern": "read_layer\\(CODEMAP_DIR_NAME\\)"
                }),
            ),
        )
        .await
        .unwrap();
    let grep_out = text(&grep);
    let (context, raw) = grep_out.split_once("\n# results\n").unwrap();
    assert!(
        context.contains("read_layer — src/config.rs:4"),
        "{grep_out}"
    );
    assert!(
        context.contains("CODEMAP_DIR_NAME — src/config.rs:1 = \".codemap\""),
        "{grep_out}"
    );
    assert!(
        raw.contains("src/config.rs:6:    read_layer(CODEMAP_DIR_NAME);"),
        "{raw}"
    );
    std::fs::write(
        temp.path().join(".codemap/config.toml"),
        "[update]\nconfig_auto_update = false\n[caller_context]\nnavigation_context_default = true\n",
    ).unwrap();
    let precise = client
        .send_tool_until(
            "read",
            serde_json::json!({"file_path": "src/config.rs", "offset": 5, "limit": 5}),
            |out| out.contains("callers (tree-sitter precise"),
        )
        .await
        .unwrap();
    let out = text(&precise);
    assert!(out.contains("actual_caller (src/noise.rs:5)"), "{out}");
    for false_caller in [
        "string_only",
        "raw_only",
        "comment_only",
        "method_only",
        "foreign_caller",
    ] {
        assert!(
            !out.contains(false_caller),
            "precise false caller {false_caller}: {out}"
        );
    }
}

#[tokio::test]
async fn test_live_constant_context_respects_the_read_output_budget() {
    let mut source = String::new();
    for index in 0..100 {
        source.push_str(&format!("const VALUE_{index}: &str = \"value {index}\";\n"));
    }
    source.push_str("fn consume_values() {\n");
    for index in 0..100 {
        source.push_str(&format!("    consume(VALUE_{index});\n"));
    }
    source.push_str("}\n");
    let temp = create_mock_repo(&[
        ("src/values.rs", &source),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update = false\n[tool_output]\nread_output_byte_cap = 1800\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let response = client
        .send_tool_until(
            "read",
            serde_json::json!({
                "file_path": "src/values.rs", "offset": 101, "limit": 1
            }),
            |out| out.contains("consume_values [function"),
        )
        .await
        .unwrap();
    let out = text(&response);
    assert!(out.len() <= 1800, "{} bytes: {out}", out.len());
    assert!(out.contains("[Reference output cap:"), "{out}");
    assert_eq!(
        out.split_once("\n# results\n").unwrap().1.trim_end(),
        "   101→fn consume_values() {"
    );
}

#[tokio::test]
async fn test_test_context_rules_reload_and_can_disable_builtin_detection() {
    let base_config = "[update]\nconfig_auto_update = false\n[exclude]\nexcluded_directories = [\"**/tests/fixtures\"]\n";
    let temp = create_mock_repo(&[
        (".codemap/config.toml", base_config),
        (
            "src/inline.rs",
            "#[test]\nfn verify_rust() { helper(); }\nfn helper() {}\n",
        ),
        (
            "src/inline.py",
            "@pytest.mark.slow\ndef verify_python():\n    helper()\n",
        ),
        (
            "src/inline.ts",
            "test('typescript case', () => { helper(); });\n",
        ),
        (
            "src/Checks.java",
            "class Checks {\n  @Test void verify_java() { helper(); }\n}\n",
        ),
        ("tests/unit.rs", "fn verify_file() { helper(); }\n"),
        (
            "tests/fixtures/ignored.rs",
            "fn verify_excluded_fixture() { helper(); }\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let cases = [
        ("src/inline.rs", 2, "verify_rust"),
        ("src/inline.py", 2, "verify_python"),
        ("src/inline.ts", 1, "typescript case"),
        ("src/Checks.java", 2, "verify_java"),
        ("tests/unit.rs", 1, "verify_file"),
    ];
    let mut raw_results = Vec::new();
    for (phase, (settings, should_show)) in [
        ("should_include_test_code = false\n", false),
        ("should_include_test_code = true\n", true),
        ("should_include_test_code = false\ntest_file_patterns = []\ntest_attributes = { rust = [], java = [] }\ntest_decorators = { python = [] }\ntest_calls = { typescript = [] }\n", true),
    ].into_iter().enumerate() {
        std::fs::write(temp.path().join(".codemap/config.toml"), format!("{base_config}{settings}")).unwrap();
        // Poll the actual consumer, including config reload and index warm-up.
        let ready = client.send_tool_until("read", serde_json::json!({
            "file_path": "src/inline.rs", "offset": 2, "limit": 1
        }), |out| {
            let context = out.split_once("\n# results\n").map(|(context, _)| context).unwrap_or("");
            if should_show { context.contains("verify_rust [function") }
            else { context.contains("Test code excluded") && !context.contains("verify_rust") }
        }).await.unwrap();
        assert!(!is_error(&ready), "{ready}");
        if !should_show {
            assert!(text(&ready).contains("exclude.should_include_test_code=true"), "{ready}");
        }
        for (index, (path, offset, name)) in cases.iter().enumerate() {
            let response = client.send_request("tools/call", call("read", serde_json::json!({
                "file_path": path, "offset": offset, "limit": 1
            }))).await.unwrap();
            let out = text(&response);
            let (context, raw) = out.split_once("\n# results\n").unwrap();
            assert_eq!(context.contains(name), should_show, "phase {phase}, {path}: {out}");
            if phase == 0 { raw_results.push(raw.to_string()); }
            else { assert_eq!(raw, raw_results[index], "raw source changed for {path}"); }
        }
        let helper = client.send_request("tools/call", call("read", serde_json::json!({
            "file_path": "src/inline.rs", "offset": 3, "limit": 1
        }))).await.unwrap();
        let helper_out = text(&helper);
        let context = helper_out.split_once("\n# results\n").unwrap().0;
        assert_eq!(context.contains("verify_rust"), should_show, "{helper_out}");
        assert!(!context.contains("verify_excluded_fixture"), "directory exclusions must survive test inclusion: {helper_out}");
    }
}

#[tokio::test]
async fn test_read_basic_arrow_format() {
    let temp = sample_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    // Alias equality compares a stable symbol snapshot as well as the live source.
    client
        .send_request("tools/call", call("overview", serde_json::json!({})))
        .await
        .unwrap();
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
        out.starts_with("# symbols\n")
            && out
                .split_once("\n# results\n")
                .unwrap()
                .1
                .lines()
                .next()
                .unwrap()
                .contains("1\u{2192}"),
        "symbol context should precede the numbered source results: {out:?}"
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
    // Alias equality compares a stable symbol snapshot as well as the live source.
    client
        .send_request("tools/call", call("overview", serde_json::json!({})))
        .await
        .unwrap();
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
    std::fs::write(temp.path().join("package.json"), "{}").unwrap();
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
    assert!(
        !text(&resp).contains("# symbols"),
        "count must omit symbol context: {}",
        text(&resp)
    );
    client
        .send_tool_until(
            "read",
            serde_json::json!({"file_path": "src/core.rs", "offset": 1, "limit": 1}),
            |out| out.contains("run_engine [function"),
        )
        .await
        .unwrap();
    for pattern in ["run_engine", "no_such_identifier"] {
        let response = client
            .send_request(
                "tools/call",
                call(
                    "grep",
                    serde_json::json!({"pattern": pattern, "output_mode": "files_with_matches"}),
                ),
            )
            .await
            .unwrap();
        let out = text(&response);
        assert!(
            !out.contains("# symbols"),
            "file list must omit symbol context: {out}"
        );
        assert!(
            !out.contains("# results"),
            "file list must stay compact: {out}"
        );
        if pattern == "run_engine" {
            assert_eq!(out.trim(), "Found 1 file(s)\nsrc/core.rs");
        } else {
            assert!(out.starts_with("No matches found"), "{out}");
        }
    }
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
    assert_eq!(out.trim(), "Found 1 file(s)\nsrc/util.rs");
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
