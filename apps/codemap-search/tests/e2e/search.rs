use crate::e2e::helpers::{create_mock_repo, run_cli};
use filetime::{set_file_mtime, FileTime};
use predicates::prelude::*;
use std::fs;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_bm25_basic_search() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn find_my_function_name() {}")]).unwrap();

    // Index the repository
    let assert_index = run_cli(&["index"], temp.path());
    assert_index.success();

    // Search for the function
    let assert_search = run_cli(&["search", "find_my_function_name"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_bm25_field_weighting() {
    let temp = create_mock_repo(&[
        ("src/file_a.rs", "/// QueryTerm\npub fn dummy() {}"), // Term in docstring
        ("src/file_b.rs", "pub fn QueryTerm() {}"), // Term in symbol name (highest weight)
        ("src/file_c.rs", "pub fn other() { let x = \"QueryTerm\"; }"), // Term only in a string literal
    ])
    .unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Search for QueryTerm: all three tiers rank in (v3 indexes string literals at the
    // lowest boost), with the symbol match (file_b) first.
    let assert_search = run_cli(&["search", "QueryTerm"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::starts_with("src/file_b.rs"))
        .stdout(predicates::str::contains("src/file_a.rs"))
        .stdout(predicates::str::contains("src/file_c.rs"));
}

#[test]
fn test_bm25_incremental_no_change() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn hello() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    let index_dir = temp.path().join(".codemap/index");
    let initial_mtime = if index_dir.exists() {
        fs::metadata(&index_dir).unwrap().modified().unwrap()
    } else {
        std::time::SystemTime::now()
    };

    sleep(Duration::from_millis(100));

    // Index again with no changes
    let _ = run_cli(&["index"], temp.path());

    let final_mtime = if index_dir.exists() {
        fs::metadata(&index_dir).unwrap().modified().unwrap()
    } else {
        initial_mtime
    };

    assert_eq!(initial_mtime, final_mtime);
}

#[test]
fn test_bm25_incremental_update() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn hello() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Modify a file
    let file_path = temp.path().join("src/lib.rs");
    fs::write(&file_path, "pub fn hello_world() {}").unwrap();

    // Set a newer mtime
    let new_time =
        FileTime::from_system_time(std::time::SystemTime::now() + Duration::from_secs(10));
    set_file_mtime(&file_path, new_time).unwrap();

    // Re-index
    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "hello_world"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_bm25_index_persistence() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn persistent_func() {}")]).unwrap();

    // 1. Run index
    let _ = run_cli(&["index"], temp.path());

    // 2. Search immediately
    let assert_search_1 = run_cli(&["search", "persistent_func"], temp.path());
    assert_search_1
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));

    // 3. Search in a fresh process, ensuring index is persistent
    let assert_search_2 = run_cli(&["search", "persistent_func"], temp.path());
    assert_search_2
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}

#[test]
fn test_bm25_search_non_existent() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn hello() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "NonExistentFunctionToken"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs").not());
}

#[test]
fn test_bm25_search_special_chars() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn find_regex() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Search with wildcards, regex chars, or special punctuations
    let assert_search = run_cli(&["search", "*()!@#+$^&?"], temp.path());
    assert_search.success();
}

#[test]
fn test_bm25_concurrent_access() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn concurrent() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // We can run two searches or an index and a search concurrently.
    // In our test skeleton, we simulate this by spawning multiple CLI commands.
    use std::thread;
    let path_clone_1 = temp.path().to_path_buf();
    let path_clone_2 = temp.path().to_path_buf();

    let handle1 = thread::spawn(move || run_cli(&["search", "concurrent"], &path_clone_1));
    let handle2 = thread::spawn(move || run_cli(&["index"], &path_clone_2));

    let _ = handle1.join();
    let _ = handle2.join();
}

#[test]
fn test_bm25_corrupt_index() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn test() {}")]).unwrap();

    let _ = run_cli(&["index"], temp.path());

    // Corrupt index files in .codemap/index
    let index_dir = temp.path().join(".codemap/index");
    if index_dir.exists() {
        let meta_file = index_dir.join("meta.json");
        fs::write(meta_file, "{invalid json}").unwrap();
    }

    // Server should auto-recovery/rebuild index
    let assert_search = run_cli(&["search", "test"], temp.path());
    assert_search.success();
}

#[test]
fn test_bm25_mtime_oscillation() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn oscillating() {}")]).unwrap();

    let file_path = temp.path().join("src/lib.rs");
    let _ = run_cli(&["index"], temp.path());

    // Set mtime backwards (oscillation)
    let past_time =
        FileTime::from_system_time(std::time::SystemTime::now() - Duration::from_secs(3600));
    set_file_mtime(&file_path, past_time).unwrap();

    let _ = run_cli(&["index"], temp.path());

    let assert_search = run_cli(&["search", "oscillating"], temp.path());
    assert_search.success();
}

#[test]
fn test_bm25_partial_failure_non_utf8() {
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn utf8_function() {}")]).unwrap();

    // Create a file with invalid UTF-8 (binary payload)
    let invalid_utf8_path = temp.path().join("src/binary.rs");
    std::fs::write(&invalid_utf8_path, b"\xFF\xFE\xFD\xFC").unwrap();

    // Index the directory (should print warning but succeed overall)
    let assert_index = run_cli(&["index"], temp.path());
    assert_index.success();

    // Verify search still works for the valid file
    let assert_search = run_cli(&["search", "utf8_function"], temp.path());
    assert_search
        .success()
        .stdout(predicates::str::contains("src/lib.rs"));
}
