//! Conservative, same-file constant context without enabling the reference index.
mod declarations;

use super::structure::{callable, symbol_node};
use crate::parser::ExtractedFile;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use tree_sitter::{Node, Tree};

fn is_enclosed_by(inner: Node<'_>, outer: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

fn declaration_scope(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        node = node.parent()?;
        if !matches!(
            node.kind(),
            "export_statement"
                | "lexical_declaration"
                | "const_declaration"
                | "field_declaration"
                | "variable_declaration"
                | "declaration"
                | "static_final_declaration_list"
                | "top_level_variable_declaration"
                | "class_member"
        ) {
            return Some(node);
        }
    }
}

fn namespace(mut node: Node<'_>) -> Option<usize> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "mod_item"
                | "module"
                | "internal_module"
                | "namespace_definition"
                | "class"
                | "class_declaration"
                | "class_definition"
                | "class_specifier"
                | "struct_specifier"
                | "struct_declaration"
                | "object_declaration"
        ) {
            return Some(parent.id());
        }
        node = parent;
    }
    None
}

enum IdentifierRole {
    Reference,
    Binding,
    Ignore,
}

fn identifier_role(mut node: Node<'_>, root: Node<'_>) -> IdentifierRole {
    while node != root {
        let Some(parent) = node.parent() else { break };
        let kind = parent.kind();
        if matches!(
            kind,
            "scoped_identifier"
                | "scoped_type_identifier"
                | "qualified_identifier"
                | "qualified_name"
                | "scope_resolution"
                | "scoped_property_access_expression"
                | "class_constant_access_expression"
                | "variable_name"
                | "type_annotation"
                | "user_type"
        ) || parent
            .child_by_field_name("type")
            .is_some_and(|typ| typ == node)
        {
            return IdentifierRole::Ignore;
        }
        if matches!(
            kind,
            "member_expression"
                | "field_expression"
                | "selector_expression"
                | "attribute"
                | "member_access_expression"
                | "field_access"
                | "member_call_expression"
        ) && ["property", "field", "attribute", "name"]
            .iter()
            .any(|field| parent.child_by_field_name(field) == Some(node))
        {
            return IdentifierRole::Ignore;
        }
        if matches!(kind, "navigation_suffix")
            || (kind == "navigation_expression" && parent.named_child(0) != Some(node))
        {
            return IdentifierRole::Ignore;
        }
        if kind == "pair" && parent.child_by_field_name("key") == Some(node) {
            return IdentifierRole::Ignore;
        }
        // PHP has separate constant, variable, and function namespaces.
        if kind == "function_call_expression"
            && parent.child_by_field_name("function") == Some(node)
        {
            return IdentifierRole::Ignore;
        }
        if matches!(
            kind,
            "parameters"
                | "formal_parameters"
                | "parameter_list"
                | "formal_parameter_list"
                | "function_value_parameters"
                | "method_parameters"
                | "closure_parameters"
                | "lambda_parameters"
                | "use_declaration"
                | "import_statement"
                | "import_from_statement"
        ) || parent.child_by_field_name("pattern") == Some(node)
            || parent.child_by_field_name("bound_identifier") == Some(node)
            || ((kind.ends_with("declarator") || kind.ends_with("declaration"))
                && parent.child_by_field_name("declarator") == Some(node))
            || (kind == "assignment" && parent.child_by_field_name("left") == Some(node))
            || (kind == "variable_declaration" && parent.named_child(0) == Some(node))
            || ((kind.ends_with("declaration")
                || kind.ends_with("declarator")
                || kind.ends_with("parameter")
                || kind.ends_with("_item")
                || kind.ends_with("definition"))
                && parent.child_by_field_name("name") == Some(node))
        {
            return IdentifierRole::Binding;
        }
        node = parent;
    }
    IdentifierRole::Reference
}

fn value_preview(
    value: Node<'_>,
    source: &str,
    scan: &crate::redact::SourceScan,
) -> Option<String> {
    // Preserve spaces inside literals; actual line breaks are escaped for a single row.
    let value = scan
        .render_range(source, value.byte_range())
        .trim()
        .replace('\r', "\\r")
        .replace('\n', "\\n");
    if value.chars().count() > 240 {
        Some(format!(
            "{}… [value shortened]",
            value.chars().take(240).collect::<String>()
        ))
    } else {
        Some(value)
    }
}

#[cfg(test)]
pub(super) fn collect(
    file: &ExtractedFile,
    selected: &BTreeSet<usize>,
    tree: &Tree,
    source: &str,
) -> BTreeMap<usize, Vec<String>> {
    collect_with_resolver(file, selected, tree, source, None)
}

pub(super) fn collect_with_resolver(
    file: &ExtractedFile,
    selected: &BTreeSet<usize>,
    tree: &Tree,
    source: &str,
    resolver: Option<&crate::callers::resolution::SourceResolver<'_>>,
) -> BTreeMap<usize, Vec<String>> {
    let redaction = crate::redact::SourceScan::with_tree(source, tree);
    let mut definitions: HashMap<&str, Vec<_>> = HashMap::new();
    let language = crate::lang::spec_for_path(std::path::Path::new(&file.file_path))
        .map(|spec| spec.language_name())
        .unwrap_or("");
    for symbol in &file.symbols {
        if let Some(node) = symbol_node(tree, symbol, source) {
            if let Some((declaration, value)) =
                declarations::binding(node, symbol, language, source)
            {
                definitions
                    .entry(&symbol.name)
                    .or_default()
                    .push((symbol, declaration, value));
            }
        }
    }
    let mut output = BTreeMap::new();
    if definitions.is_empty() && resolver.is_none() {
        return output;
    }
    for &index in selected {
        let symbol = &file.symbols[index];
        if !callable(symbol) {
            continue;
        }
        let Some(root) = symbol_node(tree, symbol, source) else {
            continue;
        };
        let mut references = BTreeMap::new();
        let mut shadowed = HashSet::new();
        let mut nodes = vec![root];
        while let Some(node) = nodes.pop() {
            let kind = node.kind();
            // Tokens inside macros are not guaranteed to be expressions. Nested named
            // declarations get their own context rather than becoming outer dependencies.
            if kind.contains("comment")
                || matches!(
                    kind,
                    "string" | "string_literal" | "raw_string_literal" | "token_tree"
                )
                || (node != root
                    && matches!(
                        kind,
                        "function_item"
                            | "function_declaration"
                            | "function_definition"
                            | "method_definition"
                            | "method_declaration"
                            | "method"
                            | "singleton_method"
                            | "class_declaration"
                            | "mod_item"
                    ))
            {
                continue;
            }
            if matches!(
                kind,
                "identifier"
                    | "simple_identifier"
                    | "name"
                    | "shorthand_property_identifier"
                    | "shorthand_field_identifier"
                    | "constant"
            ) {
                if let Ok(name) = node.utf8_text(source.as_bytes()) {
                    if let Some(candidates) = definitions.get(name) {
                        match identifier_role(node, root) {
                            IdentifierRole::Binding => {
                                shadowed.insert(name);
                            }
                            IdentifierRole::Reference
                                if candidates.len() == 1 && !references.contains_key(name) =>
                            {
                                let (definition, declaration, value) = candidates[0];
                                if declaration_scope(declaration)
                                    .is_some_and(|scope| is_enclosed_by(node, scope))
                                    && namespace(declaration) == namespace(node)
                                    && !is_enclosed_by(node, declaration)
                                {
                                    let value = value
                                        .and_then(|value| value_preview(value, source, &redaction))
                                        .map(|value| format!(" = {value}"))
                                        .unwrap_or_default();
                                    references.insert(
                                        name,
                                        format!(
                                            "    - {name} — {}:{}{value}\n",
                                            file.file_path, definition.range.start_line
                                        ),
                                    );
                                }
                            }
                            _ => {}
                        }
                    } else if matches!(identifier_role(node, root), IdentifierRole::Reference) {
                        if let Some(target) = resolver.and_then(|resolver| {
                            resolver.resolve_name(
                                file,
                                &crate::parser::CodeRange {
                                    start_line: node.start_position().row + 1,
                                    start_col: node.start_position().column + 1,
                                    end_line: node.end_position().row + 1,
                                    end_col: node.end_position().column + 1,
                                },
                                name,
                                "const",
                            )
                        }) {
                            references.entry(name).or_insert_with(|| {
                                format!(
                                    "    - {name} — {}:{}\n",
                                    target.file.file_path, target.symbol.range.start_line
                                )
                            });
                        }
                    } else if matches!(identifier_role(node, root), IdentifierRole::Binding) {
                        shadowed.insert(name);
                    }
                }
            }
            let mut cursor = node.walk();
            nodes.extend(node.named_children(&mut cursor));
        }
        // Any local binding with this spelling makes the bare-name link uncertain.
        // Prefer omitting it to attaching a value from another lexical binding.
        let rows: Vec<_> = references
            .into_iter()
            .filter(|(name, _)| !shadowed.contains(name))
            .map(|(_, row)| row)
            .collect();
        if !rows.is_empty() {
            output.insert(index, rows);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{CodeExtractor, TreeSitterExtractor};
    use std::path::Path;
    use tree_sitter::Parser;

    fn context(source: &str, path: &str, function: &str) -> String {
        let file = TreeSitterExtractor::new().extract(source, path).unwrap();
        let index = file
            .symbols
            .iter()
            .position(|symbol| symbol.name == function)
            .unwrap();
        let spec = crate::lang::spec_for_path(Path::new(path)).unwrap();
        let mut parser = Parser::new();
        parser
            .set_language(&spec.grammar(Path::new(path).extension().unwrap().to_str().unwrap()))
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        collect(&file, &BTreeSet::from([index]), &tree, source)
            .remove(&index)
            .unwrap_or_default()
            .concat()
    }

    #[test]
    fn test_constant_references_do_not_cross_bindings_or_namespaces() {
        let source = r#"const ROOT: &str = ".codemap";
const PARAM: &str = "global parameter";
const LOCAL: &str = "global local";
const NESTED: &str = "only used in a nested function";
const TEXT: &str = "only in comments and strings";
mod hidden { pub const HIDDEN: &str = "not in scope"; }
fn load(PARAM: &str) {
    let LOCAL = "local value";
    consume(ROOT, PARAM, LOCAL, other::ROOT, HIDDEN);
    let text = "TEXT"; // TEXT
    fn nested() { consume(NESTED); }
}
mod child { fn load_child() { consume(ROOT); } }
"#;
        let output = context(source, "src/config.rs", "load");
        assert_eq!(output, "    - ROOT — src/config.rs:1 = \".codemap\"\n");
        assert!(context(source, "src/config.rs", "load_child").is_empty());
        let ambiguous = "const SAME: i32 = 1; mod inner { const SAME: i32 = 2; } fn use_same() { consume(SAME); }";
        assert!(context(ambiguous, "src/config.rs", "use_same").is_empty());
    }

    #[test]
    fn test_language_constant_references_have_source_locations_and_values() {
        for (path, source) in [
            (
                "probe.java",
                r##"class CmValidationProbe {
    static final int CM_VALIDATION_LIMIT = 7;
    static int cm_validation_target() { return CM_VALIDATION_LIMIT; }
    static int cm_validation_caller() { return cm_validation_target(); }
    static String cm_validation_string() { return "CM_VALIDATION_LIMIT"; }
    static int cm_validation_comment() { /* CM_VALIDATION_LIMIT; */ return 0; }
    static int cm_validation_unknown(External receiver) { return receiver.cm_validation_target(); }
}
"##,
            ),
            (
                "probe.cs",
                r##"class CmValidationProbe {
    const int CM_VALIDATION_LIMIT = 7;
    static int cm_validation_target() { return CM_VALIDATION_LIMIT; }
    static int cm_validation_caller() { return cm_validation_target(); }
    static string cm_validation_string() { return "CM_VALIDATION_LIMIT"; }
    static int cm_validation_comment() { /* CM_VALIDATION_LIMIT; */ return 0; }
    static int cm_validation_unknown(dynamic receiver) { return receiver.cm_validation_target(); }
}
"##,
            ),
            (
                "probe.php",
                r##"<?php
const CM_VALIDATION_LIMIT = 7;
function cm_validation_target() { return CM_VALIDATION_LIMIT; }
function cm_validation_caller() { return cm_validation_target(); }
function cm_validation_string() { return "CM_VALIDATION_LIMIT"; }
function cm_validation_comment() { /* CM_VALIDATION_LIMIT; */ return 0; }
function cm_validation_unknown($receiver) { return $receiver->cm_validation_target(); }
"##,
            ),
            (
                "probe.rb",
                r##"CM_VALIDATION_LIMIT = 7
def cm_validation_target()
  CM_VALIDATION_LIMIT
end
def cm_validation_caller()
  cm_validation_target()
end
def cm_validation_string()
  "CM_VALIDATION_LIMIT"
end
def cm_validation_comment()
  # CM_VALIDATION_LIMIT
  0
end
def cm_validation_unknown(receiver)
  receiver.cm_validation_target()
end
"##,
            ),
            (
                "probe.kt",
                r##"const val CM_VALIDATION_LIMIT = 7
fun cm_validation_target(): Int { return CM_VALIDATION_LIMIT; }
fun cm_validation_caller(): Int { return cm_validation_target(); }
fun cm_validation_string(): String { return "CM_VALIDATION_LIMIT"; }
fun cm_validation_comment(): Int { /* CM_VALIDATION_LIMIT; */ return 0; }
fun cm_validation_unknown(receiver: External): Int { return receiver.cm_validation_target(); }
"##,
            ),
            (
                "probe.swift",
                r##"let CM_VALIDATION_LIMIT = 7
func cm_validation_target() -> Int { return CM_VALIDATION_LIMIT; }
func cm_validation_caller() -> Int { return cm_validation_target(); }
func cm_validation_string() -> String { return "CM_VALIDATION_LIMIT"; }
func cm_validation_comment() -> Int { /* CM_VALIDATION_LIMIT; */ return 0; }
func cm_validation_unknown(_ receiver: External) -> Int { return receiver.cm_validation_target(); }
"##,
            ),
            (
                "probe.dart",
                r##"const CM_VALIDATION_LIMIT = 7;
int cm_validation_target() { return CM_VALIDATION_LIMIT; }
int cm_validation_caller() { return cm_validation_target(); }
String cm_validation_string() { return "CM_VALIDATION_LIMIT"; }
int cm_validation_comment() { /* CM_VALIDATION_LIMIT; */ return 0; }
int cm_validation_unknown(dynamic receiver) { return receiver.cm_validation_target(); }
"##,
            ),
            (
                "probe.scala",
                r##"val CM_VALIDATION_LIMIT = 7
def cm_validation_target(): Int = { return CM_VALIDATION_LIMIT; }
def cm_validation_caller(): Int = { return cm_validation_target(); }
def cm_validation_string(): String = { return "CM_VALIDATION_LIMIT"; }
def cm_validation_comment(): Int = { /* CM_VALIDATION_LIMIT; */ return 0; }
def cm_validation_unknown(receiver: External): Int = { return receiver.cm_validation_target(); }
"##,
            ),
            (
                "probe.groovy",
                r##"class CmValidationProbe {
    static final int CM_VALIDATION_LIMIT = 7;
    static def cm_validation_target() { return CM_VALIDATION_LIMIT; }
    static def cm_validation_caller() { return cm_validation_target(); }
    static String cm_validation_string() { return "CM_VALIDATION_LIMIT"; }
    static def cm_validation_comment() { /* CM_VALIDATION_LIMIT; */ return 0; }
    static def cm_validation_unknown(def receiver) { return receiver.cm_validation_target(); }
}
"##,
            ),
            (
                "probe.c",
                r##"const int CM_VALIDATION_LIMIT = 7;
int cm_validation_target(void) { return CM_VALIDATION_LIMIT; }
int cm_validation_caller(void) { return cm_validation_target(); }
const char *cm_validation_string(void) { return "CM_VALIDATION_LIMIT"; }
int cm_validation_comment(void) { /* CM_VALIDATION_LIMIT; */ return 0; }
int cm_validation_unknown(int (*cm_validation_target)(void)) { return cm_validation_target(); }
"##,
            ),
            (
                "probe.cpp",
                r##"const int CM_VALIDATION_LIMIT = 7;
int cm_validation_target(void) { return CM_VALIDATION_LIMIT; }
int cm_validation_caller(void) { return cm_validation_target(); }
const char *cm_validation_string(void) { return "CM_VALIDATION_LIMIT"; }
int cm_validation_comment(void) { /* CM_VALIDATION_LIMIT; */ return 0; }
int cm_validation_unknown(int (*cm_validation_target)(void)) { return cm_validation_target(); }
"##,
            ),
        ] {
            let line = source
                .lines()
                .position(|line| line.contains("CM_VALIDATION_LIMIT ="))
                .unwrap()
                + 1;
            assert_eq!(
                context(source, path, "cm_validation_target"),
                format!("    - CM_VALIDATION_LIMIT — {path}:{line} = 7\n"),
                "{path}"
            );
            assert!(
                context(source, path, "cm_validation_string").is_empty(),
                "{path}"
            );
            assert!(
                context(source, path, "cm_validation_comment").is_empty(),
                "{path}"
            );
        }
    }

    #[test]
    fn test_constant_values_preserve_literal_whitespace_and_skip_comments() {
        let source = "export const TITLE = '두  칸';\nlet MUTABLE = 3;\nfunction show() { consume(TITLE, obj.TITLE, MUTABLE); }\nfunction shadow(TITLE: string) { consume(TITLE); }\n";
        assert_eq!(
            context(source, "src/page.ts", "show"),
            "    - TITLE — src/page.ts:1 = '두  칸'\n"
        );
        assert!(context(source, "src/page.ts", "shadow").is_empty());
        let scala = "val LIMIT = 7\n\ndef first(): Unit =\n  println(LIMIT)\n\n// next function\ndef second(): Unit = ()\n";
        assert_eq!(
            context(scala, "layout.scala", "first"),
            "    - LIMIT — layout.scala:1 = 7\n"
        );
        for (path, source) in [
            (
                "comment.cs",
                "class A { const int LIMIT /* kept */ = 7; int f() { return LIMIT; } }",
            ),
            ("comment.kt", "val LIMIT /* kept */ = 7\nfun f() = LIMIT"),
        ] {
            assert_eq!(
                context(source, path, "f"),
                format!("    - LIMIT — {path}:1 = 7\n")
            );
        }
    }

    #[test]
    fn language_constants_do_not_link_parameters_locals_or_foreign_members() {
        for (path, source) in [
            ("scope.java", "class A { static final int LIMIT = 7; int f(int LIMIT) { return LIMIT; } }"),
            ("scope.cs", "class A { const int LIMIT = 7; int f(dynamic other) { return other.LIMIT; } }"),
            ("scope.php", "<?php const LIMIT = 7; function f($LIMIT) { return $LIMIT; }"),
            ("class.php", "<?php define('LIMIT', 3); class A { const LIMIT = 7; function f() { return LIMIT; } }"),
            ("namespace.php", "<?php namespace A; const LIMIT = 7; namespace B; function f() { return LIMIT; }"),
            ("scope.rb", "module M\n LIMIT = 7\nend\ndef f\n M::LIMIT\nend\n"),
            ("scope.kt", "const val LIMIT = 7\nfun f(LIMIT: Int): Int { return LIMIT; }"),
            ("scope.swift", "let LIMIT = 7\nfunc f(_ LIMIT: Int) -> Int { return LIMIT }"),
            ("scope.dart", "const LIMIT = 7;\nint f(int LIMIT) { return LIMIT; }"),
            ("scope.scala", "val LIMIT = 7\ndef f(LIMIT: Int): Int = LIMIT"),
            ("scope.groovy", "class A { static final int LIMIT = 7; int f(int LIMIT) { return LIMIT; } }"),
            ("scope.c", "const int LIMIT = 7;\nint f(void) { int LIMIT = 3; return LIMIT; }"),
            ("scope.cpp", "const int LIMIT = 7;\nint f(int LIMIT) { return LIMIT; }"),
            ("class.java", "class Outer { static final int LIMIT = 7; class Inner { int LIMIT = 3; int f() { return LIMIT; } } }"),
        ] {
            assert!(context(source, path, "f").is_empty(), "{path}: {}", context(source, path, "f"));
        }
    }
}
