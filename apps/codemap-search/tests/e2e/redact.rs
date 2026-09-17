use super::helpers::{create_mock_repo, run_cli, McpClient};
use serde_json::{json, Value};

fn text(response: &Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("MCP text result")
}

async fn call(client: &mut McpClient, name: &str, arguments: Value) -> Value {
    client
        .send_request("tools/call", json!({"name":name,"arguments":arguments}))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_redact_masks_all_mcp_views_and_preserves_matching_and_disk() {
    let source = "pub const PASSWORD: &str = \"correct-horse-credential\";\npub const API_KEY: &str = \"sk-proj-abcdefghijklmnopqrstuv0123456789\";\npub const SAFE: &str = \"visible-message\";\npub fn authenticate() -> &'static str { PASSWORD }\n";
    let repo = create_mock_repo(&[("src/lib.rs", source)]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    call(&mut client, "overview", json!({"path":"src/lib.rs"})).await;
    for view in ["full", "source", "definitions", "relations"] {
        let response = call(
            &mut client,
            "read",
            json!({"file_path":"src/lib.rs","offset":4,"limit":1,"view":view}),
        )
        .await;
        assert!(
            !response.to_string().contains("correct-horse-credential"),
            "{response}"
        );
        if view == "full" || view == "relations" {
            assert!(text(&response).contains("PASSWORD"), "{response}");
            assert!(text(&response).contains("[REDACTED]"), "{response}");
        }
    }
    for expand in ["none", "callable"] {
        let response = call(&mut client, "grep", json!({"path":"src/lib.rs","pattern":"correct-horse-credential","expand":expand,"view":"source"})).await;
        assert!(text(&response).contains("src/lib.rs:1:"), "{response}");
        assert!(
            !response.to_string().contains("correct-horse-credential"),
            "{response}"
        );
    }
    let response = call(
        &mut client,
        "grep",
        json!({"path":"src/lib.rs","pattern":"correct-horse-credential","output_mode":"count"}),
    )
    .await;
    assert!(text(&response).contains("src/lib.rs:1"), "{response}");
    let response = call(
        &mut client,
        "search",
        json!({"query":"correct-horse-credential","caller_context":false}),
    )
    .await;
    assert!(
        text(&response).contains("src/lib.rs") && text(&response).contains("[REDACTED]"),
        "{response}"
    );
    assert!(
        !response.to_string().contains("correct-horse-credential"),
        "{response}"
    );
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"src/lib.rs","view":"source"}),
    )
    .await;
    assert!(!response
        .to_string()
        .contains("sk-proj-abcdefghijklmnopqrstuv0123456789"));
    assert!(text(&response).contains("3→pub const SAFE: &str = \"visible-message\";"));
    assert_eq!(
        std::fs::read_to_string(repo.path().join("src/lib.rs")).unwrap(),
        source
    );
    run_cli(&["parse", "src/lib.rs"], repo.path())
        .success()
        .stdout(predicates::str::contains("correct-horse-credential"));
}

#[tokio::test]
async fn test_redact_protects_interior_lines_and_search_truncation() {
    let long_token = format!("sk-proj-{}", "a".repeat(120));
    let python = format!("password = \"\"\"\ninterior-password-value\n\"\"\"\npublic = 'visible'\napi_key = '{long_token}'\n");
    let pem = "-----BEGIN PRIVATE KEY-----\nprivate-key-body-value\n-----END PRIVATE KEY-----\n";
    let yaml = "password: | # credential\n  interior-yaml-value\npublic: visible\n";
    let json_source = "{\"password\":\n\"interior-json-value\"}\n";
    let repo = create_mock_repo(&[
        ("secrets.py", &python), ("key.pem", pem), ("settings.yaml", yaml), ("settings.json", json_source),
        (".codemap/config.toml", "[update]\nconfig_auto_update=false\n[search]\nsearch_literal_max_len=8\nsearch_detail_snippet_max_lines=1\n"),
    ]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    for (path, secret) in [
        ("secrets.py", "interior-password-value"),
        ("key.pem", "private-key-body-value"),
        ("settings.yaml", "interior-yaml-value"),
        ("settings.json", "interior-json-value"),
    ] {
        let response = call(
            &mut client,
            "read",
            json!({"file_path":path,"offset":2,"limit":1,"view":"source"}),
        )
        .await;
        assert!(text(&response).contains("2→"), "{response}");
        assert!(!response.to_string().contains(secret), "{response}");
        let response = call(
            &mut client,
            "grep",
            json!({"path":path,"pattern":secret,"expand":"none","view":"source"}),
        )
        .await;
        assert!(text(&response).contains(":2:"), "{response}");
        assert!(!response.to_string().contains(secret), "{response}");
    }
    let response = call(
        &mut client,
        "search",
        json!({"query":"api_key","caller_context":false}),
    )
    .await;
    assert!(text(&response).contains("secrets.py"), "{response}");
    assert!(!response.to_string().contains("sk-proj-"), "{response}");
    let response = call(&mut client, "grep", json!({
        "path":"secrets.py", "pattern":"password|interior|public", "expand":"none", "view":"source", "-n":false
    })).await;
    assert!(
        text(&response).contains("secrets.py:public = 'visible'"),
        "{response}"
    );
    assert!(
        !text(&response).contains("interior-password-value"),
        "{response}"
    );
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"secrets.py","view":"source"}),
    )
    .await;
    assert!(
        text(&response).contains("4→public = 'visible'"),
        "{response}"
    );
}

#[tokio::test]
async fn test_redact_applies_to_errors_and_can_be_disabled_in_config() {
    let repo = create_mock_repo(&[("config.env", "API_KEY=plain-secret-value\n")]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"missing-sk-proj-abcdefghijklmnopqrstuv0123456789"}),
    )
    .await;
    assert_eq!(response["error"]["code"], -32602);
    assert!(
        !response
            .to_string()
            .contains("sk-proj-abcdefghijklmnopqrstuv0123456789"),
        "{response}"
    );
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"config.env","view":"source"}),
    )
    .await;
    assert!(!text(&response).contains("plain-secret-value"));
    client.kill().await.unwrap();
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        "[update]\nconfig_auto_update=false\n[tool_output]\nis_redact_enabled=false\n",
    )
    .unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"config.env","view":"source"}),
    )
    .await;
    assert!(
        text(&response).contains("API_KEY=plain-secret-value"),
        "{response}"
    );
}

#[tokio::test]
async fn test_redact_preserves_grep_original_column_limits() {
    let repo = create_mock_repo(&[
        ("values.py", "api_key = 'abcdefghijklmnopqrstuv-secret'\n"),
        (
            ".codemap/config.toml",
            "[update]\nconfig_auto_update=false\n[tool_output]\ngrep_max_columns=25\n",
        ),
    ])
    .unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    for expand in ["none", "callable"] {
        let response = call(
            &mut client,
            "grep",
            json!({
                "path":"values.py", "pattern":"api_key", "expand":expand, "view":"source"
            }),
        )
        .await;
        assert!(
            text(&response).contains("[Omitted long matching line]"),
            "{response}"
        );
        assert!(
            !text(&response).contains("abcdefghijklmnopqrstuv-secret"),
            "{response}"
        );
    }
}
