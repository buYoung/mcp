//! E2E coverage for the filesystem-watcher refresh path (`src/watcher.rs`).
//!
//! Isolation technique: `index_staleness_ms` is set to one hour, so after the very first
//! request at most one request-triggered refresh could ever fire (and with a healthy
//! watcher even that is suppressed). Any post-edit change that shows up in search/overview
//! results therefore got there through the watcher, not through the request fallback.
//! The `watch = false` test inverts this: a tiny staleness window and a disabled watcher
//! must reproduce the pre-watcher request-triggered behavior exactly.

use crate::e2e::helpers::{create_mock_repo, McpClient};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Repo config that leaves only the watcher able to refresh (see module docs).
const WATCHER_ONLY_CONFIG: &str = "\
index_staleness_ms = 3600000
watch_debounce_ms = 100
[language_support]
is_shell_support_enabled = true
is_infrastructure_support_enabled = true
is_interface_support_enabled = true
is_build_support_enabled = true
";

/// Settle window between the first request and the test's mutation. If the suppression
/// gate ever regressed, that first request could seed ONE request-fallback refresh; on
/// these tiny repos any such pass finishes well within this window, so a post-mutation
/// reflection can only have come from the watcher (no race-through false pass).
async fn let_seeded_refresh_settle() {
    tokio::time::sleep(Duration::from_millis(1500)).await;
}

#[tokio::test]
async fn test_events_watcher_dependencies_rules_filters_and_restart() {
    use crate::e2e::helpers::{event_navigation_repo, EVENT_NAVIGATION_CONFIG};
    let temp = event_navigation_repo();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args = |key: &str| serde_json::json!({"query":key,"event_key":key});
    let text = |response: serde_json::Value| {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    let saved = text(
        client
            .send_tool_until("search", args("saved"), |out| {
                out.contains("publisher: src/users.ts:2")
            })
            .await
            .unwrap(),
    );
    assert!(saved.contains("registration: src/events.ts:6"), "{saved}");
    let_seeded_refresh_settle().await;
    // Imported constants change both endpoints even though users.ts is unchanged.
    let events = fs::read_to_string(temp.path().join("src/events.ts")).unwrap();
    fs::write(
        temp.path().join("src/events.ts"),
        events.replace("'saved'", "'changed'"),
    )
    .unwrap();
    let changed = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("publisher: src/users.ts:2")
            })
            .await
            .unwrap(),
    );
    assert!(
        changed.contains("registration: src/events.ts:6"),
        "{changed}"
    );
    let previous = text(
        client
            .send_request(
                "tools/call",
                serde_json::json!({"name":"search","arguments":args("saved")}),
            )
            .await
            .unwrap(),
    );
    assert!(
        !previous.contains("src/users.ts") && !previous.contains("registration:"),
        "{previous}"
    );
    // Reexporting a different allocation must split the previously joined route.
    fs::write(
        temp.path().join("src/barrel.ts"),
        "export {otherBus as appBus, SAVED} from './events';\n",
    )
    .unwrap();
    let split = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.matches("### Event").count() == 2 && out.contains("publisher: src/users.ts:2")
            })
            .await
            .unwrap(),
    );
    assert_eq!(split.matches("### Event").count(), 2, "{split}");
    for section in split.split("### Event").skip(1) {
        assert!(
            !(section.contains("src/users.ts") && section.contains("registration:")),
            "{split}"
        );
    }
    fs::write(
        temp.path().join("src/new.ts"),
        "import {appBus,SAVED} from './events'; appBus.emit(SAVED);\n",
    )
    .unwrap();
    let created = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("publisher: src/new.ts:1")
            })
            .await
            .unwrap(),
    );
    assert!(created.contains("publisher: src/new.ts:1"), "{created}");
    fs::remove_file(temp.path().join("src/new.ts")).unwrap();
    // A live freshness check can hide a just-deleted endpoint before the watcher commits.
    let deleted = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                !out.contains("src/new.ts")
                    && out.contains("src/users.ts")
                    && !out.contains("stale/unverified")
            })
            .await
            .unwrap(),
    );
    assert!(!deleted.contains("src/new.ts"), "{deleted}");
    fs::write(
        temp.path().join("src/events.test.ts"),
        "import {appBus,SAVED} from './events'; appBus.emit(SAVED);\n",
    )
    .unwrap();
    let filtered = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("test/exclusion-filtered endpoints")
            })
            .await
            .unwrap(),
    );
    assert!(
        !filtered.contains("publisher: src/events.test.ts"),
        "{filtered}"
    );
    let config_path = temp.path().join(".codemap/config.toml");
    fs::write(
        &config_path,
        format!("{EVENT_NAVIGATION_CONFIG}\n[exclude]\nshould_include_test_code=true\n"),
    )
    .unwrap();
    let included = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("publisher: src/events.test.ts:1")
            })
            .await
            .unwrap(),
    );
    assert!(
        included.contains("publisher: src/events.test.ts:1"),
        "{included}"
    );
    fs::create_dir_all(temp.path().join("hidden")).unwrap();
    fs::write(
        temp.path().join("hidden/publisher.ts"),
        "import {appBus,SAVED} from '../src/events'; appBus.emit(SAVED);",
    )
    .unwrap();
    let visible = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("publisher: hidden/publisher.ts:1")
            })
            .await
            .unwrap(),
    );
    assert!(
        visible.contains("publisher: hidden/publisher.ts:1"),
        "{visible}"
    );
    fs::write(
        &config_path,
        format!("{EVENT_NAVIGATION_CONFIG}\n[exclude]\nexcluded_directories=['hidden']\n"),
    )
    .unwrap();
    let excluded = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("src/users.ts")
                    && !out.contains("hidden/publisher.ts")
                    && !out.contains("stale/unverified")
            })
            .await
            .unwrap(),
    );
    assert!(!excluded.contains("hidden/publisher.ts"), "{excluded}");
    // Rules are reloaded/removed without touching API source; disabled builtins also apply.
    fs::write(&config_path,"[update]\nconfig_auto_update=false\n[event_navigation]\nis_enabled=true\nuse_builtin_rules=false\nrules=[]\n").unwrap();
    let removed = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("No eligible event endpoints")
            })
            .await
            .unwrap(),
    );
    assert!(removed.contains("No eligible event endpoints"), "{removed}");
    fs::write(&config_path, EVENT_NAVIGATION_CONFIG).unwrap();
    let restored = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("publisher: src/users.ts:2")
            })
            .await
            .unwrap(),
    );
    assert!(restored.contains("publisher: src/users.ts:2"), "{restored}");
    client.kill().await.unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let restarted = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("publisher: src/users.ts:2")
            })
            .await
            .unwrap(),
    );
    assert!(
        restarted.contains("registration: src/events.ts:6"),
        "{restarted}"
    );
    fs::write(
        &config_path,
        EVENT_NAVIGATION_CONFIG.replace("is_enabled = true", "is_enabled = false"),
    )
    .unwrap();
    let disabled = text(
        client
            .send_tool_until("search", args("changed"), |out| {
                out.contains("Event navigation disabled")
            })
            .await
            .unwrap(),
    );
    assert!(
        disabled.contains("Event navigation disabled") && !disabled.contains("publisher:"),
        "{disabled}"
    );
}

#[tokio::test]
async fn test_events_target_global_rules_and_git_exclusions_reload() {
    let global = r#"
[event_navigation]
is_enabled=true
use_builtin_rules=false
rules=[
{id='rust',language='rust',module='src/output.rs',symbol='send',role='publish',event_arg=0,bus='fixed',bus_identity='app',target='any'},
{id='frontend',language='typescript',module='@tauri-apps/api/event',symbol='listen',role='subscribe',event_arg=0,handler_arg=1,bus='fixed',bus_identity='app'}
]
"#;
    let config = |target: &str| {
        format!("[update]\nconfig_auto_update=false\n[analysis]\ntarget_os='{target}'\n[exclude]\nuse_git_exclude=true\n")
    };
    let temp = create_mock_repo(&[
        (
            "Cargo.toml",
            "[package]\nname='event-target'\nversion='0.1.0'\n",
        ),
        (
            "src/lib.rs",
            "mod output;\n#[cfg(target_os=\"macos\")]\nmod publish;\n",
        ),
        ("src/output.rs", "pub fn send(key: &str) {}"),
        (
            "src/publish.rs",
            "use crate::output::send; pub fn publish() { send(\"saved\"); }",
        ),
        (
            "src/listen.ts",
            "import {listen} from '@tauri-apps/api/event'; listen('saved',()=>{});",
        ),
        ("config.toml", global),
        (".codemap/config.toml", &config("")),
    ])
    .unwrap();
    run_git(temp.path(), &["init", "-q"]);
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let args = serde_json::json!({"query":"saved","event_key":"saved"});
    let text = |response: serde_json::Value| {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    let neutral = text(
        client
            .send_tool_until("search", args.clone(), |out| out.contains("registration:"))
            .await
            .unwrap(),
    );
    assert!(!neutral.contains("publisher:"), "{neutral}");
    for (target, has_publisher) in [("macos", true), ("linux", false), ("macos", true)] {
        fs::write(temp.path().join(".codemap/config.toml"), config(target)).unwrap();
        let out = text(
            client
                .send_tool_until("search", args.clone(), |out| {
                    out.contains(&format!("target_os={target}"))
                        && out.contains("registration:")
                        && out.contains("publisher:") == has_publisher
                })
                .await
                .unwrap(),
        );
        assert_eq!(out.contains("publisher:"), has_publisher, "{out}");
        assert!(out.contains("registration:"), "{out}");
    }
    // An invalid repo selector falls back to the global list through the live consumer.
    fs::write(temp.path().join(".codemap/config.toml"),format!("{}\n[event_navigation]\nrules=[{{id='bad',language='rust',module='*',symbol='send',role='publish',event_arg=0,bus='fixed',bus_identity='wrong'}}]\n",config("macos"))).unwrap();
    let fallback = text(
        client
            .send_tool_until("search", args.clone(), |out| {
                out.contains("publisher:") && out.contains("configured:app")
            })
            .await
            .unwrap(),
    );
    assert!(fallback.contains("registration:"), "{fallback}");
    fs::write(
        temp.path().join("config.toml"),
        global.replace("bus_identity='app'", "bus_identity='reloaded'"),
    )
    .unwrap();
    let reloaded = text(
        client
            .send_tool_until("search", args.clone(), |out| {
                out.contains("configured:reloaded") && out.contains("publisher:")
            })
            .await
            .unwrap(),
    );
    assert!(reloaded.contains("registration:"), "{reloaded}");
    fs::write(temp.path().join(".git/info/exclude"), "src/publish.rs\n").unwrap();
    let excluded = text(
        client
            .send_tool_until("search", args.clone(), |out| {
                out.contains("registration:") && !out.contains("publisher:")
            })
            .await
            .unwrap(),
    );
    assert!(!excluded.contains("publisher:"), "{excluded}");
    fs::write(
        temp.path().join(".codemap/config.toml"),
        config("macos").replace("use_git_exclude=true", "use_git_exclude=false"),
    )
    .unwrap();
    let widened = text(
        client
            .send_tool_until("search", args.clone(), |out| {
                out.contains("publisher:") && out.contains("configured:reloaded")
            })
            .await
            .unwrap(),
    );
    assert!(widened.contains("publisher:"), "{widened}");
    fs::write(
        temp.path().join(".codemap/config.toml"),
        format!("{}\n[event_navigation]\nrules=[]\n", config("macos")),
    )
    .unwrap();
    let removed = text(
        client
            .send_tool_until("search", args, |out| {
                out.contains("No eligible event endpoints")
            })
            .await
            .unwrap(),
    );
    assert!(removed.contains("No eligible event endpoints"), "{removed}");
}

/// Run a git command in `cwd`, panicking with context on failure (test setup only).
fn run_git(cwd: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} failed to spawn: {e}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn test_watcher_autonomous_modify_refresh() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (".codemap/config.toml", WATCHER_ONLY_CONFIG),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

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

    let_seeded_refresh_settle().await;

    // Edit WITHOUT any further search/overview involvement in refreshing: the one-hour
    // staleness window means only the watcher can pick this up.
    fs::write(
        temp.path().join("src/lib.rs"),
        "pub fn find_something_else() {}",
    )
    .unwrap();

    let res_2 = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "find_something_else" }),
            |t| t.contains("lib.rs"),
        )
        .await
        .unwrap();
    assert!(
        res_2["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("lib.rs"),
        "watcher must reflect the edit without a request-triggered refresh"
    );
}

#[tokio::test]
async fn test_watcher_incremental_delete() {
    let temp = create_mock_repo(&[
        ("src/alpha.rs", "pub fn alpha_symbol() {}"),
        ("src/beta.rs", "pub fn beta_symbol() {}"),
        (".codemap/config.toml", WATCHER_ONLY_CONFIG),
    ])
    .unwrap();

    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let res_1 = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "beta_symbol" }
            }),
        )
        .await
        .unwrap();
    assert!(res_1["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("beta.rs"));

    let_seeded_refresh_settle().await;

    fs::remove_file(temp.path().join("src/beta.rs")).unwrap();

    // The remove event must delete just that path from the index (no set-difference over
    // the whole index), while the untouched sibling stays searchable.
    let res_2 = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "beta_symbol" }),
            |t| !t.contains("beta.rs"),
        )
        .await
        .unwrap();
    assert!(
        !res_2["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("beta.rs"),
        "deleted file must leave the index via the watcher remove event"
    );

    let res_3 = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "alpha_symbol" }
            }),
        )
        .await
        .unwrap();
    assert!(
        res_3["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("alpha.rs"),
        "sibling file must survive the path-scoped delete"
    );
}

#[tokio::test]
async fn test_watcher_branch_switch_full_walk() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn main_branch_symbol() {}"),
        (".codemap/config.toml", WATCHER_ONLY_CONFIG),
    ])
    .unwrap();
    let repo = temp.path();

    run_git(repo, &["init", "-b", "main"]);
    run_git(repo, &["config", "user.email", "e2e@example.com"]);
    run_git(repo, &["config", "user.name", "e2e"]);
    // Keep the index dir out of git so checkout never touches it.
    fs::write(repo.join(".gitignore"), ".codemap/\n").unwrap();
    run_git(repo, &["add", "."]);
    run_git(repo, &["commit", "-m", "main"]);

    run_git(repo, &["checkout", "-b", "feature"]);
    fs::write(repo.join("src/lib.rs"), "pub fn feature_branch_symbol() {}").unwrap();
    fs::write(repo.join("src/extra.rs"), "pub fn extra_symbol() {}").unwrap();
    run_git(repo, &["add", "."]);
    run_git(repo, &["commit", "-m", "feature"]);

    let mut client = McpClient::spawn(repo).await.unwrap();

    let res_1 = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "extra_symbol" }
            }),
        )
        .await
        .unwrap();
    assert!(res_1["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("extra.rs"));

    let_seeded_refresh_settle().await;

    // Branch switch: lib.rs content reverts and extra.rs disappears. The HEAD-change hint
    // escalates the batch to a full-walk pass that must land all of it.
    run_git(repo, &["checkout", "main"]);

    let res_2 = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "main_branch_symbol" }),
            |t| t.contains("lib.rs"),
        )
        .await
        .unwrap();
    assert!(
        res_2["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("lib.rs"),
        "the switched-to branch's working tree must be searchable"
    );

    let res_3 = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "extra_symbol" }),
            |t| !t.contains("extra.rs"),
        )
        .await
        .unwrap();
    assert!(
        !res_3["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("extra.rs"),
        "a file absent on the switched-to branch must leave the index"
    );
}

#[tokio::test]
async fn test_watch_false_preserves_request_triggered_refresh() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        // Watcher off + near-zero debounce: the pre-watcher lazy behavior, verbatim.
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

    fs::write(
        temp.path().join("src/lib.rs"),
        "pub fn find_something_else() {}",
    )
    .unwrap();

    let res_2 = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "find_something_else" }),
            |t| t.contains("lib.rs"),
        )
        .await
        .unwrap();
    assert!(
        res_2["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("lib.rs"),
        "watch=false must keep the request-triggered refresh working"
    );
}

#[tokio::test]
async fn test_watcher_refreshes_priority_format_create_modify_and_delete() {
    let temp = create_mock_repo(&[(".codemap/config.toml", WATCHER_ONLY_CONFIG)]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    // Seed the first request, then mutate without relying on request-triggered refresh.
    client.send_request("tools/call", serde_json::json!({ "name": "search", "arguments": { "query": "priority_watch_created" } })).await.unwrap();
    let_seeded_refresh_settle().await;
    let config = temp.path().join("config.yaml");
    fs::write(&config, "value: priority_watch_created\n").unwrap();
    client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "priority_watch_created" }),
            |text| text.contains("config.yaml"),
        )
        .await
        .unwrap();
    fs::write(&config, "value: priority_watch_updated\n").unwrap();
    client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "priority_watch_updated" }),
            |text| text.contains("config.yaml"),
        )
        .await
        .unwrap();
    fs::remove_file(&config).unwrap();
    let removed = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "priority_watch_updated" }),
            |text| !text.contains("config.yaml"),
        )
        .await
        .unwrap();
    assert!(!removed["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("config.yaml"));
}

#[tokio::test]
async fn test_watcher_refreshes_programming_languages_on_create_modify_and_delete() {
    let temp = create_mock_repo(&[(".codemap/config.toml", WATCHER_ONLY_CONFIG)]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "fifth_priority_watch_seed" }
            }),
        )
        .await
        .unwrap();
    let_seeded_refresh_settle().await;

    for (path, created_source, created_symbol, updated_source, updated_symbol) in [
        (
            "watch.rs",
            "pub struct WatchCreatedRust;\n",
            "WatchCreatedRust",
            "pub struct WatchUpdatedRust;\n",
            "WatchUpdatedRust",
        ),
        (
            "watch.py",
            "class WatchCreatedPython:\n    pass\n",
            "WatchCreatedPython",
            "class WatchUpdatedPython:\n    pass\n",
            "WatchUpdatedPython",
        ),
        (
            "watch.ts",
            "export class WatchCreatedTypeScript {}\n",
            "WatchCreatedTypeScript",
            "export class WatchUpdatedTypeScript {}\n",
            "WatchUpdatedTypeScript",
        ),
        (
            "watch.js",
            "export class WatchCreatedJavaScript {}\n",
            "WatchCreatedJavaScript",
            "export class WatchUpdatedJavaScript {}\n",
            "WatchUpdatedJavaScript",
        ),
        (
            "watch.go",
            "package watch\ntype WatchCreatedGo struct{}\n",
            "WatchCreatedGo",
            "package watch\ntype WatchUpdatedGo struct{}\n",
            "WatchUpdatedGo",
        ),
        (
            "Watch.java",
            "class WatchCreatedJava {}\n",
            "WatchCreatedJava",
            "class WatchUpdatedJava {}\n",
            "WatchUpdatedJava",
        ),
        (
            "Watch.kt",
            "class WatchCreatedKotlin\n",
            "WatchCreatedKotlin",
            "class WatchUpdatedKotlin\n",
            "WatchUpdatedKotlin",
        ),
        (
            "watch.c",
            "struct WatchCreatedC { int value; };\n",
            "WatchCreatedC",
            "struct WatchUpdatedC { int value; };\n",
            "WatchUpdatedC",
        ),
        (
            "watch.cpp",
            "class WatchCreatedCpp {};\n",
            "WatchCreatedCpp",
            "class WatchUpdatedCpp {};\n",
            "WatchUpdatedCpp",
        ),
        (
            "watch.s",
            ".globl WatchCreatedAsm\nWatchCreatedAsm:\n  ret\n",
            "WatchCreatedAsm",
            ".globl WatchUpdatedAsm\nWatchUpdatedAsm:\n  ret\n",
            "WatchUpdatedAsm",
        ),
        (
            "Watch.cs",
            "public class WatchCreatedCSharp {}\n",
            "WatchCreatedCSharp",
            "public class WatchUpdatedCSharp {}\n",
            "WatchUpdatedCSharp",
        ),
        (
            "watch.php",
            "<?php class WatchCreatedPhp {}\n",
            "WatchCreatedPhp",
            "<?php class WatchUpdatedPhp {}\n",
            "WatchUpdatedPhp",
        ),
        (
            "watch.rb",
            "class WatchCreatedRuby\nend\n",
            "WatchCreatedRuby",
            "class WatchUpdatedRuby\nend\n",
            "WatchUpdatedRuby",
        ),
        (
            "watch.lua",
            "function WatchCreatedLua() end\n",
            "WatchCreatedLua",
            "function WatchUpdatedLua() end\n",
            "WatchUpdatedLua",
        ),
        (
            "Watch.swift",
            "public struct WatchCreatedSwift {}\n",
            "WatchCreatedSwift",
            "public struct WatchUpdatedSwift {}\n",
            "WatchUpdatedSwift",
        ),
        (
            "watch.dart",
            "class WatchCreatedDart {}\n",
            "WatchCreatedDart",
            "class WatchUpdatedDart {}\n",
            "WatchUpdatedDart",
        ),
        (
            "Watch.scala",
            "class WatchCreatedScala\n",
            "WatchCreatedScala",
            "class WatchUpdatedScala\n",
            "WatchUpdatedScala",
        ),
        (
            "watch.sc",
            "object WatchCreatedScalaScript\n",
            "WatchCreatedScalaScript",
            "object WatchUpdatedScalaScript\n",
            "WatchUpdatedScalaScript",
        ),
        (
            "Watch.groovy",
            "class WatchCreatedGroovy {}\n",
            "WatchCreatedGroovy",
            "class WatchUpdatedGroovy {}\n",
            "WatchUpdatedGroovy",
        ),
        (
            "build.gradle",
            "task('WatchCreatedGradle')\n",
            "WatchCreatedGradle",
            "task('WatchUpdatedGradle')\n",
            "WatchUpdatedGradle",
        ),
        (
            "watch.ps1",
            "function WatchCreatedPowerShell {}\n",
            "WatchCreatedPowerShell",
            "function WatchUpdatedPowerShell {}\n",
            "WatchUpdatedPowerShell",
        ),
        (
            "watch.psm1",
            "function WatchCreatedModule {}\n",
            "WatchCreatedModule",
            "function WatchUpdatedModule {}\n",
            "WatchUpdatedModule",
        ),
    ] {
        let file = temp.path().join(path);
        fs::write(&file, created_source).unwrap();
        client
            .send_tool_until(
                "search",
                serde_json::json!({ "query": created_symbol }),
                |text| text.contains(path),
            )
            .await
            .unwrap();

        fs::write(&file, updated_source).unwrap();
        client
            .send_tool_until(
                "search",
                serde_json::json!({ "query": updated_symbol }),
                |text| text.contains(path),
            )
            .await
            .unwrap();

        fs::remove_file(&file).unwrap();
        let removed = client
            .send_tool_until(
                "search",
                serde_json::json!({ "query": updated_symbol }),
                |text| !text.contains(path),
            )
            .await
            .unwrap();
        assert!(
            !removed["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(path),
            "{path} should be removed from the watcher index"
        );
    }
}

#[tokio::test]
async fn test_lsr_007_watcher_covers_supported_priority_inputs() {
    let temp = create_mock_repo(&[(".codemap/config.toml", WATCHER_ONLY_CONFIG)]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    client
        .send_request(
            "tools/call",
            serde_json::json!({ "name": "search", "arguments": { "query": "lsr_watcher_seed" } }),
        )
        .await
        .unwrap();
    let_seeded_refresh_settle().await;

    // Each priority reaches the same watcher → index → MCP-search final consumer. Exact-name
    // formats use the same grammar-backed indexing path as extension-based formats.
    for (file, body) in [
        ("config.json", "{\"value\": \"needle\"}"),
        ("config.jsonc", "// needle\n{}"),
        ("settings.toml", "value = \"needle\""),
        ("config.yaml", "value: needle"),
        ("config.yml", "value: needle"),
        ("page.html", "<main>needle</main>"),
        ("page.htm", "<main>needle</main>"),
        ("page.xml", "<main>needle</main>"),
        ("schema.xsd", "<main>needle</main>"),
        ("page.xsl", "<main>needle</main>"),
        ("page.xslt", "<main>needle</main>"),
        ("Info.plist", "<main>needle</main>"),
        ("app.csproj", "<main>needle</main>"),
        ("app.props", "<main>needle</main>"),
        ("app.targets", "<main>needle</main>"),
        ("site.css", ".needle {}"),
        ("deploy.sh", "echo needle"),
        ("deploy.bash", "echo needle"),
        ("main.hcl", "value = \"needle\""),
        ("main.tf", "value = \"needle\""),
        ("values.tfvars", "value = \"needle\""),
        ("api.proto", "// needle"),
        ("schema.graphql", "# needle"),
        ("schema.gql", "# needle"),
        ("site.less", "// needle"),
        ("site.sass", "// needle"),
        ("Widget.vue", "<template><main>needle</main></template>"),
        ("Widget.astro", "<div>needle</div>"),
        ("Widget.svelte", "<main>needle</main>"),
        ("Dockerfile", "# needle"),
        ("Makefile", "# needle"),
        ("rules.mk", "# needle"),
        ("CMakeLists.txt", "# needle"),
        ("module.cmake", "# needle"),
        ("BUILD", "# needle"),
        ("BUILD.bazel", "# needle"),
        ("defs.bzl", "# needle"),
        ("default.nix", "# needle\n{}"),
    ] {
        let created = format!("lsr_007_created_{}", file.replace('.', "_"));
        let updated = format!("lsr_007_updated_{}", file.replace('.', "_"));
        let path = temp.path().join(file);
        fs::write(&path, format!("{body}\n# {created}\n")).unwrap();
        client
            .send_tool_until("search", serde_json::json!({ "query": created }), |text| {
                text.contains(file)
            })
            .await
            .unwrap();
        fs::write(&path, format!("{body}\n# {updated}\n")).unwrap();
        client
            .send_tool_until("search", serde_json::json!({ "query": updated }), |text| {
                text.contains(file)
            })
            .await
            .unwrap();
        fs::remove_file(&path).unwrap();
        let removed = client
            .send_tool_until("search", serde_json::json!({ "query": updated }), |text| {
                text.starts_with("No indexed matches")
            })
            .await
            .unwrap();
        assert!(
            removed["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("No indexed matches"),
            "watcher retained deleted {file}: {}",
            removed["result"]["content"][0]["text"]
        );
    }
}

#[tokio::test]
async fn test_watcher_does_not_index_repo_local_custom_index() {
    let temp = create_mock_repo(&[
        ("source.json", r#"{"value": "watcher_source"}"#),
        (".codemap/config.toml", "index_path = \"search-index\"\nindex_staleness_ms = 3600000\nwatch_debounce_ms = 100\n"),
    ]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "watcher_source" }),
            |text| text.contains("source.json"),
        )
        .await
        .unwrap();
    let_seeded_refresh_settle().await;

    let generated = temp.path().join("search-index/watcher-generated.json");
    fs::write(&generated, r#"{"needle": "watcher_self_index_needle"}"#).unwrap();
    let_seeded_refresh_settle().await;
    let result = client.send_request("tools/call", serde_json::json!({ "name": "search", "arguments": { "query": "watcher_self_index_needle" } })).await.unwrap();
    assert!(result["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("No indexed matches"));
}
