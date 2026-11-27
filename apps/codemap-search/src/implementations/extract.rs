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

/// The arguments of a type argument list written between `<` and `>`, or `[` and `]`
/// (Python/Scala/Go generics), without whitespace. Empty `[]` array suffixes return `None`.
pub(super) fn type_arguments(list: &str) -> Option<Vec<String>> {
    let (open, close) = if list.starts_with('<') && list.ends_with('>') {
        ('<', '>')
    } else if list.starts_with('[') && list.ends_with(']') {
        ('[', ']')
    } else {
        return None;
    };
    let inner = list.strip_prefix(open)?.strip_suffix(close)?;
    if inner.is_empty() {
        return None;
    }
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut offset = 0;
    for (i, c) in inner.char_indices() {
        match c {
            '<' | '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.checked_sub(1)?,
            '>' if depth > 0 => depth -= 1,
            ',' if depth == 0 => {
                let piece = &inner[offset..i];
                if !piece.is_empty() {
                    arguments.push(piece.to_string());
                }
                offset = i + 1;
            }
            _ => {}
        }
    }
    let tail = &inner[offset..];
    if !tail.is_empty() {
        arguments.push(tail.to_string());
    }
    (!arguments.is_empty()).then_some(arguments)
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
    let generic_start = match (value.find('<'), value.find('[')) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) if b > 0 && &value[b..] != "[]" => Some(b),
        _ => None,
    };
    if let Some(start) = generic_start {
        arguments = type_arguments(&value[start..])?;
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
        conditions: Vec::new(),
    })
}

/// Fallback owner spelling for detached impl blocks targeting external/primitive types
/// (e.g. Rust `impl LocalTrait for &str` or `impl LocalTrait for [u8]`), so explicit impl
/// block locations can be reported without pretending the owner is a local indexed type.
pub(super) fn raw_owner_reference(node: Node<'_>, source: &[u8]) -> Option<TypeReference> {
    if node.kind() != "reference_type" {
        if let Some(parsed) = reference(node, source) {
            return Some(parsed);
        }
    }
    if node.has_error() || node.is_missing() {
        return None;
    }
    let name = compact(text(node, source));
    if name.is_empty() || name.len() > 256 {
        return None;
    }
    Some(TypeReference {
        name,
        arguments: Vec::new(),
        range: range(node),
        conditions: Vec::new(),
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
    conditions_around("", node, source, true)
}

pub(super) fn conditions_for(language: &str, node: Node<'_>, source: &[u8]) -> Vec<String> {
    conditions_around(language, node, source, true)
}

pub(crate) fn preproc_conditions_for_node(
    language: &str,
    node: Node<'_>,
    source: &[u8],
) -> Vec<String> {
    conditions_around(language, node, source, true)
}

/// The conditions under which a call is compiled. A runtime `if` around a call is not one of
/// them: whenever the call runs, it names the same declaration.
pub(super) fn call_conditions(node: Node<'_>, source: &[u8]) -> Vec<String> {
    conditions_around("", node, source, false)
}

fn preproc_condition_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    fn strip_comment(s: &str) -> &str {
        s.split("//")
            .next()
            .unwrap_or("")
            .split("/*")
            .next()
            .unwrap_or("")
            .trim()
    }
    let raw = text(node, source);
    let first_line = raw.lines().next().unwrap_or("").trim();
    for (prefix, canonical) in [
        ("#ifdef", "#ifdef"),
        ("#ifndef", "#ifndef"),
        ("#elif", "#if"),
        ("#if", "#if"),
    ] {
        if let Some(rest) = first_line.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('(') {
                let expr = strip_comment(rest);
                if !expr.is_empty() && expr.len() <= 160 {
                    return Some(format!("{canonical} {expr}"));
                }
            }
        }
    }
    if first_line.starts_with("#else") {
        // Derive the negated condition from the enclosing `#if`/`#ifdef`/`#ifndef` when available.
        for ancestor in crate::parser::ancestors(node) {
            if matches!(ancestor.kind(), "preproc_if" | "preproc_ifdef") {
                let parent_line = text(ancestor, source).lines().next().unwrap_or("").trim();
                for (prefix, negated) in [
                    ("#ifdef", "#ifndef"),
                    ("#ifndef", "#ifdef"),
                    ("#if", "#if !"),
                ] {
                    if let Some(rest) = parent_line.strip_prefix(prefix) {
                        if rest.is_empty()
                            || rest.starts_with(char::is_whitespace)
                            || rest.starts_with('(')
                        {
                            let expr = strip_comment(rest);
                            if !expr.is_empty() && expr.len() <= 140 {
                                return Some(if negated == "#if !" {
                                    format!("#if !({expr})")
                                } else {
                                    format!("{negated} {expr}")
                                });
                            }
                        }
                    }
                }
                break;
            }
        }
        return Some("#else".into());
    }
    None
}

fn conditions_around(
    language: &str,
    node: Node<'_>,
    source: &[u8],
    counts_runtime_if: bool,
) -> Vec<String> {
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
            if matches!(language, "csharp" | "cpp") {
                if let Some(label) = preproc_condition_label(node, source) {
                    result.push(label);
                } else {
                    result.push(
                        "conditional declaration requires unavailable compile/runtime facts".into(),
                    );
                }
            } else {
                result.push(
                    "conditional declaration requires unavailable compile/runtime facts".into(),
                );
            }
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
    // Collect typed fields for each named type so Rust `self.field.m()` and cross-file
    // chained receivers (`x.field.m()`) resolve against the snapshot without reparsing.
    for &(owner_node, index) in &owners {
        let mut declared = fields::Fields::of(owner_node, language, source).to_declarations();
        if declared.len() > 64 {
            declared.truncate(64);
        }
        unit.types[index].fields = declared;
    }
    // Collect anonymous implementation owners: Java `new Base<X>() { ... }` and Kotlin
    // `object : Base { ... }`. They stay out of `by_name` and out of `symbol.owner`,
    // attaching overrides only to their own anonymous `TypeDeclaration`.
    if matches!(language, "java" | "kotlin") {
        collect_anonymous_owners(
            tree.root_node(),
            language,
            source,
            &mut owners,
            &mut unit,
            &mut omitted,
        );
    }
    if matches!(language, "c" | "cpp") {
        unit.export_macros = collect_cpp_export_macros(tree.root_node(), source);
    }
    if language == "cpp" {
        unit.specialized_templates =
            collect_cpp_specialized_templates(tree.root_node(), source, &unit.namespace);
    }
    if language == "csharp" && tree.root_node().has_error() {
        recover_csharp_preproc_header_types(source, &mut unit, &mut omitted);
    }
    if matches!(language, "typescript" | "javascript") {
        for statement in children(tree.root_node())
            .into_iter()
            .filter(|node| node.kind() == "export_statement")
        {
            let mut cursor = statement.walk();
            let is_default = statement
                .children(&mut cursor)
                .any(|child| !child.is_named() && child.kind() == "default");
            if !is_default {
                continue;
            }
            if let Some(value) = statement
                .child_by_field_name("value")
                .filter(|node| node.kind() == "identifier")
            {
                let name = text(value, source);
                let matching: Vec<_> = unit
                    .types
                    .iter_mut()
                    .filter(|typ| typ.namespace.is_empty() && typ.name == name)
                    .collect();
                if matching.len() == 1 {
                    let typ = matching.into_iter().next().unwrap();
                    typ.is_exported = true;
                    typ.is_default_export = true;
                }
            }
        }
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

/// Collects anonymous implementation owners in Java (`new Base<X>() { ... }`) and Kotlin
/// (`object : Base { ... }`), inheriting enclosing type parameters so outer generic binders
/// remain in scope while keeping methods unattached to any enclosing named class.
fn collect_anonymous_owners<'tree>(
    root: Node<'tree>,
    language: &str,
    source: &[u8],
    owners: &mut Vec<(Node<'tree>, usize)>,
    unit: &mut ImplementationUnit,
    omitted: &mut usize,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let is_java_anon = language == "java"
            && node.kind() == "object_creation_expression"
            && child(node, &["class_body"]).is_some();
        let is_kotlin_anon = language == "kotlin"
            && node.kind() == "object_literal"
            && child(node, &["class_body"]).is_some()
            && child(node, &["delegation_specifiers"]).is_some();
        if is_java_anon || is_kotlin_anon {
            if unit.types.len() >= TYPES_PER_FILE {
                *omitted += 1;
            } else {
                let bases: Vec<TypeReference> = if is_java_anon {
                    node.child_by_field_name("type")
                        .and_then(|typ| reference(typ, source))
                        .into_iter()
                        .collect()
                } else {
                    declarations::base_references(node, source)
                };
                if !bases.is_empty() {
                    let first_base = bases[0]
                        .name
                        .rsplit(['.', ':', '\\'])
                        .next()
                        .unwrap_or(&bases[0].name);
                    let r = range(node);
                    let anon_name = format!("<anonymous {first_base}@L{}>", r.start_line);
                    let ns = namespace(node, &unit.namespace, source);
                    let qualified_name = if ns.is_empty() {
                        anon_name.clone()
                    } else {
                        format!("{ns}.{anon_name}")
                    };
                    // Inherit type parameters of enclosing named types/methods so references like
                    // `new TypeAdapter<T>() { public T read(...) }` preserve the binder `T`.
                    let mut inherited_params = Vec::new();
                    for anc in crate::parser::ancestors(node) {
                        for p in declarations::extract_type_parameters(anc, source) {
                            if !inherited_params.contains(&p) {
                                inherited_params.push(p);
                            }
                        }
                    }
                    let index = unit.types.len();
                    let body_err = child(node, &["class_body"]).is_some_and(|b| b.has_error());
                    unit.types.push(TypeDeclaration {
                        name: anon_name,
                        qualified_name,
                        range: r,
                        namespace: ns,
                        scope: range(imports::scope(node)),
                        parameters: inherited_params,
                        bases,
                        methods: Vec::new(),
                        is_contract: false,
                        is_exported: false,
                        is_default_export: false,
                        is_abstract: false,
                        is_final: true,
                        is_complete: !node.is_error() && !node.is_missing(),
                        has_body_error: body_err,
                        is_partial: false,
                        is_anonymous: true,
                        fields: Vec::new(),
                        conditions: conditions(node, source),
                    });
                    owners.push((node, index));
                }
            }
        }
        let mut cursor = node.walk();
        for child_node in node.named_children(&mut cursor) {
            stack.push(child_node);
        }
    }
}

/// Collects C/C++ `#define NAME ...` macro names whose expansion is empty or consists strictly
/// of export/visibility attributes (`__attribute__((visibility(...)))`, `__declspec(dllexport)`,
/// `[[...]]`, `alignas(...)`). Only confirmed macros are ever masked during header recovery.
fn collect_cpp_export_macros(root: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut macros = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "preproc_def" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = text(name_node, source).trim();
                let value = node
                    .child_by_field_name("value")
                    .map(|v| text(v, source).trim())
                    .unwrap_or("");
                if !name.is_empty()
                    && name.len() >= 3
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !name.ends_with("_H")
                    && !name.ends_with("_HPP")
                    && !name.ends_with("_H_")
                    && is_pure_export_macro_body(value)
                {
                    macros.push(name.to_string());
                }
            }
            continue;
        }
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            if matches!(
                c.kind(),
                "preproc_def"
                    | "preproc_if"
                    | "preproc_ifdef"
                    | "preproc_else"
                    | "preproc_elif"
                    | "linkage_specification"
                    | "declaration_list"
            ) {
                stack.push(c);
            }
        }
    }
    macros.sort();
    macros.dedup();
    macros
}

fn is_pure_export_macro_body(value: &str) -> bool {
    let trimmed = value
        .split("//")
        .next()
        .unwrap_or("")
        .split("/*")
        .next()
        .unwrap_or("")
        .trim();
    if trimmed.is_empty() {
        return true;
    }
    // Accept only explicit compiler visibility / DLL export / C++ attribute decorators.
    let allowed_prefixes = ["__attribute__", "__declspec", "[[", "alignas"];
    let has_export_marker = trimmed.contains("__attribute__")
        || trimmed.contains("__declspec")
        || trimmed.contains("dllexport")
        || trimmed.contains("dllimport")
        || trimmed.contains("visibility");
    has_export_marker && allowed_prefixes.iter().any(|p| trimmed.starts_with(p))
}

/// Recovers C# type declarations whose base list contains inline `#if ... #endif` directives
/// (e.g. `public partial class JContainer : JToken #if HAVE_COMP , ITypedList #endif { ... }`),
/// which cause tree-sitter-c-sharp to drop the enclosing `class_declaration` into an `ERROR` node.
/// Masks ONLY the `#if ... #endif` spans inside broken headers with same-length spaces so byte
/// offsets and line coordinates remain exact, re-parses the class and its methods, and reads both
/// unconditional and `#if`-tagged bases from the original `source` bytes.
fn recover_csharp_preproc_header_types(
    source: &[u8],
    unit: &mut ImplementationUnit,
    omitted: &mut usize,
) {
    let Ok(text_str) = std::str::from_utf8(source) else {
        return;
    };
    if !text_str.contains("#if") {
        return;
    }
    // Mask `#if ... #endif` blocks that appear after a class/struct/interface/record header line
    // containing `:` before its opening `{`, keeping newlines intact so line/col coordinates match.
    let mut masked = source.to_vec();
    let mut in_header = false;
    let mut if_depth = 0usize;
    let mut touched_header_lines = std::collections::HashSet::new();
    let mut current_header_line = 0usize;
    let mut offset = 0usize;
    for (idx, line) in text_str.split_inclusive('\n').enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        let line_len = line.len();
        let content_end = offset + line.trim_end_matches(['\r', '\n']).len();
        if !in_header {
            let is_type_header = (trimmed.contains("class ")
                || trimmed.contains("struct ")
                || trimmed.contains("interface ")
                || trimmed.contains("record "))
                && trimmed.contains(':')
                && !trimmed.contains('{')
                && !trimmed.ends_with(';');
            if is_type_header {
                in_header = true;
                current_header_line = line_no;
                if_depth = 0;
            }
        } else {
            if trimmed.starts_with("#if") {
                if_depth += 1;
                for b in &mut masked[offset..content_end] {
                    *b = b' ';
                }
                touched_header_lines.insert(current_header_line);
            } else if trimmed.starts_with("#elif") || trimmed.starts_with("#else") {
                if if_depth > 0 {
                    for b in &mut masked[offset..content_end] {
                        *b = b' ';
                    }
                    touched_header_lines.insert(current_header_line);
                }
            } else if trimmed.starts_with("#endif") {
                if if_depth > 0 {
                    if_depth -= 1;
                    for b in &mut masked[offset..content_end] {
                        *b = b' ';
                    }
                    touched_header_lines.insert(current_header_line);
                }
            } else if if_depth > 0 {
                // Mask conditional base tokens inside the `#if ... #endif` header span so the
                // unconditional header parses cleanly; the original source still supplies all
                // conditional bases with their `#if` tags below.
                for b in &mut masked[offset..content_end] {
                    *b = b' ';
                }
            } else if trimmed.contains('{') || trimmed.ends_with(';') {
                in_header = false;
            }
        }
        offset += line_len;
    }
    if touched_header_lines.is_empty() {
        return;
    }
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .is_err()
    {
        return;
    }
    let Some(tree) = parser.parse(&masked, None) else {
        return;
    };
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if matches!(
            node.kind(),
            "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "record_declaration"
        ) && body(node).is_some()
            && touched_header_lines.contains(&(node.start_position().row + 1))
        {
            if unit.types.len() >= TYPES_PER_FILE {
                *omitted += 1;
                continue;
            }
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = compact(text(name_node, &masked));
                let r = range(node);
                let already_present = unit
                    .types
                    .iter()
                    .any(|t| t.range.start_line == r.start_line && t.name == name);
                if !name.is_empty() && !already_present {
                    let sym = ExtractedSymbol {
                        name: name.clone(),
                        kind: match node.kind() {
                            "interface_declaration" => "interface".into(),
                            "struct_declaration" => "struct".into(),
                            "record_declaration" => "record".into(),
                            _ => "class".into(),
                        },
                        range: r,
                        docstring: None,
                        owner: None,
                        flags: crate::parser::SymbolFlags {
                            has_todo: false,
                            has_fixme: false,
                            is_test: false,
                            is_exported: modifier(node, "public", &masked),
                            is_deprecated: false,
                        },
                    };
                    let mut record = declarations::type_declaration(node, &sym, &masked, unit);
                    record.bases.clear();
                    declarations::collect_csharp_conditional_bases(node, source, &mut record);
                    record.fields = fields::Fields::of(node, "csharp", &masked).to_declarations();
                    if let Some(body_node) = body(node) {
                        for member in children(body_node) {
                            if member.kind() != "method_declaration" {
                                continue;
                            }
                            let Some(m_name_node) = member.child_by_field_name("name") else {
                                continue;
                            };
                            let m_name = compact(text(m_name_node, &masked));
                            if m_name.is_empty() || m_name == name {
                                continue;
                            }
                            let m_sym = ExtractedSymbol {
                                name: m_name,
                                kind: "fn".into(),
                                range: range(member),
                                docstring: None,
                                owner: Some(name.clone()),
                                flags: crate::parser::SymbolFlags {
                                    has_todo: false,
                                    has_fixme: false,
                                    is_test: false,
                                    is_exported: modifier(member, "public", &masked),
                                    is_deprecated: false,
                                },
                            };
                            let m = declarations::method(
                                member,
                                &m_sym,
                                &masked,
                                "csharp",
                                record.is_contract,
                            );
                            record.is_abstract |= m.is_abstract;
                            record.methods.push(m);
                        }
                    }
                    unit.types.push(record);
                }
            }
        }
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            stack.push(c);
        }
    }
}

/// Collects qualified names of C++ template classes/structs that declare partial or explicit
/// specializations (`template<> class Box<int> {}` or `template<typename T> class Box<T*> {}`).
fn collect_cpp_specialized_templates(root: Node<'_>, source: &[u8], unit_ns: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "class_specifier" | "struct_specifier")
            && node
                .parent()
                .is_some_and(|p| p.kind() == "template_declaration")
        {
            if let Some(name_node) = node.child_by_field_name("name") {
                if name_node.kind() == "template_type" {
                    let base = name_node
                        .child_by_field_name("name")
                        .map(|n| compact(text(n, source)))
                        .unwrap_or_else(|| {
                            compact(text(name_node, source))
                                .split('<')
                                .next()
                                .unwrap_or("")
                                .to_string()
                        });
                    if !base.is_empty() {
                        let ns = namespace(node, unit_ns, source);
                        let qname = if ns.is_empty() {
                            base
                        } else {
                            format!("{ns}.{base}")
                        };
                        found.push(qname);
                    }
                }
            }
        }
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            stack.push(c);
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Recovers C++ class/struct declarations whose header was hidden from tree-sitter-cpp by a
/// confirmed export decorator macro (e.g. `class LEVELDB_EXPORT Comparator { ... }`).
/// Only tokens present in `confirmed_macros` (proven from `#define` in included headers or the
/// file itself) are replaced with same-length spaces so byte offsets and line coordinates remain
/// exact and ordinary declarations like `struct REAL instance {};` are never altered.
pub(crate) fn recover_cpp_decorated_types(
    source: &[u8],
    existing: &ImplementationUnit,
    confirmed_macros: &std::collections::HashSet<&str>,
) -> Vec<TypeDeclaration> {
    if existing.language != "cpp" || confirmed_macros.is_empty() {
        return Vec::new();
    }
    let Ok(text_str) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let mut masked = source.to_vec();
    let mut touched_lines = std::collections::HashSet::new();
    let mut line_offset = 0usize;
    for (idx, line) in text_str.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_start();
        let leading = line.len() - trimmed.len();
        let kw_len = if trimmed.starts_with("class ") || trimmed.starts_with("class\t") {
            Some(5)
        } else if trimmed.starts_with("struct ") || trimmed.starts_with("struct\t") {
            Some(6)
        } else {
            None
        };
        if let Some(kw) = kw_len {
            let after_kw = &trimmed[kw..];
            let ws_after_kw = after_kw.len() - after_kw.trim_start().len();
            let rest = &after_kw[ws_after_kw..];
            let tok_len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            if tok_len > 0 {
                let tok = &rest[..tok_len];
                if confirmed_macros.contains(tok) {
                    // Require a valid type name immediately following the macro token.
                    let after_tok = rest[tok_len..].trim_start();
                    let next_len = after_tok
                        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                        .unwrap_or(after_tok.len());
                    if next_len > 0 {
                        let start = line_offset + leading + kw + ws_after_kw;
                        for b in &mut masked[start..start + tok_len] {
                            *b = b' ';
                        }
                        touched_lines.insert(idx + 1);
                    }
                }
            }
        } else {
            // Also handle `MACRO class Name` / `MACRO struct Name` where the confirmed export
            // macro precedes the keyword.
            let tok_len = trimmed
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(trimmed.len());
            if tok_len > 0 && confirmed_macros.contains(&trimmed[..tok_len]) {
                let after_tok = trimmed[tok_len..].trim_start();
                if after_tok.starts_with("class ")
                    || after_tok.starts_with("class\t")
                    || after_tok.starts_with("struct ")
                    || after_tok.starts_with("struct\t")
                {
                    let start = line_offset + leading;
                    for b in &mut masked[start..start + tok_len] {
                        *b = b' ';
                    }
                    touched_lines.insert(idx + 1);
                }
            }
        }
        line_offset += line.len();
    }
    if touched_lines.is_empty() {
        return Vec::new();
    }
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .is_err()
    {
        return Vec::new();
    }
    let Some(tree) = parser.parse(&masked, None) else {
        return Vec::new();
    };
    let mut recovered = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "class_specifier" | "struct_specifier")
            && body(node).is_some()
            && touched_lines.contains(&(node.start_position().row + 1))
        {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = compact(text(name_node, &masked));
                let r = range(node);
                let already_present = existing
                    .types
                    .iter()
                    .any(|t| t.range.start_line == r.start_line && t.name == name);
                if !name.is_empty() && !already_present {
                    let sym = ExtractedSymbol {
                        name: name.clone(),
                        kind: if node.kind() == "struct_specifier" {
                            "struct".into()
                        } else {
                            "class".into()
                        },
                        range: r,
                        docstring: None,
                        owner: None,
                        flags: crate::parser::SymbolFlags {
                            has_todo: false,
                            has_fixme: false,
                            is_test: false,
                            is_exported: true,
                            is_deprecated: false,
                        },
                    };
                    let mut record = declarations::type_declaration(node, &sym, &masked, existing);
                    if let Some(body_node) = body(node) {
                        for member in children(body_node) {
                            let m_node = if member.kind() == "field_declaration"
                                || member.kind() == "function_definition"
                                || member.kind() == "declaration"
                            {
                                member
                            } else {
                                continue;
                            };
                            if let Some(m_name) = cpp_member_method_name(m_node, &masked) {
                                if m_name == name || m_name.starts_with('~') {
                                    continue;
                                }
                                let m_sym = ExtractedSymbol {
                                    name: m_name,
                                    kind: "fn".into(),
                                    range: range(m_node),
                                    docstring: None,
                                    owner: Some(name.clone()),
                                    flags: crate::parser::SymbolFlags {
                                        has_todo: false,
                                        has_fixme: false,
                                        is_test: false,
                                        is_exported: true,
                                        is_deprecated: false,
                                    },
                                };
                                let m = declarations::method(m_node, &m_sym, &masked, "cpp", false);
                                record.is_abstract |= m.is_abstract;
                                record.methods.push(m);
                            }
                        }
                    }
                    recovered.push(record);
                }
            }
        }
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            stack.push(c);
        }
    }
    recovered
}

fn cpp_member_method_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut decl = node.child_by_field_name("declarator")?;
    while let Some(inner) = decl.child_by_field_name("declarator") {
        if decl.kind() == "function_declarator" {
            let name_node = inner;
            let name = text(name_node, source).trim();
            return (!name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_~".contains(c)))
            .then(|| name.to_string());
        }
        decl = inner;
    }
    None
}
