//! C and C++ language specs. C++ is a superset of C; both share the declarator-chain name
//! walk (`c_declarator_name`) and the `static` storage-class check (`c_has_static_storage`).

pub(crate) mod c;
pub(crate) mod cpp;

use tree_sitter::Node;

use super::{is_inside_function, NameDecision};

/// Lexical-scope kinds used by the C/C++ accept-and-name cluster's "inside a function"
/// skips. The original generic walk used this exact list for BOTH C and C++ (C source has
/// no `lambda_expression` nodes, so listing it is a harmless no-op for C); keep it shared
/// so the skips stay byte-identical across both grammars.
const CFN_SCOPE_KINDS: &[&str] = &["function_definition", "lambda_expression"];

/// Shared C/C++ accept-and-name cluster. Returns `None` when `capture_name` is not
/// `symbol.cfn` and the match is not a function-local type reference (the generic name path
/// then applies); otherwise either skips or yields the extracted name.
pub(crate) fn name_for_cfn(
    capture_name: &str,
    node: Node,
    kind: &str,
    source: &[u8],
) -> Option<NameDecision> {
    // C/C++: struct_specifier / union_specifier / enum_specifier appear not only at
    // declaration scope but also as type references inside function parameter lists and
    // bodies (`void f(struct Foo *x)`). These are type references, not type declarations —
    // skip them to avoid polluting the symbol index with duplicate/noise entries.
    if matches!(kind, "struct" | "enum" | "union" | "variant")
        && is_inside_function(node, CFN_SCOPE_KINDS)
    {
        return Some(NameDecision::Skip);
    }

    if capture_name != "symbol.cfn" {
        return None;
    }

    // C/C++: a function-local prototype-shaped `declaration` is a vexing-parse local
    // variable, not a function — e.g. `std::lock_guard lock(spawn_mutex);` parses as a
    // `declaration` whose declarator is a `function_declarator`. These have no navigation
    // value and pollute the index (the captured "name" is the local variable). Real
    // `function_definition` nodes are a different kind and unaffected.
    if node.kind() == "declaration" && is_inside_function(node, CFN_SCOPE_KINDS) {
        return Some(NameDecision::Skip);
    }

    // C/C++: extract the function name from the declarator chain. The `symbol.name` capture
    // does not fire for `symbol.cfn` nodes because the name is nested (not a direct field
    // named `name`). Walk the declarator field to find the innermost identifier.
    let decl_name = node
        .child_by_field_name("declarator")
        .and_then(|d| c_declarator_name(d, source));
    match decl_name {
        Some(n) => Some(NameDecision::Name(n)),
        None => Some(NameDecision::Skip), // no usable name: skip this match
    }
}

/// C/C++: walk a declarator chain to extract the leaf function name. The declarator field
/// of a `function_definition` or `declaration` (with function_declarator) can be:
///   - `function_declarator` → `identifier`  (plain function)
///   - `pointer_declarator` → `function_declarator` → `identifier`  (pointer-returning fn)
///   - `reference_declarator` → `function_declarator` → `identifier`  (reference-returning fn:
///     `int& getref()`, `T& operator=(...)`, `auto&& fwd_ret()`). Unlike `pointer_declarator`,
///     `reference_declarator` has NO `declarator` field; its inner declarator is a positional child.
///   - `function_declarator` → `qualified_identifier`  (C++ out-of-line member: `Foo::bar`)
///   - `function_declarator` → `operator_name`  (C++ operator overload: `operator<`)
///   - `function_declarator` → `destructor_name`  (C++ destructor: `~Ops`)
///   - `function_declarator` → `parenthesized_declarator` → ... (typedef fn-ptr: `(*cb)(...)`)
///
/// Returns the innermost name string, or `None`.
pub(crate) fn c_declarator_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" => {
            node.utf8_text(source).ok().map(|t| t.to_string())
        }
        "function_declarator" => {
            // The declarator field is the name node or another declarator layer.
            let inner = node.child_by_field_name("declarator")?;
            c_declarator_name(inner, source)
        }
        "pointer_declarator" | "abstract_function_declarator" => {
            // Peel one pointer layer, then recurse. Both carry a `declarator` field.
            let inner = node.child_by_field_name("declarator")?;
            c_declarator_name(inner, source)
        }
        "reference_declarator" | "parenthesized_declarator" => {
            // Neither node has a `declarator` field (tree-sitter-cpp 0.23.4 node-types.json
            // confirmed: `reference_declarator` fields {}, `parenthesized_declarator` fields {}).
            // The inner declarator is a positional named child. Walk named children to find it.
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.is_named() {
                    if let Some(name) = c_declarator_name(child, source) {
                        return Some(name);
                    }
                }
            }
            None
        }
        "operator_name" => {
            // C++ operator overload: `operator<`, `operator[]`, etc.
            // Use the full source text of the node (e.g. "operator<").
            node.utf8_text(source).ok().map(|t| t.trim().to_string())
        }
        "destructor_name" => {
            // C++ destructor: `~Ops`. Use the full source text (e.g. "~Ops").
            node.utf8_text(source).ok().map(|t| t.trim().to_string())
        }
        "qualified_identifier" => {
            // C++ out-of-line member: `Foo::bar` — the `name` field is the member part.
            // `scope` field gives the owning class; extract it for owner resolution.
            let name = node.child_by_field_name("name")?;
            c_declarator_name(name, source)
        }
        "template_function" | "template_method" => {
            // Template specialization name: `foo<T>` — extract the base `name` child.
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.kind() == "identifier" || child.kind() == "field_identifier" {
                    return child.utf8_text(source).ok().map(|t| t.to_string());
                }
            }
            None
        }
        _ => None,
    }
}

/// C/C++: a `function_definition` or `declaration` (function prototype) is file-local
/// (not exported) iff it has a `storage_class_specifier` direct child with text "static".
pub(crate) fn c_has_static_storage(node: Node, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "storage_class_specifier" {
            if let Ok(text) = child.utf8_text(source) {
                if text.trim() == "static" {
                    return true;
                }
            }
        }
    }
    false
}
