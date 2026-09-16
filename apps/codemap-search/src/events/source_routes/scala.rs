//! Source-selected companion implicits and explicit constructor/copy schemas.
use super::engine::*;
use super::model::*;
use super::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(crate) struct ClassParameter {
    pub name: String,
    pub type_text: String,
    pub default: Option<NodeId>,
}
#[derive(Default)]
pub(crate) struct Scala {
    pub class_parameters: BTreeMap<String, Vec<ClassParameter>>,
    pub case_classes: BTreeSet<String>,
    pub bases: BTreeMap<String, Vec<String>>,
    pub anonymous: BTreeMap<(usize, NodeId), String>,
    pub implicit_names: BTreeMap<usize, BTreeSet<String>>,
    pub providers: Vec<usize>,
    pub package_members: BTreeMap<(String, String), usize>,
}
fn type_parts(text: &str) -> (String, Vec<String>) {
    let text = text.trim();
    if let Some((base, rest)) = text.split_once('[') {
        if let Some(rest) = rest.strip_suffix(']') {
            return (base.into(), split_arguments(rest));
        }
    }
    (text.into(), Vec::new())
}
impl Program {
    fn scala_macro_template(&self, target: usize) -> Option<(usize, NodeId)> {
        let function = &self.functions[target];
        let source = &self.sources[function.source];
        let text = source.text(function.body).trim_start();
        let invocation = text
            .strip_prefix("macro ")?
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
            .next()?;
        let (prefix, name) = invocation.rsplit_once('.')?;
        let mut owner = self.resolve_type(function.source, prefix, "");
        if owner.is_empty() {
            owner = self.resolve_type(
                function.source,
                prefix.rsplit('.').next().unwrap_or(prefix),
                "",
            );
        }
        let targets = self.method_candidates(&owner, name);
        if targets.len() != 1 {
            return None;
        }
        let implementation = &self.functions[targets[0]];
        let source = &self.sources[implementation.source];
        let body = implementation.body?;
        let tail = source.nodes[body]
            .children
            .iter()
            .rev()
            .find(|n| !source.nodes[**n].kind.contains("comment"))?;
        let mut templates = Vec::new();
        for &declaration in &source.nodes[body].children {
            if source.nodes[declaration].kind != "val_definition" {
                continue;
            }
            let Some(value) = source.child(declaration, &["value"]) else {
                continue;
            };
            if source.nodes[value].kind != "call_expression"
                || source.text(source.child(value, &["function"])) != "reify"
            {
                continue;
            }
            let Some(template) = source.first(value, &["block"]) else {
                continue;
            };
            let name = source.text(source.child(declaration, &["pattern", "name"]));
            if !name.is_empty() && source.text(Some(*tail)).contains(&format!("{name}.tree")) {
                templates.push((implementation.source, template));
            }
        }
        (templates.len() == 1).then(|| templates[0])
    }
    pub fn collect_scala(&mut self) {
        for (index, source) in self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| s.language == "scala")
        {
            for id in source.walk(0, false) {
                if !matches!(
                    source.nodes[id].kind.as_str(),
                    "class_definition" | "object_definition" | "trait_definition"
                ) {
                    continue;
                }
                let name = source.text(source.child(id, &["name"]));
                let owner = self.owner_key(index, name);
                if let Some(extend) = source.child(id, &["extend"]) {
                    let bases = source.nodes[extend]
                        .children
                        .iter()
                        .filter(|n| {
                            matches!(
                                source.nodes[**n].kind.as_str(),
                                "type_identifier" | "stable_type_identifier" | "generic_type"
                            )
                        })
                        .map(|n| self.resolve_type(index, source.text(Some(*n)), ""))
                        .filter(|n| !n.is_empty())
                        .collect();
                    self.scala.bases.insert(owner.clone(), bases);
                }
                if source.nodes[id].kind == "class_definition" {
                    let mut parameters = Vec::new();
                    let is_case = source
                        .text(Some(id))
                        .split('{')
                        .next()
                        .unwrap_or_default()
                        .contains("case class");
                    if is_case {
                        self.scala.case_classes.insert(owner.clone());
                    }
                    if let Some(params) = source.child(id, &["class_parameters"]) {
                        for &param in &source.nodes[params].children {
                            let name = source.text(source.child(param, &["name"])).to_owned();
                            let ty = source.text(source.child(param, &["type"])).to_owned();
                            if name.is_empty() {
                                continue;
                            }
                            if is_case
                                || words(source.text(Some(param)))
                                    .iter()
                                    .any(|w| matches!(w.as_str(), "val" | "var"))
                            {
                                self.fields
                                    .insert((owner.clone(), name.clone()), ty.clone());
                            }
                            parameters.push(ClassParameter {
                                name,
                                type_text: ty,
                                default: source.child(param, &["default_value"]),
                            });
                        }
                    }
                    self.scala.class_parameters.insert(owner, parameters);
                }
            }
        }
        for (index, function) in self.functions.iter().enumerate() {
            let source = &self.sources[function.source];
            if source.language != "scala" {
                continue;
            }
            let mut names = BTreeSet::new();
            for &group in &source.nodes[function.node].children {
                if source.nodes[group].kind == "parameters" {
                    let text = source
                        .text(Some(group))
                        .trim_start_matches('(')
                        .trim_start();
                    if matches!(text.split_whitespace().next(), Some("implicit" | "using")) {
                        for &param in &source.nodes[group].children {
                            names.insert(source.text(source.child(param, &["name"])).to_owned());
                        }
                    }
                }
            }
            self.scala.implicit_names.insert(index, names);
            let modifiers = source.text(source.first(function.node, &["modifiers"]));
            if modifiers.split_whitespace().any(|m| m == "implicit")
                && function.parameters.is_empty()
            {
                self.scala.providers.push(index);
            }
            let mut ancestor = source.nodes[function.node].parent;
            while let Some(parent) = ancestor {
                if source.nodes[parent].kind == "package_object" {
                    let ns = format!(
                        "{}.{}",
                        self.namespaces
                            .get(&source.path)
                            .map(String::as_str)
                            .unwrap_or_default(),
                        source.text(source.child(parent, &["name"]))
                    );
                    self.scala
                        .package_members
                        .insert((ns, function.name.clone()), index);
                    break;
                }
                ancestor = source.nodes[parent].parent;
            }
        }
    }
    pub fn scala_ancestors(&self, owner: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut pending = vec![owner.to_owned()];
        while !pending.is_empty() && result.len() < 32 {
            let next = pending.remove(0);
            if result.contains(&next) {
                continue;
            }
            result.push(next.clone());
            pending.extend(self.scala.bases.get(&next).into_iter().flatten().cloned());
        }
        result
    }
}
impl Interpreter<'_, '_> {
    pub fn construct_scala(
        &mut self,
        owner: &str,
        arguments: &[Value],
        argument_nodes: &[NodeId],
        id: NodeId,
        original: Option<&Value>,
    ) -> Value {
        let Some(parameters) = self
            .analyzer
            .program
            .scala
            .class_parameters
            .get(owner)
            .cloned()
        else {
            return Value::unknown();
        };
        let value = self.allocation(id, ":scala_instance");
        self.analyzer.owners.insert(value.clone(), owner.into());
        let source_index = self.analyzer.program.owner_sources[owner];
        let mut supplied = BTreeMap::new();
        for (index, argument) in arguments.iter().enumerate() {
            let name = argument_nodes
                .get(index)
                .filter(|n| self.source.nodes[**n].kind == "assignment_expression")
                .map(|n| {
                    self.source
                        .text(self.source.child(*n, &["left"]))
                        .to_owned()
                })
                .unwrap_or_else(|| {
                    parameters
                        .get(index)
                        .map(|p| p.name.clone())
                        .unwrap_or_default()
                });
            if !name.is_empty() {
                supplied.insert(name, argument.clone());
            }
        }
        let mut nested = Interpreter::new(self.analyzer, source_index, None);
        nested.env.insert("this".into(), value.clone());
        nested.env.insert("self".into(), value.clone());
        nested.depth = self.depth;
        nested.instance_context = self.instance_context.clone();
        nested.conditions = self.conditions.clone();
        let mut stores = Vec::new();
        for parameter in parameters {
            let actual = if let Some(value) = supplied.get(&parameter.name) {
                value.clone()
            } else if let Some(original) = original {
                original.clone().field(&parameter.name)
            } else {
                nested.expression(parameter.default, 0, true)
            };
            nested.env.insert(parameter.name.clone(), actual.clone());
            let target = value.clone().field(&parameter.name);
            nested
                .analyzer
                .record_type(&target, source_index, &parameter.type_text, owner);
            if nested
                .analyzer
                .program
                .fields
                .contains_key(&(owner.into(), parameter.name))
            {
                stores.push((target, actual));
            }
        }
        let initializers = nested.analyzer.program.initializers.clone();
        for (source, initial_owner, name, value_node, location) in initializers {
            if initial_owner == owner && source == source_index {
                let initial = nested.expression(Some(value_node), 0, true);
                nested.emit(
                    "store",
                    value.clone().field(&name),
                    initial,
                    location,
                    &["constructor_initializer"],
                    None,
                );
            }
        }
        let facts = nested.facts;
        for fact in facts {
            self.append_fact(fact);
        }
        for (target, actual) in stores {
            self.emit(
                "store",
                target,
                actual,
                id,
                &[if original.is_some() {
                    "case_class_copy"
                } else {
                    "source_constructor_parameter"
                }],
                None,
            );
        }
        value
    }
    fn scala_implicit_arguments(
        &mut self,
        target: usize,
        arguments: &[Value],
        id: NodeId,
    ) -> Option<Vec<Value>> {
        let function = self.analyzer.program.functions[target].clone();
        let implicit = self
            .analyzer
            .program
            .scala
            .implicit_names
            .get(&target)
            .cloned()
            .unwrap_or_default();
        if implicit.is_empty() || arguments.len() == function.parameters.len() {
            return Some(arguments.to_vec());
        }
        let explicit: Vec<_> = function
            .parameters
            .iter()
            .filter(|(name, _)| !implicit.contains(name))
            .collect();
        if explicit.len() != arguments.len() {
            return None;
        }
        let mut bindings = BTreeMap::new();
        for ((_, declared), actual) in explicit.iter().zip(arguments) {
            let owner = self.analyzer.owner(actual, self.source_index);
            if !owner.is_empty() {
                bindings.insert(
                    declared.clone(),
                    owner
                        .split_once(':')
                        .map(|p| p.1)
                        .unwrap_or(&owner)
                        .replace("::", "."),
                );
            } else if actual.kind == "literal" && actual.name.chars().all(|c| c.is_ascii_digit()) {
                bindings.insert(declared.clone(), "Int".into());
            }
        }
        let mut result = Vec::new();
        let mut index = 0;
        let binding_pattern = regex::Regex::new(r"\b[A-Za-z_]\w*\b").unwrap();
        for (name, declared) in &function.parameters {
            if !implicit.contains(name) {
                result.push(arguments[index].clone());
                index += 1;
                continue;
            }
            let required = binding_pattern
                .replace_all(declared, |m: &regex::Captures<'_>| {
                    bindings.get(&m[0]).cloned().unwrap_or_else(|| m[0].into())
                })
                .into_owned();
            let Some(provider) = self.scala_provider(function.source, &required) else {
                self.analyzer.notices.insert((
                    "implicit_resolution_unproven".into(),
                    format!("{}:{name}", function.identifier),
                ));
                return None;
            };
            let owner = self.analyzer.program.functions[provider].owner.clone();
            let value = self.apply_scala(provider, Value::new("type", owner), Vec::new(), id);
            if value.kind == "unknown" {
                return None;
            }
            self.analyzer
                .value_conditions
                .entry(value.clone())
                .or_default()
                .insert("source_companion_implicit_selection_required".into());
            result.push(value);
        }
        Some(result)
    }
    fn scala_provider(&self, source_index: usize, required: &str) -> Option<usize> {
        let (base, args) = type_parts(required);
        let companion = self.analyzer.program.resolve_type(source_index, &base, "");
        if companion.is_empty() || args.is_empty() {
            return None;
        }
        let ancestors = self.analyzer.program.scala_ancestors(&companion);
        let mut candidates = Vec::new();
        let parameter_pattern = regex::Regex::new(r"(?:\[|,)\s*([A-Za-z_]\w*)").unwrap();
        for &index in &self.analyzer.program.scala.providers {
            let function = &self.analyzer.program.functions[index];
            if !ancestors.contains(&function.owner) {
                continue;
            }
            let source = &self.analyzer.program.sources[function.source];
            let parameters = source.text(source.first(function.node, &["type_parameters"]));
            let stripped = parameters.replace("<:", "").replace(">:", "");
            if stripped.contains(':') {
                continue;
            }
            let (offered, offered_args) =
                type_parts(source.text(source.child(function.node, &["return_type"])));
            if self
                .analyzer
                .program
                .resolve_type(function.source, &offered, "")
                != companion
                || offered_args.len() != args.len()
            {
                continue;
            }
            let variables: Vec<_> = parameter_pattern
                .captures_iter(parameters)
                .map(|m| m[1].to_owned())
                .collect();
            let first = &offered_args[0];
            if variables.contains(first) {
                if parameters.contains("AnyRef")
                    && self
                        .analyzer
                        .program
                        .resolve_type(source_index, &args[0], "")
                        .is_empty()
                {
                    continue;
                }
            } else if first != &args[0] {
                continue;
            }
            candidates.push(index);
        }
        if candidates.len() == 1 {
            Some(candidates[0])
        } else {
            None
        }
    }
    pub fn apply_scala(
        &mut self,
        target: usize,
        receiver: Value,
        arguments: Vec<Value>,
        id: NodeId,
    ) -> Value {
        if self.active_functions.contains(&target)
            || Some(target) == self.function
            || self.active_functions.len() >= 10
        {
            self.analyzer.notices.insert((
                "source_call_depth_bound".into(),
                self.analyzer.program.functions[target].identifier.clone(),
            ));
            return Value::unknown();
        }
        let Some(actuals) = self.scala_implicit_arguments(target, &arguments, id) else {
            return Value::unknown();
        };
        let function = self.analyzer.program.functions[target].clone();
        if actuals.len() != function.parameters.len() {
            return Value::unknown();
        }
        let mut requirements = self.conditions.clone();
        for value in std::iter::once(&receiver).chain(&actuals) {
            for part in value.referenced() {
                if let Some(conditions) = self.analyzer.value_conditions.get(part) {
                    requirements.extend(conditions.iter().cloned());
                }
            }
        }
        for value in std::iter::once(&receiver).chain(&actuals) {
            for (alias, conditions, _) in self.analyzer.target_aliases(value, 0) {
                requirements.extend(conditions);
                for part in alias.referenced() {
                    if let Some(extra) = self.analyzer.value_conditions.get(part) {
                        requirements.extend(extra.iter().cloned());
                    }
                }
            }
        }
        let mut active = self.active_functions.clone();
        if let Some(function) = self.function {
            active.push(function);
        }
        let context = format!(
            "{}@{}:{}:{}",
            self.instance_context,
            self.source.path,
            self.source.nodes[id].line,
            self.source.nodes[id].column
        );
        let mut nested = Interpreter::new(self.analyzer, function.source, Some(target));
        nested.active_functions = active;
        nested.conditions.extend(requirements);
        nested.depth = self.depth + 1;
        nested.instance_context = context;
        let receiver = if receiver.kind == "unknown" {
            function.receiver()
        } else {
            receiver
        };
        nested.env.insert("this".into(), receiver.clone());
        nested.env.insert("self".into(), receiver);
        for ((name, _), actual) in function.parameters.iter().zip(actuals) {
            nested.env.insert(name.clone(), actual);
        }
        if let Some((source, body)) = nested.analyzer.program.scala_macro_template(target) {
            nested.source_index = source;
            nested.source = nested.analyzer.program.sources[source].clone();
            nested.is_macro_template = true;
            nested.conditions.extend([
                "source_macro_template_preservation_required".into(),
                "compiler_macro_typing_unproven".into(),
            ]);
            nested.statement(Some(body));
        } else if let Some(body) = function.body {
            if nested.source.language == "scala" && !is_block(&nested.source.nodes[body].kind) {
                let value = nested.expression(Some(body), 0, true);
                nested.returns.push(value);
            } else {
                nested.statement(Some(body));
            }
        }
        let returned = Value::merge(nested.returns);
        let conditions = nested.conditions;
        let facts = nested.facts;
        for mut fact in facts {
            fact.conditions
                .push("source_callable_argument_binding".into());
            fact.via.push(self.source.location(id));
            self.append_fact(fact);
        }
        if returned.kind != "unknown" {
            self.analyzer
                .value_conditions
                .entry(returned.clone())
                .or_default()
                .extend(conditions);
        }
        returned
    }
}
