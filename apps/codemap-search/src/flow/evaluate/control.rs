use super::*;

impl Query<'_> {
    fn equivalent(&self, left: ValueId, right: ValueId) -> bool {
        if left == right {
            return true;
        }
        match (&self.values[left].kind, &self.values[right].kind) {
            (ValueKind::Constant(left), ValueKind::Constant(right)) => left == right,
            (ValueKind::Function(left), ValueKind::Function(right)) => {
                let (left, right) = (&self.closures[*left], &self.closures[*right]);
                left.key == right.key
                    && left.captures == right.captures
                    && left.receiver == right.receiver
            }
            (ValueKind::Object(left), ValueKind::Object(right)) => left == right,
            _ => false,
        }
    }

    fn merge_values(&mut self, left: ValueId, right: ValueId, location: &Location) -> ValueId {
        if self.equivalent(left, right) {
            return left;
        }
        let mut candidates = Vec::new();
        for value in [left, right] {
            let values = match self.values[value].kind.clone() {
                ValueKind::Alternatives(values) => values,
                _ => vec![value],
            };
            for value in values {
                if !candidates
                    .iter()
                    .any(|&other| self.equivalent(other, value))
                {
                    candidates.push(value);
                }
            }
        }
        if candidates
            .iter()
            .all(|&value| matches!(self.values[value].kind, ValueKind::Unknown(_)))
        {
            return self.unknown(location, "conditional values remain unresolved");
        }
        if candidates.len() > CALL_DEPTH {
            self.analysis_limit(location, "conditional value candidate budget exceeded");
            return self.unknown(location, "conditional alternatives exceeded summary budget");
        }
        self.value(
            ValueKind::Alternatives(candidates),
            location.clone(),
            Evidence::Candidate,
        )
    }

    /// Fork only while the condition is unknown. Value/object identities remain
    /// append-only; existing heap cells are restored between paths and differing
    /// mutations are invalidated. No branch's state becomes an unconditional fact.
    pub(super) fn branches(
        &mut self,
        condition: ValueId,
        frame: &mut Frame,
        location: &Location,
        mut execute: impl FnMut(&mut Self, &mut Frame, bool) -> Option<ValueId>,
    ) -> Option<ValueId> {
        if let ValueKind::Constant(Constant::Boolean(is_consequence)) = self.values[condition].kind
        {
            return execute(self, frame, is_consequence);
        }
        if self.conditions.len() >= CALL_DEPTH || !self.tick(location) {
            self.analysis_limit(location, "conditional path depth budget exceeded");
            return Some(self.unknown(location, "conditional path budget exceeded"));
        }
        let original_frame = frame.clone();
        let globals = self.globals.clone();
        let objects = self.objects.clone();
        let writes = self.field_writes.clone();
        let models = (self.should_model_collections, self.should_model_instances);
        let id = self.next_condition;
        self.next_condition += 1;
        self.conditions.push((id, true));
        let left = execute(self, frame, true);
        self.conditions.pop();
        let left_frame = frame.clone();
        let left_globals = self.globals.clone();
        let left_objects = self.objects[..objects.len()].to_vec();
        let left_models = (self.should_model_collections, self.should_model_instances);
        *frame = original_frame;
        self.globals = globals;
        for (i, object) in objects.iter().enumerate() {
            self.objects[i] = object.clone();
        }
        self.field_writes = writes;
        (self.should_model_collections, self.should_model_instances) = models;
        self.conditions.push((id, false));
        let right = execute(self, frame, false);
        self.conditions.pop();

        let keys: BTreeSet<_> = left_frame
            .values
            .keys()
            .chain(frame.values.keys())
            .cloned()
            .collect();
        for key in keys {
            let a = left_frame.values.get(&key).copied().unwrap_or(0);
            let b = frame.values.get(&key).copied().unwrap_or(0);
            let value = self.merge_values(a, b, location);
            frame.values.insert(key, value);
        }
        let keys: BTreeSet<_> = left_globals
            .keys()
            .chain(self.globals.keys())
            .cloned()
            .collect();
        for key in keys {
            let a = left_globals.get(&key).copied().unwrap_or(0);
            let b = self.globals.get(&key).copied().unwrap_or(0);
            let value = self.merge_values(a, b, location);
            self.globals.insert(key, value);
        }
        for (i, left) in left_objects.iter().enumerate() {
            let right = &self.objects[i];
            if left.is_invalidated || right.is_invalidated || left.fields != right.fields {
                self.objects[i].is_invalidated = true;
            }
        }
        self.should_model_collections &= left_models.0;
        self.should_model_instances &= left_models.1;
        match (left, right) {
            (None, None) => None,
            (Some(a), Some(b)) => Some(self.merge_values(a, b, location)),
            (a, b) => Some(self.merge_values(a.unwrap_or(0), b.unwrap_or(0), location)),
        }
    }

    pub(super) fn tuple_item(
        &mut self,
        tuple: ValueId,
        index: usize,
        location: &Location,
    ) -> ValueId {
        match self.values[tuple].kind.clone() {
            ValueKind::Tuple(items) => items
                .get(index)
                .copied()
                .map(|item| self.transfer(item, location, "tuple → element", true))
                .unwrap_or_else(|| self.unknown(location, "tuple index out of range")),
            ValueKind::Alternatives(items) => {
                let start = self.steps.len();
                let mut values = items
                    .into_iter()
                    .map(|item| self.tuple_item(item, index, location))
                    .collect::<Vec<_>>();
                for step in &mut self.steps[start..] {
                    step.evidence = Evidence::Candidate;
                }
                for &value in &values {
                    self.values[value].evidence = Evidence::Candidate;
                }
                let mut values = values.drain(..);
                let mut result = values.next().unwrap_or(0);
                for value in values {
                    result = self.merge_values(result, value, location);
                }
                result
            }
            _ => self.unknown(location, "tuple shape is unresolved"),
        }
    }

    pub(super) fn conditional_call(&mut self, value: ValueId, location: &Location) {
        let values = match self.values[value].kind.clone() {
            ValueKind::Alternatives(values) => values,
            _ => vec![value],
        };
        for value in values {
            if let ValueKind::Function(closure) = self.values[value].kind {
                if let Some(definition) = self.index.location(&self.closures[closure].key) {
                    self.step(
                        &definition,
                        location,
                        "conditional call target",
                        Evidence::Candidate,
                        Some("branch selection unresolved; target body not composed".into()),
                        true,
                    );
                }
            }
        }
    }
}
