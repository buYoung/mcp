//! Compose proven API contracts into conditional facts, not execution events.
mod cargo;
mod rust_types;
use super::*;
use crate::flow::outcome::{Condition, ConditionalOutcome, ReturnedValue, ValueType};
use crate::parser::CodeRange;

fn method(unit: &FlowUnit, id: NodeId, name: &str) -> Option<(NodeId, Vec<NodeId>)> {
    let ExpressionKind::Call { callee, arguments } = &unit.nodes.get(id)?.kind else {
        return None;
    };
    let ExpressionKind::Field { object, key } = &unit.nodes.get(*callee)?.kind else {
        return None;
    };
    matches!(&unit.nodes.get(*key)?.kind, ExpressionKind::Literal(Constant::String(value)) if value == name)
        .then(|| (*object, arguments.clone()))
}

fn input_binding(unit: &FlowUnit, mut receiver: NodeId) -> Option<BindingId> {
    for _ in 0..8 {
        let ExpressionKind::Read {
            binding: Some(binding),
            ..
        } = &unit.nodes.get(receiver)?.kind
        else {
            return None;
        };
        let definition = unit.bindings.get(*binding)?;
        if definition.is_mutated {
            return None;
        }
        if let Some(initializer) = definition.initializer {
            if matches!(
                unit.nodes.get(initializer)?.kind,
                ExpressionKind::Read {
                    binding: Some(_),
                    ..
                }
            ) {
                receiver = initializer;
                continue;
            }
        }
        return Some(*binding);
    }
    None
}

fn merge(left: ReturnedValue, right: ReturnedValue) -> ReturnedValue {
    if left == right {
        return left;
    }
    match (left, right) {
        (ReturnedValue::Tuple(left), ReturnedValue::Tuple(right)) if left.len() == right.len() => {
            ReturnedValue::Tuple(
                left.into_iter()
                    .zip(right)
                    .map(|(a, b)| merge(a, b))
                    .collect(),
            )
        }
        _ => ReturnedValue::Unknown,
    }
}

impl Query<'_> {
    fn outcome_value(&self, value: ValueId, remaining: &mut usize, depth: usize) -> ReturnedValue {
        if *remaining == 0 || depth > 8 {
            return ReturnedValue::Unknown;
        }
        *remaining -= 1;
        match &self.values[value].kind {
            ValueKind::Constant(value) => ReturnedValue::Constant(value.clone()),
            ValueKind::Tuple(values) => ReturnedValue::Tuple(
                values
                    .iter()
                    .map(|value| self.outcome_value(*value, remaining, depth + 1))
                    .collect(),
            ),
            ValueKind::Alternatives(values) => values
                .iter()
                .map(|value| self.outcome_value(*value, remaining, depth + 1))
                .reduce(merge)
                .unwrap_or(ReturnedValue::Unknown),
            _ => ReturnedValue::Unknown,
        }
    }
    fn conditional_factory(&mut self, key: FunctionKey) -> (ReturnedValue, Vec<Step>) {
        let Some(location) = self.index.location(&key) else {
            return (ReturnedValue::Unknown, Vec::new());
        };
        // Reuse the existing branch/tuple evaluator in an isolated context. Its
        // calls and writes are hypothetical, and never become execution facts in
        // the caller. Charge every operation/value/read to the original request.
        let mut budget = RequestBudget::default();
        self.save_budget(&mut budget);
        budget.deadline = Some(self.deadline);
        let mut summary = Query::new(self.index, self.root, self.scope);
        summary.should_collect_outcomes = false;
        summary.restore_budget(&mut budget);
        let closure = summary.closure(key, BTreeMap::new(), None);
        let value = summary.invoke(closure, Vec::new(), None, &location);
        let result = summary.outcome_value(value, &mut 64, 0);
        summary.save_budget(&mut budget);
        self.operations = summary.operations;
        self.source_bytes = summary.source_bytes;
        self.files = std::mem::take(&mut budget.files);
        self.deadline = summary.deadline;
        self.work_limit = summary.work_limit;
        self.prior_values = budget
            .values
            .saturating_sub(self.values.len().saturating_sub(1));
        self.prior_steps = budget
            .steps
            .saturating_sub(self.steps.len() + self.outcomes.len());
        let calls = summary
            .steps
            .into_iter()
            .filter(|step| step.relation == "source-resolved call" && step.from.name != "<closure>")
            .collect();
        (result, calls)
    }
    pub(super) fn collect_outcomes(&mut self, anchors: &[(String, usize, usize)]) {
        let paths = anchors
            .iter()
            .map(|anchor| anchor.0.clone())
            .collect::<BTreeSet<_>>();
        for path in paths {
            if !path.ends_with(".rs") {
                continue;
            }
            let Some(file) = self.index.file(&path) else {
                continue;
            };
            for (unit_id, unit) in file.units.iter().enumerate() {
                if unit.language != "rust" {
                    continue;
                }
                for expression in &unit.nodes {
                    let ExpressionKind::Try(try_value) = expression.kind else {
                        continue;
                    };
                    if expression.function == 0 {
                        continue;
                    }
                    let Some((option, arguments)) = method(unit, try_value, "ok_or_else") else {
                        continue;
                    };
                    let Some((receiver, as_object_arguments)) = method(unit, option, "as_object")
                    else {
                        continue;
                    };
                    if arguments.len() != 1 || !as_object_arguments.is_empty() {
                        continue;
                    }
                    let ExpressionKind::Function(callback) = unit.nodes[arguments[0]].kind else {
                        continue;
                    };
                    let function = &unit.functions[expression.function];
                    let owner = Location {
                        path: path.clone(),
                        range: function.range.clone(),
                        name: function.name.clone(),
                    };
                    if !super::super::index::overlaps(&owner, anchors) {
                        continue;
                    }
                    let location = Location {
                        path: path.clone(),
                        range: expression.range.clone(),
                        name: "conditional error propagation".into(),
                    };
                    if !self.has_work(&location) || self.outcomes.len() >= 128 {
                        return;
                    }
                    if !self.allowed(&location) || !function.is_available {
                        continue;
                    }
                    self.resolver().reset_dependency_tracking();
                    if self.resolver().condition_at(&path, &function.range) != Some(true) {
                        continue;
                    }
                    if self
                        .resolver()
                        .source_dependencies()
                        .into_iter()
                        .any(|(path, range)| {
                            !self.allowed(&Location {
                                path,
                                range,
                                name: "model activation evidence".into(),
                            })
                        })
                    {
                        continue;
                    }
                    let binding = input_binding(unit, receiver).map(|id| &unit.bindings[id].range);
                    let facts = self
                        .resolver()
                        .inspect_rust(&path, |source, tree| {
                            rust_types::inspect(
                                source,
                                tree,
                                &unit.imports,
                                binding,
                                &unit.nodes[receiver].range,
                                &function.range,
                                &unit.functions[callback].range,
                            )
                        })
                        .flatten();
                    let Some(facts) = facts else {
                        self.analysis_limit(&location, "conditional outcome model skipped: receiver type or method scope is not proven");
                        continue;
                    };
                    if std::iter::once(&facts.input_range)
                        .chain(facts.import_ranges.iter())
                        .chain(std::iter::once(&unit.functions[callback].range))
                        .any(|range| self.resolver().condition_at(&path, range) != Some(true))
                    {
                        continue;
                    }
                    let Some(mut dependency) = self.serde_dependency(&path, &facts.crate_name)
                    else {
                        self.analysis_limit(&location, "conditional outcome model skipped: registry serde_json 1.x dependency is not proven");
                        continue;
                    };
                    let mut has_shadow = false;
                    for root in dependency.roots {
                        let Some(source) = self.model_metadata(&self.root.join(&root)) else {
                            has_shadow = true;
                            break;
                        };
                        let mut parser = tree_sitter::Parser::new();
                        if parser
                            .set_language(&tree_sitter_rust::LANGUAGE.into())
                            .is_err()
                        {
                            has_shadow = true;
                            break;
                        }
                        let Ok(tree) = crate::parser::parse_source(&mut parser, source.as_bytes())
                        else {
                            has_shadow = true;
                            break;
                        };
                        if rust_types::root_shadows(&source, &tree, &facts.crate_name) {
                            has_shadow = true;
                            break;
                        }
                        dependency.evidence.push(Location {
                            path: root,
                            range: CodeRange {
                                start_line: 1,
                                start_col: 1,
                                end_line: 1,
                                end_col: 2,
                            },
                            name: "crate namespace checked".into(),
                        });
                    }
                    if has_shadow {
                        self.analysis_limit(&location, "conditional outcome model skipped: crate namespace is shadowed or unavailable");
                        continue;
                    }
                    if facts.is_known_object {
                        continue;
                    }
                    let callback_key = FunctionKey {
                        path: path.clone(),
                        unit: unit_id,
                        function: callback,
                    };
                    let Some(callback_location) = self.index.location(&callback_key) else {
                        continue;
                    };
                    if !unit.functions[callback].parameters.is_empty() {
                        continue;
                    }
                    let (value, calls) = self.conditional_factory(callback_key);
                    if matches!(value, ReturnedValue::Unknown) {
                        // An opaque/diverging factory (e.g. `panic!()`) does not
                        // establish a returned error, even though the API and
                        // receiver are known. Keep its passing link and limits.
                        self.analysis_limit(
                            &location,
                            "conditional outcome model skipped: error factory return is not proven",
                        );
                        continue;
                    }
                    let is_identity = facts.has_identity_error
                        && rust_types::has_identity_tuple_error(&facts.result, &value);
                    let returned_value = if matches!(facts.result, rust_types::Type::Result(_)) {
                        ReturnedValue::Variant {
                            name: "Err".into(),
                            value: Box::new(if is_identity {
                                value
                            } else {
                                ReturnedValue::Unknown
                            }),
                        }
                    } else {
                        ReturnedValue::Unknown
                    };
                    let limitation =
                        (!is_identity).then(|| "오류 타입 변환 또는 생성값이 미확정".into());
                    let mut evidence = dependency.evidence;
                    evidence.push(Location {
                        path: path.clone(),
                        range: facts.input_range,
                        name: format!("{}::Value receiver type", facts.crate_name),
                    });
                    evidence.extend(facts.import_ranges.into_iter().map(|range| Location {
                        path: path.clone(),
                        range,
                        name: "type/import identity".into(),
                    }));
                    evidence.push(Location {
                        path: path.clone(),
                        range: unit.nodes[option].range.clone(),
                        name: "serde_json::Value::as_object returns std::option::Option".into(),
                    });
                    evidence.push(Location {
                        path: path.clone(),
                        range: unit.nodes[try_value].range.clone(),
                        name: "Option::ok_or_else creates Err only for None".into(),
                    });
                    evidence.push(callback_location.clone());
                    let outcome = ConditionalOutcome {
                        condition: Condition::NotType {
                            subject: Location {
                                path: path.clone(),
                                range: unit.nodes[receiver].range.clone(),
                                name: match &unit.nodes[receiver].kind {
                                    ExpressionKind::Read { name, .. } => name.clone(),
                                    _ => "input".into(),
                                },
                            },
                            expected: ValueType::JsonObject,
                        },
                        returned_value,
                        is_early_return: true,
                        location: location.clone(),
                        owner,
                        callback: callback_location,
                        evidence,
                        certainty: Evidence::Model,
                        limitation,
                    };
                    if !self
                        .outcomes
                        .iter()
                        .any(|previous| previous.key() == outcome.key())
                    {
                        self.outcomes.push(outcome);
                    }
                    // Preserve useful links into named project error factories,
                    // explicitly conditional on the failure branch.
                    for call in calls {
                        self.step(
                            &call.from,
                            &call.to,
                            "conditional call target",
                            Evidence::Candidate,
                            Some("only on the modeled error branch; no execution observed".into()),
                            true,
                        );
                    }
                }
            }
        }
    }
}
