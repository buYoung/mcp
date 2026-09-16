use super::engine::*;
use super::model::*;
use super::syntax::*;
use std::collections::BTreeSet;

impl Interpreter<'_, '_> {
    pub fn expression(&mut self, id: Option<NodeId>, depth: usize, is_read: bool) -> Value {
        let Some(id) = id else {
            return Value::unknown();
        };
        if depth > 48 || !self.analyzer.tick(&self.identifier) {
            self.analyzer
                .notices
                .insert(("expression_depth_cap".into(), self.identifier.clone()));
            return Value::unknown();
        }
        let node = self.source.nodes[id].clone();
        let kind = node.kind.as_str();
        if self.is_polyglot() {
            if let Some(value) = self.polyglot_expression(id, depth, is_read) {
                return value;
            }
        }
        if is_identifier(kind)
            || matches!(
                kind,
                "this"
                    | "self"
                    | "this_expression"
                    | "self_expression"
                    | "scoped_identifier"
                    | "scoped_type_identifier"
            )
        {
            return self.binding(self.source.text(Some(id)));
        }
        if kind == "instance_variable" {
            return self
                .binding("self")
                .field(self.source.text(Some(id)).trim_start_matches('@'));
        }
        if matches!(
            kind,
            "string"
                | "string_literal"
                | "interpreted_string_literal"
                | "raw_string_literal"
                | "template_string"
                | "encapsed_string"
                | "simple_symbol"
                | "symbol"
        ) {
            let text = self.source.text(Some(id));
            if kind == "template_string" && text.contains("${") {
                let pattern = regex::Regex::new(r"\$\{([A-Za-z_$][\w$]*)\}").unwrap();
                let matches: Vec<_> = pattern.captures_iter(text).collect();
                if !matches.is_empty() && matches.len() == text.matches("${").count() {
                    let values: std::collections::BTreeMap<_, _> = matches
                        .iter()
                        .map(|m| (m[1].to_owned(), self.binding(&m[1])))
                        .collect();
                    if values
                        .values()
                        .all(|v| matches!(v.kind.as_str(), "key" | "literal"))
                    {
                        return Value::new(
                            "key",
                            pattern
                                .replace_all(&text[1..text.len() - 1], |m: &regex::Captures<'_>| {
                                    values[&m[1]].name.clone()
                                })
                                .into_owned(),
                        );
                    }
                }
            }
            if text.contains("${") || text.contains('\\') || text.contains("#{") {
                return Value::new("dynamic_key", format!("{}:{}", self.identifier, node.start));
            }
            return Value::new(
                "key",
                text.trim_matches(['\'', '"', '`']).trim_start_matches(':'),
            );
        }
        if matches!(
            kind,
            "number"
                | "number_literal"
                | "integer"
                | "float"
                | "integer_literal"
                | "real_literal"
                | "float_literal"
                | "int_literal"
                | "decimal_integer_literal"
                | "true"
                | "false"
                | "boolean_literal"
                | "null"
                | "nil"
                | "null_literal"
                | "none"
        ) {
            return Value::new("literal", self.source.text(Some(id)));
        }
        if kind == "match_expression" && self.source.language == "rust" {
            return self.match_value(id, depth);
        }
        if kind == "range_expression" {
            return Value::new(
                "range",
                format!("{}:{}", self.identifier, self.source.text(Some(id))),
            );
        }
        if is_function(kind) {
            if let Some(function) = self
                .analyzer
                .program
                .function_nodes
                .get(&(self.source_index, id))
                .copied()
            {
                return self.capture_callable(function);
            }
            return Value::unknown();
        }
        if is_member(kind) {
            return self.member(id, depth, is_read);
        }
        if matches!(
            kind,
            "subscript_expression"
                | "index_expression"
                | "subscript"
                | "element_access_expression"
                | "bracket_index_expression"
                | "indexing_expression"
        ) {
            let base_node = self
                .source
                .child(id, &["object", "operand", "value", "argument", "table"])
                .or_else(|| node.children.first().copied());
            let index_node = self
                .source
                .child(id, &["index", "subscript", "field"])
                .or_else(|| {
                    node.children
                        .last()
                        .copied()
                        .filter(|part| Some(*part) != base_node)
                });
            let mut base = self.expression(base_node, depth + 1, true);
            let index = self.expression(index_node, depth + 1, true);
            if matches!(base.kind.as_str(), "tuple" | "tuple_end") && index.kind == "literal" {
                if let Ok(index) = index.name.parse::<usize>() {
                    return base
                        .tuple_values()
                        .get(index)
                        .cloned()
                        .unwrap_or_else(Value::unknown);
                }
            }
            if self.source.language == "rust" {
                base = self.dereferenced_receiver(
                    base,
                    if is_read { "index" } else { "index_mut" },
                    id,
                );
            }
            if self.source.language == "go" && self.analyzer.kind(&base, self.source_index) == "map"
            {
                base = base.slot(Value::entries());
            }
            return base.slot(index);
        }
        if matches!(
            kind,
            "parenthesized_expression"
                | "parenthesized_expression_list"
                | "await_expression"
                | "reference_expression"
                | "unary_expression"
                | "as_expression"
                | "non_null_expression"
                | "type_assertion_expression"
                | "try_expression"
                | "generic_function"
                | "instantiation_expression"
                | "spread_element"
                | "expression_statement"
                | "literal_element"
                | "argument"
                | "value_argument"
                | "named_argument"
                | "pointer_expression"
                | "cast_expression"
                | "ref_expression"
                | "nullable_type"
                | "arrow_expression_clause"
        ) {
            let part = self
                .source
                .child(
                    id,
                    &["argument", "value", "expression", "function", "operand"],
                )
                .or_else(|| node.children.first().copied());
            let value = self.expression(part, depth + 1, true);
            if kind == "unary_expression" && self.source.text(Some(id)).starts_with("delete ") {
                self.emit("remove", value, Value::unknown(), id, &[], None);
                return Value::unknown();
            }
            if kind == "try_expression" && value.kind == "wrapper" {
                if matches!(value.name.as_str(), "rust:option" | "rust:result_ok") {
                    self.conditions.push(
                        if value.name == "rust:option" {
                            "option_some_required"
                        } else {
                            "result_ok_required"
                        }
                        .into(),
                    );
                    return value.base().clone();
                }
                if matches!(value.name.as_str(), "rust:option_none" | "rust:result_err") {
                    return Value::unknown();
                }
            }
            if self.source.language == "rust"
                && kind == "unary_expression"
                && self.source.text(Some(id)).starts_with('*')
            {
                if value.kind == "wrapper"
                    && matches!(value.name.as_str(), "rust:nonnull" | "rust:raw_pointer")
                {
                    self.conditions
                        .push("pointer_validity_and_aliasing_unproven".into());
                    return value.base().clone();
                }
                if self
                    .analyzer
                    .type_text(&value, self.source_index)
                    .starts_with('*')
                {
                    self.analyzer
                        .notices
                        .insert(("pointer_origin_unresolved".into(), self.identifier.clone()));
                    return Value::unknown();
                }
            }
            return value;
        }
        if kind == "unsafe_block" {
            return self.expression(self.source.first(id, &["block"]), depth + 1, true);
        }
        if kind == "type_cast_expression" && self.source.language == "rust" {
            let source = self.source.child(id, &["value"]);
            let value = self.expression(source, depth + 1, true);
            let target = self.source.text(self.source.child(id, &["type"]));
            if target.starts_with('*')
                && (source.is_some_and(|n| self.source.nodes[n].kind == "reference_expression")
                    || self
                        .analyzer
                        .type_text(&value, self.source_index)
                        .starts_with('&')
                    || (value.kind == "wrapper"
                        && matches!(value.name.as_str(), "rust:raw_pointer" | "rust:nonnull")))
            {
                self.conditions.extend([
                    "pointer_provenance_required".into(),
                    "pointer_lifetime_alignment_and_aliasing_unproven".into(),
                    "pointer_cast_layout_compatibility_required".into(),
                ]);
                return Value::nested(
                    "wrapper",
                    "rust:raw_pointer",
                    if value.kind == "wrapper" {
                        value.base().clone()
                    } else {
                        value
                    },
                    None,
                );
            }
            return Value::unknown();
        }
        if is_assignment(kind) || kind == "short_var_declaration" {
            return self.assignment(id, depth + 1);
        }
        if matches!(
            kind,
            "object"
                | "object_pattern"
                | "array"
                | "array_expression"
                | "struct_expression"
                | "composite_literal"
                | "list"
                | "list_expression"
                | "list_literal"
                | "dictionary"
                | "dictionary_literal"
                | "hash"
                | "array_creation_expression"
                | "array_initializer"
                | "initializer_list"
                | "table_constructor"
                | "array_creation"
                | "set_or_map_literal"
        ) {
            return self.object_value(id, depth);
        }
        if matches!(kind, "tuple_expression" | "tuple") {
            let values: Vec<_> = node
                .children
                .into_iter()
                .map(|part| self.expression(Some(part), depth + 1, true))
                .collect();
            return Value::tuple(&values);
        }
        if matches!(
            kind,
            "new_expression"
                | "object_creation_expression"
                | "new_object"
                | "new_expression_no_parentheses"
        ) {
            return self.construct(id, depth);
        }
        if is_call(kind) || kind == "macro_invocation" {
            return self.call(id, depth);
        }
        if matches!(kind, "conditional_expression" | "ternary_expression") {
            let condition = self.expression(self.source.child(id, &["condition"]), depth + 1, true);
            let yes = self.source.child(id, &["consequence", "consequent"]);
            let no = self.source.child(id, &["alternative"]);
            if condition == Value::new("literal", "true") {
                return self.expression(yes, depth + 1, true);
            }
            if condition == Value::new("literal", "false") {
                return self.expression(no, depth + 1, true);
            }
            let a = self.expression(yes, depth + 1, true);
            let b = self.expression(no, depth + 1, true);
            return Value::merge([a, b]);
        }
        if matches!(kind, "binary_expression" | "binary_operator" | "binary") {
            let values: Vec<_> = node
                .children
                .iter()
                .map(|part| self.expression(Some(*part), depth + 1, true))
                .collect();
            let operator = node
                .tokens
                .iter()
                .find(|op| matches!(op.as_str(), "===" | "!==" | "==" | "!=" | "||" | "??" | "+"))
                .cloned()
                .unwrap_or_default();
            if matches!(operator.as_str(), "===" | "!==" | "==" | "!=")
                && values.len() == 2
                && values.iter().all(|v| v.kind == "literal")
            {
                let equal = values[0] == values[1];
                return Value::new(
                    "literal",
                    (if matches!(operator.as_str(), "===" | "==") {
                        equal
                    } else {
                        !equal
                    })
                    .to_string(),
                );
            }
            if matches!(operator.as_str(), "||" | "??") {
                self.conditions
                    .push(format!("fallback_path_unresolved:{}", node.line));
                return values
                    .into_iter()
                    .find(|v| {
                        matches!(
                            v.kind.as_str(),
                            "slot" | "receiver" | "allocation" | "parameter"
                        )
                    })
                    .unwrap_or_else(Value::unknown);
            }
            return Value::unknown();
        }
        if matches!(
            kind,
            "expression_list"
                | "arguments"
                | "argument_list"
                | "value_arguments"
                | "bracketed_argument_list"
        ) {
            let values: Vec<_> = node
                .children
                .iter()
                .map(|n| self.expression(Some(*n), depth + 1, true))
                .collect();
            return if values.len() == 1 {
                values[0].clone()
            } else {
                Value::tuple(&values)
            };
        }
        if kind == "send_statement" {
            let channel = self.expression(self.source.child(id, &["channel"]), depth + 1, true);
            let value = self.expression(self.source.child(id, &["value"]), depth + 1, true);
            self.emit("transport_boundary", channel, value, id, &[], None);
            return Value::unknown();
        }
        if is_block(kind) {
            if self.source.language == "rust" {
                self.block_returns.push(Vec::new());
                self.statement(Some(id));
                return self.finish_block(id);
            }
            let previous = self.returns.len();
            self.statement(Some(id));
            return self
                .returns
                .get(previous..)
                .and_then(|v| v.last())
                .cloned()
                .unwrap_or_else(Value::unknown);
        }
        if self.source.language == "rust" && kind == "if_expression" {
            self.block_returns.push(Vec::new());
            self.branch(id);
            return self.finish_block(id);
        }
        if matches!(kind, "return_expression" | "return_statement") {
            self.statement(Some(id));
            return Value::unknown();
        }
        for part in node.children {
            self.expression(Some(part), depth + 1, true);
        }
        Value::unknown()
    }
    pub fn member(&mut self, id: NodeId, depth: usize, is_read: bool) -> Value {
        let node = self.source.nodes[id].clone();
        let base_node = self
            .source
            .child(
                id,
                &[
                    "object",
                    "value",
                    "operand",
                    "expression",
                    "argument",
                    "table",
                    "scope",
                ],
            )
            .or_else(|| node.children.first().copied());
        let field_node = self
            .source
            .child(
                id,
                &["property", "field", "attribute", "name", "member", "method"],
            )
            .or_else(|| {
                node.children
                    .last()
                    .copied()
                    .filter(|n| Some(*n) != base_node)
            });
        let mut base = self.expression(base_node, depth + 1, true);
        base = self.resolve_call_result(base, id);
        let field = self
            .source
            .text(field_node)
            .trim_start_matches('$')
            .to_owned();
        if field.is_empty() {
            return Value::unknown();
        }
        if base.kind == "wrapper" && base.name == "javascript:iterator_result" {
            return match field.as_str() {
                "value" => base.base().clone(),
                "done" => base.key().clone(),
                _ => Value::unknown(),
            };
        }
        if matches!(base.kind.as_str(), "tuple" | "tuple_end") && field == "length" {
            return Value::new("literal", base.tuple_values().len().to_string());
        }
        if base.kind == "namespace" {
            if let Some(exported) = self
                .analyzer
                .program
                .exports
                .get(&(base.name.clone(), field.clone()))
                .cloned()
            {
                if let Some((path, name)) = exported.rsplit_once('#') {
                    if let Some(index) = self
                        .analyzer
                        .program
                        .sources
                        .iter()
                        .position(|s| s.path == path)
                    {
                        if let Some(value) = self.analyzer.globals[index].get(name) {
                            return value.clone();
                        }
                        let owner = self.analyzer.program.resolve_type(index, name, "");
                        if !owner.is_empty() {
                            return Value::new("type", owner);
                        }
                    }
                    if let Some(functions) = self
                        .analyzer
                        .program
                        .methods
                        .get(&(path.into(), name.into()))
                        .filter(|f| f.len() == 1)
                    {
                        return Value::new(
                            "function",
                            &self.analyzer.program.functions[functions[0]].identifier,
                        );
                    }
                } else {
                    return Value::new("namespace", exported);
                }
            }
            if let Some(index) = self
                .analyzer
                .program
                .sources
                .iter()
                .position(|s| s.path == base.name)
            {
                if let Some(value) = self.analyzer.globals[index].get(&field) {
                    return value.clone();
                }
            }
            if let Some(functions) = self
                .analyzer
                .program
                .methods
                .get(&(base.name.clone(), field.clone()))
            {
                if functions.len() == 1 {
                    return Value::new(
                        "function",
                        &self.analyzer.program.functions[functions[0]].identifier,
                    );
                }
            }
        }
        if base.kind == "type"
            && matches!(self.source.language.as_str(), "typescript" | "javascript")
        {
            let owner = base.name.clone();
            base = Value::new("global", format!("{owner}::static"));
            self.analyzer.owners.insert(base.clone(), owner);
        }
        if is_read {
            let owner = self.analyzer.owner(&base, self.source_index);
            if let Some(getters) = self
                .analyzer
                .program
                .getters
                .get(&(owner, field.clone()))
                .cloned()
            {
                return if getters.len() == 1 {
                    self.apply_summary(getters[0], base, Vec::new(), id)
                } else {
                    Value::unknown()
                };
            }
        }
        base.field(&field)
    }
    pub fn object_value(&mut self, id: NodeId, depth: usize) -> Value {
        let node = self.source.nodes[id].clone();
        let value = self.allocation(id, "");
        let type_node = self.source.child(id, &["type", "name"]);
        let type_text = self.source.text(type_node).to_owned();
        self.analyzer
            .record_type(&value, self.source_index, &type_text, &self.owner());
        let is_array = matches!(
            node.kind.as_str(),
            "array"
                | "array_expression"
                | "list"
                | "list_expression"
                | "list_literal"
                | "array_creation_expression"
                | "array_initializer"
                | "array_creation"
        );
        if is_array {
            self.analyzer.kinds.insert(value.clone(), "array".into());
        }
        let is_map = matches!(
            node.kind.as_str(),
            "dictionary" | "dictionary_literal" | "hash" | "table_constructor"
        );
        if is_map {
            self.analyzer.kinds.insert(value.clone(), "map".into());
        }
        let owner = if type_text == "Self" {
            self.owner()
        } else {
            self.analyzer.owner(&value, self.source_index)
        };
        if !owner.is_empty() {
            self.analyzer.owners.insert(value.clone(), owner.clone());
        }
        let body = self.source.child(id, &["body"]);
        let fields = body
            .map(|b| self.source.nodes[b].children.clone())
            .unwrap_or_else(|| node.children.clone());
        let mut literal_fields = BTreeSet::new();
        for (field_index, field) in fields.iter().copied().enumerate() {
            let field_kind = self.source.nodes[field].kind.clone();
            if Some(field) == type_node || matches!(field_kind.as_str(), "comment" | "line_comment")
            {
                continue;
            }
            if is_array {
                let stored = self.expression(Some(field), depth + 1, true);
                self.emit(
                    "store",
                    value.clone().slot(Value::element()),
                    stored,
                    field,
                    &[],
                    None,
                );
                continue;
            }
            if matches!(
                field_kind.as_str(),
                "pair"
                    | "field_initializer"
                    | "keyed_element"
                    | "field"
                    | "element_initializer"
                    | "association"
                    | "dictionary_entry"
            ) {
                let key_node = self
                    .source
                    .child(field, &["key", "field", "name"])
                    .or_else(|| self.source.nodes[field].children.first().copied());
                let stored_node = self.source.child(field, &["value"]).or_else(|| {
                    self.source.nodes[field]
                        .children
                        .last()
                        .copied()
                        .filter(|n| Some(*n) != key_node)
                });
                let key = if key_node
                    .is_some_and(|n| self.source.nodes[n].kind == "computed_property_name")
                {
                    self.expression(
                        key_node.and_then(|n| self.source.nodes[n].children.first().copied()),
                        depth + 1,
                        true,
                    )
                } else {
                    Value::new("key", self.source.text(key_node).trim_matches(['\'', '"']))
                };
                let stored = self.expression(stored_node, depth + 1, true);
                literal_fields.insert(key.name.clone());
                self.emit(
                    "store",
                    value.clone().slot(key.clone()),
                    stored.clone(),
                    field,
                    &[],
                    None,
                );
                if !owner.is_empty() {
                    self.emit(
                        "store",
                        Value::new("receiver", &owner).slot(key),
                        stored,
                        field,
                        &["constructor_schema_only"],
                        None,
                    );
                }
            } else if matches!(
                field_kind.as_str(),
                "shorthand_property_identifier" | "shorthand_field_initializer"
            ) {
                let name = self.source.text(Some(field)).to_owned();
                let stored = self.binding(&name);
                literal_fields.insert(name.clone());
                self.emit(
                    "store",
                    value.clone().field(&name),
                    stored.clone(),
                    field,
                    &[],
                    None,
                );
                if !owner.is_empty() {
                    self.emit(
                        "store",
                        Value::new("receiver", &owner).field(&name),
                        stored,
                        field,
                        &["constructor_schema_only"],
                        None,
                    );
                }
            } else if field_kind == "method_definition" {
                if let Some(function) = self
                    .analyzer
                    .program
                    .function_nodes
                    .get(&(self.source_index, field))
                    .copied()
                {
                    let callback = self.capture_callable(function);
                    let named = self.source.child(field, &["name"]);
                    let key = if named
                        .is_some_and(|n| self.source.nodes[n].kind == "computed_property_name")
                    {
                        self.expression(
                            named.and_then(|n| self.source.nodes[n].children.first().copied()),
                            depth + 1,
                            true,
                        )
                    } else {
                        Value::new("key", self.source.text(named))
                    };
                    let first = self.facts.len();
                    self.emit("store", value.clone().slot(key), callback, field, &[], None);
                    for fact in &mut self.facts[first..] {
                        fact.is_method_declaration = true;
                    }
                }
            } else if field_kind == "spread_element" {
                let copied = self.expression(Some(field), depth + 1, true);
                let excluded: Vec<_> = fields[field_index + 1..]
                    .iter()
                    .filter_map(|later| {
                        if matches!(
                            self.source.nodes[*later].kind.as_str(),
                            "pair" | "shorthand_property_identifier"
                        ) {
                            let key = self.source.child(*later, &["key"]).unwrap_or(*later);
                            if is_identifier(&self.source.nodes[key].kind)
                                || matches!(
                                    self.source.nodes[key].kind.as_str(),
                                    "string" | "number"
                                )
                            {
                                return Some(Value::new(
                                    "key",
                                    self.source.text(Some(key)).trim_matches(['\'', '"']),
                                ));
                            }
                        }
                        None
                    })
                    .collect();
                self.emit(
                    "copy_properties",
                    value.clone(),
                    Value::nested("object_copy", "", copied, Some(Value::tuple(&excluded))),
                    field,
                    &[],
                    None,
                );
            }
        }
        if node.kind == "object" {
            self.analyzer
                .literal_fields
                .insert(value.clone(), literal_fields);
        }
        value
    }
    pub fn construct(&mut self, id: NodeId, depth: usize) -> Value {
        let type_node = self
            .source
            .child(id, &["constructor", "type", "class", "name"])
            .or_else(|| self.source.nodes[id].children.first().copied());
        let type_name = self.source.text(type_node).to_owned();
        let value = self.allocation(id, "");
        let type_arguments = self.source.text(self.source.child(id, &["type_arguments"]));
        self.analyzer.record_type(
            &value,
            self.source_index,
            &format!("{type_name}{type_arguments}"),
            &self.owner(),
        );
        if !self.source.module_bindings.contains(&type_name)
            && !self.local_bindings.contains(&type_name)
        {
            let kind = match type_name.as_str() {
                "Map" => "map",
                "Set" => "set",
                "Array" => "array",
                _ => "",
            };
            if !kind.is_empty() {
                self.analyzer.kinds.insert(value.clone(), kind.into());
            }
        }
        let args_node = self
            .source
            .child(id, &["arguments"])
            .or_else(|| self.source.first(id, &["argument_list", "arguments"]));
        let nodes = args_node
            .map(|n| self.source.nodes[n].children.clone())
            .unwrap_or_default();
        let arguments: Vec<_> = nodes
            .iter()
            .map(|n| self.expression(Some(*n), depth + 1, true))
            .collect();
        let owner = self.analyzer.owner(&value, self.source_index);
        let target = self.expression(type_node, depth + 1, true);
        if matches!(target.kind.as_str(), "function" | "closure") {
            if let Some(index) = self.function_index(&target.name) {
                self.apply_source(
                    index,
                    value.clone(),
                    arguments,
                    id,
                    Some(closure_captures(&target)),
                );
            }
            let prototypes = self
                .analyzer
                .read_values(&target.clone().field("prototype"));
            if prototypes.len() == 1 {
                self.analyzer
                    .prototypes
                    .insert(value.clone(), prototypes[0].clone());
            }
        } else {
            for name in [
                "constructor",
                "__init__",
                "initialize",
                "init",
                type_name.as_str(),
            ] {
                if let Some(functions) = self
                    .analyzer
                    .program
                    .methods
                    .get(&(owner.clone(), name.into()))
                    .cloned()
                {
                    if functions.len() == 1 {
                        self.apply_summary(functions[0], value.clone(), arguments.clone(), id);
                        break;
                    }
                }
            }
        }
        value
    }
    pub fn capture_callable(&mut self, index: usize) -> Value {
        let function = self.analyzer.program.functions[index].clone();
        if function.parent.is_none() {
            return Value::new("function", function.identifier);
        }
        let source = &self.analyzer.program.sources[function.source];
        let Some(body) = function.body else {
            return Value::unknown();
        };
        let free: BTreeSet<_> = source
            .walk(body, false)
            .into_iter()
            .filter(|id| {
                is_identifier(&source.nodes[*id].kind)
                    || matches!(source.nodes[*id].kind.as_str(), "this" | "self")
            })
            .map(|id| source.text(Some(id)).to_owned())
            .collect();
        let bound: BTreeSet<_> = source
            .lexical_bindings(body)
            .into_iter()
            .chain(function.parameters.iter().map(|p| p.0.clone()))
            .collect();
        let mut enclosing = BTreeSet::from(["this".to_owned()]);
        let mut parent = function.parent;
        while let Some(index) = parent {
            let ancestor = &self.analyzer.program.functions[index];
            enclosing.extend(ancestor.parameters.iter().map(|p| p.0.clone()));
            if let Some(body) = ancestor.body {
                enclosing
                    .extend(self.analyzer.program.sources[ancestor.source].lexical_bindings(body));
            }
            if self.analyzer.program.sources[ancestor.source].nodes[ancestor.node].kind
                != "arrow_function"
            {
                enclosing.insert("arguments".into());
            }
            parent = ancestor.parent;
        }
        let mut captures: Environment = self
            .env
            .iter()
            .filter(|(name, _)| {
                free.contains(*name)
                    && !bound.contains(*name)
                    && (!matches!(source.language.as_str(), "typescript" | "javascript")
                        || enclosing.contains(*name))
            })
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect();
        if captures.len() > 32 {
            self.analyzer
                .notices
                .insert(("closure_capture_cap".into(), function.identifier));
            return Value::unknown();
        }
        self.analyzer.captures.insert(index, captures.clone());
        let identity = Value::new(
            "allocation",
            format!("{}:closure{}", function.identifier, self.instance_context),
        );
        self.analyzer
            .allocation_owners
            .insert(identity.clone(), self.identifier.clone());
        captures.insert("__source_callable_identity__".into(), identity);
        let mut bindings = Value::new("capture_end", "");
        for (name, value) in captures.into_iter().rev() {
            bindings = Value::nested("capture_binding", name, value, Some(bindings));
        }
        Value::nested("closure", function.identifier, bindings, None)
    }
}
