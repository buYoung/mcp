//! Syntax augments the textual fallback only where a binding/type is understood.
use super::{rules, Detection};
use std::ops::Range;
use std::path::Path;
use tree_sitter::{Node, Parser};

#[derive(Default)]
pub(super) struct Context {
    pub decided: Vec<Range<usize>>,
    pub detections: Vec<Detection>,
    pub literals: Vec<(Range<usize>, usize)>,
}

pub(super) fn parse(path: &Path, source: &str) -> Option<Context> {
    let spec = crate::lang::spec_for_path(path)?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut parser = Parser::new();
    parser.set_language(&spec.grammar(&ext)).ok()?;
    // Reuse the project's existing parse deadline and cancellation behavior.
    let tree = crate::parser::parse_source(&mut parser, source.as_bytes()).ok()?;
    Some(inspect(tree.root_node(), source))
}

fn is_type(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "type_annotation"
            | "type_alias_declaration"
            | "type_alias_statement"
            | "type_parameters"
            | "type_arguments"
            | "type_parameter"
            | "type_identifier"
            | "type_descriptor"
            | "type_spec"
            | "type_definition"
            | "interface_declaration"
    )
}

fn binding(node: Node<'_>) -> Option<(Node<'_>, Option<Node<'_>>)> {
    match node.kind() {
        "pair" | "block_mapping_pair" | "flow_pair" | "attribute" => Some((
            node.child_by_field_name("key")
                .or_else(|| node.child_by_field_name("name"))?,
            node.child_by_field_name("value"),
        )),
        "assignment"
        | "assignment_expression"
        | "augmented_assignment"
        | "named_expression"
        | "short_var_declaration" => Some((
            node.child_by_field_name("left")?,
            node.child_by_field_name("right"),
        )),
        "let_declaration"
        | "const_item"
        | "static_item"
        | "variable_declarator"
        | "variable_assignment"
        | "var_spec"
        | "const_spec"
        | "keyword_argument"
        | "default_parameter"
        | "typed_default_parameter"
        | "optional_parameter"
        | "required_parameter"
        | "parameter_declaration"
        | "property_signature"
        | "public_field_definition"
        | "field_declaration"
        | "property_declaration"
        | "val_definition"
        | "parameter" => {
            let name = node
                .child_by_field_name("name")
                .or_else(|| node.child_by_field_name("pattern"))
                .or_else(|| node.child_by_field_name("declarator"))?;
            Some((
                name,
                node.child_by_field_name("value")
                    .or_else(|| node.child_by_field_name("default_value")),
            ))
        }
        "init_declarator" => Some((
            node.child_by_field_name("declarator")?,
            node.child_by_field_name("value"),
        )),
        _ => None,
    }
}

fn string_body(node: Node<'_>, source: &str) -> Range<usize> {
    let raw = &source[node.byte_range()];
    if node.kind() == "block_scalar" {
        return raw
            .find('\n')
            .map_or(node.end_byte(), |at| node.start_byte() + at + 1)
            ..node.end_byte();
    }
    let stripped = crate::lang::strip_quotes(raw);
    if let Some(offset) = raw.find(&stripped) {
        node.start_byte() + offset..node.start_byte() + offset + stripped.len()
    } else {
        node.byte_range()
    }
}

fn is_string(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "string"
            | "string_literal"
            | "raw_string_literal"
            | "raw_string"
            | "interpreted_string_literal"
            | "template_string"
            | "verbatim_string_literal"
            | "character_literal"
            | "char_literal"
            | "single_quote_scalar"
            | "double_quote_scalar"
            | "string_scalar"
            | "block_scalar"
            | "bare_string"
            | "word"
    )
}

fn values(node: Node<'_>, source: &str, detections: &mut Vec<Detection>) -> bool {
    if is_string(node) {
        let body = string_body(node, source);
        // Preserve interpolation expressions, masking only their literal surroundings.
        let mut cursor = node.walk();
        let mut start = body.start;
        for child in node.named_children(&mut cursor).filter(|child| {
            matches!(
                child.kind(),
                "interpolation"
                    | "template_substitution"
                    | "string_interpolation"
                    | "expansion"
                    | "simple_expansion"
            )
        }) {
            if start < child.start_byte() {
                detections.push(Detection::field(start..child.start_byte()));
            }
            start = child.end_byte();
        }
        if start < body.end {
            detections.push(Detection::field(start..body.end));
        }
        return true;
    }
    if matches!(
        node.kind(),
        "integer" | "float" | "number" | "integer_literal" | "float_literal"
    ) {
        detections.push(Detection::field(node.byte_range()));
        return true;
    }
    // Do not infer data flow through calls, references, arrays or object containers.
    // Named members of a container are inspected separately by the main traversal.
    if matches!(
        node.kind(),
        "parenthesized_expression"
            | "binary_expression"
            | "binary_operator"
            | "concatenated_string"
            | "concatenation"
            | "expression_list"
            | "flow_node"
            | "block_node"
            | "plain_scalar"
            | "as_expression"
            | "type_assertion"
            | "expression"
    ) {
        let mut cursor = node.walk();
        let mut is_understood = true;
        for child in node.named_children(&mut cursor) {
            if !is_type(child) {
                is_understood &= values(child, source, detections);
            }
        }
        return is_understood;
    }
    matches!(
        node.kind(),
        "identifier"
            | "constant"
            | "scoped_identifier"
            | "qualified_identifier"
            | "member_expression"
            | "field_expression"
            | "attribute"
            | "subscript_expression"
            | "subscript"
            | "call"
            | "call_expression"
            | "invocation_expression"
            | "method_invocation"
            | "object"
            | "object_creation_expression"
            | "array"
            | "array_expression"
            | "list"
            | "dictionary"
            | "flow_mapping"
            | "block_mapping"
            | "flow_sequence"
            | "block_sequence"
            | "true"
            | "false"
            | "boolean"
            | "boolean_scalar"
            | "null"
            | "null_scalar"
            | "none"
            | "nil"
            | "alias"
            | "expansion"
            | "simple_expansion"
    )
}

fn is_sensitive_name(mut node: Node<'_>, source: &str) -> bool {
    if matches!(
        node.kind(),
        "member_expression"
            | "attribute"
            | "field_expression"
            | "subscript_expression"
            | "subscript"
    ) {
        if let Some(member) = node
            .child_by_field_name("property")
            .or_else(|| node.child_by_field_name("attribute"))
            .or_else(|| node.child_by_field_name("field"))
            .or_else(|| node.child_by_field_name("index"))
            .or_else(|| node.child_by_field_name("subscript"))
        {
            node = member;
        }
    }
    let Ok(raw) = node.utf8_text(source.as_bytes()) else {
        return false;
    };
    let decoded = serde_json::from_str::<String>(raw).ok();
    rules::is_sensitive_key(decoded.as_deref().unwrap_or(raw))
}

pub(super) fn inspect(root: Node<'_>, source: &str) -> Context {
    let mut context = Context::default();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if !node.has_error() && !node.is_missing() {
            if is_string(node) {
                context
                    .literals
                    .push((string_body(node, source), node.start_position().row + 1));
            }
            if is_type(node) {
                context.decided.push(node.byte_range());
            }
            if let Some((name, value)) = binding(node) {
                if is_sensitive_name(name, source) {
                    // Some grammars have an unnamed initializer after '=' instead
                    // of a value field (e.g. C#). Unknown forms must retain fallback.
                    let mut cursor = node.walk();
                    let assignment = node.children(&mut cursor).find(|child| child.kind() == "=");
                    let value = value.or_else(|| {
                        assignment.and_then(|operator| {
                            let mut cursor = node.walk();
                            let value = node
                                .named_children(&mut cursor)
                                .find(|child| child.start_byte() >= operator.end_byte());
                            value
                        })
                    });
                    let is_understood = value.map_or(assignment.is_none(), |value| {
                        values(value, source, &mut context.detections)
                    });
                    if is_understood {
                        context.decided.push(node.byte_range());
                    }
                }
            }
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    context
}
