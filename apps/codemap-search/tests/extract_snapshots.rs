//! Per-language extraction snapshot tests.
//!
//! Pins the full `ExtractedFile` output of `TreeSitterExtractor::extract` for one fixture
//! per supported source extension against a committed golden JSON file. This is the
//! behavior-preservation net for the upcoming `lang/` migration (briefset children 08/09),
//! which converts the inline per-extension branch chains (is_test / is_exported /
//! is_deprecated / docstring / owner / query selection) into registry hooks: a silently
//! flipped per-language flag would change a golden here even when the coarser parser
//! unit tests and the e2e suite stay green.
//!
//! Regeneration: snapshots document *current* behavior, not a correction. Intentional
//! updates (e.g. an explicitly approved behavior change) regenerate with:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test --manifest-path apps/codemap-search/Cargo.toml \
//!     --test extract_snapshots
//! ```
//!
//! then commit the changed goldens. A run without the env var compares and fails on any
//! drift, printing a line-level diff.

use std::path::PathBuf;

use codemap_search::parser::{CodeExtractor, TreeSitterExtractor};

/// Every supported source extension and the bare, stable `file_path` argument passed to
/// `extract` for it. The fixture file on disk is `sample.<ext>`; the `file_path` argument
/// is the same bare name (never an absolute path) so goldens stay machine-independent —
/// `extract` copies `file_path` verbatim into the output's `filePath` field and otherwise
/// consumes only the extension. js/jsx have their own fixtures even though they share the
/// TS/TSX query today, because the `lang/` migration reroutes their grammar/query and these
/// snapshots must catch any drift in that rerouting.
const FIXTURES: &[&str] = &[
    "sample.rs",
    "sample.py",
    "sample.ts",
    "sample.tsx",
    "sample.js",
    "sample.jsx",
    "sample.go",
    "sample.java",
    "sample.kt",
    "sample.c",
    "sample.cpp",
    "sample.hpp",
    "sample.s",
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extract")
}

fn golden_dir() -> PathBuf {
    fixtures_dir().join("golden")
}

/// Run `extract` on one fixture and return the pretty-printed `ExtractedFile` JSON.
fn extract_snapshot(file_name: &str) -> String {
    let fixture_path = fixtures_dir().join(file_name);
    let content = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture_path.display()));
    let extractor = TreeSitterExtractor::new();
    let extracted = extractor
        .extract(&content, file_name)
        .unwrap_or_else(|e| panic!("extract {file_name}: {e}"));
    serde_json::to_string_pretty(&extracted)
        .unwrap_or_else(|e| panic!("serialize {file_name}: {e}"))
}

/// First line index at which `actual` and `expected` differ, with both lines, for a
/// readable mismatch message (the raw JSON blocks are large and hard to eyeball).
fn first_diff(expected: &str, actual: &str) -> String {
    for (line_number, (expected_line, actual_line)) in
        expected.lines().zip(actual.lines()).enumerate()
    {
        if expected_line != actual_line {
            return format!(
                "first difference at line {}:\n  golden: {expected_line}\n  actual: {actual_line}",
                line_number + 1
            );
        }
    }
    format!(
        "golden has {} lines, actual has {} lines (no differing line within the shared prefix)",
        expected.lines().count(),
        actual.lines().count()
    )
}

/// One snapshot assertion per fixture. With `UPDATE_SNAPSHOTS=1` set, writes (or rewrites)
/// the golden; otherwise reads the golden — hard-failing if it is missing so a deleted
/// golden fails the suite instead of being silently skipped — and compares.
fn check_fixture(file_name: &str) {
    let actual = extract_snapshot(file_name);
    let golden_path = golden_dir().join(format!("{file_name}.json"));

    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(golden_dir()).expect("create golden dir");
        // The committed file is exactly the pretty JSON plus a trailing newline.
        std::fs::write(&golden_path, format!("{actual}\n"))
            .unwrap_or_else(|e| panic!("write golden {}: {e}", golden_path.display()));
        return;
    }

    let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!(
            "missing golden for {file_name} at {} ({e}); regenerate with \
             UPDATE_SNAPSHOTS=1 cargo test --test extract_snapshots",
            golden_path.display()
        )
    });
    let expected = golden.strip_suffix('\n').unwrap_or(&golden);

    assert!(
        expected == actual,
        "extraction snapshot drift for {file_name}\n{}\n(regenerate intentionally with \
         UPDATE_SNAPSHOTS=1 cargo test --test extract_snapshots)",
        first_diff(expected, &actual)
    );
}

#[test]
fn rust_extraction_matches_golden() {
    check_fixture("sample.rs");
}

#[test]
fn python_extraction_matches_golden() {
    check_fixture("sample.py");
}

#[test]
fn typescript_extraction_matches_golden() {
    check_fixture("sample.ts");
}

#[test]
fn tsx_extraction_matches_golden() {
    check_fixture("sample.tsx");
}

#[test]
fn javascript_extraction_matches_golden() {
    check_fixture("sample.js");
}

#[test]
fn jsx_extraction_matches_golden() {
    check_fixture("sample.jsx");
}

#[test]
fn go_extraction_matches_golden() {
    check_fixture("sample.go");
}

#[test]
fn java_extraction_matches_golden() {
    check_fixture("sample.java");
}

#[test]
fn kotlin_extraction_matches_golden() {
    check_fixture("sample.kt");
}

#[test]
fn c_extraction_matches_golden() {
    check_fixture("sample.c");
}

#[test]
fn cpp_extraction_matches_golden() {
    check_fixture("sample.cpp");
}

#[test]
fn hpp_extraction_matches_golden() {
    check_fixture("sample.hpp");
}

#[test]
fn asm_extraction_matches_golden() {
    check_fixture("sample.s");
}

/// Guards the fixture/golden roster itself: every extension in `FIXTURES` has a fixture
/// file on disk, and (outside regeneration) a committed golden. Without this, adding an
/// extension to the product but forgetting its fixture would go unnoticed.
#[test]
fn every_fixture_and_golden_is_present() {
    for file_name in FIXTURES {
        let fixture_path = fixtures_dir().join(file_name);
        assert!(
            fixture_path.exists(),
            "missing fixture file {}",
            fixture_path.display()
        );
        if std::env::var_os("UPDATE_SNAPSHOTS").is_none() {
            let golden_path = golden_dir().join(format!("{file_name}.json"));
            assert!(
                golden_path.exists(),
                "missing golden file {} (regenerate with UPDATE_SNAPSHOTS=1)",
                golden_path.display()
            );
        }
    }
}
