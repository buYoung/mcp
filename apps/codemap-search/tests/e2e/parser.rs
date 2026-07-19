use crate::e2e::helpers::{create_mock_repo, run_cli};
use predicates::prelude::*;
use serde_json::Value;

fn parsed_file(file: &str, content: &str) -> Value {
    let temp = create_mock_repo(&[(file, content)]).unwrap();
    let output = run_cli(&["parse", file], temp.path())
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).expect("parse must retain its JSON contract")
}

fn symbol<'a>(value: &'a Value, name: &str) -> &'a Value {
    value["symbols"]
        .as_array()
        .and_then(|symbols| symbols.iter().find(|symbol| symbol["name"] == name))
        .unwrap_or_else(|| panic!("missing symbol {name}: {value}"))
}

#[test]
fn test_tree_sitter_rust_extraction() {
    let temp = create_mock_repo(&[(
        "src/main.rs",
        r#"
            pub struct Config {
                pub port: u16,
            }
            impl Config {
                pub fn load() -> Self {
                    Config { port: 8080 }
                }
            }
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["parse", "src/main.rs"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("Config"))
        .stdout(predicates::str::contains("port"))
        .stdout(predicates::str::contains("load"));
}

#[test]
fn test_tree_sitter_docstring_association() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        r#"
            /// This is a docstring for initialize.
            /// It has multiple lines.
            pub fn initialize() {}
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert.success().stdout(predicates::str::contains(
        "This is a docstring for initialize",
    ));
}

#[test]
fn test_tree_sitter_flags_todo_fixme() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        r#"
            // TODO: implement this
            // FIXME: critical bug here
            fn stub() {}
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("hasTodo"))
        .stdout(predicates::str::contains("hasFixme"));
}

#[test]
fn test_tree_sitter_flags_attributes() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        r#"
            #[deprecated(since = "1.0.0")]
            #[test]
            pub fn test_deprecated_feature() {}
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("isTest"))
        .stdout(predicates::str::contains("isExported"))
        .stdout(predicates::str::contains("isDeprecated"));
}

#[test]
fn test_tree_sitter_sub_tokenization() {
    let temp = create_mock_repo(&[]).unwrap();
    let assert = run_cli(&["tokenize", "handleLoginError"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("handle"))
        .stdout(predicates::str::contains("login"))
        .stdout(predicates::str::contains("error"));
}

#[test]
fn test_tree_sitter_empty_file() {
    let temp = create_mock_repo(&[("src/empty.rs", "   \n\n  ")]).unwrap();

    let assert = run_cli(&["parse", "src/empty.rs"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("\"symbols\": []"))
        .stdout(predicates::str::contains("Config").not());
}

#[test]
fn test_tree_sitter_invalid_syntax() {
    let temp = create_mock_repo(&[
        ("src/bad.rs", "fn main() { struct Bad { }"), // Missing closing braces
    ])
    .unwrap();

    // Invalid syntax should be parsed gracefully without panic
    let assert = run_cli(&["parse", "src/bad.rs"], temp.path());
    assert.success().stdout(predicates::str::contains("Bad")); // Still extracts partial content
}

#[test]
fn test_tree_sitter_deeply_nested() {
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        r#"
            mod outer {
                mod inner {
                    pub struct Target {}
                }
            }
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert
        .success()
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
    let temp = create_mock_repo(&[(
        "src/lib.rs",
        r#"
            /// 🚀 This is a special docstring!
            pub fn handle_日本語() {}
        "#,
    )])
    .unwrap();

    let assert = run_cli(&["parse", "src/lib.rs"], temp.path());
    assert
        .success()
        .stdout(predicates::str::contains("🚀"))
        .stdout(predicates::str::contains("日本語"));
}

#[test]
fn test_existing_language_extraction_matrix() {
    // Each row uses the declaration forms documented by the grammar version locked in this
    // package. These are public CLI reproductions: a future query change must preserve both
    // the extracted name/kind and the original source line.
    type ExpectedSymbol<'a> = (&'a str, &'a str, usize);
    type ExtractionCase<'a> = (&'a str, &'a str, &'a [ExpectedSymbol<'a>]);
    let cases: &[ExtractionCase<'_>] = &[
        (
            "src/matrix.rs",
            "pub trait RustMatrix {}\nimpl RustMatrix { pub fn rust_matrix_method() {} }\n",
            &[("RustMatrix", "trait", 1), ("rust_matrix_method", "fn", 2)],
        ),
        (
            "src/matrix.go",
            "type GoMatrix struct{}\nfunc (g GoMatrix) goMatrixMethod() {}\n",
            &[("GoMatrix", "struct", 1), ("goMatrixMethod", "fn", 2)],
        ),
        (
            "src/Matrix.java",
            "class JavaMatrixClass {}\ninterface JavaMatrix { void javaMatrixMethod(); }\nrecord JavaMatrixRecord(int value) {}\n",
            &[
                ("JavaMatrixClass", "class", 1),
                ("JavaMatrix", "interface", 2),
                ("javaMatrixMethod", "fn", 2),
                ("JavaMatrixRecord", "record", 3),
            ],
        ),
        (
            "src/Matrix.kt",
            "class KotlinMatrix {\n  fun kotlinMatrixMethod() {}\n}\nobject KotlinMatrixObject {\n  fun objectMethod() {}\n}\n",
            &[
                ("KotlinMatrix", "class", 1),
                ("kotlinMatrixMethod", "fn", 2),
                ("KotlinMatrixObject", "object", 4),
                ("objectMethod", "fn", 5),
            ],
        ),
        (
            "src/matrix.py",
            "@decorator\nclass PythonMatrix:\n    @decorator\n    def python_matrix_method(self):\n        pass\n",
            &[
                ("PythonMatrix", "class", 2),
                ("python_matrix_method", "fn", 4),
            ],
        ),
        (
            "src/matrix.c",
            "struct CMatrix { int field; };\nint c_matrix_fn(void) { return 0; }\n",
            &[("CMatrix", "struct", 1), ("c_matrix_fn", "fn", 2)],
        ),
        (
            "src/matrix.cpp",
            "namespace Matrix { template<class T> class CppMatrix {}; }\ntemplate<class T> T cpp_matrix_fn(T value) { return value; }\n",
            &[("CppMatrix", "class", 1), ("cpp_matrix_fn", "fn", 2)],
        ),
        (
            "src/matrix.s",
            ".globl asm_matrix_label\nasm_matrix_label:\n  ret\n.macro asm_matrix_macro\n.endm\n",
            &[
                ("asm_matrix_label", "fn", 2),
                ("asm_matrix_macro", "fn", 4),
            ],
        ),
    ];

    for (file, content, expected_symbols) in cases {
        let parsed = parsed_file(file, content);
        for &(name, kind, start_line) in *expected_symbols {
            let extracted = symbol(&parsed, name);
            assert_eq!(extracted["kind"], kind, "{file}");
            assert_eq!(extracted["range"]["startLine"], start_line, "{file}");
        }
        assert!(
            parsed["navigation"].is_object(),
            "{file} must retain the observable parse-navigation shape"
        );
    }
}

#[test]
fn test_existing_language_cross_language_sentinels_are_not_symbols() {
    // This is a public parse-negative guarantee only. The separately maintained parser unit
    // matrix covers index-auxiliary definitions/references, which are not part of this JSON.
    let cases = [
        (
            "rs",
            "package go_only_sentinel\nfunc go_only_sentinel() {}\n",
            "go_only_sentinel",
        ),
        (
            "go",
            "trait rust_only_sentinel { fn run(&self); }\n",
            "rust_only_sentinel",
        ),
        (
            "java",
            "object kotlin_only_sentinel { fun run() = Unit }\n",
            "kotlin_only_sentinel",
        ),
        (
            "kt",
            "def python_only_sentinel():\n    pass\n",
            "python_only_sentinel",
        ),
        (
            "py",
            "struct c_only_sentinel { int value; };\n",
            "c_only_sentinel",
        ),
        (
            "c",
            "template <typename T> requires CppOnlySentinel<T> void run(T value) {}\n",
            "CppOnlySentinel",
        ),
        (
            "cpp",
            ".globl assembly_only_sentinel\nassembly_only_sentinel:\n  ret\n",
            "assembly_only_sentinel",
        ),
        (
            "s",
            "class java_only_sentinel { void run() {} }\n",
            "java_only_sentinel",
        ),
    ];
    for (extension, content, sentinel) in cases {
        let file = format!("src/cross_language.{extension}");
        let parsed = parsed_file(&file, content);
        assert!(
            parsed["symbols"]
                .as_array()
                .is_some_and(|symbols| symbols.iter().all(|symbol| symbol["name"] != sentinel)),
            "{file} must not turn actual foreign-language syntax into a symbol"
        );
    }
}

#[test]
fn test_sql_declarations_are_extracted_without_navigation() {
    let parsed = parsed_file(
        "schema/matrix.sql",
        "CREATE TABLE sql_matrix_table (id INT, note TEXT DEFAULT 'sql_matrix_literal');\nCREATE VIEW sql_matrix_view AS SELECT id FROM sql_matrix_table;\nCREATE MATERIALIZED VIEW sql_matrix_materialized_view AS\nSELECT id, 123, TRUE, FALSE, NULL /* TODO retain materialized body */ FROM sql_matrix_table;\nCREATE FUNCTION sql_matrix_function() RETURNS INT LANGUAGE SQL AS $$ SELECT 1; $$;\n",
    );
    assert_eq!(symbol(&parsed, "sql_matrix_table")["kind"], "struct");
    assert_eq!(symbol(&parsed, "sql_matrix_view")["kind"], "type");
    assert_eq!(
        symbol(&parsed, "sql_matrix_materialized_view")["kind"],
        "type"
    );
    assert_eq!(
        symbol(&parsed, "sql_matrix_materialized_view")["range"]["endLine"],
        4
    );
    assert_eq!(
        symbol(&parsed, "sql_matrix_materialized_view")["flags"]["hasTodo"],
        true
    );
    assert_eq!(symbol(&parsed, "sql_matrix_function")["kind"], "fn");
    assert!(parsed["literals"]
        .as_array()
        .is_some_and(|literals| literals
            .iter()
            .any(|literal| literal["text"] == "sql_matrix_literal")));
    assert!(parsed["literals"].as_array().is_some_and(|literals| {
        literals.iter().all(|literal| {
            !matches!(
                literal["text"].as_str(),
                Some("123" | "TRUE" | "FALSE" | "NULL")
            )
        })
    }));
    assert!(parsed["navigation"].is_null());
}

#[test]
fn test_composite_boundaries_attributes_and_source_order() {
    let vue_source = "<template><p>한글 Don't \"stop</p></template><SCRIPT data-kind=\"client\" LANG='TypeScript' setup>function vue_boundary_symbol() {}</SCRIPT>\r\n";
    let vue = parsed_file("src/boundary.vue", vue_source);
    let vue_symbol = symbol(&vue, "vue_boundary_symbol");
    assert_eq!(vue_symbol["range"]["startLine"], 1);
    assert_eq!(
        vue_symbol["range"]["startCol"],
        vue_source.find("function vue_boundary_symbol").unwrap() + 1
    );

    let script_after_quote = parsed_file(
        "src/quote.vue",
        "\"<script>function script_after_markup_quote() {}</script>",
    );
    assert!(symbol(&script_after_quote, "script_after_markup_quote")["range"]["startCol"].is_u64());

    let svelte = parsed_file(
        "src/boundary.svelte",
        "<script>// first block comment</script><script>function svelte_after_comment() {}</script><script LANG='ts' context=\"module\">function svelte_typed_after_comment() {}</script>",
    );
    let svelte_symbols = svelte["symbols"].as_array().unwrap();
    let comment_index = svelte_symbols
        .iter()
        .position(|item| item["name"] == "svelte_after_comment")
        .unwrap();
    let typed_index = svelte_symbols
        .iter()
        .position(|item| item["name"] == "svelte_typed_after_comment")
        .unwrap();
    assert!(comment_index < typed_index);

    let svelte_coordinate_source =
        "<div>한글</div>\r\n<script>\r\n  function exact_svelte_coordinate() {}\r\n</script>\r\n";
    let svelte_coordinate = parsed_file("src/coordinate.svelte", svelte_coordinate_source);
    let svelte_coordinate_symbol = symbol(&svelte_coordinate, "exact_svelte_coordinate");
    assert_eq!(svelte_coordinate_symbol["range"]["startLine"], 3);
    assert_eq!(svelte_coordinate_symbol["range"]["startCol"], 3);

    let incomplete = parsed_file(
        "src/incomplete.svelte",
        "<script>const incomplete =</script><script>function svelte_after_incomplete() {}</script>",
    );
    assert!(symbol(&incomplete, "svelte_after_incomplete")["range"]["startCol"].is_u64());

    let mixed = parsed_file(
        "src/order.svelte",
        "<script lang=\"TS\">function first_in_file() {}</script><script>function second_in_file() {}</script>",
    );
    let mixed_symbols = mixed["symbols"].as_array().unwrap();
    assert!(
        mixed_symbols
            .iter()
            .position(|item| item["name"] == "first_in_file")
            .unwrap()
            < mixed_symbols
                .iter()
                .position(|item| item["name"] == "second_in_file")
                .unwrap()
    );

    let astro = parsed_file(
        "src/boundary.astro",
        "<html><body><script>const marker = \"</\"; function nested_astro_script() {}</script></body></html>",
    );
    assert!(symbol(&astro, "nested_astro_script")["range"]["startCol"].is_u64());

    let astro_coordinate_source =
        "<main>한글<script>function exact_astro_coordinate() {}</script></main>\r\n";
    let astro_coordinate = parsed_file("src/coordinate.astro", astro_coordinate_source);
    let astro_coordinate_symbol = symbol(&astro_coordinate, "exact_astro_coordinate");
    assert_eq!(astro_coordinate_symbol["range"]["startLine"], 1);
    assert_eq!(
        astro_coordinate_symbol["range"]["startCol"],
        astro_coordinate_source
            .find("function exact_astro_coordinate")
            .unwrap()
            + 1
    );
}

#[test]
fn test_astro_excludes_expression_strings_and_capitalized_components() {
    let astro = parsed_file(
        "src/expression.astro",
        "<div>{\"<script>function expression_only_symbol() {}</script>\"}</div>\n<Script>function component_child_text() {}</Script>\n{enabled && <script>function expression_markup_symbol() {}</script>}\n<script>function real_astro_symbol() {}</script>\n",
    );
    assert!(astro.to_string().contains("real_astro_symbol"));
    assert!(astro.to_string().contains("expression_markup_symbol"));
    assert!(!astro.to_string().contains("expression_only_symbol"));
    assert!(!astro.to_string().contains("component_child_text"));
}

#[test]
fn test_unterminated_astro_frontmatter_is_not_reinterpreted_as_markup() {
    let astro = parsed_file(
        "src/unterminated.astro",
        "---\r\nconst not_verified = true;\r\n<script>function not_verified_script() {}</script>\r\n",
    );
    assert_eq!(astro["symbols"], serde_json::json!([]));
    assert!(!astro.to_string().contains("not_verified_script"));
}

#[test]
fn test_composite_code_preserves_original_lines_and_excludes_non_code() {
    let vue = parsed_file(
        "src/matrix.vue",
        "<template>\n<div>vue_template_only_token</div>\n<!-- <script>const vue_fake_script = 'no';</script> -->\n\"<script>const vue_template_string_fake_script = 'no';</script>\"\n</template>\n<script setup lang=\"ts\">\nconst vue_line_preserved = 'vue_literal_kept';\nfunction vue_real_call() { return vue_line_preserved; }\n</script>\n<style>.vue_style_only_token { color: red; }</style>\n",
    );
    assert_eq!(symbol(&vue, "vue_real_call")["range"]["startLine"], 8);
    assert!(vue["literals"].as_array().is_some_and(|literals| {
        literals
            .iter()
            .any(|literal| literal["text"] == "vue_literal_kept")
    }));
    let vue_json = vue.to_string();
    assert!(!vue_json.contains("vue_template_only_token"));
    assert!(!vue_json.contains("vue_style_only_token"));
    assert!(!vue_json.contains("vue_fake_script"));
    assert!(!vue_json.contains("vue_template_string_fake_script"));

    let svelte = parsed_file(
        "src/matrix.svelte",
        "<script context=\"module\">\nexport function svelte_module_symbol() {}\n</script>\n<script>\nfunction svelte_instance_symbol() { return svelte_module_symbol(); }\n</script>\n<div>svelte_template_only_token</div>\n",
    );
    assert_eq!(
        symbol(&svelte, "svelte_module_symbol")["range"]["startLine"],
        2
    );
    assert_eq!(
        symbol(&svelte, "svelte_instance_symbol")["range"]["startLine"],
        5
    );
    assert!(!svelte.to_string().contains("svelte_template_only_token"));

    let astro = parsed_file(
        "src/matrix.astro",
        "---\nconst astro_frontmatter_symbol = 'astro_frontmatter_literal';\nfunction astro_frontmatter_fn() {}\n---\n<div>astro_template_only_token</div>\n<script>\nfunction astro_client_symbol() {}\n</script>\n",
    );
    assert_eq!(
        symbol(&astro, "astro_frontmatter_fn")["range"]["startLine"],
        3
    );
    assert_eq!(
        symbol(&astro, "astro_client_symbol")["range"]["startLine"],
        7
    );
    assert!(!astro.to_string().contains("astro_template_only_token"));
}

#[test]
fn test_mdx_remains_unsupported() {
    let parsed = parsed_file(
        "docs/matrix.mdx",
        "export const mdx_matrix_symbol = 'must_not_index';\n# Markdown heading\n",
    );
    assert_eq!(parsed["symbols"], serde_json::json!([]));
    assert!(parsed["navigation"].is_null());
}
