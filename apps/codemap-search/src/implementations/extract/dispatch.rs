use super::*;
use std::collections::HashMap;

/// The file's type declarations and the typed fields each declares, read on first use.
struct Owners<'a, 'tree> {
    nodes: &'a [(Node<'tree>, usize)],
    fields: HashMap<usize, fields::Fields>,
}

impl Owners<'_, '_> {
    fn field_type(
        &mut self,
        owner: Node<'_>,
        name: &str,
        language: &str,
        source: &[u8],
    ) -> Option<TypeReference> {
        self.fields
            .entry(owner.id())
            .or_insert_with(|| fields::Fields::of(owner, language, source))
            .get(name)
    }
}

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

/// Languages whose bare calls and names reach members of the enclosing type.
fn has_implicit_members(language: &str) -> bool {
    matches!(
        language,
        "java" | "csharp" | "kotlin" | "scala" | "groovy" | "swift" | "cpp" | "ruby" | "dart"
    )
}

/// The field named by a receiver such as `this.f`, `self.f`, `$this->f` or `@f`.
fn self_field<'a>(receiver: &'a str, language: &str) -> Option<&'a str> {
    let field = match language {
        "php" => receiver.strip_prefix("$this->"),
        "cpp" => receiver.strip_prefix("this->"),
        "python" | "swift" => receiver.strip_prefix("self."),
        "ruby" => receiver.strip_prefix('@'),
        "powershell" => receiver.strip_prefix("$this."),
        "rust" | "go" => None,
        _ => receiver.strip_prefix("this."),
    }?;
    (!field.is_empty() && field.chars().all(|c| c.is_alphanumeric() || c == '_')).then_some(field)
}

fn is_base_receiver(receiver: Option<&str>, language: &str) -> bool {
    matches!(
        (language, receiver),
        (
            "java" | "kotlin" | "scala" | "groovy" | "swift" | "dart" | "typescript" | "javascript",
            Some("super")
        ) | ("python", Some("super()"))
            | ("php", Some("parent"))
            | ("csharp", Some("base"))
    )
}

/// The type declaration whose instance `node` runs in: methods nested in methods and
/// JavaScript functions bind another receiver.
fn enclosing_owner<'a>(
    node: Node<'a>,
    owners: &[(Node<'a>, usize)],
    unit: &ImplementationUnit,
) -> Option<(Node<'a>, usize)> {
    let mut current = node.parent();
    let mut crossed_method = false;
    while let Some(parent) = current {
        if let Some(owner) = owners.iter().find(|(owner, _)| owner.id() == parent.id()) {
            return Some(*owner);
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
    None
}

fn at_call(mut typ: TypeReference, node: Node<'_>) -> TypeReference {
    typ.range = range(node);
    typ
}

/// The receiver type of a call and how the receiver relates to it.
fn receiver_type(
    tree: &Tree,
    source: &[u8],
    navigation: &NavigationFile,
    call: &crate::parser::CallSite,
    node: Node<'_>,
    owners: &mut Owners<'_, '_>,
    unit: &ImplementationUnit,
) -> Option<(TypeReference, ReceiverKind)> {
    let language = unit.language.as_str();
    let receiver = call.receiver.as_deref();
    let is_self = matches!(receiver, Some("this" | "self" | "$this"))
        || language == "python" && receiver == Some("cls");
    let is_implicit = receiver.is_none() && has_implicit_members(language);
    if language == "rust" && (is_self || is_implicit) {
        let mut current = node.parent();
        let mut crossed_method = false;
        while let Some(parent) = current {
            if let Some((_, index)) = owners
                .nodes
                .iter()
                .find(|(owner, _)| owner.id() == parent.id())
            {
                let owner = &unit.types[*index];
                return Some((
                    TypeReference {
                        name: owner.qualified_name.clone(),
                        arguments: Vec::new(),
                        range: range(node),
                    },
                    ReceiverKind::Value,
                ));
            }
            if parent.kind() == "impl_item" {
                return parent
                    .child_by_field_name("type")
                    .and_then(|typ| reference(typ, source))
                    .map(|typ| (typ, ReceiverKind::Value));
            }
            if is_method(parent) {
                if crossed_method {
                    return None;
                }
                crossed_method = true;
            }
            current = parent.parent();
        }
        return None;
    }
    let is_base = is_base_receiver(receiver, language);
    let field = receiver.and_then(|receiver| self_field(receiver, language));
    if is_self || is_implicit || is_base || field.is_some() {
        let (owner_node, index) = enclosing_owner(node, owners.nodes, unit)?;
        let owner = &unit.types[index];
        if let Some(field) = field {
            return owners
                .field_type(owner_node, field, language, source)
                .map(|typ| (at_call(typ, node), ReceiverKind::Value));
        }
        if is_base {
            // Several bases leave `super` to a resolution order this proof does not model.
            let [base] = owner.bases.as_slice() else {
                return None;
            };
            return Some((at_call(base.clone(), node), ReceiverKind::Base));
        }
        return Some((
            TypeReference {
                name: owner.qualified_name.clone(),
                arguments: Vec::new(),
                range: range(node),
            },
            ReceiverKind::Value,
        ));
    }
    let receiver = receiver?;
    if !receiver
        .chars()
        .all(|c| c.is_alphanumeric() || "_$".contains(c))
    {
        return None;
    }
    if let Some(found) = declared_receiver_type(tree, source, navigation, receiver, node) {
        return found.map(|typ| (typ, ReceiverKind::Value));
    }
    // An undeclared name in a type with implicit members may be one of its fields.
    if has_implicit_members(language) {
        if let Some((owner_node, _)) = enclosing_owner(node, owners.nodes, unit) {
            if let Some(typ) = owners.field_type(owner_node, receiver, language, source) {
                return Some((at_call(typ, node), ReceiverKind::Value));
            }
        }
    }
    // A capitalized receiver that no binding declares names a type and its static members.
    // Rust and Go resolve `Type::f` and package members through their module resolver.
    if !matches!(language, "go" | "rust")
        && receiver
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
    {
        return Some((
            TypeReference {
                name: receiver.to_string(),
                arguments: Vec::new(),
                range: range(node),
            },
            ReceiverKind::Type,
        ));
    }
    None
}

/// The declared type of the nearest local binding or parameter named `receiver`:
/// `Some(None)` when that declaration has no written type, `None` when nothing declares it.
fn declared_receiver_type(
    tree: &Tree,
    source: &[u8],
    navigation: &NavigationFile,
    receiver: &str,
    node: Node<'_>,
) -> Option<Option<TypeReference>> {
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
                    typ.named_child(0)
                } else {
                    Some(typ)
                };
                return Some(typ.and_then(|typ| reference(typ, source)));
            }
        }
        // An untyped nearest binding shadows a typed outer binding.
        return Some(None);
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
                    return Some(declarations::parameter_type(parameter).and_then(|typ| {
                        let typ = if typ.kind() == "type_annotation" {
                            typ.named_child(0)?
                        } else {
                            typ
                        };
                        reference(typ, source)
                    }));
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
    let mut owners = Owners {
        nodes: owners,
        fields: HashMap::new(),
    };
    for call in &navigation.calls {
        if unit.calls.len() >= CALLS_PER_FILE {
            *omitted += 1;
            continue;
        }
        let Some(node) = at(tree, &call.range) else {
            continue;
        };
        let Some((receiver, receiver_kind)) =
            receiver_type(tree, source, navigation, call, node, &mut owners, unit)
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
            receiver_kind,
        });
    }
}
