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
        (".gitignore", "ignored_by_git/\n"),
        (".codemapignore", "ignored_by_codemap/\n"),
        ("ignored_by_git/hidden.rs", "pub fn ignored_git_probe() {}"),
        (
            "ignored_by_codemap/hidden.rs",
            "pub fn ignored_codemap_probe() {}",
        ),
        ("src/visible.rs", "pub fn visible_probe() {}"),
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
    assert!(config.starts_with("# codemap-config-version: 17"));
    let parsed: toml::Value = toml::from_str(&config).unwrap();
    let patterns = parsed["exclude"]["excluded_directories"]
        .as_array()
        .unwrap();
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
    // A directly named file must obey the same ancestor rules as a directory walk.
    // Exercise all output modes, relative/absolute paths, and the explicit bypass.
    for (path, pattern, is_optional) in [
        (
            "apps/web/node_modules/lib/generated.js",
            "excluded_dependency",
            true,
        ),
        (".idea/settings.json", "common_exclusion", true),
        ("ignored_by_git/hidden.rs", "ignored_git_probe", true),
        (
            "ignored_by_codemap/hidden.rs",
            "ignored_codemap_probe",
            true,
        ),
        (".git/internal.json", "common_exclusion", false),
        (".codemap/config.toml", "codemap-config-version", false),
    ] {
        for path in [
            path.to_string(),
            repo.path().join(path).to_string_lossy().into_owned(),
        ] {
            for output_mode in ["content", "files_with_matches", "count"] {
                for include_ignored in [false, true] {
                    let response = call(
                        &mut client,
                        "grep",
                        json!({
                            "path": path,
                            "pattern": pattern,
                            "output_mode": output_mode,
                            "include_ignored": include_ignored,
                        }),
                    )
                    .await;
                    let text = result_text(&response);
                    let should_match = is_optional && include_ignored;
                    if should_match {
                        assert!(
                            !text.contains("No matches found")
                                && !text.contains("Found 0 total occurrence"),
                            "{path}, {output_mode}, include_ignored={include_ignored}: {text}"
                        );
                        assert!(
                            text.contains(if output_mode == "count" {
                                "Found 1 total occurrence"
                            } else {
                                path.rsplit('/').next().unwrap()
                            }),
                            "{text}"
                        );
                    } else {
                        assert!(
                            text.contains(if output_mode == "count" {
                                "Found 0 total occurrence"
                            } else {
                                "No matches found"
                            }),
                            "{path}, {output_mode}, include_ignored={include_ignored}: {text}"
                        );
                    }
                }
            }
        }
    }
    let visible = call(&mut client, "grep", json!({
        "path": "src/visible.rs", "pattern": "visible_probe", "glob": "*.py", "output_mode": "count"
    })).await;
    assert!(
        result_text(&visible).contains("Found 1 total occurrence"),
        "{}",
        result_text(&visible)
    );
    let read = call(&mut client, "read", json!({"path": ".idea/settings.json"})).await;
    assert!(result_text(&read).contains("common_exclusion"));
}

#[tokio::test]
async fn test_exclusions_manual_reload_reconciles_index_without_source_edits() {
    let config_path = ".codemap/config.toml";
    let original = "# codemap-config-version: 9\n[index]\nindex_path = '.custom-codemap-index'\n[exclude]\nexcluded_directories = []\n[refresh]\nindex_staleness_ms = 3600000\n";
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
    fs::write(
        repo.path().join(".custom-codemap-index/probe.rs"),
        "pub fn internal_index_probe() {}",
    )
    .unwrap();
    for include_ignored in [false, true] {
        let hidden = call(&mut client, "grep", json!({
            "path": ".custom-codemap-index/probe.rs", "pattern": "internal_index_probe", "output_mode": "count", "include_ignored": include_ignored
        })).await;
        assert!(
            result_text(&hidden).contains("Found 0 total occurrence"),
            "{}",
            result_text(&hidden)
        );
    }
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
    let direct = call(
        &mut client,
        "grep",
        json!({"path": "build", "pattern": "exclusion_probe", "output_mode": "count"}),
    )
    .await;
    assert!(
        result_text(&direct).contains("Found 1 total occurrence"),
        "{}",
        result_text(&direct)
    );
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
        // This test isolates config exclusions from inherited user ignore files.
        (".gitignore", "!node_modules/\n!node_modules/**\n"),
        (
            "node_modules/source.js",
            "export const visible_dependency = 1;",
        ),
        (".idea/source.json", "{}"),
    ])
    .unwrap();
    let mut previous = None;
    for _ in 0..2 {
        let mut client = McpClient::spawn(repo.path()).await.unwrap();
        let found = call(&mut client, "find", json!({"pattern": "**/*"})).await;
        assert!(
            result_text(&found).contains("node_modules/source.js"),
            "{}",
            result_text(&found)
        );
        assert!(result_text(&found).contains(".idea/source.json"));
        let updated = fs::read_to_string(repo.path().join(".codemap/config.toml")).unwrap();
        assert!(updated.starts_with("# codemap-config-version: 17"));
        let parsed: toml::Value = toml::from_str(&updated).unwrap();
        assert_eq!(
            parsed["exclude"]["excluded_directories"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        if let Some(previous) = &previous {
            assert_eq!(
                &updated, previous,
                "the next restart must not rewrite a current config"
            );
        }
        previous = Some(updated);
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
        assert!(updated.starts_with("# codemap-config-version: 17"));
        assert!(updated.contains("# keep"));
        assert!(updated.contains("# custom rule"));
        let value: toml::Value = toml::from_str(&updated).unwrap();
        let patterns = value["exclude"]["excluded_directories"].as_array().unwrap();
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
