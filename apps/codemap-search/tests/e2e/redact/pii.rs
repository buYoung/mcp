use super::super::helpers::{create_mock_repo, McpClient};
use super::{call, text};
use serde_json::json;

#[tokio::test]
async fn test_pii_live_configuration_and_context_survive_clipped_outputs() {
    let base = "[update]\nconfig_auto_update=false\n[search]\nsearch_literal_max_len=4\nsearch_detail_snippet_max_lines=1\n";
    let source = "export const password = 'fixture-live-secret';\nexport const card = '4111111111111111';\nexport const postalCode =\n  '10115';\nexport const timeoutMs = 10000;\nexport const bankAccount = '945456787654';\n";
    let repo =
        create_mock_repo(&[("src/profile.ts", source), (".codemap/config.toml", base)]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let arguments = json!({"file_path":"src/profile.ts","view":"source"});
    let response = call(&mut client, "read", arguments.clone()).await;
    assert!(text(&response).contains("4111111111111111"));
    assert!(!text(&response).contains("fixture-live-secret"));
    let selected = "[redact]\npii_entities=['CREDIT_CARD','DE_PLZ','US_BANK_NUMBER']\n";
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        format!("{base}{selected}"),
    )
    .unwrap();
    let response = client
        .send_tool_until("read", arguments.clone(), |output| {
            !output.contains("4111111111111111")
        })
        .await
        .unwrap();
    for value in [
        "4111111111111111",
        "10115",
        "945456787654",
        "fixture-live-secret",
    ] {
        assert!(!response.to_string().contains(value), "{response}");
    }
    assert!(text(&response).contains("timeoutMs = 10000"));
    for (name, arguments) in [
        (
            "read",
            json!({"file_path":"src/profile.ts","offset":4,"limit":1,"view":"source"}),
        ),
        (
            "grep",
            json!({"path":"src/profile.ts","pattern":"10115","expand":"none","view":"source"}),
        ),
    ] {
        let response = call(&mut client, name, arguments).await;
        assert!(!response.to_string().contains("10115"), "{response}");
        assert!(text(&response).contains("*****"), "{response}");
    }
    let response = call(
        &mut client,
        "search",
        json!({"query":"945456787654","caller_context":false}),
    )
    .await;
    assert!(text(&response).contains("src/profile.ts"), "{response}");
    assert!(
        !response.to_string().contains("9454"),
        "PII prefix escaped truncation: {response}"
    );
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        format!(
            "{base}{selected}exceptions=[{{rule_id='pii.credit-card',value='4111111111111111'}}]\n"
        ),
    )
    .unwrap();
    let response = client
        .send_tool_until("read", arguments.clone(), |output| {
            output.contains("4111111111111111")
        })
        .await
        .unwrap();
    assert!(text(&response).contains("4111111111111111"));
    assert!(!text(&response).contains("945456787654"));
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        format!("{base}[redact]\npii_entities=[]\n"),
    )
    .unwrap();
    let response = client
        .send_tool_until("read", arguments.clone(), |output| {
            output.contains("945456787654")
        })
        .await
        .unwrap();
    assert!(text(&response).contains("10115"));
    assert!(text(&response).contains("945456787654"));
    assert!(!text(&response).contains("fixture-live-secret"));
    std::fs::write(
        repo.path().join(".codemap/config.toml"),
        format!("{base}[tool_output]\nis_redact_enabled=false\n{selected}"),
    )
    .unwrap();
    let response = client
        .send_tool_until("read", arguments, |output| {
            output.contains("fixture-live-secret")
        })
        .await
        .unwrap();
    assert!(text(&response).contains("fixture-live-secret"));
    assert!(text(&response).contains("4111111111111111"));
    assert_eq!(
        std::fs::read_to_string(repo.path().join("src/profile.ts")).unwrap(),
        source
    );
}

#[tokio::test]
async fn test_all_pii_types_preserve_mcp_negotiation() {
    let catalog: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/redact/pii/catalog.json"
    )))
    .unwrap();
    let entities: Vec<_> = catalog
        .iter()
        .map(|entry| entry["entity"].as_str().unwrap())
        .collect();
    let config = format!(
        "[redact]\npii_entities={}\n",
        serde_json::to_string(&entities).unwrap()
    );
    let repo = create_mock_repo(&[
        ("src/date.ts", "export const date = '2025-06-18';\n"),
        (".codemap/config.toml", &config),
    ])
    .unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let response = client
        .send_request("initialize", json!({"protocolVersion":"2025-06-18"}))
        .await
        .unwrap();
    assert_eq!(
        response["result"]["protocolVersion"], "2025-06-18",
        "{response}"
    );
    assert_eq!(
        response["result"]["serverInfo"]["name"],
        "codemap-search-server"
    );
    assert_eq!(
        response["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(response["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("tool_output.is_redact_enabled"));
    let response = client.send_request("tools/list", json!({})).await.unwrap();
    let tools = response["result"]["tools"].as_array().unwrap();
    let mut names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "analyze",
            "find",
            "grep",
            "initial_instructions",
            "overview",
            "read",
            "search"
        ]
    );
    assert!(tools
        .iter()
        .all(|tool| tool["inputSchema"]["type"] == "object"));
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"src/date.ts", "view":"source"}),
    )
    .await;
    assert!(!text(&response).contains("2025-06-18"), "{response}");
}

#[tokio::test]
async fn test_selected_pii_rules_reach_mcp_source_and_indexed_outputs() {
    let config = "[redact]\npii_entities=['CREDIT_CARD','EMAIL_ADDRESS','US_SSN']\n";
    let source = "export const card = '4111111111111111';\nexport const email = 'info@presidio.site';\nexport const ssn = '321-54-9876';\nexport const site = 'https://example.org';\nexport const rrn = '900101-1234567';\nexport function checkout(value: string = '4111111111111111') { return value; }\n";
    let repo =
        create_mock_repo(&[("src/account.ts", source), (".codemap/config.toml", config)]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let requests = [
        (
            "read",
            json!({"file_path":"src/account.ts", "view":"source"}),
        ),
        ("read", json!({"file_path":"src/account.ts", "view":"full"})),
        (
            "read",
            json!({"file_path":"src/account.ts", "offset":6, "expand":"callable"}),
        ),
        (
            "grep",
            json!({"path":"src/account.ts", "pattern":"4111111111111111", "expand":"none", "view":"source"}),
        ),
        (
            "grep",
            json!({"path":"src/account.ts", "pattern":"4111111111111111", "expand":"callable"}),
        ),
        (
            "search",
            json!({"query":"4111111111111111", "caller_context":false}),
        ),
        ("overview", json!({"path":"src"})),
    ];
    for (tool, arguments) in requests {
        let response = call(&mut client, tool, arguments).await;
        assert!(response.get("error").is_none(), "{tool}: {response}");
        for value in ["4111111111111111", "info@presidio.site", "321-54-9876"] {
            assert!(!response.to_string().contains(value), "{tool}: {response}");
        }
        if tool == "search" {
            assert!(text(&response).contains("[REDACTED]"), "{response}");
        }
    }
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"src/account.ts", "view":"source"}),
    )
    .await;
    assert!(text(&response).contains("https://example.org"));
    assert!(text(&response).contains("900101-1234567"));
    assert_eq!(
        std::fs::read_to_string(repo.path().join("src/account.ts")).unwrap(),
        source
    );
    let response = call(
        &mut client,
        "read",
        json!({"file_path":"info@presidio.site"}),
    )
    .await;
    assert!(response.get("error").is_some());
    assert!(
        !response.to_string().contains("info@presidio.site"),
        "{response}"
    );
}
