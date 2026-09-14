use super::*;

fn parameters(node: Node<'_>, source: &[u8]) -> Vec<String> {
    node.child_by_field_name("type_parameters")
        .or_else(|| child(node, &["type_parameters", "type_parameter_list"]))
        .map(|list| {
            children(list)
                .into_iter()
                .filter_map(|parameter| {
                    let name = parameter
                        .child_by_field_name("name")
                        .or_else(|| child(parameter, &["type_identifier", "identifier"]))
                        .unwrap_or(parameter);
                    let value = compact(text(name, source));
                    (!value.is_empty() && value.chars().all(|c| c.is_alphanumeric() || c == '_'))
                        .then_some(value)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn base_nodes(node: Node<'_>) -> Vec<Node<'_>> {
    let mut result = Vec::new();
    let mut pending = children(node)
        .into_iter()
        .filter(|node| {
            matches!(
                node.kind(),
                "class_heritage"
                    | "superclass"
                    | "super_interfaces"
                    | "extends_interfaces"
                    | "extends_type_clause"
                    | "base_list"
                    | "delegation_specifiers"
                    | "inheritance_specifier"
                    | "interfaces"
                    | "mixins"
                    | "extends_clause"
                    | "base_clause"
                    | "class_base"
            )
        })
        .collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        if matches!(
            node.kind(),
            "argument_list" | "arguments" | "annotation" | "access_specifier" | "modifiers"
        ) {
            continue;
        }
        if matches!(
            node.kind(),
            "class_heritage"
                | "superclass"
                | "super_interfaces"
                | "extends_interfaces"
                | "extends_type_clause"
                | "base_list"
                | "delegation_specifiers"
                | "delegation_specifier"
                | "interfaces"
                | "mixins"
                | "extends_clause"
                | "implements_clause"
                | "type_list"
                | "base_clause"
                | "class_base"
                | "mixin_clause"
        ) {
            pending.extend(children(node));
        } else if node.kind() == "inheritance_specifier" {
            if let Some(typ) = node.child_by_field_name("inherits_from") {
                result.push(typ);
            }
        } else if matches!(
            node.kind(),
            "constructor_invocation" | "primary_constructor_base_type" | "constructor_type"
        ) {
            if let Some(typ) = node.child_by_field_name("type").or_else(|| {
                child(
                    node,
                    &["user_type", "type_identifier", "identifier", "generic_name"],
                )
            }) {
                result.push(typ);
            }
        } else if matches!(node.kind(), "type_arguments" | "type_argument_list") {
            continue;
        } else {
            result.push(node);
        }
    }
    result.sort_by_key(Node::start_byte);
    result
}

pub(super) fn type_declaration(
    node: Node<'_>,
    symbol: &ExtractedSymbol,
    source: &[u8],
    unit: &ImplementationUnit,
) -> TypeDeclaration {
    let language = unit.language.as_str();
    let is_contract = matches!(symbol.kind.as_str(), "interface" | "trait")
        || matches!(
            node.kind(),
            "interface_declaration"
                | "trait_item"
                | "trait_definition"
                | "trait_declaration"
                | "protocol_declaration"
        )
        || modifier(node, "interface", source)
        || modifier(node, "trait", source)
        || modifier(node, "protocol", source);
    let namespace = namespace(node, &unit.namespace, source);
    let mut record = TypeDeclaration {
        name: symbol.name.clone(),
        qualified_name: if namespace.is_empty() {
            symbol.name.clone()
        } else {
            format!("{namespace}.{}", symbol.name)
        },
        range: symbol.range.clone(),
        namespace,
        scope: range(imports::scope(node)),
        parameters: parameters(node, source),
        bases: Vec::new(),
        methods: Vec::new(),
        is_contract,
        is_exported: symbol.flags.is_exported,
        is_abstract: is_contract
            || modifier(node, "abstract", source)
            || node.kind() == "abstract_class_declaration",
        is_final: modifier(node, "final", source) || modifier(node, "sealed", source),
        is_complete: !node.has_error(),
        conditions: conditions(node, source),
    };
    let mut candidates = base_nodes(node);
    if language == "python" {
        candidates.extend(
            node.child_by_field_name("superclasses")
                .map(children)
                .unwrap_or_default(),
        );
    }
    if language == "ruby" {
        candidates.extend(
            node.child_by_field_name("superclass")
                .map(|node| children(node))
                .unwrap_or_default(),
        );
    }
    if language == "cpp" {
        candidates.extend(
            child(node, &["base_class_clause"])
                .map(children)
                .unwrap_or_default()
                .into_iter()
                .filter(|node| !matches!(node.kind(), "access_specifier" | "virtual")),
        );
    }
    if language == "powershell" {
        candidates.extend(
            children(node)
                .into_iter()
                .filter(|node| node.kind() == "simple_name")
                .skip(1),
        );
    }
    for candidate in candidates {
        if matches!(candidate.kind(), "keyword_argument" | "argument_list") {
            record.is_complete = false;
            continue;
        }
        if let Some(mut base) = reference(candidate, source) {
            if base.arguments.is_empty() {
                if let Some(arguments) = candidate
                    .parent()
                    .filter(|node| node.kind() == "extends_clause")
                    .and_then(|node| node.child_by_field_name("type_arguments"))
                {
                    base.arguments = children(arguments)
                        .into_iter()
                        .map(|node| compact(text(node, source)))
                        .collect();
                }
            }
            if language == "python"
                && unit.imports.iter().any(|import| {
                    visible_import(import, node, unit)
                        && import.entry.source.as_deref() == Some("abc")
                        && import.entry.imported_name.as_deref() == Some("ABC")
                        && import.entry.local_name == base.name
                })
            {
                record.is_abstract = true;
                continue;
            }
            if language == "python"
                && unit.imports.iter().any(|import| {
                    visible_import(import, node, unit)
                        && matches!(
                            import.entry.source.as_deref(),
                            Some("typing" | "typing_extensions")
                        )
                        && import.entry.imported_name.as_deref() == Some("Protocol")
                        && import.entry.local_name == base.name
                })
            {
                record.is_contract = true;
                record.is_abstract = true;
                continue;
            }
            if !record
                .bases
                .iter()
                .any(|old| old.name == base.name && old.arguments == base.arguments)
            {
                record.bases.push(base);
            }
        } else {
            record.is_complete = false;
        }
    }
    if language == "go" {
        if let Some(typ) = node.child_by_field_name("type") {
            record.is_contract = typ.kind() == "interface_type";
            record.is_abstract = record.is_contract;
            // Embedded/type-set constraints need a complete method-set proof; never use a subset.
            if record.is_contract
                && children(typ)
                    .iter()
                    .any(|node| !matches!(node.kind(), "method_elem" | "comment"))
            {
                record.is_complete = false;
            }
            if typ.kind() == "struct_type" {
                if let Some(fields) = child(typ, &["field_declaration_list"]) {
                    if children(fields).iter().any(|node| {
                        node.kind() == "field_declaration"
                            && node.child_by_field_name("name").is_none()
                    }) {
                        record.is_complete = false;
                    }
                }
            }
        }
        if !record.parameters.is_empty() {
            record.is_complete = false;
        }
    }
    record
}

pub(super) fn parameter_nodes(node: Node<'_>) -> Vec<Node<'_>> {
    let node = node.child_by_field_name("signature").unwrap_or(node);
    let node = child(
        node,
        &["function_signature", "getter_signature", "setter_signature"],
    )
    .unwrap_or(node);
    let list = node.child_by_field_name("parameters").or_else(|| {
        child(
            node,
            &[
                "formal_parameters",
                "parameter_list",
                "parameters",
                "method_parameters",
                "function_value_parameters",
                "function_parameter_declaration",
                "formal_parameter_list",
            ],
        )
    });
    if let Some(list) = list {
        let list = child(list, &["parameter_list"]).unwrap_or(list);
        return children(list)
            .into_iter()
            .filter(|node| {
                !matches!(
                    node.kind(),
                    "comment" | "type_parameters" | "type_parameter_list"
                )
            })
            .collect();
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        if declarator.id() != node.id() {
            return parameter_nodes(declarator);
        }
    }
    children(node)
        .into_iter()
        .filter(|node| node.kind() == "parameter")
        .collect()
}

pub(super) fn parameter_type<'a>(node: Node<'a>) -> Option<Node<'a>> {
    node.child_by_field_name("type")
        .or_else(|| {
            child(
                node,
                &[
                    "type_annotation",
                    "type",
                    "user_type",
                    "type_literal",
                    "type_constraint",
                    "type_constraints",
                ],
            )
        })
        .or_else(|| {
            child(
                node,
                &[
                    "formal_parameter",
                    "normal_formal_parameter",
                    "simple_formal_parameter",
                    "script_parameter",
                ],
            )
            .and_then(parameter_type)
        })
}

pub(super) fn parameter_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let value = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("pattern"))
        .or_else(|| {
            child(
                node,
                &[
                    "identifier",
                    "simple_identifier",
                    "variable_name",
                    "variable",
                ],
            )
        })?;
    let name = compact(text(value, source));
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || "_$".contains(c)))
    .then_some(name)
}

fn parameter_text(node: Node<'_>, source: &[u8], language: &str) -> Option<String> {
    let typ = parameter_type(node)?;
    let mut value = compact(text(typ, source));
    value = value.trim_start_matches(':').to_string();
    if language == "powershell" {
        value = value.trim_matches(['[', ']']).to_ascii_lowercase();
    }
    if value.is_empty() || value.len() > 256 || typ.has_error() {
        return None;
    }
    Some(value)
}

fn has_body(node: Node<'_>) -> bool {
    if let Some(body) = body(node) {
        return body.child_count() > 0
            || matches!(
                body.kind(),
                "block" | "statement_block" | "function_body" | "script_block"
            );
    }
    false
}

pub(super) fn method(
    node: Node<'_>,
    symbol: &ExtractedSymbol,
    source: &[u8],
    language: &str,
    is_contract: bool,
) -> MethodDeclaration {
    let signature = node
        .child_by_field_name("signature")
        .and_then(|node| child(node, &["function_signature"]).or(Some(node)))
        .or_else(|| child(node, &["function_signature"]))
        .unwrap_or(node);
    let mut params = Vec::new();
    let mut has_known_signature =
        !node.has_error() && child(node, &["explicit_interface_specifier"]).is_none();
    let mut has_self = false;
    for parameter in parameter_nodes(signature) {
        let name = parameter_name(parameter, source);
        if parameter.kind() == "self_parameter"
            || matches!(name.as_deref(), Some("self" | "cls")) && language == "python"
        {
            has_self = true;
            continue;
        }
        if matches!(parameter.kind(), "comment" | "/" | "*") {
            continue;
        }
        let value = parameter_text(parameter, source, language);
        has_known_signature &= value.is_some();
        let mut value = value.unwrap_or_else(|| "?".into());
        if language == "csharp" {
            for mode in ["ref", "out", "in"] {
                if modifier(parameter, mode, source) {
                    value = format!("{mode}:{value}");
                }
            }
        }
        if language == "swift" {
            if let Some(label) = parameter
                .child_by_field_name("external_name")
                .or_else(|| parameter.child_by_field_name("name"))
            {
                value = format!("{}:{value}", text(label, source));
            } else {
                has_known_signature = false;
            }
        }
        let count = if language == "go" {
            let mut cursor = parameter.walk();
            parameter
                .children_by_field_name("name", &mut cursor)
                .count()
                .max(1)
        } else {
            1
        };
        params.extend(std::iter::repeat_n(value, count));
    }
    if params.len() > 32 {
        has_known_signature = false;
        params.truncate(32);
    }
    if !parameters(signature, source).is_empty() {
        has_known_signature = false;
    }
    let return_type = signature
        .child_by_field_name("return_type")
        .or_else(|| signature.child_by_field_name("returns"))
        .or_else(|| signature.child_by_field_name("result"))
        .or_else(|| signature.child_by_field_name("type"))
        .map(|node| {
            compact(text(node, source))
                .trim_start_matches(':')
                .to_string()
        })
        .unwrap_or_default();
    let is_abstract = modifier(node, "abstract", source)
        || node.kind() == "abstract_method_signature"
        || is_contract && !has_body(node)
        || language == "dart" && node.kind() == "declaration" && !has_body(node)
        || language == "cpp"
            && node
                .child_by_field_name("default_value")
                .is_some_and(|node| node.kind() == "number_literal" && text(node, source) == "0");
    let is_static = modifier(node, "static", source) || language == "rust" && !has_self;
    let mut qualifiers = String::new();
    if language == "cpp" {
        let mut declarator = node;
        while let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "function_declarator" {
                declarator = inner;
                break;
            }
            declarator = inner;
        }
        qualifiers = children(declarator)
            .into_iter()
            .filter(|child| matches!(child.kind(), "type_qualifier" | "ref_qualifier"))
            .map(|node| compact(text(node, source)))
            .collect::<Vec<_>>()
            .join(" ");
    }
    MethodDeclaration {
        name: symbol.name.clone(),
        range: symbol.range.clone(),
        parameters: params,
        return_type,
        qualifiers,
        is_abstract,
        is_static,
        is_private: modifier(node, "private", source)
            || modifier(node, "new", source)
            || symbol.name.starts_with('#')
            || language == "python"
                && symbol.name.starts_with("__")
                && !symbol.name.ends_with("__"),
        is_final: modifier(node, "final", source) || modifier(node, "sealed", source),
        is_virtual: is_contract
            || is_abstract
            || modifier(node, "virtual", source)
            || modifier(node, "open", source)
            || modifier(node, "override", source)
            || !matches!(language, "cpp" | "csharp" | "kotlin"),
        is_override: modifier(node, "override", source),
        is_pointer_receiver: false,
        has_known_signature,
        conditions: conditions(node, source),
    }
}

fn visible_import(import: &ScopedImport, node: Node<'_>, unit: &ImplementationUnit) -> bool {
    super::super::resolve::encloses(&import.scope, &range(node))
        && !unit.shadows.iter().any(|shadow| {
            shadow.name == import.entry.local_name
                && super::super::resolve::encloses(&shadow.scope, &range(node))
        })
}

pub(super) fn python_abstract(node: Node<'_>, source: &[u8], unit: &ImplementationUnit) -> bool {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .is_some_and(|parent| {
            children(parent).iter().any(|decorator| {
                if decorator.kind() != "decorator" {
                    return false;
                }
                let name = text(*decorator, source).trim().trim_start_matches('@');
                unit.imports.iter().any(|import| {
                    visible_import(import, node, unit)
                        && import.entry.source.as_deref() == Some("abc")
                        && (import.entry.imported_name.as_deref() == Some("abstractmethod")
                            && import.entry.local_name == name
                            || import.entry.kind == crate::parser::ImportKind::Namespace
                                && name == format!("{}.abstractmethod", import.entry.local_name))
                })
            })
        })
}

pub(super) fn detached_method(
    node: Node<'_>,
    symbol: &ExtractedSymbol,
    source: &[u8],
    unit: &mut ImplementationUnit,
) -> bool {
    let language = unit.language.as_str();
    let mut parent = node.parent();
    if language == "swift" {
        while let Some(candidate) = parent {
            if candidate.kind() == "class_declaration" && modifier(candidate, "extension", source) {
                let Some(owner) = candidate
                    .child_by_field_name("name")
                    .and_then(|node| reference(node, source))
                else {
                    return true;
                };
                let contracts = base_nodes(candidate)
                    .into_iter()
                    .filter_map(|node| reference(node, source))
                    .collect();
                let method = method(node, symbol, source, language, false);
                if let Some(block) = unit
                    .implementations
                    .iter_mut()
                    .find(|block| block.owner.range == owner.range)
                {
                    block.methods.push(method);
                } else {
                    unit.implementations.push(ImplementationBlock {
                        owner,
                        contracts,
                        methods: vec![method],
                        namespace: namespace(candidate, &unit.namespace, source),
                        conditions: conditions(candidate, source),
                    });
                }
                return true;
            }
            if is_method(candidate) || is_type(candidate) {
                break;
            }
            parent = candidate.parent();
        }
        return false;
    }
    if language == "rust" {
        while let Some(candidate) = parent {
            if candidate.kind() == "trait_item" {
                return false;
            }
            if candidate.kind() == "impl_item" {
                let Some(owner) = candidate
                    .child_by_field_name("type")
                    .and_then(|node| reference(node, source))
                else {
                    return true;
                };
                let contracts = candidate
                    .child_by_field_name("trait")
                    .and_then(|node| reference(node, source))
                    .into_iter()
                    .collect::<Vec<_>>();
                let method = method(node, symbol, source, language, !contracts.is_empty());
                if let Some(block) = unit
                    .implementations
                    .iter_mut()
                    .find(|block| block.owner.range == owner.range)
                {
                    block.methods.push(method);
                } else {
                    unit.implementations.push(ImplementationBlock {
                        owner,
                        contracts,
                        methods: vec![method],
                        namespace: namespace(candidate, &unit.namespace, source),
                        conditions: conditions(candidate, source),
                    });
                }
                return true;
            }
            if is_method(candidate) || is_type(candidate) {
                return false;
            }
            parent = candidate.parent();
        }
    }
    if language == "go" && node.kind() == "method_declaration" {
        let typ = node
            .child_by_field_name("receiver")
            .and_then(|list| list.named_child(0))
            .and_then(|parameter| parameter.child_by_field_name("type"));
        if let Some(owner) = typ.and_then(|node| reference(node, source)) {
            let mut method = method(node, symbol, source, language, false);
            method.is_pointer_receiver = owner.name.starts_with('*');
            unit.implementations.push(ImplementationBlock {
                owner,
                contracts: Vec::new(),
                methods: vec![method],
                namespace: unit.namespace.clone(),
                conditions: Vec::new(),
            });
        }
        return true;
    }
    false
}

pub(super) fn detached_types(
    root: Node<'_>,
    source: &[u8],
    unit: &mut ImplementationUnit,
    omitted: &mut usize,
) {
    if unit.language != "go" {
        return;
    }
    let mut method_count = unit
        .types
        .iter()
        .map(|typ| typ.methods.len())
        .sum::<usize>()
        + unit
            .implementations
            .iter()
            .map(|block| block.methods.len())
            .sum::<usize>();
    // Go's required interface methods are not ordinary function symbols in every grammar query.
    for typ in &mut unit.types {
        if !typ.is_contract {
            continue;
        }
        let Some(node) = root.named_descendant_for_point_range(
            Point::new(typ.range.start_line - 1, typ.range.start_col - 1),
            Point::new(typ.range.end_line - 1, typ.range.end_col - 1),
        ) else {
            continue;
        };
        let interface = if node.kind() == "interface_type" {
            Some(node)
        } else {
            node.child_by_field_name("type")
                .filter(|node| node.kind() == "interface_type")
        };
        let Some(interface) = interface else { continue };
        for element in children(interface)
            .into_iter()
            .filter(|node| node.kind() == "method_elem")
        {
            let Some(name) = element.child_by_field_name("name") else {
                continue;
            };
            if method_count >= METHODS_PER_FILE {
                *omitted += 1;
                typ.is_complete = false;
                continue;
            }
            let symbol = ExtractedSymbol {
                name: text(name, source).into(),
                kind: "fn".into(),
                range: range(element),
                docstring: None,
                flags: crate::parser::SymbolFlags {
                    has_todo: false,
                    has_fixme: false,
                    is_test: false,
                    is_exported: true,
                    is_deprecated: false,
                },
                owner: Some(typ.name.clone()),
            };
            let method = method(element, &symbol, source, "go", true);
            if !typ
                .methods
                .iter()
                .any(|old| old.range == method.range && old.name == method.name)
            {
                typ.methods.push(method);
                method_count += 1;
            }
        }
    }
}
