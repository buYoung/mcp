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
async fn test_generic_value_relationships_persist_and_follow_live_views() {
    let source = "const routes = new Map();\nexport function connect(key) { return { on(handler) { return add(key, handler); } }; }\nfunction add(key, handler) { routes.set(key, handler); }\nfunction dispatch(key, payload) { const callback = routes.get(key); callback(payload); }\nexport function notify(payload) {}\nexport function setup() { const handle = connect('changed'); handle.on(notify); }\nexport function publish() { dispatch('changed', 1); }\n";
    let temp = create_mock_repo(&[("src/bus.ts", source), ("src/plain.ts", "// nothing to compose\n42;\n"), (".codemap/config.toml", "[update]\nconfig_auto_update=false\n[event_navigation]\nis_enabled=false\n[refresh]\nwatch=false\nindex_staleness_ms=600000\n")]).unwrap();
    for _ in 0..2 {
        let mut client = McpClient::spawn(temp.path()).await.unwrap();
        let response = client
            .send_tool_until(
                "read",
                serde_json::json!({"file_path":"src/bus.ts","offset":6,"limit":1}),
                |out| out.contains("possible callback invocation"),
            )
            .await
            .unwrap();
        let output = text(&response);
        assert!(
            output.contains("notify — L5") && output.contains("6→export function setup"),
            "{output}"
        );
        assert!(output.len() <= 16_384, "{}", output.len());
        for (tool, args, expected) in [
            (
                "grep",
                serde_json::json!({"path":"src/bus.ts","pattern":"export function setup"}),
                true,
            ),
            (
                "grep",
                serde_json::json!({"path":"src/bus.ts","pattern":"setup","output_mode":"count"}),
                false,
            ),
            (
                "read",
                serde_json::json!({"file_path":"src/bus.ts","offset":6,"limit":1,"view":"source"}),
                false,
            ),
            (
                "read",
                serde_json::json!({"file_path":"src/bus.ts","offset":6,"limit":1,"view":"definitions"}),
                false,
            ),
            (
                "read",
                serde_json::json!({"file_path":"src/bus.ts","offset":6,"limit":1,"view":"relations","include_events":false}),
                true,
            ),
            (
                "read",
                serde_json::json!({"file_path":"src/plain.ts","offset":1,"limit":2}),
                false,
            ),
            ("search", serde_json::json!({"query":"setup"}), true),
            (
                "search",
                serde_json::json!({"query":"setup","caller_context":false}),
                false,
            ),
        ] {
            let output = text(
                &client
                    .send_request("tools/call", call(tool, args.clone()))
                    .await
                    .unwrap(),
            );
            assert_eq!(
                output.contains("## Value relationships"),
                expected,
                "{tool} {args}: {output}"
            );
        }
        client.kill().await.unwrap();
    }
}

#[tokio::test]
async fn test_live_diagnostics_distinguish_empty_excluded_and_stale_source() {
    let temp=create_mock_repo(&[("src/empty.ts","// comment\n42;\n"),("src/defs.ts","export function beforeChange() {}\n"),("ignored/hidden.ts","export function hidden() {}\n"),(".codemap/config.toml","[update]\nconfig_auto_update=false\n[exclude]\nexcluded_directories=['ignored']\n[refresh]\nwatch=false\nindex_staleness_ms=600000\n")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    client
        .send_tool_until(
            "read",
            serde_json::json!({"file_path":"src/defs.ts","offset":1,"limit":1}),
            |text| text.contains("beforeChange [function"),
        )
        .await
        .unwrap();
    let empty = text(
        &client
            .send_request(
                "tools/call",
                call(
                    "read",
                    serde_json::json!({"file_path":"src/empty.ts","offset":1,"limit":2}),
                ),
            )
            .await
            .unwrap(),
    );
    assert!(
        empty.contains("No declaration context — src/empty.ts:1") && !empty.contains("Next: read"),
        "{empty}"
    );
    assert!(
        !empty.contains("No callable identified") && !empty.contains("No indexed declaration"),
        "{empty}"
    );
    let excluded = text(
        &client
            .send_request(
                "tools/call",
                call(
                    "read",
                    serde_json::json!({"file_path":"ignored/hidden.ts","offset":1,"limit":1}),
                ),
            )
            .await
            .unwrap(),
    );
    assert!(
        excluded.contains("excluded from the current index")
            && excluded.contains("1→export function hidden"),
        "{excluded}"
    );
    std::fs::write(
        temp.path().join("src/defs.ts"),
        "export function afterChange() {}\n",
    )
    .unwrap();
    let stale = text(
        &client
            .send_request(
                "tools/call",
                call(
                    "read",
                    serde_json::json!({"file_path":"src/defs.ts","offset":1,"limit":1}),
                ),
            )
            .await
            .unwrap(),
    );
    assert!(
        stale.contains("source changed since indexing") && !stale.contains("beforeChange"),
        "{stale}"
    );
    assert!(stale.contains("afterChange()"), "{stale}");
}

#[tokio::test]
async fn test_implementation_rust_target_reload_with_events_disabled() {
    let config = |target: &str| {
        format!("[update]\nconfig_auto_update=false\n[event_navigation]\nis_enabled=false\n[analysis]\ntarget_os='{target}'\n")
    };
    let temp=create_mock_repo(&[
        ("src/lib.rs","pub trait Runner { fn run(&self); }\npub struct Linux;\n#[cfg(target_os=\"linux\")]\nimpl Runner for Linux { fn run(&self) {} }\npub struct Mac;\n#[cfg(target_os=\"macos\")]\nimpl Runner for Mac { fn run(&self) {} }\n"),
        (".codemap/config.toml",&config("linux")),
    ]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args =
        serde_json::json!({"file_path":"src/lib.rs","offset":1,"limit":1,"view":"relations"});
    let output = text(
        &client
            .send_tool_until("read", args.clone(), |out| {
                out.contains("implementation candidate: Linux.run")
            })
            .await
            .unwrap(),
    );
    assert!(
        output.contains("Linux.run — L4")
            && !output.contains("Mac.run")
            && !output.contains("Event relationships"),
        "{output}"
    );
    std::fs::write(temp.path().join(".codemap/config.toml"), config("macos")).unwrap();
    let output = text(
        &client
            .send_tool_until("read", args, |out| {
                out.contains("implementation candidate: Mac.run")
            })
            .await
            .unwrap(),
    );
    assert!(
        output.contains("Mac.run — L7")
            && !output.contains("Linux.run")
            && !output.contains("Event relationships"),
        "{output}"
    );
}

#[tokio::test]
async fn test_implementation_context_cli_views_refresh_and_restart() {
    let implementation="import {Base} from './base';\nexport class Child extends Base { run(): number { return 1; } }\nexport function invoke(value: Base) { return value.run(); }\n";
    let temp=create_mock_repo(&[
        ("src/base.ts","export abstract class Base { abstract run(): number; }\nexport const unrelated = 'class Fake extends Base { run() {} }';\n"),
        ("src/child.ts",implementation),
        ("src/child.test.ts","import {Base} from './base'; class TestChild extends Base { run(): number { return 2; } }"),
        ("ignored/child.ts","import {Base} from '../src/base'; class IgnoredChild extends Base { run(): number { return 3; } }"),
        (".codemap/config.toml","[update]\nconfig_auto_update=false\n[exclude]\nexcluded_directories=['ignored']\n"),
    ]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args = serde_json::json!({"file_path":"src/base.ts","offset":1,"limit":1});
    let baseline = text(
        &client
            .send_tool_until("read", args.clone(), |out| {
                out.contains("implementation candidate: Child.run")
            })
            .await
            .unwrap(),
    );
    assert!(
        baseline.contains("Child.run — src/child.ts:2"),
        "{baseline}"
    );
    assert!(
        baseline.contains("declaration reference: invoke — src/child.ts:3"),
        "{baseline}"
    );
    assert!(
        !baseline.contains("TestChild")
            && !baseline.contains("IgnoredChild")
            && !baseline.contains("Fake.run"),
        "{baseline}"
    );
    let raw = baseline.split_once("\n### results\n").unwrap().1;
    for view in ["source", "definitions", "relations"] {
        let mut variant = args.clone();
        variant["view"] = view.into();
        let output = text(
            &client
                .send_request("tools/call", call("read", variant))
                .await
                .unwrap(),
        );
        assert_eq!(
            output.contains("## Implementations"),
            view == "relations",
            "{output}"
        );
        if view == "source" {
            assert_eq!(output, raw);
        }
    }
    for (tool, args) in [
        (
            "grep",
            serde_json::json!({"path":"src/child.ts","pattern":"value\\.run","view":"relations"}),
        ),
        (
            "search",
            serde_json::json!({"query":"Base","workspace_scope":"all"}),
        ),
    ] {
        let output = text(
            &client
                .send_request("tools/call", call(tool, args))
                .await
                .unwrap(),
        );
        assert!(output.contains("## Implementations"), "{output}");
        if tool == "grep" {
            assert!(output.contains("runtime target: unresolved"), "{output}");
        }
    }
    for (tool, args) in [
        (
            "read",
            serde_json::json!({"file_path":"src/base.ts","offset":2,"limit":1}),
        ),
        (
            "grep",
            serde_json::json!({"path":"src/base.ts","pattern":"unrelated"}),
        ),
        (
            "grep",
            serde_json::json!({"path":"src/child.ts","pattern":"run","output_mode":"files_with_matches"}),
        ),
    ] {
        let output = text(
            &client
                .send_request("tools/call", call(tool, args))
                .await
                .unwrap(),
        );
        assert!(!output.contains("## Implementations"), "{output}");
    }
    let output=text(&client.send_request("tools/call",call("search",serde_json::json!({"query":"Base","workspace_scope":"all","caller_context":false}))).await.unwrap());
    let relations = output.split_once("## Implementations").unwrap().1;
    assert!(
        !relations.contains("declaration reference:") && !relations.contains("call declaration:"),
        "{output}"
    );
    client.child.kill().await.unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let restarted = text(
        &client
            .send_tool_until("read", args.clone(), |out| {
                out.contains("implementation candidate: Child.run")
            })
            .await
            .unwrap(),
    );
    assert!(
        restarted.contains("Child.run — src/child.ts:2"),
        "{restarted}"
    );
    std::fs::write(
        temp.path().join("src/child.ts"),
        "export const replacement = true;\n",
    )
    .unwrap();
    let changed = text(
        &client
            .send_tool_until("read", args, |out| {
                out.contains("implementation: unresolved")
            })
            .await
            .unwrap(),
    );
    assert!(
        !changed.contains("Child.run") && !changed.contains("declaration reference: invoke"),
        "{changed}"
    );
}

#[tokio::test]
async fn test_events_live_modes_forward_reverse_and_schema() {
    let temp = crate::e2e::helpers::event_navigation_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args = serde_json::json!({"file_path":"src/users.ts","offset":2,"limit":1});
    let baseline = client
        .send_tool_until("read", args.clone(), |out| out.contains("save [function"))
        .await
        .unwrap();
    let baseline = text(&baseline);
    assert!(baseline.contains("Event relationships"), "{baseline}");
    let mut explicit = args.clone();
    explicit["include_events"] = true.into();
    assert_eq!(
        text(
            &client
                .send_request("tools/call", call("read", explicit))
                .await
                .unwrap()
        ),
        baseline
    );
    let mut disabled = args.clone();
    disabled["include_events"] = false.into();
    let disabled = text(
        &client
            .send_request("tools/call", call("read", disabled))
            .await
            .unwrap(),
    );
    assert!(!disabled.contains("Event relationships"), "{disabled}");
    let raw = baseline.split_once("\n### results\n").unwrap().1;
    for view in ["full", "source", "definitions", "relations"] {
        let mut variant = args.clone();
        variant["view"] = view.into();
        variant["include_events"] = true.into();
        let response = client
            .send_request("tools/call", call("read", variant))
            .await
            .unwrap();
        assert!(!is_error(&response), "{response}");
        let out = text(&response);
        if matches!(view, "source" | "definitions") {
            assert!(!out.contains("Event relationships"), "{out}");
            if view == "source" {
                assert_eq!(out, raw);
            }
        } else {
            let events = out.split_once("## Event relationships").unwrap().1;
            for expected in [
                "publisher: L2",
                "registration: src/events.ts:6",
                "handler definition: handleSaved — src/events.ts:5",
                "allocation:src/events.ts:2",
                "key evidence: SAVED — src/events.ts:4",
            ] {
                assert!(events.contains(expected), "{expected}: {out}");
            }
            assert!(!events.contains("src/other.ts"), "{out}");
            assert!(!events.contains("precise"), "{out}");
            if view == "full" {
                assert_eq!(out.split_once("\n### results\n").unwrap().1, raw);
            }
        }
    }
    for (tool, args) in [
        (
            "grep",
            serde_json::json!({"path":"src/events.ts","pattern":"appBus\\.on","view":"relations"}),
        ),
        (
            "read",
            serde_json::json!({"file_path":"src/events.ts","offset":5,"limit":1,"view":"relations"}),
        ),
    ] {
        let out = text(
            &client
                .send_request("tools/call", call(tool, args))
                .await
                .unwrap(),
        );
        assert!(
            out.contains("publisher: src/users.ts:2") && out.contains("registration: L6"),
            "{out}"
        );
    }
    // Ordinary code, unrelated methods, strings and comments in a file that
    // also contains real events must retain their full non-event context.
    for (tool, args) in [
        (
            "read",
            serde_json::json!({"file_path":"src/controls.ts","offset":11,"limit":3}),
        ),
        (
            "grep",
            serde_json::json!({"path":"src/controls.ts","pattern":"wrong-method|wrong-string|wrong-comment"}),
        ),
        (
            "grep",
            serde_json::json!({"path":"src","pattern":"saved","output_mode":"files_with_matches"}),
        ),
        (
            "grep",
            serde_json::json!({"path":"src","pattern":"saved","output_mode":"count"}),
        ),
    ] {
        let automatic = client
            .send_request("tools/call", call(tool, args.clone()))
            .await
            .unwrap();
        assert!(!is_error(&automatic), "{automatic}");
        let automatic = text(&automatic);
        assert!(!automatic.contains("Event relationships"), "{automatic}");
        assert!(!automatic.contains("Event output"), "{automatic}");
        let mut disabled = args;
        disabled["include_events"] = false.into();
        let disabled = client
            .send_request("tools/call", call(tool, disabled))
            .await
            .unwrap();
        assert!(!is_error(&disabled), "{disabled}");
        assert_eq!(automatic, text(&disabled));
    }
    let invalid=client.send_request("tools/call",call("grep",serde_json::json!({"path":"src","pattern":"saved","output_mode":"files_with_matches","include_events":true}))).await.unwrap();
    assert!(is_error(&invalid), "{invalid}");
    let invalid = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({"file_path":"src/users.ts","include_events":"yes"}),
            ),
        )
        .await
        .unwrap();
    assert!(is_error(&invalid));
    let schema = client
        .send_request("tools/list", serde_json::json!({}))
        .await
        .unwrap();
    for name in ["read", "grep", "search"] {
        let tool = schema["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        assert_eq!(
            tool["inputSchema"]["properties"]["include_events"]["default"],
            true
        );
    }
}

#[tokio::test]
async fn test_events_many_grep_anchors_do_not_spend_the_candidate_budget_twice() {
    let registrations = format!(
        "import {{bus}} from './bus'; function handler() {{}}\n{}",
        "bus.on('saved',handler);\n".repeat(50)
    );
    let temp = create_mock_repo(&[
        (
            "src/bus.ts",
            "import {EventEmitter} from 'events'; export const bus=new EventEmitter();",
        ),
        (
            "src/a_publish.ts",
            "import {bus} from './bus'; bus.emit('saved');",
        ),
        ("src/register.ts", &registrations),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update=false\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let response=client.send_tool_until("grep",serde_json::json!({"path":"src/register.ts","pattern":"bus\\.on","head_limit":100,"view":"relations"}),|out|out.contains("Event relationships")).await.unwrap();
    let out = text(&response);
    assert!(out.contains("publisher: src/a_publish.ts:1"), "{out}");
    assert!(out.contains("registration: L2"), "{out}");
    assert!(out.contains("Event output cap reached"), "{out}");
}

#[tokio::test]
async fn test_live_views_preserve_source_and_unresolved_totals() {
    let source = "const LIMIT: usize = 7;\nfn target() {\n    unknown_one();\n    object.unknown_two();\n    let value = LIMIT;\n}\nfn caller() { target(); }\n";
    let temp = create_mock_repo(&[
        ("src/lib.rs", source),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update=false\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args = serde_json::json!({"file_path":"src/lib.rs","offset":2,"limit":5});
    let baseline = client
        .send_tool_until("read", args.clone(), |out| {
            out.contains("2 callee(s) unresolved")
        })
        .await
        .unwrap();
    let full = text(&baseline);
    let raw = full.split_once("\n### results\n").unwrap().1;
    for view in ["source", "definitions", "relations", "full"] {
        let mut variant = args.clone();
        variant["view"] = view.into();
        let response = client
            .send_request("tools/call", call("read", variant))
            .await
            .unwrap();
        assert!(!is_error(&response), "{response}");
        let out = text(&response);
        match view {
            "source" => assert_eq!(out, raw),
            "definitions" => {
                assert!(out.contains("target [function"), "{out}");
                for absent in [
                    "# results",
                    "_calls",
                    "_callers",
                    "_references",
                    "unknown_one",
                    "unresolved",
                ] {
                    assert!(!out.contains(absent), "{out}");
                }
            }
            "relations" => {
                assert!(out.contains("\n### target relations\n"), "{out}");
                assert!(out.contains("caller (L7)"), "{out}");
                assert!(out.contains("LIMIT — L1 = 7"), "{out}");
                assert!(!out.contains("# results"), "{out}");
            }
            _ => {
                // Bounded value summaries can stop at a different point under load.
                // The declarations, call totals and live source contract is stable.
                let stable = |value: &str| {
                    let (context, source) = value.split_once("\n### results\n").unwrap();
                    let context = context
                        .split("\n#### Value relationships")
                        .next()
                        .unwrap()
                        .split("\n#### Analysis diagnostics")
                        .next()
                        .unwrap();
                    (context.trim_end().to_string(), source.to_string())
                };
                assert_eq!(stable(&out), stable(&full));
            }
        }
    }
    let count = client.send_request("tools/call", call("read", serde_json::json!({"file_path":"src/lib.rs","offset":2,"limit":5,"view":"relations","unresolved":"count"}))).await.unwrap();
    let out = text(&count);
    assert!(out.contains("2 callee(s) unresolved"), "{out}");
    assert!(
        !out.contains("unknown_one") && !out.contains("unknown_two"),
        "{out}"
    );
    let alias = client.send_request("tools/call", call("read", serde_json::json!({"path":"src/lib.rs","startLine":"2","end-line":"6","view":"source"}))).await.unwrap();
    assert_eq!(text(&alias), raw);
    let schema = client
        .send_request("tools/list", serde_json::json!({}))
        .await
        .unwrap();
    for tool in schema["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| ["read", "grep"].contains(&t["name"].as_str().unwrap()))
    {
        assert_eq!(tool["inputSchema"]["properties"]["view"]["default"], "full");
        assert_eq!(
            tool["inputSchema"]["properties"]["expand"]["default"],
            "none"
        );
        assert_eq!(
            tool["inputSchema"]["properties"]["unresolved"]["default"],
            "list"
        );
    }
    for args in [
        serde_json::json!({"pattern":"target","output_mode":"count","view":"source"}),
        serde_json::json!({"pattern":"target","output_mode":"files_with_matches","expand":"callable"}),
        serde_json::json!({"pattern":"target","view":"invalid"}),
    ] {
        let response = client
            .send_request("tools/call", call("grep", args))
            .await
            .unwrap();
        assert!(is_error(&response), "{response}");
    }
}

#[tokio::test]
async fn test_live_callable_expansion_deduplicates_and_uses_live_boundaries() {
    let source = "#[allow(dead_code)]\nfn first() {\n    let marker = 1;\n    let marker_two = 2;\n}\nfn neighbor() { do_not_include(); }\nfn last() {\n    let marker = 3;\n}\n";
    let temp = create_mock_repo(&[("src/lib.rs",source),(".codemap/config.toml","[update]\nconfig_auto_update=false\n[refresh]\nwatch=false\nindex_staleness_ms=600000\n")]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    client
        .send_tool_until(
            "read",
            serde_json::json!({"file_path":"src/lib.rs","offset":2,"limit":1}),
            |out| out.contains("first [function"),
        )
        .await
        .unwrap();
    let expanded = client.send_request("tools/call",call("read",serde_json::json!({"file_path":"src/lib.rs","offset":3,"limit":1,"end_line":9,"expand":"callable","view":"source"}))).await.unwrap();
    let out = text(&expanded);
    assert!(out.starts_with("     1→#[allow(dead_code)]"), "{out}");
    assert!(out.ends_with("     5→}"), "{out}");
    assert!(!out.contains("neighbor"), "{out}");
    let first = client.send_request("tools/call",call("grep",serde_json::json!({"path":"src/lib.rs","pattern":"marker","-A":50,"-B":10,"expand":"callable","head_limit":1,"view":"source"}))).await.unwrap();
    let out = text(&first);
    assert_eq!(out.matches("fn first()").count(), 1, "{out}");
    assert!(out.contains("next_offset=1"), "{out}");
    assert!(
        !out.contains("neighbor") && !out.contains("fn last"),
        "{out}"
    );
    let last = client.send_request("tools/call",call("grep",serde_json::json!({"path":"src/lib.rs","pattern":"marker","expand":"callable","head_limit":1,"offset":1,"view":"source"}))).await.unwrap();
    assert!(
        text(&last).contains("fn last") && !text(&last).contains("fn first"),
        "{last}"
    );
    std::fs::write(
        temp.path().join("src/lib.rs"),
        "fn inserted() {}\nfn fresh() {\n    let fresh_value = 1;\n}\nfn next() {}\n",
    )
    .unwrap();
    let fresh = client.send_request("tools/call",call("read",serde_json::json!({"file_path":"src/lib.rs","offset":3,"expand":"callable","view":"source"}))).await.unwrap();
    let out = text(&fresh);
    assert!(
        out.starts_with("     2→fn fresh()") && out.ends_with("     4→}"),
        "{out}"
    );
    assert!(
        !out.contains("inserted") && !out.contains("fn next"),
        "{out}"
    );
}

#[tokio::test]
async fn test_live_callable_shapes_and_unavailable_notices() {
    let source="\u{feff}#[allow(dead_code)]\r\nfn outer() {\r\n    let closure = || 1;\r\n    fn inner() {\r\n        let inner_value = 2;\r\n    }\r\n}\r\nfn after() {}\r\n";
    let temp = create_mock_repo(&[
        ("src/lib.rs", source),
        ("bad.rs", "fn broken( {\n"),
        ("same.rs", "fn one() {} fn two() {}\n"),
        ("shared_end.rs", "fn one() {\n} fn two() {}\n"),
        ("plain.txt", "a marker\n"),
        (
            "decorated.py",
            "@decorate\ndef target():\n    return 7\ndef next():\n    pass\n",
        ),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update=false\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    for (path, line, start, end, absent) in [
        ("src/lib.rs", 1, 1, 7, "fn after"),
        ("src/lib.rs", 3, 1, 7, "fn after"),
        ("src/lib.rs", 5, 4, 6, "fn outer"),
        ("decorated.py", 2, 1, 3, "def next"),
    ] {
        let response=client.send_request("tools/call",call("read",serde_json::json!({"file_path":path,"offset":line,"expand":"callable","view":"source"}))).await.unwrap();
        let out = text(&response);
        assert!(out.starts_with(&format!("{start:>6}→")), "{out}");
        assert!(
            out.lines()
                .last()
                .unwrap()
                .starts_with(&format!("{end:>6}→")),
            "{out}"
        );
        assert!(
            !out.contains(absent) && !out.contains('\r') && !out.contains('\u{feff}'),
            "{out}"
        );
    }
    for path in ["bad.rs", "same.rs", "shared_end.rs", "plain.txt"] {
        let response=client.send_request("tools/call",call("read",serde_json::json!({"file_path":path,"offset":1,"limit":1,"expand":"callable","view":"source"}))).await.unwrap();
        let out = text(&response);
        assert!(
            out.contains("Callable expansion unavailable") && out.contains("     1→"),
            "{path}: {out}"
        );
    }
    let none=client.send_request("tools/call",call("grep",serde_json::json!({"path":"src","pattern":"no_such_marker","expand":"callable","view":"source"}))).await.unwrap();
    assert!(text(&none).contains("No matches found"), "{none}");
}

#[tokio::test]
async fn test_live_callable_caps_are_explicit_and_continuable() {
    let source = format!(
        "fn large() {{\n{}\n}}\nfn small() {{}}\n",
        (0..60)
            .map(|n| format!("    let value_{n} = {n};"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let temp = create_mock_repo(&[
        ("src/lib.rs", source.as_str()),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update=false\n[tool_output]\nread_output_byte_cap=1024\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let read=client.send_request("tools/call",call("read",serde_json::json!({"file_path":"src/lib.rs","offset":2,"limit":1,"expand":"callable","view":"source"}))).await.unwrap();
    assert!(
        is_error(&read) && read.to_string().contains("expand=none"),
        "{read}"
    );
    let page=client.send_request("tools/call",call("grep",serde_json::json!({"path":"src/lib.rs","pattern":"fn ","expand":"callable","head_limit":1,"view":"source"}))).await.unwrap();
    let out = text(&page);
    assert!(
        out.len() <= 1024
            && out.contains("Callable body unavailable")
            && out.contains("next_offset=1"),
        "{out}"
    );
    let next=client.send_request("tools/call",call("grep",serde_json::json!({"path":"src/lib.rs","pattern":"fn ","expand":"callable","head_limit":1,"offset":1,"view":"source"}))).await.unwrap();
    assert!(text(&next).contains("fn small"), "{next}");
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
    let (context, raw) = out.split_once("\n### results\n").unwrap();
    assert!(context.contains("read_layer — L4"), "{out}");
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
        context.contains("CODEMAP_DIR_NAME — L1 = \".codemap\""),
        "{out}"
    );
    assert!(
        context.contains("CONFIG_FILE_NAME — L2 = \"config.toml\""),
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
    let (context, raw) = grep_out.split_once("\n### results\n").unwrap();
    assert!(context.contains("read_layer — L4"), "{grep_out}");
    assert!(
        context.contains("CODEMAP_DIR_NAME — L1 = \".codemap\""),
        "{grep_out}"
    );
    assert!(
        raw.contains("6:    read_layer(CODEMAP_DIR_NAME);") && !raw.contains("src/config.rs:"),
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
    assert!(out.contains("[Symbol context budget:"), "{out}");
    assert_eq!(
        out.split_once("\n### results\n").unwrap().1.trim_end(),
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
            let context = out.split_once("\n### results\n").map(|(context, _)| context).unwrap_or("");
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
            let (context, raw) = out.split_once("\n### results\n").unwrap();
            assert_eq!(context.contains(name), should_show, "phase {phase}, {path}: {out}");
            if phase == 0 { raw_results.push(raw.to_string()); }
            else { assert_eq!(raw, raw_results[index], "raw source changed for {path}"); }
        }
        let helper = client.send_request("tools/call", call("read", serde_json::json!({
            "file_path": "src/inline.rs", "offset": 3, "limit": 1
        }))).await.unwrap();
        let helper_out = text(&helper);
        let context = helper_out.split_once("\n### results\n").unwrap().0;
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
        out.starts_with("# codemap-search\n")
            && out
                .split_once("\n### results\n")
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
    let aliased = text(&backslash_resp);
    let stable_parts = |value: &str| {
        let (context, source) = value.split_once("\n### results\n").unwrap();
        let declarations = context
            .lines()
            .filter(|line| line.starts_with("## ") || line.contains(" [function, "))
            .collect::<Vec<_>>()
            .join("\n");
        (declarations, source.to_string())
    };
    assert_eq!(stable_parts(&aliased), stable_parts(&out),
        "path aliases preserve file identity, declarations and source; optional analysis can hit its time budget");
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
    std::fs::write(temp.path().join("data.qqq"), b"text\0binary\xff").unwrap();
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
        out.contains("text") && out.contains("binary�"),
        "content surfaced: {out}"
    );
}

#[tokio::test]
async fn test_non_utf8_source_explains_index_exclusion_without_hiding_read() {
    let temp = create_mock_repo(&[("healthy.rs", "pub fn healthy() {}\n")]).unwrap();
    let bytes = b"// legacy \xff\npub fn excluded_encoding() {}\n";
    std::fs::write(temp.path().join("legacy.rs"), bytes).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let resp = client
        .send_tool_until(
            "read",
            serde_json::json!({"file_path":"legacy.rs"}),
            |out| out.contains("Not indexed: invalid UTF-8"),
        )
        .await
        .unwrap();
    let out = text(&resp);
    assert!(!is_error(&resp), "{resp}");
    assert!(
        out.contains("1→// legacy �") && out.contains("2→pub fn excluded_encoding()"),
        "{out}"
    );
    assert!(!out.contains("No indexed declaration or callable"), "{out}");
    let overview = client
        .send_request(
            "tools/call",
            call("overview", serde_json::json!({"path":"legacy.rs"})),
        )
        .await
        .unwrap();
    assert!(
        overview.to_string().contains("Not indexed: invalid UTF-8"),
        "{overview}"
    );
    assert_eq!(std::fs::read(temp.path().join("legacy.rs")).unwrap(), bytes);
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
    // The file heading owns the path; source rows keep their exact line numbers.
    assert!(
        out.lines()
            .any(|l| l.starts_with("## ") && l.ends_with("src/util.rs"))
            && out
                .lines()
                .any(|l| l.starts_with("2:") && l.contains("TODO"))
            && !out.contains("src/util.rs:2:"),
        "file heading and pathless `line:text` expected by default: {out:?}"
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
    // A grouped result must not repeat its file heading on every source row.
    assert!(
        out.lines()
            .any(|l| l.starts_with("## ") && l.ends_with("src/core.rs"))
            && out
                .lines()
                .any(|l| l.starts_with("1:") && l.contains("run_engine"))
            && !out.contains("src/core.rs:1:"),
        "file heading and pathless `line:text` expected: {out:?}"
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

#[tokio::test]
#[ignore = "Requires an installed Clang preprocessor"]
async fn test_macro_expansion_reaches_search_read_and_refreshes_header_changes() {
    let source =
        "#include \"macros.h\"\nDECLARE(chosen);\nint ordinary(void) { return chosen_v1(); }\n";
    let config = "[update]\nconfig_auto_update = false\n[macro_expansion]\nis_enabled = true\ntimeout_ms = 5000\n[exclude]\nshould_include_test_code = true\n[refresh]\nwatch_debounce_ms = 20\n";
    let temp = create_mock_repo(&[
        ("probe.c", source),
        (
            "macros.h",
            "#define DECLARE(name) int name##_v1(void) { return 7; }\n",
        ),
        (".codemap/config.toml", config),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let overview = client
        .send_tool_until("overview", serde_json::json!({"path": "probe.c"}), |out| {
            out.contains("chosen_v1 (fn)")
        })
        .await
        .unwrap();
    assert!(text(&overview).contains("Macro expansion:"), "{overview}");
    let read = client
        .send_request(
            "tools/call",
            call(
                "read",
                serde_json::json!({"file_path": "probe.c", "offset": 2, "limit": 1}),
            ),
        )
        .await
        .unwrap();
    let read = text(&read);
    assert!(read.contains("chosen_v1 [function"), "{read}");
    assert!(read.contains("2→DECLARE(chosen);"), "{read}");
    assert!(read.contains("Macro expansion:"), "{read}");
    assert!(read.contains("macro expansion]"), "{read}");
    assert!(
        read.contains("attribution unresolved after preprocessing"),
        "{read}"
    );
    assert!(!read.contains("(precise)"), "{read}");
    let search = client
        .send_request(
            "tools/call",
            call(
                "search",
                serde_json::json!({"query": "chosen_v1", "caller_context": false}),
            ),
        )
        .await
        .unwrap();
    assert!(text(&search).contains("probe.c"), "{search}");
    assert!(text(&search).contains("Macro expansion:"), "{search}");
    std::fs::write(
        temp.path().join("macros.h"),
        "#define DECLARE(name) int name##_v2(void) { return 8; }\n",
    )
    .unwrap();
    let changed = client
        .send_tool_until("overview", serde_json::json!({"path": "probe.c"}), |out| {
            out.contains("chosen_v2 (fn)") && !out.contains("chosen_v1 (fn)")
        })
        .await
        .unwrap();
    assert!(text(&changed).contains("[L2-2]"), "{changed}");
    std::fs::write(
        temp.path().join(".codemap/config.toml"),
        config.replace("is_enabled = true", "is_enabled = false"),
    )
    .unwrap();
    let disabled = client
        .send_tool_until("overview", serde_json::json!({"path": "probe.c"}), |out| {
            out.contains("ordinary (fn)")
                && !out.contains("Macro expansion:")
                && !out.contains("chosen_v2 (fn)")
        })
        .await
        .unwrap();
    assert!(!text(&disabled).contains("chosen_v2 (fn)"));
    assert_eq!(
        std::fs::read_to_string(temp.path().join("probe.c")).unwrap(),
        source
    );
}
