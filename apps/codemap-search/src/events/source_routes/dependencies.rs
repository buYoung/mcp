//! Conservative proof dependencies. Imports and visible type candidates are
//! retained even when they contribute no displayed endpoint.
use super::model::{Fact, Relation};
use super::syntax::Program;
use std::collections::{BTreeMap, BTreeSet, HashMap};

impl Program {
    pub fn dependency_graph(&self) -> (BTreeMap<String, BTreeSet<String>>, BTreeSet<String>) {
        let mut graph = BTreeMap::new();
        let mut incomplete = BTreeSet::new();
        let paths: BTreeSet<_> = self.sources.iter().map(|s| s.path.as_str()).collect();
        // Source groups looked up per file, instead of rescanning every source for each one.
        let swift_paths: Vec<&String> = self
            .sources
            .iter()
            .filter(|source| source.language == "swift")
            .map(|source| &source.path)
            .collect();
        let mut paths_by_namespace: HashMap<&str, Vec<&String>> = HashMap::new();
        let mut go_paths_by_namespace: HashMap<String, Vec<&String>> = HashMap::new();
        for (index, source) in self.sources.iter().enumerate() {
            if let Some(namespace) = self.namespaces.get(&source.path) {
                paths_by_namespace
                    .entry(namespace.as_str())
                    .or_default()
                    .push(&source.path);
            }
            if source.language == "go" {
                go_paths_by_namespace
                    .entry(self.type_namespace(index))
                    .or_default()
                    .push(&source.path);
            }
        }
        for source in &self.sources {
            let mut selected = BTreeSet::from([source.path.clone()]);
            let namespace = self
                .namespaces
                .get(&source.path)
                .cloned()
                .unwrap_or_default();
            if matches!(source.language.as_str(), "typescript" | "javascript")
                && !source.nodes.is_empty()
            {
                for &id in &source.nodes[0].children {
                    if matches!(
                        source.nodes[id].kind.as_str(),
                        "import_statement" | "export_statement"
                    ) {
                        let specifier = source
                            .text(source.child(id, &["source"]))
                            .trim_matches(['\'', '"']);
                        if !specifier.is_empty() {
                            if let Some(path) = self.resolve_relative(&source.path, specifier) {
                                selected.insert(path);
                            }
                        }
                    }
                }
            }
            if source.language == "swift" {
                selected.extend(swift_paths.iter().map(|path| (*path).clone()));
            }
            if !namespace.is_empty() {
                selected.extend(
                    paths_by_namespace
                        .get(namespace.as_str())
                        .into_iter()
                        .flatten()
                        .map(|path| (*path).clone()),
                );
            }
            for (_, import) in self
                .imports
                .range((source.path.clone(), String::new())..)
                .take_while(|((path, _), _)| path == &source.path)
            {
                let path = import.split('#').next().unwrap_or(import);
                if paths.contains(path) {
                    selected.insert(path.into());
                }
                selected.extend(
                    go_paths_by_namespace
                        .get(path)
                        .into_iter()
                        .flatten()
                        .map(|path| (*path).clone()),
                );
                for owner in self.type_index.qualified(import) {
                    if let Some(index) = self.owner_sources.get(owner) {
                        selected.insert(self.sources[*index].path.clone());
                    }
                }
            }
            for prefix in self
                .namespace_imports
                .get(&source.path)
                .into_iter()
                .flatten()
            {
                for owner in self.type_index.under_prefix(prefix) {
                    if let Some(index) = self.owner_sources.get(owner) {
                        selected.insert(self.sources[*index].path.clone());
                    }
                }
            }
            if selected.len() > super::super::SOURCE_FILES_PER_QUERY {
                incomplete.insert(source.path.clone());
                selected = selected
                    .into_iter()
                    .take(super::super::SOURCE_FILES_PER_QUERY)
                    .collect();
            }
            graph.insert(source.path.clone(), selected);
        }
        (graph, incomplete)
    }
    fn fact_sources(&self, fact: &Fact, selected: &mut BTreeSet<String>) {
        selected.insert(fact.location.path.clone());
        selected.extend(fact.via.iter().map(|l| l.path.clone()));
        for value in fact
            .target
            .referenced()
            .into_iter()
            .chain(fact.value.referenced())
        {
            if let Some(index) = self.owner_sources.get(&value.name) {
                selected.insert(self.sources[*index].path.clone());
            }
            if let Some((path, _)) = value.name.split_once(':') {
                if self.type_index.source_indexes.contains_key(path) {
                    selected.insert(path.into());
                }
            }
        }
    }
    pub fn relation_sources(&self, relation: &Relation) -> BTreeSet<String> {
        let mut selected = BTreeSet::new();
        self.fact_sources(&relation.storage, &mut selected);
        self.fact_sources(&relation.invocation, &mut selected);
        selected.extend(relation.mutations.iter().map(|l| l.path.clone()));
        selected
    }
}
