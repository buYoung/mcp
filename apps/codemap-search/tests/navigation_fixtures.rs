//! Navigation-query fixture tests.
//!
//! These fixtures pin two separate contracts:
//! - `navigation.scm` is part of the runtime extraction query and must produce stable
//!   `NavigationFile` calls/imports/local bindings.
//! - `tags.scm` is a compile/fixture validation gate only; it is intentionally not included
//!   in the runtime query concat.
//!
//! Regeneration:
//!
//! ```sh
//! UPDATE_NAVIGATION_FIXTURES=1 cargo test --manifest-path apps/codemap-search/Cargo.toml \
//!     --test navigation_fixtures
//! ```

use std::path::{Path, PathBuf};

use codemap_search::parser::{CodeExtractor, CodeRange, TreeSitterExtractor};
use serde::Serialize;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

struct NavigationFixture {
    name: &'static str,
    source_file: &'static str,
    tags_query: &'static str,
}

const FIXTURES: &[NavigationFixture] = &[
    NavigationFixture {
        name: "typescript",
        source_file: "typescript/basic.ts",
        tags_query: "queries/typescript/tags.scm",
    },
    NavigationFixture {
        name: "tsx",
        source_file: "tsx/basic.tsx",
        tags_query: "queries/typescript/tags.scm",
    },
    NavigationFixture {
        name: "python",
        source_file: "python/basic.py",
        tags_query: "queries/python/tags.scm",
    },
    NavigationFixture {
        name: "go",
        source_file: "go/basic.go",
        tags_query: "queries/go/tags.scm",
    },
    NavigationFixture {
        name: "rust",
        source_file: "rust/basic.rs",
        tags_query: "queries/rust/tags.scm",
    },
    NavigationFixture {
        name: "java",
        source_file: "java/basic.java",
        tags_query: "queries/java/tags.scm",
    },
    NavigationFixture {
        name: "kotlin",
        source_file: "kotlin/basic.kt",
        tags_query: "queries/kotlin/tags.scm",
    },
    NavigationFixture {
        name: "c",
        source_file: "c/basic.c",
        tags_query: "queries/c/tags.scm",
    },
    NavigationFixture {
        name: "cpp",
        source_file: "cpp/basic.cpp",
        tags_query: "queries/cpp/tags.scm",
    },
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TagCapture {
    capture: String,
    text: String,
    range: CodeRange,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/navigation")
}

fn manifest_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn language_for_source(source_file: &str) -> Language {
    match Path::new(source_file)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
    {
        "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "py" => tree_sitter_python::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "kt" => tree_sitter_kotlin_ng::LANGUAGE.into(),
        "c" => tree_sitter_c::LANGUAGE.into(),
        "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        ext => panic!("unsupported navigation fixture extension: {ext}"),
    }
}

fn range_for_node(node: tree_sitter::Node) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("serialize navigation fixture")
}

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

fn compare_or_update(path: &Path, actual: &str) {
    if std::env::var_os("UPDATE_NAVIGATION_FIXTURES").is_some() {
        std::fs::write(path, format!("{actual}\n"))
            .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
        return;
    }

    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read expected fixture {}: {err}", path.display()));
    let expected = expected.strip_suffix('\n').unwrap_or(&expected);
    assert!(
        expected == actual,
        "navigation fixture drift at {}\n{}",
        path.display(),
        first_diff(expected, actual)
    );
}

fn actual_navigation_json(fixture: &NavigationFixture, source: &str) -> String {
    let extractor = TreeSitterExtractor::new();
    let extracted = extractor
        .extract(source, fixture.source_file)
        .unwrap_or_else(|err| panic!("extract {}: {err}", fixture.source_file));
    pretty_json(&extracted.navigation)
}

fn actual_tags_json(fixture: &NavigationFixture, source: &str) -> String {
    let language = language_for_source(fixture.source_file);
    let query_text = std::fs::read_to_string(manifest_path(fixture.tags_query))
        .unwrap_or_else(|err| panic!("read {}: {err}", fixture.tags_query));
    let query = Query::new(&language, &query_text)
        .unwrap_or_else(|err| panic!("compile {}: {err}", fixture.tags_query));
    let source_bytes = source.as_bytes();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .unwrap_or_else(|err| panic!("set language for {}: {err}", fixture.source_file));
    let tree = parser
        .parse(source, None)
        .unwrap_or_else(|| panic!("parse {}", fixture.source_file));
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    let mut captures = Vec::new();
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            let text = capture
                .node
                .utf8_text(source_bytes)
                .unwrap_or("")
                .trim()
                .to_string();
            captures.push(TagCapture {
                capture: capture_name.to_string(),
                text,
                range: range_for_node(capture.node),
            });
        }
    }
    pretty_json(&captures)
}

fn check_fixture(fixture: &NavigationFixture) {
    let source_path = fixtures_dir().join(fixture.source_file);
    assert!(
        source_path.exists(),
        "missing navigation fixture source for {} at {}",
        fixture.name,
        source_path.display()
    );
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", source_path.display()));
    let fixture_dir = source_path
        .parent()
        .unwrap_or_else(|| panic!("fixture without parent: {}", fixture.source_file));
    compare_or_update(
        &fixture_dir.join("expected.navigation.json"),
        &actual_navigation_json(fixture, &source),
    );
    compare_or_update(
        &fixture_dir.join("expected.tags.json"),
        &actual_tags_json(fixture, &source),
    );
}

#[test]
fn navigation_fixtures_match_expected_json() {
    for fixture in FIXTURES {
        check_fixture(fixture);
    }
}
