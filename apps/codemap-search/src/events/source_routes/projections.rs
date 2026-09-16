use super::engine::*;
use super::model::*;
use std::collections::{BTreeMap, BTreeSet};

impl Analyzer<'_> {
    pub fn project_facts(&mut self, facts: &mut Vec<Fact>) {
        for _ in 0..2 {
            let mut fields: BTreeMap<Value, Vec<Fact>> = BTreeMap::new();
            for fact in facts
                .iter()
                .filter(|f| f.kind == "store" && f.target.split_path().0.kind == "allocation")
            {
                fields
                    .entry(fact.target.split_path().0.clone())
                    .or_default()
                    .push(fact.clone());
            }
            let mut added = Vec::new();
            for store in facts
                .iter()
                .filter(|f| f.kind == "store" && f.value.kind == "allocation")
            {
                for field in fields.get(&store.value).into_iter().flatten() {
                    let destination = store.target.split_path().0;
                    let source_context = self.allocation_contexts.get(&store.value);
                    let destination_context = self.allocation_contexts.get(destination);
                    let has_distinct_calls = source_context.is_some()
                        && destination_context.is_some()
                        && source_context != destination_context
                        && self.allocation_origins.get(&store.value)
                            == self.allocation_origins.get(destination);
                    if field.via.len() >= 6
                        || ((field.via.contains(&store.location)
                            || field.location == store.location)
                            && !has_distinct_calls)
                    {
                        continue;
                    }
                    let replacements =
                        BTreeMap::from([(store.value.clone(), store.target.clone())]);
                    let mut projected = field.clone();
                    projected.target = projected.target.substitute(&replacements);
                    projected.conditions.extend(store.conditions.clone());
                    projected.conditions.sort();
                    projected.conditions.dedup();
                    projected.via.push(store.location.clone());
                    if let Some(info) = self.value_types.get(&field.value).cloned() {
                        self.value_types.insert(projected.target.clone(), info);
                    }
                    added.push(projected);
                    if added.len() >= TOTAL_FACTS {
                        break;
                    }
                }
                if added.len() >= TOTAL_FACTS {
                    self.notices.insert((
                        "stored_field_projection_cap".into(),
                        TOTAL_FACTS.to_string(),
                    ));
                    break;
                }
            }
            facts.extend(added);
            dedup_facts(facts);
            self.cap_facts(facts);
        }
        self.refresh_heap(facts);
        self.project_polyglot(facts);
        let typed = self.project_typed_receivers(facts);
        facts.extend(typed);
        self.refresh_heap(facts);
        let copies = self.project_object_copies(facts);
        facts.extend(copies);
        self.refresh_heap(facts);
        let prototypes = self.project_prototype_bodies(facts);
        facts.extend(prototypes);
        self.cap_facts(facts);
        self.refresh_heap(facts);
        let mut added = Vec::new();
        let schema_roots: BTreeSet<_> = facts
            .iter()
            .filter(|f| f.kind == "store" && f.target.split_path().0.kind == "receiver")
            .map(|f| f.target.split_path().0.clone())
            .collect();
        for fact in facts
            .iter()
            .filter(|f| f.kind == "return" && f.target.split_path().0.kind == "parameter")
        {
            let Some(source) = self
                .program
                .sources
                .iter()
                .position(|s| s.path == fact.location.path)
            else {
                continue;
            };
            let root = fact.target.split_path().0;
            let owner = self.owner(root, source);
            let schema = Value::new("receiver", &owner);
            if owner.is_empty() || !schema_roots.contains(&schema) {
                continue;
            }
            let mut projected = fact.clone();
            projected.target = projected
                .target
                .substitute(&BTreeMap::from([(root.clone(), schema)]));
            projected.conditions.extend([
                "typed_parameter_receiver_binding_required".into(),
                "declared_receiver_schema_only".into(),
            ]);
            projected.conditions.sort();
            projected.conditions.dedup();
            added.push(projected);
        }
        let registrations: BTreeMap<Value, Vec<&Fact>> = facts
            .iter()
            .filter(|f| f.kind == "store" && f.value.kind == "binding")
            .fold(BTreeMap::new(), |mut map, f| {
                map.entry(f.value.clone()).or_default().push(f);
                map
            });
        for fact in facts.iter().filter(|f| {
            matches!(
                f.kind.as_str(),
                "store" | "invoke" | "member_invoke" | "key_lookup" | "return"
            )
        }) {
            let (root, keys) = fact.target.split_path();
            if keys.is_empty() {
                continue;
            }
            for registration in registrations.get(root).into_iter().flatten() {
                if registration.location == fact.location {
                    continue;
                }
                let mut projected = fact.clone();
                projected.target = projected.target.substitute(&BTreeMap::from([(
                    root.clone(),
                    registration.target.clone(),
                )]));
                projected.conditions.extend(registration.conditions.clone());
                projected.conditions.extend([
                    "explicit_stored_value_binding".into(),
                    "initializer_value_unresolved".into(),
                ]);
                projected.conditions.sort();
                projected.conditions.dedup();
                projected.via.push(registration.location.clone());
                added.push(projected);
                if added.len() >= 4096 {
                    break;
                }
            }
            if added.len() >= 4096 {
                self.notices
                    .insert(("stored_binding_projection_bound".into(), "4096".into()));
                break;
            }
        }
        facts.extend(added);
        dedup_facts(facts);
        self.cap_facts(facts);
        self.refresh_heap(facts);
        let callbacks = self.specialize_callback_arguments(facts);
        facts.extend(callbacks);
        dedup_facts(facts);
        self.cap_facts(facts);
        self.refresh_heap(facts);
        let forwarded = self.forward_callback_arguments(facts);
        facts.extend(forwarded);
        dedup_facts(facts);
        self.cap_facts(facts);
    }
    fn forward_callback_arguments(&mut self, facts: &[Fact]) -> Vec<Fact> {
        let mut result = Vec::new();
        for argument in facts.iter().filter(|f| {
            f.kind == "argument"
                && f.argument_index.is_some()
                && f.value.kind == "slot"
                && f.value.key().kind == "key"
        }) {
            let Some(source) = self
                .program
                .sources
                .iter()
                .position(|s| s.path == argument.location.path)
            else {
                continue;
            };
            let method = &argument.value.key().name;
            let receiver = self.promoted_receiver(argument.value.base(), method, source);
            for actual in self.read_values(&receiver) {
                let owner = self.owner(&actual, source);
                if actual.kind != "allocation" || owner.is_empty() {
                    continue;
                }
                let targets = self.program.method_candidates(&owner, method);
                if targets.len() != 1 {
                    continue;
                }
                let Some(summary) = self.summaries.get(&targets[0]) else {
                    continue;
                };
                let Some(formal) = summary.parameters.get(argument.argument_index.unwrap()) else {
                    continue;
                };
                for fact in summary.facts.iter().filter(|f| {
                    matches!(f.kind.as_str(), "invoke" | "member_invoke")
                        && f.target.split_path().0 == formal
                }) {
                    let mut mapped = fact.clone();
                    mapped.target = mapped
                        .target
                        .substitute(&BTreeMap::from([(formal.clone(), argument.target.clone())]));
                    mapped.conditions.extend(argument.conditions.clone());
                    mapped.conditions.extend([
                        "runtime_branch_selection_required".into(),
                        "dispatch_instance_unproven".into(),
                        format!("source_receiver_type:{owner}"),
                    ]);
                    mapped.conditions.sort();
                    mapped.conditions.dedup();
                    mapped.via.push(argument.location.clone());
                    result.push(mapped);
                }
                if result.len() >= 4096 {
                    self.notices
                        .insert(("callback_forwarding_cap".into(), "4096".into()));
                    return result;
                }
            }
        }
        result
    }
    pub fn promoted_receiver(&self, receiver: &Value, method: &str, source: usize) -> Value {
        if self.program.sources[source].language != "go" {
            return receiver.clone();
        }
        let owner = self.owner(receiver, source);
        if !self.program.method_candidates(&owner, method).is_empty() {
            return receiver.clone();
        }
        let candidates: Vec<_> = self
            .program
            .embedded_fields
            .get(&owner)
            .into_iter()
            .flatten()
            .map(|name| receiver.clone().field(name))
            .filter(|value| {
                let owner = self.owner(value, source);
                self.program
                    .interface_methods
                    .get(&owner)
                    .is_some_and(|names| names.contains(method))
                    || !self.program.method_candidates(&owner, method).is_empty()
            })
            .collect();
        if candidates.len() == 1 {
            candidates[0].clone()
        } else {
            receiver.clone()
        }
    }
}
