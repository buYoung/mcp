use std::fs;
use crate::e2e::helpers::{create_mock_repo, run_cli};
use predicates::prelude::*;

#[test]
fn test_tree_sitter_rust_extraction() {
    let temp = create_mock_repo(&[
        ("src/main.rs", r#"
            pub struct Config {
                pub port: u16,
            }
            impl Config {
                pub fn load() -> Self {
                    Config { port: 8080 }
                }
            }
        "#)
    ]).unwrap();

    let assert = run_cli(&["parse", "src/main.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("Config"))
        .stdout(predicates::str::contains("port"))
        .stdout(predicates::str::contains("load"));
}

#[test]
fn test_tree_sitter_docstring_association() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", r#"
            /// This is a docstring for initialize.
            /// It has multiple lines.
            pub fn initialize() {}
        "#)
    ]).unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("This is a docstring for initialize"));
}

#[test]
fn test_tree_sitter_flags_todo_fixme() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", r#"
            // TODO: implement this
            // FIXME: critical bug here
            fn stub() {}
        "#)
    ]).unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("hasTodo"))
        .stdout(predicates::str::contains("hasFixme"));
}

#[test]
fn test_tree_sitter_flags_attributes() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", r#"
            #[deprecated(since = "1.0.0")]
            #[test]
            pub fn test_deprecated_feature() {}
        "#)
    ]).unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("isTest"))
        .stdout(predicates::str::contains("isExported"))
        .stdout(predicates::str::contains("isDeprecated"));
}

#[test]
fn test_tree_sitter_sub_tokenization() {
    let temp = create_mock_repo(&[]).unwrap();
    let assert = run_cli(&["tokenize", "handleLoginError"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("handle"))
        .stdout(predicates::str::contains("login"))
        .stdout(predicates::str::contains("error"));
}

#[test]
fn test_tree_sitter_empty_file() {
    let temp = create_mock_repo(&[
        ("src/empty.rs", "   \n\n  ")
    ]).unwrap();

    let assert = run_cli(&["parse", "src/empty.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("\"symbols\": []"))
        .stdout(predicates::str::contains("Config").not());
}

#[test]
fn test_tree_sitter_invalid_syntax() {
    let temp = create_mock_repo(&[
        ("src/bad.rs", "fn main() { struct Bad { }") // Missing closing braces
    ]).unwrap();

    // Invalid syntax should be parsed gracefully without panic
    let assert = run_cli(&["parse", "src/bad.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("Bad")); // Still extracts partial content
}

#[test]
fn test_tree_sitter_deeply_nested() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", r#"
            mod outer {
                mod inner {
                    pub struct Target {}
                }
            }
        "#)
    ]).unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("outer"))
        .stdout(predicates::str::contains("inner"))
        .stdout(predicates::str::contains("Target"));
}

#[test]
fn test_tree_sitter_large_file() {
    // Generate a file with >10,000 lines
    let mut content = String::new();
    for i in 0..10100 {
        content.push_str(&format!("// comment {}\nfn func_{}() {{}}\n", i, i));
    }
    let temp = create_mock_repo(&[("src/large.rs", &content)]).unwrap();

    let assert = run_cli(&["parse", "src/large.rs"], temp.path());
    assert.success();
}

#[test]
fn test_tree_sitter_special_chars() {
    let temp = create_mock_repo(&[
        ("src/lib.rs", r#"
            /// 🚀 This is a special docstring!
            pub fn handle_日本語() {}
        "#)
    ]).unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert.success()
        .stdout(predicates::str::contains("🚀"))
        .stdout(predicates::str::contains("日本語"));
}
