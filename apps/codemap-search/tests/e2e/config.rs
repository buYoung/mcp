use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

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
        text.matches("### File:").count(),
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
    let Some(override_repo) = make_git_repo_with_excludes("use_git_exclude = false\n") else {
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
