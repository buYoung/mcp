//! Extract compact type facts from the parser's existing tree, never from free-text matches.
mod declarations;
mod dispatch;
mod fields;
mod imports;
pub(crate) use imports::{dart_library_exports, dart_library_imports};

use super::*;
use crate::parser::{CodeRange, ExtractedSymbol, NavigationFile};
use tree_sitter::{Node, Point, Tree};

pub(super) fn text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

pub(super) fn range(node: Node<'_>) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

pub(super) fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

pub(super) fn child<'a>(node: Node<'a>, kinds: &[&str]) -> Option<Node<'a>> {
    children(node)
        .into_iter()
        .find(|node| kinds.contains(&node.kind()))
}

pub(super) fn contains(outer: Node<'_>, inner: Node<'_>) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

pub(super) fn at<'a>(tree: &'a Tree, location: &CodeRange) -> Option<Node<'a>> {
    tree.root_node().named_descendant_for_point_range(
        Point::new(
            location.start_line.saturating_sub(1),
            location.start_col.saturating_sub(1),
        ),
        if location.end_col > 1 {
            Point::new(
                location.end_line.saturating_sub(1),
                location.end_col.saturating_sub(2),
            )
        } else {
            Point::new(
                location.start_line.saturating_sub(1),
                location.start_col.saturating_sub(1),
            )
        },
    )
}

pub(super) fn is_type(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class"
            | "class_declaration"
            | "abstract_class_declaration"
            | "class_definition"
            | "class_specifier"
            | "struct_specifier"
            | "struct_declaration"
            | "record_declaration"
            | "interface_declaration"
            | "trait_definition"
            | "trait_declaration"
            | "protocol_declaration"
            | "object_definition"
            | "object_declaration"
            | "class_statement"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_spec"
    )
}

pub(super) fn is_method(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "method_definition"
            | "method_declaration"
            | "method_signature"
            | "abstract_method_signature"
            | "function_declaration"
            | "function_definition"
            | "function_item"
            | "function_signature_item"
            | "function_signature"
            | "method"
            | "class_method_definition"
            | "protocol_function_declaration"
            | "declaration"
            | "field_declaration"
            | "method_elem"
    )
}

pub(super) fn declaration<'a>(
    tree: &'a Tree,
    symbol: &ExtractedSymbol,
    is_owner: bool,
) -> Option<Node<'a>> {
    let start = at(tree, &symbol.range)?;
    let node = std::iter::once(start)
        .chain(crate::parser::ancestors(start))
        .take(16)
        .find(|node| {
            if is_owner {
                is_type(*node)
            } else {
                is_method(*node)
            }
        })?;
    if !is_owner && node.kind() == "function_signature" {
        return crate::parser::ancestors(node)
            .take_while(|parent| {
                matches!(
                    parent.kind(),
                    "declaration" | "method_signature" | "method_declaration"
                )
            })
            .last()
            .or(Some(node));
    }
    Some(node)
}

pub(super) fn body(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("body").or_else(|| {
        child(
            node,
            &[
                "class_body",
                "class_member",
                "function_body",
                "protocol_body",
                "template_body",
                "declaration_list",
                "field_declaration_list",
                "interface_type",
                "struct_type",
                "function_statement_list",
                "script_block",
            ],
        )
    })
}

pub(super) fn modifier(node: Node<'_>, word: &str, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    let matches = node.children(&mut cursor).any(|candidate| {
        candidate.kind() == word
            || matches!(
                candidate.kind(),
                "modifier"
                    | "modifiers"
                    | "access_modifier"
                    | "accessibility_modifier"
                    | "visibility_modifier"
                    | "inheritance_modifier"
                    | "override_modifier"
                    | "function_modifiers"
                    | "member_modifier"
                    | "abstract_modifier"
                    | "final_modifier"
                    | "static_modifier"
            ) && text(candidate, source)
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|token| token == word)
    });
    matches
}

pub(super) fn compact(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).collect()
}

pub(super) fn reference(node: Node<'_>, source: &[u8]) -> Option<TypeReference> {
    if node.has_error() || node.is_missing() {
        return None;
    }
    if node.kind() == "reference_type" {
        return reference(node.child_by_field_name("type")?, source);
    }
    if matches!(node.kind(), "dynamic_type" | "abstract_type") {
        return reference(node.child_by_field_name("trait")?, source);
    }
    let mut value = compact(text(node, source));
    if value.len() > 1024 {
        return None;
    }
    // Constructor arguments are excluded by callers selecting the type child.
    if value.contains(['(', ')', '{', '}', ';', '=', '|', '~', '"', '\'']) {
        return None;
    }
    let mut arguments = Vec::new();
    if let Some(start) = value.find('<') {
        if !value.ends_with('>') {
            return None;
        }
        let mut depth = 0usize;
        let mut offset = start + 1;
        for (i, c) in value.char_indices().skip_while(|(i, _)| *i <= start) {
            match c {
                '<' | '[' => depth += 1,
                ']' => depth = depth.checked_sub(1)?,
                '>' if depth > 0 => depth -= 1,
                ',' if depth == 0 => {
                    arguments.push(value[offset..i].to_string());
                    offset = i + 1;
                }
                _ => {}
            }
        }
        arguments.push(value[offset..value.len() - 1].to_string());
        value.truncate(start);
    }
    if value.is_empty()
        || !value
            .chars()
            .all(|c| c.is_alphanumeric() || "_.$:\\*&".contains(c))
    {
        return None;
    }
    Some(TypeReference {
        name: value,
        arguments,
        range: range(node),
    })
}

pub(super) fn namespace(node: Node<'_>, top: &str, source: &[u8]) -> String {
    let mut names = Vec::new();
    for parent in crate::parser::ancestors(node) {
        if is_type(parent)
            || matches!(
                parent.kind(),
                "mod_item"
                    | "namespace_definition"
                    | "namespace_declaration"
                    | "module"
                    | "module_definition"
            )
        {
            if let Some(name) = parent.child_by_field_name("name") {
                let name = compact(text(name, source));
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
    }
    names.reverse();
    if !top.is_empty() {
        names.insert(0, top.into());
    }
    names.join(".")
}

/// Whether `node` is an include guard: an `#ifndef NAME` or `#if !defined(NAME)` that wraps
/// the whole file and defines `NAME` first. Its body is compiled wherever the file is first
/// included, so it does not make the declarations inside it conditional.
fn is_include_guard(node: Node<'_>, source: &[u8]) -> bool {
    let guard = match node.kind() {
        "preproc_ifdef" if node.child(0).is_some_and(|token| token.kind() == "#ifndef") => {
            node.child_by_field_name("name")
        }
        "preproc_if" => node.child_by_field_name("condition"),
        _ => None,
    };
    let Some(guard) = guard else {
        return false;
    };
    let tested = compact(text(guard, source));
    let name = match node.kind() {
        "preproc_if" => tested
            .strip_prefix("!defined")
            .map(|name| name.trim_start_matches('(').trim_end_matches(')')),
        _ => Some(tested.as_str()),
    };
    let wraps_file = node.parent().is_some_and(|root| {
        root.parent().is_none()
            && children(root)
                .into_iter()
                .all(|sibling| sibling == node || sibling.kind() == "comment")
    });
    let first = children(node)
        .into_iter()
        .find(|inner| *inner != guard && inner.kind() != "comment");
    wraps_file
        && node.child_by_field_name("alternative").is_none()
        && first.is_some_and(|first| {
            first.kind() == "preproc_def"
                && first
                    .child_by_field_name("name")
                    .map(|defined| text(defined, source))
                    == name
        })
}

pub(super) fn conditions(node: Node<'_>, source: &[u8]) -> Vec<String> {
    conditions_around(node, source, true)
}

/// The conditions under which a call is compiled. A runtime `if` around a call is not one of
/// them: whenever the call runs, it names the same declaration.
pub(super) fn call_conditions(node: Node<'_>, source: &[u8]) -> Vec<String> {
    conditions_around(node, source, false)
}

fn conditions_around(node: Node<'_>, source: &[u8], counts_runtime_if: bool) -> Vec<String> {
    let mut result = Vec::new();
    for node in std::iter::once(node).chain(crate::parser::ancestors(node)) {
        if (matches!(
            node.kind(),
            "preproc_if"
                | "preproc_ifdef"
                | "preproc_elif"
                | "preproc_else"
                | "conditional_declaration"
        ) || counts_runtime_if && node.kind() == "if_statement")
            && !is_include_guard(node, source)
        {
            result
                .push("conditional declaration requires unavailable compile/runtime facts".into());
        }
        let mut before = node.prev_named_sibling();
        while let Some(attribute) = before.filter(|node| node.kind() == "attribute_item") {
            let value = text(attribute, source);
            if value.contains("cfg") {
                result.push(value.to_string());
            }
            before = attribute.prev_named_sibling();
        }
    }
    result.sort();
    result.dedup();
    result
}

/// Whether `node` declares a constructor or destructor: a function declarator without a
/// return type. It names its own type and binds nothing that could hide a type name.
fn declares_constructor(node: Node<'_>) -> bool {
    // A declaration that starts with its declarator is located by that declarator.
    let declaration = if node.kind() == "function_declarator" {
        node.parent()
    } else {
        Some(node)
    };
    declaration.is_some_and(|declaration| {
        declaration.kind() == "declaration"
            && declaration.child_by_field_name("type").is_none()
            && declaration
                .child_by_field_name("declarator")
                .is_some_and(|declarator| declarator.kind() == "function_declarator")
    })
}

pub(crate) fn collect(
    language: &str,
    file_path: &str,
    tree: &Tree,
    source: &[u8],
    symbols: &[ExtractedSymbol],
    navigation: &NavigationFile,
) -> Option<ImplementationFile> {
    if !super::supports(language) {
        return None;
    }
    let mut unit = ImplementationUnit {
        language: language.into(),
        ..ImplementationUnit::default()
    };
    imports::collect(tree.root_node(), source, &mut unit, navigation);
    let mut omitted = 0;
    for binding in &navigation.local_bindings {
        if unit.shadows.len() >= METHODS_PER_FILE {
            omitted += 1;
            break;
        }
        if let Some(node) = at(tree, &binding.range).filter(|node| !declares_constructor(*node)) {
            unit.shadows.push(ScopedName {
                name: binding.name.clone(),
                scope: range(imports::scope(node)),
                range: binding.range.clone(),
            });
        }
    }
    for symbol in symbols.iter().filter(|symbol| symbol.kind == "type") {
        if unit.shadows.len() >= METHODS_PER_FILE {
            omitted += 1;
            break;
        }
        if let Some(node) = at(tree, &symbol.range) {
            if !is_type(node) {
                unit.shadows.push(ScopedName {
                    name: symbol.name.clone(),
                    scope: range(imports::scope(node)),
                    range: symbol.range.clone(),
                });
            }
        }
    }
    let mut owners = Vec::new();
    for symbol in symbols.iter().filter(|symbol| {
        matches!(
            symbol.kind.as_str(),
            "class" | "struct" | "trait" | "interface" | "object" | "record" | "enum"
        )
    }) {
        if unit.types.len() >= TYPES_PER_FILE {
            omitted += 1;
            continue;
        }
        let Some(node) = declaration(tree, symbol, true) else {
            continue;
        };
        if language == "swift" && modifier(node, "extension", source) {
            continue;
        }
        // `class Name;` and `struct Name;` name a type that is defined elsewhere.
        if matches!(node.kind(), "class_specifier" | "struct_specifier") && body(node).is_none() {
            continue;
        }
        if owners
            .iter()
            .any(|(existing, _): &(Node<'_>, usize)| existing.id() == node.id())
        {
            continue;
        }
        let record = declarations::type_declaration(node, symbol, source, &unit);
        owners.push((node, unit.types.len()));
        unit.types.push(record);
    }
    let mut method_count = 0;
    for symbol in symbols.iter().filter(|symbol| symbol.kind == "fn") {
        let Some(node) = declaration(tree, symbol, false) else {
            continue;
        };
        if method_count >= METHODS_PER_FILE {
            omitted += 1;
            continue;
        }
        if declarations::detached_method(node, symbol, source, &mut unit) {
            method_count += 1;
            continue;
        }
        let mut owner = None;
        for candidate in crate::parser::ancestors(node) {
            if let Some((_, index)) = owners
                .iter()
                .find(|(owner, _)| owner.id() == candidate.id())
            {
                owner = Some(*index);
                break;
            }
            if is_method(candidate) || candidate.kind() == "lambda_expression" {
                break;
            }
        }
        let Some(index) = owner else { continue };
        let mut method = declarations::method(
            node,
            symbol,
            source,
            language,
            unit.types[index].is_contract,
        );
        if language == "python" {
            method.is_abstract = declarations::python_abstract(node, source, &unit)
                || unit.types[index].is_contract
                    && body(node)
                        .is_some_and(|body| matches!(text(body, source).trim(), "..." | "pass"));
        }
        if method.name == unit.types[index].name
            || matches!(
                method.name.as_str(),
                "constructor" | "__init__" | "initialize"
            )
        {
            continue;
        }
        if !unit.types[index]
            .methods
            .iter()
            .any(|old| old.range == method.range && old.name == method.name)
        {
            unit.types[index].is_abstract |= method.is_abstract;
            unit.types[index].methods.push(method);
            method_count += 1;
        }
    }
    declarations::detached_types(tree.root_node(), source, &mut unit, &mut omitted);
    for (node, index) in &owners {
        if let Some(body) = body(*node) {
            for member in children(body)
                .into_iter()
                .filter(|node| node.kind() == "function_signature_item")
            {
                let Some(name) = member.child_by_field_name("name") else {
                    continue;
                };
                if method_count >= METHODS_PER_FILE {
                    omitted += 1;
                    continue;
                }
                let symbol = ExtractedSymbol {
                    name: text(name, source).into(),
                    kind: "fn".into(),
                    range: range(member),
                    docstring: None,
                    owner: None,
                    flags: crate::parser::SymbolFlags {
                        has_todo: false,
                        has_fixme: false,
                        is_test: false,
                        is_exported: false,
                        is_deprecated: false,
                    },
                };
                if !unit.types[*index]
                    .methods
                    .iter()
                    .any(|method| method.range == symbol.range)
                {
                    unit.types[*index].methods.push(declarations::method(
                        member, &symbol, source, language, true,
                    ));
                    method_count += 1;
                }
            }
        }
    }
    dispatch::collect(
        tree,
        source,
        symbols,
        navigation,
        &owners,
        &mut unit,
        &mut omitted,
    );
    if language == "go" {
        let stem = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        let has_target_suffix = stem.split('_').skip(1).any(|part| {
            matches!(
                part,
                "aix"
                    | "android"
                    | "darwin"
                    | "dragonfly"
                    | "freebsd"
                    | "illumos"
                    | "ios"
                    | "js"
                    | "linux"
                    | "netbsd"
                    | "openbsd"
                    | "plan9"
                    | "solaris"
                    | "wasip1"
                    | "windows"
                    | "386"
                    | "amd64"
                    | "arm"
                    | "arm64"
                    | "loong64"
                    | "mips"
                    | "mips64"
                    | "mips64le"
                    | "mipsle"
                    | "ppc64"
                    | "ppc64le"
                    | "riscv64"
                    | "s390x"
                    | "wasm"
            )
        });
        let has_build_tag = children(tree.root_node())
            .iter()
            .take_while(|node| node.kind() != "package_clause")
            .any(|node| {
                node.kind() == "comment"
                    && (text(*node, source).starts_with("//go:build")
                        || text(*node, source).starts_with("// +build"))
            });
        if has_target_suffix || has_build_tag {
            let reason = "Go build constraints require unavailable target/tag facts".to_string();
            for typ in &mut unit.types {
                typ.is_complete = false;
                typ.conditions.push(reason.clone());
                for method in &mut typ.methods {
                    method.conditions.push(reason.clone());
                }
            }
            for block in &mut unit.implementations {
                block.conditions.push(reason.clone());
            }
            for call in &mut unit.calls {
                call.conditions.push(reason.clone());
            }
        }
    }
    Some(ImplementationFile {
        source_digest: digest(source),
        units: vec![unit],
        omitted,
    })
}
