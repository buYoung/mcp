mod execute;
mod resolve;
use super::index::{BindingKey, FunctionKey, Location};
use super::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

type ValueId = usize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Evidence {
    Source,
    Model,
    Candidate,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_elapsed_query_stops_with_a_diagnostic() {
        let index = FlowIndex::build(
            std::sync::Arc::new(Vec::new()),
            BTreeMap::new(),
            std::sync::Arc::new(HashMap::new()),
        );
        let root = tempfile::tempdir().unwrap();
        let mut query = Query::new(&index, root.path(), None);
        query.deadline = Instant::now() - Duration::from_millis(1);
        let location = Location {
            path: "slow.ts".into(),
            range: crate::parser::CodeRange {
                start_line: 7,
                start_col: 1,
                end_line: 7,
                end_col: 2,
            },
            name: "slow".into(),
        };
        assert!(!query.tick(&location));
        let output = super::super::render::render(&query, 1024);
        assert!(
            output.contains("time budget reached")
                && output.contains("slow.ts")
                && output.contains("read"),
            "{output}"
        );
    }
}
#[derive(Clone, Debug)]
pub(super) struct Step {
    pub from: Location,
    pub to: Location,
    pub relation: String,
    pub evidence: Evidence,
    pub detail: Option<String>,
    pub is_interesting: bool,
}
#[derive(Clone, Debug)]
pub(super) struct Diagnostic {
    pub location: Location,
    pub reason: String,
}
#[derive(Clone)]
struct Value {
    kind: ValueKind,
    location: Location,
    evidence: Evidence,
}
#[derive(Clone)]
enum ValueKind {
    Constant(Constant),
    Function(usize),
    Class {
        path: String,
        unit: usize,
        class: usize,
    },
    Object(usize),
    Parameter,
    Unknown(String),
    Namespace {
        path: String,
        unit: usize,
    },
    MapConstructor,
    MapPrototype,
    MapMethod {
        object: usize,
        method: String,
    },
    DeferredRead(usize),
}
#[derive(Clone)]
struct Closure {
    key: FunctionKey,
    captures: BTreeMap<BindingKey, ValueId>,
    receiver: Option<ValueId>,
}
#[derive(Clone)]
struct Frame {
    key: FunctionKey,
    values: BTreeMap<BindingKey, ValueId>,
    receiver: Option<ValueId>,
}
struct Object {
    location: Location,
    fields: BTreeMap<Constant, ValueId>,
    class: Option<(String, usize, usize)>,
    is_map: bool,
    is_invalidated: bool,
}
struct Store {
    object: usize,
    key: Option<Constant>,
    value: ValueId,
    location: Location,
}
struct Read {
    object: usize,
    key: Option<Constant>,
    location: Location,
}
struct CallbackUse {
    read: usize,
    location: Location,
}

pub(super) struct Query<'a> {
    index: &'a FlowIndex,
    root: &'a Path,
    scope: Option<&'a str>,
    resolver: std::cell::OnceCell<crate::callers::resolution::SourceResolver<'a>>,
    filter: crate::callers::test_code::TestCodeFilter,
    deadline: Instant,
    operations: usize,
    files: HashMap<String, bool>,
    source_bytes: usize,
    values: Vec<Value>,
    closures: Vec<Closure>,
    objects: Vec<Object>,
    root_context: usize,
    field_writes: HashMap<(usize, Constant), usize>,
    should_model_collections: bool,
    globals: HashMap<BindingKey, ValueId>,
    loading: HashSet<BindingKey>,
    touched: BTreeSet<BindingKey>,
    executed: BTreeSet<FunctionKey>,
    stack: Vec<FunctionKey>,
    stores: Vec<Store>,
    reads: Vec<Read>,
    callback_uses: Vec<CallbackUse>,
    pub steps: Vec<Step>,
    pub diagnostics: Vec<Diagnostic>,
    pub is_relevant: bool,
    pub should_list_unresolved: bool,
    pub anchors: Vec<(String, usize, usize)>,
}

impl<'a> Query<'a> {
    pub(super) fn new(index: &'a FlowIndex, root: &'a Path, scope: Option<&'a str>) -> Self {
        let sentinel = Location {
            path: String::new(),
            range: crate::parser::CodeRange {
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            },
            name: "unresolved".into(),
        };
        Self {
            index,
            root,
            scope,
            resolver: std::cell::OnceCell::new(),
            filter: crate::callers::test_code::TestCodeFilter::from_config(root),
            deadline: Instant::now() + Duration::from_millis(QUERY_TIME_MS),
            operations: 0,
            files: HashMap::new(),
            source_bytes: 0,
            values: vec![Value {
                kind: ValueKind::Unknown("analysis budget exhausted".into()),
                location: sentinel,
                evidence: Evidence::Candidate,
            }],
            closures: Vec::new(),
            objects: Vec::new(),
            root_context: 0,
            field_writes: HashMap::new(),
            should_model_collections: true,
            globals: HashMap::new(),
            loading: HashSet::new(),
            touched: BTreeSet::new(),
            executed: BTreeSet::new(),
            stack: Vec::new(),
            stores: Vec::new(),
            reads: Vec::new(),
            callback_uses: Vec::new(),
            steps: Vec::new(),
            diagnostics: Vec::new(),
            is_relevant: false,
            should_list_unresolved: true,
            anchors: Vec::new(),
        }
    }
    pub(super) fn run(&mut self, anchors: &[(String, usize, usize)]) {
        self.anchors = anchors.iter().take(FILES_PER_QUERY).cloned().collect();
        let mut roots = BTreeSet::new();
        for (path, start, end) in anchors.iter().take(FILES_PER_QUERY) {
            let location = Location {
                path: path.clone(),
                range: crate::parser::CodeRange {
                    start_line: *start,
                    start_col: 1,
                    end_line: *end,
                    end_col: 2,
                },
                name: "flow input".into(),
            };
            if !self.index.has_file(path) || !self.allowed(&location) {
                continue;
            }
            let Some(file) = self.index.file(path) else {
                self.diagnostic(&location, "stored function summary could not be loaded");
                continue;
            };
            for issue in file
                .omissions
                .iter()
                .filter(|issue| {
                    issue.range.start_line <= *end && *start <= issue.range.end_line_inclusive()
                })
                .take(4)
            {
                self.diagnostic(
                    &Location {
                        range: issue.range.clone(),
                        ..location.clone()
                    },
                    &issue.reason,
                );
            }
            for (unit, summary) in file.units.iter().enumerate() {
                let mut found = false;
                for (function, procedure) in summary.functions.iter().enumerate().skip(1) {
                    if procedure.range.start_line <= *end
                        && *start <= procedure.range.end_line_inclusive()
                    {
                        // Reading an enclosing function must not execute every nested
                        // closure/method merely because its declaration shares the window.
                        if summary
                            .functions
                            .iter()
                            .enumerate()
                            .skip(1)
                            .any(|(other_id, other)| {
                                other_id != function
                                    && other.range != procedure.range
                                    && super::index::contains(&other.range, &procedure.range)
                                    && other.range.start_line <= *end
                                    && *start <= other.range.end_line_inclusive()
                            })
                        {
                            continue;
                        }
                        found = true;
                        let key = FunctionKey {
                            path: path.clone(),
                            unit,
                            function,
                        };
                        let callers = self.callers(&key);
                        if callers.is_empty() {
                            roots.insert(key);
                        } else {
                            roots.extend(callers);
                        }
                    }
                }
                if !found {
                    roots.insert(FunctionKey {
                        path: path.clone(),
                        unit,
                        function: 0,
                    });
                }
            }
        }
        for key in roots.into_iter().take(16) {
            self.analyze_root(key);
            if self.operations > NODES_PER_QUERY || Instant::now() > self.deadline {
                break;
            }
        }
        // Expand only procedures sharing a proven lexical storage/receiver binding.
        // Runtime object equality and collection keys are checked again when joining.
        let mut peers = BTreeSet::new();
        for binding in self.touched.iter().take(32) {
            let users = self.index.users(binding);
            for user in users.iter().filter(|key| key.function != 0).take(16) {
                let callers = self.callers(user);
                if callers.is_empty() {
                    peers.insert(user.clone());
                } else {
                    peers.extend(callers);
                }
            }
        }
        for key in peers.into_iter().take(16) {
            if self.operations > NODES_PER_QUERY || Instant::now() > self.deadline {
                break;
            }
            if !self.executed.contains(&key) {
                self.analyze_root(key);
            }
        }
        self.join_collections();
        self.is_relevant = self.steps.iter().any(|step| step.is_interesting)
            && self.steps.iter().any(|step| {
                super::index::overlaps(&step.from, anchors)
                    || super::index::overlaps(&step.to, anchors)
            });
    }
    fn analyze_root(&mut self, key: FunctionKey) {
        self.root_context += 1;
        if self
            .index
            .function(&key)
            .is_some_and(|function| function.is_available && function.body.is_empty())
        {
            return;
        }
        let Some(location) = self.index.location(&key) else {
            return;
        };
        if !self.allowed(&location) {
            return;
        }
        let parameters = self.index.function(&key).unwrap().parameters.clone();
        let mut arguments = Vec::new();
        for parameter in parameters {
            let binding = &self.index.unit(&key.path, key.unit).unwrap().bindings[parameter];
            let location = Location {
                path: key.path.clone(),
                range: binding.range.clone(),
                name: binding.name.clone(),
            };
            arguments.push(self.value(ValueKind::Parameter, location, Evidence::Source));
        }
        let closure = self.closure(key, BTreeMap::new(), None);
        self.invoke(closure, arguments, None, &location);
    }
    fn callers(&self, target: &FunctionKey) -> Vec<FunctionKey> {
        let Some(unit) = self.index.unit(&target.path, target.unit) else {
            return Vec::new();
        };
        let Some(binding)=unit.bindings.iter().position(|binding|matches!(binding.kind,BindingKind::Function(function) if function==target.function)) else{return Vec::new()};
        self.index
            .users(&BindingKey {
                path: target.path.clone(),
                unit: target.unit,
                binding,
            })
            .into_iter()
            .filter(|user| user != target)
            .filter(|user| {
                self.index.location(user).is_some_and(|location| {
                    !self.filter.is_file_excluded(&location.path)
                        && !self.filter.is_excluded(&location.path, &location.range)
                })
            })
            .take(8)
            .collect()
    }
    fn resolver(&self) -> &crate::callers::resolution::SourceResolver<'a> {
        self.resolver.get_or_init(|| {
            crate::callers::resolution::SourceResolver::from_stored_sources(
                &self.index.codemap,
                self.root,
                &self.index.sources,
            )
        })
    }
    fn tick(&mut self, location: &Location) -> bool {
        self.operations += 1;
        if self.operations > NODES_PER_QUERY || Instant::now() > self.deadline {
            self.diagnostic(
                location,
                if self.operations > NODES_PER_QUERY {
                    "value-flow node budget reached"
                } else {
                    "value-flow time budget reached"
                },
            );
            false
        } else {
            true
        }
    }
    fn diagnostic(&mut self, location: &Location, reason: &str) {
        if self.diagnostics.len() < 16
            && !self.diagnostics.iter().any(|item| {
                item.location.path == location.path
                    && item.location.range == location.range
                    && item.reason == reason
            })
        {
            self.diagnostics.push(Diagnostic {
                location: location.clone(),
                reason: reason.into(),
            });
        }
    }
    fn value(&mut self, kind: ValueKind, location: Location, evidence: Evidence) -> ValueId {
        if self.values.len() >= NODES_PER_QUERY {
            self.diagnostic(&location, "value-flow value budget reached");
            return 0;
        }
        let id = self.values.len();
        self.values.push(Value {
            kind,
            location,
            evidence,
        });
        id
    }
    fn unknown(&mut self, location: &Location, reason: &str) -> ValueId {
        self.value(
            ValueKind::Unknown(reason.into()),
            location.clone(),
            Evidence::Candidate,
        )
    }
    fn closure(
        &mut self,
        key: FunctionKey,
        captures: BTreeMap<BindingKey, ValueId>,
        receiver: Option<ValueId>,
    ) -> usize {
        let mut bounded = BTreeMap::new();
        if captures.len() > 64 {
            if let Some(location) = self.index.location(&key) {
                self.diagnostic(&location, "closure capture budget reached");
            }
        }
        for (binding, mut value) in captures.into_iter().take(64) {
            let is_mutated = self
                .index
                .unit(&binding.path, binding.unit)
                .is_some_and(|unit| unit.bindings[binding.binding].is_mutated);
            if is_mutated {
                if let Some(location) = self.index.location(&key) {
                    value = self.unknown(
                        &location,
                        "mutable closure capture has no proven value version",
                    );
                    if self.index.users(&binding).contains(&key) {
                        self.diagnostic(&location, "mutable closure capture is unresolved");
                    }
                }
            }
            bounded.insert(binding, value);
        }
        let id = self.closures.len();
        self.closures.push(Closure {
            key,
            captures: bounded,
            receiver,
        });
        id
    }
    fn step(
        &mut self,
        from: &Location,
        to: &Location,
        relation: &str,
        evidence: Evidence,
        detail: Option<String>,
        is_interesting: bool,
    ) {
        if from.path.is_empty() || to.path.is_empty() || self.steps.len() >= NODES_PER_QUERY {
            return;
        }
        self.steps.push(Step {
            from: from.clone(),
            to: to.clone(),
            relation: relation.into(),
            evidence,
            detail,
            is_interesting,
        });
    }
    fn transfer(
        &mut self,
        value: ValueId,
        to: &Location,
        relation: &str,
        interesting: bool,
    ) -> ValueId {
        let value = self.values[value].clone();
        let interesting = (interesting
            || (matches!(
                value.kind,
                ValueKind::Object(_) | ValueKind::Function(_) | ValueKind::DeferredRead(_)
            ) && matches!(relation, "value → binding" | "initializer → binding")))
            && !matches!(value.kind, ValueKind::Unknown(_));
        self.step(
            &value.location,
            to,
            relation,
            value.evidence,
            None,
            interesting,
        );
        self.value(value.kind, to.clone(), value.evidence)
    }
    fn join_collections(&mut self) {
        if !self.should_model_collections {
            return;
        }
        for read_id in 0..self.reads.len() {
            let read = &self.reads[read_id];
            let matching = self
                .stores
                .iter()
                .filter(|store| {
                    store.object == read.object && store.key.is_some() && store.key == read.key
                })
                .take(16)
                .map(|store| (store.location.clone(), store.value))
                .collect::<Vec<_>>();
            let location = read.location.clone();
            if read.key.is_none() {
                self.diagnostic(
                    &location,
                    "collection lookup key unresolved; stored callbacks are not attributed",
                );
            }
            for (stored, value) in matching {
                let source = self.values[value].clone();
                self.step(&stored,&location,"collection store → lookup",Evidence::Candidate,Some("same source-proven storage and literal key; ordering/overwrites are not proven".into()),true);
                let uses = self
                    .callback_uses
                    .iter()
                    .filter(|use_| use_.read == read_id)
                    .map(|use_| use_.location.clone())
                    .collect::<Vec<_>>();
                for use_ in uses {
                    if let ValueKind::Function(closure) = source.kind {
                        if let Some(definition) = self.index.location(&self.closures[closure].key) {
                            self.step(
                                &definition,
                                &use_,
                                "possible callback invocation",
                                Evidence::Candidate,
                                Some(
                                    "collection contents and execution timing remain unresolved"
                                        .into(),
                                ),
                                true,
                            );
                        }
                    }
                }
            }
        }
    }
}
