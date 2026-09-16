//! Additional-language syntax and standard-library adapters. Matching remains
//! shared; source declarations take precedence over primitive assumptions.
use super::engine::*;
use super::model::*;
use super::syntax::*;
use std::collections::BTreeSet;

impl Interpreter<'_, '_> {
    pub fn is_polyglot(&self) -> bool {
        !matches!(
            self.source.language.as_str(),
            "javascript" | "typescript" | "go" | "rust" | "assembly"
        )
    }
    fn receiver(&self) -> Value {
        self.env
            .get("this")
            .or_else(|| self.env.get("self"))
            .cloned()
            .unwrap_or_else(|| {
                self.function
                    .map(|f| self.analyzer.program.functions[f].receiver())
                    .unwrap_or_else(Value::unknown)
            })
    }
    pub fn polyglot_call(&mut self, id: NodeId, depth: usize) -> Value {
        let node = self.source.nodes[id].clone();
        let method = if matches!(
            node.kind.as_str(),
            "method_invocation"
                | "member_call_expression"
                | "nullsafe_member_call_expression"
                | "scoped_call_expression"
        ) {
            self.source.child(id, &["name"])
        } else if node.kind == "call" && self.source.language == "ruby" {
            self.source.child(id, &["method"])
        } else {
            None
        };
        let callee = if let Some(method) = method {
            let object = self.source.child(id, &["object", "scope", "receiver"]);
            let receiver = if object.is_some() {
                self.expression(object, depth + 1, true)
            } else {
                self.receiver()
            };
            let name = self.source.text(Some(method));
            if receiver.kind != "unknown" {
                receiver.field(name)
            } else {
                self.binding(name)
            }
        } else {
            let function = self
                .source
                .child(id, &["function", "name"])
                .or_else(|| node.children.first().copied());
            self.expression(function, depth + 1, true)
        };
        let suffix = self.source.first(id, &["call_suffix"]);
        let arguments_node = self
            .source
            .child(id, &["arguments"])
            .or_else(|| self.source.first(id, &["value_arguments"]))
            .or_else(|| {
                suffix.and_then(|n| self.source.first(n, &["value_arguments", "arguments"]))
            });
        let argument_nodes: Vec<_> = arguments_node
            .map(|n| {
                self.source.nodes[n]
                    .children
                    .iter()
                    .copied()
                    .filter(|n| !self.source.nodes[*n].kind.contains("comment"))
                    .collect()
            })
            .unwrap_or_default();
        if self.source.language == "php"
            && argument_nodes.len() == 1
            && self.source.nodes[argument_nodes[0]].kind == "variadic_placeholder"
        {
            self.analyzer
                .value_conditions
                .entry(callee.clone())
                .or_default()
                .insert("first_class_callable_adapter".into());
            self.emit(
                "callable_adapter",
                callee.clone(),
                Value::unknown(),
                id,
                &[],
                None,
            );
            return callee;
        }
        let arguments: Vec<_> = argument_nodes
            .iter()
            .map(|n| {
                self.expression(
                    if self.source.language == "scala"
                        && self.source.nodes[*n].kind == "assignment_expression"
                    {
                        self.source.child(*n, &["right"])
                    } else {
                        Some(*n)
                    },
                    depth + 1,
                    true,
                )
            })
            .collect();
        let block = self
            .source
            .child(id, &["block"])
            .or_else(|| {
                self.source
                    .first(id, &["lambda_literal", "do_block", "block"])
            })
            .or_else(|| {
                self.source
                    .first(id, &["annotated_lambda"])
                    .or_else(|| suffix.and_then(|n| self.source.first(n, &["annotated_lambda"])))
                    .and_then(|n| self.source.first(n, &["lambda_literal"]))
            });
        self.invoke_polyglot(callee, arguments, id, &argument_nodes, block)
    }
    pub fn invoke_polyglot(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        id: NodeId,
        argument_nodes: &[NodeId],
        block: Option<NodeId>,
    ) -> Value {
        if callee.kind == "choice" {
            return Value::merge(
                callee
                    .options()
                    .into_iter()
                    .map(|option| {
                        self.invoke_polyglot(option, arguments.clone(), id, argument_nodes, block)
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if self.source.language == "scala"
            && self.analyzer.kind(&callee, self.source_index) == "array"
            && arguments.len() == 1
        {
            return callee.slot(arguments[0].clone());
        }
        if callee.kind == "closure" {
            if let Some(target) = self.function_index(&callee.name) {
                let conditions = self.conditions.clone();
                if self.source.language == "cpp" {
                    self.conditions
                        .push("closure_capture_binding_required".into());
                }
                let value = self.apply_source(
                    target,
                    Value::unknown(),
                    arguments,
                    id,
                    Some(closure_captures(&callee)),
                );
                self.conditions = conditions;
                return value;
            }
            return Value::unknown();
        }
        if callee.kind == "type"
            && matches!(
                self.source.language.as_str(),
                "swift" | "kotlin" | "scala" | "groovy"
            )
        {
            if self.source.language == "scala" {
                let targets = self
                    .analyzer
                    .program
                    .method_candidates(&callee.name, "apply");
                if targets.len() == 1 {
                    return self.apply_scala(targets[0], callee, arguments, id);
                }
                return self.construct_scala(&callee.name, &arguments, argument_nodes, id, None);
            }
            let value = self.allocation(id, ":instance");
            self.analyzer
                .owners
                .insert(value.clone(), callee.name.clone());
            let name = if self.source.language == "swift" {
                "init"
            } else {
                callee.name.rsplit("::").next().unwrap_or_default()
            };
            if let Some(targets) = self
                .analyzer
                .program
                .methods
                .get(&(callee.name.clone(), name.into()))
                .filter(|v| v.len() == 1)
                .cloned()
            {
                self.apply_summary(targets[0], value.clone(), arguments, id);
            }
            return value;
        }
        if callee.kind == "slot" {
            let mut receiver = callee.base().clone();
            let method = callee.key().name.clone();
            if self.source.language == "scala" && method == "copy" {
                let owner = self.analyzer.owner(&receiver, self.source_index);
                if self.analyzer.program.scala.case_classes.contains(&owner) {
                    return self.construct_scala(
                        &owner,
                        &arguments,
                        argument_nodes,
                        id,
                        Some(&receiver),
                    );
                }
            }
            if let Some(value) = self.invoke_jvm_handle(&receiver, &method, &arguments, id) {
                return value;
            }
            if receiver.kind == "bound_method" {
                return if !method.is_empty() && method == receiver.name {
                    self.invoke_polyglot(
                        receiver.base().clone().slot(receiver.key().clone()),
                        arguments,
                        id,
                        argument_nodes,
                        block,
                    )
                } else {
                    Value::unknown()
                };
            }
            if self.source.language == "csharp" {
                let has_ref = argument_nodes
                    .first()
                    .is_some_and(|n| self.source.text(Some(*n)).trim_start().starts_with("ref "));
                if has_ref
                    && method == "Read"
                    && arguments.len() == 1
                    && self.is_standard_static(&receiver, "System.Threading.Volatile")
                {
                    self.analyzer
                        .value_conditions
                        .entry(arguments[0].clone())
                        .or_default()
                        .insert("snapshot_read_time_unproven".into());
                    return arguments[0].clone();
                }
                if has_ref
                    && method == "CompareExchange"
                    && arguments.len() == 3
                    && self.is_standard_static(&receiver, "System.Threading.Interlocked")
                {
                    self.emit(
                        "store",
                        arguments[0].clone(),
                        arguments[1].clone(),
                        id,
                        &["compare_exchange_success_required"],
                        None,
                    );
                    self.analyzer.notices.insert((
                        "atomic_previous_value_snapshot_unresolved".into(),
                        self.identifier.clone(),
                    ));
                    return Value::new(
                        "result",
                        format!(
                            "{}:atomic_previous:{}",
                            self.source.path, self.source.nodes[id].start
                        ),
                    );
                }
            }
            if self.source.language == "java"
                && method == "invoke"
                && self.has_standard_type(&receiver, "java.lang.reflect.Method")
            {
                self.emit(
                    "invoke",
                    receiver.clone(),
                    arguments.first().cloned().unwrap_or_else(Value::unknown),
                    id,
                    &[
                        "reflective_method_target_unproven",
                        "reflective_dispatch_compatibility_required",
                    ],
                    None,
                );
                for (index, argument) in arguments.iter().enumerate() {
                    self.emit(
                        "argument",
                        argument.clone(),
                        receiver.clone(),
                        id,
                        &["reflective_argument_role"],
                        Some(index),
                    );
                }
                return Value::unknown();
            }
            if self.source.language == "ruby"
                && method == "instance_variable_get"
                && receiver == self.receiver()
                && arguments.len() == 1
                && arguments[0].kind == "key"
                && arguments[0].name.starts_with('@')
                && !self
                    .analyzer
                    .program
                    .functions
                    .iter()
                    .any(|f| f.name == method)
            {
                return receiver.field(arguments[0].name.trim_start_matches('@'));
            }
            if matches!(method.as_str(), "call" | "invoke" | "Invoke")
                && matches!(receiver.kind.as_str(), "slot" | "parameter")
            {
                self.emit(
                    "invoke",
                    receiver,
                    Value::unknown(),
                    id,
                    &["callable_receiver_required"],
                    None,
                );
                return Value::unknown();
            }
            let kind = self.analyzer.kind(&receiver, self.source_index);
            if matches!(kind.as_str(), "map" | "array" | "set") {
                let entries = if kind == "map" {
                    receiver.clone().slot(Value::entries())
                } else {
                    receiver.clone()
                };
                if matches!(
                    method.as_str(),
                    "get" | "GetValueOrDefault" | "getOrDefault"
                ) && !arguments.is_empty()
                {
                    return entries.slot(arguments[0].clone());
                }
                if matches!(method.as_str(), "put" | "set" | "Set" | "Add")
                    && kind == "map"
                    && arguments.len() > 1
                {
                    self.emit(
                        "store",
                        entries.slot(arguments[0].clone()),
                        arguments[1].clone(),
                        id,
                        &[],
                        None,
                    );
                    self.emit(
                        "store",
                        receiver.clone().slot(Value::keys()).slot(Value::element()),
                        arguments[0].clone(),
                        id,
                        &[],
                        None,
                    );
                    return receiver;
                }
                if matches!(
                    method.as_str(),
                    "append" | "push" | "push_back" | "add" | "Add" | "insert" | "Insert"
                ) && !arguments.is_empty()
                {
                    self.emit(
                        "store",
                        entries.slot(Value::element()),
                        arguments.last().unwrap().clone(),
                        id,
                        &[],
                        None,
                    );
                    return receiver;
                }
                if matches!(
                    method.as_str(),
                    "remove" | "Remove" | "delete" | "discard" | "clear" | "Clear" | "removeAll"
                ) {
                    self.emit(
                        "remove",
                        entries.slot(if kind == "map" && !arguments.is_empty() {
                            arguments[0].clone()
                        } else {
                            Value::element()
                        }),
                        Value::unknown(),
                        id,
                        &[],
                        None,
                    );
                    return Value::unknown();
                }
                if matches!(
                    method.as_str(),
                    "values"
                        | "Values"
                        | "items"
                        | "entrySet"
                        | "keys"
                        | "Keys"
                        | "iterator"
                        | "toArray"
                ) {
                    let base = if method.eq_ignore_ascii_case("keys") {
                        receiver.clone().slot(Value::keys())
                    } else {
                        entries
                    };
                    if self.source.language == "scala" && method == "toArray" {
                        let value = self.allocation(id, ":array_copy");
                        self.analyzer.kinds.insert(value.clone(), "array".into());
                        self.emit(
                            "store",
                            value.clone().slot(Value::element()),
                            base.slot(Value::element()),
                            id,
                            &["collection_snapshot_time_unproven"],
                            None,
                        );
                        return value;
                    }
                    return Value::nested("iterator", method, base.slot(Value::element()), None);
                }
                if matches!(
                    method.as_str(),
                    "forEach" | "foreach" | "each" | "map" | "collect"
                ) {
                    let callback = argument_nodes
                        .iter()
                        .copied()
                        .find(|n| is_lambda(&self.source.nodes[*n].kind))
                        .or(block);
                    if let Some(callback) = callback {
                        let first = if kind == "map"
                            && method == "each"
                            && self.source.language == "ruby"
                        {
                            receiver.clone().slot(Value::keys()).slot(Value::element())
                        } else {
                            entries.clone().slot(Value::element())
                        };
                        let second = entries.slot(Value::element());
                        self.inline_polyglot_lambda(callback, first, second);
                    }
                    return Value::unknown();
                }
            }
            if matches!(method.as_str(), "__send__" | "send" | "public_send")
                && self.source.language == "ruby"
                && !arguments.is_empty()
            {
                self.emit(
                    "member_invoke",
                    receiver.clone().slot(arguments[0].clone()),
                    Value::unknown(),
                    id,
                    &["dynamic_method_resolution_required"],
                    None,
                );
            } else {
                self.emit("invoke", callee.clone(), Value::unknown(), id, &[], None);
                if callee.key().kind == "key" {
                    self.emit(
                        "member_invoke",
                        callee.clone(),
                        Value::unknown(),
                        id,
                        &["method_dispatch_unproven"],
                        None,
                    );
                }
            }
            let owner = self.analyzer.owner(&receiver, self.source_index);
            let mut targets = self.analyzer.program.method_candidates(&owner, &method);
            if owner.starts_with("java:") && targets.is_empty() {
                let actuals: BTreeSet<_> = self
                    .analyzer
                    .resolved_aliases(&receiver)
                    .into_iter()
                    .filter(|v| {
                        v.kind == "allocation"
                            && !self
                                .analyzer
                                .program
                                .method_candidates(
                                    &self.analyzer.owner(v, self.source_index),
                                    &method,
                                )
                                .is_empty()
                    })
                    .collect();
                if actuals.len() == 1 {
                    receiver = actuals.into_iter().next().unwrap();
                    targets = self.analyzer.program.method_candidates(
                        &self.analyzer.owner(&receiver, self.source_index),
                        &method,
                    );
                }
            }
            if targets.len() > 1 && self.source.language == "scala" {
                let matching: Vec<_> = targets
                    .iter()
                    .copied()
                    .filter(|index| {
                        let f = &self.analyzer.program.functions[*index];
                        let implicit = self
                            .analyzer
                            .program
                            .scala
                            .implicit_names
                            .get(index)
                            .map_or(0, |s| s.len());
                        f.parameters.len().saturating_sub(implicit) <= arguments.len()
                            && arguments.len() <= f.parameters.len()
                    })
                    .collect();
                if matching.len() == 1 {
                    targets = matching;
                }
            }
            if targets.len() == 1 {
                let value = if self.source.language == "scala"
                    || (self.source.language == "java"
                        && self
                            .analyzer
                            .program
                            .sources
                            .iter()
                            .any(|s| s.language == "scala"))
                {
                    self.apply_scala(targets[0], receiver, arguments.clone(), id)
                } else {
                    self.apply_summary(targets[0], receiver, arguments.clone(), id)
                };
                if value.kind != "unknown" {
                    return value;
                }
            }
        } else if callee.kind == "function" {
            if let Some(target) = self.function_index(&callee.name) {
                let value = if self.source.language == "scala" {
                    self.apply_scala(target, self.receiver(), arguments.clone(), id)
                } else {
                    self.apply_summary(target, self.receiver(), arguments.clone(), id)
                };
                if value.kind != "unknown" {
                    return value;
                }
            }
        } else {
            let name = callee.name.rsplit(':').next().unwrap_or_default();
            if self.source.language == "python"
                && name == "getattr"
                && callee.kind == "unresolved"
                && !self.env.contains_key(name)
                && !self.local_bindings.contains(name)
                && !self.source.module_bindings.contains(name)
                && arguments.len() == 2
                && arguments[1].kind == "key"
            {
                return arguments[0].clone().slot(arguments[1].clone());
            }
            if matches!(name, "pairs" | "ipairs" | "iter" | "enumerate") && !arguments.is_empty() {
                let base = if name == "pairs" {
                    arguments[0].clone().slot(Value::keys())
                } else {
                    arguments[0].clone()
                };
                return Value::nested(
                    "iterator",
                    if name == "pairs" { "keys" } else { "values" },
                    base.slot(Value::element()),
                    None,
                );
            }
            let kind = type_kind(name);
            if !kind.is_empty()
                || matches!(
                    name,
                    "list" | "dict" | "set" | "defaultdict" | "mutableListOf" | "mutableSetOf"
                )
            {
                let value = self.allocation(id, ":container");
                self.analyzer.kinds.insert(
                    value.clone(),
                    if kind.is_empty() {
                        if matches!(name, "dict" | "defaultdict") {
                            "map"
                        } else {
                            "array"
                        }
                        .into()
                    } else {
                        kind
                    },
                );
                if self.source.language == "scala" && name == "Array" {
                    for argument in &arguments {
                        self.emit(
                            "store",
                            value.clone().slot(Value::element()),
                            argument.clone(),
                            id,
                            &[],
                            None,
                        );
                    }
                }
                return value;
            }
            self.emit("invoke", callee.clone(), Value::unknown(), id, &[], None);
        }
        for (index, argument) in arguments.iter().enumerate() {
            if argument.kind == "slot" {
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
        Value::new(
            "result",
            format!("{}:{}", self.source.path, self.source.nodes[id].start),
        )
    }
    pub fn is_standard_static(&self, receiver: &Value, qualified: &str) -> bool {
        let (root, keys) = receiver.split_path();
        if root.kind != "unresolved" || keys.iter().any(|v| v.kind != "key") {
            return false;
        }
        let name = root.name.rsplit(':').next().unwrap_or_default();
        if self.env.contains_key(name) || self.local_bindings.contains(name) {
            return false;
        }
        let Some((namespace, short)) = qualified.rsplit_once('.') else {
            return false;
        };
        if self.analyzer.program.types.keys().any(|(path, n)| {
            n == short
                && (path == &self.source.path
                    || self
                        .analyzer
                        .program
                        .namespaces
                        .get(path)
                        .is_some_and(|ns| ns == namespace))
        }) {
            return false;
        }
        let spelling = std::iter::once(name)
            .chain(keys.iter().map(|k| k.name.as_str()))
            .collect::<Vec<_>>()
            .join(".");
        spelling == qualified
            || (spelling == short
                && self
                    .analyzer
                    .program
                    .namespace_imports
                    .get(&self.source.path)
                    .is_some_and(|ns| ns.contains(namespace)))
    }
    pub fn has_standard_type(&self, value: &Value, qualified: &str) -> bool {
        let (source, ty, scope) = self.analyzer.type_info(value, self.source_index, 0);
        let spelling = self
            .analyzer
            .program
            .expanded_type(source, &ty)
            .trim_end_matches('?')
            .to_owned();
        let owner = self
            .analyzer
            .program
            .resolve_type(source, &spelling, &scope);
        if self.analyzer.program.owner_sources.contains_key(&owner) {
            return false;
        }
        spelling == qualified
            || self
                .analyzer
                .program
                .imports
                .get(&(self.analyzer.program.sources[source].path.clone(), spelling))
                .is_some_and(|name| name == qualified)
    }
    pub fn java_sam_method(&self, type_text: &str) -> String {
        let name = type_text.split('<').next().unwrap_or_default().trim();
        let owner = self
            .analyzer
            .program
            .resolve_type(self.source_index, name, &self.owner());
        if !owner.is_empty() {
            return self
                .analyzer
                .program
                .sam_methods
                .get(&owner)
                .cloned()
                .unwrap_or_default();
        }
        let import = self
            .analyzer
            .program
            .imports
            .get(&(self.source.path.clone(), name.into()));
        if name == "java.lang.Runnable"
            || (name == "Runnable" && import.is_none_or(|i| i == "java.lang.Runnable"))
        {
            "run".into()
        } else {
            String::new()
        }
    }
    pub fn inline_polyglot_lambda(&mut self, id: NodeId, value: Value, second: Value) {
        let params = self.source.child(id, &["parameters"]).or_else(|| {
            self.source
                .first(id, &["lambda_parameters", "block_parameters"])
        });
        let names: Vec<_> = params
            .map(|n| {
                self.source
                    .walk(n, false)
                    .into_iter()
                    .filter(|n| is_identifier(&self.source.nodes[*n].kind))
                    .map(|n| self.source.text(Some(n)).to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let previous = self.env.clone();
        if let Some(name) = names.first() {
            self.env.insert(name.clone(), value);
        } else {
            self.env.insert("it".into(), value.clone());
            self.env.insert("$0".into(), value);
        }
        if let Some(name) = names.get(1) {
            self.env.insert(name.clone(), second);
        }
        let body = self.source.child(id, &["body"]).or_else(|| {
            self.source
                .first(id, &["statements", "block", "body_statement"])
        });
        if body.is_some() {
            self.statement(body);
        } else {
            for child in self.source.nodes[id].children.clone() {
                if Some(child) != params {
                    self.statement(Some(child));
                }
            }
        }
        self.env = previous;
    }
}
