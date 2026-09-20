//! Language-independent source evidence. A relation is a conditional schema, not
//! a proof of runtime object identity, execution, or event delivery.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct Location {
    pub path: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Value {
    pub kind: String,
    pub name: String,
    pub base: Option<Arc<Value>>,
    pub key: Option<Arc<Value>>,
}

impl Value {
    pub fn new(kind: &str, name: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            base: None,
            key: None,
        }
    }
    pub fn nested(kind: &str, name: impl Into<String>, base: Value, key: Option<Value>) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            base: Some(Arc::new(base)),
            key: key.map(Arc::new),
        }
    }
    pub fn unknown() -> Self {
        Self::new("unknown", "")
    }
    pub fn element() -> Self {
        Self::new("element", "*")
    }
    pub fn entries() -> Self {
        Self::new("container", "entries")
    }
    pub fn keys() -> Self {
        Self::new("container", "keys")
    }
    pub fn base(&self) -> &Self {
        self.base.as_deref().expect("nested source value")
    }
    pub fn key(&self) -> &Self {
        self.key.as_deref().expect("keyed source value")
    }
    pub fn field(self, name: &str) -> Self {
        self.slot(Self::new("key", name))
    }
    pub fn slot(self, key: Self) -> Self {
        if self.kind == "choice" {
            return Self::merge(self.options().into_iter().map(|v| v.slot(key.clone())));
        }
        Self::nested("slot", "", self, Some(key))
    }
    pub fn options(&self) -> Vec<Self> {
        if self.kind == "choice" {
            let mut options = self.base().options();
            options.extend(self.key().options());
            options
        } else {
            vec![self.clone()]
        }
    }
    pub fn merge(values: impl IntoIterator<Item = Self>) -> Self {
        let mut options = Vec::new();
        for value in values {
            for option in value.options() {
                if !options.contains(&option) {
                    options.push(option);
                }
                if options.len() > 8 {
                    return Self::new("unknown", "value_alternative_cap");
                }
            }
        }
        let mut iter = options.into_iter();
        let mut value = iter.next().unwrap_or_else(Self::unknown);
        for option in iter {
            value = Self::nested("choice", "", value, Some(option));
        }
        value
    }
    pub fn tuple(values: &[Self]) -> Self {
        if values.len() > 8 {
            return Self::new("unknown", "tuple_arity_cap");
        }
        let mut result = Self::new("tuple_end", "");
        for value in values.iter().rev() {
            result = Self::nested("tuple", "", value.clone(), Some(result));
        }
        result
    }
    pub fn tuple_values(&self) -> Vec<Self> {
        let mut values = Vec::new();
        let mut value = self;
        while value.kind == "tuple" {
            values.push(value.base().clone());
            value = value.key();
        }
        values
    }
    pub fn split_path(&self) -> (&Self, Vec<&Self>) {
        let mut value = self;
        let mut keys = Vec::new();
        while value.kind == "slot" {
            keys.push(value.key());
            value = value.base();
        }
        keys.reverse();
        (value, keys)
    }
    pub fn substitute(&self, replacements: &BTreeMap<Self, Self>) -> Self {
        if let Some(value) = replacements.get(self) {
            return value.clone();
        }
        if self.kind == "slot" {
            return self
                .base()
                .substitute(replacements)
                .slot(self.key().substitute(replacements));
        }
        if self.kind == "choice" {
            return Self::merge(self.options().iter().map(|v| v.substitute(replacements)));
        }
        if self.kind == "tuple" {
            return Self::tuple(
                &self
                    .tuple_values()
                    .iter()
                    .map(|v| v.substitute(replacements))
                    .collect::<Vec<_>>(),
            );
        }
        if matches!(
            self.kind.as_str(),
            "iterator" | "wrapper" | "bound_method" | "closure" | "object_copy" | "capture_binding"
        ) {
            return Self::nested(
                &self.kind,
                &self.name,
                self.base().substitute(replacements),
                self.key.as_ref().map(|v| v.substitute(replacements)),
            );
        }
        self.clone()
    }
    pub fn referenced(&self) -> Vec<&Self> {
        let mut values = vec![self];
        if let Some(base) = &self.base {
            values.extend(base.referenced());
        }
        if let Some(key) = &self.key {
            values.extend(key.referenced());
        }
        values
    }
    pub fn display(&self) -> String {
        match self.kind.as_str() {
            "slot" => format!("{}[{}]", self.base().display(), self.key().display()),
            "iterator" => format!("iterator:{}({})", self.name, self.base().display()),
            "choice" => format!("choice({}|{})", self.base().display(), self.key().display()),
            "tuple" => format!(
                "tuple({})",
                self.tuple_values()
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            "wrapper" | "bound_method" | "closure" | "object_copy" => format!(
                "{}:{}({}{})",
                self.kind,
                self.name,
                self.base().display(),
                self.key
                    .as_ref()
                    .map(|v| format!(",{}", v.display()))
                    .unwrap_or_default()
            ),
            "capture_binding" => format!(
                "capture:{}={};{}",
                self.name,
                self.base().display(),
                self.key().display()
            ),
            _ => format!("{}:{}", self.kind, self.name),
        }
    }
}
impl Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.display())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub(crate) struct Consumer {
    pub location: Location,
    pub target: Value,
    pub kind: String,
    pub argument_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Fact {
    pub kind: String,
    pub is_method_declaration: bool,
    pub target: Value,
    pub value: Value,
    pub location: Location,
    pub function: String,
    pub conditions: Vec<String>,
    pub via: Vec<Location>,
    pub argument_index: Option<usize>,
    pub consumer: Option<Consumer>,
}
impl Fact {
    pub fn new(
        kind: &str,
        target: Value,
        value: Value,
        location: Location,
        function: &str,
        conditions: Vec<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            is_method_declaration: false,
            target,
            value,
            location,
            function: function.into(),
            conditions,
            via: Vec::new(),
            argument_index: None,
            consumer: None,
        }
    }
    fn identity(&self) -> impl Hash + Eq + '_ {
        (
            &self.kind,
            self.is_method_declaration,
            &self.target,
            &self.value,
            &self.location,
            &self.function,
            &self.conditions,
            &self.argument_index,
            &self.consumer,
        )
    }
}
impl PartialEq for Fact {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}
impl Eq for Fact {}
impl Hash for Fact {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity().hash(state);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Summary {
    pub receiver: Option<Value>,
    pub parameters: Vec<Value>,
    pub facts: Vec<Fact>,
    pub returns: Vec<Value>,
    pub calls: Vec<UnresolvedCall>,
}

/// Preserved operands permit later source resolution; the record alone is never
/// an edge. A partial return does not erase an unresolved call's other effects.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct UnresolvedCall {
    pub result: Value,
    pub callee: Value,
    pub receiver: Value,
    pub arguments: Vec<Value>,
    pub type_arguments: Vec<String>,
    pub candidates: Vec<String>,
    pub return_types: Vec<String>,
    pub type_bindings: BTreeMap<String, String>,
    pub reason: String,
    pub location: Location,
    pub function: String,
    pub conditions: Vec<String>,
    pub via: Vec<Location>,
    pub generic_types: BTreeSet<String>,
    pub return_value: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Relation {
    pub kind: String,
    pub certainty: &'static str,
    pub identity_scope: &'static str,
    pub concrete_instance_proven: bool,
    pub event_classification: &'static str,
    pub storage: Fact,
    pub invocation: Fact,
    pub conditions: Vec<String>,
    pub mutations: Vec<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternative_count: Option<usize>,
}

pub(crate) fn match_storage(stored: &Value, called: &Value) -> Option<Vec<String>> {
    let (left_root, left_keys) = stored.split_path();
    let (right_root, right_keys) = called.split_path();
    if left_root != right_root
        || !matches!(
            left_root.kind.as_str(),
            "receiver" | "allocation" | "global" | "parameter" | "binding"
        )
        || left_keys.is_empty()
        || left_keys.len() > right_keys.len()
    {
        return None;
    }
    let mut conditions: Vec<String> = match left_root.kind.as_str() {
        "receiver" => vec!["same_receiver_required"],
        "allocation" => vec!["same_allocation_instance_required"],
        "global" => vec!["same_runtime_context_required"],
        "parameter" => vec!["same_parameter_binding_required"],
        "binding" => vec![
            "same_lexical_value_binding_required",
            "initializer_value_unresolved",
        ],
        _ => Vec::new(),
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    for (index, (left, right)) in left_keys.iter().zip(&right_keys).enumerate() {
        if left.kind == "opaque_key" || right.kind == "opaque_key" {
            return None;
        }
        if left == right {
            if left.kind == "element" {
                conditions.push("collection_membership_required".into());
            }
            if !matches!(
                left.kind.as_str(),
                "key" | "literal" | "tuple_index" | "element" | "container"
            ) {
                conditions.push(format!(
                    "key_equality_required:{}={}",
                    left.display(),
                    right.display()
                ));
            }
            continue;
        }
        if matches!(left.kind.as_str(), "container" | "tuple_index")
            || matches!(right.kind.as_str(), "container" | "tuple_index")
        {
            return None;
        }
        if left.kind == "element" || right.kind == "element" {
            let other = if left.kind == "element" { right } else { left };
            let is_map_entry = index > 0 && *left_keys[index - 1] == Value::entries();
            if other.kind == "key" && !is_map_entry {
                return None;
            }
            conditions.push("collection_membership_required".into());
            continue;
        }
        if left.kind == right.kind && matches!(left.kind.as_str(), "key" | "literal") {
            return None;
        }
        conditions.push(format!(
            "key_equality_required:{}={}",
            left.display(),
            right.display()
        ));
    }
    Some(conditions)
}

pub(crate) fn connect(facts: &[Fact], max_relations: usize) -> (Vec<Relation>, usize) {
    // Keep declaration bindings available to source method resolution, but do
    // not present an ordinary object method definition as callback registration.
    let stores: Vec<_> = facts
        .iter()
        .filter(|f| f.kind == "store" && !f.is_method_declaration)
        .collect();
    let mutations: Vec<_> = facts.iter().filter(|f| f.kind == "remove").collect();
    let mut calls: Vec<_> = facts
        .iter()
        .filter(|f| {
            matches!(
                f.kind.as_str(),
                "invoke" | "member_invoke" | "argument" | "return" | "key_lookup"
            )
        })
        .collect();
    calls.extend(
        stores
            .iter()
            .copied()
            .filter(|f| f.target.split_path().1.contains(&&Value::entries())),
    );
    fn priority(kind: &str) -> usize {
        match kind {
            "invoke" => 0,
            "member_invoke" => 1,
            "argument" => 2,
            "return" => 3,
            _ => 4,
        }
    }
    calls.sort_by_key(|f| {
        (
            priority(&f.kind),
            &f.location,
            f.argument_index.unwrap_or(0),
        )
    });
    let mut by_root: BTreeMap<&Value, Vec<&Fact>> = BTreeMap::new();
    for store in stores {
        by_root
            .entry(store.target.split_path().0)
            .or_default()
            .push(store);
    }
    let mut buckets: BTreeMap<(&str, &str), Vec<Relation>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut omitted = 0;
    let mut comparisons = 0usize;
    for call in calls {
        let (root, called_keys) = call.target.split_path();
        for store in by_root.get(root).into_iter().flatten().copied() {
            comparisons += 1;
            // This bounds a hostile single-root cross product independently of
            // result caps. The caller surfaces nonzero omissions as incomplete.
            if comparisons > 2_000_000 {
                omitted += 1;
                break;
            }
            if store.consumer.as_ref().is_some_and(|c| {
                c.location != call.location
                    || c.target != call.target
                    || c.kind != call.kind
                    || c.argument_index != call.argument_index
            }) {
                continue;
            }
            if call.kind == "invoke" && matches!(store.value.kind.as_str(), "literal" | "key") {
                continue;
            }
            let stored_keys = store.target.split_path().1;
            let distance = called_keys.len() as isize - stored_keys.len() as isize;
            if call.kind == "store" {
                if !(1..=6).contains(&distance)
                    || store.location == call.location
                    || stored_keys.last().is_none_or(|k| **k == Value::entries())
                    || !stored_keys.contains(&&Value::entries())
                {
                    continue;
                }
            } else if call.kind == "return"
                && distance > 0
                && stored_keys.contains(&&Value::entries())
            {
                if distance > 6 {
                    continue;
                }
            } else if distance != 0 && !(call.kind == "member_invoke" && distance == 1) {
                continue;
            }
            let Some(conditions) = match_storage(&store.target, &call.target) else {
                continue;
            };
            if !seen.insert((
                &store.location,
                &call.location,
                &store.target,
                &store.value,
                &call.target,
                &call.kind,
                call.argument_index,
            )) {
                continue;
            }
            let related_mutations: Vec<_> = mutations
                .iter()
                .filter(|f| match_storage(&f.target, &call.target).is_some())
                .map(|f| f.location.clone())
                .collect();
            let related_stores: BTreeSet<_> = by_root
                .get(root)
                .into_iter()
                .flatten()
                .filter(|f| match_storage(&f.target, &call.target).is_some())
                .map(|f| &f.location)
                .collect();
            let mut obligations: BTreeSet<_> = conditions
                .into_iter()
                .chain(store.conditions.iter().cloned())
                .chain(call.conditions.iter().cloned())
                .collect();
            obligations.insert("registration_and_call_order_unproven".into());
            let mut relation_kind = match call.kind.as_str() {
                "invoke" => "storage_to_invocation",
                "member_invoke" => "stored_object_method_candidate",
                "argument" => {
                    obligations.insert("callee_consumption_unproven".into());
                    "stored_value_argument"
                }
                "return" => {
                    obligations.insert("returned_value_only_not_callback_execution".into());
                    "stored_value_return"
                }
                "store" => {
                    obligations.insert("object_write_only_not_callback_execution".into());
                    "stored_object_write"
                }
                _ => {
                    obligations.insert("lookup_key_only_not_callback_execution".into());
                    "stored_key_lookup"
                }
            };
            if call.kind == "return" && distance > 0 {
                relation_kind = "stored_object_read";
                obligations.insert("object_read_only_not_callback_execution".into());
            }
            if matches!(
                store.value.kind.as_str(),
                "unknown" | "result" | "unresolved" | "binding"
            ) {
                obligations.insert("stored_value_unresolved".into());
            }
            if !related_mutations.is_empty() {
                obligations.insert("removal_may_prevent_call".into());
            }
            if related_stores.len() > 1 {
                obligations.insert("multiple_storage_writes_present".into());
            }
            let category = if matches!(call.kind.as_str(), "invoke" | "member_invoke") {
                "call"
            } else if call.kind == "argument" {
                "argument"
            } else {
                "data"
            };
            let bucket = buckets.entry((category, &call.location.path)).or_default();
            if bucket.len() >= 512.min(max_relations) {
                omitted += 1;
                continue;
            }
            bucket.push(Relation {
                kind: relation_kind.into(),
                certainty: "conditional_source_relation",
                concrete_instance_proven: false,
                event_classification: "not_inferred",
                identity_scope: match root.kind.as_str() {
                    "receiver" => "receiver_schema",
                    "parameter" => "parameter_binding",
                    _ => "allocation_or_global_context",
                },
                storage: store.clone(),
                invocation: call.clone(),
                conditions: obligations.into_iter().collect(),
                mutations: related_mutations,
                alternative_count: (call.kind == "argument").then_some(1),
            });
        }
        if comparisons > 2_000_000 {
            break;
        }
    }
    let total = buckets.values().map(Vec::len).sum::<usize>();
    let mut result = Vec::new();
    let mut offset = 0;
    while result.len() < max_relations {
        let mut progressed = false;
        for bucket in buckets.values() {
            if let Some(relation) = bucket.get(offset) {
                result.push(relation.clone());
                progressed = true;
                if result.len() == max_relations {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
        offset += 1;
    }
    omitted += total - result.len();
    (result, omitted)
}
