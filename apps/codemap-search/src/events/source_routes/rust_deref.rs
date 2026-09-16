use super::engine::Interpreter;
use super::model::*;
use super::syntax::NodeId;
use std::collections::BTreeMap;

impl Interpreter<'_, '_> {
    pub fn dereferenced_receiver(&mut self, receiver: Value, method: &str, node: NodeId) -> Value {
        let mutable = matches!(
            method,
            "push"
                | "push_back"
                | "insert"
                | "clear"
                | "remove"
                | "retain"
                | "pop"
                | "pop_front"
                | "iter_mut"
                | "get_mut"
                | "index_mut"
        );
        if !mutable && !matches!(method, "get" | "iter" | "len" | "is_empty" | "index") {
            return receiver;
        }
        let owner = self.analyzer.owner(&receiver, self.source_index);
        if owner.is_empty()
            || self
                .analyzer
                .program
                .methods
                .contains_key(&(owner.clone(), method.into()))
        {
            return receiver;
        }
        let mode = if mutable { "mutable" } else { "shared" };
        let Some(candidates) = self
            .analyzer
            .program
            .dereferences
            .get(&(owner, mode.into()))
            .filter(|v| v.len() == 1)
            .cloned()
        else {
            return receiver;
        };
        let Some(summary) = self
            .analyzer
            .summaries
            .get(&candidates[0])
            .filter(|s| s.returns.len() == 1)
            .cloned()
        else {
            return receiver;
        };
        let Some(original) = summary.receiver else {
            return receiver;
        };
        let target = summary.returns[0].substitute(&BTreeMap::from([(original, receiver.clone())]));
        if target.kind != "slot" || target.split_path().0 != receiver.split_path().0 {
            return receiver;
        }
        let source = self.analyzer.program.functions[candidates[0]].source;
        if self.analyzer.kind(&target, source).is_empty()
            || !self.analyzer.owner(&target, source).is_empty()
        {
            return receiver;
        }
        let projected = self.apply_summary(candidates[0], receiver.clone(), Vec::new(), node);
        if projected != target {
            return receiver;
        }
        self.conditions.extend([
            "source_deref_body_applied".into(),
            "dereference_execution_unproven".into(),
        ]);
        self.emit(
            "source_dereference",
            target.clone(),
            receiver,
            node,
            &[],
            None,
        );
        target
    }
}
