//! Conservative proof dependencies. Imports and visible type candidates are
//! retained even when they contribute no displayed endpoint.
use super::model::{Fact, Relation};
use super::syntax::Program;
use std::collections::{BTreeMap, BTreeSet};

impl Program {
    pub fn dependency_graph(&self) -> (BTreeMap<String, BTreeSet<String>>, BTreeSet<String>) {
        let mut graph = BTreeMap::new();
        let mut incomplete = BTreeSet::new();
        let paths: BTreeSet<_> = self.sources.iter().map(|s| s.path.as_str()).collect();
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
            for candidate in &self.sources {
                if source.language == "swift" && candidate.language == "swift"
                    || !namespace.is_empty()
                        && self.namespaces.get(&candidate.path) == Some(&namespace)
                {
                    selected.insert(candidate.path.clone());
                }
            }
            for ((path, _), import) in &self.imports {
                if path != &source.path {
                    continue;
                }
                let path = import.split('#').next().unwrap_or(import);
                if paths.contains(path) {
                    selected.insert(path.into());
                }
                for candidate in &self.sources {
                    if candidate.language == "go"
                        && self.type_namespace(
                            self.sources
                                .iter()
                                .position(|s| s.path == candidate.path)
                                .unwrap(),
                        ) == path
                    {
                        selected.insert(candidate.path.clone());
                    }
                }
                for ((path, name), owner) in &self.types {
                    let qualified = format!(
                        "{}.{}",
                        self.namespaces
                            .get(path)
                            .map(String::as_str)
                            .unwrap_or_default(),
                        name
                    );
                    if &qualified == import {
                        if let Some(index) = self.owner_sources.get(owner) {
                            selected.insert(self.sources[*index].path.clone());
                        }
                    }
                }
            }
            for prefix in self
                .namespace_imports
                .get(&source.path)
                .into_iter()
                .flatten()
            {
                for ((path, name), owner) in &self.types {
                    let qualified = format!(
                        "{}.{}",
                        self.namespaces
                            .get(path)
                            .map(String::as_str)
                            .unwrap_or_default(),
                        name
                    );
                    if qualified
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('.'))
                    {
                        if let Some(index) = self.owner_sources.get(owner) {
                            selected.insert(self.sources[*index].path.clone());
                        }
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
                if self.sources.iter().any(|s| s.path == path) {
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
