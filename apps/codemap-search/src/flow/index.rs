use super::*;
use crate::parser::ExtractedFile;
use std::collections::{BTreeMap, HashMap};
#[cfg(test)]
use std::ops::Deref;
#[cfg(test)]
use std::path::Path;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct FunctionKey {
    pub path: String,
    pub unit: usize,
    pub function: usize,
}
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct BindingKey {
    pub path: String,
    pub unit: usize,
    pub binding: usize,
}
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Location {
    pub path: String,
    pub range: crate::parser::CodeRange,
    pub name: String,
}

#[derive(Clone)]
pub(crate) struct IndexedFlowStore {
    #[cfg(test)]
    pub searcher: tantivy::Searcher,
    #[cfg(test)]
    pub field: tantivy::schema::Field,
    pub documents: BTreeMap<String, (tantivy::DocAddress, String)>,
}
#[derive(Clone)]
enum Store {
    Memory(BTreeMap<String, Arc<FlowFile>>),
    Indexed(IndexedFlowStore),
}
#[cfg(test)]
struct FileData {
    file: Arc<FlowFile>,
    users: HashMap<BindingKey, Vec<FunctionKey>>,
    cost: usize,
}
#[cfg(test)]
#[derive(Default)]
struct Cache {
    entries: HashMap<String, (Arc<FileData>, u64)>,
    clock: u64,
    nodes: usize,
}
#[cfg(test)]
pub(super) struct UnitRef {
    file: Arc<FileData>,
    unit: usize,
}
#[cfg(test)]
impl Deref for UnitRef {
    type Target = FlowUnit;
    fn deref(&self) -> &Self::Target {
        &self.file.file.units[self.unit]
    }
}

/// Navigation uses the stored source digests for freshness checks. Decoding
/// summaries and their query cache are retained only for internal evaluator tests.
#[derive(Clone)]
pub(crate) struct FlowIndex {
    store: Store,
    #[cfg(test)]
    cache: Arc<Mutex<Cache>>,
    #[cfg(test)]
    pub(super) codemap: Arc<Vec<ExtractedFile>>,
    #[cfg(test)]
    pub(super) sources: Arc<HashMap<String, String>>,
    #[cfg(test)]
    pub(super) target_os: Option<String>,
}
impl std::fmt::Debug for FlowIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlowIndex")
            .field("files", &self.paths().len())
            .finish()
    }
}

impl FlowIndex {
    pub(crate) fn build(
        codemap: Arc<Vec<ExtractedFile>>,
        inputs: BTreeMap<String, FlowFile>,
        sources: Arc<HashMap<String, String>>,
    ) -> Self {
        Self::new(
            codemap,
            Store::Memory(
                inputs
                    .into_iter()
                    .map(|(path, file)| (path, Arc::new(file)))
                    .collect(),
            ),
            sources,
        )
    }
    pub(crate) fn indexed(
        codemap: Arc<Vec<ExtractedFile>>,
        store: IndexedFlowStore,
        sources: Arc<HashMap<String, String>>,
    ) -> Self {
        Self::new(codemap, Store::Indexed(store), sources)
    }
    fn new(
        codemap: Arc<Vec<ExtractedFile>>,
        store: Store,
        sources: Arc<HashMap<String, String>>,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (codemap, sources);
        Self {
            store,
            #[cfg(test)]
            cache: Arc::new(Mutex::new(Cache::default())),
            #[cfg(test)]
            codemap,
            #[cfg(test)]
            sources,
            #[cfg(test)]
            target_os: crate::config::get().analysis_target_os.clone(),
        }
    }
    pub(super) fn paths(&self) -> Vec<&str> {
        match &self.store {
            Store::Memory(files) => files.keys().map(String::as_str).collect(),
            Store::Indexed(store) => store.documents.keys().map(String::as_str).collect(),
        }
    }
    #[cfg(test)]
    pub(super) fn has_file(&self, path: &str) -> bool {
        match &self.store {
            Store::Memory(files) => files.contains_key(path),
            Store::Indexed(store) => store.documents.contains_key(path),
        }
    }
    pub(crate) fn digest(&self, path: &str) -> Option<&str> {
        match &self.store {
            Store::Memory(files) => files.get(path).map(|file| file.digest.as_str()),
            Store::Indexed(store) => store.documents.get(path).map(|(_, digest)| digest.as_str()),
        }
    }
    #[cfg(test)]
    fn data(&self, path: &str) -> Option<Arc<FileData>> {
        {
            let mut cache = self.cache.lock().ok()?;
            cache.clock += 1;
            let clock = cache.clock;
            if let Some((file, used)) = cache.entries.get_mut(path) {
                *used = clock;
                return Some(Arc::clone(file));
            }
        }
        let file = match &self.store {
            Store::Memory(files) => Arc::clone(files.get(path)?),
            Store::Indexed(store) => {
                use tantivy::schema::Value;
                #[derive(serde::Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Stored {
                    flow_file: Option<FlowFile>,
                }
                let (address, _) = store.documents.get(path)?;
                let document = store
                    .searcher
                    .doc::<tantivy::TantivyDocument>(*address)
                    .ok()?;
                let json = document.get_first(store.field)?.as_str()?;
                Arc::new(serde_json::from_str::<Stored>(json).ok()?.flow_file?)
            }
        };
        let mut users: HashMap<BindingKey, Vec<FunctionKey>> = HashMap::new();
        let mut cost = 0;
        for (unit_id, unit) in file.units.iter().enumerate() {
            cost += unit.nodes.len() + unit.bindings.len();
            for node in &unit.nodes {
                if let ExpressionKind::Read {
                    binding: Some(binding),
                    ..
                } = node.kind
                {
                    let entry = users
                        .entry(BindingKey {
                            path: path.into(),
                            unit: unit_id,
                            binding,
                        })
                        .or_default();
                    let function = FunctionKey {
                        path: path.into(),
                        unit: unit_id,
                        function: node.function,
                    };
                    if !entry.contains(&function) && entry.len() < 64 {
                        entry.push(function);
                    }
                }
            }
        }
        let file = Arc::new(FileData { file, users, cost });
        let mut cache = self.cache.lock().ok()?;
        // A concurrent reader may have populated this entry while we decoded it.
        if let Some((existing, _)) = cache.entries.get(path) {
            return Some(Arc::clone(existing));
        }
        while cache.entries.len() >= 32 || cache.nodes + cost > 32_768 {
            let Some(oldest) = cache
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            if let Some((file, _)) = cache.entries.remove(&oldest) {
                cache.nodes = cache.nodes.saturating_sub(file.cost);
            }
        }
        cache.clock += 1;
        let clock = cache.clock;
        if cost <= 32_768 {
            cache.nodes += cost;
            cache
                .entries
                .insert(path.into(), (Arc::clone(&file), clock));
        }
        Some(file)
    }
    #[cfg(test)]
    pub(super) fn file(&self, path: &str) -> Option<Arc<FlowFile>> {
        self.data(path).map(|data| Arc::clone(&data.file))
    }
    #[cfg(test)]
    pub(super) fn unit(&self, path: &str, unit: usize) -> Option<UnitRef> {
        let file = self.data(path)?;
        (unit < file.file.units.len()).then_some(UnitRef { file, unit })
    }
    #[cfg(test)]
    pub(super) fn users(&self, key: &BindingKey) -> Vec<FunctionKey> {
        self.data(&key.path)
            .and_then(|data| data.users.get(key).cloned())
            .unwrap_or_default()
    }
    #[cfg(test)]
    pub(super) fn function(&self, key: &FunctionKey) -> Option<FunctionSummary> {
        self.unit(&key.path, key.unit)?
            .functions
            .get(key.function)
            .cloned()
    }
    #[cfg(test)]
    pub(super) fn location(&self, key: &FunctionKey) -> Option<Location> {
        let unit = self.unit(&key.path, key.unit)?;
        let function = unit.functions.get(key.function)?;
        Some(Location {
            path: key.path.clone(),
            range: function.range.clone(),
            name: function.name.clone(),
        })
    }
    #[cfg(test)]
    pub(crate) fn for_paths_with_debug(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
        should_list_unresolved: bool,
        should_debug: bool,
    ) -> String {
        if cap < 256
            || anchors.is_empty()
            || self.target_os != crate::config::get().analysis_target_os
        {
            return String::new();
        }
        let mut query = super::evaluate::Query::new(self, root, scope);
        query.should_list_unresolved = should_list_unresolved;
        query.run(anchors);
        super::render::render_with_context(&query, cap, None, None, None, should_debug)
    }
}

#[cfg(test)]
pub(super) fn overlaps(location: &Location, anchors: &[(String, usize, usize)]) -> bool {
    anchors.iter().any(|(path, start, end)| {
        path == &location.path
            && location.range.start_line <= *end
            && *start <= location.range.end_line_inclusive()
    })
}
#[cfg(test)]
pub(super) fn contains(outer: &crate::parser::CodeRange, inner: &crate::parser::CodeRange) -> bool {
    (outer.start_line, outer.start_col) <= (inner.start_line, inner.start_col)
        && (inner.end_line, inner.end_col) <= (outer.end_line, outer.end_col)
}
