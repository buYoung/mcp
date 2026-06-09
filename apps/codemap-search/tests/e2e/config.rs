use crate::e2e::helpers::{create_mock_repo, run_cli, McpClient};
use predicates::prelude::*;

#[tokio::test]
async fn test_config_threshold_override_changes_branching() {
    // result_threshold = 1 means 2 matches (> 1) must return the codemap overview, where
    // the default threshold of 5 would return file details. Proves config drives branching.
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
        text.contains("Codemap overview"),
        "threshold=1 + 2 matches should branch to the overview: {text:?}"
    );
    assert!(
        !text.contains("fn shared_branch_fn"),
        "overview must not include raw source: {text:?}"
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
async fn test_respect_git_exclude_scopes_to_git_info_exclude_only() {
    // The dedicated `respect_git_exclude` toggle governs ONLY `.git/info/exclude`:
    //  - default (true): a file hidden by `.git/info/exclude` stays out of the index.
    //  - false: that file becomes searchable, BUT `.gitignore` is still honored.
    // `register_git_exclude = false` keeps the binary from touching `.git/info/exclude`.

    // Default: `.git/info/exclude` respected → the locally-excluded file is absent.
    let Some(default_repo) = make_git_repo_with_excludes("register_git_exclude = false\n") else {
        eprintln!("git unavailable — skipping respect_git_exclude test");
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
    let Some(override_repo) =
        make_git_repo_with_excludes("respect_git_exclude = false\nregister_git_exclude = false\n")
    else {
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
        "respect_git_exclude=false should index the .git/info/exclude-hidden file: {text:?}"
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
        ".gitignore must stay honored under respect_git_exclude=false: {text:?}"
    );
}

#[test]
fn test_git_exclude_registration_is_idempotent() {
    // AC#2 (Child 05): in a git repo, `.codemap/` is registered in `.git/info/exclude` so
    // the index/config stay out of `git status`; re-running must not duplicate the entry.
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn x() {}")]).unwrap();
    let init = std::process::Command::new("git")
        .arg("-C")
        .arg(temp.path())
        .arg("init")
        .output();
    if init.map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("git unavailable — skipping git-exclude registration test");
        return;
    }

    run_cli(&["index"], temp.path()).success();
    let exclude_path = temp.path().join(".git/info/exclude");
    let first = std::fs::read_to_string(&exclude_path).unwrap();
    assert!(
        first.contains(".codemap/"),
        ".git/info/exclude should register .codemap/: {first:?}"
    );

    // Idempotent: a second run leaves exactly one entry.
    run_cli(&["index"], temp.path()).success();
    let second = std::fs::read_to_string(&exclude_path).unwrap();
    assert_eq!(
        second.matches(".codemap/").count(),
        1,
        "git-exclude registration must be idempotent: {second:?}"
    );
}

#[test]
fn test_register_git_exclude_false_suppresses_write() {
    // The override side of the AC: register_git_exclude = false must suppress the
    // `.git/info/exclude` write entirely (the default-true path is covered above).
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "register_git_exclude = false\n"),
        ("src/lib.rs", "pub fn x() {}"),
    ])
    .unwrap();
    let init = std::process::Command::new("git")
        .arg("-C")
        .arg(temp.path())
        .arg("init")
        .output();
    if init.map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("git unavailable — skipping register_git_exclude=false test");
        return;
    }

    run_cli(&["index"], temp.path()).success();
    let contents =
        std::fs::read_to_string(temp.path().join(".git/info/exclude")).unwrap_or_default();
    assert!(
        !contents.contains(".codemap/"),
        "register_git_exclude=false must suppress the .git/info/exclude write: {contents:?}"
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
        .stdout(predicates::str::contains("src/lib.rs"))
        .stdout(predicates::str::contains(".codemap").not());
}
