use super::engine::*;
use super::model::*;
use super::syntax::*;

impl Interpreter<'_, '_> {
    pub fn polyglot_expression(
        &mut self,
        id: NodeId,
        depth: usize,
        is_read: bool,
    ) -> Option<Value> {
        let node = self.source.nodes[id].clone();
        let kind = node.kind.as_str();
        if kind == "null_assertion_expression" {
            return Some(self.expression(self.source.child(id, &["value"]), depth + 1, is_read));
        }
        if matches!(kind, "yield" | "yield_expression" | "yield_statement") {
            self.polyglot_statement(id);
            return Some(Value::unknown());
        }
        if matches!(self.source.language.as_str(), "scala" | "java")
            && matches!(kind, "match_expression" | "switch_expression")
        {
            let compared = self.expression(
                self.source.child(id, &["value", "condition"]),
                depth + 1,
                true,
            );
            let mut selected = Vec::new();
            let cases = self
                .source
                .child(id, &["body"])
                .map(|n| self.source.nodes[n].children.clone())
                .unwrap_or_default();
            for case in cases {
                if self.source.nodes[case].kind == "case_clause" {
                    let candidate =
                        self.expression(self.source.child(case, &["pattern"]), depth + 1, true);
                    if candidate == compared
                        && matches!(compared.kind.as_str(), "type" | "enum" | "literal" | "key")
                    {
                        if let Some(body) = self.source.child(case, &["body"]) {
                            selected.push(body);
                        }
                    }
                } else if self.source.nodes[case].kind == "switch_block_statement_group" {
                    let label = self.source.first(case, &["switch_label"]);
                    if compared.kind == "enum"
                        && label.is_some_and(|label| {
                            self.source.nodes[label].children.iter().any(|n| {
                                self.source.text(Some(*n))
                                    == compared.name.rsplit('.').next().unwrap_or_default()
                            })
                        })
                    {
                        selected.extend(
                            self.source.nodes[case]
                                .children
                                .iter()
                                .copied()
                                .filter(|n| Some(*n) != label),
                        );
                    }
                }
            }
            if selected.is_empty() {
                self.analyzer
                    .notices
                    .insert(("case_selection_unproven".into(), self.identifier.clone()));
                return Some(Value::unknown());
            }
            let before = self.returns.len();
            let mut values = Vec::new();
            for part in selected {
                if self.source.nodes[part].kind == "return_statement" {
                    self.statement(Some(part));
                } else {
                    values.push(self.expression(Some(part), depth + 1, true));
                }
            }
            values.extend(self.returns[before..].iter().cloned());
            return Some(Value::merge(
                values.into_iter().filter(|v| v.kind != "unknown"),
            ));
        }
        if is_identifier(kind)
            && is_read
            && matches!(self.source.language.as_str(), "kotlin" | "groovy")
            && !self.owner().is_empty()
            && !self.env.contains_key(self.source.text(Some(id)))
        {
            let name = self.source.text(Some(id)).to_owned();
            let receiver = Value::new("receiver", self.owner());
            let value = self.polyglot_property(receiver.clone(), &name, id);
            if value != receiver.field(&name) {
                return Some(value);
            }
        }
        if self.source.language == "scala" && kind == "generic_function" {
            let target = self.source.child(id, &["function"]);
            if target.is_some_and(|n| {
                self.source.nodes[n].kind == "field_expression"
                    && self.source.text(self.source.child(n, &["field"])) == "asInstanceOf"
            }) {
                let value = self.expression(
                    target.and_then(|n| self.source.child(n, &["value"])),
                    depth + 1,
                    true,
                );
                self.analyzer
                    .value_conditions
                    .entry(value.clone())
                    .or_default()
                    .insert("scala_cast_compatibility_required".into());
                return Some(value);
            }
            return Some(self.expression(target, depth + 1, true));
        }
        if self.source.language == "scala" && kind == "instance_expression" {
            if let Some(owner) = self
                .analyzer
                .program
                .scala
                .anonymous
                .get(&(self.source_index, id))
                .cloned()
            {
                let value = self.allocation(id, ":anonymous");
                self.analyzer.owners.insert(value.clone(), owner);
                return Some(value);
            }
            let ty = self
                .source
                .child(id, &["type"])
                .or_else(|| self.source.first(id, &["type_identifier", "generic_type"]));
            let owner = self.analyzer.program.resolve_type(
                self.source_index,
                self.source.text(ty),
                &self.owner(),
            );
            let nodes = self
                .source
                .child(id, &["arguments"])
                .map(|n| self.source.nodes[n].children.clone())
                .unwrap_or_default();
            let arguments: Vec<_> = nodes
                .iter()
                .map(|n| self.expression(Some(*n), depth + 1, true))
                .collect();
            if owner.starts_with("java:") {
                let value = self.allocation(id, ":instance");
                self.analyzer.owners.insert(value.clone(), owner.clone());
                let targets = self
                    .analyzer
                    .program
                    .method_candidates(&owner, owner.rsplit("::").next().unwrap_or_default());
                if targets.len() == 1 {
                    self.apply_scala(targets[0], value.clone(), arguments, id);
                }
                return Some(value);
            }
            if !owner.is_empty() {
                return Some(self.construct_scala(&owner, &arguments, &nodes, id, None));
            }
            self.analyzer.notices.insert((
                "anonymous_or_unresolved_constructor".into(),
                self.identifier.clone(),
            ));
            return Some(Value::unknown());
        }
        if is_member(kind) || (kind == "directly_assignable_expression" && node.children.len() > 1)
        {
            return Some(self.polyglot_member(id, depth, is_read));
        }
        if self.source.language == "java" && kind == "class_literal" {
            let name = self.source.text(node.children.first().copied());
            let owner = self
                .analyzer
                .program
                .resolve_type(self.source_index, name, &self.owner());
            return Some(if owner.is_empty() {
                Value::unknown()
            } else {
                Value::new("type", owner)
            });
        }
        if self.source.language == "java" && kind == "method_reference" && node.children.len() == 2
        {
            let receiver = self.expression(Some(node.children[0]), depth + 1, true);
            return Some(
                if matches!(
                    receiver.kind.as_str(),
                    "slot" | "parameter" | "receiver" | "allocation"
                ) {
                    Value::nested(
                        "bound_method",
                        "",
                        receiver,
                        Some(Value::new("key", self.source.text(Some(node.children[1])))),
                    )
                } else {
                    Value::unknown()
                },
            );
        }
        if matches!(kind, "tuple_expression" | "record_literal") {
            let parts = self.polyglot_tuple_parts(id);
            return Some(
                parts
                    .map(|parts| {
                        Value::tuple(
                            &parts
                                .iter()
                                .map(|n| self.expression(Some(*n), depth + 1, true))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_else(Value::unknown),
            );
        }
        if matches!(
            kind,
            "subscript"
                | "subscript_expression"
                | "element_access_expression"
                | "element_reference"
                | "array_access"
                | "bracket_index_expression"
                | "index_expression"
        ) {
            let base_node = self
                .source
                .child(id, &["object", "value", "argument", "table", "array"])
                .or_else(|| node.children.first().copied());
            let mut index = self
                .source
                .child(id, &["index", "subscript", "indices"])
                .or_else(|| {
                    node.children
                        .last()
                        .copied()
                        .filter(|n| Some(*n) != base_node)
                });
            if index.is_some_and(|n| {
                matches!(
                    self.source.nodes[n].kind.as_str(),
                    "argument" | "argument_list" | "bracketed_argument_list"
                )
            }) {
                index = index.and_then(|n| self.source.nodes[n].children.first().copied());
            }
            let mut base = self.expression(base_node, depth + 1, true);
            let mut value = self.expression(index, depth + 1, true);
            if self.source.language == "php" && index.is_none() && !is_read {
                value = Value::element();
            }
            if value.kind == "unknown" {
                value = Value::new("dynamic_key", format!("{}:{}", self.identifier, node.start));
            }
            if self.analyzer.kind(&base, self.source_index) == "map"
                || (self.source.language == "lua" && value.kind != "key")
            {
                base = base.slot(Value::entries());
            }
            return Some(base.slot(value));
        }
        if self.source.language == "dart"
            && matches!(
                kind,
                "expression_statement"
                    | "assignable_expression"
                    | "argument"
                    | "initializer_list_entry"
            )
        {
            return Some(self.dart_chain(id, depth));
        }
        if matches!(
            kind,
            "expression_statement"
                | "argument"
                | "value_argument"
                | "parenthesized_expression"
                | "parenthesized_lvalue"
                | "directly_assignable_expression"
                | "await_expression"
                | "try_expression"
                | "postfix_expression"
                | "prefix_expression"
                | "cast_expression"
                | "as_expression"
                | "argument_value"
                | "unary_expression"
                | "pointer_expression"
                | "argument_list"
                | "expression_list"
                | "call_suffix"
                | "callable_expression"
        ) {
            let target = self
                .source
                .child(id, &["value", "argument", "expression", "body"])
                .or_else(|| node.children.last().copied());
            return Some(self.expression(target, depth + 1, true));
        }
        if matches!(
            kind,
            "instance_creation_expression"
                | "constructor_expression"
                | "object_creation_expression"
                | "new_expression"
        ) {
            let value = self.allocation(id, ":instance");
            let ty = self
                .source
                .text(
                    self.source
                        .child(id, &["type", "constructor", "constructed_type"]),
                )
                .to_owned();
            let owner = self
                .analyzer
                .program
                .resolve_type(self.source_index, &ty, &self.owner());
            self.analyzer
                .record_type(&value, self.source_index, &ty, &self.owner());
            let container = type_kind(&ty);
            if !container.is_empty() {
                self.analyzer.kinds.insert(value.clone(), container);
            }
            let args = self
                .source
                .child(id, &["arguments"])
                .or_else(|| self.source.first(id, &["value_arguments"]));
            let args = args
                .map(|n| self.source.nodes[n].children.clone())
                .unwrap_or_default();
            let values = args
                .iter()
                .map(|n| self.expression(Some(*n), depth + 1, true))
                .collect::<Vec<_>>();
            if !owner.is_empty() {
                self.analyzer.owners.insert(value.clone(), owner.clone());
                let name = if self.source.language == "swift" {
                    "init"
                } else {
                    owner.rsplit("::").next().unwrap_or_default()
                };
                if let Some(targets) = self
                    .analyzer
                    .program
                    .methods
                    .get(&(owner.clone(), name.into()))
                    .filter(|v| v.len() == 1)
                    .cloned()
                {
                    self.apply_summary(targets[0], value.clone(), values, id);
                }
            }
            return Some(value);
        }
        if matches!(
            kind,
            "binary_expression"
                | "binary_operator"
                | "infix_expression"
                | "infix_function_call"
                | "elvis_expression"
        ) {
            if self.source.language == "scala"
                && self.source.text(self.source.child(id, &["operator"])) == "+"
            {
                let left = self.expression(self.source.child(id, &["left"]), depth + 1, true);
                if self.analyzer.kind(&left, self.source_index) == "set" {
                    let right = self.expression(self.source.child(id, &["right"]), depth + 1, true);
                    let value = self.allocation(id, ":set_copy");
                    self.analyzer.kinds.insert(value.clone(), "set".into());
                    self.emit(
                        "store",
                        value.clone().slot(Value::element()),
                        left.slot(Value::element()),
                        id,
                        &["immutable_collection_copy"],
                        None,
                    );
                    self.emit(
                        "store",
                        value.clone().slot(Value::element()),
                        right,
                        id,
                        &["immutable_collection_copy"],
                        None,
                    );
                    return Some(value);
                }
            }
            let values: Vec<_> = node
                .children
                .iter()
                .map(|n| self.expression(Some(*n), depth + 1, true))
                .filter(|v| {
                    matches!(
                        v.kind.as_str(),
                        "slot" | "allocation" | "receiver" | "parameter"
                    )
                })
                .collect();
            return Some(Value::merge(values));
        }
        if is_lambda(kind) && self.source.language == "cpp" {
            let captures = self.source.child(id, &["captures"]);
            let text = self.source.text(captures);
            if text.contains(['&', '=', '*']) {
                self.analyzer.notices.insert((
                    "closure_capture_form_unsupported".into(),
                    self.identifier.clone(),
                ));
                return Some(Value::unknown());
            }
            let mut env = Environment::new();
            for capture in captures
                .map(|n| self.source.nodes[n].children.clone())
                .unwrap_or_default()
            {
                let name = self.source.text(Some(capture));
                if name == "this" || self.source.nodes[capture].kind == "identifier" {
                    env.insert(name.into(), self.binding(name));
                } else {
                    self.analyzer.notices.insert((
                        "closure_capture_form_unsupported".into(),
                        self.identifier.clone(),
                    ));
                    return Some(Value::unknown());
                }
            }
            if env.len() > 8 {
                return Some(Value::unknown());
            }
            if let Some(index) = self
                .analyzer
                .program
                .function_nodes
                .get(&(self.source_index, id))
                .copied()
            {
                let mut bindings = Value::new("capture_end", "");
                for (name, value) in env.into_iter().rev() {
                    bindings = Value::nested("capture_binding", name, value, Some(bindings));
                }
                return Some(Value::nested(
                    "closure",
                    &self.analyzer.program.functions[index].identifier,
                    bindings,
                    None,
                ));
            }
        }
        None
    }
    pub fn polyglot_member(&mut self, id: NodeId, depth: usize, is_read: bool) -> Value {
        let node = self.source.nodes[id].clone();
        let base = self
            .source
            .child(
                id,
                &[
                    "object",
                    "argument",
                    "value",
                    "table",
                    "target",
                    "receiver",
                    "expression",
                ],
            )
            .or_else(|| (node.children.len() > 1).then(|| node.children[0]));
        let mut field = self
            .source
            .child(
                id,
                &["attribute", "field", "property", "method", "suffix", "name"],
            )
            .or_else(|| node.children.last().copied());
        if field.is_some_and(|n| {
            matches!(
                self.source.nodes[n].kind.as_str(),
                "navigation_suffix" | "member_binding_expression"
            )
        }) {
            field = field.and_then(|n| self.source.nodes[n].children.last().copied());
        }
        let mut base = if base.is_some() {
            self.expression(base, depth + 1, true)
        } else if self.source.text(Some(id)).starts_with("this.") {
            Value::new("receiver", self.owner())
        } else {
            Value::unknown()
        };
        if self.source.language == "php"
            && field.is_some_and(|n| self.source.nodes[n].kind == "variable_name")
        {
            let key = self.expression(field, depth + 1, true);
            return if base.kind != "unknown" && key.kind == "key" {
                base.slot(key)
            } else {
                Value::unknown()
            };
        }
        let name = self
            .source
            .text(field)
            .trim_start_matches(['.', '$', '?', '-', '>'])
            .to_owned();
        if self.source.language == "lua" && self.analyzer.kind(&base, self.source_index) == "map" {
            base = base.slot(Value::entries());
        }
        if base.kind == "unknown" || name.is_empty() {
            return Value::unknown();
        }
        if is_read {
            self.polyglot_property(base, &name, id)
        } else if self.source.language == "swift" && name.parse::<usize>().is_ok() {
            base.slot(Value::new("tuple_index", name))
        } else {
            base.field(&name)
        }
    }
    pub fn polyglot_property(&mut self, base: Value, name: &str, id: NodeId) -> Value {
        let owner = self.analyzer.owner(&base, self.source_index);
        if base.kind == "type"
            && self
                .analyzer
                .program
                .jvm
                .enum_constants
                .get(&owner)
                .is_some_and(|c| c.contains(name))
        {
            return Value::new("enum", format!("{owner}.{name}"));
        }
        if self.source.language == "scala" {
            if self.is_macro_template && name == "splice" {
                return base;
            }
            let methods = self.analyzer.program.method_candidates(&owner, name);
            if methods.len() == 1 {
                let function = &self.analyzer.program.functions[methods[0]];
                let source = &self.analyzer.program.sources[function.source];
                if source.language == "scala"
                    && !source.nodes[function.node]
                        .children
                        .iter()
                        .any(|n| source.nodes[*n].kind == "parameters")
                {
                    return self.apply_scala(methods[0], base, Vec::new(), id);
                }
            }
            if name == "toArray"
                && matches!(
                    self.analyzer.kind(&base, self.source_index).as_str(),
                    "array" | "set"
                )
            {
                return self.invoke_polyglot(base.field(name), Vec::new(), id, &[], None);
            }
        }
        let mut getters = self
            .analyzer
            .program
            .getters
            .get(&(owner.clone(), name.into()))
            .cloned()
            .unwrap_or_default();
        if self.source.language == "groovy" && !self.source.text(Some(id)).contains(".@") {
            let mut chars = name.chars();
            let getter = format!(
                "get{}{}",
                chars
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_default(),
                chars.as_str()
            );
            getters = self
                .analyzer
                .program
                .methods
                .get(&(owner, getter))
                .cloned()
                .unwrap_or_default();
        }
        if !getters.is_empty() {
            return if getters.len() == 1
                && self.analyzer.program.functions[getters[0]]
                    .parameters
                    .is_empty()
            {
                self.apply_summary(getters[0], base, Vec::new(), id)
            } else {
                Value::unknown()
            };
        }
        if self.source.language == "swift" && name.parse::<usize>().is_ok() {
            return base.slot(Value::new("tuple_index", name));
        }
        if self.source.language == "scala" && name.len() == 2 && name.starts_with('_') {
            if let Ok(index) = name[1..].parse::<usize>() {
                if (1..=8).contains(&index) {
                    return base.slot(Value::new("tuple_index", (index - 1).to_string()));
                }
            }
        }
        base.field(name)
    }
    pub fn polyglot_tuple_parts(&mut self, id: NodeId) -> Option<Vec<NodeId>> {
        let node = &self.source.nodes[id];
        if self.source.language == "swift" {
            if node.fields.contains_key("name") {
                self.analyzer.notices.insert((
                    "named_tuple_projection_unsupported".into(),
                    self.identifier.clone(),
                ));
                return None;
            }
            return Some(node.fields.get("value").cloned().unwrap_or_default());
        }
        let children: Vec<_> = node
            .children
            .iter()
            .copied()
            .filter(|n| !self.source.nodes[*n].kind.contains("comment"))
            .collect();
        if node.kind == "record_literal" {
            if children.iter().any(|n| {
                self.source.nodes[*n].kind != "record_field"
                    || self.source.nodes[*n].children.len() != 1
            }) {
                self.analyzer.notices.insert((
                    "named_record_projection_unsupported".into(),
                    self.identifier.clone(),
                ));
                return None;
            }
            return Some(
                children
                    .iter()
                    .map(|n| self.source.nodes[*n].children[0])
                    .collect(),
            );
        }
        Some(children)
    }
    pub fn dart_chain(&mut self, id: NodeId, depth: usize) -> Value {
        let mut value = Value::unknown();
        for item in self.source.nodes[id].children.clone() {
            if matches!(
                self.source.nodes[item].kind.as_str(),
                "selector"
                    | "unconditional_assignable_selector"
                    | "conditional_assignable_selector"
            ) {
                let selector = if self.source.nodes[item].kind == "selector" {
                    self.source.nodes[item]
                        .children
                        .first()
                        .copied()
                        .unwrap_or(item)
                } else {
                    item
                };
                if self.source.nodes[selector].children.is_empty() {
                    continue;
                }
                let args = if self.source.nodes[selector].kind == "argument_part" {
                    Some(selector)
                } else {
                    self.source.first(selector, &["argument_part", "arguments"])
                };
                if let Some(args) = args {
                    let args = self.source.first(args, &["arguments"]).unwrap_or(args);
                    let nodes = self.source.nodes[args].children.clone();
                    let arguments = nodes
                        .iter()
                        .map(|n| self.expression(Some(*n), depth + 1, true))
                        .collect();
                    value = self.invoke_polyglot(value, arguments, id, &nodes, None);
                } else if self.source.text(Some(selector)).contains('[') {
                    let index = self
                        .source
                        .first(selector, &["index_selector"])
                        .unwrap_or(selector);
                    let key = self.expression(
                        self.source.nodes[index].children.first().copied(),
                        depth + 1,
                        true,
                    );
                    value = value.slot(key);
                } else {
                    let name = self.source.text(self.source.identifier(Some(selector)));
                    value = value.field(name);
                }
            } else {
                value = self.expression(Some(item), depth + 1, true);
            }
        }
        value
    }
}
