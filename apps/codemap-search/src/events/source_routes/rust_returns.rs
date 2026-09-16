use super::engine::Interpreter;
use super::model::*;
use super::syntax::*;

impl Interpreter<'_, '_> {
    pub fn match_value(&mut self, id: NodeId, depth: usize) -> Value {
        let value = self.expression(self.source.child(id, &["value"]), depth + 1, true);
        let Some(body) = self.source.child(id, &["body"]) else {
            return Value::unknown();
        };
        let env = self.env.clone();
        let conditions = self.conditions.clone();
        let mut values = Vec::new();
        let mut origins = Vec::new();
        for arm in self.source.nodes[body].children.clone() {
            if self.source.nodes[arm].kind != "match_arm" {
                continue;
            }
            self.env = env.clone();
            self.conditions = conditions.clone();
            self.conditions.push("match_arm_selection_required".into());
            let mut pattern = self.source.child(arm, &["pattern"]);
            if pattern.is_some_and(|n| self.source.nodes[n].kind == "match_pattern") {
                pattern = pattern.and_then(|n| self.source.nodes[n].children.first().copied());
            }
            if let Some(pattern) =
                pattern.filter(|n| self.source.nodes[*n].kind == "tuple_struct_pattern")
            {
                let ty = self.source.child(pattern, &["type"]);
                if self.rust_standard_path(
                    self.source.text(ty),
                    &["std::option::Option::Some", "core::option::Option::Some"],
                    Some("Some"),
                ) {
                    let options: Vec<_> = value
                        .options()
                        .into_iter()
                        .filter(|v| v.kind == "wrapper" && v.name == "rust:option")
                        .map(|v| v.base().clone())
                        .collect();
                    if options.is_empty() {
                        continue;
                    }
                    let bindings: Vec<_> = self.source.nodes[pattern]
                        .children
                        .iter()
                        .copied()
                        .filter(|n| Some(*n) != ty)
                        .collect();
                    if bindings.len() == 1 {
                        self.bind(Some(bindings[0]), Value::merge(options), "");
                    }
                }
            } else if self.source.text(pattern) == "None" {
                values.push(Value::nested(
                    "wrapper",
                    "rust:option",
                    Value::unknown(),
                    None,
                ));
                continue;
            }
            let origin = self.source.child(arm, &["value"]);
            let returned = self.expression(origin, depth + 1, true);
            values.push(returned.clone());
            if let Some(origin) = origin {
                origins.extend(
                    self.block_origins
                        .get(&origin)
                        .cloned()
                        .unwrap_or_else(|| vec![(returned, origin)]),
                );
            }
        }
        self.env = env;
        self.conditions = conditions;
        self.block_origins.insert(id, origins);
        Value::merge(values)
    }
    pub fn resolve_call_result(&mut self, value: Value, id: NodeId) -> Value {
        if self.source.language != "rust" || value.kind != "result" || self.depth >= 6 {
            return value;
        }
        if let Some(resolved) = self.resolved_call_results.get(&value) {
            return resolved.clone();
        }
        let record = self
            .calls
            .iter()
            .rev()
            .find(|c| c.result == value)
            .cloned()
            .or_else(|| {
                self.analyzer
                    .summaries
                    .values()
                    .flat_map(|s| &s.calls)
                    .find(|c| c.result == value)
                    .cloned()
            });
        let Some(record) = record.filter(|c| c.candidates.len() == 1) else {
            return value;
        };
        let Some(target) = self
            .function_index(&record.candidates[0])
            .filter(|t| Some(*t) != self.function)
        else {
            return value;
        };
        if self.analyzer.summaries.get(&target).is_some_and(|s| {
            s.facts.iter().any(|f| {
                f.kind == "remove"
                    || (f.kind == "store"
                        && f.target.split_path().0.kind != "allocation"
                        && !f.conditions.iter().any(|c| c == "constructor_schema_only"))
            })
        }) {
            self.analyzer.notices.insert((
                "source_return_requires_effect_context".into(),
                record.candidates[0].clone(),
            ));
            return value;
        }
        let conditions = self.conditions.clone();
        self.conditions.extend(record.conditions);
        self.conditions
            .push("source_return_revisited_at_consumer".into());
        let start = self.facts.len();
        self.next_rust_bindings = record
            .type_bindings
            .iter()
            .map(|(name, ty)| {
                (
                    name.clone(),
                    super::rust_types::RustType::named("nominal", ty),
                )
            })
            .collect();
        let returned = self.apply_source(target, record.receiver, record.arguments, id, None);
        self.conditions = conditions;
        for fact in &mut self.facts[start..] {
            if fact.via.len() < 8 && !fact.via.contains(&record.location) {
                fact.via.push(record.location.clone());
            }
        }
        let resolved = if !matches!(returned.kind.as_str(), "unknown" | "result") {
            returned
        } else {
            value.clone()
        };
        self.resolved_call_results.insert(value, resolved.clone());
        resolved
    }
}
