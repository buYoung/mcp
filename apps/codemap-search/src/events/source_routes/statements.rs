use super::engine::*;
use super::model::*;
use super::syntax::*;
use std::collections::BTreeSet;

impl Interpreter<'_, '_> {
    pub fn statement(&mut self, node: Option<NodeId>) {
        let Some(id) = node else {
            return;
        };
        if !self.analyzer.tick(&self.identifier) {
            return;
        }
        let node = self.source.nodes[id].clone();
        let kind = node.kind.as_str();
        if self.is_polyglot() && self.polyglot_statement(id) {
            return;
        }
        if is_function(kind) {
            self.expression(Some(id), 0, true);
            return;
        }
        if is_class(kind) {
            return;
        }
        if matches!(
            kind,
            "variable_declarator"
                | "let_declaration"
                | "var_spec"
                | "const_spec"
                | "init_declarator"
                | "var_definition"
                | "val_definition"
                | "initialized_identifier"
                | "pattern_variable_declaration"
        ) {
            let pattern = self.source.child(id, &["name", "pattern", "declarator"]);
            let value_node = self.source.child(id, &["value"]).or_else(|| {
                (kind == "initialized_identifier")
                    .then(|| node.children.last().copied())
                    .flatten()
            });
            let value = self.expression(value_node, 0, true);
            if self
                .source
                .expansion(id)
                .is_some_and(|e| e.definition.is_some())
            {
                self.analyzer
                    .value_conditions
                    .entry(value.clone())
                    .or_default()
                    .extend([
                        "source_declarative_statement_expansion".into(),
                        "macro_template_typing_unproven".into(),
                    ]);
            }
            let type_text = self
                .source
                .text(self.source.child(id, &["type"]))
                .to_owned();
            self.bind(pattern, value, &type_text);
            return;
        }
        if matches!(
            kind,
            "property_declaration"
                | "local_variable_declaration"
                | "initialized_variable_definition"
        ) {
            let declaration = self.source.first(id, &["variable_declaration"]);
            let pattern = self
                .source
                .child(id, &["name", "pattern"])
                .or_else(|| declaration.and_then(|d| self.source.identifier(Some(d))));
            if let Some(pattern) = pattern {
                let value_node = self.source.child(id, &["value"]).or_else(|| {
                    node.children
                        .last()
                        .copied()
                        .filter(|last| Some(*last) != declaration && *last != pattern)
                });
                let value = self.expression(value_node, 0, true);
                let ty = self
                    .source
                    .child(id, &["type"])
                    .or_else(|| declaration.and_then(|d| self.source.child(d, &["type"])));
                let type_text = self.source.text(ty).to_owned();
                self.bind(Some(pattern), value, &type_text);
                return;
            }
        }
        if kind == "short_var_declaration" || is_assignment(kind) {
            self.assignment(id, 0);
            return;
        }
        if matches!(
            kind,
            "return_statement" | "return_expression" | "return" | "jump_expression"
        ) {
            let value_node = self
                .source
                .child(id, &["argument", "value", "expression"])
                .or_else(|| node.children.first().copied());
            let value = self.expression(value_node, 0, true);
            self.record_return(value, id);
            return;
        }
        if matches!(
            kind,
            "if_statement"
                | "if_expression"
                | "switch_statement"
                | "expression_switch_statement"
                | "type_switch_statement"
                | "match_expression"
                | "conditional_expression"
                | "unless"
                | "if"
        ) {
            self.branch(id);
            return;
        }
        if is_loop(kind) {
            self.loop_statement(id);
            return;
        }
        if matches!(
            kind,
            "while_statement" | "while_expression" | "do_statement"
        ) {
            let prior = self.conditions.clone();
            self.conditions.push("iteration_may_be_empty".into());
            for part in node.children {
                self.statement(Some(part));
            }
            self.conditions = prior;
            return;
        }
        if is_call(kind)
            || matches!(
                kind,
                "expression_statement"
                    | "new_expression"
                    | "await_expression"
                    | "send_statement"
                    | "object_creation_expression"
            )
        {
            self.expression(Some(id), 0, true);
            return;
        }
        if matches!(
            kind,
            "type_declaration"
                | "type_alias_declaration"
                | "interface_declaration"
                | "struct_item"
                | "enum_item"
                | "import_statement"
                | "import_declaration"
                | "use_declaration"
                | "comment"
                | "line_comment"
                | "block_comment"
                | "attribute_item"
        ) {
            return;
        }
        let parts: Vec<_> = node
            .children
            .into_iter()
            .filter(|p| {
                !matches!(
                    self.source.nodes[*p].kind.as_str(),
                    "comment" | "line_comment" | "block_comment"
                )
            })
            .collect();
        let has_rust_tail = self.source.language == "rust"
            && kind == "block"
            && parts.last().is_some_and(|last| {
                let kind = self.source.nodes[*last].kind.as_str();
                !matches!(
                    kind,
                    "let_declaration" | "return_expression" | "function_item" | "attribute_item"
                ) && (kind != "expression_statement"
                    || !self.source.text(Some(*last)).trim_end().ends_with(';'))
            });
        for (index, part) in parts.iter().enumerate() {
            if has_rust_tail && index + 1 == parts.len() {
                let value = self.expression(Some(*part), 0, true);
                self.record_return(value, *part);
            } else {
                self.statement(Some(*part));
            }
        }
    }
    pub fn assignment(&mut self, id: NodeId, depth: usize) -> Value {
        if self.is_polyglot() {
            return self.polyglot_assignment(id, depth);
        }
        let left = self
            .source
            .child(id, &["left", "target", "name", "pattern"]);
        let right = self.source.child(id, &["right", "value"]);
        let left_nodes = left
            .map(|n| {
                if matches!(
                    self.source.nodes[n].kind.as_str(),
                    "expression_list" | "variable_list"
                ) {
                    self.source.nodes[n].children.clone()
                } else {
                    vec![n]
                }
            })
            .unwrap_or_default();
        let right_nodes = right
            .map(|n| {
                if self.source.nodes[n].kind == "expression_list" {
                    self.source.nodes[n].children.clone()
                } else {
                    vec![n]
                }
            })
            .unwrap_or_default();
        let mut values: Vec<_> = right_nodes
            .iter()
            .map(|n| self.expression(Some(*n), depth + 1, true))
            .collect();
        if values.len() == 1 && left_nodes.len() > 1 && values[0].kind == "tuple" {
            values = values[0].tuple_values();
        } else if values.len() == 1 && left_nodes.len() > 1 && values[0].kind == "choice" {
            let alternatives = values[0].options();
            if alternatives
                .iter()
                .all(|v| v.kind == "tuple" && v.tuple_values().len() == left_nodes.len())
            {
                values = (0..left_nodes.len())
                    .map(|index| {
                        Value::merge(alternatives.iter().map(|v| v.tuple_values()[index].clone()))
                    })
                    .collect();
            }
        }
        let result = values.first().cloned().unwrap_or_else(Value::unknown);
        for (index, target_node) in left_nodes.into_iter().enumerate() {
            let value = values.get(index).cloned().unwrap_or_else(Value::unknown);
            let kind = self.source.nodes[target_node].kind.clone();
            let name = self
                .source
                .text(Some(target_node))
                .trim_start_matches('$')
                .to_owned();
            let implicit_field = self
                .analyzer
                .program
                .fields
                .contains_key(&(self.owner(), name.clone()))
                && !self.env.contains_key(&name);
            if (is_identifier(&kind) && !implicit_field)
                || matches!(
                    kind.as_str(),
                    "tuple_pattern" | "array_pattern" | "object_pattern" | "pattern_list"
                )
            {
                self.bind(Some(target_node), value, "");
            } else {
                let target = self.expression(Some(target_node), depth + 1, false);
                let operator = self.source.nodes[id]
                    .tokens
                    .iter()
                    .find(|t| matches!(t.as_str(), "+=" | "-="))
                    .cloned();
                let is_event = target.kind == "slot"
                    && self.analyzer.program.events.contains(&(
                        self.analyzer.owner(target.base(), self.source_index),
                        target.key().name.clone(),
                    ));
                if is_event && operator.as_deref() == Some("-=") {
                    self.emit(
                        "remove",
                        target.clone(),
                        value,
                        id,
                        &["delegate_removal_order_unproven"],
                        None,
                    );
                } else {
                    self.emit(
                        "store",
                        target.clone(),
                        value,
                        id,
                        if is_event {
                            &["delegate_invocation_list_membership_required"]
                        } else {
                            &[]
                        },
                        None,
                    );
                }
                if target.kind == "slot"
                    && self.analyzer.kind(target.base(), self.source_index) == "map"
                {
                    let base = if target.base().kind == "slot"
                        && *target.base().key() == Value::entries()
                    {
                        target.base().base()
                    } else {
                        target.base()
                    };
                    self.emit(
                        "store",
                        base.clone().slot(Value::keys()).slot(Value::element()),
                        target.key().clone(),
                        id,
                        &[],
                        None,
                    );
                }
            }
        }
        result
    }
    pub fn branch(&mut self, id: NodeId) {
        let condition = self.source.child(id, &["condition"]);
        let is_let = self.source.language == "rust"
            && condition.is_some_and(|n| {
                matches!(
                    self.source.nodes[n].kind.as_str(),
                    "let_condition" | "let_chain"
                )
            });
        let base_env = self.env.clone();
        let base_slots = self.slots.clone();
        let prior_conditions = self.conditions.clone();
        self.conditions.push(format!(
            "conditional_control:{}:{}",
            self.source.path, self.source.nodes[id].line
        ));
        let mut branch_envs = vec![base_env.clone()];
        let mut branch_slots = vec![base_slots.clone()];
        let mut bound_names = BTreeSet::new();
        if is_let {
            self.conditions
                .push("rust_let_pattern_match_required".into());
            let condition = condition.unwrap();
            let parts = if self.source.nodes[condition].kind == "let_chain" {
                self.source.nodes[condition].children.clone()
            } else {
                vec![condition]
            };
            for part in parts {
                if self.source.nodes[part].kind == "let_condition" {
                    let pattern = self.source.child(part, &["pattern"]);
                    let value = self.expression(self.source.child(part, &["value"]), 0, true);
                    if let Some(pattern) = pattern {
                        // Unknown pattern payloads may not fall back to an outer
                        // variable with the same spelling.
                        for name in self.source.rust_pattern_bindings(pattern) {
                            bound_names.insert(name.clone());
                            self.env.insert(name, Value::unknown());
                        }
                    }
                    self.bind(pattern, value, "");
                } else {
                    self.expression(Some(part), 0, true);
                }
            }
            self.statement(self.source.child(id, &["consequence", "body"]));
            let mut success = self.env.clone();
            for name in &bound_names {
                if let Some(value) = base_env.get(name) {
                    success.insert(name.clone(), value.clone());
                } else {
                    success.remove(name);
                }
            }
            branch_envs.push(success);
            branch_slots.push(self.slots.clone());
            self.env = base_env.clone();
            self.slots = base_slots.clone();
            self.conditions = prior_conditions.clone();
            self.conditions.push("rust_let_pattern_not_matched".into());
            self.statement(self.source.child(id, &["alternative"]));
            branch_envs.push(self.env.clone());
            branch_slots.push(self.slots.clone());
        } else {
            for part in self.source.nodes[id].children.clone() {
                self.env = base_env.clone();
                self.slots = base_slots.clone();
                self.statement(Some(part));
                branch_envs.push(self.env.clone());
                branch_slots.push(self.slots.clone());
            }
        }
        self.env = base_env
            .into_iter()
            .map(|(name, value)| {
                let equal = branch_envs
                    .iter()
                    .all(|env| env.get(&name).unwrap_or(&value) == &value);
                (name, if equal { value } else { Value::unknown() })
            })
            .collect();
        if matches!(self.source.language.as_str(), "php" | "python" | "ruby") {
            for name in branch_envs
                .iter()
                .flat_map(|env| env.keys())
                .cloned()
                .collect::<BTreeSet<_>>()
            {
                let value = Value::merge(
                    branch_envs
                        .iter()
                        .map(|env| env.get(&name).cloned().unwrap_or_else(Value::unknown)),
                );
                self.analyzer
                    .value_conditions
                    .entry(value.clone())
                    .or_default()
                    .insert("branch_definition_required".into());
                self.env.insert(name, value);
            }
        }
        let keys: BTreeSet<_> = branch_slots
            .iter()
            .flat_map(|s| s.keys().cloned())
            .collect();
        self.slots = keys
            .into_iter()
            .map(|key| {
                let value = Value::merge(
                    branch_slots
                        .iter()
                        .map(|slots| slots.get(&key).cloned().unwrap_or_else(Value::unknown)),
                );
                (key, value)
            })
            .collect();
        self.conditions = prior_conditions;
    }
    pub fn record_return(&mut self, value: Value, id: NodeId) {
        if value.kind == "unknown" {
            return;
        }
        if self.source.language == "rust"
            && !self.block_returns.is_empty()
            && !matches!(
                self.source.nodes[id].kind.as_str(),
                "return_expression" | "return_statement"
            )
        {
            self.block_returns.last_mut().unwrap().push((value, id));
            return;
        }
        self.returns.push(value.clone());
        if !matches!(
            self.source.language.as_str(),
            "rust" | "typescript" | "javascript"
        ) {
            return;
        }
        if matches!(self.source.language.as_str(), "typescript" | "javascript")
            && self.analyzer.kind(&value, self.source_index) != "array"
        {
            return;
        }
        if value.kind == "allocation"
            && matches!(self.source.language.as_str(), "typescript" | "javascript")
        {
            let origins: Vec<_> = self
                .facts
                .iter()
                .filter(|f| {
                    f.kind == "store"
                        && f.target == value.clone().slot(Value::element())
                        && f.value.kind == "slot"
                })
                .map(|f| f.value.clone())
                .collect();
            for origin in origins {
                self.emit(
                    "return",
                    origin,
                    Value::unknown(),
                    id,
                    &["returned_array_element_membership_required"],
                    None,
                );
            }
            return;
        }
        let expression = if self.source.nodes[id].kind == "expression_statement" {
            self.source.nodes[id]
                .children
                .first()
                .copied()
                .unwrap_or(id)
        } else {
            id
        };
        let origins = self
            .block_origins
            .get(&expression)
            .cloned()
            .unwrap_or_else(|| vec![(value, id)]);
        for (value, origin) in origins {
            let mut leaves = vec![value];
            while let Some(value) = leaves.pop() {
                match value.kind.as_str() {
                    "tuple" => leaves.extend(value.tuple_values()),
                    "choice" => leaves.extend(value.options()),
                    "wrapper" if value.name == "rust:option" => leaves.push(value.base().clone()),
                    "slot" => self.emit("return", value, Value::unknown(), origin, &[], None),
                    _ => {}
                }
            }
        }
    }
    pub fn finish_block(&mut self, id: NodeId) -> Value {
        let origins = self.block_returns.pop().unwrap_or_default();
        let value = Value::merge(origins.iter().map(|p| p.0.clone()));
        self.block_origins.insert(id, origins);
        value
    }
    pub fn loop_statement(&mut self, id: NodeId) {
        let mut left = self.source.child(id, &["left", "pattern", "name"]);
        let mut right = self.source.child(id, &["right", "value", "iterable"]);
        if let Some(range) = self.source.first(id, &["range_clause"]) {
            left = self.source.child(range, &["left"]);
            right = self.source.child(range, &["right"]);
        }
        let body = self.source.child(id, &["body"]);
        if right.is_none() {
            for part in self.source.nodes[id].children.clone() {
                if Some(part) != body {
                    self.statement(Some(part));
                }
            }
            self.statement(body);
            return;
        }
        let mut container = self.expression(right, 0, true);
        let mode = if container.kind == "iterator" {
            let mode = container.name.clone();
            container = container.base().clone();
            mode
        } else {
            String::new()
        };
        let kind = self.analyzer.kind(&container, self.source_index);
        let old_env = self.env.clone();
        let old_conditions = self.conditions.clone();
        self.conditions.push("iteration_may_be_empty".into());
        if let Some(mut target) = left {
            if matches!(
                self.source.nodes[target].kind.as_str(),
                "lexical_declaration" | "variable_declaration"
            ) {
                if let Some(declaration) = self.source.first(target, &["variable_declarator"]) {
                    target = self.source.child(declaration, &["name"]).unwrap_or(target);
                }
            }
            let parts = if matches!(
                self.source.nodes[target].kind.as_str(),
                "expression_list" | "tuple_pattern" | "array_pattern" | "pattern_list"
            ) {
                self.source.nodes[target].children.clone()
            } else {
                vec![target]
            };
            if kind == "map" && (parts.len() > 1 || self.source.language == "go") {
                if let Some(first) = parts.first() {
                    self.bind(
                        Some(*first),
                        container.clone().slot(Value::keys()).slot(Value::element()),
                        "",
                    );
                }
                if let Some(second) = parts.get(1) {
                    self.bind(
                        Some(*second),
                        container
                            .clone()
                            .slot(Value::entries())
                            .slot(Value::element()),
                        "",
                    );
                }
            } else {
                for target in parts {
                    let value = if kind != "map" {
                        container.clone().slot(Value::element())
                    } else if matches!(mode.as_str(), "values" | "keys") {
                        container
                            .clone()
                            .slot(if mode == "keys" {
                                Value::keys()
                            } else {
                                Value::entries()
                            })
                            .slot(Value::element())
                    } else {
                        Value::unknown()
                    };
                    self.bind(Some(target), value, "");
                }
            }
        }
        self.statement(body);
        self.env = old_env;
        self.conditions = old_conditions;
    }
}
