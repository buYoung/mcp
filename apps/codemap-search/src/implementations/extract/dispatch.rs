use super::*;

fn count_arguments(node: Node<'_>) -> Option<usize> {
    node.child_by_field_name("arguments")
        .or_else(|| child(node, &["argument_list", "arguments", "value_arguments"]))
        .map(|list| {
            children(list)
                .into_iter()
                .filter(|node| !matches!(node.kind(), "comment"))
                .count()
        })
}

fn receiver_type(
    tree: &Tree,
    source: &[u8],
    navigation: &NavigationFile,
    call: &crate::parser::CallSite,
    node: Node<'_>,
    owners: &[(Node<'_>, usize)],
    unit: &ImplementationUnit,
) -> Option<TypeReference> {
    let receiver = call.receiver.as_deref();
    let is_self = matches!(receiver, Some("this" | "self" | "$this"));
    let is_implicit = receiver.is_none()
        && matches!(
            unit.language.as_str(),
            "java" | "csharp" | "kotlin" | "scala" | "groovy" | "swift" | "cpp" | "ruby"
        );
    if is_self || is_implicit {
        let mut current = node.parent();
        let mut crossed_method = false;
        while let Some(parent) = current {
            if let Some((_, index)) = owners.iter().find(|(owner, _)| owner.id() == parent.id()) {
                let owner = &unit.types[*index];
                return Some(TypeReference {
                    name: owner.qualified_name.clone(),
                    arguments: Vec::new(),
                    range: range(node),
                });
            }
            if parent.kind() == "impl_item" && unit.language == "rust" {
                return parent
                    .child_by_field_name("type")
                    .and_then(|typ| reference(typ, source));
            }
            if is_method(parent) {
                if crossed_method {
                    return None;
                }
                crossed_method = true;
            }
            if matches!(
                parent.kind(),
                "function_expression" | "function_declaration"
            ) && matches!(unit.language.as_str(), "typescript" | "javascript")
            {
                return None;
            }
            current = parent.parent();
        }
        return None;
    }
    let receiver = receiver?;
    if !receiver
        .chars()
        .all(|c| c.is_alphanumeric() || "_$".contains(c))
    {
        return None;
    }
    let mut candidates = Vec::new();
    for binding in navigation
        .local_bindings
        .iter()
        .filter(|binding| binding.name == receiver)
    {
        let Some(binding_node) = at(tree, &binding.range) else {
            continue;
        };
        let scope = imports::scope(binding_node);
        if !contains(scope, node) || binding_node.start_byte() > node.start_byte() {
            continue;
        }
        candidates.push((
            scope.end_byte() - scope.start_byte(),
            std::cmp::Reverse(binding_node.start_byte()),
            binding_node,
            binding.type_name.as_deref(),
        ));
    }
    candidates.sort_by_key(|(size, position, _, _)| (*size, *position));
    if let Some((_, _, binding, typ)) = candidates.first() {
        if typ.is_some() {
            if let Some(typ) = declarations::parameter_type(*binding) {
                let typ = if typ.kind() == "type_annotation" {
                    typ.named_child(0)?
                } else {
                    typ
                };
                return reference(typ, source);
            }
        }
        // An untyped nearest binding shadows a typed outer binding.
        return None;
    }
    let mut current = node.parent();
    while let Some(parent) = current {
        if is_method(parent) || matches!(parent.kind(), "arrow_function" | "lambda_expression") {
            let mut parameters = declarations::parameter_nodes(parent);
            parameters.extend(
                parent
                    .child_by_field_name("receiver")
                    .map(children)
                    .unwrap_or_default(),
            );
            for parameter in parameters {
                if declarations::parameter_name(parameter, source).as_deref() == Some(receiver) {
                    return declarations::parameter_type(parameter).and_then(|typ| {
                        let typ = if typ.kind() == "type_annotation" {
                            typ.named_child(0)?
                        } else {
                            typ
                        };
                        reference(typ, source)
                    });
                }
            }
        }
        current = parent.parent();
    }
    None
}

pub(super) fn collect(
    tree: &Tree,
    source: &[u8],
    symbols: &[ExtractedSymbol],
    navigation: &NavigationFile,
    owners: &[(Node<'_>, usize)],
    unit: &mut ImplementationUnit,
    omitted: &mut usize,
) {
    for call in &navigation.calls {
        if unit.calls.len() >= CALLS_PER_FILE {
            *omitted += 1;
            continue;
        }
        let Some(node) = at(tree, &call.range) else {
            continue;
        };
        let Some(receiver) = receiver_type(tree, source, navigation, call, node, owners, unit)
        else {
            continue;
        };
        let enclosing = symbols
            .iter()
            .filter(|symbol| {
                symbol.kind == "fn"
                    && symbol.range.start_line <= call.range.start_line
                    && call.range.end_line <= symbol.range.end_line
            })
            .min_by_key(|symbol| symbol.range.end_line - symbol.range.start_line)
            .map(|symbol| symbol.name.clone());
        unit.calls.push(DispatchCall {
            name: call.name.clone(),
            range: call.range.clone(),
            receiver,
            namespace: namespace(node, &unit.namespace, source),
            enclosing_symbol: enclosing,
            argument_count: count_arguments(node),
            conditions: conditions(node, source),
        });
    }
}
