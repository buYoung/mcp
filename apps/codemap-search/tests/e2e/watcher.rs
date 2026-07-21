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
const WATCHER_ONLY_CONFIG: &str = "index_staleness_ms = 3600000\nwatch_debounce_ms = 100\n";

/// Settle window between the first request and the test's mutation. If the suppression
/// gate ever regressed, that first request could seed ONE request-fallback refresh; on
/// these tiny repos any such pass finishes well within this window, so a post-mutation
/// reflection can only have come from the watcher (no race-through false pass).
async fn let_seeded_refresh_settle() {
    tokio::time::sleep(Duration::from_millis(1500)).await;
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
        ("site.scss", "// needle"),
        ("site.less", "// needle"),
        ("Widget.astro", "<div>needle</div>"),
        ("Dockerfile", "# needle"),
        ("Makefile", "# needle"),
        ("CMakeLists.txt", "# needle"),
        ("BUILD", "# needle"),
        ("BUILD.bazel", "# needle"),
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
