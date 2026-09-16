//! Instantiate selected source prototype bodies on their stored receivers.
//! Execution remains an obligation; construction never proves invocation.
use super::engine::*;
use super::model::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn callable_origin(analyzer: &Analyzer<'_>, value: &Value, depth: usize) -> String {
    let Some(function) = analyzer
        .program
        .functions
        .iter()
        .find(|f| f.identifier == value.name)
    else {
        return String::new();
    };
    let source = &analyzer.program.sources[function.source];
    if value.kind != "closure" || depth >= 6 {
        return source.path.clone();
    }
    let captures = closure_captures(value);
    let mut paths = BTreeSet::new();
    for id in function
        .body
        .map(|n| source.walk(n, false))
        .unwrap_or_default()
    {
        if source.nodes[id].kind != "call_expression" {
            continue;
        }
        let mut callee = source.child(id, &["function"]);
        if let Some(member) = callee.filter(|n| source.nodes[*n].kind == "member_expression") {
            if matches!(
                source.text(source.child(member, &["property"])),
                "call" | "apply"
            ) {
                callee = source.child(member, &["object"]);
            }
        }
        if let Some(callee) = callee.filter(|n| source.nodes[*n].kind == "identifier") {
            if let Some(actual) = captures
                .get(source.text(Some(callee)))
                .filter(|v| matches!(v.kind.as_str(), "function" | "closure") && *v != value)
            {
                paths.insert(callable_origin(analyzer, actual, depth + 1));
            }
        }
    }
    if paths.len() == 1 {
        paths.into_iter().next().unwrap()
    } else {
        source.path.clone()
    }
}
fn has_callable(
    analyzer: &Analyzer<'_>,
    by_parent: &BTreeMap<Value, Vec<Fact>>,
    value: &Value,
    depth: usize,
) -> bool {
    if matches!(value.kind.as_str(), "function" | "closure") {
        return true;
    }
    if depth >= 3 {
        return false;
    }
    analyzer
        .read_values(value)
        .into_iter()
        .chain(
            by_parent
                .get(value)
                .into_iter()
                .flatten()
                .map(|f| f.value.clone()),
        )
        .take(32)
        .any(|v| {
            v.options()
                .iter()
                .any(|v| has_callable(analyzer, by_parent, v, depth + 1))
        })
}
impl Analyzer<'_> {
    pub fn project_prototype_bodies(&mut self, facts: &[Fact]) -> Vec<Fact> {
        let mut added = Vec::new();
        let mut visited = BTreeSet::new();
        for generation in 0..6 {
            let previous = added.len();
            let mut parents: BTreeMap<Value, Vec<Fact>> = BTreeMap::new();
            for f in facts
                .iter()
                .chain(&added)
                .filter(|f| f.kind == "store" && f.target.kind == "slot")
            {
                parents
                    .entry(f.target.base().clone())
                    .or_default()
                    .push(f.clone());
            }
            let mut pairs: Vec<_> = self
                .prototypes
                .iter()
                .map(|(i, p)| (i.clone(), p.clone()))
                .collect();
            pairs.sort_by_key(|(instance, _)| {
                let refs: BTreeSet<_> = parents
                    .get(instance)
                    .into_iter()
                    .flatten()
                    .flat_map(|f| f.value.referenced())
                    .filter(|v| v.kind == "slot")
                    .collect();
                std::cmp::Reverse((
                    refs.iter()
                        .filter(|v| {
                            matches!(
                                v.split_path().0.kind.as_str(),
                                "allocation" | "receiver" | "global"
                            )
                        })
                        .count(),
                    refs.len(),
                    self.allocation_contexts.get(instance).map_or(0, Vec::len),
                ))
            });
            let mut groups: BTreeMap<String, VecDeque<(Value, Value)>> = BTreeMap::new();
            for (instance, prototype) in pairs {
                let callback = parents
                    .get(&instance)
                    .into_iter()
                    .flatten()
                    .flat_map(|f| f.value.options())
                    .find(|v| {
                        matches!(v.kind.as_str(), "function" | "closure")
                            && self
                                .program
                                .functions
                                .iter()
                                .any(|f| f.identifier == v.name)
                    });
                let origin = callback
                    .map(|v| callable_origin(self, &v, 0))
                    .unwrap_or_default();
                groups
                    .entry(origin)
                    .or_default()
                    .push_back((instance, prototype));
            }
            let mut ordered = Vec::new();
            loop {
                let mut found = false;
                for group in groups.values_mut() {
                    if let Some(pair) = group.pop_front() {
                        ordered.push(pair);
                        found = true;
                    }
                }
                if !found {
                    break;
                }
            }
            let remaining = (256usize.saturating_sub(visited.len())) / (6 - generation);
            let mut processed = 0;
            for (instance, prototype) in ordered {
                if processed >= remaining {
                    self.notices
                        .insert(("prototype_instance_summary_bound".into(), "256".into()));
                    break;
                }
                let keys: BTreeSet<_> = parents
                    .get(&prototype)
                    .into_iter()
                    .flatten()
                    .map(|f| f.target.key().clone())
                    .collect();
                for key in keys {
                    let methods: Vec<_> = self
                        .read_values(&prototype.clone().slot(key.clone()))
                        .into_iter()
                        .filter(|v| matches!(v.kind.as_str(), "function" | "closure"))
                        .collect();
                    if methods.len() != 1
                        || visited.contains(&(instance.clone(), key.clone(), methods[0].clone()))
                    {
                        continue;
                    }
                    let method = &methods[0];
                    let Some(target) = self
                        .program
                        .functions
                        .iter()
                        .position(|f| f.identifier == method.name)
                    else {
                        continue;
                    };
                    let function = self.program.functions[target].clone();
                    let source = self.program.sources[function.source].clone();
                    let callback_keys: Vec<_> = function
                        .body
                        .map(|n| source.walk(n, true))
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|n| {
                            source.nodes[*n].kind == "subscript_expression"
                                && source.text(source.child(*n, &["object"])) == "this"
                        })
                        .filter_map(|n| source.child(n, &["index"]))
                        .collect();
                    if callback_keys.is_empty() {
                        continue;
                    }
                    let mut interpreter = Interpreter::new(self, function.source, None);
                    if method.kind == "closure" {
                        interpreter.env.extend(closure_captures(method));
                    }
                    let has_callback = callback_keys.into_iter().any(|id| {
                        let key = interpreter.expression(Some(id), 0, true);
                        has_callable(
                            interpreter.analyzer,
                            &parents,
                            &instance.clone().slot(key),
                            0,
                        )
                    });
                    if !has_callback {
                        continue;
                    }
                    if visited.len() >= 256 {
                        interpreter
                            .analyzer
                            .notices
                            .insert(("prototype_instance_summary_bound".into(), "256".into()));
                        return added;
                    }
                    visited.insert((instance.clone(), key, method.clone()));
                    processed += 1;
                    interpreter.conditions.extend([
                        "prototype_method_execution_required".into(),
                        "source_constructor_prototype_binding".into(),
                    ]);
                    interpreter.apply_source(
                        target,
                        instance.clone(),
                        Vec::new(),
                        function.node,
                        (method.kind == "closure").then(|| closure_captures(method)),
                    );
                    added.extend(interpreter.facts);
                }
            }
            dedup_facts(&mut added);
            self.cap_facts(&mut added);
            let mut combined = facts.to_vec();
            combined.extend(added.clone());
            self.refresh_heap(&combined);
            if added.len() == previous {
                break;
            }
        }
        added
    }
}
