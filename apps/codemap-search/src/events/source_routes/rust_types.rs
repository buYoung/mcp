use super::engine::Interpreter;
use super::model::Value;
use super::syntax::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RustType {
    pub kind: String,
    pub name: String,
    pub arguments: Vec<RustType>,
    pub scope: String,
}
impl RustType {
    pub(super) fn named(kind: &str, name: &str) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            arguments: Vec::new(),
            scope: String::new(),
        }
    }
    pub fn is_concrete(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "nominal" | "primitive" | "application" | "tuple" | "reference" | "pointer"
        ) && self.arguments.iter().all(Self::is_concrete)
    }
    pub fn display(&self) -> String {
        let args = self
            .arguments
            .iter()
            .map(Self::display)
            .collect::<Vec<_>>()
            .join(",");
        match self.kind.as_str() {
            "variable" => format!("{}@{{{}}}", self.name, self.scope),
            "application" => format!("{}<{args}>", self.name),
            "tuple" => format!(
                "({args}{})",
                if self.arguments.len() == 1 { "," } else { "" }
            ),
            "reference" | "pointer" | "projection" => format!("{}({args})", self.name),
            _ => self.name.clone(),
        }
    }
}
pub(crate) fn scoped_parameters(source: &Source, id: Option<NodeId>) -> BTreeMap<String, RustType> {
    let mut node = id;
    let mut result = BTreeMap::new();
    while let Some(id) = node {
        if let Some(params) = source.child(id, &["type_parameters"]) {
            for &param in &source.nodes[params].children {
                if matches!(
                    source.nodes[param].kind.as_str(),
                    "type_parameter" | "const_parameter"
                ) {
                    let name = source.text(source.child(param, &["name"]));
                    let ty = RustType {
                        kind: "variable".into(),
                        name: name.into(),
                        arguments: Vec::new(),
                        scope: format!(
                            "{}:{}-{}",
                            source.path, source.nodes[id].start, source.nodes[id].end
                        ),
                    };
                    result.entry(name.into()).or_insert(ty);
                }
            }
        }
        node = source.nodes[id].parent;
    }
    result
}
impl Interpreter<'_, '_> {
    pub fn rust_type_expression(&self, node: Option<NodeId>) -> RustType {
        let Some(id) = node else {
            return RustType::named("unresolved", "<missing>");
        };
        let node = &self.source.nodes[id];
        let text = self.source.text(Some(id));
        match node.kind.as_str() {
            "generic_type" => {
                let base = self.rust_type_expression(self.source.child(id, &["type"]));
                let args = self.source.child(id, &["type_arguments"]);
                let arguments = args
                    .map(|n| {
                        self.source.nodes[n]
                            .children
                            .iter()
                            .filter(|p| self.source.nodes[**p].kind != "lifetime")
                            .map(|p| self.rust_type_expression(Some(*p)))
                            .collect()
                    })
                    .unwrap_or_default();
                RustType {
                    kind: if base.kind == "nominal" {
                        "application"
                    } else {
                        "unresolved_application"
                    }
                    .into(),
                    name: base.name,
                    arguments,
                    scope: base.scope,
                }
            }
            "tuple_type" | "unit_type" => RustType {
                kind: "tuple".into(),
                name: String::new(),
                arguments: node
                    .children
                    .iter()
                    .map(|n| self.rust_type_expression(Some(*n)))
                    .collect(),
                scope: String::new(),
            },
            "reference_type" | "pointer_type" => RustType {
                kind: if node.kind == "reference_type" {
                    "reference"
                } else {
                    "pointer"
                }
                .into(),
                name: if text.trim_start().starts_with("&mut") {
                    "&mut"
                } else if text.trim_start().starts_with('&') {
                    "&"
                } else if text.trim_start().starts_with("*mut") {
                    "*mut"
                } else {
                    "*const"
                }
                .into(),
                arguments: vec![self.rust_type_expression(self.source.child(id, &["type"]))],
                scope: String::new(),
            },
            "primitive_type" | "type_identifier" | "scoped_type_identifier" => {
                if let Some(bound) = self.rust_type_bindings.get(text) {
                    return bound.clone();
                }
                if let Some(parameter) = self.rust_type_parameters.get(text) {
                    return parameter.clone();
                }
                if text == "Self" {
                    let mut ancestor = self
                        .function
                        .map(|f| self.analyzer.program.functions[f].node);
                    while let Some(id) = ancestor {
                        if self.source.nodes[id].kind == "impl_item" {
                            if let Some(target) = self.source.child(id, &["type"]) {
                                if self.source.text(Some(target)) != "Self" {
                                    return self.rust_type_expression(Some(target));
                                }
                            }
                            break;
                        }
                        ancestor = self.source.nodes[id].parent;
                    }
                }
                if let Some((prefix, member)) = text.split_once("::") {
                    if let Some(base) = self
                        .rust_type_bindings
                        .get(prefix)
                        .or_else(|| self.rust_type_parameters.get(prefix))
                    {
                        return RustType {
                            kind: "projection".into(),
                            name: member.into(),
                            arguments: vec![base.clone()],
                            scope: String::new(),
                        };
                    }
                }
                let owner =
                    self.analyzer
                        .program
                        .resolve_type(self.source_index, text, &self.owner());
                if !owner.is_empty() {
                    return RustType::named("nominal", &owner);
                }
                let primitive = text.rsplit("::").next().unwrap_or(text);
                if matches!(
                    primitive,
                    "u8" | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "usize"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "isize"
                        | "f32"
                        | "f64"
                        | "bool"
                        | "char"
                        | "str"
                ) {
                    let std = format!("std::primitive::{primitive}");
                    let core = format!("core::primitive::{primitive}");
                    if (primitive == text && !self.source.module_bindings.contains(text))
                        || self.rust_standard_path(text, &[&std, &core], None)
                    {
                        return RustType::named("primitive", &core);
                    }
                }
                RustType::named("unresolved", text)
            }
            _ => RustType::named("unresolved", text),
        }
    }
    pub fn rust_call_bindings(
        &self,
        target: usize,
        callee: Option<NodeId>,
        arguments: &[Value],
    ) -> BTreeMap<String, RustType> {
        let function = &self.analyzer.program.functions[target];
        let source = &self.analyzer.program.sources[function.source];
        let names = source
            .child(function.node, &["type_parameters"])
            .map(|id| {
                source.nodes[id]
                    .children
                    .iter()
                    .filter(|n| source.nodes[**n].kind == "type_parameter")
                    .map(|n| source.text(source.child(*n, &["name"])).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if names.is_empty() {
            return BTreeMap::new();
        }
        let explicit = callee
            .and_then(|id| self.source.child(id, &["type_arguments"]))
            .map(|id| {
                self.source.nodes[id]
                    .children
                    .iter()
                    .copied()
                    .filter(|n| self.source.nodes[*n].kind != "lifetime")
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !explicit.is_empty() && explicit.len() != names.len() {
            return BTreeMap::new();
        }
        let mut bindings = BTreeMap::new();
        for (name, id) in names.iter().zip(explicit) {
            let ty = self.rust_type_expression(Some(id));
            if ty.is_concrete() {
                bindings.insert(name.clone(), ty);
            }
        }
        for ((_, declaration), actual) in function.parameters.iter().zip(arguments) {
            let name = declaration
                .trim_start_matches('&')
                .trim_start_matches("mut ")
                .trim();
            if !names.iter().any(|n| n == name) {
                continue;
            }
            let owner = self.analyzer.owner(actual, self.source_index);
            if owner.is_empty()
                || !self.analyzer.program.types.values().any(|v| v == &owner)
                || self.analyzer.program.generic_types.contains(&owner)
                || self
                    .analyzer
                    .type_text(actual, self.source_index)
                    .contains('<')
            {
                continue;
            }
            let ty = RustType::named("nominal", &owner);
            if bindings.get(name).is_some_and(|old| old != &ty) {
                return BTreeMap::new();
            }
            bindings.insert(name.into(), ty);
        }
        bindings
    }
    pub fn rust_map_entry(&mut self, receiver: &Value, key: &Value) -> Value {
        fn append(base: Value, key: &Value) -> Value {
            if matches!(key.kind.as_str(), "tuple" | "tuple_end") {
                let values = key.tuple_values();
                let mut base = base.slot(Value::new(
                    "container",
                    format!("tuple_key:{}", values.len()),
                ));
                for value in values {
                    base = append(base, &value);
                }
                base
            } else {
                base.slot(key.clone())
            }
        }
        let target = append(receiver.clone().slot(Value::entries()), key);
        let declared = element_type(&self.analyzer.type_text(receiver, self.source_index));
        if !declared.is_empty() {
            self.analyzer
                .record_type(&target, self.source_index, &declared, &self.owner());
        }
        target
    }
}
