use crate::e2e::helpers::{create_mock_repo, run_cli};
use predicates::prelude::*;

#[test]
fn test_codemap_root_view() {
    let temp = create_mock_repo(&[
        ("src/main.rs", "fn main() {}"),
        ("src/lib.rs", "pub fn init() {}"),
    ])
    .unwrap();

    let assert = run_cli(&["codemap"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("src/main.rs"))
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_codemap_folder_view() {
    let temp = create_mock_repo(&[
        ("src/utils/mod.rs", "pub fn utils() {}"),
        ("src/utils/math.rs", "pub fn add() {}"),
    ])
    .unwrap();

    let assert = run_cli(&["codemap", "--path", "src/utils"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("src/utils/mod.rs"))
        .stdout(predicates::str::contains("src/utils/math.rs"));
}

#[test]
fn test_codemap_details_view() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        r#"
            /// Details view test
            pub fn details() {}
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["codemap", "--path", "src/lib.rs"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("details"))
        // File view is a trimmed outline now; docstrings are read/grep's job.
        .stdout(predicates::str::contains("Details view test").not());
}

#[test]
fn test_codemap_hierarchical_navigation() {
    let temp = create_mock_repo(&[
        ("src/main.rs", "fn main() {}"),
        ("src/core/mod.rs", "fn core_init() {}"),
        ("src/core/engine.rs", "/// Engine\npub fn run() {}"),
    ])
    .unwrap();

    // 1. Root view
    let assert_root = run_cli(&["codemap"], temp.path());
    assert_root
        .success()
        .stdout(predicates::str::contains("src/core"));

    // 2. Folder view
    let assert_folder = run_cli(&["codemap", "--path", "src/core"], temp.path());
    assert_folder
        .success()
        .stdout(predicates::str::contains("src/core/engine.rs"));

    // 3. Details view
    let assert_details = run_cli(&["codemap", "--path", "src/core/engine.rs"], temp.path());
    assert_details
        .success()
        .stdout(predicates::str::contains("run"))
        // Docstring is no longer rendered in the trimmed file outline.
        .stdout(predicates::str::contains("Engine").not());
}

#[test]
fn test_codemap_formatting_validation() {
    let temp = create_mock_repo(&[("src/lib.rs", "/// Formatted doc\npub fn check() {}")]).unwrap();

    let assert = run_cli(&["codemap", "--format", "llms-txt"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("llms.txt"));
}

#[test]
fn test_codemap_empty_directory() {
    let temp = create_mock_repo(&[]).unwrap();

    let assert = run_cli(&["codemap"], temp.path());
    // Empty folder should succeed but return message indicating empty repo
    assert
        .success()
        .stdout(predicates::str::contains("empty").or(predicates::str::contains("No files")));
}

#[test]
fn test_codemap_missing_folder() {
    let temp = create_mock_repo(&[]).unwrap();

    let assert = run_cli(&["codemap", "--path", "non_existent_folder"], temp.path());
    assert
        .failure()
        .stderr(predicates::str::contains("not found").or(predicates::str::contains("error")));
}

#[test]
fn test_codemap_missing_file() {
    let temp = create_mock_repo(&[]).unwrap();

    let assert = run_cli(&["codemap", "--path", "non_existent_file.rs"], temp.path());
    assert
        .failure()
        .stderr(predicates::str::contains("not found").or(predicates::str::contains("error")));
}

#[test]
fn test_codemap_deep_nested_dirs() {
    let temp = create_mock_repo(&[("src/a/b/c/d/e/f.rs", "pub fn deep() {}")]).unwrap();

    let assert = run_cli(&["codemap", "--path", "src/a/b/c/d"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("src/a/b/c/d/e"));
}

#[test]
fn test_codemap_non_source_files() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn code() {}"),
        ("src/image.png", "binary_data"),
        ("src/archive.zip", "zip_data"),
    ])
    .unwrap();

    let assert = run_cli(&["codemap"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("src/lib.rs"))
        .stdout(predicates::str::contains("src/image.png").not())
        .stdout(predicates::str::contains("src/archive.zip").not());
}
