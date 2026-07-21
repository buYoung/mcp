use crate::e2e::helpers::{create_mock_repo, run_cli};
use predicates::prelude::*;

#[test]
fn test_codemap_root_view() {
    let temp = create_mock_repo(&[
        ("src/main.rs", "fn main() {}"),
        ("src/lib.rs", "pub fn init() {}"),
    ])
    .unwrap();

    // Root view keeps the directory skeleton and includes bounded compact file-symbol rows.
    let assert = run_cli(&["codemap"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("- src ("))
        .stdout(predicates::str::contains("src/main.rs"))
        .stdout(predicates::str::contains("{fn: main}"));
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

    let assert_backslash = run_cli(&["codemap", "--path", "src\\lib.rs"], temp.path());
    assert_backslash
        .success()
        .stdout(predicates::str::contains("details"))
        .stdout(predicates::str::contains("Details view test").not());

    let assert_dot_backslash = run_cli(&["codemap", "--path", ".\\src\\lib.rs"], temp.path());
    assert_dot_backslash
        .success()
        .stdout(predicates::str::contains("details"))
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

    // 1. Root view — `src`'s only child `core` is a leaf directory, so it folds onto
    // the `src` line (Rust-`use`-style): `- src (..): core (..)`.
    let assert_root = run_cli(&["codemap"], temp.path());
    assert_root
        .success()
        .stdout(predicates::str::contains("core ("));

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

    // The `src` directory surfaces (it holds a parseable source file); the binary
    // assets never do. Root view rolls files up by directory rather than listing each.
    let assert = run_cli(&["codemap"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("- src ("))
        .stdout(predicates::str::contains("image.png").not())
        .stdout(predicates::str::contains("archive.zip").not());
}

#[test]
fn test_codemap_renders_structured_and_graphql_declarations() {
    let temp = create_mock_repo(&[
        ("config.yaml", "server:\n  port: 5000\n"),
        (
            "schema.graphql",
            "fragment UserFields on User { id }\nquery GetUser { user { id } }\n",
        ),
    ])
    .unwrap();
    run_cli(&["codemap", "--path", "config.yaml"], temp.path())
        .success()
        .stdout(predicates::str::contains("server.port"));
    run_cli(&["codemap", "--path", "schema.graphql"], temp.path())
        .success()
        .stdout(predicates::str::contains("UserFields"))
        .stdout(predicates::str::contains("GetUser"));
}

#[test]
fn test_codemap_renders_every_checked_priority_grammar_capability() {
    let cases = [
        ("config.json", r#"{"service": 1}"#, "service"),
        ("config.jsonc", "// comment\n{ \"service\": 1 }", "service"),
        ("config.toml", "service = 1", "service"),
        ("config.yaml", "service: 1", "service"),
        ("config.yml", "service: 1", "service"),
        ("page.html", "<main />", "main"),
        ("page.htm", "<main />", "main"),
        ("page.xml", "<main />", "main"),
        ("schema.xsd", "<main />", "main"),
        ("page.xsl", "<main />", "main"),
        ("page.xslt", "<main />", "main"),
        ("Info.plist", "<main />", "main"),
        ("app.csproj", "<main />", "main"),
        ("app.props", "<main />", "main"),
        ("app.targets", "<main />", "main"),
        ("site.css", ".card {}", ".card"),
        ("deploy.sh", "run() { :; }", "run"),
        ("deploy.bash", "run() { :; }", "run"),
        ("main.hcl", "variable \"region\" {}", "region"),
        ("main.tf", "terraform {}", "terraform"),
        ("api.proto", "message Request {}", "Request"),
        (
            "schema.graphql",
            "schema { query: Query }\ntype Query { id: ID! }",
            "schema",
        ),
        ("schema.gql", "type Query { id: ID! }", "Query"),
    ];
    let temp = create_mock_repo(
        &cases
            .iter()
            .map(|(path, body, _)| (*path, *body))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for (path, _, symbol) in cases {
        run_cli(&["codemap", "--path", path], temp.path())
            .success()
            .stdout(predicates::str::contains(symbol));
    }
}

#[test]
fn test_codemap_renders_tfvars_detail_with_an_explicit_empty_symbol_boundary() {
    let temp = create_mock_repo(&[("values.tfvars", "region = \"kr\"\n")]).unwrap();

    run_cli(&["codemap", "--path", "values.tfvars"], temp.path())
        .success()
        .stdout("# Detailed Codemap: values.tfvars (1 lines)\n\n## Symbols\n\n");
}

#[test]
fn test_codemap_excludes_minified_and_generated_bundles() {
    let temp = create_mock_repo(&[
        ("src/keep.js", "function visible_codemap_symbol() {}"),
        (
            "src/generated.MIN.js",
            "function hidden_minified_codemap_symbol() {}",
        ),
        (
            "src/generated.bundle.js",
            "function hidden_bundle_codemap_symbol() {}",
        ),
    ])
    .unwrap();

    run_cli(&["codemap", "--path", "src"], temp.path())
        .success()
        .stdout(predicates::str::contains("visible_codemap_symbol"))
        .stdout(predicates::str::contains("hidden_minified_codemap_symbol").not())
        .stdout(predicates::str::contains("hidden_bundle_codemap_symbol").not());
}
