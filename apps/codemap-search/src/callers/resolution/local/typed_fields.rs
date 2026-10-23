//! TypeScript `this.<field>.<method>()` through a field the enclosing class declares with a class
//! type: a constructor parameter property or an instance property. The type resolves through the
//! file's named import (relative or tsconfig paths) or a class declared in the same file.

use super::*;

fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).any(|child| child.kind() == kind);
    found
}

/// The class whose instance `this` denotes at `at`. Static members, object-literal methods and
/// regular functions bind another `this`; arrow functions keep the enclosing one.
fn instance_class(at: Node) -> Option<Node> {
    let mut node = at;
    let mut member = None;
    while let Some(parent) = node.parent() {
        node = parent;
        match node.kind() {
            "method_definition" | "public_field_definition" if member.is_none() => {
                if node.parent().map(|parent| parent.kind()) != Some("class_body") {
                    return None;
                }
                member = Some(node);
            }
            "function_declaration"
            | "function_expression"
            | "generator_function"
            | "generator_function_declaration"
                if member.is_none() =>
            {
                return None;
            }
            "class_declaration" | "abstract_class_declaration" | "class" => {
                return member
                    .is_some_and(|member| !has_child_kind(member, "static"))
                    .then_some(node);
            }
            _ => {}
        }
    }
    None
}

/// The plain class type of exactly one instance field declaration named `field`.
fn declared_field_type<'s>(class: Node, field: &str, source: &'s Source) -> Option<&'s str> {
    let body = class.child_by_field_name("body")?;
    let mut annotations = Vec::new();
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        let name = member
            .child_by_field_name("name")
            .and_then(|name| text(name, source));
        match member.kind() {
            "public_field_definition" if name == Some(field) => {
                if !has_child_kind(member, "static") {
                    annotations.push(member.child_by_field_name("type"));
                }
            }
            "method_definition" if name == Some("constructor") => {
                let Some(parameters) = member.child_by_field_name("parameters") else {
                    continue;
                };
                let mut cursor = parameters.walk();
                for parameter in parameters.named_children(&mut cursor) {
                    // Only an accessibility modifier or `readonly` makes a parameter a property.
                    let is_property = has_child_kind(parameter, "accessibility_modifier")
                        || has_child_kind(parameter, "readonly");
                    if matches!(
                        parameter.kind(),
                        "required_parameter" | "optional_parameter"
                    ) && is_property
                        && parameter
                            .child_by_field_name("pattern")
                            .and_then(|pattern| text(pattern, source))
                            == Some(field)
                    {
                        annotations.push(parameter.child_by_field_name("type"));
                    }
                }
            }
            _ => {}
        }
    }
    let [Some(annotation)] = annotations.as_slice() else {
        return None;
    };
    let typ = annotation.named_child(0)?;
    (typ.kind() == "type_identifier")
        .then(|| text(typ, source))
        .flatten()
}

impl<'a> SourceResolver<'a> {
    pub(super) fn typed_field_call(
        &self,
        file: &'a ExtractedFile,
        call: &CallSite,
        source: &Source,
        at: Node,
    ) -> Option<Target<'a>> {
        let field = call.receiver.as_deref()?.strip_prefix("this.")?;
        if field.is_empty()
            || !field
                .chars()
                .all(|character| character.is_alphanumeric() || matches!(character, '_' | '$'))
        {
            return None;
        }
        let class = instance_class(at)?;
        let type_name = declared_field_type(class, field, source)?;
        let (target_file, class_name) = self.class_declaration(file, source, at, type_name)?;
        let target_source = self.source(&target_file.file_path)?;
        unique(
            target_file
                .symbols
                .iter()
                .filter(|symbol| {
                    symbol.kind == "fn"
                        && symbol.name == call.name
                        && symbol.owner.as_deref() == Some(class_name.as_str())
                        && current_symbol(&target_source, symbol)
                })
                .map(|symbol| Target {
                    file: target_file,
                    symbol,
                    is_precise: true,
                })
                .collect(),
        )
    }

    /// The file declaring class `type_name` as seen from `file`: its named import, else a class
    /// of that name declared in `file` itself. Exactly one class declaration must match.
    fn class_declaration(
        &self,
        file: &'a ExtractedFile,
        source: &Source,
        at: Node,
        type_name: &str,
    ) -> Option<(&'a ExtractedFile, String)> {
        let imports: Vec<_> = source
            .extracted
            .navigation
            .as_ref()?
            .imports
            .iter()
            .filter(|import| {
                import.local_name == type_name
                    && node_at(source, &import.range).is_some_and(|node| same_tree(node, at))
            })
            .collect();
        let (target_file, class_name) = match imports.as_slice() {
            [] => (file, type_name.to_string()),
            [import] if import.kind == ImportKind::Named => (
                self.script_module(&file.file_path, import.source.as_deref()?)?,
                import
                    .imported_name
                    .clone()
                    .unwrap_or_else(|| type_name.to_string()),
            ),
            _ => return None,
        };
        let target_source = self.source(&target_file.file_path)?;
        let classes = target_file
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.kind == "class"
                    && symbol.name == class_name
                    && current_symbol(&target_source, symbol)
            })
            .count();
        (classes == 1).then_some((target_file, class_name))
    }
}
