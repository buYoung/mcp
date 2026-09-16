//! Native generator suspension is modeled only at explicit next() calls. No
//! framework scheduler, arbitrary iterator or async continuation is executed.
use super::engine::*;
use super::model::*;
use super::syntax::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct GeneratorFrame {
    function: usize,
    env: Environment,
    slots: BTreeMap<Value, Value>,
    statements: Vec<NodeId>,
    index: usize,
    pending: Option<(Option<NodeId>, bool)>,
    delegate: Option<(Value, NodeId)>,
    is_closed: bool,
    steps: usize,
}
pub(super) fn create_generator(
    nested: &mut Interpreter<'_, '_>,
    id: NodeId,
    identity: Value,
) -> Value {
    let function = nested.function.unwrap();
    let definition = &nested.analyzer.program.functions[function];
    if nested.source.nodes[definition.node]
        .tokens
        .contains("async")
    {
        nested.analyzer.notices.insert((
            "async_generator_unsupported".into(),
            definition.identifier.clone(),
        ));
        return Value::unknown();
    }
    let Some(body) = definition.body else {
        return Value::unknown();
    };
    let names: std::collections::BTreeSet<_> = nested
        .source
        .walk(body, false)
        .into_iter()
        .filter(|n| {
            is_identifier(&nested.source.nodes[*n].kind) || nested.source.nodes[*n].kind == "this"
        })
        .map(|n| nested.source.text(Some(n)).to_owned())
        .collect();
    let globals = &nested.analyzer.globals[definition.source];
    let mut captures: Environment = nested
        .env
        .iter()
        .filter(|(name, value)| names.contains(*name) && globals.get(*name) != Some(*value))
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect();
    if captures.len() > 32 {
        nested.analyzer.notices.insert((
            "generator_capture_bound".into(),
            definition.identifier.clone(),
        ));
        return Value::unknown();
    }
    captures.insert("__source_callable_identity__".into(), identity);
    let mut bindings = Value::new("capture_end", "");
    for (name, value) in captures.into_iter().rev() {
        bindings = Value::nested("capture_binding", name, value, Some(bindings));
    }
    let _ = id;
    Value::nested(
        "wrapper",
        "javascript:generator",
        Value::nested("closure", &definition.identifier, bindings, None),
        None,
    )
}
fn result(value: Value, is_done: bool) -> Value {
    Value::nested(
        "wrapper",
        "javascript:iterator_result",
        value,
        Some(Value::new("literal", is_done.to_string())),
    )
}
fn yield_site(source: &Source, statement: NodeId) -> Option<(NodeId, Option<NodeId>, bool)> {
    let mut current = statement;
    if matches!(
        source.nodes[current].kind.as_str(),
        "lexical_declaration" | "variable_declaration"
    ) {
        let declarations: Vec<_> = source.nodes[current]
            .children
            .iter()
            .copied()
            .filter(|n| source.nodes[*n].kind == "variable_declarator")
            .collect();
        if declarations.len() != 1 {
            return None;
        }
        current = declarations[0];
    }
    if source.nodes[current].kind == "variable_declarator" {
        let expression = source.child(current, &["value"])?;
        return (source.nodes[expression].kind == "yield_expression")
            .then(|| (expression, source.child(current, &["name"]), false));
    }
    if matches!(
        source.nodes[current].kind.as_str(),
        "return_statement" | "expression_statement"
    ) {
        let expression = *source.nodes[current].children.first()?;
        return (source.nodes[expression].kind == "yield_expression").then(|| {
            (
                expression,
                None,
                source.nodes[current].kind == "return_statement",
            )
        });
    }
    None
}
impl Interpreter<'_, '_> {
    pub fn generator_call(
        &mut self,
        receiver: &Value,
        method: &str,
        arguments: &[Value],
        id: NodeId,
    ) -> Option<Value> {
        if method != "next"
            || arguments.len() > 1
            || self
                .analyzer
                .heap
                .contains_key(&receiver.clone().field("next"))
        {
            return None;
        }
        let mut generators = receiver.options();
        generators.extend(
            self.analyzer
                .read_values(receiver)
                .iter()
                .flat_map(Value::options),
        );
        generators.retain(|v| v.kind == "wrapper" && v.name == "javascript:generator");
        generators.sort();
        generators.dedup();
        if generators.len() != 1 {
            return None;
        }
        Some(
            self.resume_generator(
                generators.remove(0),
                arguments
                    .first()
                    .cloned()
                    .unwrap_or_else(|| Value::new("literal", "undefined")),
                id,
                0,
            ),
        )
    }
    fn resume_generator(
        &mut self,
        generator: Value,
        mut sent: Value,
        id: NodeId,
        depth: usize,
    ) -> Value {
        if depth >= 6 {
            self.analyzer
                .notices
                .insert(("generator_delegation_bound".into(), self.identifier.clone()));
            return Value::unknown();
        }
        let Some(function) = self.function_index(&generator.base().name) else {
            return Value::unknown();
        };
        let definition = self.analyzer.program.functions[function].clone();
        let source = self.analyzer.program.sources[definition.source].clone();
        let mut frame = self
            .analyzer
            .generator_frames
            .remove(&generator)
            .unwrap_or_else(|| GeneratorFrame {
                function,
                env: closure_captures(generator.base()),
                slots: BTreeMap::new(),
                statements: definition
                    .body
                    .map(|b| source.nodes[b].children.clone())
                    .unwrap_or_default(),
                index: 0,
                pending: None,
                delegate: None,
                is_closed: false,
                steps: 0,
            });
        if frame.is_closed {
            self.analyzer.generator_frames.insert(generator, frame);
            return result(Value::new("literal", "undefined"), true);
        }
        let mut nested = Interpreter::new(self.analyzer, definition.source, Some(frame.function));
        nested.env.extend(frame.env.clone());
        nested.slots = frame.slots.clone();
        nested.conditions.extend(self.conditions.clone());
        nested.conditions.extend([
            "generator_resume_order_required".into(),
            "generator_execution_required".into(),
        ]);
        nested.depth = self.depth + 1;
        nested.instance_context = format!(
            "~generator:{}",
            blake3::hash(generator.display().as_bytes()).to_hex()
        );
        let returned = loop {
            if frame.steps >= 64 {
                nested
                    .analyzer
                    .notices
                    .insert(("generator_step_bound".into(), nested.identifier.clone()));
                frame.is_closed = true;
                break Value::unknown();
            }
            frame.steps += 1;
            if let Some((delegate, site)) = frame.delegate.clone() {
                let delegated = nested.resume_generator(delegate, sent.clone(), site, depth + 1);
                if delegated.kind != "wrapper" || delegated.name != "javascript:iterator_result" {
                    frame.is_closed = true;
                    break Value::unknown();
                }
                if *delegated.key() == Value::new("literal", "false") {
                    break delegated;
                }
                sent = delegated.base().clone();
                frame.delegate = None;
            }
            if let Some((pattern, is_return)) = frame.pending.take() {
                if is_return {
                    frame.is_closed = true;
                    break result(sent, true);
                }
                if pattern.is_some() {
                    nested.bind(pattern, sent.clone(), "");
                }
            }
            if frame.index == frame.statements.len() {
                frame.is_closed = true;
                break result(Value::new("literal", "undefined"), true);
            }
            let statement = frame.statements[frame.index];
            frame.index += 1;
            if let Some((expression, pattern, is_return)) = yield_site(&source, statement) {
                let value =
                    nested.expression(source.nodes[expression].children.last().copied(), 0, true);
                frame.pending = Some((pattern, is_return));
                if source.nodes[expression].tokens.contains("*") {
                    let mut generators = vec![value.clone()];
                    generators.extend(nested.analyzer.read_values(&value));
                    generators.retain(|v| v.kind == "wrapper" && v.name == "javascript:generator");
                    generators.sort();
                    generators.dedup();
                    if generators.len() != 1 {
                        nested.analyzer.notices.insert((
                            "generator_delegate_unresolved".into(),
                            nested.identifier.clone(),
                        ));
                        frame.is_closed = true;
                        break Value::unknown();
                    }
                    frame.delegate = Some((generators.remove(0), expression));
                    sent = Value::new("literal", "undefined");
                    continue;
                }
                break result(value, false);
            }
            if !is_function(&source.nodes[statement].kind)
                && source.walk(statement, true).iter().any(|n| {
                    matches!(
                        source.nodes[*n].kind.as_str(),
                        "yield_expression" | "throw_statement"
                    ) || (source.nodes[*n].kind == "return_statement" && *n != statement)
                })
            {
                nested.analyzer.notices.insert((
                    "generator_control_flow_unsupported".into(),
                    nested.identifier.clone(),
                ));
                frame.is_closed = true;
                break Value::unknown();
            }
            if source.nodes[statement].kind == "return_statement" {
                let value =
                    nested.expression(source.nodes[statement].children.first().copied(), 0, true);
                frame.is_closed = true;
                break result(value, true);
            }
            nested.statement(Some(statement));
        };
        frame.env = nested.env;
        frame.slots = nested.slots;
        let mut facts = nested.facts;
        self.analyzer.generator_frames.insert(generator, frame);
        for fact in &mut facts {
            fact.via.push(self.source.location(id));
            fact.conditions.extend(self.conditions.clone());
            fact.conditions.extend([
                "generator_resume_order_required".into(),
                "generator_execution_required".into(),
            ]);
            fact.conditions.sort();
            fact.conditions.dedup();
        }
        for fact in facts {
            self.append_fact(fact);
        }
        self.conditions.extend([
            "generator_resume_order_required".into(),
            "generator_execution_required".into(),
        ]);
        returned
    }
}
