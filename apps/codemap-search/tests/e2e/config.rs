use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn test_auth_diagnostics_never_print_values_or_parser_source() {
    for auth in [
        "[jev]\napi_key = 'auth-secret-sentinel\n",
        "[jev]\napi_key = ['auth-secret-sentinel']\n",
        "[jev]\n'auth-secret-sentinel' = 'ignored'\n",
        "['auth-secret-sentinel']\napi_key = 'ignored'\n",
    ] {
        let temp = create_mock_repo(&[
            (".codemap/auth.toml", auth),
            ("src/a.rs", "fn visible_function() {}"),
        ])
        .unwrap();
        run_cli(&["codemap"], temp.path())
            .success()
            .stdout(predicate::str::contains("auth-secret-sentinel").not())
            .stderr(predicate::str::contains("auth-secret-sentinel").not())
            .stderr(predicate::str::contains("auth.toml"));
    }
}

#[tokio::test]
async fn test_config_threshold_override_changes_branching() {
    // result_threshold = 1 means 2 matches render hybrid: detail for the single top file,
    // the other as a ranked-tail line — where the default threshold of 5 would render both
    // in full detail. Proves config drives the detail/tail split.
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "result_threshold = 1\n"),
        ("src/a.rs", "fn shared_branch_fn() {}"),
        ("src/b.rs", "fn shared_branch_fn() {}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let res = client
        .send_request(
            "tools/call",
            serde_json::json!({ "name": "search", "arguments": { "query": "shared_branch_fn" } }),
        )
        .await
        .unwrap();
    let text = res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("Other matches — 1 more files"),
        "threshold=1 + 2 matches should push one match into the ranked tail: {text:?}"
    );
    assert_eq!(
        text.matches("\n### results\n").count(),
        1,
        "exactly one file gets the detail view at threshold=1: {text:?}"
    );
}

#[tokio::test]
async fn test_config_excluded_directories_augment() {
    // A configured exclude dir is ADDED to the built-ins: a source file inside it is not
    // indexed, while built-in excludes still apply (augment, not replace).
    let temp = create_mock_repo(&[
        (
            ".codemap/config.toml",
            "excluded_directories = [\"customjunk\"]\n",
        ),
        ("src/keep.rs", "pub fn unique_keepme() {}"),
        ("customjunk/gen.rs", "pub fn unique_keepme() {}"),
    ])
    .unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();

    let res = client
        .send_request(
            "tools/call",
            serde_json::json!({ "name": "search", "arguments": { "query": "unique_keepme" } }),
        )
        .await
        .unwrap();
    let text = res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("src/keep.rs"),
        "in-tree file should be indexed: {text:?}"
    );
    assert!(
        !text.contains("customjunk"),
        "configured exclude dir must not be indexed: {text:?}"
    );
}

/// Build a git repo whose `.git/info/exclude` hides `locally_excluded_dir/` and whose
/// `.gitignore` hides `gitignored_dir/`, optionally writing a `.codemap/config.toml`.
/// Returns `None` (skip) when git is unavailable.
fn make_git_repo_with_excludes(config_body: &str) -> Option<tempfile::TempDir> {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", config_body),
        (".gitignore", "gitignored_dir/\n"),
        (
            "gitignored_dir/by_gitignore.rs",
            "pub fn unique_gitignore_sym() {}",
        ),
        (
            "locally_excluded_dir/by_git_exclude.rs",
            "pub fn unique_gitexclude_sym() {}",
        ),
        ("src/keep.rs", "pub fn unique_kept_sym() {}"),
    ])
    .unwrap();
    let init = std::process::Command::new("git")
        .arg("-C")
        .arg(temp.path())
        .arg("init")
        .output();
    if init.map(|o| !o.status.success()).unwrap_or(true) {
        return None;
    }
    std::fs::write(
        temp.path().join(".git/info/exclude"),
        "locally_excluded_dir/\n",
    )
    .unwrap();
    Some(temp)
}

#[tokio::test]
async fn test_use_git_exclude_scopes_to_git_info_exclude_only() {
    // The dedicated `use_git_exclude` toggle governs ONLY `.git/info/exclude`:
    //  - default (true): a file hidden by `.git/info/exclude` stays out of the index.
    //  - false: that file becomes searchable, BUT `.gitignore` is still honored.

    // Default: `.git/info/exclude` honored → the locally-excluded file is absent.
    let Some(default_repo) = make_git_repo_with_excludes("") else {
        eprintln!("git unavailable — skipping use_git_exclude test");
        return;
    };
    let mut client = McpClient::spawn(default_repo.path()).await.unwrap();
    let res = client
        .send_request(
            "tools/call",
            serde_json::json!({ "name": "search", "arguments": { "query": "unique_gitexclude_sym" } }),
        )
        .await
        .unwrap();
    let text = res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.contains("locally_excluded_dir"),
        "default should honor .git/info/exclude: {text:?}"
    );

    // Override false: the `.git/info/exclude`-hidden file is now indexed...
    let settings = "[update]\nconfig_auto_update = false\n[refresh]\nindex_staleness_ms = 3600000\n[exclude]\nuse_git_exclude = false\n";
    let Some(override_repo) = make_git_repo_with_excludes(settings) else {
        return;
    };
    let mut client = McpClient::spawn(override_repo.path()).await.unwrap();
    let res = client
        .send_request(
            "tools/call",
            serde_json::json!({ "name": "search", "arguments": { "query": "unique_gitexclude_sym" } }),
        )
        .await
        .unwrap();
    let text = res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("locally_excluded_dir"),
        "use_git_exclude=false should index the .git/info/exclude-hidden file: {text:?}"
    );

    // ...while `.gitignore` is still honored (the toggle is scoped to git_exclude alone).
    let res = client
        .send_request(
            "tools/call",
            serde_json::json!({ "name": "search", "arguments": { "query": "unique_gitignore_sym" } }),
        )
        .await
        .unwrap();
    let text = res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.contains("gitignored_dir"),
        ".gitignore must stay honored under use_git_exclude=false: {text:?}"
    );

    for (tool, arguments) in [
        ("find", serde_json::json!({"pattern": "**/*.rs"})),
        (
            "grep",
            serde_json::json!({"pattern": "unique_gitexclude_sym", "output_mode": "files_with_matches"}),
        ),
    ] {
        let response = client
            .send_request(
                "tools/call",
                serde_json::json!({"name": tool, "arguments": arguments}),
            )
            .await
            .unwrap();
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("locally_excluded_dir/by_git_exclude.rs"),
            "{tool}: {text}"
        );
        assert!(
            !text.contains("gitignored_dir/by_gitignore.rs"),
            "{tool}: {text}"
        );
    }
    std::fs::write(
        override_repo.path().join(".codemap/config.toml"),
        settings.replace("use_git_exclude = false", "use_git_exclude = true"),
    )
    .unwrap();
    let refreshed = client
        .send_tool_until(
            "search",
            serde_json::json!({"query": "unique_gitexclude_sym"}),
            |text| !text.contains("locally_excluded_dir") && !text.contains("warming up"),
        )
        .await
        .unwrap();
    let text = refreshed["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        !text.contains("locally_excluded_dir"),
        "config change must refresh the index without a source edit: {text}"
    );
}

#[test]
fn test_default_index_materializes_under_codemap_dir() {
    // Default index path is now `.codemap/index` (Child 05 relocation), not the legacy
    // `.codemap-index`. The `.codemap/` dir is in EXCLUDED_DIRS so it never surfaces.
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn indexed_symbol() {}")]).unwrap();
    run_cli(&["index"], temp.path()).success();

    assert!(
        temp.path().join(".codemap/index").exists(),
        "index should materialize at .codemap/index"
    );
    assert!(
        !temp.path().join(".codemap-index").exists(),
        "the legacy .codemap-index must not be created"
    );

    // The index dir must not leak into the codemap output.
    run_cli(&["codemap"], temp.path())
        .success()
        .stdout(predicates::str::contains("- src ("))
        .stdout(predicates::str::contains(".codemap").not());
}

fn index_command(cwd: &std::path::Path, home: Option<&std::path::Path>) -> Command {
    let mut command = Command::cargo_bin("codemap-search").unwrap();
    command
        .current_dir(cwd)
        .env("CODEMAP_HOME", cwd.join("global-config"));
    match home {
        Some(home) => {
            command.env("HOME", home).env("USERPROFILE", home);
        }
        None => {
            command.env_remove("HOME").env_remove("USERPROFILE");
        }
    }
    command.arg("index");
    command
}

#[test]
fn test_index_refuses_user_home_before_creating_state() {
    let home = create_mock_repo(&[("src/lib.rs", "pub fn must_not_be_indexed() {}")]).unwrap();

    index_command(home.path(), Some(home.path()))
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Refusing to index the user home directory",
        ));

    assert!(
        !home.path().join(".codemap").exists(),
        "rejected home indexing must not create repo config or index state"
    );
}

#[test]
fn test_mcp_refuses_user_home_before_creating_state() {
    let home = create_mock_repo(&[("src/lib.rs", "pub fn must_not_start_mcp() {}")]).unwrap();
    let mut command = Command::cargo_bin("codemap-search").unwrap();
    command
        .current_dir(home.path())
        .env("CODEMAP_HOME", home.path().join("global-config"))
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .arg("mcp")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Refusing to index the user home directory",
        ));

    assert!(
        !home.path().join(".codemap").exists(),
        "rejected MCP startup must not create repo config or index state"
    );
}

#[test]
fn test_index_refuses_explicit_home_target_from_descendant_project() {
    let home = tempfile::tempdir().unwrap();
    let project = home.path().join("work/project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("src/lib.rs"), "pub fn project_symbol() {}").unwrap();

    let mut command = index_command(&project, Some(home.path()));
    command.arg(home.path());
    command.assert().failure().stderr(predicates::str::contains(
        "Refusing to index the user home directory",
    ));

    assert!(
        !project.join(".codemap").exists(),
        "the index directory must not be created before target validation"
    );
}

#[test]
fn test_index_allows_project_below_user_home() {
    let home = tempfile::tempdir().unwrap();
    let project = home.path().join("work/project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("src/lib.rs"),
        "pub fn allowed_project_symbol() {}",
    )
    .unwrap();

    index_command(&project, Some(home.path()))
        .assert()
        .success();

    assert!(
        project.join(".codemap/index").exists(),
        "a project below the home directory must remain indexable"
    );
}

#[test]
fn test_index_warns_and_continues_when_home_is_unknown() {
    let project = create_mock_repo(&[("src/lib.rs", "pub fn unknown_home_symbol() {}")]).unwrap();

    index_command(project.path(), None)
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "Cannot determine the user home directory",
        ));

    assert!(project.path().join(".codemap/index").exists());
}

// --- `[analysis.jev]`: defaults, template, migration and threshold normalization ------------

mod jev_config {
    use crate::e2e::helpers::{create_mock_repo, response_text, with_in_process_server, McpClient};
    use codemap_search::jev::mock::{answers, MockEvaluator};
    use codemap_search::jev::EvaluationRequest;
    use serde_json::json;
    use std::sync::Arc;

    const BUDGET_TS: &str =
        "export function keepMe(remainingBytes: number, footer: string): number {
  const reserve = footer.length + 8;
  if (remainingBytes <= reserve) {
    return 0;
  }
  return remainingBytes - reserve;
}

export function dropMe(input: string): string {
  const parts = input.split(\",\");
  const trimmed = parts.map((part) => part.trim());
  return trimmed.join(\"|\").toUpperCase();
}
";

    fn unrelated_judge(probability: f64) -> MockEvaluator {
        MockEvaluator::new(move |request: &EvaluationRequest| {
            Ok(request
                .questions()
                .iter()
                .map(|question| (question.id().clone(), answers::noul(1.0 - probability)))
                .collect())
        })
    }

    #[tokio::test]
    async fn test_jev_template_and_migration_add_a_commented_section_after_analysis() {
        // A fresh repo scaffolds the current template with the commented section.
        let temp = create_mock_repo(&[("src/a.rs", "fn scaffold_me() {}")]).unwrap();
        let mut client = McpClient::spawn(temp.path()).await.unwrap();
        client.send_request("initialize", json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } })).await.unwrap();
        let scaffolded = std::fs::read_to_string(temp.path().join(".codemap/config.toml")).unwrap();
        assert!(
            scaffolded.starts_with("# codemap-config-version: 27\n"),
            "{scaffolded}"
        );
        assert!(scaffolded.contains("\n[analysis.jev]\n"), "{scaffolded}");
        assert!(!scaffolded.contains("overview_enabled"));
        assert!(scaffolded.contains("# search_filter_enabled = false\n"));
        assert!(scaffolded.contains("# api_key_env = \"TYPESAFE_API_KEY\"\n"));
        assert!(scaffolded.contains("# search_filter_min_unrelated_probability = 0.70\n"));
        assert!(
            !scaffolded.contains("TYPESAFE_API_KEY =") && !scaffolded.contains("sk-"),
            "no secret is ever written"
        );
        drop(client);

        // A v23 file gains the section as one commented paragraph between [analysis] and the
        // next table, with its own settings untouched.
        let original = "# codemap-config-version: 23\n[output.search]\ndetail_file_limit = 7\n\n[analysis]\n# target_os = \"\"\n\n[filesystem_permissions]\nfind = \"workspace\"\n";
        let temp = create_mock_repo(&[
            (".codemap/config.toml", original),
            ("src/a.rs", "fn migrate_me() {}"),
        ])
        .unwrap();
        let mut client = McpClient::spawn(temp.path()).await.unwrap();
        client.send_request("initialize", json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } })).await.unwrap();
        let migrated = std::fs::read_to_string(temp.path().join(".codemap/config.toml")).unwrap();
        assert!(
            migrated.starts_with("# codemap-config-version: 27\n"),
            "{migrated}"
        );
        assert!(migrated.contains("detail_file_limit = 7\n"), "{migrated}");
        let analysis = migrated.find("[analysis]\n").expect("analysis header kept");
        let jev = migrated
            .find("# [analysis.jev]\n")
            .expect("commented jev header added");
        let permissions = migrated
            .find("[filesystem_permissions]\n")
            .expect("next table kept");
        assert!(
            analysis < jev && jev < permissions,
            "section order: {migrated}"
        );
        assert!(
            migrated.contains("# search_filter_enabled = false\n"),
            "{migrated}"
        );
        assert!(
            !migrated.contains("\n[analysis.jev]\n"),
            "migration never activates the section: {migrated}"
        );
        drop(client);

        // Existing active values survive; additional stage options remain commented.
        let already =
            "# codemap-config-version: 23\n[analysis.jev]\nsearch_filter_enabled = true\n";
        let temp = create_mock_repo(&[
            (".codemap/config.toml", already),
            ("src/a.rs", "fn keep_me() {}"),
        ])
        .unwrap();
        let mut client = McpClient::spawn(temp.path()).await.unwrap();
        client.send_request("initialize", json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } })).await.unwrap();
        let kept = std::fs::read_to_string(temp.path().join(".codemap/config.toml")).unwrap();
        assert!(kept.starts_with("# codemap-config-version: 27\n"), "{kept}");
        assert!(kept.contains("# read_filter_enabled = false\n"), "{kept}");
        assert!(kept.contains("# grep_filter_enabled = false\n"), "{kept}");
        assert_eq!(
            toml::from_str::<toml::Value>(&kept).unwrap(),
            toml::from_str::<toml::Value>(already).unwrap(),
            "active values must be unchanged: {kept}"
        );
    }

    #[tokio::test]
    async fn test_jev_threshold_overrides_and_invalid_values_reach_the_retention_policy() {
        // 0.90 keeps both bodies judged 0.80 unrelated (the output is the plain search);
        // an invalid 0.5 or a string falls back to the 0.70 default, which omits both. The
        // evaluator never sees the threshold.
        for (configured, expected_omissions) in [("0.90", 0), ("0.5", 2), ("\"seventy\"", 2)] {
            let config = format!("[analysis.jev]\nsearch_filter_enabled = true\nsearch_filter_min_unrelated_probability = {configured}\n");
            let temp = create_mock_repo(&[
                (".codemap/config.toml", &config),
                ("src/budget.ts", BUDGET_TS),
            ])
            .unwrap();
            let evaluator = Arc::new(unrelated_judge(0.80));
            let judge = Arc::clone(&evaluator);
            with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
                client
                    .register_task("where is the output budget reserved?")
                    .await;
                let plain = client
                    .plain_call("search", json!({ "query": "keepMe dropMe" }))
                    .await
                    .unwrap();
                let response = client
                    .call(
                        "tools/call",
                        json!({ "name": "search", "arguments": { "query": "keepMe dropMe" } }),
                    )
                    .await
                    .unwrap();
                let text = response_text(&response);
                assert_eq!(
                    judge.request_count(),
                    1,
                    "configured {configured}: one evaluation"
                );
                assert_eq!(
                    text.matches("- _omitted body:").count(),
                    expected_omissions,
                    "configured {configured}: {text}"
                );
                assert_eq!(
                    !text.contains("input.split"),
                    expected_omissions > 0,
                    "configured {configured}: {text}"
                );
                if expected_omissions == 0 {
                    assert_eq!(
                        text,
                        response_text(&plain),
                        "configured {configured}: all-keep is the plain output"
                    );
                }
                assert!(text.contains("dropMe (fn) [L9-13]"), "rows stay: {text}");
            })
            .await;
        }
    }
}
