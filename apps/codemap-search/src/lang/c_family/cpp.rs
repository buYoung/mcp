//! C++ language spec: query, compiled query, and the C++-specific extraction hooks. Shared
//! C/C++ helpers (declarator name walk, static-storage check) live in the parent [`super`]
//! module.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::super::{generic_find_owner, is_inside_function, LanguageSpec, NameDecision};
use super::{c_has_static_storage, name_for_cfn};

// C++: superset of C, node kinds verified against tree-sitter-cpp 0.23.4 node-types.json.
// Additional shapes confirmed:
//   - class_specifier carries `name` field (type_identifier for simple classes;
//     qualified_identifier / template_type for specializations — skip those).
//   - field_declaration with function_declarator inside a class body = method declaration.
//   - function_definition whose declarator → function_declarator → qualified_identifier
//     = out-of-line member definition (void Foo::bar() {}).
//   - template_declaration wraps function_definition or class_specifier — the inner node
//     is captured directly via inner pattern alternatives.
//   - alias_declaration carries a `name` field (type_identifier).
//   - namespace_definition is passthrough (members are free), no symbol emitted.
//   - access_specifier is a sibling inside field_declaration_list (used in export detection).
//   - lambda_expression is a stop kind for owner and export (inside-function detection).
const CPP_QUERY_STR: &str = concat!(
    include_str!("../../../queries/cpp/symbols.scm"),
    "\n",
    include_str!("../../../queries/cpp/navigation.scm")
);

const CPP_TAGS_QUERY_STR: &str = include_str!("../../../queries/cpp/tags.scm");
const CPP_STATIC_COLLECTION_QUERY_STR: &str =
    include_str!("../../../queries/cpp/static_collection_edges.scm");

fn get_cpp_query() -> &'static Query {
    static CPP_QUERY: OnceLock<Query> = OnceLock::new();
    CPP_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_cpp::LANGUAGE.into(), CPP_QUERY_STR)
            .expect("Failed to compile C++ query")
    })
}

fn get_cpp_tags_query() -> &'static Query {
    static CPP_TAGS_QUERY: OnceLock<Query> = OnceLock::new();
    CPP_TAGS_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_cpp::LANGUAGE.into(), CPP_TAGS_QUERY_STR)
            .expect("Failed to compile C++ tags query")
    })
}

fn get_cpp_static_collection_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_cpp::LANGUAGE.into(),
            CPP_STATIC_COLLECTION_QUERY_STR,
        )
        .expect("Failed to compile C++ static collection query")
    })
}

/// C++ out-of-line member: extract the class/struct scope from the `qualified_identifier`
/// inside a `function_declarator` child of a `function_definition`. For `void Foo::bar()`,
/// the declarator chain is `function_declarator → qualified_identifier { scope, name }`; we
/// need the innermost class scope, not the outermost. For `void ns::Cls::method()`, the
/// structure nests: `qualified_identifier { scope: ns, name: qualified_identifier { scope:
/// Cls, name: method } }`. We must follow the `name` chain until the innermost
/// `qualified_identifier` whose `name` field is a simple identifier (the member name);
/// its `scope` is the class that owns the member.
/// Returns `None` if the declarator is not a qualified identifier (i.e. a free function).
fn cpp_outofline_owner(function_def_node: Node, source: &[u8]) -> Option<String> {
    let declarator = function_def_node.child_by_field_name("declarator")?;
    // Peel pointer/reference layers to reach function_declarator.
    let fn_decl = find_function_declarator(declarator)?;
    let inner = fn_decl.child_by_field_name("declarator")?;
    if inner.kind() != "qualified_identifier" {
        return None;
    }
    // Walk the name chain to the innermost qualified_identifier whose name is not itself
    // a qualified_identifier — that's the one where scope is the owning class.
    let mut qi = inner;
    loop {
        let name_child = qi.child_by_field_name("name")?;
        if name_child.kind() == "qualified_identifier" {
            // One more level of nesting; descend.
            qi = name_child;
        } else {
            // `name_child` is the function name (identifier); `scope` is the class.
            let scope = qi.child_by_field_name("scope")?;
            return cpp_scope_to_name(scope, source);
        }
    }
}

/// Reduce a C++ `qualified_identifier` scope to the rightmost simple name, stripping
/// template args. `Foo::Bar` → `Bar`, `std::vector` → `vector`, `Foo<T>` → `Foo`.
fn cpp_scope_to_name(scope_node: Node, source: &[u8]) -> Option<String> {
    match scope_node.kind() {
        "namespace_identifier" | "type_identifier" | "identifier" => {
            scope_node.utf8_text(source).ok().map(|t| t.to_string())
        }
        "template_type" => {
            // `Foo<T>` — the base is the first `type_identifier` child.
            for i in 0..scope_node.child_count() {
                let child = scope_node.child(i as u32).unwrap();
                if child.kind() == "type_identifier" || child.kind() == "identifier" {
                    return child.utf8_text(source).ok().map(|t| t.to_string());
                }
            }
            None
        }
        "nested_namespace_specifier" | "qualified_identifier" => {
            // Nested: take the rightmost identifier portion.
            for i in (0..scope_node.child_count()).rev() {
                let child = scope_node.child(i as u32).unwrap();
                if let Some(name) = cpp_scope_to_name(child, source) {
                    return Some(name);
                }
            }
            None
        }
        _ => None,
    }
}

/// Walk a declarator to find the first `function_declarator` node (peeling pointer/
/// reference/parenthesized layers). Returns `None` if none found.
fn find_function_declarator(node: Node) -> Option<Node> {
    if node.kind() == "function_declarator" {
        return Some(node);
    }
    // `pointer_declarator` / `abstract_function_declarator` carry a `declarator` field.
    if let Some(inner) = node.child_by_field_name("declarator") {
        return find_function_declarator(inner);
    }
    // `reference_declarator` and `parenthesized_declarator` have NO `declarator` field
    // (tree-sitter-cpp 0.23.4 node-types.json confirmed); their inner declarator is a
    // positional named child. Recurse over named children, but only into declarator-wrapper
    // kinds — never `parameter_list` — so the walk cannot pick up a parameter's name.
    if matches!(
        node.kind(),
        "reference_declarator" | "parenthesized_declarator"
    ) {
        for i in 0..node.child_count() {
            let child = node.child(i as u32).unwrap();
            if matches!(
                child.kind(),
                "function_declarator"
                    | "pointer_declarator"
                    | "reference_declarator"
                    | "parenthesized_declarator"
                    | "array_declarator"
                    | "attributed_declarator"
            ) {
                if let Some(found) = find_function_declarator(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// C++ class member access: walk backward through previous siblings in a
/// `field_declaration_list` to find the nearest `access_specifier`. The default
/// visibility depends on the container kind: `class_specifier` defaults to `private`
/// (returns false when no specifier found), `struct_specifier` defaults to `public`
/// (returns true when none found). This function only returns the found specifier text;
/// the caller decides the default.
fn cpp_nearest_access_specifier(member_node: Node, source: &[u8]) -> Option<String> {
    let mut sibling = member_node.prev_sibling();
    while let Some(curr) = sibling {
        if curr.kind() == "access_specifier" {
            if let Ok(text) = curr.utf8_text(source) {
                // Text is e.g. "public:" or "private:" — grab the keyword.
                return Some(text.trim().trim_end_matches(':').trim().to_string());
            }
        }
        sibling = curr.prev_sibling();
    }
    None
}

/// C++ member export rule: `public` access (or struct default public) → exported;
/// `private`/`protected` (or class default private) → not exported. `member_node` is
/// the `field_declaration` inside a `field_declaration_list`; `container_kind` is the
/// parent `class_specifier` or `struct_specifier` kind.
fn cpp_member_is_exported(member_node: Node, container_kind: &str, source: &[u8]) -> bool {
    match cpp_nearest_access_specifier(member_node, source) {
        Some(ref spec) if spec == "public" => true,
        Some(_) => false,                             // private or protected
        None => container_kind == "struct_specifier", // struct defaults public, class private
    }
}

pub(crate) struct CppSpec;

impl LanguageSpec for CppSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_cpp_query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_cpp_tags_query())
    }

    fn static_collection_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_cpp_static_collection_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        // `.h` is parsed with the C++ grammar (tolerant of plain C).
        &["h", "cpp", "cc", "cxx", "hpp", "hh", "hxx"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn is_import_line(&self, line: &str) -> bool {
        // C/C++: `#include` is the only import-equivalent construct.
        line.trim_start().starts_with("#include")
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node,
        kind: &str,
        _ext: &str,
        source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        name_for_cfn(capture_name, node, kind, source)
    }

    fn is_test(
        &self,
        _node: Node,
        _name: &str,
        _kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        // C/C++/ASM: use path-only test detection (no framework-specific attribute parsing).
        super::super::path_indicates_test(file_path)
    }

    fn is_exported(
        &self,
        node: Node,
        _name: &str,
        kind: &str,
        source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        // C++: function/declaration with `static` → file-local.
        // Class members: check access specifier. In-class members are inside a
        // `field_declaration_list` whose parent is a `class_specifier` or `struct_specifier`.
        // They appear as `field_declaration` (method prototype or data member) OR as
        // `function_definition` (inline method body). Both honor the access specifier; the
        // prev-sibling walk in `cpp_member_is_exported` is position-based and works for either.
        let is_in_class_member = matches!(node.kind(), "field_declaration" | "function_definition")
            && node
                .parent()
                .is_some_and(|p| p.kind() == "field_declaration_list");
        if is_in_class_member {
            node.parent() // field_declaration_list
                .and_then(|fdl| fdl.parent()) // class/struct_specifier
                .filter(|c| matches!(c.kind(), "class_specifier" | "struct_specifier"))
                .map(|c| cpp_member_is_exported(node, c.kind(), source))
                .unwrap_or(false) // unexpected shape: not-exported
        } else if kind == "fn"
            && !is_inside_function(node, &["function_definition", "lambda_expression"])
        {
            !c_has_static_storage(node, source)
        } else {
            !is_inside_function(node, &["function_definition", "lambda_expression"])
        }
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        // C++ out-of-line member definition `void Foo::bar() {}`: the scope is encoded in the
        // qualified_identifier inside the function_declarator on the function_definition node
        // itself — read it directly before the ancestor walk (which would just see global scope).
        if node.kind() == "function_definition" {
            if let Some(owner) = cpp_outofline_owner(node, source) {
                return Some(owner);
            }
        }
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        // C++: function_definition and lambda_expression both introduce new lexical scopes.
        &["function_definition", "lambda_expression"]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        // C++: additionally class_specifier. template_declaration is passthrough.
        &[
            "struct_specifier",
            "union_specifier",
            "enum_specifier",
            "class_specifier",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        // C++: additionally declaration_list (namespace body — members are free, walk
        // continues but never attributes to the namespace itself) and template_declaration
        // (wraps the real class/function; the inner node carries the name).
        &[
            "field_declaration_list",
            "enumerator_list",
            "declaration_list", // namespace body — transparent, never owned by namespace
            "template_declaration",
        ]
    }

    // is_deprecated uses the trait default (docstring `@deprecated` marker).
}
