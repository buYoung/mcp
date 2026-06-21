mod tokenize;
mod types;

pub use tokenize::{split_identifier, QueryTokens};
pub use types::*;

use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, QueryCursor};

use crate::lang::{
    clean_docstring, contains_case_insensitive, find_name, spec_for_ext, strip_quotes, NameDecision,
};

pub trait CodeExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String>;
}

pub struct TreeSitterExtractor;

impl Default for TreeSitterExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl CodeExtractor for TreeSitterExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String> {
        let path = Path::new(file_path);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        // Resolve the per-language spec from the registry. An unsupported extension yields an
        // empty `ExtractedFile`, preserving the prior unknown-extension behavior exactly.
        let Some(spec) = spec_for_ext(ext) else {
            return Ok(ExtractedFile {
                file_path: file_path.to_string(),
                total_lines: file_content.lines().count(),
                symbols: Vec::new(),
                literals: Vec::new(),
                docstrings: Vec::new(),
            });
        };

        let mut parser = Parser::new();
        let lang = spec.grammar(ext);
        let query = spec.query(ext);

        parser.set_language(&lang).map_err(|e| e.to_string())?;
        let tree = parser
            .parse(file_content, None)
            .ok_or("Failed to parse file content")?;

        let mut symbols = Vec::new();
        let mut literals = Vec::new();
        let source = file_content.as_bytes();

        // Per-language file-wide pre-pass collecting exported symbol names (TS `export {...}`
        // specifiers; ASM `.globl`/`.global` directives). No-op for languages without one.
        let mut exported_names = std::collections::HashSet::new();
        spec.collect_exported_names(tree.root_node(), source, &mut exported_names);

        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), source);

        while let Some(mat) = matches.next() {
            let mut main_node: Option<(Node, &str)> = None;
            let mut symbol_name: Option<String> = None;
            let mut is_valid_test_call = true;
            // ASM query: tracks the `.macro` directive kind so we know whether a `meta`
            // node should be emitted (only `.macro` definitions are captured as symbols).
            let mut asm_meta_kind_text: Option<String> = None;

            for capture in mat.captures {
                let name_idx = capture.index as usize;
                let name = query.capture_names()[name_idx];

                // `symbol.*` and `literal.*` both record `main_node`; the distinguishing
                // capture name is carried in the tuple and branched on below. This is
                // frozen Child 03 extraction routing — suppress, don't merge.
                #[allow(clippy::if_same_then_else)]
                if name.starts_with("symbol.") && name != "symbol.name" {
                    main_node = Some((capture.node, name));
                } else if name.starts_with("literal.") {
                    main_node = Some((capture.node, name));
                } else if name == "symbol.name" {
                    if let Ok(text) = capture.node.utf8_text(source) {
                        symbol_name = Some(text.to_string());
                    }
                } else if name == "fn_name" {
                    if let Ok(text) = capture.node.utf8_text(source) {
                        let t = text.trim();
                        if t != "describe" && t != "it" && t != "test" {
                            is_valid_test_call = false;
                        }
                    }
                } else if name == "meta_kind" {
                    // ASM query: capture the kind text so we can filter to `.macro` only.
                    if let Ok(text) = capture.node.utf8_text(source) {
                        asm_meta_kind_text = Some(text.trim().to_ascii_lowercase());
                    }
                }
            }

            if let Some((node, capture_name)) = main_node {
                if capture_name.starts_with("symbol.") {
                    if is_valid_test_call {
                        let kind = match capture_name {
                            "symbol.struct" => "struct",
                            "symbol.enum" => "enum",
                            "symbol.variant" => "variant",
                            "symbol.trait" => "trait",
                            "symbol.mod" => "mod",
                            "symbol.fn" | "symbol.method" => "fn",
                            "symbol.type" => "type",
                            "symbol.const" => "const",
                            "symbol.static" => "static",
                            "symbol.field" => "field",
                            "symbol.class" => "class",
                            "symbol.variable" => "variable",
                            "symbol.interface" => "interface",
                            "symbol.test" => "test",
                            "symbol.record" => "record",
                            "symbol.object" => "object",
                            "symbol.property" => "property",
                            // Go `type_spec` and Kotlin `class_declaration` carry a single
                            // capture; the concrete kind comes from the node itself.
                            // C/C++: function_definition and function prototype declarations are
                            // both captured as `symbol.cfn`; always emitted as kind "fn".
                            "symbol.cfn" => "fn",
                            // ASM: labels and `.macro` definitions captured as `symbol.asmfn`.
                            // `.macro` nodes are filtered below using asm_meta_kind_text.
                            "symbol.asmfn" => "fn",
                            _ => "unknown",
                        };
                        // Per-language refine of a node-dependent kind (Go `type_spec` struct/
                        // interface/alias; Kotlin `class`/`interface`). No-op for other captures.
                        let kind = spec.refine_kind(capture_name, node, kind);

                        // Per-language accept-and-name cluster (ASM `.macro` filter and label/
                        // macro name extraction; C/C++ type-reference and vexing-parse skips and
                        // declarator-chain name walk). `Skip` reproduces the original `continue`;
                        // `None` falls through to the generic name resolution below.
                        let mut name = match spec.name_for_capture(
                            capture_name,
                            node,
                            kind,
                            ext,
                            source,
                            &asm_meta_kind_text,
                        ) {
                            Some(NameDecision::Skip) => continue,
                            Some(NameDecision::Name(n)) => n,
                            None => symbol_name
                                .unwrap_or_else(|| find_name(node, source).unwrap_or_default()),
                        };

                        if kind == "test" {
                            name = strip_quotes(&name);
                        }

                        if !name.is_empty() {
                            let start = node.start_position();
                            let end = node.end_position();
                            let range = CodeRange {
                                start_line: start.row + 1,
                                start_col: start.column + 1,
                                end_line: end.row + 1,
                                end_col: end.column + 1,
                            };

                            // Associated comments proximity search. The per-language anchor
                            // adjusts the start node (Python `decorated_definition`, TS
                            // `export_statement`, Go outer declaration); default is the node.
                            let walk_start_node = spec.docstring_anchor(node);

                            let mut current_sibling = walk_start_node.prev_sibling();
                            let mut comments = Vec::new();
                            let mut last_row = walk_start_node.start_position().row;

                            while let Some(sibling) = current_sibling {
                                let sk = sibling.kind();
                                if sk == "comment" || sk == "line_comment" || sk == "block_comment"
                                {
                                    let end_row = sibling.end_position().row;
                                    if end_row >= last_row - 1 {
                                        if let Ok(text) = sibling.utf8_text(source) {
                                            comments.push(text.to_string());
                                        }
                                        last_row = sibling.start_position().row;
                                        current_sibling = sibling.prev_sibling();
                                    } else {
                                        break;
                                    }
                                } else if sk == "attribute_item" || sk == "decorator" {
                                    last_row = sibling.start_position().row;
                                    current_sibling = sibling.prev_sibling();
                                } else {
                                    break;
                                }
                            }

                            let mut docstring = clean_docstring(&comments);
                            // Per-language docstring fallback when comment promotion yields
                            // `None` (Python inline `"""` docstrings, Go plain `//` comments).
                            if docstring.is_none() {
                                docstring = spec.docstring_fallback(node, source, &comments);
                            }

                            let node_text = node.utf8_text(source).unwrap_or("");

                            // Preceding comments are no longer promoted into docstrings,
                            // so scan them directly for TODO/FIXME (covers `// TODO` above a
                            // symbol); node_text covers in-body and python `"""` docstrings.
                            let comments_text = comments.join("\n");
                            let has_todo = contains_case_insensitive(node_text, "todo")
                                || contains_case_insensitive(&comments_text, "todo");
                            let has_fixme = contains_case_insensitive(node_text, "fixme")
                                || contains_case_insensitive(&comments_text, "fixme");

                            let is_test =
                                spec.is_test(node, &name, kind, file_path, source, &comments_text);

                            let is_exported =
                                spec.is_exported(node, &name, kind, source, &exported_names);

                            let is_deprecated =
                                spec.is_deprecated(node, source, &docstring, &comments_text);

                            // Owner (enclosing type) is computed for callables — the search
                            // annotation qualifies `fn`/method names — and for enum variants
                            // (the owning enum names which type the variant belongs to).
                            // Other members are deferred (see the brief's Open Questions).
                            // Best-effort: any unexpected shape yields `None`.
                            // Note: `symbol.method` maps to kind "fn" (see match arm above),
                            // so "method" is never a possible kind value here.
                            let owner = if kind == "fn" || kind == "variant" {
                                spec.find_owner(node, ext, source)
                            } else {
                                None
                            };

                            symbols.push(ExtractedSymbol {
                                name,
                                kind: kind.to_string(),
                                range,
                                docstring,
                                flags: SymbolFlags {
                                    has_todo,
                                    has_fixme,
                                    is_test,
                                    is_exported,
                                    is_deprecated,
                                },
                                owner,
                            });
                        }
                    }
                } else if capture_name.starts_with("literal.string") {
                    // Only string literals carry search/detail value; numeric and boolean
                    // literals are dropped (low value, index/detail bloat — Child 03).
                    if let Ok(text) = node.utf8_text(source) {
                        let stripped = strip_quotes(text);
                        let line = node.start_position().row + 1;
                        literals.push(ExtractedLiteral {
                            text: stripped,
                            line,
                        });
                    }
                }
            }
        }

        let docstrings = symbols.iter().filter_map(|s| s.docstring.clone()).collect();

        Ok(ExtractedFile {
            file_path: file_path.to_string(),
            total_lines: file_content.lines().count(),
            symbols,
            literals,
            docstrings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Rust Parser Tests ---
    #[test]
    fn test_rust_parser_struct_and_fields() {
        let content = r#"
            /// Config struct description
            pub struct Config {
                pub port: u16,
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/config.rs").unwrap();

        // Assert struct symbol
        let struct_sym = file.symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(struct_sym.kind, "struct");
        assert!(struct_sym.flags.is_exported);
        assert_eq!(
            struct_sym.docstring.as_deref(),
            Some("Config struct description")
        );

        // Assert field variable symbol (verifies e2e target "port")
        let field_sym = file.symbols.iter().find(|s| s.name == "port").unwrap();
        assert_eq!(field_sym.kind, "field");
        assert!(field_sym.flags.is_exported);
    }

    #[test]
    fn test_rust_parser_flags_deprecated_and_todo() {
        let content = r#"
            // TODO: refactor this
            #[deprecated(since = "1.0.0")]
            fn deprecated_function() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "deprecated_function")
            .unwrap();

        assert!(sym.flags.has_todo);
        assert!(sym.flags.is_deprecated);
        assert!(!sym.flags.is_test);
    }

    #[test]
    fn test_rust_parser_test_detection() {
        let content = r#"
            #[test]
            fn my_test_case() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/tests.rs").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "my_test_case")
            .unwrap();

        assert!(sym.flags.is_test);
    }

    // --- Python Parser Tests ---
    #[test]
    fn test_python_parser_class_and_methods() {
        let content = r#"
class Database:
    """Manages db connection."""
    
    def __init__(self, url):
        self.url = url
        
    def query(self, sql):
        # FIXME: handle sql injection
        pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "db.py").unwrap();

        let class_sym = file.symbols.iter().find(|s| s.name == "Database").unwrap();
        assert_eq!(class_sym.kind, "class");
        assert_eq!(
            class_sym.docstring.as_deref(),
            Some("Manages db connection.")
        );
        assert!(class_sym.flags.is_exported); // No leading underscore

        let method_sym = file.symbols.iter().find(|s| s.name == "query").unwrap();
        assert_eq!(method_sym.kind, "fn");
        assert!(method_sym.flags.has_fixme);
    }

    #[test]
    fn test_python_private_and_deprecated() {
        let content = r#"
@deprecated
def _private_func():
    pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "utils.py").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "_private_func")
            .unwrap();

        assert!(!sym.flags.is_exported); // Starts with underscore
        assert!(sym.flags.is_deprecated);
    }

    // --- TypeScript / JavaScript Parser Tests ---
    #[test]
    fn test_ts_parser_interface_and_exports() {
        let content = r#"
            /** User info interface */
            export interface User {
                id: string;
                name: string;
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        // TS extension: parsed using typescript grammar
        let file = extractor.extract(content, "types.ts").unwrap();

        let sym = file.symbols.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(sym.kind, "interface");
        assert!(sym.flags.is_exported);
        assert_eq!(sym.docstring.as_deref(), Some("User info interface"));
    }

    #[test]
    fn test_tsx_jsx_grammar_selection() {
        let content = r#"
            export function Component() {
                return <div>Hello</div>;
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        // TSX extension: verifies TSX grammar parses JSX syntax successfully without erroring
        let file = extractor.extract(content, "component.tsx").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "Component").unwrap();
        assert_eq!(sym.kind, "fn");
        assert!(sym.flags.is_exported);
    }

    #[test]
    fn test_js_test_suite_hook_detection() {
        let content = r#"
            describe("auth service", () => {
                it("should validate credentials", () => {
                    // TODO: add boundary testing
                });
            });
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "auth.test.js").unwrap();

        let describe_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "auth service")
            .unwrap();
        assert_eq!(describe_sym.kind, "test");
        assert!(describe_sym.flags.is_test);

        let it_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "should validate credentials")
            .unwrap();
        assert_eq!(it_sym.kind, "test");
        assert!(it_sym.flags.is_test);
        assert!(it_sym.flags.has_todo);
    }

    #[test]
    fn test_raw_string_literal_quote_stripping() {
        let content = r##"
            pub const VAL: &str = r#"magic_value"#;
        "##;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        assert_eq!(file.literals.len(), 1);
        assert_eq!(file.literals[0].text, "magic_value");
        assert_eq!(file.literals[0].line, 2);
    }

    #[test]
    fn test_ts_named_exports_at_bottom() {
        let content = r#"
            function myFunc() {}
            export { myFunc };
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "index.ts").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "myFunc").unwrap();
        assert!(sym.flags.is_exported);
    }

    #[test]
    fn test_python_class_methods_export_status() {
        let content = r#"
class Calculator:
    def add(self, x, y):
        pass
    def _private_helper(self):
        pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "calc.py").unwrap();

        let add_sym = file.symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add_sym.flags.is_exported);

        let helper_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "_private_helper")
            .unwrap();
        assert!(!helper_sym.flags.is_exported);
    }

    #[test]
    fn test_block_comment_trailing_asterisks() {
        let content = r#"
            /** check function **/
            pub fn check() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "check").unwrap();
        assert_eq!(sym.docstring.as_deref(), Some("check function"));
    }

    #[test]
    fn test_ts_test_file_pattern_matching() {
        let content = r#"
            export function helper() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "tests/parser_test.ts").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert!(sym.flags.is_test);
    }

    // --- Owner (enclosing type) Tests (Phase A) ---

    /// Helper: extract `content` as `path` and return the `owner` of the first symbol named
    /// `name`. Panics if no such symbol exists, so a missing extraction fails loudly.
    fn owner_of(content: &str, path: &str, name: &str) -> Option<String> {
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, path).unwrap();
        file.symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("symbol `{name}` not extracted from {path}"))
            .owner
            .clone()
    }

    #[test]
    fn test_owner_rust_impl_method() {
        let content = "impl Server { pub fn start(&self) {} }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "start"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_rust_impl_trait_for_type_uses_type() {
        // `impl Trait for Type` → the implementing Type, not the trait.
        let content = "impl Display for Widget { fn fmt(&self) {} }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "fmt"),
            Some("Widget".to_string())
        );
    }

    #[test]
    fn test_owner_rust_generic_and_scoped_impl_normalized() {
        // Generic args stripped: `impl Cache<K, V>` → `Cache`.
        assert_eq!(
            owner_of(
                "impl<K, V> Cache<K, V> { fn get(&self) {} }",
                "src/lib.rs",
                "get"
            ),
            Some("Cache".to_string())
        );
        // Module path reduced to the rightmost segment: `impl a::b::Store` → `Store`.
        assert_eq!(
            owner_of(
                "impl crate::store::Store { fn put(&self) {} }",
                "src/lib.rs",
                "put"
            ),
            Some("Store".to_string())
        );
    }

    #[test]
    fn test_owner_rust_trait_default_method() {
        let content = "trait Greeter { fn greet(&self) { println!(\"hi\"); } }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "greet"),
            Some("Greeter".to_string())
        );
    }

    #[test]
    fn test_owner_rust_free_fn_is_none() {
        assert_eq!(owner_of("pub fn run() {}", "src/lib.rs", "run"), None);
    }

    #[test]
    fn test_owner_rust_fn_in_module_is_none() {
        // A function nested only inside a module (not a type) has no owner.
        assert_eq!(
            owner_of("mod util { pub fn helper() {} }", "src/lib.rs", "helper"),
            None
        );
    }

    #[test]
    fn test_owner_rust_local_fn_in_method_is_none() {
        let content = "impl Server { fn run(&self) { fn helper() {} } }";
        assert_eq!(owner_of(content, "src/lib.rs", "helper"), None);
    }

    #[test]
    fn test_owner_rust_fn_in_closure_in_method_is_none() {
        let content = "impl Server { fn run(&self) { let c = || { fn inner() {} }; } }";
        assert_eq!(owner_of(content, "src/lib.rs", "inner"), None);
    }

    #[test]
    fn test_owner_go_method_receiver_base_type() {
        // Pointer receiver `*Server` → `Server`.
        assert_eq!(
            owner_of(
                "package p\nfunc (s *Server) Start() {}\n",
                "main.go",
                "Start"
            ),
            Some("Server".to_string())
        );
        // Value receiver `Server` → `Server`.
        assert_eq!(
            owner_of("package p\nfunc (s Server) Stop() {}\n", "main.go", "Stop"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_go_generic_receiver_normalized() {
        // `*Box[T]` → `Box` (square-bracketed generic args stripped).
        assert_eq!(
            owner_of("package p\nfunc (b *Box[T]) Get() {}\n", "main.go", "Get"),
            Some("Box".to_string())
        );
    }

    #[test]
    fn test_owner_go_interface_method_elem() {
        let content = "package p\ntype Reader interface {\n Read() error\n}\n";
        assert_eq!(
            owner_of(content, "main.go", "Read"),
            Some("Reader".to_string())
        );
    }

    #[test]
    fn test_owner_go_free_fn_is_none() {
        assert_eq!(
            owner_of("package p\nfunc Run() {}\n", "main.go", "Run"),
            None
        );
    }

    #[test]
    fn test_owner_go_local_fn_in_method_is_none() {
        let content = "package p\nfunc (s *Server) Run() {\n inner := func() {}\n _ = inner\n}\n";
        // A `func_literal` is anonymous (no name symbol) — assert the method itself instead:
        // the receiver method resolves, and no spurious owner leaks to nested closures.
        assert_eq!(
            owner_of(content, "main.go", "Run"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_python_class_method() {
        let content = "class Foo:\n    def bar(self):\n        pass\n";
        assert_eq!(owner_of(content, "x.py", "bar"), Some("Foo".to_string()));
    }

    #[test]
    fn test_owner_python_free_fn_is_none() {
        assert_eq!(owner_of("def run():\n    pass\n", "x.py", "run"), None);
    }

    #[test]
    fn test_owner_python_local_fn_in_method_is_none() {
        let content = "class Foo:\n    def bar(self):\n        def helper():\n            pass\n";
        assert_eq!(owner_of(content, "x.py", "helper"), None);
    }

    #[test]
    fn test_owner_ts_class_method() {
        let content = "class Service { handle() {} }";
        assert_eq!(
            owner_of(content, "x.ts", "handle"),
            Some("Service".to_string())
        );
    }

    #[test]
    fn test_owner_ts_free_fn_is_none() {
        assert_eq!(owner_of("function run() {}", "x.ts", "run"), None);
    }

    #[test]
    fn test_owner_ts_local_fn_in_method_is_none() {
        let content = "class A { method() { function localFn() { return 1; } } }";
        assert_eq!(owner_of(content, "x.ts", "localFn"), None);
    }

    #[test]
    fn test_owner_ts_object_literal_method_is_none() {
        // A `method_definition` inside an object-literal value, not a named type.
        let content = "class A { config = { handler() {} }; }";
        assert_eq!(owner_of(content, "x.ts", "handler"), None);
    }

    #[test]
    fn test_owner_ts_class_expression_method_is_none() {
        // A class *expression* (a value) is anonymous — no owner.
        let content = "const X = class { doThing() {} };";
        assert_eq!(owner_of(content, "x.ts", "doThing"), None);
    }

    #[test]
    fn test_owner_js_class_method() {
        let content = "class Widget { render() {} }";
        assert_eq!(
            owner_of(content, "x.js", "render"),
            Some("Widget".to_string())
        );
    }

    #[test]
    fn test_owner_ts_abstract_class_method() {
        // `abstract_class_declaration` is a named type container (verified against the
        // grammar's node-types.json: `name` field is a `type_identifier`).
        let content = "abstract class Base { run() {} }";
        assert_eq!(owner_of(content, "x.ts", "run"), Some("Base".to_string()));
    }

    #[test]
    fn test_owner_java_class_method() {
        let content = "class A { void m() {} }";
        assert_eq!(owner_of(content, "A.java", "m"), Some("A".to_string()));
    }

    #[test]
    fn test_owner_java_interface_enum_record_methods() {
        assert_eq!(
            owner_of("interface I { void doI(); }", "I.java", "doI"),
            Some("I".to_string())
        );
        assert_eq!(
            owner_of("enum E { A; void doE() {} }", "E.java", "doE"),
            Some("E".to_string())
        );
        assert_eq!(
            owner_of("record R(int x) { void doR() {} }", "R.java", "doR"),
            Some("R".to_string())
        );
    }

    #[test]
    fn test_owner_java_anonymous_class_method_is_none() {
        let content =
            "class A { void m() { Runnable r = new Runnable() { public void run() {} }; } }";
        assert_eq!(owner_of(content, "A.java", "run"), None);
    }

    #[test]
    fn test_owner_kotlin_class_method() {
        let content = "class Service {\n  fun handle() {}\n}\n";
        assert_eq!(
            owner_of(content, "x.kt", "handle"),
            Some("Service".to_string())
        );
    }

    #[test]
    fn test_owner_kotlin_object_method() {
        let content = "object Singleton {\n  fun go() {}\n}\n";
        assert_eq!(
            owner_of(content, "x.kt", "go"),
            Some("Singleton".to_string())
        );
    }

    #[test]
    fn test_owner_kotlin_object_literal_method_is_none() {
        let content = "fun build() {\n  val x = object {\n    fun anon() {}\n  }\n}\n";
        assert_eq!(owner_of(content, "x.kt", "anon"), None);
    }

    #[test]
    fn test_owner_kotlin_free_fn_is_none() {
        assert_eq!(owner_of("fun run() {}\n", "x.kt", "run"), None);
    }

    #[test]
    fn test_owner_kotlin_companion_object_resolves_enclosing_class() {
        // Kotlin `companion object` members resolve to the enclosing class name. Note:
        // tree-sitter-kotlin-ng 1.1.0 is shape-sensitive here — a multi-line body nests the
        // member under the class so `companion_object` (passthrough) is traversed to
        // `class_declaration`; a single-line body can instead collapse the companion into an
        // `ERROR` node, in which case the walk yields `None` (best-effort, never wrong). This
        // test pins the common multi-line shape resolving to the enclosing class.
        let extractor = TreeSitterExtractor::new();
        let content = "class A {\n  companion object {\n    fun create() {}\n  }\n}\n";
        let file = extractor.extract(content, "x.kt").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "create")
            .expect("companion member should be extracted");
        assert_eq!(sym.owner, Some("A".to_string()));
    }

    /// Owner is best-effort and never panics: feed each grammar a method-in-type fixture and
    /// confirm the call returns without crashing (a wrong owner is worse than `None`).
    #[test]
    fn test_owner_no_panic_across_grammars() {
        let cases = [
            ("impl T { fn a(&self) {} }", "x.rs"),
            ("package p\nfunc (s *T) A() {}\n", "x.go"),
            ("class T:\n    def a(self): pass\n", "x.py"),
            ("class T { a() {} }", "x.ts"),
            ("class T { void a() {} }", "T.java"),
            ("class T {\n  fun a() {}\n}\n", "x.kt"),
            // C: struct with a nested function pointer field (no method ownership in C,
            // but exercises the owner walk without panicking).
            (
                "struct S { int x; };\nint add(int a, int b) { return a + b; }",
                "x.c",
            ),
            // C++: class with an in-class method declaration and an out-of-line definition.
            (
                "class Widget { public: void draw(); };\nvoid Widget::draw() {}",
                "x.cpp",
            ),
            // ASM: a globl label and a non-globl label.
            (".globl _main\n_main:\n  ret\n_local:\n  ret\n", "x.s"),
        ];
        for (content, path) in cases {
            let extractor = TreeSitterExtractor::new();
            // Must not panic; owner correctness is asserted in the per-language tests above.
            let _ = extractor.extract(content, path).unwrap();
        }
    }
}
