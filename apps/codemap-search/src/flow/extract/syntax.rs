use crate::flow::Constant;
use crate::parser::CodeRange;
use tree_sitter::{Node, Point};

pub(super) fn text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}
pub(super) fn range(node: Node<'_>) -> CodeRange {
    let a = node.start_position();
    let b = node.end_position();
    CodeRange {
        start_line: a.row + 1,
        start_col: a.column + 1,
        end_line: b.row + 1,
        end_col: b.column + 1,
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
pub(super) fn encloses(outer: &CodeRange, inner: &CodeRange) -> bool {
    (outer.start_line, outer.start_col) <= (inner.start_line, inner.start_col)
        && (inner.end_line, inner.end_col) <= (outer.end_line, outer.end_col)
}
pub(super) fn node_at<'a>(root: Node<'a>, location: &CodeRange) -> Option<Node<'a>> {
    root.named_descendant_for_point_range(
        Point::new(
            location.start_line.saturating_sub(1),
            location.start_col.saturating_sub(1),
        ),
        Point::new(
            location.end_line.saturating_sub(1),
            location.end_col.saturating_sub(2),
        ),
    )
}
pub(super) fn scope(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
        node = parent;
        if is_function(node)
            || matches!(
                node.kind(),
                "program"
                    | "module"
                    | "source_file"
                    | "translation_unit"
                    | "compilation_unit"
                    | "block"
                    | "statement_block"
                    | "class_body"
                    | "declaration_list"
                    | "function_body"
                    | "body_statement"
                    | "script_block"
            )
        {
            return node;
        }
    }
    node
}
pub(super) fn is_class(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class"
            | "class_declaration"
            | "abstract_class_declaration"
            | "class_definition"
            | "class_specifier"
            | "struct_specifier"
            | "struct_declaration"
            | "struct_item"
            | "type_spec"
            | "class_statement"
    )
}
pub(super) fn is_function(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration"
            | "function_definition"
            | "function_item"
            | "function_signature_item"
            | "method_definition"
            | "method_declaration"
            | "constructor_declaration"
            | "arrow_function"
            | "function_expression"
            | "lambda"
            | "lambda_expression"
            | "anonymous_function"
            | "closure_expression"
            | "method"
            | "singleton_method"
            | "class_method_definition"
            | "function_statement"
    )
}
pub(super) fn is_method(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "method_definition"
            | "method_declaration"
            | "constructor_declaration"
            | "method"
            | "singleton_method"
            | "class_method_definition"
    )
}
pub(super) fn is_comment(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "comment" | "line_comment" | "block_comment" | "multiline_comment"
    )
}
pub(super) fn is_import(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "import_statement"
            | "import_declaration"
            | "import_from_statement"
            | "import_header"
            | "use_declaration"
            | "namespace_use_declaration"
            | "using_directive"
            | "import_spec"
            | "import_spec_list"
    )
}
pub(super) fn is_identifier(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "simple_identifier"
            | "type_identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
            | "variable_name"
            | "variable"
            | "self"
            | "this"
            | "scoped_identifier"
            | "scoped_type_identifier"
            | "constant"
            | "instance_variable"
            | "simple_name"
            | "command_name"
            | "word"
            | "name"
    )
}
pub(super) fn identifier(node: Node<'_>, source: &[u8]) -> String {
    text(node, source).trim_start_matches('$').to_string()
}
pub(super) fn name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let Some(signature) = node.child_by_field_name("signature") {
        return name(signature, source);
    }
    if let Some(node) = node.child_by_field_name("name") {
        return Some(text(node, source).to_string());
    }
    if let Some(node) = node.child_by_field_name("declarator") {
        return name(node, source)
            .or_else(|| is_identifier(node).then(|| identifier(node, source)));
    }
    child(
        node,
        &[
            "identifier",
            "simple_identifier",
            "type_identifier",
            "simple_name",
            "function_name",
        ],
    )
    .map(|node| text(node, source).to_string())
}
pub(super) fn body(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("body").or_else(|| {
        child(
            node,
            &[
                "function_body",
                "block",
                "compound_statement",
                "body_statement",
                "statement_block",
                "function_statement_list",
                "script_block",
            ],
        )
    })
}
pub(super) fn parameters(node: Node<'_>) -> Vec<Node<'_>> {
    let node = node
        .child_by_field_name("signature")
        .and_then(|node| child(node, &["function_signature"]).or(Some(node)))
        .unwrap_or(node);
    if let Some(parameter) = node.child_by_field_name("parameter") {
        return vec![parameter];
    }
    if let Some(list) = node.child_by_field_name("parameters").or_else(|| {
        child(
            node,
            &[
                "formal_parameters",
                "parameters",
                "parameter_list",
                "function_value_parameters",
                "formal_parameter_list",
                "method_parameters",
                "lambda_parameters",
            ],
        )
    }) {
        return children(list)
            .into_iter()
            .filter(|node| {
                !is_comment(*node) && !matches!(node.kind(), "identifier_list" | "type_parameters")
            })
            .collect();
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        return parameters(declarator);
    }
    children(node)
        .into_iter()
        .filter(|node| node.kind() == "parameter")
        .collect()
}
pub(super) fn parameter_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if text(node, source).contains(',')
        || text(node, source).contains("...")
        || node.kind().contains("variadic")
        || node.kind().contains("splat")
    {
        return None;
    }
    if matches!(
        node.kind(),
        "rest_pattern"
            | "list_splat_pattern"
            | "dictionary_splat_pattern"
            | "variadic_parameter"
            | "spread_parameter"
            | "optional_parameter"
    ) {
        return None;
    }
    if is_identifier(node) || node.kind() == "self_parameter" {
        return Some(
            text(node, source)
                .trim_start_matches(['&', '$'])
                .trim_start_matches("mut ")
                .to_string(),
        );
    }
    let name = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("pattern"))
        .or_else(|| node.child_by_field_name("declarator"));
    if let Some(name) = name {
        return parameter_name(name, source);
    }
    child(
        node,
        &[
            "identifier",
            "simple_identifier",
            "variable_name",
            "variable",
            "simple_name",
        ],
    )
    .map(|node| identifier(node, source))
}
pub(super) fn is_binding(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "variable_declarator"
            | "let_declaration"
            | "var_spec"
            | "const_spec"
            | "init_declarator"
            | "assignment"
            | "short_var_declaration"
            | "variable_assignment"
            | "local_variable_declaration"
            | "property_declaration"
            | "variable_declaration"
    )
}
pub(super) fn binding_parts(node: Node<'_>) -> Option<(Node<'_>, Option<Node<'_>>)> {
    let pattern = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("pattern"))
        .or_else(|| node.child_by_field_name("left"))
        .or_else(|| node.child_by_field_name("declarator"))?;
    let value = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("right"))
        .or_else(|| child(node, &["equals_value_clause"]).and_then(|node| node.named_child(0)));
    let pattern = if pattern.kind() == "expression_list" && pattern.named_child_count() == 1 {
        pattern.named_child(0)?
    } else {
        pattern
    };
    let value = value.map(|value| {
        if value.kind() == "expression_list" && value.named_child_count() == 1 {
            value.named_child(0).unwrap()
        } else {
            value
        }
    });
    Some((pattern, value))
}
pub(super) fn pattern_names(node: Node<'_>, source: &[u8]) -> Vec<String> {
    if is_identifier(node) {
        return vec![identifier(node, source)];
    }
    if matches!(node.kind(), "pointer_declarator" | "reference_declarator") {
        return node
            .child_by_field_name("declarator")
            .map(|node| pattern_names(node, source))
            .unwrap_or_default();
    }
    if matches!(node.kind(), "pattern" | "binding_pattern") {
        return children(node)
            .into_iter()
            .flat_map(|node| pattern_names(node, source))
            .collect();
    }
    Vec::new()
}
pub(super) fn literal(node: Node<'_>, source: &[u8]) -> Option<Constant> {
    let value = text(node, source);
    if value.len() > 512 {
        return None;
    }
    if matches!(node.kind(), "true" | "false" | "boolean_literal") {
        return Some(Constant::Boolean(value.eq_ignore_ascii_case("true")));
    }
    if matches!(
        node.kind(),
        "null" | "null_literal" | "nil" | "none" | "None"
    ) {
        return Some(Constant::Null);
    }
    if matches!(
        node.kind(),
        "number"
            | "number_literal"
            | "integer"
            | "integer_literal"
            | "float"
            | "float_literal"
            | "decimal_integer_literal"
            | "int_literal"
    ) {
        return Some(Constant::Number(value.into()));
    }
    if matches!(
        node.kind(),
        "string"
            | "string_literal"
            | "interpreted_string_literal"
            | "raw_string_literal"
            | "string_value"
            | "line_string_literal"
            | "verbatim_string_literal"
    ) {
        if children(node).iter().any(|node| {
            matches!(
                node.kind(),
                "interpolation"
                    | "string_interpolation"
                    | "template_substitution"
                    | "variable_name"
                    | "embedded_expression"
            )
        }) {
            return None;
        }
        if value.starts_with('"') {
            if let Ok(value) = serde_json::from_str::<String>(value) {
                return Some(Constant::String(value));
            }
        }
        if value.len() >= 2
            && value.starts_with('\'')
            && value.ends_with('\'')
            && !value[1..value.len() - 1].contains('\\')
        {
            return Some(Constant::String(value[1..value.len() - 1].into()));
        }
    }
    None
}
pub(super) fn field_parts(node: Node<'_>) -> Option<(Node<'_>, Node<'_>, bool)> {
    if matches!(
        node.kind(),
        "member_expression"
            | "field_expression"
            | "selector_expression"
            | "attribute"
            | "member_access_expression"
            | "member_access"
            | "property_access_expression"
    ) {
        let object = node
            .child_by_field_name("object")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| node.child_by_field_name("operand"))
            .or_else(|| node.child_by_field_name("expression"))?;
        let key = node
            .child_by_field_name("property")
            .or_else(|| node.child_by_field_name("field"))
            .or_else(|| node.child_by_field_name("attribute"))
            .or_else(|| node.child_by_field_name("name"))?;
        return Some((object, key, true));
    }
    if matches!(
        node.kind(),
        "subscript_expression" | "subscript" | "index_expression" | "element_access_expression"
    ) {
        let object = node
            .child_by_field_name("object")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| node.child_by_field_name("operand"))
            .or_else(|| node.child_by_field_name("expression"))?;
        let key = node
            .child_by_field_name("index")
            .or_else(|| node.child_by_field_name("subscript"))
            .or_else(|| {
                node.child_by_field_name("arguments")
                    .and_then(|node| node.named_child(0))
            })?;
        return Some((object, key, false));
    }
    None
}
pub(super) fn is_call(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "call_expression"
            | "call"
            | "function_call"
            | "function_call_expression"
            | "method_invocation"
            | "invocation_expression"
            | "method_call_expression"
            | "member_call_expression"
            | "scoped_call_expression"
            | "new_expression"
            | "object_creation_expression"
            | "command"
    )
}
pub(super) fn call_parts(
    node: Node<'_>,
) -> Option<(Node<'_>, Option<Node<'_>>, Vec<Node<'_>>, bool)> {
    if !is_call(node) {
        return None;
    }
    let is_constructor = matches!(node.kind(), "new_expression" | "object_creation_expression");
    let (callee, receiver) = if matches!(
        node.kind(),
        "method_invocation"
            | "method_call_expression"
            | "member_call_expression"
            | "scoped_call_expression"
    ) {
        (
            node.child_by_field_name("name")?,
            node.child_by_field_name("object")
                .or_else(|| node.child_by_field_name("receiver"))
                .or_else(|| node.child_by_field_name("scope")),
        )
    } else if node.kind() == "call" && node.child_by_field_name("method").is_some() {
        (
            node.child_by_field_name("method")?,
            node.child_by_field_name("receiver"),
        )
    } else {
        (
            node.child_by_field_name("function")
                .or_else(|| node.child_by_field_name("constructor"))
                .or_else(|| node.child_by_field_name("type").filter(|_| is_constructor))
                .or_else(|| node.child_by_field_name("command_name"))
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| node.named_child(0))?,
            None,
        )
    };
    let arguments = node
        .child_by_field_name("arguments")
        .or_else(|| child(node, &["arguments", "argument_list", "value_arguments"]))
        .or_else(|| {
            child(node, &["call_suffix"]).and_then(|suffix| child(suffix, &["value_arguments"]))
        })
        .map(|node| {
            children(node)
                .into_iter()
                .filter(|node| !is_comment(*node))
                .map(|node| {
                    if matches!(node.kind(), "argument" | "value_argument")
                        && node.named_child_count() == 1
                    {
                        node.named_child(0).unwrap()
                    } else {
                        node
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Some((callee, receiver, arguments, is_constructor))
}
pub(super) fn is_object(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "object"
            | "dictionary"
            | "object_creation_literal"
            | "struct_expression"
            | "composite_literal"
            | "table_constructor"
    )
}
pub(super) fn object_field<'a>(node: Node<'a>, source: &[u8]) -> Option<(String, Node<'a>)> {
    if matches!(
        node.kind(),
        "shorthand_property_identifier" | "shorthand_field_initializer"
    ) {
        return Some((identifier(node, source), node));
    }
    if matches!(
        node.kind(),
        "pair" | "field_initializer" | "keyed_element" | "field"
    ) {
        let key = node
            .child_by_field_name("key")
            .or_else(|| node.child_by_field_name("field"))
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.named_child(0))?;
        let value = node
            .child_by_field_name("value")
            .or_else(|| node.named_child(1))?;
        let name = if let Some(Constant::String(value)) = literal(key, source) {
            value
        } else if is_identifier(key) {
            text(key, source).into()
        } else {
            return None;
        };
        return Some((name, value));
    }
    None
}
pub(super) fn transparent(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(
        node.kind(),
        "parenthesized_expression"
            | "parenthesized_expr"
            | "as_expression"
            | "type_assertion"
            | "non_null_expression"
            | "expression_list"
            | "argument"
            | "value_argument"
    ) && node.named_child_count() == 1
    {
        node.named_child(0)
    } else {
        None
    }
}
pub(super) fn is_positional_argument(node: Node<'_>) -> bool {
    !matches!(
        node.kind(),
        "spread_element"
            | "spread_argument"
            | "list_splat"
            | "dictionary_splat"
            | "keyword_argument"
            | "named_argument"
            | "variadic_argument"
    ) && !(matches!(node.kind(), "argument" | "value_argument") && node.named_child_count() != 1)
}

pub(super) fn has_opaque_modifiers(node: Node<'_>, source: &[u8]) -> bool {
    let prefix = node
        .child_by_field_name("name")
        .and_then(|name| source.get(node.start_byte()..name.start_byte()))
        .and_then(|prefix| std::str::from_utf8(prefix).ok())
        .unwrap_or_else(|| text(node, source).split(['(', '{']).next().unwrap_or(""));
    prefix
        .split_whitespace()
        .any(|word| matches!(word, "get" | "set" | "async"))
        || prefix.contains("function*")
        || prefix.contains("function *")
        || node
            .parent()
            .is_some_and(|parent| parent.kind() == "decorated_definition")
        || children(node)
            .iter()
            .any(|child| child.kind() == "decorator")
}
pub(super) fn has_conditional_parent(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if is_control(parent) || matches!(parent.kind(), "preproc_elif" | "preproc_else") {
            return true;
        }
        node = parent;
    }
    false
}
pub(super) fn has_accessor_modifiers(node: Node<'_>, source: &[u8]) -> bool {
    node.child_by_field_name("name")
        .and_then(|name| source.get(node.start_byte()..name.start_byte()))
        .and_then(|prefix| std::str::from_utf8(prefix).ok())
        .is_some_and(|prefix| {
            prefix
                .split_whitespace()
                .any(|word| matches!(word, "get" | "set"))
        })
}
pub(super) fn is_return(node: Node<'_>) -> bool {
    (node.kind() == "control_transfer_statement" && node.child_by_field_name("result").is_some())
        || matches!(
            node.kind(),
            "return_statement" | "return_expression" | "return" | "jump_expression"
        )
}
pub(super) fn return_values(node: Node<'_>) -> Vec<Node<'_>> {
    let children = children(node);
    if children.len() == 1 && matches!(children[0].kind(), "expression_list" | "argument_list") {
        self::children(children[0])
    } else {
        children
    }
}
pub(super) fn assignment(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    if !matches!(
        node.kind(),
        "assignment" | "assignment_expression" | "assignment_statement"
    ) {
        return None;
    }
    Some((
        node.child_by_field_name("left")?,
        node.child_by_field_name("right"),
    ))
    .and_then(|(left, right)| right.map(|right| (left, right)))
}
pub(super) fn is_statement_container(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "program"
            | "module"
            | "source_file"
            | "translation_unit"
            | "compilation_unit"
            | "block"
            | "compound_statement"
            | "statement_block"
            | "function_body"
            | "body_statement"
            | "statements"
            | "statement_list"
            | "function_statement_list"
            | "script_block"
            | "chunk"
            | "closure"
    )
}
pub(super) fn is_control(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "if_statement"
            | "if_expression"
            | "if"
            | "unless"
            | "for_statement"
            | "for_expression"
            | "for_in_statement"
            | "while_statement"
            | "while_expression"
            | "while"
            | "match_expression"
            | "switch_statement"
            | "try_statement"
            | "try_expression"
            | "try"
            | "preproc_if"
            | "preproc_ifdef"
            | "conditional_expression"
            | "with_statement"
            | "yield_expression"
    )
}
pub(super) fn is_expression(node: Node<'_>) -> bool {
    is_identifier(node)
        || is_object(node)
        || node.kind().ends_with("literal")
        || matches!(
            node.kind(),
            "integer"
                | "string"
                | "number"
                | "binary_expression"
                | "binary_operator"
                | "parenthesized_expression"
                | "await_expression"
                | "member_expression"
                | "field_expression"
                | "attribute"
                | "selector_expression"
        )
}
