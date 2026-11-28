use super::*;

pub(super) fn extract_type_parameters(node: Node<'_>, source: &[u8]) -> Vec<String> {
    parameters(node, source)
}

pub(super) fn base_references(node: Node<'_>, source: &[u8]) -> Vec<TypeReference> {
    base_nodes(node)
        .into_iter()
        .filter_map(|candidate| reference(candidate, source))
        .collect()
}

fn parameters(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let list = node
        .child_by_field_name("type_parameters")
        .or_else(|| child(node, &["type_parameters", "type_parameter_list"]))
        .or_else(|| {
            node.parent()
                .filter(|parent| parent.kind() == "template_declaration")
                .and_then(|parent| {
                    parent
                        .child_by_field_name("parameters")
                        .or_else(|| child(parent, &["template_parameter_list"]))
                })
        });
    list.map(|list| {
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
                    | "class_interface_clause"
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
                | "class_interface_clause"
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

/// Whether a type declaration's header (modifiers, name, type parameters, base list — everything
/// outside its body) parsed without syntax errors.
fn header_intact(node: Node<'_>) -> bool {
    if node.is_error() || node.is_missing() {
        return false;
    }
    let body_id = body(node).map(|b| b.id());
    let mut cursor = node.walk();
    let intact = node
        .children(&mut cursor)
        .all(|child| body_id == Some(child.id()) || (!child.has_error() && !child.is_missing()));
    intact
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
        is_default_export: matches!(language, "typescript" | "javascript")
            && node.parent().is_some_and(|parent| {
                parent.kind() == "export_statement" && {
                    let mut cursor = parent.walk();
                    let found = parent
                        .children(&mut cursor)
                        .any(|child| !child.is_named() && child.kind() == "default");
                    found
                }
            }),
        is_abstract: is_contract
            || modifier(node, "abstract", source)
            || node.kind() == "abstract_class_declaration",
        is_final: modifier(node, "final", source) || modifier(node, "sealed", source),
        is_complete: if language == "go" {
            !node.has_error()
        } else {
            header_intact(node)
        },
        has_body_error: language != "go" && body(node).is_some_and(|b| b.has_error()),
        is_partial: modifier(node, "partial", source),
        is_anonymous: false,
        fields: Vec::new(),
        conditions: conditions_for(language, node, source),
    };
    let mut candidates = base_nodes(node);
    // C# headers with inline `#if` directives inside the base list (e.g. Newtonsoft.Json
    // `JContainer : JToken, IList<JToken>, IList #if HAVE_COMPONENT_MODEL , ITypedList #endif`)
    // split or error the tree-sitter `base_list`. Parse unconditional vs `#if`-guarded bases
    // directly from the header slice between `:` and `{`, preserving per-base conditions.
    if language == "csharp"
        && csharp_header_has_preproc(node, source)
        && collect_csharp_conditional_bases(node, source, &mut record)
    {
        return record;
    }
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
        // Dart writes the arguments of a base as a node of their own, right after its name.
        let written = range(candidate);
        if let Some(base) = record.bases.last_mut().filter(|base| {
            base.arguments.is_empty()
                && (base.range.end_line, base.range.end_col)
                    == (written.start_line, written.start_col)
        }) {
            if let Some(arguments) = type_arguments(&compact(text(candidate, source))) {
                base.arguments = arguments;
                continue;
            }
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
                        && import.entry.imported_name.as_deref() == Some("Generic")
                        && import.entry.local_name == base.name
                })
            {
                for arg in &base.arguments {
                    if arg.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && !record.parameters.contains(arg)
                    {
                        record.parameters.push(arg.clone());
                    }
                }
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
                for arg in &base.arguments {
                    if arg.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && !record.parameters.contains(arg)
                    {
                        record.parameters.push(arg.clone());
                    }
                }
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
            // Embedded interfaces enter `record.bases` so explicit assertions and complete
            // embedded hierarchies resolve cleanly; union/tilde type-set constraints stay
            // incomplete so no partial method-set guess is ever made.
            if record.is_contract {
                for element in children(typ) {
                    match element.kind() {
                        "method_elem" | "comment" => {}
                        "type_elem" => {
                            let inner = children(element);
                            if inner.len() == 1
                                && matches!(inner[0].kind(), "type_identifier" | "qualified_type")
                            {
                                if let Some(base) = reference(inner[0], source) {
                                    if !record.bases.iter().any(|old| {
                                        old.name == base.name && old.arguments == base.arguments
                                    }) {
                                        record.bases.push(base);
                                    }
                                    continue;
                                }
                            }
                            record.is_complete = false;
                        }
                        "type_identifier" | "qualified_type" => {
                            if let Some(base) = reference(element, source) {
                                if !record.bases.iter().any(|old| {
                                    old.name == base.name && old.arguments == base.arguments
                                }) {
                                    record.bases.push(base);
                                }
                            } else {
                                record.is_complete = false;
                            }
                        }
                        _ => record.is_complete = false,
                    }
                }
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
                "class_method_parameter_list",
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
                    "comment" | "documentation_comment" | "type_parameters" | "type_parameter_list"
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
    // A bracketed PowerShell type literal names the type in its `type_spec`.
    let typ = declared_parameter_type(node)?;
    Some(if typ.kind() == "type_literal" {
        child(typ, &["type_spec"]).unwrap_or(typ)
    } else {
        typ
    })
}

fn declared_parameter_type<'a>(node: Node<'a>) -> Option<Node<'a>> {
    node.child_by_field_name("type")
        .or_else(|| {
            child(
                node,
                &[
                    "type_annotation",
                    "type",
                    "user_type",
                    "nullable_type",
                    "non_nullable_type",
                    "function_type",
                    "parenthesized_type",
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
                    // A Kotlin property names its type beside its name.
                    "variable_declaration",
                ],
            )
            .and_then(declared_parameter_type)
        })
}

pub(super) fn parameter_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let value = if matches!(
        node.kind(),
        "identifier" | "simple_identifier" | "variable_name" | "variable"
    ) {
        node
    } else {
        node.child_by_field_name("name")
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
            })
            .or_else(|| declarator_name(node))?
    };
    let name = compact(text(value, source));
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || "_$".contains(c)))
    .then_some(name)
}

/// The identifier inside C/C++ reference, pointer and array declarators.
fn declarator_name(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.child_by_field_name("declarator")?;
    loop {
        if current.kind() == "identifier" {
            return Some(current);
        }
        current = current
            .child_by_field_name("declarator")
            .or_else(|| child(current, &["identifier"]))?;
    }
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
        if matches!(
            parameter.kind(),
            "comment" | "documentation_comment" | "/" | "*"
        ) {
            continue;
        }
        let value = parameter_text(parameter, source, language);
        has_known_signature &= value.is_some();
        let mut value = value.unwrap_or_else(|| "?".into());
        if language == "go" && parameter.kind() == "variadic_parameter_declaration" {
            value = format!("...{value}");
        }
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
    let type_parameters = parameters(signature, source);
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
    // The modifiers of a method can stand on its signature rather than on the declaration.
    let is_static = modifier(node, "static", source)
        || node
            .child_by_field_name("signature")
            .is_some_and(|signature| modifier(signature, "static", source))
        || language == "rust" && !has_self;
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
        type_parameters,
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
        is_package_scoped: language == "java"
            && !is_contract
            && !["public", "protected", "private"]
                .iter()
                .any(|access| modifier(node, access, source))
            || language == "dart" && symbol.name.starts_with('_'),
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

fn swift_has_where_clause(node: Node<'_>, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).any(|child| {
        child.kind() == "type_constraints" || (!child.is_named() && text(child, source) == "where")
    });
    found
}

pub(super) fn detached_method(
    node: Node<'_>,
    symbol: &ExtractedSymbol,
    source: &[u8],
    unit: &mut ImplementationUnit,
) -> bool {
    let language = unit.language.as_str();
    if language == "swift" {
        for candidate in crate::parser::ancestors(node) {
            if candidate.kind() == "class_declaration" && modifier(candidate, "extension", source) {
                let Some(owner) = candidate
                    .child_by_field_name("name")
                    .and_then(|node| reference(node, source))
                else {
                    return true;
                };
                let contracts = if swift_has_where_clause(candidate, source) {
                    Vec::new()
                } else {
                    base_nodes(candidate)
                        .into_iter()
                        .filter_map(|node| reference(node, source))
                        .collect()
                };
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
                        range: Some(range(candidate)),
                    });
                }
                return true;
            }
            if is_method(candidate) || is_type(candidate) {
                break;
            }
        }
        return false;
    }
    if language == "rust" {
        for candidate in crate::parser::ancestors(node) {
            if candidate.kind() == "trait_item" {
                return false;
            }
            if candidate.kind() == "impl_item" {
                let contracts = candidate
                    .child_by_field_name("trait")
                    .and_then(|node| reference(node, source))
                    .into_iter()
                    .collect::<Vec<_>>();
                let owner_node = candidate.child_by_field_name("type");
                let Some(owner) = owner_node.and_then(|node| {
                    if contracts.is_empty() {
                        reference(node, source)
                    } else {
                        raw_owner_reference(node, source)
                    }
                }) else {
                    return true;
                };
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
                        range: Some(range(candidate)),
                    });
                }
                return true;
            }
            if is_method(candidate) || is_type(candidate) {
                return false;
            }
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
                range: Some(range(node)),
            });
        }
        return true;
    }
    false
}

/// Extract concrete owner type from a Go compile-time interface assertion value:
/// `(*Impl)(nil)`, `&Impl{}`, `Impl{}`, or `(Impl)(...)`.
fn go_assertion_owner(mut expr: Node<'_>, source: &[u8]) -> Option<TypeReference> {
    while expr.kind() == "parenthesized_expression" {
        expr = children(expr).into_iter().next()?;
    }
    match expr.kind() {
        "composite_literal" => {
            let typ = expr.child_by_field_name("type")?;
            reference(typ, source)
        }
        "unary_expression" => {
            let op = expr.child_by_field_name("operator")?;
            if text(op, source) != "&" {
                return None;
            }
            let operand = expr.child_by_field_name("operand")?;
            if operand.kind() != "composite_literal" {
                return None;
            }
            let typ = operand.child_by_field_name("type")?;
            let mut owner = reference(typ, source)?;
            if !owner.name.starts_with('*') {
                owner.name = format!("*{}", owner.name);
            }
            Some(owner)
        }
        "type_conversion_expression" => {
            let operand = expr.child_by_field_name("operand")?;
            if text(operand, source).trim() != "nil" {
                return None;
            }
            let mut typ = expr.child_by_field_name("type")?;
            while matches!(
                typ.kind(),
                "parenthesized_type" | "parenthesized_expression"
            ) {
                typ = children(typ).into_iter().next()?;
            }
            if matches!(
                typ.kind(),
                "pointer_type" | "type_identifier" | "qualified_type"
            ) {
                reference(typ, source)
            } else {
                None
            }
        }
        "call_expression" => {
            let args = expr.child_by_field_name("arguments")?;
            let arg_nodes = children(args);
            if arg_nodes.len() != 1 || text(arg_nodes[0], source) != "nil" {
                return None;
            }
            let mut func = expr.child_by_field_name("function")?;
            while matches!(
                func.kind(),
                "parenthesized_type" | "parenthesized_expression"
            ) {
                func = children(func).into_iter().next()?;
            }
            // tree-sitter-go parses `(*Impl)(nil)` as a `call_expression` whose function is a
            // dereference `unary_expression` (`*Impl`), or as a `pointer_type`.
            if func.kind() == "unary_expression" {
                let op = func.child_by_field_name("operator")?;
                if text(op, source) != "*" {
                    return None;
                }
                let operand = func.child_by_field_name("operand")?;
                let mut owner = reference(operand, source)?;
                if !owner.name.starts_with('*') {
                    owner.name = format!("*{}", owner.name);
                }
                return Some(owner);
            }
            if matches!(
                func.kind(),
                "pointer_type"
                    | "type_identifier"
                    | "qualified_type"
                    | "identifier"
                    | "selector_expression"
            ) {
                reference(func, source)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn collect_go_assertions(root: Node<'_>, source: &[u8], unit: &mut ImplementationUnit) {
    let mut specs = Vec::new();
    for node in children(root) {
        if node.kind() != "var_declaration" {
            continue;
        }
        for child in children(node) {
            if child.kind() == "var_spec" {
                specs.push(child);
            } else if child.kind() == "var_spec_list" {
                specs.extend(
                    children(child)
                        .into_iter()
                        .filter(|n| n.kind() == "var_spec"),
                );
            }
        }
    }
    for spec in specs {
        let mut cursor = spec.walk();
        let names: Vec<_> = spec.children_by_field_name("name", &mut cursor).collect();
        if names.len() != 1 || text(names[0], source) != "_" {
            continue;
        }
        let Some(iface) = spec
            .child_by_field_name("type")
            .and_then(|node| reference(node, source))
        else {
            continue;
        };
        let Some(value) = spec.child_by_field_name("value") else {
            continue;
        };
        let exprs = if value.kind() == "expression_list" {
            children(value)
        } else {
            vec![value]
        };
        if exprs.len() != 1 {
            continue;
        }
        let Some(owner) = go_assertion_owner(exprs[0], source) else {
            continue;
        };
        unit.implementations.push(ImplementationBlock {
            owner,
            contracts: vec![iface],
            methods: Vec::new(),
            namespace: unit.namespace.clone(),
            conditions: conditions(spec, source),
            range: Some(range(spec)),
        });
    }
}

fn collect_swift_conformance_extensions(
    root: Node<'_>,
    source: &[u8],
    unit: &mut ImplementationUnit,
) {
    for node in children(root) {
        if node.kind() != "class_declaration"
            || !modifier(node, "extension", source)
            || swift_has_where_clause(node, source)
        {
            continue;
        }
        let Some(owner) = node
            .child_by_field_name("name")
            .and_then(|node| reference(node, source))
        else {
            continue;
        };
        let contracts: Vec<_> = base_nodes(node)
            .into_iter()
            .filter_map(|node| reference(node, source))
            .collect();
        if contracts.is_empty()
            || unit
                .implementations
                .iter()
                .any(|block| block.owner.range == owner.range)
        {
            continue;
        }
        unit.implementations.push(ImplementationBlock {
            owner,
            contracts,
            methods: Vec::new(),
            namespace: namespace(node, &unit.namespace, source),
            conditions: conditions(node, source),
            range: Some(range(node)),
        });
    }
}

/// Records Rust trait `impl` blocks that had no extracted methods (e.g. marker traits or
/// default-method-only impls such as `impl SanitizeErr for &str {}`) so `explicit impl block`
/// discovery covers every explicit trait impl in the file.
fn collect_rust_trait_impl_blocks(root: Node<'_>, source: &[u8], unit: &mut ImplementationUnit) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "impl_item" {
            let contracts: Vec<TypeReference> = node
                .child_by_field_name("trait")
                .and_then(|t| reference(t, source))
                .into_iter()
                .collect();
            if !contracts.is_empty() {
                if let Some(owner) = node
                    .child_by_field_name("type")
                    .and_then(|t| raw_owner_reference(t, source))
                {
                    if !unit
                        .implementations
                        .iter()
                        .any(|block| block.owner.range == owner.range)
                    {
                        unit.implementations.push(ImplementationBlock {
                            owner,
                            contracts,
                            methods: Vec::new(),
                            namespace: namespace(node, &unit.namespace, source),
                            conditions: conditions(node, source),
                            range: Some(range(node)),
                        });
                    }
                }
            }
            continue;
        }
        let mut cursor = node.walk();
        for c in node.named_children(&mut cursor) {
            if matches!(c.kind(), "impl_item" | "mod_item" | "declaration_list") {
                stack.push(c);
            }
        }
    }
}

/// Whether a C# type header (between the start of the declaration and its opening `{`) contains
/// preprocessor `#if` directives affecting its base list.
fn csharp_header_has_preproc(node: Node<'_>, source: &[u8]) -> bool {
    let start = node.start_byte();
    let end = body(node)
        .map(|b| b.start_byte())
        .unwrap_or(node.end_byte());
    source
        .get(start..end)
        .and_then(|s| std::str::from_utf8(s).ok())
        .is_some_and(|hdr| hdr.contains(':') && hdr.contains("#if"))
}

/// Parses a C# type header containing `#if` / `#elif` / `#else` / `#endif` inside its base list,
/// keeping unconditional bases condition-free and tagging each conditional base with its exact
/// preprocessor condition stack (`#if SYM`).
pub(super) fn collect_csharp_conditional_bases(
    node: Node<'_>,
    source: &[u8],
    record: &mut TypeDeclaration,
) -> bool {
    let start = node.start_byte();
    let end = body(node)
        .map(|b| b.start_byte())
        .unwrap_or(node.end_byte());
    let Ok(header) = std::str::from_utf8(source.get(start..end).unwrap_or(&[])) else {
        return false;
    };
    let Some(colon_idx) = header.find(':') else {
        return false;
    };
    let base_slice = &header[colon_idx + 1..];
    let base_start_line = node.start_position().row
        + 1
        + header[..colon_idx + 1]
            .bytes()
            .filter(|&b| b == b'\n')
            .count();
    let mut cond_stack: Vec<String> = Vec::new();
    let mut parsed_any = false;
    for (line_offset, raw_line) in base_slice.lines().enumerate() {
        let line_no = base_start_line + line_offset;
        let trimmed = raw_line.split("//").next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(expr) = trimmed.strip_prefix("#if") {
            let expr = expr.trim();
            cond_stack.push(format!("#if {expr}"));
            continue;
        }
        if let Some(expr) = trimmed.strip_prefix("#elif") {
            cond_stack.pop();
            let expr = expr.trim();
            cond_stack.push(format!("#if {expr}"));
            continue;
        }
        if trimmed.starts_with("#else") {
            if let Some(prev) = cond_stack.pop() {
                let inner = prev.strip_prefix("#if ").unwrap_or(&prev);
                cond_stack.push(format!("#if !({inner})"));
            }
            continue;
        }
        if trimmed.starts_with("#endif") {
            cond_stack.pop();
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with("where ") {
            continue;
        }
        // Split top-level commas respecting `<...>`.
        let mut depth = 0usize;
        let mut piece_start = 0usize;
        let bytes = trimmed.as_bytes();
        let mut pieces = Vec::new();
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'<' => depth += 1,
                b'>' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    pieces.push(trimmed[piece_start..i].trim());
                    piece_start = i + 1;
                }
                _ => {}
            }
        }
        pieces.push(trimmed[piece_start..].trim());
        for piece in pieces {
            let token = compact(piece);
            if token.is_empty() {
                continue;
            }
            let (name, arguments) = if let Some(lt) = token.find('<') {
                let Some(args) = type_arguments(&token[lt..]) else {
                    continue;
                };
                (token[..lt].to_string(), args)
            } else {
                (token, Vec::new())
            };
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || "_.$".contains(c))
            {
                continue;
            }
            if !record
                .bases
                .iter()
                .any(|old| old.name == name && old.arguments == arguments)
            {
                record.bases.push(TypeReference {
                    name,
                    arguments,
                    range: CodeRange {
                        start_line: line_no,
                        start_col: 1,
                        end_line: line_no,
                        end_col: raw_line.len().max(1),
                    },
                    conditions: cond_stack.clone(),
                });
                parsed_any = true;
            }
        }
    }
    if parsed_any && body(node).is_some() {
        // Unconditional bases are complete even though `#if` inside the base list caused a
        // header parse error in tree-sitter-c-sharp.
        record.is_complete = true;
    }
    parsed_any
}

pub(super) fn detached_types(
    root: Node<'_>,
    source: &[u8],
    unit: &mut ImplementationUnit,
    omitted: &mut usize,
) {
    if unit.language == "swift" {
        collect_swift_conformance_extensions(root, source, unit);
        return;
    }
    if unit.language == "rust" {
        collect_rust_trait_impl_blocks(root, source, unit);
        return;
    }
    if unit.language != "go" {
        return;
    }
    collect_go_assertions(root, source, unit);
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
