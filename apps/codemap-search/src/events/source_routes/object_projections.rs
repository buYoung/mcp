use super::engine::*;
use super::model::*;
use std::collections::{BTreeMap, BTreeSet};

impl Analyzer<'_> {
    fn explicit_object_keys(&self, value: &Value) -> Option<BTreeSet<String>> {
        if value.kind != "allocation" {
            return None;
        }
        let origin = Value::new(
            "allocation",
            self.allocation_origins.get(value).unwrap_or(&value.name),
        );
        let mut keys = self.literal_fields.get(&origin)?.clone();
        for target in self.heap.keys() {
            if target.kind == "slot" && target.base() == value {
                if target.key().kind != "key" {
                    return None;
                }
                keys.insert(target.key().name.clone());
            }
        }
        Some(keys)
    }
    fn copied_property_paths(
        &self,
        source: &Value,
        suffix: &[&Value],
        excluded: &BTreeSet<Value>,
    ) -> Vec<Vec<Value>> {
        let mut paths = vec![(source.clone(), Vec::new())];
        for (index, requested) in suffix.iter().enumerate() {
            let mut next = Vec::new();
            for (value, keys) in paths {
                let known = self.explicit_object_keys(&value);
                let choices = if !matches!(requested.kind.as_str(), "key" | "literal") {
                    known
                        .as_ref()
                        .map(|set| set.iter().map(|name| Value::new("key", name)).collect())
                        .unwrap_or_else(|| vec![(*requested).clone()])
                } else {
                    vec![(*requested).clone()]
                };
                for key in choices {
                    if index == 0 && excluded.contains(&key) {
                        continue;
                    }
                    if known
                        .as_ref()
                        .is_some_and(|known| key.kind == "key" && !known.contains(&key.name))
                    {
                        continue;
                    }
                    let target = value.clone().slot(key.clone());
                    let actuals = self.read_values(&target);
                    let actuals = if actuals.is_empty() {
                        vec![target]
                    } else {
                        actuals
                    };
                    for actual in actuals {
                        let mut path = keys.clone();
                        path.push(key.clone());
                        next.push((actual, path));
                    }
                }
            }
            next.sort();
            next.dedup();
            next.truncate(16);
            paths = next;
        }
        let mut keys: Vec<_> = paths.into_iter().map(|p| p.1).collect();
        keys.sort();
        keys.dedup();
        keys
    }
    pub fn project_object_copies(&mut self, facts: &[Fact]) -> Vec<Fact> {
        let mut copies: BTreeMap<Value, Vec<&Fact>> = BTreeMap::new();
        let mut calls: BTreeMap<Value, Vec<&Fact>> = BTreeMap::new();
        for fact in facts {
            if fact.kind == "copy_properties" {
                copies.entry(fact.target.clone()).or_default().push(fact);
            }
            if matches!(fact.kind.as_str(), "invoke" | "member_invoke" | "argument") {
                calls
                    .entry(fact.target.split_path().0.clone())
                    .or_default()
                    .push(fact);
            }
        }
        let mut result = Vec::new();
        for store in facts
            .iter()
            .filter(|f| f.kind == "store" && copies.contains_key(&f.value))
        {
            let (root, keys) = store.target.split_path();
            for call in calls.get(root).into_iter().flatten() {
                let called = call.target.split_path().1;
                if called.len() <= keys.len() || called.len() > keys.len() + 3 {
                    continue;
                }
                let suffix = &called[keys.len()..];
                let Some(conditions) = match_storage(&store.target, &call.target) else {
                    continue;
                };
                let copying = &copies[&store.value];
                for (index, copied) in copying.iter().enumerate() {
                    let mut excluded: BTreeSet<_> =
                        copied.value.key().tuple_values().into_iter().collect();
                    for later in &copying[index + 1..] {
                        if let Some(keys) = self.explicit_object_keys(later.value.base()) {
                            excluded.extend(keys.into_iter().map(|k| Value::new("key", k)));
                        }
                    }
                    let source = copied.value.base();
                    for path in self.copied_property_paths(source, suffix, &excluded) {
                        let mut target = store.target.clone();
                        let mut value = source.clone();
                        for key in path {
                            target = target.slot(key.clone());
                            value = value.slot(key);
                        }
                        let mut obligations = store.conditions.clone();
                        obligations.extend(copied.conditions.clone());
                        obligations.extend(conditions.clone());
                        obligations.extend(
                            [
                                "own_enumerable_property_required",
                                "copied_property_not_overridden",
                                "nested_member_presence_required",
                                "object_copy_registration_projection",
                            ]
                            .map(str::to_owned),
                        );
                        obligations.sort();
                        obligations.dedup();
                        let mut fact = Fact::new(
                            "store",
                            target,
                            value,
                            store.location.clone(),
                            &store.function,
                            obligations,
                        );
                        fact.via = store.via.clone();
                        fact.via.extend(copied.via.clone());
                        fact.via.push(copied.location.clone());
                        fact.consumer = Some(Consumer {
                            location: call.location.clone(),
                            target: call.target.clone(),
                            kind: call.kind.clone(),
                            argument_index: call.argument_index,
                        });
                        result.push(fact);
                        if result.len() + facts.len() >= TOTAL_FACTS {
                            self.notices.insert((
                                "object_copy_projection_cap".into(),
                                TOTAL_FACTS.to_string(),
                            ));
                            return result;
                        }
                    }
                }
            }
        }
        result
    }
    pub fn project_typed_receivers(&self, facts: &[Fact]) -> Vec<Fact> {
        let mut schemas: BTreeMap<String, Vec<&Fact>> = BTreeMap::new();
        let mut bindings: BTreeMap<Value, Vec<&Fact>> = BTreeMap::new();
        for fact in facts.iter().filter(|f| f.kind == "store") {
            bindings.entry(fact.target.clone()).or_default().push(fact);
            let root = fact.target.split_path().0;
            if root.kind == "receiver"
                && fact.via.is_empty()
                && self
                    .program
                    .functions
                    .iter()
                    .any(|f| f.identifier == fact.function && f.owner == root.name)
            {
                schemas.entry(root.name.clone()).or_default().push(fact);
            }
        }
        let mut result = Vec::new();
        for (prefix, stores) in bindings {
            if prefix.kind != "slot"
                || prefix.split_path().0.kind != "receiver"
                || !stores
                    .iter()
                    .all(|s| matches!(s.value.kind.as_str(), "result" | "unknown" | "unresolved"))
            {
                continue;
            }
            let Some(source) = self.program.sources.iter().position(|s| {
                s.path == stores[0].location.path
                    && matches!(s.language.as_str(), "typescript" | "javascript")
            }) else {
                continue;
            };
            let owner = self.owner(&prefix, source);
            for store in schemas.get(&owner).into_iter().flatten() {
                for binding in &stores {
                    let mut fact = (*store).clone();
                    fact.target = fact.target.substitute(&BTreeMap::from([(
                        Value::new("receiver", &owner),
                        prefix.clone(),
                    )]));
                    fact.conditions.extend(binding.conditions.clone());
                    fact.conditions.extend(
                        [
                            "opaque_receiver_binding_required",
                            "declared_receiver_schema_only",
                            "method_dispatch_unproven",
                        ]
                        .map(str::to_owned),
                    );
                    fact.conditions.sort();
                    fact.conditions.dedup();
                    fact.via.extend(binding.via.clone());
                    fact.via.push(binding.location.clone());
                    result.push(fact);
                }
            }
        }
        result
    }
}
