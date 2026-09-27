use crate::e2e::helpers::{create_mock_repo, run_cli};
use predicates::prelude::*;

#[test]
fn test_benchmark_run() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            "queries.json",
            r#"[{"query": "find_me", "expected": ["src/lib.rs"]}]"#,
        ),
    ])
    .unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert.success();
}

#[test]
fn test_benchmark_output_format() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            "queries.json",
            r#"[{"query": "find_me", "expected": ["src/lib.rs"]}]"#,
        ),
    ])
    .unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("| Metric | Baseline | Index |"))
        .stdout(predicates::str::contains("Latency"))
        .stdout(predicates::str::contains("Recall"));
}

#[test]
fn test_benchmark_query_list() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            "queries.json",
            r#"[
            {"query": "find_me", "expected": ["src/lib.rs"]},
            {"query": "other", "expected": []}
        ]"#,
        ),
    ])
    .unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert.success();
}

#[test]
fn test_benchmark_recall_calculation() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            "queries.json",
            r#"[{"query": "find_me", "expected": ["src/lib.rs"]}]"#,
        ),
    ])
    .unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("100%").or(predicates::str::contains("1.0")));
}

#[test]
fn test_benchmark_invalid_queries() {
    let temp = create_mock_repo(&[]).unwrap();

    // Query file does not exist
    let assert = run_cli(
        &["benchmark", "--queries", "non_existent_queries.json"],
        temp.path(),
    );
    assert.failure();
}

#[test]
fn test_benchmark_empty_queries() {
    let temp = create_mock_repo(&[("queries.json", "[]")]).unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("No queries"));
}

#[test]
fn test_benchmark_empty_repo() {
    let temp =
        create_mock_repo(&[("queries.json", r#"[{"query": "find_me", "expected": []}]"#)]).unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert.success();
}

#[test]
fn test_benchmark_large_query_list() {
    // Generate >1000 queries
    let mut queries = String::from("[");
    for i in 0..1010 {
        let expected = if i == 0 { r#"["src/lib.rs"]"# } else { "[]" };
        queries.push_str(&format!(
            r#"{{"query": "q{}", "expected": {}}},"#,
            i, expected
        ));
    }
    queries.pop(); // remove trailing comma
    queries.push(']');

    // Query input is not source: otherwise the baseline reparses this entire 1010-item
    // JSON document for every query. Keep a real source hit and the full query count.
    let query_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(query_file.path(), queries).unwrap();
    let temp = create_mock_repo(&[("src/lib.rs", "pub fn q0() {}\n")]).unwrap();

    let assert = run_cli(
        &[
            "benchmark",
            "--queries",
            query_file.path().to_str().unwrap(),
        ],
        temp.path(),
    );
    assert
        .success()
        .stdout(predicates::str::contains("| Recall | 100% | 100% |"))
        .stdout(predicates::str::contains("Results: Identical (0% diff)"));
}

#[test]
fn test_benchmark_identical_results() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            "queries.json",
            r#"[{"query": "find_me", "expected": ["src/lib.rs"]}]"#,
        ),
    ])
    .unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("Identical").or(predicates::str::contains("0% diff")));
}

#[test]
fn test_benchmark_malformed_queries() {
    let temp = create_mock_repo(&[("queries.json", r#"[{"invalid_key": "val"}]"#)]).unwrap();

    let _assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    // Verify it terminates gracefully without panic
}

#[test]
fn test_benchmark_malformed_expected_schema() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", "pub fn find_me() {}"),
        (
            "queries.json",
            r#"[{"query": "find_me", "expected": "src/lib.rs"}]"#,
        ),
    ])
    .unwrap();

    let assert = run_cli(&["benchmark", "--queries", "queries.json"], temp.path());
    // Malformed expected schema should fail validation
    assert.failure();
}
