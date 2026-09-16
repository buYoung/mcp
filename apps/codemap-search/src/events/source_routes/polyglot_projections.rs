use super::engine::*;
use super::model::*;
use std::collections::{BTreeMap, BTreeSet};

impl Analyzer<'_> {
    pub fn resolved_aliases(&self, target: &Value) -> Vec<Value> {
        let mut seen = BTreeSet::from([target.clone()]);
        let mut pending = vec![target.clone()];
        let mut values = Vec::new();
        for _ in 0..6 {
            let mut next = Vec::new();
            for current in pending {
                for (alias, _, _) in self.target_aliases(&current, 0) {
                    if seen.insert(alias.clone()) {
                        values.push(alias.clone());
                        next.push(alias);
                        if seen.len() >= 96 {
                            return values;
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            pending = next;
        }
        values
    }
    pub fn target_aliases(
        &self,
        target: &Value,
        depth: usize,
    ) -> Vec<(Value, Vec<String>, Vec<Location>)> {
        if depth >= 6 {
            return Vec::new();
        }
        let mut candidates: Vec<_> = self
            .heap_facts
            .get(target)
            .into_iter()
            .flatten()
            .map(|f| (f, Vec::new()))
            .collect();
        if candidates.is_empty() && target.kind == "slot" {
            let (root, keys) = target.split_path();
            for fact in self
                .heap_roots
                .get(root)
                .into_iter()
                .flatten()
                .filter(|f| f.target.split_path().1.len() == keys.len())
            {
                if let Some(conditions) = match_storage(&fact.target, target) {
                    candidates.push((fact, conditions));
                }
            }
        }
        let mut values = Vec::new();
        for (fact, conditions) in candidates.into_iter().take(8) {
            for option in fact.value.options() {
                if matches!(
                    option.kind.as_str(),
                    "slot" | "receiver" | "parameter" | "allocation"
                ) {
                    let mut conditions = conditions.clone();
                    conditions.extend(fact.conditions.clone());
                    conditions.sort();
                    conditions.dedup();
                    let mut via = fact.via.clone();
                    via.push(fact.location.clone());
                    values.push((option, conditions, via));
                }
            }
        }
        if target.kind == "slot" {
            values.extend(
                self.target_aliases(target.base(), depth + 1)
                    .into_iter()
                    .map(|(v, c, p)| (v.slot(target.key().clone()), c, p)),
            );
        }
        values.sort();
        values.dedup();
        values.truncate(32);
        values
    }
    pub fn project_polyglot(&mut self, facts: &mut Vec<Fact>) {
        let is_poly = |path: &str| {
            self.program.sources.iter().any(|s| {
                s.path == path
                    && !matches!(
                        s.language.as_str(),
                        "javascript" | "typescript" | "go" | "rust" | "assembly"
                    )
            })
        };
        let selected: Vec<_> = facts
            .iter()
            .filter(|f| is_poly(&f.location.path))
            .cloned()
            .collect();
        if selected.is_empty() {
            return;
        }
        let mut fields: BTreeMap<Value, Vec<Fact>> = BTreeMap::new();
        for fact in selected.iter().filter(|f| {
            f.kind == "store" && f.target.kind == "slot" && f.target.key().kind == "key"
        }) {
            fields
                .entry(fact.target.base().clone())
                .or_default()
                .push(fact.clone());
        }
        let mut added = Vec::new();
        for store in selected.iter().filter(|f| f.kind == "store") {
            let mut candidates = fields.get(&store.value).cloned().unwrap_or_default();
            if candidates.is_empty() && store.value.kind == "parameter" {
                if let Some(source) = self
                    .program
                    .sources
                    .iter()
                    .position(|s| s.path == store.location.path)
                {
                    let owner = self.owner(&store.value, source);
                    if !owner.is_empty() {
                        candidates = fields
                            .get(&Value::new("receiver", owner))
                            .cloned()
                            .unwrap_or_default();
                    }
                }
            }
            for member in candidates.into_iter().take(16) {
                if member.target.base() == &store.target {
                    continue;
                }
                let target = store.target.clone().slot(member.target.key().clone());
                let value = if member.target.base() == &store.value {
                    member.value
                } else {
                    store.value.clone().slot(member.target.key().clone())
                };
                let mut conditions = store.conditions.clone();
                conditions.extend(member.conditions);
                conditions.push("stored_object_field_projection".into());
                conditions.sort();
                conditions.dedup();
                let mut fact = Fact::new(
                    "store",
                    target,
                    value,
                    store.location.clone(),
                    &store.function,
                    conditions,
                );
                fact.via = store.via.clone();
                fact.via.push(member.location);
                added.push(fact);
            }
            if facts.len() + added.len() >= TOTAL_FACTS {
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
        self.refresh_heap(facts);
        let mut allocations: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (value, owner) in &self.owners {
            if value.kind == "allocation" {
                allocations
                    .entry(owner.clone())
                    .or_default()
                    .push(value.clone());
            }
        }
        let mut result = Vec::new();
        for fact in selected.iter().filter(|f| {
            matches!(
                f.kind.as_str(),
                "store" | "invoke" | "member_invoke" | "argument"
            )
        }) {
            let root = fact.target.split_path().0;
            let mut targets = vec![(
                fact.target.clone(),
                fact.conditions.clone(),
                fact.via.clone(),
            )];
            if fact.kind != "store" {
                if let Some(owner) = self.owners.get(root).filter(|o| !o.is_empty()) {
                    targets.push((
                        fact.target.substitute(&BTreeMap::from([(
                            root.clone(),
                            Value::new("receiver", owner),
                        )])),
                        fact.conditions.clone(),
                        fact.via.clone(),
                    ));
                }
                if root.kind == "receiver" {
                    for allocation in allocations.get(&root.name).into_iter().flatten().take(8) {
                        targets.push((
                            fact.target
                                .substitute(&BTreeMap::from([(root.clone(), allocation.clone())])),
                            fact.conditions.clone(),
                            fact.via.clone(),
                        ));
                    }
                }
            }
            let mut visited = BTreeSet::new();
            for _ in 0..4 {
                let mut next = Vec::new();
                for (target, conditions, via) in targets {
                    if !visited.insert(target.clone()) {
                        continue;
                    }
                    if target != fact.target {
                        let mut projected = fact.clone();
                        projected.target = target.clone();
                        projected.conditions = conditions.clone();
                        projected.conditions.extend([
                            "receiver_dispatch_instance_unproven".into(),
                            "explicit_store_projection".into(),
                        ]);
                        projected.conditions.sort();
                        projected.conditions.dedup();
                        projected.via = via.clone();
                        result.push(projected);
                    }
                    let aliases = if fact.kind == "store" {
                        if target.kind == "slot" {
                            self.target_aliases(target.base(), 0)
                                .into_iter()
                                .map(|(v, c, p)| (v.slot(target.key().clone()), c, p))
                                .collect()
                        } else {
                            Vec::new()
                        }
                    } else {
                        self.target_aliases(&target, 0)
                    };
                    for (value, extra, trail) in aliases {
                        let mut conditions = conditions.clone();
                        conditions.extend(extra);
                        conditions.sort();
                        conditions.dedup();
                        let mut via = via.clone();
                        for point in trail {
                            if !via.contains(&point) {
                                via.push(point);
                            }
                        }
                        next.push((value, conditions, via));
                    }
                    if target != fact.target {
                        let alias_root = target.split_path().0;
                        if alias_root.kind == "parameter" {
                            if let Some(owner) =
                                self.owners.get(alias_root).filter(|o| !o.is_empty())
                            {
                                next.push((
                                    target.substitute(&BTreeMap::from([(
                                        alias_root.clone(),
                                        Value::new("receiver", owner),
                                    )])),
                                    conditions,
                                    via,
                                ));
                            }
                        }
                    }
                }
                if result.len() >= TOTAL_FACTS {
                    self.notices
                        .insert(("projection_fact_cap".into(), TOTAL_FACTS.to_string()));
                    break;
                }
                next.truncate(32);
                targets = next;
            }
        }
        facts.extend(result);
        dedup_facts(facts);
        self.cap_facts(facts);
        self.refresh_heap(facts);
    }
}
