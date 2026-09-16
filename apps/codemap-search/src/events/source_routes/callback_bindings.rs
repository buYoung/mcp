use super::engine::*;
use super::model::*;
use std::collections::{BTreeMap, BTreeSet};

fn callable_targets(
    analyzer: &Analyzer<'_>,
    bindings: &BTreeMap<Value, Vec<Fact>>,
    formal: &Value,
    seen: &[Value],
) -> Vec<(Value, Vec<Fact>)> {
    if seen.contains(formal) || seen.len() >= 6 {
        return Vec::new();
    }
    let mut seen = seen.to_vec();
    seen.push(formal.clone());
    let mut result = Vec::new();
    for binding in bindings.get(formal).into_iter().flatten() {
        let mut actuals = binding.value.options();
        actuals.extend(analyzer.read_values(&binding.value));
        for actual in actuals {
            if matches!(actual.kind.as_str(), "function" | "closure") {
                result.push((actual, vec![binding.clone()]));
            } else if actual.kind == "parameter" {
                for (value, mut trail) in callable_targets(analyzer, bindings, &actual, &seen) {
                    trail.insert(0, binding.clone());
                    result.push((value, trail));
                }
            }
            if result.len() >= 16 {
                result.truncate(16);
                return result;
            }
        }
    }
    result
}
impl Analyzer<'_> {
    pub fn specialize_callback_arguments(&mut self, facts: &[Fact]) -> Vec<Fact> {
        let mut bindings: BTreeMap<Value, Vec<Fact>> = BTreeMap::new();
        let mut calls: BTreeMap<(Value, Location, String), Vec<Fact>> = BTreeMap::new();
        for fact in facts {
            if fact.kind == "parameter_binding" {
                bindings
                    .entry(fact.target.clone())
                    .or_default()
                    .push(fact.clone());
            } else if fact.kind == "argument"
                && fact.value.kind == "parameter"
                && fact.argument_index.is_some()
            {
                calls
                    .entry((
                        fact.value.clone(),
                        fact.location.clone(),
                        fact.function.clone(),
                    ))
                    .or_default()
                    .push(fact.clone());
            }
        }
        let mut result = Vec::new();
        let mut visited = BTreeSet::new();
        for ((formal, location, owner), arguments) in calls {
            let formal_owner = formal
                .name
                .rsplit_once(':')
                .map(|p| p.0)
                .unwrap_or_default();
            let mut enclosing = BTreeSet::new();
            let mut function = self
                .program
                .functions
                .iter()
                .position(|f| f.identifier == owner);
            while let Some(index) = function {
                let f = &self.program.functions[index];
                enclosing.insert(f.identifier.clone());
                function = f.parent;
            }
            if !enclosing.contains(formal_owner) {
                continue;
            }
            let roots: BTreeSet<_> = arguments
                .iter()
                .map(|f| f.target.split_path().0.clone())
                .collect();
            if roots.iter().any(|root| {
                matches!(root.kind.as_str(), "allocation" | "binding")
                    && (self
                        .allocation_contexts
                        .get(root)
                        .is_some_and(|v| !v.is_empty())
                        || root.name.contains('~')
                        || self
                            .allocation_owners
                            .get(root)
                            .is_none_or(|o| !enclosing.contains(o)))
            }) {
                continue;
            }
            let arity = arguments
                .iter()
                .filter_map(|f| f.argument_index)
                .max()
                .unwrap()
                + 1;
            let supplied: Vec<_> = (0..arity)
                .map(|index| {
                    Value::merge(
                        arguments
                            .iter()
                            .filter(|f| f.argument_index == Some(index))
                            .map(|f| f.target.clone()),
                    )
                })
                .collect();
            let Some(source_index) = self
                .program
                .sources
                .iter()
                .position(|s| s.path == location.path)
            else {
                continue;
            };
            let source = self.program.sources[source_index].clone();
            let nodes: Vec<_> = source
                .walk(0, false)
                .into_iter()
                .filter(|id| {
                    source.nodes[*id].kind == "call_expression"
                        && source.location(*id) == location
                        && source.expansion(*id).is_none()
                })
                .collect();
            if nodes.len() != 1 {
                continue;
            }
            for (callback, trail) in callable_targets(self, &bindings, &formal, &[]) {
                if !visited.insert((
                    callback.clone(),
                    supplied.clone(),
                    location.clone(),
                    owner.clone(),
                )) {
                    continue;
                }
                if visited.len() > 256 {
                    self.notices
                        .insert(("callback_binding_summary_bound".into(), "256".into()));
                    return result;
                }
                let Some(target) = self
                    .program
                    .functions
                    .iter()
                    .position(|f| f.identifier == callback.name)
                else {
                    continue;
                };
                let mut conditions: BTreeSet<String> = [
                    "source_callable_parameter_binding",
                    "enclosing_callable_execution_unproven",
                    "enclosing_function_schema_only",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect();
                for fact in arguments.iter().chain(&trail) {
                    conditions.extend(fact.conditions.clone());
                }
                let mut interpreter = Interpreter::new(self, source_index, None);
                interpreter.conditions = conditions.into_iter().collect();
                interpreter.apply_source(
                    target,
                    Value::unknown(),
                    supplied.clone(),
                    nodes[0],
                    (callback.kind == "closure").then(|| closure_captures(&callback)),
                );
                for mut fact in interpreter.facts {
                    if fact.kind == "key_lookup"
                        || (matches!(
                            fact.kind.as_str(),
                            "invoke" | "member_invoke" | "argument" | "return"
                        ) && supplied
                            .iter()
                            .any(|v| v.split_path().0 == fact.target.split_path().0))
                    {
                        for binding in &trail {
                            if !fact.via.contains(&binding.location) {
                                fact.via.push(binding.location.clone());
                            }
                        }
                        result.push(fact);
                    }
                }
            }
        }
        dedup_facts(&mut result);
        result
    }
}
