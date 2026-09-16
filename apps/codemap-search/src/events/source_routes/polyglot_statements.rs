use super::engine::*;
use super::model::*;
use super::syntax::*;

impl Interpreter<'_, '_> {
    pub fn polyglot_bind(&mut self, id: NodeId, mut value: Value, type_text: &str) -> bool {
        let node = self.source.nodes[id].clone();
        if self.source.text(Some(id)) == "_" {
            return true;
        }
        if value.kind == "bound_method" && !type_text.is_empty() {
            value.name = self.java_sam_method(type_text);
        }
        if matches!(node.kind.as_str(), "tuple_pattern" | "record_pattern") {
            let patterns: Vec<_> = node
                .children
                .iter()
                .copied()
                .filter(|n| !self.source.nodes[*n].kind.contains("comment"))
                .collect();
            let values = value.tuple_values();
            if patterns.len() > 8 || (!values.is_empty() && values.len() != patterns.len()) {
                self.analyzer.notices.insert((
                    "tuple_assignment_shape_unresolved".into(),
                    self.identifier.clone(),
                ));
                return true;
            }
            for (index, mut pattern) in patterns.into_iter().enumerate() {
                if matches!(
                    self.source.nodes[pattern].kind.as_str(),
                    "constant_pattern" | "variable_pattern"
                ) && self.source.nodes[pattern].children.len() == 1
                {
                    pattern = self.source.nodes[pattern].children[0];
                }
                self.bind(
                    Some(pattern),
                    values.get(index).cloned().unwrap_or_else(|| {
                        value
                            .clone()
                            .slot(Value::new("tuple_index", index.to_string()))
                    }),
                    "",
                );
            }
            return true;
        }
        if is_identifier(&node.kind)
            || matches!(
                node.kind.as_str(),
                "pattern" | "variable_declaration" | "parameter" | "directly_assignable_expression"
            )
        {
            if let Some(named) = self.source.identifier(Some(id)) {
                let name = self
                    .source
                    .text(Some(named))
                    .trim_start_matches('$')
                    .to_owned();
                let owner = self.owner();
                if matches!(
                    self.source.language.as_str(),
                    "cpp" | "csharp" | "dart" | "groovy" | "java" | "kotlin" | "scala" | "swift"
                ) && self
                    .analyzer
                    .program
                    .fields
                    .contains_key(&(owner.clone(), name.clone()))
                    && !self.env.contains_key(&name)
                {
                    self.emit(
                        "store",
                        Value::new("receiver", owner).field(&name),
                        value,
                        id,
                        &[],
                        None,
                    );
                } else {
                    self.analyzer
                        .record_type(&value, self.source_index, type_text, &owner);
                    let declared =
                        self.analyzer
                            .program
                            .resolve_type(self.source_index, type_text, &owner);
                    if !declared.is_empty()
                        && type_text.contains('*')
                        && matches!(self.source.language.as_str(), "c" | "cpp")
                    {
                        self.env
                            .insert(name.clone(), Value::new("receiver", &declared));
                        self.declared_receivers.insert(name, declared);
                    } else {
                        self.env.insert(name, value);
                    }
                }
                return true;
            }
        }
        false
    }
    pub fn polyglot_statement(&mut self, id: NodeId) -> bool {
        let node = self.source.nodes[id].clone();
        let kind = node.kind.as_str();
        if is_function(kind) || is_class(kind) || kind.contains("comment") {
            return true;
        }
        if is_assignment(kind) {
            self.polyglot_assignment(id, 0);
            return true;
        }
        if self.source.language == "java" && kind == "explicit_constructor_invocation" {
            if self.source.text(self.source.child(id, &["constructor"])) == "super" {
                if let Some(parent) = self
                    .analyzer
                    .program
                    .jvm
                    .parents
                    .get(&self.owner())
                    .cloned()
                {
                    let nodes = self
                        .source
                        .child(id, &["arguments"])
                        .map(|n| self.source.nodes[n].children.clone())
                        .unwrap_or_default();
                    let values = nodes
                        .iter()
                        .map(|n| self.expression(Some(*n), 0, true))
                        .collect();
                    let targets = self
                        .analyzer
                        .program
                        .method_candidates(&parent, parent.rsplit("::").next().unwrap_or_default());
                    if targets.len() == 1 {
                        let receiver = self
                            .env
                            .get("this")
                            .cloned()
                            .unwrap_or_else(|| Value::new("receiver", self.owner()));
                        self.apply_scala(targets[0], receiver, values, id);
                    }
                }
            }
            return true;
        }
        if matches!(self.source.language.as_str(), "scala" | "java")
            && matches!(kind, "match_expression" | "switch_expression")
        {
            let value = self.expression(Some(id), 0, true);
            if value.kind != "unknown" {
                self.returns.push(value);
            }
            return true;
        }
        if matches!(self.source.language.as_str(), "scala" | "java")
            && matches!(kind, "if_expression" | "if_statement")
        {
            let condition = self.expression(self.source.child(id, &["condition"]), 0, true);
            let consequence = self.source.child(id, &["consequence", "body"]);
            let alternative = self.source.child(id, &["alternative"]);
            if consequence.is_some() {
                let previous = self.conditions.clone();
                self.conditions.push(format!(
                    "source_branch_condition_required:{}:{}",
                    self.source.path, node.line
                ));
                if condition == Value::new("literal", "true") {
                    self.statement(consequence);
                } else if condition == Value::new("literal", "false") {
                    self.statement(alternative);
                } else {
                    self.statement(consequence);
                    self.statement(alternative);
                }
                self.conditions = previous;
                return true;
            }
        }
        if matches!(
            kind,
            "return_statement"
                | "return_expression"
                | "yield"
                | "yield_expression"
                | "yield_statement"
        ) {
            for item in node.children {
                let value = self.expression(Some(item), 0, true);
                if value.kind != "unknown" {
                    self.returns.push(if kind.starts_with("yield") {
                        Value::nested("iterator", "yield", value, None)
                    } else {
                        value
                    });
                }
            }
            return true;
        }
        if kind == "arrow_expression_clause" {
            let value = self.expression(node.children.last().copied(), 0, true);
            self.returns.push(value);
            return true;
        }
        if self.source.language == "swift"
            && kind == "control_transfer_statement"
            && node.tokens.contains("return")
        {
            let value = self.expression(
                self.source
                    .child(id, &["result"])
                    .or_else(|| node.children.last().copied()),
                0,
                true,
            );
            self.returns.push(value);
            return true;
        }
        if is_loop(kind) {
            self.polyglot_loop(id);
            return true;
        }
        if is_declaration(kind) {
            if kind == "local_variable_declaration"
                && self.source.language == "dart"
                && node
                    .children
                    .iter()
                    .any(|n| self.source.nodes[*n].kind == "initialized_variable_definition")
            {
                for child in node.children {
                    if self.source.nodes[child].kind == "initialized_variable_definition" {
                        self.statement(Some(child));
                    }
                }
                return true;
            }
            if kind == "local_variable_declaration" && self.source.language == "java" {
                for part in node.children {
                    if self.source.nodes[part].kind == "variable_declarator" {
                        let value = self.expression(self.source.child(part, &["value"]), 0, true);
                        let ty = self
                            .source
                            .text(self.source.child(id, &["type"]))
                            .to_owned();
                        self.bind(self.source.child(part, &["name"]), value, &ty);
                    }
                }
                return true;
            }
            if kind == "local_variable_declaration" {
                if let Some(pattern) = self.source.first(id, &["pattern_variable_declaration"]) {
                    self.statement(Some(pattern));
                    return true;
                }
            }
            let mut pattern = self.source.child(id, &["name", "pattern"]).or_else(|| {
                self.source.first(
                    id,
                    &["variable_declaration", "identifier", "simple_identifier"],
                )
            });
            if kind == "pattern_variable_declaration" {
                pattern = self.source.first(id, &["record_pattern"]);
            }
            let value_node = self.source.child(id, &["value"]).or_else(|| {
                node.children
                    .last()
                    .copied()
                    .filter(|n| Some(*n) != pattern)
            });
            if value_node.is_some() {
                let value = self.expression(value_node, 0, true);
                let ty = self
                    .source
                    .text(self.source.child(id, &["type"]))
                    .to_owned();
                self.bind(pattern, value, &ty);
            } else {
                for part in node.children {
                    if Some(part) != pattern {
                        self.statement(Some(part));
                    }
                }
            }
            return true;
        }
        if kind == "declaration" && matches!(self.source.language.as_str(), "c" | "cpp") {
            let ty = self
                .source
                .text(self.source.child(id, &["type"]))
                .to_owned();
            for part in node.children {
                if matches!(
                    self.source.nodes[part].kind.as_str(),
                    "init_declarator" | "pointer_declarator" | "identifier"
                ) {
                    let named = self
                        .source
                        .identifier(self.source.child(part, &["declarator"]).or(Some(part)));
                    let name = self.source.text(named).to_owned();
                    if name.is_empty() {
                        continue;
                    }
                    if self.function.is_none()
                        && self
                            .analyzer
                            .program
                            .global_values
                            .contains_key(&(self.source.path.clone(), name.clone()))
                    {
                        continue;
                    }
                    let mut value = self.expression(self.source.child(part, &["value"]), 0, true);
                    let owner =
                        self.analyzer
                            .program
                            .resolve_type(self.source_index, &ty, &self.owner());
                    if !owner.is_empty() && self.source.text(Some(part)).contains('*') {
                        value = Value::new("receiver", &owner);
                        self.declared_receivers.insert(name.clone(), owner);
                    }
                    self.env.insert(name, value);
                }
            }
            return true;
        }
        if is_call(kind) || matches!(kind, "expression_statement" | "expression_list") {
            self.expression(Some(id), 0, true);
            return true;
        }
        if is_block(kind)
            && matches!(
                self.source.language.as_str(),
                "scala" | "swift" | "kotlin" | "ruby"
            )
        {
            for part in &node.children {
                self.statement(Some(*part));
            }
            if let Some(tail) = node
                .children
                .iter()
                .rev()
                .copied()
                .find(|n| !self.source.nodes[*n].kind.contains("comment"))
            {
                if !is_assignment(&self.source.nodes[tail].kind)
                    && !is_function(&self.source.nodes[tail].kind)
                {
                    let value = self.expression(Some(tail), 0, true);
                    if value.kind != "unknown" {
                        self.returns.push(value);
                    }
                }
            }
            return true;
        }
        false
    }
    pub fn polyglot_assignment(&mut self, id: NodeId, depth: usize) -> Value {
        let node = self.source.nodes[id].clone();
        let mut left = self
            .source
            .child(id, &["left", "target"])
            .or_else(|| node.children.first().copied());
        let mut right = self
            .source
            .child(id, &["right", "result", "value"])
            .or_else(|| node.children.last().copied().filter(|n| Some(*n) != left));
        if left.is_some_and(|n| self.source.nodes[n].kind == "variable_list") {
            left = left.and_then(|n| self.source.nodes[n].children.first().copied());
        }
        if left.is_some_and(|n| {
            self.source.nodes[n].kind == "directly_assignable_expression"
                && self.source.nodes[n].children.len() == 1
        }) {
            left = left.and_then(|n| self.source.nodes[n].children.first().copied());
        }
        if right.is_some_and(|n| {
            self.source.nodes[n].kind == "expression_list"
                && self.source.nodes[n].children.len() == 1
        }) {
            right = right.and_then(|n| self.source.nodes[n].children.first().copied());
        }
        if self.source.language == "php" && node.kind == "reference_assignment_expression" {
            let value = self.expression(right, depth + 1, false);
            let name = self.source.text(left).trim_start_matches('$').to_owned();
            self.references.insert(name.clone(), value.clone());
            self.env.insert(name, value.clone());
            return value;
        }
        let value = self.expression(right, depth + 1, true);
        let Some(left) = left else {
            return value;
        };
        if self.source.nodes[left].kind == "tuple_expression" {
            let Some(targets) = self.polyglot_tuple_parts(left) else {
                return value;
            };
            let values = value.tuple_values();
            if targets.len() != values.len() || values.len() > 8 {
                self.analyzer.notices.insert((
                    "tuple_assignment_shape_unresolved".into(),
                    self.identifier.clone(),
                ));
                return value;
            }
            for (target, value) in targets.into_iter().zip(values) {
                if self.source.text(Some(target)) == "_" {
                    continue;
                }
                if is_identifier(&self.source.nodes[target].kind) {
                    self.bind(Some(target), value, "");
                } else {
                    let target = self.expression(Some(target), depth + 1, false);
                    self.emit("store", target, value, id, &[], None);
                }
            }
            return value;
        }
        let target = self.expression(Some(left), depth + 1, false);
        let name = self
            .source
            .text(Some(left))
            .trim_start_matches('$')
            .to_owned();
        let operator = node
            .tokens
            .iter()
            .find(|op| matches!(op.as_str(), "+=" | "-=" | "<<" | "||="))
            .map(String::as_str)
            .unwrap_or("=");
        if self.source.language == "php" {
            if let Some(reference) = self.references.get(&name).cloned() {
                self.emit(
                    "store",
                    reference,
                    value.clone(),
                    id,
                    &["reference_binding_required"],
                    None,
                );
                return value;
            }
        }
        if target.kind == "slot"
            && target.base().kind == "receiver"
            && self
                .analyzer
                .program
                .events
                .contains(&(target.base().name.clone(), target.key().name.clone()))
            && matches!(operator, "+=" | "-=")
        {
            self.emit(
                if operator == "+=" { "store" } else { "remove" },
                target.clone(),
                value,
                id,
                &["multicast_event_membership_required"],
                None,
            );
            return target;
        }
        if matches!(operator, "+=" | "-=" | "<<")
            && matches!(
                self.analyzer.kind(&target, self.source_index).as_str(),
                "array" | "set"
            )
        {
            self.emit(
                if operator == "-=" { "remove" } else { "store" },
                target.clone().slot(Value::element()),
                value,
                id,
                &["collection_operator"],
                None,
            );
            return target;
        }
        if is_identifier(&self.source.nodes[left].kind)
            && (!self.env.contains_key(&name) && target.kind != "slot"
                || self.env.contains_key(&name))
        {
            let bound = if matches!(value.kind.as_str(), "unknown" | "result" | "unresolved") {
                self.declared_receivers
                    .get(&name)
                    .map(|owner| Value::new("receiver", owner))
                    .unwrap_or_else(|| value.clone())
            } else {
                value.clone()
            };
            self.env.insert(name, bound);
        } else if target.kind == "slot" {
            self.emit("store", target.clone(), value.clone(), id, &[], None);
            if value.kind == "literal" && matches!(value.name.as_str(), "nil" | "null" | "None") {
                self.emit("remove", target.clone(), Value::unknown(), id, &[], None);
            }
            let kind = self.analyzer.kind(&value, self.source_index);
            if !kind.is_empty() {
                self.analyzer.kinds.insert(target.clone(), kind);
            }
            if target.base().kind == "slot" && *target.base().key() == Value::entries() {
                self.emit(
                    "store",
                    target
                        .base()
                        .base()
                        .clone()
                        .slot(Value::keys())
                        .slot(Value::element()),
                    target.key().clone(),
                    id,
                    &["map_key_role"],
                    None,
                );
            }
        }
        value
    }
    pub fn polyglot_loop(&mut self, id: NodeId) {
        let node = self.source.nodes[id].clone();
        let mut right = self
            .source
            .child(id, &["right", "value", "collection", "iterable"]);
        let mut pattern = self.source.child(id, &["left", "name", "pattern"]);
        let mut body = self.source.child(id, &["body"]).or_else(|| {
            self.source
                .first(id, &["block", "compound_statement", "statements"])
        });
        let mut is_reference = false;
        if self.source.language == "php"
            && node.kind == "foreach_statement"
            && node.children.len() >= 3
        {
            right = Some(node.children[0]);
            pattern = Some(node.children[1]);
            body = node.children.last().copied();
            if pattern.is_some_and(|n| self.source.nodes[n].kind == "pair") {
                pattern = pattern.and_then(|n| self.source.nodes[n].children.last().copied());
            }
            if pattern.is_some_and(|n| self.source.nodes[n].kind == "by_ref") {
                is_reference = true;
                pattern = pattern.and_then(|n| self.source.nodes[n].children.last().copied());
            }
        }
        if self.source.language == "lua" {
            let clause = self.source.first(id, &["for_generic_clause"]).unwrap_or(id);
            pattern = self
                .source
                .first(clause, &["variable_list"])
                .and_then(|n| self.source.nodes[n].children.first().copied());
            right = self
                .source
                .first(clause, &["expression_list"])
                .and_then(|n| self.source.nodes[n].children.first().copied());
        }
        if right.is_some() && pattern.is_some() {
            let source = self.expression(right, 0, true);
            let mut alternatives = Vec::new();
            for option in source.options() {
                if option.kind == "iterator" {
                    alternatives.push(option.base().clone());
                } else {
                    let base = if self.analyzer.kind(&option, self.source_index) == "map" {
                        option.slot(Value::keys())
                    } else {
                        option
                    };
                    alternatives.push(base.slot(Value::element()));
                }
            }
            let value = Value::merge(alternatives);
            self.bind(pattern, value.clone(), "");
            if is_reference {
                self.references.insert(
                    self.source.text(pattern).trim_start_matches('$').into(),
                    value,
                );
            }
        }
        for item in node.children {
            if Some(item) != right && Some(item) != pattern && Some(item) != body {
                self.statement(Some(item));
            }
        }
        self.statement(body);
    }
}
