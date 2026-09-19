use super::{
    model::{Location, Relation},
    Analysis,
};
use crate::parser::{CodeRange, ExtractedFile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Route {
    pub kind: String,
    pub storage: Location,
    pub invocation: Location,
    pub via: Vec<Location>,
    pub mutations: Vec<Location>,
    pub conditions: Vec<String>,
    #[serde(with = "proof_paths")]
    pub proof_paths: Arc<Vec<String>>,
    pub has_complete_dependencies: bool,
}
impl Route {
    pub(super) fn locations(&self) -> impl Iterator<Item = &Location> {
        std::iter::once(&self.storage)
            .chain(std::iter::once(&self.invocation))
            .chain(&self.via)
            .chain(&self.mutations)
    }
}

mod proof_paths {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        paths: &Arc<Vec<String>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        paths.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<Vec<String>>, D::Error> {
        Vec::<String>::deserialize(deserializer).map(Arc::new)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub routes: Vec<Route>,
    #[serde(skip)]
    pub by_path: BTreeMap<String, Vec<usize>>,
    pub notices: BTreeMap<String, BTreeSet<String>>,
}
pub(super) fn range(location: &Location) -> CodeRange {
    CodeRange {
        start_line: location.line,
        start_col: location.column,
        end_line: location.line,
        end_col: location.column.saturating_add(1),
    }
}
fn covers(outer: &CodeRange, location: &Location) -> bool {
    (outer.start_line, outer.start_col) <= (location.line, location.column)
        && (location.line, location.column) < (outer.end_line, outer.end_col)
}
fn locations(relation: &Relation) -> Vec<&Location> {
    std::iter::once(&relation.storage.location)
        .chain(std::iter::once(&relation.invocation.location))
        .chain(&relation.storage.via)
        .chain(&relation.invocation.via)
        .chain(&relation.mutations)
        .collect()
}

impl Snapshot {
    pub fn build(
        files: &[ExtractedFile],
        sources: &HashMap<String, String>,
        root: &Path,
        max_endpoints: usize,
        configured_endpoints_per_file: &BTreeMap<String, usize>,
        cache_directory: Option<&Path>,
    ) -> Self {
        if !crate::config::get().event_navigation.is_enabled {
            return Self::default();
        }
        let filter = crate::callers::test_code::TestCodeFilter::from_config(root);
        let mut inputs = BTreeMap::new();
        for (path, data) in sources {
            if filter.is_file_excluded(path) {
                continue;
            }
            let mut bytes = data.as_bytes().to_vec();
            filter.mask_source(path, &mut bytes);
            if let Ok(data) = String::from_utf8(bytes) {
                inputs.insert(path.clone(), data);
            }
        }
        let cache = cache_directory.and_then(|directory| {
            super::cache::SnapshotCache::open(
                directory,
                &(
                    root,
                    super::super::config_stamp(),
                    crate::config::get().max_file_size,
                    files,
                    sources.iter().collect::<BTreeMap<_, _>>(),
                    &inputs,
                    max_endpoints,
                    configured_endpoints_per_file,
                ),
            )
        });
        super::cache::load_or_build(cache, || {
            let bindings = super::manifests::bindings(sources);
            let analysis = super::analyze_with_bindings(&inputs, &bindings);
            Self::from_analysis(
                analysis,
                files,
                sources,
                root,
                max_endpoints,
                configured_endpoints_per_file,
            )
        })
    }

    pub(super) fn restore_indexes(&mut self) {
        self.by_path.clear();
        let mut proofs = BTreeMap::new();
        for (index, route) in self.routes.iter_mut().enumerate() {
            route.proof_paths = proofs
                .entry(route.proof_paths.as_ref().clone())
                .or_insert_with(|| Arc::clone(&route.proof_paths))
                .clone();
            for path in route
                .locations()
                .map(|point| &point.path)
                .collect::<BTreeSet<_>>()
            {
                self.by_path.entry(path.clone()).or_default().push(index);
            }
        }
    }
    fn from_analysis(
        analysis: Analysis,
        files: &[ExtractedFile],
        sources: &HashMap<String, String>,
        root: &Path,
        max_endpoints: usize,
        configured_endpoints_per_file: &BTreeMap<String, usize>,
    ) -> Self {
        let mut result = Self::default();
        let mut graph = analysis.dependencies;
        let incomplete = analysis.incomplete_dependencies;
        let filter = crate::callers::test_code::TestCodeFilter::from_config(root);
        let resolver =
            crate::callers::resolution::SourceResolver::from_stored_sources(files, root, sources);
        let files: BTreeMap<_, _> = files.iter().map(|f| (f.file_path.as_str(), f)).collect();
        let mut cfg_cache: BTreeMap<Location, Option<bool>> = BTreeMap::new();
        let mut proofs: BTreeMap<Vec<String>, Arc<Vec<String>>> = BTreeMap::new();
        let mut endpoint_locations: BTreeMap<String, BTreeSet<Location>> = BTreeMap::new();
        for (path, dependencies) in &mut graph {
            let kind = Path::new(path)
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or_default();
            if kind == "rs"
                || matches!(
                    kind,
                    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
                )
            {
                let manifest = if kind == "rs" {
                    "Cargo.toml"
                } else {
                    "package.json"
                };
                dependencies.extend(
                    sources
                        .keys()
                        .filter(|p| Path::new(p).file_name().is_some_and(|n| n == manifest))
                        .cloned(),
                );
            }
        }
        for (kind, detail) in analysis.notices {
            let paths: Vec<_> = sources
                .keys()
                .filter(|path| detail == **path || detail.starts_with(&format!("{path}:")))
                .cloned()
                .collect();
            if paths.is_empty() {
                result
                    .notices
                    .entry(String::new())
                    .or_default()
                    .insert(kind);
            } else {
                for path in paths {
                    result.notices.entry(path).or_default().insert(kind.clone());
                }
            }
        }
        for (mut relation, mut seeds) in analysis
            .relations
            .into_iter()
            .zip(analysis.relation_sources)
        {
            if result.routes.len().saturating_mul(2) + 2 > max_endpoints {
                result
                    .notices
                    .entry(String::new())
                    .or_default()
                    .insert("snapshot_endpoint_cap".into());
                break;
            }
            let points = locations(&relation);
            if points
                .iter()
                .any(|l| filter.is_excluded(&l.path, &range(l)))
            {
                continue;
            }
            let mut is_inactive = false;
            let mut is_cfg_unknown = false;
            let has_rust = points.iter().any(|l| l.path.ends_with(".rs"));
            for location in points.iter().filter(|l| l.path.ends_with(".rs")) {
                let active = *cfg_cache.entry((*location).clone()).or_insert_with(|| {
                    let file = files.get(location.path.as_str())?;
                    let enclosing = file
                        .symbols
                        .iter()
                        .filter(|symbol| covers(&symbol.range, location))
                        .min_by_key(|symbol| {
                            (
                                symbol.range.end_line - symbol.range.start_line,
                                symbol.range.end_col.saturating_sub(symbol.range.start_col),
                            )
                        });
                    resolver.condition_at(
                        &location.path,
                        &enclosing
                            .map(|symbol| symbol.range.clone())
                            .unwrap_or_else(|| range(location)),
                    )
                });
                if active == Some(false) {
                    is_inactive = true;
                } else if active.is_none() {
                    is_cfg_unknown = true;
                }
            }
            if is_inactive {
                continue;
            }
            if is_cfg_unknown {
                relation
                    .conditions
                    .push("rust_cfg_and_module_activation_required".into());
                relation.conditions.sort();
                relation.conditions.dedup();
            }
            let mut additions: BTreeMap<String, BTreeSet<Location>> = BTreeMap::new();
            for point in [&relation.storage.location, &relation.invocation.location] {
                if !endpoint_locations
                    .get(&point.path)
                    .is_some_and(|set| set.contains(point))
                {
                    additions
                        .entry(point.path.clone())
                        .or_default()
                        .insert(point.clone());
                }
            }
            if additions.iter().any(|(path, added)| {
                configured_endpoints_per_file
                    .get(path)
                    .copied()
                    .unwrap_or(0)
                    + endpoint_locations.get(path).map_or(0, BTreeSet::len)
                    + added.len()
                    > super::super::ENDPOINTS_PER_FILE
            }) {
                result
                    .notices
                    .entry(relation.storage.location.path.clone())
                    .or_default()
                    .insert("file_endpoint_cap".into());
                continue;
            }
            for point in [&relation.storage.location, &relation.invocation.location] {
                endpoint_locations
                    .entry(point.path.clone())
                    .or_default()
                    .insert(point.clone());
            }
            if has_rust {
                seeds.extend(
                    resolver
                        .source_dependencies()
                        .into_iter()
                        .map(|(path, _)| path),
                );
            }
            let mut pending: Vec<_> = seeds.iter().cloned().collect();
            let mut selected = BTreeSet::new();
            let mut complete = true;
            while let Some(path) = pending.pop() {
                if !selected.insert(path.clone()) {
                    continue;
                }
                if selected.len() > super::super::SOURCE_FILES_PER_QUERY {
                    complete = false;
                    break;
                }
                if incomplete.contains(&path) || !sources.contains_key(&path) {
                    complete = false;
                }
                pending.extend(
                    graph
                        .get(&path)
                        .into_iter()
                        .flatten()
                        .filter(|p| !selected.contains(*p))
                        .cloned(),
                );
            }
            let proof: Vec<_> = selected.into_iter().collect();
            let proof_paths = proofs
                .entry(proof.clone())
                .or_insert_with(|| Arc::new(proof))
                .clone();
            let index = result.routes.len();
            let paths: BTreeSet<_> = locations(&relation)
                .iter()
                .map(|l| l.path.clone())
                .collect();
            for path in paths {
                result.by_path.entry(path).or_default().push(index);
            }
            result.routes.push(Route {
                kind: relation.kind,
                storage: relation.storage.location,
                invocation: relation.invocation.location,
                via: relation
                    .storage
                    .via
                    .into_iter()
                    .chain(relation.invocation.via)
                    .collect(),
                mutations: relation.mutations,
                conditions: relation.conditions,
                proof_paths,
                has_complete_dependencies: complete,
            });
        }
        result
    }
}
