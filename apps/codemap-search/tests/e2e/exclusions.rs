use super::helpers::{create_mock_repo, run_cli, McpClient};
use predicates::prelude::PredicateBooleanExt;
use serde_json::{json, Value};
use std::fs;

fn result_text(response: &Value) -> &str {
    response["result"]["content"][0]["text"].as_str().unwrap()
}

async fn call(client: &mut McpClient, name: &str, arguments: Value) -> Value {
    client
        .send_request("tools/call", json!({"name": name, "arguments": arguments}))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_exclusions_generated_common_and_project_globs() {
    let repo = create_mock_repo(&[
        ("apps/web/package.json", "{}"),
        (
            "apps/web/node_modules/lib/generated.js",
            "export function excluded_dependency() {}",
        ),
        (
            "apps/web/dist/generated.js",
            "export function excluded_bundle() {}",
        ),
        ("apps/api/pyproject.toml", ""),
        (
            "apps/api/src/__pycache__/generated.py",
            "def excluded_cache(): pass",
        ),
        ("apps/native/Cargo.toml", ""),
        ("apps/native/build/source.rs", "pub fn excluded_build() {}"),
        (".idea/settings.json", "{\"common_exclusion\": true}"),
        (
            "apps/api/.vscode/settings.json",
            "{\"common_exclusion\": true}",
        ),
        (".git/internal.json", "{\"common_exclusion\": true}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let found = call(&mut client, "find", json!({"pattern": "**/*"})).await;
    let text = result_text(&found);
    assert!(!text.contains("apps/native/build/source.rs"), "{text}");
    for excluded in [
        "node_modules",
        "generated.js",
        "__pycache__",
        ".idea/",
        ".vscode/",
        ".git/",
    ] {
        assert!(!text.contains(excluded), "{text}");
    }
    let config = fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap();
    assert!(config.starts_with("# codemap-config-version: 6"));
    let parsed: toml::Value = toml::from_str(&config).unwrap();
    let patterns = parsed["index"]["excluded_directories"].as_array().unwrap();
    for pattern in [
        ".git",
        ".idea",
        ".vscode",
        ".codemap",
        "**/node_modules",
        "**/__pycache__",
        "**/target",
    ] {
        assert!(
            patterns.iter().any(|value| value.as_str() == Some(pattern)),
            "{config}"
        );
    }
    let search = call(
        &mut client,
        "search",
        json!({"query": "excluded_build", "workspace_scope": "all"}),
    )
    .await;
    assert!(!result_text(&search).contains("apps/native/build/source.rs"));
    let bypass = call(
        &mut client,
        "find",
        json!({"pattern": "**/*", "include_ignored": true}),
    )
    .await;
    let text = result_text(&bypass);
    assert!(text.contains(".idea/settings.json"));
    assert!(text.contains("apps/web/node_modules/lib/generated.js"));
    assert!(!text.contains(".git/internal.json"));
    assert!(!text.contains(".codemap/config.toml"));
    let read = call(&mut client, "read", json!({"path": ".idea/settings.json"})).await;
    assert!(result_text(&read).contains("common_exclusion"));
}

#[tokio::test]
async fn test_exclusions_manual_reload_reconciles_index_without_source_edits() {
    let config_path = ".codemap/config.toml";
    let original = "# codemap-config-version: 6\n[index]\nexcluded_directories = []\n[refresh]\nindex_staleness_ms = 3600000\n";
    let repo = create_mock_repo(&[
        (config_path, original),
        ("apps/web/build/hidden.rs", "pub fn exclusion_probe() {}"),
        ("apps/native/build/kept.rs", "pub fn exclusion_probe() {}"),
        (".vscode/source.rs", "pub fn exclusion_probe() {}"),
        ("build", "exclusion_probe"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let query = json!({"query": "exclusion_probe", "workspace_scope": "all"});
    let initial = call(&mut client, "search", query.clone()).await;
    assert!(result_text(&initial).contains("hidden.rs"));
    assert!(result_text(&initial).contains(".vscode/source.rs"));
    let updated = original.replace(
        "excluded_directories = []",
        "excluded_directories = ['apps/web/build', '.vscode']",
    );
    fs::write(repo.path().join(config_path), &updated).unwrap();
    let hidden = client
        .send_tool_until("search", query.clone(), |text| {
            text.contains("kept.rs")
                && !text.contains("hidden.rs")
                && !text.contains(".vscode/source.rs")
        })
        .await
        .unwrap();
    let diagnostic_find = call(&mut client, "find", json!({"pattern": "**/*.rs"})).await;
    assert!(
        !result_text(&hidden).contains("hidden.rs"),
        "search={}\nfind={}",
        result_text(&hidden),
        result_text(&diagnostic_find)
    );
    let grep = call(
        &mut client,
        "grep",
        json!({"pattern": "exclusion_probe", "output_mode": "files_with_matches"}),
    )
    .await;
    assert!(!result_text(&grep).contains("hidden.rs"));
    assert!(result_text(&grep).contains("apps/native/build/kept.rs"));
    let targeted = call(
        &mut client,
        "find",
        json!({"path": "apps/web/build", "pattern": "**/*"}),
    )
    .await;
    assert!(!result_text(&targeted).contains("hidden.rs"));
    let files = call(&mut client, "find", json!({"pattern": "build"})).await;
    assert!(result_text(&files).lines().any(|line| line == "build"));
    let bypass = call(&mut client, "grep", json!({"pattern": "exclusion_probe", "include_ignored": true, "output_mode": "files_with_matches"})).await;
    assert!(result_text(&bypass).contains("hidden.rs"));
    let overview = call(&mut client, "overview", json!({"path": "apps/web/build"})).await;
    assert!(!result_text(&overview).contains("hidden.rs"));
    run_cli(&["codemap", "--path", "apps/web/build"], repo.path())
        .success()
        .stdout(predicates::str::contains("hidden.rs").not());
    // This event must not re-add a file under an excluded directory.
    fs::write(
        repo.path().join("apps/web/build/hidden.rs"),
        "pub fn exclusion_probe() {}\npub fn changed_excluded() {}\n",
    )
    .unwrap();
    fs::write(repo.path().join(config_path), original).unwrap();
    let restored = client
        .send_tool_until("search", query, |text| {
            text.contains("hidden.rs")
                && text.contains(".vscode/source.rs")
                && text.contains("kept.rs")
        })
        .await
        .unwrap();
    assert!(result_text(&restored).contains("hidden.rs"));
    assert_eq!(
        fs::read_to_string(repo.path().join(config_path)).unwrap(),
        original
    );
}

#[tokio::test]
async fn test_exclusions_v6_is_not_regenerated_on_restart() {
    let source = "# codemap-config-version: 6\n[index]\nexcluded_directories = []\n";
    let repo = create_mock_repo(&[
        (".codemap/config.toml", source),
        ("package.json", "{}"),
        (
            "node_modules/source.js",
            "export const visible_dependency = 1;",
        ),
        (".idea/source.json", "{}"),
    ])
    .unwrap();
    for _ in 0..2 {
        let mut client = McpClient::spawn(repo.path()).await.unwrap();
        let found = call(&mut client, "find", json!({"pattern": "**/*"})).await;
        assert!(result_text(&found).contains("node_modules/source.js"));
        assert!(result_text(&found).contains(".idea/source.json"));
        assert_eq!(
            fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap(),
            source
        );
        client.child.kill().await.unwrap();
    }
}

#[tokio::test]
async fn test_exclusions_pre_v6_transition_and_opt_out() {
    for marker in ["", "# codemap-config-version: 5\n"] {
        let source =
            format!("{marker}# keep\n[index]\nexcluded_directories = ['custom'] # custom rule\n");
        let repo =
            create_mock_repo(&[(".codemap/config.toml", &source), ("package.json", "{}")]).unwrap();
        let mut client = McpClient::spawn(repo.path()).await.unwrap();
        call(&mut client, "find", json!({"pattern": "**/*"})).await;
        let updated = fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap();
        assert!(updated.starts_with("# codemap-config-version: 6"));
        assert!(updated.contains("# keep"));
        assert!(updated.contains("# custom rule"));
        let value: toml::Value = toml::from_str(&updated).unwrap();
        let patterns = value["index"]["excluded_directories"].as_array().unwrap();
        for expected in ["custom", "node_modules", ".idea", "**/node_modules"] {
            assert!(
                patterns.iter().any(|p| p.as_str() == Some(expected)),
                "{updated}"
            );
        }
    }
    let source = "# codemap-config-version: 5\n[update]\nconfig_auto_update = false\n[index]\nexcluded_directories = []\n";
    let repo = create_mock_repo(&[(".codemap/config.toml", source)]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    call(&mut client, "find", json!({"pattern": "**/*"})).await;
    assert_eq!(
        fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap(),
        source
    );
}
