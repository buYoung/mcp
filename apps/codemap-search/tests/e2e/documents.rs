use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use predicates::prelude::*;
use std::fs;
use std::time::Duration;

const DOCUMENT_CONTENT: &str =
    "# Document Feature\n\ndocument_support_unique_token\n\n[guide](https://example.com)\n";

fn config(is_enabled: bool) -> String {
    format!(
        "# codemap-config-version: 5\n\
         [refresh]\n\
         index_staleness_ms = 3600000\n\
         watch_debounce_ms = 100\n\
         [language_support]\n\
         is_document_support_enabled = {is_enabled}\n"
    )
}

fn response_text(response: &serde_json::Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
}

#[test]
fn test_document_support_gates_cli_search_and_codemap_but_not_direct_parse() {
    let disabled = create_mock_repo(&[
        (".codemap/config.toml", &config(false)),
        ("docs/guide.md", DOCUMENT_CONTENT),
        ("notes.txt", "document_support_unique_token"),
    ])
    .unwrap();
    run_cli(&["index"], disabled.path()).success();
    run_cli(
        &["search", "document_support_unique_token"],
        disabled.path(),
    )
    .success()
    .stdout(predicate::str::contains("guide.md").not());
    run_cli(&["codemap"], disabled.path())
        .success()
        .stdout(predicate::str::contains("guide.md").not());
    run_cli(&["parse", "docs/guide.md"], disabled.path())
        .success()
        .stdout(predicate::str::contains("Document Feature"));

    let enabled = create_mock_repo(&[
        (".codemap/config.toml", &config(true)),
        ("docs/guide.md", DOCUMENT_CONTENT),
        ("notes.txt", "document_support_unique_token"),
    ])
    .unwrap();
    run_cli(&["index"], enabled.path()).success();
    run_cli(&["search", "document_support_unique_token"], enabled.path())
        .success()
        .stdout(predicate::str::contains("docs/guide.md"))
        .stdout(predicate::str::contains("notes.txt").not());
    run_cli(&["codemap"], enabled.path())
        .success()
        .stdout(predicate::str::contains("guide.md"))
        .stdout(predicate::str::contains("notes.txt").not());
}

#[tokio::test]
async fn test_document_support_runtime_toggle_converges_index_while_live_tools_remain_available() {
    let initial_config = config(false);
    let temp = create_mock_repo(&[
        (".codemap/config.toml", &initial_config),
        ("docs/guide.md", DOCUMENT_CONTENT),
        ("notes.txt", "document_support_unique_token"),
        (
            "package-lock.json",
            r#"{"document_support_unique_token": true}"#,
        ),
        ("app.min.js", "const document_support_unique_token = true;"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let disabled_search = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "search",
                "arguments": { "query": "document_support_unique_token" }
            }),
        )
        .await
        .unwrap();
    assert!(!response_text(&disabled_search).contains("guide.md"));

    let default_find = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "find",
                "arguments": { "pattern": "**/*.md" }
            }),
        )
        .await
        .unwrap();
    assert!(response_text(&default_find).contains("guide.md"));
    let default_grep = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "grep",
                "arguments": {
                    "pattern": "document_support_unique_token",
                    "glob": "**/*.md"
                }
            }),
        )
        .await
        .unwrap();
    assert!(response_text(&default_grep).contains("guide.md"));
    let direct_read = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "read",
                "arguments": { "file_path": "docs/guide.md" }
            }),
        )
        .await
        .unwrap();
    assert!(response_text(&direct_read).contains("Document Feature"));

    let disabled_overview = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": {}
            }),
        )
        .await
        .unwrap();
    assert!(!response_text(&disabled_overview).contains("guide.md"));

    fs::write(temp.path().join(".codemap/config.toml"), config(true)).unwrap();
    let enabled_search = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "document_support_unique_token" }),
            |text| text.contains("docs/guide.md"),
        )
        .await
        .unwrap();
    let enabled_text = response_text(&enabled_search);
    assert!(enabled_text.contains("docs/guide.md"), "{enabled_text}");
    for excluded in ["notes.txt", "package-lock.json", "app.min.js"] {
        assert!(!enabled_text.contains(excluded), "{enabled_text}");
    }

    let overview = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "overview",
                "arguments": { "path": "docs/guide.md" }
            }),
        )
        .await
        .unwrap();
    let overview_text = response_text(&overview);
    assert!(
        overview_text.contains("Document Feature"),
        "{overview_text}"
    );

    tokio::time::sleep(Duration::from_millis(1200)).await;
    let enabled_find = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "find",
                "arguments": { "pattern": "**/*.md" }
            }),
        )
        .await
        .unwrap();
    assert!(response_text(&enabled_find).contains("guide.md"));
    let enabled_grep = client
        .send_request(
            "tools/call",
            serde_json::json!({
                "name": "grep",
                "arguments": { "pattern": "document_support_unique_token" }
            }),
        )
        .await
        .unwrap();
    let grep_text = response_text(&enabled_grep);
    assert!(grep_text.contains("guide.md"), "{grep_text}");
    for excluded in ["notes.txt", "package-lock.json", "app.min.js"] {
        assert!(!grep_text.contains(excluded), "{grep_text}");
    }

    fs::write(temp.path().join(".codemap/config.toml"), config(false)).unwrap();
    let disabled_again = client
        .send_tool_until(
            "search",
            serde_json::json!({ "query": "document_support_unique_token" }),
            |text| !text.contains("guide.md") && !text.contains("warming up"),
        )
        .await
        .unwrap();
    assert!(!response_text(&disabled_again).contains("guide.md"));
}
