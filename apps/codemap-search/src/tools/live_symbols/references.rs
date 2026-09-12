//! Conservative, same-file constant context without enabling the reference index.

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
            "export_statement" | "lexical_declaration" | "const_declaration"
        ) {
            return Some(node);
        }
    }
}

fn namespace(mut node: Node<'_>) -> Option<usize> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "mod_item" | "module" | "internal_module" | "namespace_definition"
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
                | "type_annotation"
        ) || parent
            .child_by_field_name("type")
            .is_some_and(|typ| typ == node)
        {
            return IdentifierRole::Ignore;
        }
        if matches!(
            kind,
            "member_expression" | "field_expression" | "selector_expression" | "attribute"
        ) && ["property", "field", "attribute"]
            .iter()
            .any(|field| parent.child_by_field_name(field) == Some(node))
        {
            return IdentifierRole::Ignore;
        }
        if kind == "pair" && parent.child_by_field_name("key") == Some(node) {
            return IdentifierRole::Ignore;
        }
        if matches!(
            kind,
            "parameters"
                | "formal_parameters"
                | "closure_parameters"
                | "lambda_parameters"
                | "use_declaration"
                | "import_statement"
                | "import_from_statement"
        ) || parent.child_by_field_name("pattern") == Some(node)
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

fn is_const_declaration(kind: &str, node: Node<'_>) -> bool {
    kind == "const"
        || (kind == "variable"
            && node.parent().is_some_and(|parent| {
                parent.kind() == "lexical_declaration"
                    && parent
                        .child_by_field_name("kind")
                        .is_some_and(|kind| kind.kind() == "const")
            }))
}

fn value_preview(node: Node<'_>, source: &str) -> Option<String> {
    let value = node.child_by_field_name("value")?;
    // Preserve spaces inside literals; actual line breaks are escaped for a single row.
    let value = value
        .utf8_text(source.as_bytes())
        .ok()?
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

pub(super) fn collect(
    file: &ExtractedFile,
    selected: &BTreeSet<usize>,
    tree: &Tree,
    source: &str,
) -> BTreeMap<usize, Vec<String>> {
    let mut definitions: HashMap<&str, Vec<_>> = HashMap::new();
    for symbol in &file.symbols {
        if let Some(node) = symbol_node(tree, symbol) {
            if is_const_declaration(&symbol.kind, node)
                && node
                    .child_by_field_name("name")
                    .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                    == Some(symbol.name.as_str())
            {
                definitions
                    .entry(&symbol.name)
                    .or_default()
                    .push((symbol, node));
            }
        }
    }
    let mut output = BTreeMap::new();
    if definitions.is_empty() {
        return output;
    }
    for &index in selected {
        let symbol = &file.symbols[index];
        if !callable(symbol) {
            continue;
        }
        let Some(root) = symbol_node(tree, symbol) else {
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
                            | "class_declaration"
                            | "mod_item"
                    ))
            {
                continue;
            }
            if matches!(
                kind,
                "identifier"
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
                                let (definition, declaration) = candidates[0];
                                if declaration_scope(declaration)
                                    .is_some_and(|scope| is_enclosed_by(node, scope))
                                    && namespace(declaration) == namespace(node)
                                    && !is_enclosed_by(node, declaration)
                                {
                                    let value = value_preview(declaration, source)
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
    fn test_typescript_const_values_preserve_literal_whitespace() {
        let source = "export const TITLE = '두  칸';\nlet MUTABLE = 3;\nfunction show() { consume(TITLE, obj.TITLE, MUTABLE); }\nfunction shadow(TITLE: string) { consume(TITLE); }\n";
        assert_eq!(
            context(source, "src/page.ts", "show"),
            "    - TITLE — src/page.ts:1 = '두  칸'\n"
        );
        assert!(context(source, "src/page.ts", "shadow").is_empty());
    }
}
