use crate::e2e::helpers::{create_mock_repo, run_cli, OPTIONAL_LANGUAGE_SUPPORT_CONFIG};
use filetime::{set_file_mtime, FileTime};
use predicates::prelude::*;
use std::fs;
use std::thread::sleep;
use std::time::Duration;

#[tokio::test]
async fn test_events_exact_key_negative_controls_and_ranked_search() {
    let temp = crate::e2e::helpers::event_navigation_repo();
    let mut client = crate::e2e::helpers::McpClient::spawn(temp.path())
        .await
        .unwrap();
    let text = |response: serde_json::Value| {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    let args = serde_json::json!({"query":"saved","event_key":"saved"});
    let saved = text(
        client
            .send_tool_until("search", args, |out| {
                out.contains("publisher: src/users.ts:2")
            })
            .await
            .unwrap(),
    );
    assert_eq!(saved.matches("### Event \"saved\"").count(), 2, "{saved}");
    assert!(
        saved.contains("registration: src/events.ts:6")
            && saved.contains("publisher: src/other.ts:2"),
        "{saved}"
    );
    for section in saved.split("### Event").skip(1) {
        assert!(
            !(section.contains("src/other.ts") && section.contains("registration:")),
            "{saved}"
        );
    }
    for key in [
        "wrong-method",
        "wrong-string",
        "wrong-comment",
        "wrong-shadow",
    ] {
        let out = text(
            client
                .send_request(
                    "tools/call",
                    serde_json::json!({"name":"search","arguments":{"query":key,"event_key":key}}),
                )
                .await
                .unwrap(),
        );
        assert!(out.contains("No eligible event endpoints"), "{out}");
    }
    for (key, expected) in [
        ("opaque", "unresolved publisher"),
        ("local", "unresolved publisher"),
        ("unknown-handler", "handler unresolved:"),
        ("inactive", "statically inactive condition"),
        ("conditional", "once-only registration"),
    ] {
        let out = text(
            client
                .send_request(
                    "tools/call",
                    serde_json::json!({"name":"search","arguments":{"query":key,"event_key":key}}),
                )
                .await
                .unwrap(),
        );
        assert!(out.contains(expected), "{out}");
        if key == "conditional" {
            assert!(
                out.contains("removal observed") && out.contains("conditional:"),
                "{out}"
            );
        }
    }
    let dynamic=text(client.send_request("tools/call",serde_json::json!({"name":"read","arguments":{"file_path":"src/controls.ts","offset":6,"limit":1,"view":"relations","include_events":true}})).await.unwrap());
    assert!(
        dynamic.contains("event key is not a supported literal/static immutable value"),
        "{dynamic}"
    );
    let ranked=text(client.send_request("tools/call",serde_json::json!({"name":"search","arguments":{"query":"save users","caller_context":false}})).await.unwrap());
    assert!(
        ranked.contains("## Event relationships") && ranked.contains("publisher: src/users.ts:2"),
        "{ranked}"
    );
    let hidden=text(client.send_request("tools/call",serde_json::json!({"name":"search","arguments":{"query":"save users","caller_context":false,"include_events":false}})).await.unwrap());
    assert!(!hidden.contains("Event relationships"), "{hidden}");
    for arguments in [
        serde_json::json!({"query":"saved","event_key":""}),
        serde_json::json!({"query":"saved","include_events":"yes"}),
    ] {
        let response = client
            .send_request(
                "tools/call",
                serde_json::json!({"name":"search","arguments":arguments}),
            )
            .await
            .unwrap();
        assert!(response.get("error").is_some(), "{response}");
    }
}

#[tokio::test]
async fn test_events_query_caps_keep_explicit_omissions() {
    let source = format!(
        "import {{EventEmitter}} from 'node:events'; const bus=new EventEmitter();\n{}",
        "bus.on('busy',()=>{}); bus.emit('busy');\n".repeat(150)
    );
    let temp=create_mock_repo(&[("src/busy.ts",&source),(".codemap/config.toml","[update]\nconfig_auto_update=false\n[event_navigation]\nis_enabled=true\n[search]\nsearch_detail_byte_cap=3000\n[tool_output]\nread_output_byte_cap=3000\n")]).unwrap();
    let mut client = crate::e2e::helpers::McpClient::spawn(temp.path())
        .await
        .unwrap();
    let response = client
        .send_tool_until(
            "search",
            serde_json::json!({"query":"busy","event_key":"busy"}),
            |out| out.contains("Event output cap reached"),
        )
        .await
        .unwrap();
    let out = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        out.contains("registration:") && out.contains("Event output cap reached"),
        "{out}"
    );
    assert!(
        out.contains("44 extraction omissions")
            && out.contains("query candidate/endpoint omissions"),
        "{out}"
    );
    assert!(out.len() <= 3000, "{}: {out}", out.len());
}

#[tokio::test]
async fn test_events_builtin_api_catalog_and_disable_reach_output() {
    let mut files = Vec::new();
    for extension in ["js", "ts"] {
        for (module, id) in [("events", "plain"), ("node:events", "node")] {
            files.push((format!("src/{id}.{extension}"),format!("import {{EventEmitter}} from '{module}'; const bus=new EventEmitter(); function handler() {{}}\nbus.on('catalog',handler); bus.addListener('catalog',handler); bus.once('catalog',handler); bus.emit('catalog'); bus.off('catalog',handler); bus.removeListener('catalog',handler);\nconst unrelated={{on(){{}},emit(){{}}}}; unrelated.on('wrong',handler); unrelated.emit('wrong');")));
        }
    }
    for extension in ["js", "ts"] {
        files.push((format!("src/tauri.{extension}"),"import {listen,once,emit,emitTo} from '@tauri-apps/api/event'; listen('tauri-catalog',()=>{}); once('tauri-catalog',()=>{}); emit('tauri-catalog'); emitTo('child','tauri-catalog');".into()));
    }
    let mut rust =
        "use tauri::{AppHandle,App,Window,WebviewWindow,Webview,Emitter,Listener};\n".to_string();
    for (i, typ) in ["AppHandle", "App", "Window", "WebviewWindow", "Webview"]
        .iter()
        .enumerate()
    {
        rust.push_str(&format!("fn endpoint_{i}(app: &{typ}) {{ app.emit(\"rust-catalog\",()); app.emit_to(\"child\",\"rust-catalog\",()); app.listen(\"rust-catalog\", |_| {{}}); app.once(\"rust-catalog\", |_| {{}}); }}\n"));
    }
    files.push(("src/lib.rs".into(), rust));
    files.push(("package.json".into(), "{\"name\":\"event-catalog\"}".into()));
    let config = "[update]\nconfig_auto_update=false\n";
    files.push((".codemap/config.toml".into(), config.into()));
    let borrowed: Vec<_> = files
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect();
    let temp = create_mock_repo(&borrowed).unwrap();
    let mut client = crate::e2e::helpers::McpClient::spawn(temp.path())
        .await
        .unwrap();
    for (key, count) in [("catalog", 24), ("tauri-catalog", 8), ("rust-catalog", 20)] {
        let response = client
            .send_tool_until(
                "search",
                serde_json::json!({"query":key,"event_key":key}),
                |out| out.contains("API:"),
            )
            .await
            .unwrap();
        let out = response["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(out.matches("  - API:").count(), count, "{out}");
        if key == "catalog" {
            assert_eq!(out.matches("### Event").count(), 4, "{out}");
        }
        if key == "rust-catalog" {
            assert_eq!(out.matches("unresolved ").count(), 20, "{out}");
            assert!(!out.contains("### Event"), "{out}");
        }
    }
    fs::write(
        temp.path().join(".codemap/config.toml"),
        format!("{config}\n[event_navigation]\nuse_builtin_rules=false\n"),
    )
    .unwrap();
    let response = client
        .send_tool_until(
            "search",
            serde_json::json!({"query":"catalog","event_key":"catalog"}),
            |out| out.contains("No eligible event endpoints"),
        )
        .await
        .unwrap();
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No eligible event endpoints"),
        "{response}"
    );
}

#[tokio::test]
async fn test_events_scope_and_argument_fixed_key_channel_rules() {
    let rules = r#"
[update]
config_auto_update=false
[event_navigation]
is_enabled=true
rules=[
{id='subscribe',language='typescript',module='packages/api/api.ts',symbol='subscribe',role='subscribe',event_arg=1,handler_arg=2,bus='argument',bus_arg=0},
{id='publish',language='typescript',module='packages/api/api.ts',symbol='publish',role='publish',event_arg=1,bus='argument',bus_arg=0},
{id='fixed-register',language='typescript',module='packages/api/api.ts',symbol='registerSaved',role='subscribe',event_key='fixed',handler_arg=0,bus='fixed',bus_identity='application',target='any',channel='ui'},
{id='fixed-send',language='typescript',module='packages/api/api.ts',symbol='sendSaved',role='publish',event_key='fixed',bus='fixed',bus_identity='application',target='any',channel='worker'}
]
"#;
    let temp=create_mock_repo(&[
        ("packages/api/api.ts","import {EventEmitter} from 'events'; export const bus=new EventEmitter(); export function subscribe(bus,key,handler){} export function publish(bus,key){} export function registerSaved(handler){} export function sendSaved(){}"),
        ("packages/client/register.ts","import {bus,subscribe,registerSaved} from '../api/api'; function handler(){} subscribe(bus,'arg',handler); registerSaved(handler);"),
        ("packages/api/send.ts","import {bus,publish,sendSaved} from './api'; publish(bus,'arg'); sendSaved();"),
        (".codemap/config.toml",rules),
    ]).unwrap();
    let mut client = crate::e2e::helpers::McpClient::spawn(temp.path())
        .await
        .unwrap();
    let all = client
        .send_tool_until(
            "search",
            serde_json::json!({"query":"arg","event_key":"arg","workspace_scope":"all"}),
            |out| out.contains("publisher:") && out.contains("registration:"),
        )
        .await
        .unwrap();
    let text = all["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("publisher:") && text.contains("registration:"),
        "{all}"
    );
    let scoped=client.send_request("tools/call",serde_json::json!({"name":"search","arguments":{"query":"arg","event_key":"arg","workspace_scope":"packages/client"}})).await.unwrap();
    let text = scoped["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("registration:") && !text.contains("publisher:"),
        "{scoped}"
    );
    let fixed=client.send_request("tools/call",serde_json::json!({"name":"search","arguments":{"query":"fixed","event_key":"fixed","workspace_scope":"all"}})).await.unwrap();
    let text = fixed["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(text.matches("### Event").count(), 2, "{fixed}");
    assert!(
        text.contains("configured bus assumption")
            && text.contains("configured key assumption")
            && text.contains("configured target assumption"),
        "{fixed}"
    );
}

#[test]
fn test_explicit_filename_is_recalled_after_common_symbols_fill_the_primary_pool() {
    let mut fixtures: Vec<(String, String)> = (0..40)
        .map(|index| {
            (
                format!("src/registry_search/noise_{index}.rs"),
                "pub fn lookup() {}\npub fn registry_search() {}\n".to_string(),
            )
        })
        .collect();
    fixtures.push((
        "src/registry_search.rs".to_string(),
        "pub fn lookup() {}\n".to_string(),
    ));
    let borrowed: Vec<_> = fixtures
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let temp = create_mock_repo(&borrowed).unwrap();
    run_cli(&["index"], temp.path()).success();
    run_cli(
        &["search", "lookup registry_search", "--limit", "1"],
        temp.path(),
    )
    .success()
    .stdout(predicates::str::starts_with("src/registry_search.rs\n"));
}

#[test]
fn test_bm25_explicit_test_filename_is_not_demoted() {
    let temp = create_mock_repo(&[
        ("src/production.rs", "pub fn append() {}"),
        ("tests/run/i12032.rs", "pub fn append() {}"),
    ])
    .unwrap();
    run_cli(&["index"], temp.path()).success();
    run_cli(&["search", "append i12032", "--limit", "10"], temp.path())
        .success()
        .stdout(predicates::str::starts_with("tests/run/i12032.rs\n"));
    run_cli(&["search", "append", "--limit", "10"], temp.path())
        .success()
        .stdout(predicates::str::starts_with("src/production.rs\n"));
}

#[test]
fn test_bm25_long_camel_case_identifiers() {
    let temp = create_mock_repo(&[(
        "src/WrapperScriptUpgradeCrossVersionIntegrationTest.java",
        "class WrapperScriptUpgradeCrossVersionIntegrationTest {\n    void canUseWrapperFromPreviousVersionToUpgradeToCurrentVersionWrapper() {}\n}\n",
    )])
    .unwrap();
    run_cli(&["index"], temp.path()).success();
    run_cli(
        &["search", "canUseWrapperFromPreviousVersionToUpgradeToCurrentVersionWrapper WrapperScriptUpgradeCrossVersionIntegrationTest"],
        temp.path(),
    )
    .success()
    .stdout(predicates::str::contains("src/WrapperScriptUpgradeCrossVersionIntegrationTest.java"));
    run_cli(&["search", &"x".repeat(80)], temp.path()).success();
}

#[test]
fn test_bm25_basic_search() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn find_my_function_name() {}")]).unwrap();

    // Index the repository
    let assert_index = run_cli(&["index"], temp.path());
    assert_index.success();

    // Search for the function
    let assert_search = run_cli(&["search", "find_my_function_name"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_bm25_format_text_surfaces_structured_markup_shell_and_infrastructure_files() {
    let temp = create_mock_repo(&[
        ("config.yaml", "feature_token: structured_needle\n"),
        ("page.html", "<main>markup_needle</main>"),
        ("deploy.sh", "shell_needle=1\n"),
        ("Dockerfile", "FROM rust\n# infrastructure_needle\n"),
        (".codemap/config.toml", OPTIONAL_LANGUAGE_SUPPORT_CONFIG),
    ])
    .unwrap();
    run_cli(&["index"], temp.path()).success();
    for (query, path) in [
        ("structured_needle", "config.yaml"),
        ("markup_needle", "page.html"),
        ("shell_needle", "deploy.sh"),
        ("infrastructure_needle", "Dockerfile"),
    ] {
        run_cli(&["search", query], temp.path())
            .success()
            .stdout(predicates::str::contains(path));
    }
}

#[test]
fn test_bm25_search_reaches_every_priority_alias_and_grammar_boundary() {
    let formats = [
        ("config.json", "{\"token\": \"json_token\"}", "json_token"),
        ("config.jsonc", "// jsonc_token\n{}", "jsonc_token"),
        ("config.toml", "token = \"toml_token\"", "toml_token"),
        ("config.yaml", "token: yaml_token", "yaml_token"),
        ("config.yml", "token: yml_token", "yml_token"),
        ("page.html", "<!-- html_token -->", "html_token"),
        ("page.htm", "<!-- htm_token -->", "htm_token"),
        ("page.xml", "<!-- xml_token -->", "xml_token"),
        ("schema.xsd", "<!-- xsd_token -->", "xsd_token"),
        ("page.xsl", "<!-- xsl_token -->", "xsl_token"),
        ("page.xslt", "<!-- xslt_token -->", "xslt_token"),
        ("Info.plist", "<!-- plist_token -->", "plist_token"),
        ("app.csproj", "<!-- csproj_token -->", "csproj_token"),
        ("app.props", "<!-- props_token -->", "props_token"),
        ("app.targets", "<!-- targets_token -->", "targets_token"),
        ("site.css", "/* css_token */", "css_token"),
        ("deploy.sh", "# sh_token", "sh_token"),
        ("deploy.bash", "# bash_token", "bash_token"),
        ("main.hcl", "# hcl_token", "hcl_token"),
        ("main.tf", "# tf_token", "tf_token"),
        ("values.tfvars", "# tfvars_token", "tfvars_token"),
        ("api.proto", "// proto_token", "proto_token"),
        ("schema.graphql", "# graphql_token", "graphql_token"),
        ("schema.gql", "# gql_token", "gql_token"),
        (
            "site.less",
            "// less_tree_sitter_token",
            "less_tree_sitter_token",
        ),
        ("site.sass", "// sass_ast_token", "sass_ast_token"),
        (
            "Widget.vue",
            "<template><main>vue_structured_token</main></template>",
            "vue_structured_token",
        ),
        (
            "Widget.astro",
            "<div>astro_structured_token</div>",
            "astro_structured_token",
        ),
        (
            "Widget.svelte",
            "<main>svelte_structured_token</main>",
            "svelte_structured_token",
        ),
        (
            "Dockerfile",
            "# docker_tree_sitter_token",
            "docker_tree_sitter_token",
        ),
        (
            "Makefile",
            "# make_tree_sitter_token",
            "make_tree_sitter_token",
        ),
        (
            "CMakeLists.txt",
            "# cmake_tree_sitter_token",
            "cmake_tree_sitter_token",
        ),
        (
            "BUILD",
            "# build_tree_sitter_token",
            "build_tree_sitter_token",
        ),
        (
            "BUILD.bazel",
            "# bazel_tree_sitter_token",
            "bazel_tree_sitter_token",
        ),
        (
            "default.nix",
            "# nix_tree_sitter_token\n{ value = 1; }",
            "nix_tree_sitter_token",
        ),
    ];
    let mut files = formats
        .iter()
        .map(|(path, body, _)| (*path, *body))
        .collect::<Vec<_>>();
    files.push((".codemap/config.toml", OPTIONAL_LANGUAGE_SUPPORT_CONFIG));
    let temp = create_mock_repo(&files).unwrap();
    run_cli(&["index"], temp.path()).success();
    for (path, _, token) in formats {
        run_cli(&["search", token], temp.path())
            .success()
            .stdout(predicates::str::contains(path));
    }
}

#[test]
fn test_bm25_field_weighting() {
    let temp = create_mock_repo(&[
        ("src/file_a.rs", "/// QueryTerm\npub fn dummy() {}"), // Term in docstring
        ("src/file_b.rs", "pub fn QueryTerm() {}"), // Term in symbol name (highest weight)
        ("src/file_c.rs", "pub fn other() { let x = \"QueryTerm\"; }"), // Term only in a string literal
    ])
    .unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Search for QueryTerm: all three tiers rank in (v3 indexes string literals at the
    // lowest boost), with the symbol match (file_b) first.
    let assert_search = run_cli(&["search", "QueryTerm"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::starts_with("src/file_b.rs"))
        .stdout(predicates::str::contains("src/file_a.rs"))
        .stdout(predicates::str::contains("src/file_c.rs"));
}

#[test]
fn test_bm25_language_hint_reduces_cross_language_top1_misrank() {
    let temp = create_mock_repo(&[
        (
            "src/policy.rs",
            "/// policy policy policy policy\npub fn policy() {}\npub fn policy_helper() { policy(); }\n",
        ),
        ("src/policy.ts", "export function policy() { return true; }\n"),
    ])
    .unwrap();

    run_cli(&["index"], temp.path()).success();

    run_cli(&["search", "policy"], temp.path())
        .success()
        .stdout(predicates::str::starts_with("src/policy.rs"));

    run_cli(
        &[
            "search",
            "policy",
            "--language-hint",
            "typescript",
            "--extension-hint",
            "ts",
        ],
        temp.path(),
    )
    .success()
    .stdout(predicates::str::starts_with("src/policy.ts"));
}

#[test]
fn test_bm25_composite_code_and_markup_are_indexed_without_mdx() {
    let temp = create_mock_repo(&[
        (
            "src/component.vue",
            "<template><div>component_template_noise Don't</div></template>\n<script data-kind=\"client\" LANG=\"ts\">\nexport function component_index_symbol() { return 'component_literal'; }\n</script>\n<style>.component_style_noise { color: red; }</style>\n",
        ),
        (
            "src/component.astro",
            "<html><body><script>export function nested_astro_index_symbol() { return '</'; }</script></body></html>\n",
        ),
        (
            "docs/component.mdx",
            "export const mdx_index_noise = 'mdx_index_noise';\n# mdx_index_noise\n",
        ),
    ])
    .unwrap();

    run_cli(&["index"], temp.path()).success();
    run_cli(&["search", "component_index_symbol"], temp.path())
        .success()
        .stdout(predicates::str::starts_with("src/component.vue"));
    run_cli(&["search", "nested_astro_index_symbol"], temp.path())
        .success()
        .stdout(predicates::str::starts_with("src/component.astro"));
    run_cli(&["search", "component_template_noise"], temp.path())
        .success()
        .stdout(predicates::str::contains("src/component.vue"));
    run_cli(&["search", "component_style_noise"], temp.path())
        .success()
        .stdout(predicates::str::contains("src/component.vue"));
    run_cli(&["search", "mdx_index_noise"], temp.path())
        .success()
        .stdout(predicates::str::contains("component.mdx").not());
}

#[test]
fn test_bm25_existing_language_hints_do_not_promote_new_formats() {
    let temp = create_mock_repo(&[
        ("src/rank.rs", "pub fn language_rank_matrix() {}\n"),
        (
            "src/rank.sql",
            "CREATE FUNCTION language_rank_matrix() RETURNS INT LANGUAGE SQL AS $$ SELECT 1; $$;\n",
        ),
        (
            "src/rank.vue",
            "<script>export function language_rank_matrix() {}</script>\n",
        ),
    ])
    .unwrap();

    run_cli(&["index"], temp.path()).success();
    run_cli(
        &[
            "search",
            "language_rank_matrix",
            "--language-hint",
            "rust",
            "--extension-hint",
            "rs",
        ],
        temp.path(),
    )
    .success()
    .stdout(predicates::str::starts_with("src/rank.rs"));
}

#[test]
fn test_bm25_incremental_no_change() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn hello() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    let index_dir = temp.path().join(".codemap/index");
    let initial_mtime = if index_dir.exists() {
        fs::metadata(&index_dir).unwrap().modified().unwrap()
    } else {
        std::time::SystemTime::now()
    };

    sleep(Duration::from_millis(100));

    // Index again with no changes
    let _ = run_cli(&["index"], temp.path());

    let final_mtime = if index_dir.exists() {
        fs::metadata(&index_dir).unwrap().modified().unwrap()
    } else {
        initial_mtime
    };

    assert_eq!(initial_mtime, final_mtime);
}

#[test]
fn test_bm25_incremental_update() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn hello() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Modify a file
    let file_path = temp.path().join("src/lib.rs");
    fs::write(&file_path, "pub fn hello_world() {}").unwrap();

    // Set a newer mtime
    let new_time =
        FileTime::from_system_time(std::time::SystemTime::now() + Duration::from_secs(10));
    set_file_mtime(&file_path, new_time).unwrap();

    // Re-index
    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "hello_world"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_bm25_index_persistence() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn persistent_func() {}")]).unwrap();

    // 1. Run index
    let _ = run_cli(&["index"], temp.path());

    // 2. Search immediately
    let assert_search_1 = run_cli(&["search", "persistent_func"], temp.path());
    assert_search_1
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));

    // 3. Search in a fresh process, ensuring index is persistent
    let assert_search_2 = run_cli(&["search", "persistent_func"], temp.path());
    assert_search_2
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_bm25_search_non_existent() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn hello() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "NonExistentFunctionToken"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs").not());
}

#[test]
fn test_bm25_search_special_chars() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn find_regex() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Search with wildcards, regex chars, or special punctuations
    let assert_search = run_cli(&["search", "*()!@#+$^&?"], temp.path());
    assert_search.success();
}

#[test]
fn test_bm25_concurrent_access() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn concurrent() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // We can run two searches or an index and a search concurrently.
    // In our test skeleton, we simulate this by spawning multiple CLI commands.
    use std::thread;
    let path_clone_1 = temp.path().to_path_buf();
    let path_clone_2 = temp.path().to_path_buf();

    let handle1 = thread::spawn(move || run_cli(&["search", "concurrent"], &path_clone_1));
    let handle2 = thread::spawn(move || run_cli(&["index"], &path_clone_2));

    let _ = handle1.join();
    let _ = handle2.join();
}

#[test]
fn test_bm25_corrupt_index() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn test() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Corrupt index files in .codemap/index
    let index_dir = temp.path().join(".codemap/index");
    if index_dir.exists() {
        let meta_file = index_dir.join("meta.json");
        fs::write(meta_file, "{invalid json}").unwrap();
    }

    // Server should auto-recovery/rebuild index
    let assert_search = run_cli(&["search", "test"], temp.path());
    assert_search.success();
}

#[test]
fn test_bm25_mtime_oscillation() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn oscillating() {}")]).unwrap();

    let file_path = temp.path().join("src/lib.rs");
    let _ = run_cli(&["index"], temp.path());

    // Set mtime backwards (oscillation)
    let past_time =
        FileTime::from_system_time(std::time::SystemTime::now() - Duration::from_secs(3600));
    set_file_mtime(&file_path, past_time).unwrap();

    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "oscillating"], temp.path());
    assert_search.success();
}

#[test]
fn test_bm25_partial_failure_non_utf8() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn utf8_function() {}")]).unwrap();

    // Create a file with invalid UTF-8 (binary payload)
    let invalid_utf8_path = temp.path().join("src/binary.rs");
    std::fs::write(&invalid_utf8_path, b"\xFF\xFE\xFD\xFC").unwrap();

    // Index the directory (should print warning but succeed overall)
    let assert_index = run_cli(&["index"], temp.path());
    assert_index.success();

    // Verify search still works for the valid file
    let assert_search = run_cli(&["search", "utf8_function"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}
