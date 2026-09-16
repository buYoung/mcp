use super::engine::Interpreter;
use super::model::Value;
use super::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct Jvm {
    pub parents: BTreeMap<String, String>,
    pub interfaces: BTreeMap<String, Vec<String>>,
    pub handles: BTreeMap<(String, String), Value>,
    pub enum_constants: BTreeMap<String, BTreeSet<String>>,
    pub field_modifiers: BTreeMap<(String, String), BTreeSet<String>>,
}
impl Program {
    pub fn collect_jvm(&mut self) {
        let mut declarations = Vec::new();
        for (index, source) in self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| s.language == "java")
        {
            for id in source.walk(0, false) {
                if !matches!(
                    source.nodes[id].kind.as_str(),
                    "class_declaration" | "interface_declaration" | "enum_declaration"
                ) {
                    continue;
                }
                let name = source.text(source.child(id, &["name"]));
                let owner = self.owner_key(index, name);
                declarations.push((index, id, owner.clone()));
                let Some(body) = source.child(id, &["body"]) else {
                    continue;
                };
                for &field in &source.nodes[body].children {
                    if source.nodes[field].kind == "field_declaration" {
                        let modifiers = source
                            .text(source.first(field, &["modifiers"]))
                            .split_whitespace()
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>();
                        for &variable in &source.nodes[field].children {
                            if source.nodes[variable].kind == "variable_declarator" {
                                let name = source.text(source.child(variable, &["name"]));
                                self.jvm
                                    .field_modifiers
                                    .insert((owner.clone(), name.into()), modifiers.clone());
                            }
                        }
                    }
                    if source.nodes[id].kind == "enum_declaration"
                        && source.nodes[field].kind == "enum_constant"
                    {
                        self.jvm
                            .enum_constants
                            .entry(owner.clone())
                            .or_default()
                            .insert(source.text(source.child(field, &["name"])).into());
                    }
                }
            }
        }
        for (index, id, owner) in declarations {
            let source = self.sources[index].clone();
            if let Some(parent) = source
                .child(id, &["superclass"])
                .and_then(|n| source.nodes[n].children.first().copied())
            {
                self.jvm.parents.insert(
                    owner.clone(),
                    self.resolve_type(index, source.text(Some(parent)), ""),
                );
            }
            if let Some(interfaces) = source
                .child(id, &["interfaces"])
                .or_else(|| source.first(id, &["extends_interfaces"]))
            {
                let values = source
                    .walk(interfaces, false)
                    .iter()
                    .filter(|n| source.nodes[**n].kind == "type_identifier")
                    .map(|n| self.resolve_type(index, source.text(Some(*n)), ""))
                    .filter(|v| !v.is_empty())
                    .collect();
                self.jvm.interfaces.insert(owner.clone(), values);
            }
            let Some(body) = source.child(id, &["body"]) else {
                continue;
            };
            for part in source.walk(body, false) {
                if source.nodes[part].kind != "assignment_expression" {
                    continue;
                }
                let left = source.child(part, &["left"]);
                let right = source.child(part, &["right"]);
                let (Some(left), Some(right)) = (left, right) else {
                    continue;
                };
                if source.nodes[left].kind != "identifier"
                    || source.nodes[right].kind != "method_invocation"
                    || source.text(source.child(right, &["name"])) != "findVarHandle"
                {
                    continue;
                }
                let Some(lookup) = source.child(right, &["object"]) else {
                    continue;
                };
                if source.nodes[lookup].kind != "method_invocation"
                    || source.text(source.child(lookup, &["name"])) != "lookup"
                {
                    continue;
                }
                let api = source.text(source.child(lookup, &["object"]));
                if self
                    .imports
                    .get(&(source.path.clone(), api.into()))
                    .map(String::as_str)
                    .unwrap_or(api)
                    != "java.lang.invoke.MethodHandles"
                    || !self.resolve_type(index, api, "").is_empty()
                {
                    continue;
                }
                if source.walk(body, false).iter().any(|n| {
                    source.nodes[*n].kind == "variable_declarator"
                        && source.text(source.child(*n, &["name"])) == api
                }) {
                    continue;
                }
                let Some(args) = source.child(right, &["arguments"]) else {
                    continue;
                };
                let args = &source.nodes[args].children;
                if args.len() != 3
                    || source.nodes[args[0]].kind != "class_literal"
                    || source.nodes[args[1]].kind != "string_literal"
                    || source.nodes[args[2]].kind != "class_literal"
                {
                    continue;
                }
                let target = self.resolve_type(
                    index,
                    source.text(source.nodes[args[0]].children.first().copied()),
                    "",
                );
                let key = source.text(Some(args[1])).trim_matches('"');
                let name = source.text(Some(left));
                let declared = self
                    .fields
                    .get(&(owner.clone(), name.into()))
                    .map(String::as_str)
                    .unwrap_or_default();
                let modifiers = self.jvm.field_modifiers.get(&(owner.clone(), name.into()));
                let actual = self
                    .fields
                    .get(&(target.clone(), key.into()))
                    .map(String::as_str)
                    .unwrap_or_default()
                    .trim_start_matches("java.lang.");
                let requested = source
                    .text(source.nodes[args[2]].children.first().copied())
                    .trim_start_matches("java.lang.");
                if !target.is_empty()
                    && self.fields.contains_key(&(target.clone(), key.into()))
                    && actual == requested
                    && modifiers.is_some_and(|m| m.contains("static") && m.contains("final"))
                    && !self
                        .jvm
                        .field_modifiers
                        .get(&(target.clone(), key.into()))
                        .is_some_and(|m| m.contains("static"))
                    && self
                        .imports
                        .get(&(source.path.clone(), declared.into()))
                        .map(String::as_str)
                        .unwrap_or(declared)
                        == "java.lang.invoke.VarHandle"
                {
                    self.jvm.handles.insert(
                        (owner.clone(), name.into()),
                        Value::nested(
                            "wrapper",
                            "java:VarHandle",
                            Value::new("type", target),
                            Some(Value::new("key", key)),
                        ),
                    );
                }
            }
        }
    }
    pub fn jvm_lineage(&self, owner: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut owner = owner.to_owned();
        while !owner.is_empty() && !result.contains(&owner) && result.len() < 16 {
            result.push(owner.clone());
            owner = self.jvm.parents.get(&owner).cloned().unwrap_or_default();
        }
        result
    }
    pub fn method_candidates(&self, owner: &str, name: &str) -> Vec<usize> {
        if owner.starts_with("scala:") {
            for ancestor in self.scala_ancestors(owner) {
                if let Some(methods) = self.methods.get(&(ancestor, name.into())) {
                    return methods.clone();
                }
            }
            return Vec::new();
        }
        if !owner.starts_with("java:") {
            return self
                .methods
                .get(&(owner.into(), name.into()))
                .cloned()
                .unwrap_or_default();
        }
        let lineage = self.jvm_lineage(owner);
        for parent in &lineage {
            if let Some(methods) = self.methods.get(&(parent.clone(), name.into())) {
                let bodies: Vec<_> = methods
                    .iter()
                    .copied()
                    .filter(|n| self.functions[*n].body.is_some())
                    .collect();
                if !bodies.is_empty() {
                    return bodies;
                }
            }
        }
        let mut pending: Vec<_> = lineage
            .iter()
            .flat_map(|p| self.jvm.interfaces.get(p).into_iter().flatten().cloned())
            .collect();
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        while let Some(interface) = pending.pop() {
            if seen.len() >= 32 || !seen.insert(interface.clone()) {
                continue;
            }
            result.extend(
                self.methods
                    .get(&(interface.clone(), name.into()))
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|n| self.functions[*n].body.is_some()),
            );
            pending.extend(
                self.jvm
                    .interfaces
                    .get(&interface)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        result
    }
    pub fn jvm_binding(&self, owner: &str, name: &str) -> Option<Value> {
        self.jvm_lineage(owner).iter().find_map(|parent| {
            self.jvm
                .handles
                .get(&(parent.clone(), name.into()))
                .cloned()
        })
    }
}
impl Interpreter<'_, '_> {
    pub fn invoke_jvm_handle(
        &mut self,
        handle: &Value,
        method: &str,
        args: &[Value],
        id: NodeId,
    ) -> Option<Value> {
        if handle.kind != "wrapper" || handle.name != "java:VarHandle" || args.is_empty() {
            return None;
        }
        let owner = self.analyzer.owner(&args[0], self.source_index);
        if !self
            .analyzer
            .program
            .jvm_lineage(&owner)
            .contains(&handle.base().name)
        {
            self.analyzer.notices.insert((
                "var_handle_receiver_unproven".into(),
                self.identifier.clone(),
            ));
            return Some(Value::unknown());
        }
        let target = args[0].clone().slot(handle.key().clone());
        if matches!(method, "get" | "getVolatile" | "getAcquire" | "getOpaque") && args.len() == 1 {
            self.analyzer
                .value_conditions
                .entry(target.clone())
                .or_default()
                .insert("qualified_jdk_varhandle_access".into());
            return Some(target);
        }
        if self
            .analyzer
            .program
            .jvm
            .field_modifiers
            .get(&(handle.base().name.clone(), handle.key().name.clone()))
            .is_some_and(|m| m.contains("final"))
        {
            self.analyzer.notices.insert((
                "var_handle_read_only_field".into(),
                handle.base().name.clone(),
            ));
            return Some(Value::unknown());
        }
        if method == "compareAndSet" && args.len() == 3 {
            self.emit(
                "store",
                target,
                args[2].clone(),
                id,
                &[
                    "compare_and_set_success_required",
                    "qualified_jdk_varhandle_access",
                ],
                None,
            );
            return Some(Value::unknown());
        }
        if matches!(method, "set" | "setVolatile" | "setRelease" | "setOpaque") && args.len() == 2 {
            self.emit(
                "store",
                target,
                args[1].clone(),
                id,
                &["qualified_jdk_varhandle_access"],
                None,
            );
            return Some(Value::unknown());
        }
        None
    }
}
