use super::model::*;
use super::syntax::*;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

pub(super) const FACTS_PER_FUNCTION: usize = 256;
pub(super) const TOTAL_FACTS: usize = 40_000;
pub(super) const SUMMARY_PASSES: usize = 4;
pub(super) type Environment = BTreeMap<String, Value>;

pub(super) struct Analyzer<'a> {
    pub program: &'a Program,
    pub summaries: BTreeMap<usize, Summary>,
    pub globals: Vec<Environment>,
    pub value_types: BTreeMap<Value, (usize, String, String)>,
    pub owners: BTreeMap<Value, String>,
    pub kinds: BTreeMap<Value, String>,
    pub value_conditions: BTreeMap<Value, BTreeSet<String>>,
    pub heap: BTreeMap<Value, Vec<Value>>,
    pub heap_facts: BTreeMap<Value, Vec<Fact>>,
    pub heap_roots: BTreeMap<Value, Vec<Fact>>,
    pub captures: BTreeMap<usize, Environment>,
    pub allocation_owners: BTreeMap<Value, String>,
    pub allocation_origins: BTreeMap<Value, String>,
    pub allocation_contexts: BTreeMap<Value, Vec<(Location, String)>>,
    pub prototypes: BTreeMap<Value, Value>,
    pub literal_fields: BTreeMap<Value, BTreeSet<String>>,
    pub notices: BTreeSet<(String, String)>,
    pub unresolved_calls: Vec<UnresolvedCall>,
    pub operations: usize,
    pub generator_frames: BTreeMap<Value, super::generators::GeneratorFrame>,
    #[cfg(test)]
    pub observed_calls: BTreeMap<(String, usize, usize), (Location, String, Value)>,
}

impl<'a> Analyzer<'a> {
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            summaries: BTreeMap::new(),
            globals: vec![Environment::new(); program.sources.len()],
            value_types: BTreeMap::new(),
            owners: BTreeMap::new(),
            kinds: BTreeMap::new(),
            value_conditions: BTreeMap::new(),
            heap: BTreeMap::new(),
            heap_facts: BTreeMap::new(),
            heap_roots: BTreeMap::new(),
            captures: BTreeMap::new(),
            allocation_owners: BTreeMap::new(),
            allocation_origins: BTreeMap::new(),
            allocation_contexts: BTreeMap::new(),
            prototypes: BTreeMap::new(),
            literal_fields: BTreeMap::new(),
            notices: BTreeSet::new(),
            unresolved_calls: Vec::new(),
            operations: 0,
            generator_frames: BTreeMap::new(),
            #[cfg(test)]
            observed_calls: BTreeMap::new(),
        }
    }
    pub fn tick(&mut self, function: &str) -> bool {
        self.operations += 1;
        if self.operations > 2_000_000 {
            self.notices
                .insert(("analysis_work_cap".into(), function.into()));
            return false;
        }
        true
    }
    pub fn record_type(&mut self, value: &Value, source: usize, text: &str, scope: &str) {
        if value.kind == "unknown" || text.is_empty() {
            return;
        }
        self.value_types.insert(
            value.clone(),
            (
                source,
                text.trim_start_matches([':', ' ']).into(),
                scope.into(),
            ),
        );
        let owner = self.program.resolve_type(source, text, scope);
        if !owner.is_empty() {
            self.owners.insert(value.clone(), owner);
        }
    }
    pub fn type_info(&self, value: &Value, source: usize, depth: usize) -> (usize, String, String) {
        if depth > 24 {
            return (source, String::new(), String::new());
        }
        if let Some(info) = self.value_types.get(value) {
            return info.clone();
        }
        if value.kind == "slot" {
            if *value.key() == Value::entries() {
                return self.type_info(value.base(), source, depth + 1);
            }
            if value.key().kind == "key" {
                let owner = self.owner_at(value.base(), source, depth + 1);
                if let Some(declared) = self
                    .program
                    .fields
                    .get(&(owner.clone(), value.key().name.clone()))
                    .filter(|ty| !ty.is_empty())
                {
                    return (
                        self.program
                            .owner_sources
                            .get(&owner)
                            .copied()
                            .unwrap_or(source),
                        declared.clone(),
                        owner,
                    );
                }
                let receiver = Value::new("receiver", &owner).slot(value.key().clone());
                if let Some(info) = self.value_types.get(&receiver) {
                    return info.clone();
                }
            }
            if value.key().kind != "key"
                || (value.base().kind == "slot" && *value.base().key() == Value::entries())
                || value.key().name.parse::<usize>().is_ok()
            {
                let (source, ty, scope) = self.type_info(value.base(), source, depth + 1);
                return (
                    source,
                    element_type(&self.program.expanded_type(source, &ty)),
                    scope,
                );
            }
        }
        (source, String::new(), String::new())
    }
    pub fn type_text(&self, value: &Value, source: usize) -> String {
        let (origin, text, _) = self.type_info(value, source, 0);
        self.program.expanded_type(origin, &text)
    }
    pub fn owner(&self, value: &Value, source: usize) -> String {
        self.owner_at(value, source, 0)
    }
    fn owner_at(&self, value: &Value, source: usize, depth: usize) -> String {
        if depth > 24 {
            return String::new();
        }
        if let Some(owner) = self.owners.get(value) {
            return owner.clone();
        }
        if matches!(value.kind.as_str(), "receiver" | "type") {
            return value.name.clone();
        }
        let (origin, text, scope) = self.type_info(value, source, depth + 1);
        let owner = self.program.resolve_type(origin, &text, &scope);
        if owner.is_empty()
            && depth < 5
            && self.program.sources.iter().any(|s| s.language == "scala")
        {
            let mut pending = vec![value.clone()];
            let mut visited = BTreeSet::new();
            let mut owners = BTreeSet::new();
            for _ in 0..4 {
                let mut next = Vec::new();
                for value in pending {
                    if !visited.insert(value.clone()) || visited.len() > 64 {
                        continue;
                    }
                    for (alias, _, _) in self.target_aliases(&value, 0) {
                        if let Some(owner) = self.owners.get(&alias).filter(|o| !o.is_empty()) {
                            owners.insert(owner.clone());
                        }
                        next.push(alias);
                    }
                }
                pending = next;
            }
            if owners.len() == 1 {
                return owners.into_iter().next().unwrap();
            }
        }
        owner
    }
    pub fn kind(&self, value: &Value, source: usize) -> String {
        if let Some(kind) = self.kinds.get(value) {
            return kind.clone();
        }
        if !self.owner(value, source).is_empty() {
            return String::new();
        }
        let text = self.type_text(value, source);
        let known = if matches!(text.as_str(), "map" | "set" | "array") {
            text
        } else {
            type_kind(&text)
        };
        if !known.is_empty() {
            return known;
        }
        let kinds: BTreeSet<_> = self
            .read_values(value)
            .into_iter()
            .filter_map(|v| {
                let text = self.type_text(&v, source);
                let kind = self.kinds.get(&v).cloned().unwrap_or_else(|| {
                    if matches!(text.as_str(), "map" | "set" | "array") {
                        text
                    } else {
                        type_kind(&text)
                    }
                });
                (!kind.is_empty()).then_some(kind)
            })
            .collect();
        if kinds.len() == 1 {
            kinds.into_iter().next().unwrap_or_default()
        } else {
            String::new()
        }
    }
    pub fn read_values(&self, value: &Value) -> Vec<Value> {
        self.read_heap(value, 0, &mut BTreeSet::new(), &mut 96)
    }
    fn read_heap(
        &self,
        value: &Value,
        depth: usize,
        seen: &mut BTreeSet<Value>,
        budget: &mut usize,
    ) -> Vec<Value> {
        if *budget == 0 || depth >= 6 || value.kind != "slot" || !seen.insert(value.clone()) {
            return Vec::new();
        }
        *budget -= 1;
        let mut result: Vec<_> = self
            .heap
            .get(value)
            .into_iter()
            .flatten()
            .flat_map(Value::options)
            .collect();
        for base in self.read_heap(value.base(), depth + 1, seen, budget) {
            if let Some(values) = self.heap.get(&base.slot(value.key().clone())) {
                result.extend(values.iter().flat_map(Value::options));
            }
        }
        if let Some(prototype) = self.prototypes.get(value.base()) {
            result.extend(self.read_heap(
                &prototype.clone().slot(value.key().clone()),
                depth + 1,
                seen,
                budget,
            ));
        }
        for item in result.clone() {
            result.extend(self.read_heap(&item, depth + 1, seen, budget));
        }
        let mut found = BTreeSet::new();
        result.retain(|value| found.insert(value.clone()));
        result.truncate(16);
        result
    }
    pub fn refresh_heap(&mut self, facts: &[Fact]) {
        self.heap.clear();
        self.heap_facts.clear();
        self.heap_roots.clear();
        for fact in facts {
            if fact.kind == "store" && fact.target != fact.value {
                self.heap_facts
                    .entry(fact.target.clone())
                    .or_default()
                    .push(fact.clone());
                self.heap_roots
                    .entry(fact.target.split_path().0.clone())
                    .or_default()
                    .push(fact.clone());
                let values = self.heap.entry(fact.target.clone()).or_default();
                if values.len() < 16 && !values.contains(&fact.value) {
                    values.push(fact.value.clone());
                }
            }
        }
    }
    pub fn run(&mut self) -> Vec<Fact> {
        let mut facts = Vec::new();
        for pass in 0..SUMMARY_PASSES {
            self.generator_frames.clear();
            let mut initial = Vec::new();
            for (source, target, value, location) in &self.program.global_initializers {
                let mut interpreter = Interpreter::new(self, *source, None);
                let value = interpreter.expression(Some(*value), 0, true);
                interpreter.emit("store", target.clone(), value, *location, &[], None);
                initial.extend(interpreter.facts);
            }
            for source in 0..self.program.sources.len() {
                if self.program.sources[source].nodes.is_empty() {
                    continue;
                }
                let declarations = self.program.sources[source].nodes[0].children.clone();
                let mut interpreter = Interpreter::new(self, source, None);
                for declaration in declarations {
                    interpreter.facts.clear();
                    interpreter.statement(Some(declaration));
                    initial.extend(interpreter.facts.clone());
                    interpreter.analyzer.globals[source] = interpreter.env.clone();
                }
                self.globals[source] = interpreter.env;
            }
            for (source, owner, name, value_node, location) in &self.program.initializers {
                let mut interpreter = Interpreter::new(self, *source, None);
                let source_data = &interpreter.source;
                let is_static =
                    matches!(source_data.language.as_str(), "typescript" | "javascript")
                        && source_data.nodes[*location].tokens.contains("static");
                let receiver = if is_static {
                    Value::new("global", format!("{owner}::static"))
                } else {
                    Value::new("receiver", owner)
                };
                if is_static {
                    interpreter
                        .analyzer
                        .owners
                        .insert(receiver.clone(), owner.clone());
                }
                interpreter.env.insert("this".into(), receiver.clone());
                interpreter.env.insert("self".into(), receiver.clone());
                let value = interpreter.expression(Some(*value_node), 0, true);
                let target = receiver.field(name);
                let kind = interpreter.analyzer.kind(&value, *source);
                if !kind.is_empty() {
                    interpreter.analyzer.kinds.insert(target.clone(), kind);
                }
                let value_owner = interpreter.analyzer.owner(&value, *source);
                if !value_owner.is_empty() {
                    interpreter
                        .analyzer
                        .owners
                        .insert(target.clone(), value_owner);
                }
                let declared = interpreter
                    .analyzer
                    .program
                    .fields
                    .get(&(owner.clone(), name.clone()))
                    .cloned()
                    .unwrap_or_default();
                if !declared.is_empty() {
                    interpreter
                        .analyzer
                        .record_type(&target, *source, &declared, owner);
                } else if let Some(info) = interpreter.analyzer.value_types.get(&value).cloned() {
                    interpreter
                        .analyzer
                        .value_types
                        .insert(target.clone(), info);
                }
                interpreter.emit(
                    "store",
                    target,
                    value,
                    *location,
                    &["field_initializer_schema"],
                    None,
                );
                initial.extend(interpreter.facts);
            }
            let mut summaries = BTreeMap::new();
            for index in 0..self.program.functions.len() {
                let function = self.program.functions[index].clone();
                if function.body.is_none() {
                    continue;
                }
                let mut interpreter = Interpreter::new(self, function.source, Some(index));
                let body = function.body.unwrap();
                if let Some(initializers) = interpreter
                    .source
                    .first(function.node, &["field_initializer_list"])
                {
                    for initializer in interpreter.source.nodes[initializers].children.clone() {
                        let named = interpreter
                            .source
                            .first(initializer, &["identifier", "field_identifier"]);
                        let value = interpreter
                            .source
                            .first(initializer, &["initializer_list", "argument_list"]);
                        if let (Some(named), Some(value)) = (named, value) {
                            if interpreter.source.nodes[value].children.len() == 1 {
                                let value = interpreter.expression(
                                    interpreter.source.nodes[value].children.first().copied(),
                                    0,
                                    true,
                                );
                                let name = interpreter.source.text(Some(named)).to_owned();
                                interpreter.emit(
                                    "store",
                                    function.receiver().field(&name),
                                    value,
                                    initializer,
                                    &[],
                                    None,
                                );
                            }
                        }
                    }
                }
                if (is_lambda(&interpreter.source.nodes[function.node].kind)
                    || interpreter.source.language == "scala")
                    && !is_block(&interpreter.source.nodes[body].kind)
                {
                    let value = interpreter.expression(Some(body), 0, true);
                    interpreter.record_return(value, body);
                } else {
                    interpreter.statement(Some(body));
                }
                summaries.insert(
                    index,
                    Summary {
                        receiver: Some(function.receiver()),
                        parameters: interpreter.parameters,
                        facts: interpreter.facts,
                        returns: interpreter.returns,
                        calls: interpreter.calls,
                    },
                );
            }
            let mut current = initial;
            for (index, summary) in &summaries {
                let f = &self.program.functions[*index];
                if is_lambda(&self.program.sources[f.source].nodes[f.node].kind)
                    && !matches!(
                        self.program.sources[f.source].language.as_str(),
                        "typescript" | "javascript" | "go" | "rust"
                    )
                {
                    continue;
                }
                current.extend(summary.facts.clone());
            }
            dedup_facts(&mut current);
            self.cap_facts(&mut current);
            let is_stable = current == facts;
            facts = current;
            self.summaries = summaries;
            self.refresh_heap(&facts);
            if is_stable {
                break;
            }
            if pass + 1 == SUMMARY_PASSES {
                self.notices
                    .insert(("summary_depth_bound".into(), SUMMARY_PASSES.to_string()));
            }
        }
        self.project_facts(&mut facts);
        self.unresolved_calls = self
            .summaries
            .values()
            .flat_map(|s| s.calls.clone())
            .collect();
        self.cap_facts(&mut facts);
        facts
    }
    pub fn cap_facts(&mut self, facts: &mut Vec<Fact>) {
        if facts.len() <= TOTAL_FACTS {
            return;
        }
        self.notices.insert((
            "total_fact_cap".into(),
            (facts.len() - TOTAL_FACTS).to_string(),
        ));
        let mut groups: BTreeMap<(String, String, String), Vec<Fact>> = BTreeMap::new();
        for fact in std::mem::take(facts) {
            groups
                .entry((
                    fact.location.path.clone(),
                    fact.function.clone(),
                    fact.kind.clone(),
                ))
                .or_default()
                .push(fact);
        }
        let mut offset = 0;
        while facts.len() < TOTAL_FACTS {
            let mut has_more = false;
            for group in groups.values() {
                if let Some(fact) = group.get(offset) {
                    facts.push(fact.clone());
                    has_more = true;
                    if facts.len() == TOTAL_FACTS {
                        break;
                    }
                }
            }
            if !has_more {
                break;
            }
            offset += 1;
        }
    }
}

pub(super) struct Interpreter<'a, 'p> {
    pub analyzer: &'a mut Analyzer<'p>,
    pub source: Arc<Source>,
    pub source_index: usize,
    pub function: Option<usize>,
    pub identifier: String,
    pub env: Environment,
    pub parameters: Vec<Value>,
    pub local_bindings: BTreeSet<String>,
    pub conditions: Vec<String>,
    pub facts: Vec<Fact>,
    pub returns: Vec<Value>,
    pub calls: Vec<UnresolvedCall>,
    pub depth: usize,
    pub instance_context: String,
    pub slots: BTreeMap<Value, Value>,
    pub block_returns: Vec<Vec<(Value, NodeId)>>,
    pub block_origins: BTreeMap<NodeId, Vec<(Value, NodeId)>>,
    pub rust_type_parameters: BTreeMap<String, super::rust_types::RustType>,
    pub rust_type_bindings: BTreeMap<String, super::rust_types::RustType>,
    pub next_rust_bindings: BTreeMap<String, super::rust_types::RustType>,
    pub resolved_call_results: BTreeMap<Value, Value>,
    pub references: BTreeMap<String, Value>,
    pub declared_receivers: BTreeMap<String, String>,
    pub active_functions: Vec<usize>,
    pub is_macro_template: bool,
}
impl<'a, 'p> Interpreter<'a, 'p> {
    pub fn new(
        analyzer: &'a mut Analyzer<'p>,
        source_index: usize,
        function: Option<usize>,
    ) -> Self {
        let source = analyzer.program.sources[source_index].clone();
        let definition = function.map(|f| analyzer.program.functions[f].clone());
        let identifier = definition
            .as_ref()
            .map(|f| f.identifier.clone())
            .unwrap_or_else(|| format!("module:{}", source.path));
        let mut env = if matches!(
            source.language.as_str(),
            "javascript" | "typescript" | "go" | "rust"
        ) {
            analyzer.globals[source_index].clone()
        } else {
            Environment::new()
        };
        if let Some(captures) = function.and_then(|f| analyzer.captures.get(&f)) {
            env.extend(captures.clone());
        }
        let rust_type_parameters = if source.language == "rust" {
            super::rust_types::scoped_parameters(&source, definition.as_ref().map(|f| f.node))
        } else {
            BTreeMap::new()
        };
        let mut declared_receivers = BTreeMap::new();
        let mut parameters = Vec::new();
        let mut conditions = if source.has_type_projection {
            vec!["generic_type_constraints_unproven".into()]
        } else {
            Vec::new()
        };
        if source.has_conditional_projection {
            conditions.push(
                if source.language == "swift" {
                    "conditional_compilation_selection_unproven"
                } else {
                    "preprocessor_configuration_unproven"
                }
                .into(),
            );
        }
        let mut local_bindings = BTreeSet::new();
        if let Some(function) = &definition {
            if function.parent.is_some() {
                conditions.push("enclosing_callable_execution_unproven".into());
            }
            if source.expansion(function.node).is_some() {
                conditions.extend([
                    "source_declarative_impl_expansion".into(),
                    "macro_template_typing_unproven".into(),
                ]);
            }
            if function.owner.starts_with("rust:type_variable:") {
                conditions.extend([
                    "generic_impl_receiver_unresolved".into(),
                    "generic_trait_constraints_unproven".into(),
                ]);
            }
            if let Some(body) = function.body {
                local_bindings = source.lexical_bindings(body);
            }
            for (name, ty) in &function.parameters {
                let value = Value::new("parameter", format!("{}:{name}", function.identifier));
                parameters.push(value.clone());
                env.insert(name.clone(), value.clone());
                analyzer.record_type(&value, source_index, ty, &function.owner);
                if matches!(source.language.as_str(), "c" | "cpp") && ty.contains('*') {
                    let owner = analyzer.owner(&value, source_index);
                    if !owner.is_empty() {
                        env.insert(name.clone(), Value::new("receiver", &owner));
                        declared_receivers.insert(name.clone(), owner);
                    }
                }
                if source.language == "rust" {
                    let base = ty
                        .trim_start_matches(['&', '*'])
                        .trim_start_matches("mut ")
                        .split(['<', ':'])
                        .next()
                        .unwrap_or_default()
                        .trim();
                    if rust_type_parameters.contains_key(base) {
                        analyzer.owners.insert(value.clone(), String::new());
                    }
                }
            }
            env.insert(function.receiver_name.clone(), function.receiver());
            if !function.owner.is_empty() {
                env.insert("Self".into(), Value::new("type", &function.owner));
                analyzer
                    .owners
                    .insert(function.receiver(), function.owner.clone());
            }
            if matches!(source.language.as_str(), "typescript" | "javascript")
                && source.nodes[function.node].kind != "arrow_function"
            {
                env.insert(
                    "arguments".into(),
                    Value::new("arguments", &function.identifier),
                );
            }
        }
        let mut interpreter = Self {
            analyzer,
            source,
            source_index,
            function,
            identifier,
            env,
            parameters,
            local_bindings,
            conditions,
            facts: Vec::new(),
            returns: Vec::new(),
            calls: Vec::new(),
            depth: 0,
            instance_context: String::new(),
            slots: BTreeMap::new(),
            block_returns: Vec::new(),
            block_origins: BTreeMap::new(),
            rust_type_parameters,
            rust_type_bindings: BTreeMap::new(),
            next_rust_bindings: BTreeMap::new(),
            resolved_call_results: BTreeMap::new(),
            references: BTreeMap::new(),
            declared_receivers,
            active_functions: Vec::new(),
            is_macro_template: false,
        };
        if let Some(function) = definition.filter(|f| f.name == "constructor") {
            if let Some(params) = interpreter.source.child(function.node, &["parameters"]) {
                for param in interpreter.source.nodes[params].children.clone() {
                    if interpreter
                        .source
                        .first(param, &["accessibility_modifier"])
                        .is_some()
                    {
                        let named = interpreter.source.child(param, &["pattern", "name"]);
                        let name = interpreter.source.text(named).to_owned();
                        let value = interpreter
                            .env
                            .get(&name)
                            .cloned()
                            .unwrap_or_else(Value::unknown);
                        interpreter.emit(
                            "store",
                            function.receiver().field(&name),
                            value,
                            param,
                            &[],
                            None,
                        );
                    }
                }
            }
        }
        interpreter
    }
    pub fn owner(&self) -> String {
        self.function
            .map(|f| self.analyzer.program.functions[f].owner.clone())
            .unwrap_or_else(|| {
                self.env
                    .get("this")
                    .or_else(|| self.env.get("self"))
                    .map(|v| {
                        if matches!(v.kind.as_str(), "receiver" | "type") {
                            v.name.clone()
                        } else {
                            self.analyzer.owners.get(v).cloned().unwrap_or_default()
                        }
                    })
                    .unwrap_or_default()
            })
    }
    pub fn allocation(&mut self, id: NodeId, label: &str) -> Value {
        let node = &self.source.nodes[id];
        let value = Value::new(
            "allocation",
            format!(
                "{}:{}:{}:ast:{}-{}{}{}",
                self.source.path,
                node.line,
                node.column,
                node.start,
                node.end,
                label,
                self.instance_context
            ),
        );
        self.analyzer
            .allocation_owners
            .insert(value.clone(), self.identifier.clone());
        value
    }
    pub fn emit(
        &mut self,
        kind: &str,
        target: Value,
        value: Value,
        node: NodeId,
        extra: &[&str],
        argument_index: Option<usize>,
    ) {
        if !matches!(
            self.source.language.as_str(),
            "typescript" | "javascript" | "go" | "rust"
        ) && kind == "store"
            && value.kind == "tuple"
        {
            for (index, item) in value.tuple_values().into_iter().enumerate() {
                self.emit(
                    kind,
                    target
                        .clone()
                        .slot(Value::new("tuple_index", index.to_string())),
                    item,
                    node,
                    extra,
                    argument_index,
                );
            }
            return;
        }
        if self.source.language == "rust" && kind == "store" {
            self.slots.insert(target.clone(), value.clone());
        }
        let mut conditions: BTreeSet<_> = self
            .conditions
            .iter()
            .cloned()
            .chain(extra.iter().map(|v| (*v).to_owned()))
            .collect();
        for item in target.referenced().into_iter().chain(value.referenced()) {
            if let Some(required) = self.analyzer.value_conditions.get(item) {
                conditions.extend(required.iter().cloned());
            }
        }
        if self.is_polyglot() {
            conditions.insert("control_flow_unproven".into());
        }
        if self.source.language == "swift" {
            conditions.insert("source_module_binding_required".into());
        }
        let mut fact = Fact::new(
            kind,
            target,
            value,
            self.source.location(node),
            &self.identifier,
            conditions.into_iter().collect(),
        );
        fact.argument_index = argument_index;
        if let Some(expansion) = self.source.expansion(node) {
            fact.via.push(self.source.offset_location(expansion.origin));
            if let Some(definition) = &expansion.definition {
                fact.via.push(definition.clone());
                fact.conditions.extend([
                    "source_declarative_statement_expansion".into(),
                    "macro_template_typing_unproven".into(),
                ]);
            }
        }
        self.append_fact(fact);
    }
    pub fn append_fact(&mut self, fact: Fact) {
        if self.is_polyglot() && fact.target.kind == "choice" {
            for target in fact.target.options() {
                let mut alternative = fact.clone();
                alternative.target = target;
                self.append_fact(alternative);
            }
            return;
        }
        if self.facts.contains(&fact) {
            return;
        }
        if self.facts.len() >= FACTS_PER_FUNCTION {
            self.analyzer
                .notices
                .insert(("function_fact_cap".into(), self.identifier.clone()));
            fn rank(fact: &Fact) -> (bool, u8) {
                (
                    !fact.via.is_empty(),
                    match fact.kind.as_str() {
                        "store" | "invoke" | "member_invoke" => 0,
                        "remove" => 1,
                        "return" => 2,
                        "argument" => 3,
                        _ => 4,
                    },
                )
            }
            let (index, victim) = self
                .facts
                .iter()
                .enumerate()
                .max_by_key(|(_, f)| rank(f))
                .unwrap();
            if rank(&fact) >= rank(victim) {
                return;
            }
            self.facts.remove(index);
        }
        self.facts.push(fact);
    }
    pub fn binding(&self, name: &str) -> Value {
        let name = name.trim_start_matches('$');
        if let Some(value) = self.env.get(name) {
            return value.clone();
        }
        if self.source.language == "rust"
            && self.rust_standard_path(
                name,
                &["std::option::Option::None", "core::option::Option::None"],
                Some("None"),
            )
        {
            return Value::nested("wrapper", "rust:option_none", Value::unknown(), None);
        }
        let owner = self.owner();
        if self.source.language == "java" {
            if let Some(value) = self.analyzer.program.jvm_binding(&owner, name) {
                return value;
            }
        }
        if !owner.is_empty()
            && (!self.is_polyglot()
                || matches!(
                    self.source.language.as_str(),
                    "cpp" | "csharp" | "dart" | "groovy" | "java" | "kotlin" | "scala" | "swift"
                ))
            && self
                .analyzer
                .program
                .fields
                .contains_key(&(owner.clone(), name.into()))
        {
            let receiver = self
                .function
                .map(|f| {
                    let f = &self.analyzer.program.functions[f];
                    self.env
                        .get(&f.receiver_name)
                        .cloned()
                        .unwrap_or_else(|| f.receiver())
                })
                .unwrap_or_else(Value::unknown);
            return receiver.field(name);
        }
        if self.source.language == "scala" {
            if name == "copy" && self.analyzer.program.scala.case_classes.contains(&owner) {
                return self
                    .env
                    .get("this")
                    .cloned()
                    .unwrap_or_else(|| Value::new("receiver", &owner))
                    .field("copy");
            }
            if let Some(index) = self.analyzer.program.scala.package_members.get(&(
                self.analyzer
                    .program
                    .namespaces
                    .get(&self.source.path)
                    .cloned()
                    .unwrap_or_default(),
                name.into(),
            )) {
                return Value::new(
                    "function",
                    &self.analyzer.program.functions[*index].identifier,
                );
            }
        }
        if let Some(value) = self
            .analyzer
            .program
            .global_values
            .get(&(self.source.path.clone(), name.into()))
        {
            return value.clone();
        }
        let methods = self
            .analyzer
            .program
            .methods
            .get(&(owner.clone(), name.into()));
        if !owner.is_empty() && methods.is_some_and(|v| v.len() == 1) {
            return Value::new(
                "function",
                &self.analyzer.program.functions[methods.unwrap()[0]].identifier,
            );
        }
        let namespace = self.analyzer.program.type_namespace(self.source_index);
        if self.is_polyglot() {
            let resolved = self
                .analyzer
                .program
                .resolve_type(self.source_index, name, &owner);
            if self.analyzer.program.owner_sources.contains_key(&resolved) {
                return Value::new("type", resolved);
            }
        }
        if let Some(imported) = self
            .analyzer
            .program
            .imports
            .get(&(self.source.path.clone(), name.into()))
        {
            if let Some((path, name)) = imported.rsplit_once('#') {
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
                {
                    if functions.len() == 1 {
                        return Value::new(
                            "function",
                            self.analyzer.program.functions[functions[0]]
                                .identifier
                                .clone(),
                        );
                    }
                }
            } else {
                return Value::new("namespace", imported);
            }
        }
        if let Some(functions) = self
            .analyzer
            .program
            .methods
            .get(&(namespace.clone(), name.into()))
        {
            if functions.len() == 1 {
                return Value::new(
                    "function",
                    self.analyzer.program.functions[functions[0]]
                        .identifier
                        .clone(),
                );
            }
        }
        if self
            .analyzer
            .program
            .types
            .contains_key(&(namespace.clone(), name.into()))
            || self
                .analyzer
                .program
                .aliases
                .contains_key(&(namespace.clone(), name.into()))
        {
            return Value::new(
                "type",
                self.analyzer
                    .program
                    .resolve_type(self.source_index, name, &owner),
            );
        }
        if let Some((base, member)) = name.rsplit_once("::") {
            let resolved = self
                .analyzer
                .program
                .resolve_type(self.source_index, base, &owner);
            if !resolved.is_empty() {
                return Value::new("type", resolved).field(member);
            }
            if let Some(value) = self.env.get(base) {
                if value.kind == "type" {
                    return value.clone().field(member);
                }
            }
            if let Some(namespace) = self
                .analyzer
                .program
                .imports
                .get(&(self.source.path.clone(), base.into()))
            {
                if !namespace.contains('#') {
                    if let Some(functions) = self
                        .analyzer
                        .program
                        .methods
                        .get(&(namespace.clone(), member.into()))
                        .filter(|v| v.len() == 1)
                    {
                        return Value::new(
                            "function",
                            &self.analyzer.program.functions[functions[0]].identifier,
                        );
                    }
                }
            }
        }
        if self.source.language == "go"
            && self
                .analyzer
                .program
                .import_paths
                .get(&(self.source.path.clone(), name.into()))
                .is_some_and(|path| path == "reflect")
            && !self.local_bindings.contains(name)
        {
            return Value::new("import", "go:reflect");
        }
        if matches!(self.source.language.as_str(), "typescript" | "javascript")
            && name == "globalThis"
        {
            return Value::new("global", "javascript:globalThis");
        }
        if self.source.language == "swift" {
            let resolved = self
                .analyzer
                .program
                .resolve_type(self.source_index, name, &owner);
            if self.analyzer.program.owner_sources.contains_key(&resolved) {
                return Value::new("type", resolved);
            }
            let functions: Vec<_> = self
                .analyzer
                .program
                .functions
                .iter()
                .filter(|f| {
                    f.owner.is_empty()
                        && f.name == name
                        && self.analyzer.program.sources[f.source].language == "swift"
                })
                .collect();
            if functions.len() == 1 {
                return Value::new("function", &functions[0].identifier);
            }
        }
        Value::new("unresolved", format!("{}:{name}", self.source.path))
    }
    pub fn bind(&mut self, pattern: Option<NodeId>, mut value: Value, type_text: &str) {
        let Some(pattern) = pattern else {
            return;
        };
        if self.is_polyglot() && self.polyglot_bind(pattern, value.clone(), type_text) {
            return;
        }
        let node = self.source.nodes[pattern].clone();
        if is_identifier(&node.kind) {
            let name = self
                .source
                .text(Some(pattern))
                .trim_start_matches('$')
                .to_owned();
            if value.kind == "unknown" {
                value = Value::new(
                    if matches!(self.source.language.as_str(), "typescript" | "javascript") {
                        "binding"
                    } else {
                        "unresolved"
                    },
                    format!(
                        "{}:{}:{name}{}",
                        self.identifier, node.start, self.instance_context
                    ),
                );
                self.analyzer
                    .allocation_owners
                    .insert(value.clone(), self.identifier.clone());
            }
            self.analyzer
                .record_type(&value, self.source_index, type_text, &self.owner());
            self.env.insert(name, value);
        } else if matches!(
            node.kind.as_str(),
            "object_pattern" | "object_assignment_pattern" | "struct_pattern"
        ) {
            for part in node.children {
                if is_identifier(&self.source.nodes[part].kind) {
                    let name = self.source.text(Some(part)).to_owned();
                    self.bind(Some(part), value.clone().field(&name), "");
                } else if matches!(
                    self.source.nodes[part].kind.as_str(),
                    "pair_pattern" | "pair" | "field_pattern"
                ) {
                    let key = self.source.child(part, &["key", "name", "field"]);
                    let target = self.source.child(part, &["value", "pattern"]).or(key);
                    let name = self.source.text(key).to_owned();
                    self.bind(target, value.clone().field(&name), "");
                }
            }
        } else if matches!(
            node.kind.as_str(),
            "tuple_pattern"
                | "array_pattern"
                | "expression_list"
                | "pattern_list"
                | "list_splat_pattern"
                | "tuple"
        ) {
            let parts = value.tuple_values();
            for (index, part) in node.children.into_iter().enumerate() {
                self.bind(
                    Some(part),
                    parts
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| value.clone().field(&index.to_string())),
                    "",
                );
            }
        } else if matches!(
            node.kind.as_str(),
            "mut_pattern" | "ref_pattern" | "reference_pattern" | "rest_pattern"
        ) {
            self.bind(node.children.last().copied(), value, type_text);
        } else if node.kind == "tuple_struct_pattern"
            && self.rust_standard_path(
                self.source.text(self.source.child(pattern, &["type"])),
                &["std::option::Option::Some", "core::option::Option::Some"],
                Some("Some"),
            )
        {
            let payloads = value
                .options()
                .into_iter()
                .filter(|v| v.kind == "wrapper" && v.name == "rust:option")
                .map(|v| v.base().clone());
            let payload = Value::merge(payloads);
            let pattern_type = self.source.child(pattern, &["type"]);
            let targets: Vec<_> = node
                .children
                .into_iter()
                .filter(|v| Some(*v) != pattern_type)
                .collect();
            if targets.len() == 1 && payload.kind != "unknown" {
                self.conditions.push("option_some_required".into());
                self.bind(Some(targets[0]), payload, "");
            }
        } else {
            self.analyzer
                .notices
                .insert(("binding_pattern_unsupported".into(), node.kind));
        }
    }
}

pub(super) fn dedup_facts(facts: &mut Vec<Fact>) {
    let mut seen = HashSet::new();
    facts.retain(|fact| seen.insert(fact.clone()));
}
pub(super) fn closure_captures(value: &Value) -> Environment {
    if value.kind != "closure" {
        return Environment::new();
    }
    if value.base().kind == "capture_end" {
        return Environment::new();
    }
    if value.base().kind == "capture_binding" {
        let mut result = Environment::new();
        let mut binding = value.base();
        while binding.kind == "capture_binding" {
            result.insert(binding.name.clone(), binding.base().clone());
            binding = binding.key();
        }
        return result;
    }
    value
        .key()
        .tuple_values()
        .into_iter()
        .map(|v| v.name)
        .zip(value.base().tuple_values())
        .collect()
}
