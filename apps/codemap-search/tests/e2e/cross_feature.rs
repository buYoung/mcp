use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use predicates::prelude::*;
use std::fs;

#[tokio::test]
async fn test_rust_target_reload_and_reexport_identity_reach_all_tool_views() {
    let platform = "#[cfg(target_os=\"macos\")]\npub mod macos;\n#[cfg(not(target_os=\"macos\"))]\npub mod other;\n#[cfg(target_os=\"macos\")]\npub use self::macos::run;\n#[cfg(not(target_os=\"macos\"))]\npub use self::other::run;\n";
    let config = |os: &str| {
        format!("[update]\nconfig_auto_update=false\n[analysis]\ntarget_os=\"{os}\"\n[caller_context]\nnavigation_context_default=true\nnavigation_store_references=true\n")
    };
    let temp = create_mock_repo(&[
        (
            "Cargo.toml",
            "[package]\nname='target-probe'\nversion='0.1.0'\n",
        ),
        ("src/lib.rs", "mod platform; mod client;\n"),
        ("src/platform/mod.rs", platform),
        (
            "src/platform/macos.rs",
            "pub fn run() {}\npub fn alternate() {}\n",
        ),
        ("src/platform/other.rs", "pub fn run() {}\n"),
        (
            "src/client.rs",
            "use crate::platform::run as selected;\npub fn entry() { selected(); }\n",
        ),
        (".codemap/config.toml", &config("")),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args =
        serde_json::json!({"file_path":"src/client.rs","offset":2,"limit":1,"view":"relations"});
    let text = |response: &serde_json::Value| {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    let neutral = client
        .send_tool_until("read", args.clone(), |out| {
            out.contains("selected (unresolved)")
        })
        .await
        .unwrap();
    assert!(
        text(&neutral).contains("selected (unresolved)"),
        "{neutral}"
    );
    for (os, expected, absent) in [
        (
            "macos",
            "run — src/platform/macos.rs:1",
            "run — src/platform/other.rs:1",
        ),
        (
            "linux",
            "run — src/platform/other.rs:1",
            "run — src/platform/macos.rs:1",
        ),
    ] {
        fs::write(temp.path().join(".codemap/config.toml"), config(os)).unwrap();
        let response = client
            .send_tool_until("read", args.clone(), |out| out.contains(expected))
            .await
            .unwrap();
        let out = text(&response);
        assert!(
            out.contains(expected) && out.contains(&format!("analysis target_os={os}")),
            "{out}"
        );
        assert!(!out.contains(absent), "{out}");
        let grep=client.send_request("tools/call",serde_json::json!({"name":"grep","arguments":{"path":"src/client.rs","pattern":"fn entry","view":"relations"}})).await.unwrap();
        assert!(
            text(&grep).contains(expected) && !text(&grep).contains(absent),
            "{grep}"
        );
        let search=client.send_request("tools/call",serde_json::json!({"name":"search","arguments":{"query":"entry client","caller_context":true}})).await.unwrap();
        assert!(
            text(&search).contains(expected) && !text(&search).contains(absent),
            "{search}"
        );
    }
    fs::write(temp.path().join(".codemap/config.toml"), config("macos")).unwrap();
    fs::write(
        temp.path().join("src/platform/mod.rs"),
        platform.replace(
            "pub use self::macos::run;",
            "pub use self::macos::alternate as run;",
        ),
    )
    .unwrap();
    let changed = client
        .send_tool_until("read", args.clone(), |out| {
            out.contains("alternate — src/platform/macos.rs:2")
        })
        .await
        .unwrap();
    assert!(
        text(&changed).contains("alternate — src/platform/macos.rs:2"),
        "{changed}"
    );
    let reverse=client.send_tool_until("read",serde_json::json!({"file_path":"src/platform/macos.rs","offset":2,"limit":1,"view":"relations"}),|out|out.contains("entry (src/client.rs:2)")).await.unwrap();
    assert!(
        text(&reverse).contains("entry (src/client.rs:2)"),
        "{reverse}"
    );
    fs::write(temp.path().join(".codemap/config.toml"), config("")).unwrap();
    let reset = client
        .send_tool_until("read", args, |out| out.contains("selected (unresolved)"))
        .await
        .unwrap();
    assert!(
        text(&reset).contains("selected (unresolved)") && !text(&reset).contains("alternate —"),
        "{reset}"
    );
}

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
            "watch = false\nindex_staleness_ms = 1\n[search]\nresult_threshold = 5\n",
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
    assert_eq!(text_large.matches("\n### results\n").count(), 5);
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
                    && t.matches("\n### results\n").count() == 4
            },
        )
        .await
        .unwrap();

    let text_small = res_small["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text_small.contains("fn query_func"));
    assert_eq!(text_small.matches("\n### results\n").count(), 4);
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
async fn test_programming_languages_flow_through_index_search_and_mcp_overview() {
    let temp = create_mock_repo(&[
        (
            "src/rust_flow.rs",
            "pub struct RustFlow;\nimpl RustFlow {\n    pub fn target_rust(&self) {}\n    pub fn caller_rust(&self) { self.target_rust(); }\n}\n",
        ),
        (
            "src/python_flow.py",
            "class PythonFlow:\n    def target_python(self):\n        pass\n    def caller_python(self):\n        self.target_python()\n",
        ),
        (
            "src/typescript_flow.ts",
            "export class TypeScriptFlow {\n  targetTypeScript() {}\n  callerTypeScript() { this.targetTypeScript(); }\n}\n",
        ),
        (
            "src/javascript_flow.js",
            "export class JavaScriptFlow {\n  targetJavaScript() {}\n  callerJavaScript() { this.targetJavaScript(); }\n}\n",
        ),
        (
            "src/go_flow.go",
            "package flow\ntype GoFlow struct{}\nfunc (flow GoFlow) TargetGo() {}\nfunc (flow GoFlow) CallerGo() { flow.TargetGo() }\n",
        ),
        (
            "src/JavaFlow.java",
            "class JavaFlow {\n  void targetJava() {}\n  void callerJava() { targetJava(); }\n}\n",
        ),
        (
            "src/KotlinFlow.kt",
            "class KotlinFlow {\n  fun targetKotlin() {}\n  fun callerKotlin() { targetKotlin() }\n}\n",
        ),
        (
            "src/c_flow.c",
            "void target_c(void) {}\nvoid caller_c(void) { target_c(); }\n",
        ),
        (
            "src/cpp_flow.cpp",
            "class CppFlow {\npublic:\n  void targetCpp() {}\n  void callerCpp() { targetCpp(); }\n};\n",
        ),
        (
            "src/asm_flow.s",
            ".globl target_asm\n.globl caller_asm\ntarget_asm:\n  ret\ncaller_asm:\n  call target_asm\n  ret\n",
        ),
        (
            "src/CSharpFlow.cs",
            "public class CSharpFlow {\n  public void TargetCSharp() {}\n  public void CallerCSharp() { TargetCSharp(); }\n}\n",
        ),
        (
            "src/php_flow.php",
            "<?php class PhpFlow {\n  public function targetPhp(): void {}\n  public function callerPhp(): void { $this->targetPhp(); }\n}\n",
        ),
        (
            "src/ruby_flow.rb",
            "class RubyFlow\n  def target_ruby\n  end\n  def caller_ruby\n    target_ruby()\n  end\nend\n",
        ),
        (
            "src/lua_flow.lua",
            "local LuaFlow = {}\nfunction LuaFlow.target_lua() end\nfunction LuaFlow.caller_lua() LuaFlow.target_lua() end\n",
        ),
        (
            "src/SwiftFlow.swift",
            "public class SwiftFlow {\n  public func targetSwift() {}\n  public func callerSwift() { targetSwift() }\n}\n",
        ),
        (
            "src/dart_flow.dart",
            "class DartFlow {\n  void targetDart() {}\n  void callerDart() { targetDart(); }\n}\n",
        ),
        (
            "src/ScalaFlow.scala",
            "class ScalaFlow {\n  def targetScala(): Unit = ()\n  def callerScala(): Unit = targetScala()\n}\n",
        ),
        (
            "src/GroovyFlow.groovy",
            "class GroovyFlow {\n  void targetGroovy() {}\n  void callerGroovy() { targetGroovy() }\n}\n",
        ),
        (
            "src/PowerShellFlow.ps1",
            "class PowerShellFlow {\n  [void] TargetPowerShell() {}\n  [void] CallerPowerShell() { $this.TargetPowerShell() }\n}\n",
        ),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n[caller_context]\nnavigation_context_default = true\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    for (path, query, language_hint, qualified_name) in [
        (
            "src/rust_flow.rs",
            "target_rust",
            "rust",
            "RustFlow.target_rust",
        ),
        (
            "src/python_flow.py",
            "target_python",
            "py",
            "PythonFlow.target_python",
        ),
        (
            "src/typescript_flow.ts",
            "targetTypeScript",
            "ts",
            "TypeScriptFlow.targetTypeScript",
        ),
        (
            "src/javascript_flow.js",
            "targetJavaScript",
            "js",
            "JavaScriptFlow.targetJavaScript",
        ),
        ("src/go_flow.go", "TargetGo", "go", "GoFlow.TargetGo"),
        (
            "src/JavaFlow.java",
            "targetJava",
            "java",
            "JavaFlow.targetJava",
        ),
        (
            "src/KotlinFlow.kt",
            "targetKotlin",
            "kt",
            "KotlinFlow.targetKotlin",
        ),
        ("src/c_flow.c", "target_c", "c", "target_c"),
        ("src/cpp_flow.cpp", "targetCpp", "cpp", "CppFlow.targetCpp"),
        ("src/asm_flow.s", "target_asm", "asm", "target_asm"),
        (
            "src/CSharpFlow.cs",
            "TargetCSharp",
            "c#",
            "CSharpFlow.TargetCSharp",
        ),
        ("src/php_flow.php", "targetPhp", "php", "PhpFlow.targetPhp"),
        (
            "src/ruby_flow.rb",
            "target_ruby",
            "rb",
            "RubyFlow.target_ruby",
        ),
        (
            "src/lua_flow.lua",
            "target_lua",
            "lua",
            "LuaFlow.target_lua",
        ),
        (
            "src/SwiftFlow.swift",
            "targetSwift",
            "swift",
            "SwiftFlow.targetSwift",
        ),
        (
            "src/dart_flow.dart",
            "targetDart",
            "dart",
            "DartFlow.targetDart",
        ),
        (
            "src/ScalaFlow.scala",
            "targetScala",
            "scala",
            "ScalaFlow.targetScala",
        ),
        (
            "src/GroovyFlow.groovy",
            "targetGroovy",
            "gradle",
            "GroovyFlow.targetGroovy",
        ),
        (
            "src/PowerShellFlow.ps1",
            "TargetPowerShell",
            "pwsh",
            "PowerShellFlow.TargetPowerShell",
        ),
    ] {
        let search = client
            .send_tool_until(
                "search",
                serde_json::json!({
                    "query": query,
                    "language_hint": language_hint,
                    "caller_context": true
                }),
                |text| text.contains(path) && text.contains(qualified_name),
            )
            .await
            .unwrap();
        let search_text = search["result"]["content"][0]["text"].as_str().unwrap();
        assert!(search_text.contains(path), "{path}: {search_text}");
        assert!(
            search_text.contains(qualified_name),
            "{qualified_name}: {search_text}"
        );
        let overview = client
            .send_tool_until("overview", serde_json::json!({ "path": path }), |text| {
                text.contains(query)
            })
            .await
            .unwrap();
        let overview_text = overview["result"]["content"][0]["text"].as_str().unwrap();
        assert!(overview_text.contains(query), "{path}: {overview_text}");
    }

    for (path, target_query, caller_query, qualified_target, qualified_caller, call_site) in [
        (
            "src/CSharpFlow.cs",
            "TargetCSharp",
            "CallerCSharp",
            "CSharpFlow.TargetCSharp",
            "CSharpFlow.CallerCSharp",
            "src/CSharpFlow.cs:3",
        ),
        (
            "src/php_flow.php",
            "targetPhp",
            "callerPhp",
            "PhpFlow.targetPhp",
            "PhpFlow.callerPhp",
            "src/php_flow.php:3",
        ),
        (
            "src/ruby_flow.rb",
            "target_ruby",
            "caller_ruby",
            "RubyFlow.target_ruby",
            "RubyFlow.caller_ruby",
            "src/ruby_flow.rb:5",
        ),
        (
            "src/lua_flow.lua",
            "target_lua",
            "caller_lua",
            "LuaFlow.target_lua",
            "LuaFlow.caller_lua",
            "src/lua_flow.lua:3",
        ),
        (
            "src/SwiftFlow.swift",
            "targetSwift",
            "callerSwift",
            "SwiftFlow.targetSwift",
            "SwiftFlow.callerSwift",
            "src/SwiftFlow.swift:3",
        ),
        (
            "src/dart_flow.dart",
            "targetDart",
            "callerDart",
            "DartFlow.targetDart",
            "DartFlow.callerDart",
            "src/dart_flow.dart:3",
        ),
        (
            "src/ScalaFlow.scala",
            "targetScala",
            "callerScala",
            "ScalaFlow.targetScala",
            "ScalaFlow.callerScala",
            "src/ScalaFlow.scala:3",
        ),
        (
            "src/GroovyFlow.groovy",
            "targetGroovy",
            "callerGroovy",
            "GroovyFlow.targetGroovy",
            "GroovyFlow.callerGroovy",
            "src/GroovyFlow.groovy:3",
        ),
        (
            "src/PowerShellFlow.ps1",
            "TargetPowerShell",
            "CallerPowerShell",
            "PowerShellFlow.TargetPowerShell",
            "PowerShellFlow.CallerPowerShell",
            "src/PowerShellFlow.ps1:3",
        ),
    ] {
        if path == "src/lua_flow.lua" {
            // A dynamic table receiver needs binding/mutation analysis before a definition
            // link can be confirmed. Preserve the executable call without a guessed target.
            let response = client
                .send_tool_until(
                    "search",
                    serde_json::json!({
                        "query": caller_query, "caller_context": true
                    }),
                    |text| text.contains("target_lua (unresolved)"),
                )
                .await
                .unwrap();
            let text = response["result"]["content"][0]["text"].as_str().unwrap();
            assert!(text.contains("target_lua (unresolved)"), "{text}");
            assert!(
                !text.contains("LuaFlow.target_lua —"),
                "unconfirmed table target linked: {text}"
            );
            continue;
        }
        let target_response = client
            .send_tool_until(
                "search",
                serde_json::json!({
                    "query": target_query,
                    "caller_context": true
                }),
                |text| {
                    text.contains(qualified_caller)
                        && text.contains(call_site)
                        && text.contains("tree-sitter precise")
                },
            )
            .await
            .unwrap();
        let target_text = target_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(
            target_text.contains(qualified_target),
            "{path}: target symbol missing from caller result: {target_text}"
        );
        assert!(
            target_text.contains(qualified_caller) && target_text.contains(call_site),
            "{path}: precise caller and call site missing: {target_text}"
        );
        assert!(
            target_text.contains("tree-sitter precise"),
            "{path}: caller attribution was not precise: {target_text}"
        );

        let caller_response = client
            .send_tool_until(
                "search",
                serde_json::json!({
                    "query": caller_query,
                    "caller_context": true
                }),
                |text| {
                    text.contains("calls (depth 1")
                        && text.contains(&format!("- {qualified_target} — {path}:2 (precise)"))
                },
            )
            .await
            .unwrap();
        let caller_text = caller_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(
            caller_text.contains("calls (depth 1")
                && caller_text.contains(&format!("- {qualified_target} — {path}:2 (precise)")),
            "{path}: precise callee missing from caller symbol: {caller_text}"
        );
    }
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
        ("site.less", ".surface(@color) { color: @color; }\n"),
        (
            "site.sass",
            "@mixin sass_surface($color)\n  color: $color\n",
        ),
        (
            "Widget.vue",
            "<template><main class=\"vue-shell\" /></template><style>.vue-style {}</style>",
        ),
        (
            "Widget.astro",
            "<main class=\"astro-shell\"></main><style>.astro-style {}</style>",
        ),
        (
            "Widget.svelte",
            "<main class=\"svelte-shell\"></main><style>.svelte-style {}</style>",
        ),
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
            "# starlark_mcp_token\nfirst, second = (1, 2)\ncc_library(name = \"core\")\n",
        ),
        (
            "default.nix",
            "{ nix_mcp_token = true; target = derivation { name = \"demo\"; }; }\n",
        ),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n[language_support]\nis_shell_support_enabled = true\nis_infrastructure_support_enabled = true\nis_interface_support_enabled = true\nis_build_support_enabled = true\n",
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
        ("site.less", ".surface"),
        ("site.sass", "sass_surface"),
        ("Widget.vue", "vue-shell"),
        ("Widget.astro", "astro-shell"),
        ("Widget.svelte", "svelte-shell"),
        ("deploy.sh", "deploy"),
        ("deploy.zsh", "prepare"),
        ("main.tf", "prevent_destroy"),
        ("Dockerfile", "build"),
        ("api.proto", "identity"),
        ("schema.graphql", "other"),
        ("Makefile", "package"),
        ("CMakeLists.txt", "unit"),
        ("BUILD", "second"),
        ("default.nix", "target"),
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

    for (query, path) in [
        ("package", "Makefile"),
        ("unit", "CMakeLists.txt"),
        ("starlark_mcp_token", "BUILD"),
        ("nix_mcp_token", "default.nix"),
    ] {
        let result = client
            .send_tool_until("search", serde_json::json!({ "query": query }), |text| {
                text.contains(path)
            })
            .await
            .unwrap();
        let text = result["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(path), "search omitted {path}: {text}");
    }
}

#[tokio::test]
async fn test_nix_precise_callers_callees_and_local_shadowing_flow_through_mcp() {
    let temp = create_mock_repo(&[
        (
            "default.nix",
            "let\n  targetNix = value: value;\n  callerNix = value: targetNix value;\n  shadowedNix = value: value;\n  shadowCaller = shadowedNix: shadowedNix 1;\nin { inherit targetNix callerNix shadowedNix shadowCaller; }\n",
        ),
        (
            ".codemap/config.toml",
            "watch = false\nindex_staleness_ms = 1\n[language_support]\nis_infrastructure_support_enabled = true\n[caller_context]\nnavigation_context_default = true\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let target = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "targetNix", "language_hint": "nix", "caller_context": true }),
            |text| {
                text.contains("callerNix")
                    && text.contains("default.nix:3")
                    && text.contains("tree-sitter precise")
            },
        )
        .await
        .unwrap();
    let target_text = target["result"]["content"][0]["text"].as_str().unwrap();
    assert!(target_text.contains("callerNix"));
    assert!(target_text.contains("default.nix:3"));
    assert!(target_text.contains("tree-sitter precise"));

    let caller = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "callerNix", "language_hint": "nix", "caller_context": true }),
            |text| text.contains("- targetNix — default.nix:2 (precise)"),
        )
        .await
        .unwrap();
    let caller_text = caller["result"]["content"][0]["text"].as_str().unwrap();
    assert!(caller_text.contains("- targetNix — default.nix:2 (precise)"));

    let shadowed = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "shadowedNix", "language_hint": "nix", "caller_context": true }),
            |text| text.contains("shadowedNix") && !text.contains("warming up"),
        )
        .await
        .unwrap();
    let shadowed_text = shadowed["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        !shadowed_text.contains("shadowCaller (default.nix:5)"),
        "function parameter shadowing must suppress a false precise caller: {shadowed_text}"
    );
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
