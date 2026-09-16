use super::engine::*;
use super::model::*;
use super::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

impl Interpreter<'_, '_> {
    pub fn function_index(&self, identifier: &str) -> Option<usize> {
        self.analyzer
            .program
            .functions
            .iter()
            .position(|f| f.identifier == identifier)
    }
    pub fn call(&mut self, id: NodeId, depth: usize) -> Value {
        let result = self.evaluate_call(id, depth);
        #[cfg(test)]
        if self.depth == 0
            && self.source.language == "rust"
            && self.analyzer.observed_calls.len() < 40_000
        {
            let node = &self.source.nodes[id];
            self.analyzer.observed_calls.insert(
                (self.source.path.clone(), node.start, node.end),
                (
                    self.source.location(id),
                    self.source.text(Some(id)).into(),
                    result.clone(),
                ),
            );
        }
        result
    }
    fn evaluate_call(&mut self, id: NodeId, depth: usize) -> Value {
        if self.is_polyglot() {
            return self.polyglot_call(id, depth);
        }
        let node = self.source.nodes[id].clone();
        if node.kind == "macro_invocation" {
            self.emit(
                "macro_boundary",
                Value::unknown(),
                Value::unknown(),
                id,
                &[],
                None,
            );
            return Value::unknown();
        }
        let callee_node = self.source.child(id, &["function", "method", "name"]);
        let object_node = self.source.child(id, &["object", "receiver", "scope"]);
        let callee_node = callee_node.or_else(|| node.children.first().copied());
        let mut callee = if let Some(object) = object_node {
            let receiver = self.expression(Some(object), depth + 1, true);
            receiver.field(self.source.text(callee_node).trim_start_matches('$'))
        } else {
            self.expression(callee_node, depth + 1, true)
        };
        let arguments_node = self.source.child(id, &["arguments"]).or_else(|| {
            self.source.first(
                id,
                &[
                    "arguments",
                    "argument_list",
                    "value_arguments",
                    "actual_parameters",
                ],
            )
        });
        let argument_nodes = arguments_node
            .map(|n| self.source.nodes[n].children.clone())
            .unwrap_or_default();
        let arguments: Vec<_> = argument_nodes
            .iter()
            .map(|n| self.expression(Some(*n), depth + 1, true))
            .collect();
        let callee_text = if callee_node.is_some_and(|n| {
            matches!(
                self.source.nodes[n].kind.as_str(),
                "generic_function" | "instantiation_expression"
            )
        }) {
            self.source
                .text(callee_node.and_then(|n| self.source.child(n, &["function"])))
                .to_owned()
        } else {
            self.source.text(callee_node).to_owned()
        };
        let mut bound_receiver = Value::unknown();
        if matches!(self.source.language.as_str(), "typescript" | "javascript") {
            let mut actuals = if matches!(callee.kind.as_str(), "function" | "closure") {
                vec![callee.clone()]
            } else {
                self.analyzer.read_values(&callee)
            };
            actuals.retain(|v| matches!(v.kind.as_str(), "function" | "closure"));
            actuals.sort();
            actuals.dedup();
            if actuals.len() == 1 {
                if callee.kind == "slot" {
                    bound_receiver = callee.base().clone();
                    self.emit("invoke", callee.clone(), Value::unknown(), id, &[], None);
                }
                if actuals[0].kind == "closure" {
                    if let Some(index) = self.function_index(&actuals[0].name) {
                        return self.apply_source(
                            index,
                            bound_receiver,
                            arguments,
                            id,
                            Some(closure_captures(&actuals[0])),
                        );
                    }
                }
                callee = actuals.remove(0);
            }
        }
        if self.source.language == "rust" {
            if let Some(value) = self.rust_call(&callee_text, &callee, &arguments, id) {
                return value;
            }
        }
        if self.source.language == "go"
            && callee_text == "append"
            && !arguments.is_empty()
            && !self.local_bindings.contains("append")
        {
            for value in &arguments[1..] {
                self.emit(
                    "store",
                    arguments[0].clone().slot(Value::element()),
                    value.clone(),
                    id,
                    &[],
                    None,
                );
            }
            self.analyzer
                .kinds
                .insert(arguments[0].clone(), "array".into());
            return arguments[0].clone();
        }
        if self.source.language == "go" && callee_text == "delete" && arguments.len() == 2 {
            self.emit(
                "remove",
                arguments[0]
                    .clone()
                    .slot(Value::entries())
                    .slot(arguments[1].clone()),
                Value::unknown(),
                id,
                &[],
                None,
            );
            return Value::unknown();
        }
        if self.source.language == "go" && callee_text == "make" {
            let value = self.allocation(id, ":make");
            let ty = self.source.text(argument_nodes.first().copied()).to_owned();
            self.analyzer
                .record_type(&value, self.source_index, &ty, "");
            return value;
        }
        let mut receiver = if callee.kind == "slot" {
            callee.base().clone()
        } else {
            bound_receiver.clone()
        };
        let method = if callee.kind == "slot" && callee.key().kind == "key" {
            callee.key().name.clone()
        } else {
            String::new()
        };
        if self.source.language == "go" && !method.is_empty() {
            receiver = self
                .analyzer
                .promoted_receiver(&receiver, &method, self.source_index);
            callee = receiver.clone().field(&method);
        }
        if matches!(self.source.language.as_str(), "typescript" | "javascript") {
            if let Some(value) = self.generator_call(&receiver, &method, &arguments, id) {
                return value;
            }
            if matches!(method.as_str(), "call" | "apply")
                && matches!(receiver.kind.as_str(), "function" | "closure")
                && !arguments.is_empty()
                && !self.analyzer.heap.contains_key(&callee)
            {
                let supplied = if method == "call" {
                    Some(arguments[1..].to_vec())
                } else if arguments.len() == 2
                    && matches!(arguments[1].kind.as_str(), "tuple" | "tuple_end")
                {
                    Some(arguments[1].tuple_values())
                } else {
                    None
                };
                if let (Some(supplied), Some(index)) =
                    (supplied, self.function_index(&receiver.name))
                {
                    return self.apply_source(
                        index,
                        arguments[0].clone(),
                        supplied,
                        id,
                        Some(closure_captures(&receiver)),
                    );
                }
            }
            if method == "defineProperty"
                && self.is_unshadowed_builtin(&receiver, "Object")
                && arguments.len() == 3
            {
                self.emit(
                    "store",
                    arguments[0].clone().slot(arguments[1].clone()),
                    arguments[2].clone().field("value"),
                    id,
                    &["standard_object_property_definition"],
                    None,
                );
                return arguments[0].clone();
            }
            if method == "get"
                && self.is_unshadowed_builtin(&receiver, "Reflect")
                && arguments.len() == 2
                && arguments[1].kind == "key"
            {
                self.conditions
                    .push("builtin_reflect_get_semantics_required".into());
                return arguments[0].clone().slot(arguments[1].clone());
            }
        }
        if self.source.language == "go" {
            if receiver == Value::new("import", "go:reflect")
                && method == "ValueOf"
                && arguments.len() == 1
            {
                return Value::nested("wrapper", "go:reflect.Value", arguments[0].clone(), None);
            }
            if receiver.kind == "wrapper"
                && receiver.name == "go:reflect.Value"
                && method == "Call"
                && arguments.len() == 1
            {
                self.emit(
                    "invoke",
                    receiver.base().clone(),
                    Value::unknown(),
                    id,
                    &["reflect_value_must_be_callable"],
                    None,
                );
                return Value::unknown();
            }
            if callee.kind == "type" && arguments.len() == 1 {
                if let Some((path, name)) = callee.name.rsplit_once("::") {
                    if self
                        .analyzer
                        .program
                        .aliases
                        .get(&(path.into(), name.into()))
                        .is_some_and(|ty| ty.trim_start().starts_with("func"))
                    {
                        self.emit(
                            "function_type_conversion",
                            arguments[0].clone(),
                            callee,
                            id,
                            &[],
                            None,
                        );
                        return arguments[0].clone();
                    }
                }
            }
        }
        if self.source.language == "rust" {
            receiver = self.dereferenced_receiver(receiver, &method, id);
            if !method.is_empty() {
                callee = receiver.clone().field(&method);
            }
        }
        let owner = self.analyzer.owner(&receiver, self.source_index);
        let has_source_method = self
            .analyzer
            .program
            .methods
            .contains_key(&(owner.clone(), method.clone()));
        let kind = self.analyzer.kind(&receiver, self.source_index);
        if !kind.is_empty() && !has_source_method {
            if let Some(value) = self.container_call(&kind, &receiver, &method, &arguments, id) {
                return value;
            }
        }
        for (index, argument) in arguments.iter().enumerate() {
            let declared = self.analyzer.type_text(argument, self.source_index);
            if argument.kind == "slot"
                || (argument.kind == "parameter"
                    && (declared.starts_with("func") || declared.contains("=>")))
            {
                self.emit(
                    "argument",
                    argument.clone(),
                    callee.clone(),
                    id,
                    &[],
                    Some(index),
                );
            }
        }
        let targets = if matches!(callee.kind.as_str(), "function" | "closure") {
            self.function_index(&callee.name)
                .into_iter()
                .collect::<Vec<_>>()
        } else if !method.is_empty() {
            self.analyzer
                .program
                .methods
                .get(&(
                    if receiver.kind == "namespace" {
                        receiver.name.clone()
                    } else {
                        owner
                    },
                    method,
                ))
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if targets.len() == 1 {
            let type_bindings = if self.source.language == "rust" {
                self.rust_call_bindings(targets[0], callee_node, &arguments)
            } else {
                BTreeMap::new()
            };
            let definition = &self.analyzer.program.functions[targets[0]];
            let is_generator = matches!(
                self.analyzer.program.sources[definition.source].nodes[definition.node]
                    .kind
                    .as_str(),
                "generator_function" | "generator_function_declaration"
            );
            let use_actuals = (self.source.language == "rust"
                && receiver.kind == "allocation"
                && self
                    .analyzer
                    .program
                    .tuple_fields
                    .contains_key(&self.analyzer.owner(&receiver, self.source_index)))
                || is_generator
                || !type_bindings.is_empty()
                || !bound_receiver.name.is_empty()
                || arguments.iter().any(|v| {
                    matches!(
                        v.kind.as_str(),
                        "function" | "closure" | "binding" | "wrapper" | "tuple"
                    ) || self
                        .analyzer
                        .read_values(v)
                        .iter()
                        .any(|stored| matches!(stored.kind.as_str(), "function" | "closure"))
                });
            self.next_rust_bindings = type_bindings;
            let returned = if use_actuals {
                self.apply_source(targets[0], receiver.clone(), arguments.clone(), id, None)
            } else {
                self.apply_summary(
                    targets[0],
                    if receiver.kind == "type" {
                        Value::unknown()
                    } else {
                        receiver.clone()
                    },
                    arguments.clone(),
                    id,
                )
            };
            if returned.kind != "unknown" {
                if self.source.language == "rust"
                    && returned
                        .referenced()
                        .iter()
                        .any(|v| matches!(v.kind.as_str(), "unknown" | "unresolved" | "result"))
                {
                    self.unresolved_call(
                        id,
                        callee.clone(),
                        receiver.clone(),
                        arguments.clone(),
                        targets.clone(),
                        "partial_source_return",
                    );
                    if let Some(call) = self.calls.last_mut() {
                        call.return_value = Some(returned.clone());
                    }
                }
                return returned;
            }
            return self.unresolved_call(
                id,
                callee,
                receiver,
                arguments,
                targets,
                "source_return_unresolved",
            );
        }
        if matches!(callee.kind.as_str(), "slot" | "parameter") {
            let declared = self.analyzer.type_text(&callee, self.source_index);
            let is_member = callee_node.is_some_and(|n| is_member(&self.source.nodes[n].kind))
                || object_node.is_some();
            let is_callable_type = declared.starts_with("func(")
                || declared.starts_with("unsafe fn")
                || declared.starts_with("fn(")
                || declared.contains("=>");
            self.emit(
                if is_member && !is_callable_type {
                    "member_invoke"
                } else {
                    "invoke"
                },
                callee.clone(),
                Value::unknown(),
                id,
                &[],
                None,
            );
        } else {
            self.emit(
                "external_boundary",
                callee.clone(),
                Value::unknown(),
                id,
                &[],
                None,
            );
        }
        self.unresolved_call(
            id,
            callee,
            receiver,
            arguments,
            targets,
            "target_unresolved",
        )
    }
    pub fn is_unshadowed_builtin(&self, value: &Value, name: &str) -> bool {
        *value == Value::new("unresolved", format!("{}:{name}", self.source.path))
            && !self.env.contains_key(name)
            && !self.local_bindings.contains(name)
            && !self.source.module_bindings.contains(name)
    }
    pub fn unresolved_call(
        &mut self,
        id: NodeId,
        callee: Value,
        receiver: Value,
        arguments: Vec<Value>,
        candidates: Vec<usize>,
        reason: &str,
    ) -> Value {
        let node = &self.source.nodes[id];
        let value = Value::new(
            "result",
            format!(
                "{}:{}:{}:ast:{}-{}{}",
                self.source.path,
                node.line,
                node.column,
                node.start,
                node.end,
                self.instance_context
            ),
        );
        self.analyzer
            .allocation_owners
            .insert(value.clone(), self.identifier.clone());
        if self.calls.len() < FACTS_PER_FUNCTION {
            let function = self.source.child(id, &["function"]);
            let types = function.and_then(|f| self.source.child(f, &["type_arguments"]));
            let type_arguments = types
                .map(|n| {
                    self.source.nodes[n]
                        .children
                        .iter()
                        .map(|n| self.source.text(Some(*n)).to_owned())
                        .collect()
                })
                .unwrap_or_default();
            let type_bindings = candidates
                .first()
                .map(|target| {
                    self.rust_call_bindings(*target, function, &arguments)
                        .into_iter()
                        .map(|(k, v)| (k, v.display()))
                        .collect()
                })
                .unwrap_or_default();
            let return_types = candidates
                .iter()
                .map(|f| {
                    let function = &self.analyzer.program.functions[*f];
                    self.analyzer.program.sources[function.source]
                        .text(
                            self.analyzer.program.sources[function.source]
                                .child(function.node, &["return_type"]),
                        )
                        .to_owned()
                })
                .collect();
            let candidates: Vec<_> = candidates
                .into_iter()
                .map(|f| self.analyzer.program.functions[f].identifier.clone())
                .collect();
            self.calls.push(UnresolvedCall {
                result: value.clone(),
                callee,
                receiver,
                arguments,
                type_arguments,
                candidates,
                return_types,
                type_bindings,
                reason: reason.into(),
                location: self.source.location(id),
                function: self.identifier.clone(),
                conditions: self.conditions.clone(),
                via: Vec::new(),
                generic_types: types
                    .map(|n| {
                        self.source.nodes[n]
                            .children
                            .iter()
                            .map(|n| self.rust_type_expression(Some(*n)).display())
                            .collect()
                    })
                    .unwrap_or_default(),
                return_value: None,
            });
        } else {
            self.analyzer
                .notices
                .insert(("unresolved_call_record_cap".into(), self.identifier.clone()));
        }
        value
    }
    pub fn apply_summary(
        &mut self,
        target: usize,
        receiver: Value,
        arguments: Vec<Value>,
        id: NodeId,
    ) -> Value {
        if Some(target) == self.function {
            return Value::unknown();
        }
        let Some(summary) = self.analyzer.summaries.get(&target).cloned() else {
            return Value::unknown();
        };
        self.expand_summary(target, summary, receiver, arguments, id)
    }
    pub fn apply_source(
        &mut self,
        target: usize,
        receiver: Value,
        arguments: Vec<Value>,
        id: NodeId,
        captures: Option<Environment>,
    ) -> Value {
        if Some(target) == self.function || self.depth >= 6 {
            self.analyzer
                .notices
                .insert(("callable_context_bound".into(), target.to_string()));
            return Value::unknown();
        }
        let function = self.analyzer.program.functions[target].clone();
        let mut conditions = self.conditions.clone();
        for value in receiver
            .referenced()
            .into_iter()
            .chain(arguments.iter().flat_map(Value::referenced))
        {
            if let Some(required) = self.analyzer.value_conditions.get(value) {
                conditions.extend(required.iter().cloned());
            }
        }
        let mut context = self.instance_context.clone();
        if let Some(identity) = captures
            .as_ref()
            .and_then(|c| c.get("__source_callable_identity__"))
        {
            context.push_str(&identity.display());
        }
        if receiver.kind != "unknown" {
            context.push_str(&receiver.display());
        }
        let instance_context = if context.is_empty() {
            String::new()
        } else {
            format!("~closure:{}", blake3::hash(context.as_bytes()).to_hex())
        };
        for ((name, _), actual) in function.parameters.iter().zip(&arguments) {
            if matches!(
                actual.kind.as_str(),
                "function" | "closure" | "parameter" | "slot"
            ) {
                let is_generator = matches!(
                    self.analyzer.program.sources[function.source].nodes[function.node]
                        .kind
                        .as_str(),
                    "generator_function" | "generator_function_declaration"
                );
                self.emit(
                    "parameter_binding",
                    Value::new("parameter", format!("{}:{name}", function.identifier)),
                    actual.clone(),
                    id,
                    if is_generator {
                        &["generator_execution_required"]
                    } else {
                        &[]
                    },
                    None,
                );
            }
        }
        let generator_identity = self.allocation(id, ":generator");
        let depth = self.depth + 1;
        let rust_bindings = std::mem::take(&mut self.next_rust_bindings);
        let mut nested = Interpreter::new(self.analyzer, function.source, Some(target));
        nested.rust_type_bindings = rust_bindings;
        if !nested.rust_type_bindings.is_empty() {
            nested.conditions.extend([
                "source_generic_type_argument_binding".into(),
                "generic_trait_constraints_unproven".into(),
            ]);
        }
        nested.conditions.extend(conditions);
        nested.depth = depth;
        if let Some(captures) = captures {
            nested.env.extend(captures);
        }
        nested.instance_context = instance_context;
        for ((name, _), actual) in function.parameters.iter().zip(&arguments) {
            nested.env.insert(name.clone(), actual.clone());
        }
        if matches!(nested.source.language.as_str(), "typescript" | "javascript") {
            if let Some(parameters) = nested.source.child(function.node, &["parameters"]) {
                for parameter in nested.source.nodes[parameters].children.clone() {
                    if let Some(pattern) = nested
                        .source
                        .child(parameter, &["pattern", "name"])
                        .filter(|n| nested.source.nodes[*n].kind == "rest_pattern")
                    {
                        let name = nested
                            .source
                            .text(nested.source.nodes[pattern].children.last().copied())
                            .to_owned();
                        let index = function
                            .parameters
                            .iter()
                            .position(|(n, _)| n == &name)
                            .unwrap_or(arguments.len());
                        nested.env.insert(
                            name,
                            Value::tuple(arguments.get(index..).unwrap_or_default()),
                        );
                    }
                }
            }
        }
        if nested.source.nodes[function.node].kind != "arrow_function" {
            nested
                .env
                .insert("arguments".into(), Value::tuple(&arguments));
            if receiver.kind != "unknown" {
                nested.env.insert(function.receiver_name.clone(), receiver);
            }
        }
        if matches!(
            nested.source.nodes[function.node].kind.as_str(),
            "generator_function" | "generator_function_declaration"
        ) {
            return super::generators::create_generator(&mut nested, id, generator_identity);
        }
        if let Some(body) = function.body {
            if is_lambda(&nested.source.nodes[function.node].kind)
                && !is_block(&nested.source.nodes[body].kind)
            {
                let value = nested.expression(Some(body), 0, true);
                nested.record_return(value, body);
            } else {
                nested.statement(Some(body));
            }
        }
        for fact in &mut nested.facts {
            fact.conditions
                .push("source_callable_argument_binding".into());
            fact.conditions.sort();
            fact.conditions.dedup();
        }
        let summary = Summary {
            receiver: None,
            parameters: Vec::new(),
            facts: nested.facts,
            returns: nested.returns,
            calls: nested.calls,
        };
        self.expand_summary(target, summary, Value::unknown(), Vec::new(), id)
    }
    pub fn expand_summary(
        &mut self,
        target: usize,
        summary: Summary,
        receiver: Value,
        arguments: Vec<Value>,
        id: NodeId,
    ) -> Value {
        let target_id = self.analyzer.program.functions[target].identifier.clone();
        let mut replacements: BTreeMap<_, _> = summary
            .parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        replacements.insert(
            Value::new("arguments", &target_id),
            Value::tuple(&arguments),
        );
        if let Some(original) = &summary.receiver {
            if original.kind != "unknown" && receiver.kind != "unknown" {
                replacements.insert(original.clone(), receiver);
            }
        }
        let mut values = BTreeSet::new();
        for value in summary
            .returns
            .iter()
            .chain(summary.facts.iter().flat_map(|f| [&f.target, &f.value]))
        {
            values.extend(value.referenced().into_iter().cloned());
        }
        for call in &summary.calls {
            for value in [&call.result, &call.callee, &call.receiver]
                .into_iter()
                .chain(call.arguments.iter())
            {
                values.extend(value.referenced().into_iter().cloned());
            }
        }
        let location = self.source.location(id);
        for value in values {
            if !matches!(value.kind.as_str(), "allocation" | "binding" | "result")
                || self.analyzer.allocation_owners.get(&value) != Some(&target_id)
            {
                continue;
            }
            let mut context = self
                .analyzer
                .allocation_contexts
                .get(&value)
                .cloned()
                .unwrap_or_default();
            let context_key = (
                location.clone(),
                format!(
                    "{}:ast:{}-{}",
                    self.source.expansion_identity(id),
                    self.source.nodes[id].start,
                    self.source.nodes[id].end
                ),
            );
            if context.contains(&context_key) || context.len() >= 6 {
                replacements.insert(value, Value::unknown());
                self.analyzer
                    .notices
                    .insert(("allocation_context_depth_bound".into(), target_id.clone()));
                continue;
            }
            context.push(context_key);
            let origin = self
                .analyzer
                .allocation_origins
                .get(&value)
                .cloned()
                .unwrap_or_else(|| value.name.clone());
            let suffix = context
                .iter()
                .map(|(l, identity)| format!("{}:{}:{}{}", l.path, l.line, l.column, identity))
                .collect::<Vec<_>>()
                .join(">");
            let allocated = Value::new(
                &value.kind,
                format!(
                    "{origin}@{suffix}:ast:{}-{}",
                    self.source.nodes[id].start, self.source.nodes[id].end
                ),
            );
            self.analyzer
                .allocation_owners
                .insert(allocated.clone(), self.identifier.clone());
            self.analyzer
                .allocation_origins
                .insert(allocated.clone(), origin);
            self.analyzer
                .allocation_contexts
                .insert(allocated.clone(), context);
            if let Some(ty) = self.analyzer.value_types.get(&value).cloned() {
                self.analyzer.value_types.insert(allocated.clone(), ty);
            }
            if let Some(owner) = self.analyzer.owners.get(&value).cloned() {
                self.analyzer.owners.insert(allocated.clone(), owner);
            }
            if let Some(kind) = self.analyzer.kinds.get(&value).cloned() {
                self.analyzer.kinds.insert(allocated.clone(), kind);
            }
            if let Some(conditions) = self.analyzer.value_conditions.get(&value).cloned() {
                self.analyzer
                    .value_conditions
                    .insert(allocated.clone(), conditions);
            }
            replacements.insert(value, allocated);
        }
        for (original, allocated) in &replacements {
            if let Some(prototype) = self.analyzer.prototypes.get(original).cloned() {
                self.analyzer
                    .prototypes
                    .insert(allocated.clone(), prototype.substitute(&replacements));
            }
        }
        for mut call in summary.calls {
            if call.via.len() >= 8 || call.via.contains(&location) {
                continue;
            }
            call.result = call.result.substitute(&replacements);
            call.callee = call.callee.substitute(&replacements);
            call.receiver = call.receiver.substitute(&replacements);
            call.arguments = call
                .arguments
                .iter()
                .map(|v| v.substitute(&replacements))
                .collect();
            call.return_value = call.return_value.map(|v| v.substitute(&replacements));
            call.via.push(location.clone());
            call.conditions.extend(self.conditions.clone());
            call.conditions.sort();
            call.conditions.dedup();
            if self.calls.len() < FACTS_PER_FUNCTION {
                self.calls.push(call);
            }
        }
        for mut fact in summary.facts {
            let is_argument_store = self.is_polyglot()
                && fact.kind == "store"
                && summary
                    .parameters
                    .iter()
                    .any(|p| fact.value.referenced().contains(&p));
            if fact.via.len() >= 8 || fact.via.contains(&location) {
                continue;
            }
            if !matches!(
                fact.kind.as_str(),
                "store"
                    | "remove"
                    | "invoke"
                    | "member_invoke"
                    | "argument"
                    | "return"
                    | "key_lookup"
                    | "function_type_conversion"
                    | "copy_properties"
                    | "parameter_binding"
            ) {
                continue;
            }
            fact.target = fact.target.substitute(&replacements);
            fact.value = fact.value.substitute(&replacements);
            fact.conditions.extend(self.conditions.iter().cloned());
            fact.conditions.sort();
            fact.conditions.dedup();
            fact.via.push(location.clone());
            if is_argument_store {
                let mut registered = fact.clone();
                registered.location = location.clone();
                registered
                    .conditions
                    .push("argument_store_through_summary".into());
                registered.via.push(fact.location.clone());
                self.append_fact(registered);
            }
            self.append_fact(fact);
        }
        Value::merge(summary.returns.iter().map(|v| v.substitute(&replacements)))
    }
    pub fn container_call(
        &mut self,
        kind: &str,
        receiver: &Value,
        method: &str,
        arguments: &[Value],
        id: NodeId,
    ) -> Option<Value> {
        if kind == "map"
            && matches!(method, "get" | "get_mut" | "remove" | "delete" | "fetch")
            && !arguments.is_empty()
        {
            let target = if self.source.language == "rust" {
                self.rust_map_entry(receiver, &arguments[0])
            } else {
                receiver
                    .clone()
                    .slot(Value::entries())
                    .slot(arguments[0].clone())
            };
            if matches!(method, "remove" | "delete") {
                self.emit("remove", target, Value::unknown(), id, &[], None);
                return Some(Value::unknown());
            }
            if matches!(arguments[0].kind.as_str(), "slot" | "parameter") {
                self.emit(
                    "key_lookup",
                    arguments[0].clone(),
                    receiver.clone(),
                    id,
                    &["map_entry_presence_required"],
                    None,
                );
            }
            return Some(if self.source.language == "rust" {
                Value::nested("wrapper", "rust:option", target, None)
            } else {
                target
            });
        }
        if kind == "map"
            && matches!(method, "set" | "insert" | "put" | "putIfAbsent")
            && arguments.len() >= 2
        {
            let target = if self.source.language == "rust" {
                self.rust_map_entry(receiver, &arguments[0])
            } else {
                receiver
                    .clone()
                    .slot(Value::entries())
                    .slot(arguments[0].clone())
            };
            self.emit("store", target, arguments[1].clone(), id, &[], None);
            self.emit(
                "store",
                receiver.clone().slot(Value::keys()).slot(Value::element()),
                arguments[0].clone(),
                id,
                &[],
                None,
            );
            return Some(if method == "set" {
                receiver.clone()
            } else {
                Value::unknown()
            });
        }
        if matches!(kind, "array" | "set")
            && matches!(
                method,
                "push" | "push_back" | "add" | "append" | "insert" | "Add" | "AddLast" | "Append"
            )
            && !arguments.is_empty()
        {
            for value in arguments {
                self.emit(
                    "store",
                    receiver.clone().slot(Value::element()),
                    value.clone(),
                    id,
                    &[],
                    None,
                );
            }
            return Some(
                if method == "add"
                    && matches!(self.source.language.as_str(), "typescript" | "javascript")
                {
                    receiver.clone()
                } else {
                    Value::unknown()
                },
            );
        }
        if kind == "array"
            && method == "pop"
            && arguments.is_empty()
            && matches!(self.source.language.as_str(), "typescript" | "javascript")
        {
            self.conditions.extend([
                "array_nonempty_required".into(),
                "stack_selection_and_order_required".into(),
            ]);
            self.emit(
                "remove",
                receiver.clone().slot(Value::element()),
                Value::unknown(),
                id,
                &[],
                None,
            );
            return Some(receiver.clone().slot(Value::element()));
        }
        if matches!(kind, "array" | "set")
            && matches!(method, "get" | "get_mut" | "at" | "Get")
            && !arguments.is_empty()
        {
            let target = if self.source.language == "rust" && arguments[0].kind == "range" {
                self.conditions
                    .push("slice_range_membership_required".into());
                Value::nested(
                    "wrapper",
                    "rust:slice_view",
                    receiver.clone(),
                    Some(arguments[0].clone()),
                )
            } else {
                receiver.clone().slot(arguments[0].clone())
            };
            return Some(if self.source.language == "rust" {
                Value::nested("wrapper", "rust:option", target, None)
            } else {
                target
            });
        }
        if matches!(
            method,
            "clear"
                | "delete"
                | "remove"
                | "splice"
                | "retain"
                | "pop"
                | "pop_front"
                | "Clear"
                | "Remove"
        ) {
            self.emit(
                "remove",
                receiver.clone().slot(if kind == "map" {
                    Value::entries()
                } else {
                    Value::element()
                }),
                Value::unknown(),
                id,
                &[],
                None,
            );
            return Some(Value::unknown());
        }
        if matches!(
            method,
            "values" | "keys" | "iter" | "iter_mut" | "into_iter" | "iterator" | "GetEnumerator"
        ) {
            return Some(Value::nested(
                "iterator",
                if method == "keys" {
                    "keys"
                } else if method == "values" || kind != "map" {
                    "values"
                } else {
                    "pairs"
                },
                receiver.clone(),
                None,
            ));
        }
        if matches!(
            method,
            "forEach"
                | "foreach"
                | "for_each"
                | "map"
                | "filter"
                | "find"
                | "some"
                | "every"
                | "flatMap"
                | "each"
        ) && !arguments.is_empty()
        {
            let callback = &arguments[0];
            if matches!(callback.kind.as_str(), "function" | "closure") {
                if let Some(target) = self.function_index(&callback.name) {
                    let mut values = vec![receiver.clone().slot(if kind == "map" {
                        Value::entries()
                    } else {
                        Value::element()
                    })];
                    if kind == "map" {
                        values[0] = values[0].clone().slot(Value::element());
                        values.push(receiver.clone().slot(Value::keys()).slot(Value::element()));
                    }
                    let conditions = self.conditions.clone();
                    self.conditions.push("iteration_may_be_empty".into());
                    self.apply_source(
                        target,
                        Value::unknown(),
                        values,
                        id,
                        Some(closure_captures(callback)),
                    );
                    self.conditions = conditions;
                }
            }
            return Some(if method == "filter" {
                receiver.clone()
            } else {
                Value::unknown()
            });
        }
        if matches!(method, "sort" | "sort_by" | "reverse") {
            return Some(receiver.clone());
        }
        if matches!(
            method,
            "has" | "contains" | "contains_key" | "indexOf" | "len" | "is_empty" | "size"
        ) {
            return Some(Value::unknown());
        }
        None
    }
}
