//! `[analysis.jev]` config: defaults, parsed thresholds as advertised by `tools/list`, and the
//! additive schema sync of an existing repo config. No case evaluates anything.

use serde_json::{json, Value};

use crate::e2e::helpers::{create_mock_repo, McpClient};

const SEARCH_THRESHOLD_TEXT: &str = "bodies with Jev P(unrelated) >= ";

async fn listed_tools(files: &[(&str, &str)]) -> (Vec<Value>, tempfile::TempDir) {
    let repo = create_mock_repo(files).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let response = client.send_request("tools/list", json!({})).await.unwrap();
    let tools = response["result"]["tools"].as_array().unwrap().clone();
    (tools, repo)
}

fn tool<'a>(tools: &'a [Value], name: &str) -> &'a Value {
    tools.iter().find(|tool| tool["name"] == name).unwrap()
}

fn task_query_description(tools: &[Value], name: &str) -> String {
    let property = &tool(tools, name)["inputSchema"]["properties"]["task_query"];
    assert_eq!(property["type"], "string", "{property}");
    assert_eq!(property["maxLength"], 2000, "{property}");
    property["description"].as_str().unwrap().to_string()
}

/// The search threshold `tools/list` advertises, when the filter is enabled.
fn advertised_threshold(tools: &[Value]) -> Option<String> {
    let description = task_query_description(tools, "search");
    let (_, rest) = description.split_once(SEARCH_THRESHOLD_TEXT)?;
    Some(rest.chars().take(4).collect())
}

#[tokio::test]
async fn test_jev_modes_default_off_in_generated_config_and_schema() {
    let (tools, repo) = listed_tools(&[("src/lib.rs", "pub fn indexed() {}")]).await;
    let generated = std::fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap();
    assert!(
        generated.starts_with("# codemap-config-version: 24\n"),
        "{generated}"
    );
    let jev = generated.split("[analysis.jev]").nth(1).unwrap();
    let jev = jev.split("\n[filesystem_permissions]").next().unwrap();
    for example in [
        "# is_overview_enabled = false",
        "# is_search_filter_enabled = false",
        "# model = \"jev-1.13.0\"",
        "# timeout_ms = 45000",
        "# max_in_flight_requests = 3",
        "# request_spacing_ms = 300",
        "# max_batch_bytes = 80000",
        "# pool_idle_timeout_ms = 30000",
        "# search_filter_min_unrelated_probability = 0.70",
    ] {
        assert!(jev.contains(example), "{example}: {jev}");
    }
    assert!(
        !jev.contains("api_key_env ="),
        "the key variable is global-only: {jev}"
    );
    let parsed: toml::Value = toml::from_str(&generated).unwrap();
    assert_eq!(parsed["analysis"]["jev"].as_table().unwrap().len(), 0);

    for name in ["overview", "search"] {
        assert_eq!(
            tool(&tools, name)["annotations"]["openWorldHint"],
            false,
            "{name}"
        );
        assert!(
            task_query_description(&tools, name).contains("ignored while"),
            "{name}"
        );
        let description = tool(&tools, name)["description"].as_str().unwrap();
        assert!(!description.contains("Jev"), "{name}: {description}");
        assert!(tool(&tools, name)["inputSchema"]["required"]
            .as_array()
            .is_none_or(|required| required.iter().all(|field| field != "task_query")));
    }
}

#[tokio::test]
async fn test_jev_threshold_overrides_reach_the_schema() {
    let enabled = "[analysis.jev]\nis_search_filter_enabled = true\n";
    let (tools, _repo) = listed_tools(&[("config.toml", enabled)]).await;
    assert_eq!(advertised_threshold(&tools).as_deref(), Some("0.70"));
    assert_eq!(tool(&tools, "search")["annotations"]["openWorldHint"], true);
    assert_eq!(
        tool(&tools, "overview")["annotations"]["openWorldHint"],
        false
    );
    assert!(tool(&tools, "search")["description"]
        .as_str()
        .unwrap()
        .contains("(P(unrelated) >= 0.70)"));

    let global = format!("{enabled}search_filter_min_unrelated_probability = 0.90\n");
    let (tools, _repo) = listed_tools(&[("config.toml", global.as_str())]).await;
    assert_eq!(advertised_threshold(&tools).as_deref(), Some("0.90"));

    // Repo overrides global per key; invalid values fall back to the lower layer.
    let repo_override = "[analysis.jev]\nsearch_filter_min_unrelated_probability = 0.9\n";
    let lower = format!("{enabled}search_filter_min_unrelated_probability = 0.8\n");
    let (tools, _repo) = listed_tools(&[
        ("config.toml", lower.as_str()),
        (".codemap/config.toml", repo_override),
    ])
    .await;
    assert_eq!(advertised_threshold(&tools).as_deref(), Some("0.90"));
    for invalid in ["nan", "inf", "0.5", "0.2", "1.5", "\"0.9\""] {
        let repo_config =
            format!("[analysis.jev]\nsearch_filter_min_unrelated_probability = {invalid}\n");
        let (tools, _repo) = listed_tools(&[
            ("config.toml", lower.as_str()),
            (".codemap/config.toml", repo_config.as_str()),
        ])
        .await;
        assert_eq!(
            advertised_threshold(&tools).as_deref(),
            Some("0.80"),
            "{invalid}"
        );
        let (tools, _repo) = listed_tools(&[
            ("config.toml", enabled),
            (".codemap/config.toml", repo_config.as_str()),
        ])
        .await;
        assert_eq!(
            advertised_threshold(&tools).as_deref(),
            Some("0.70"),
            "{invalid}"
        );
    }

    let overview_only = "[analysis.jev]\nis_overview_enabled = true\n";
    let (tools, _repo) = listed_tools(&[("config.toml", overview_only)]).await;
    assert_eq!(
        tool(&tools, "overview")["annotations"]["openWorldHint"],
        true
    );
    assert_eq!(
        tool(&tools, "search")["annotations"]["openWorldHint"],
        false
    );
    assert_eq!(advertised_threshold(&tools), None);
    assert!(task_query_description(&tools, "overview").contains("api.typesafe.ai"));
}

#[tokio::test]
async fn test_config_sync_adds_the_commented_jev_block_to_an_existing_file() {
    let existing = "# codemap-config-version: 23\n# team settings\n\n[output.search]\n# keep this limit\ndetail_file_limit = 7\n\n[analysis]\n# Rust analysis target OS.\ntarget_os = \"linux\"\n\n[filesystem_permissions]\nfind = \"workspace\"\n";
    let repo = create_mock_repo(&[
        ("src/lib.rs", "pub fn indexed() {}"),
        (".codemap/config.toml", existing),
    ])
    .unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let tools = client.send_request("tools/list", json!({})).await.unwrap();
    let synced = std::fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap();
    assert!(
        synced.starts_with("# codemap-config-version: 24\n# team settings\n"),
        "{synced}"
    );
    assert!(
        synced.contains("# keep this limit\ndetail_file_limit = 7\n"),
        "{synced}"
    );
    let target = synced.find("target_os = \"linux\"").unwrap();
    let block = synced.find("# [analysis.jev]").unwrap();
    let next = synced.find("[filesystem_permissions]").unwrap();
    assert!(target < block && block < next, "{synced}");
    assert!(
        synced.contains("# is_search_filter_enabled = false"),
        "{synced}"
    );
    assert!(!synced.contains("api_key_env ="), "{synced}");
    let parsed: toml::Value = toml::from_str(&synced).unwrap();
    assert_eq!(
        parsed["output"]["search"]["detail_file_limit"].as_integer(),
        Some(7)
    );
    assert_eq!(parsed["analysis"]["target_os"].as_str(), Some("linux"));
    assert!(
        parsed["analysis"].get("jev").is_none(),
        "the added block stays commented"
    );
    for tool in tools["result"]["tools"].as_array().unwrap() {
        assert_eq!(tool["annotations"]["openWorldHint"], false, "{tool}");
    }
    drop(client);

    // A current file is never rewritten.
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    client.send_request("tools/list", json!({})).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap(),
        synced
    );
}
