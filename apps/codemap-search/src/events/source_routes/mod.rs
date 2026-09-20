//! Bounded source-derived storage routes, independent of configured event APIs.
mod calls;
mod cache;
mod engine;
mod expressions;
mod model;
mod program;
mod projections;
mod statements;
pub(super) mod syntax;

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Serialize)]
pub(crate) struct Analysis {
    pub relations: Vec<model::Relation>,
    pub facts: Vec<model::Fact>,
    pub notices: Vec<(String, String)>,
    pub sources: BTreeMap<String, String>,
    pub omitted_relations: usize,
    pub dependencies: BTreeMap<String, std::collections::BTreeSet<String>>,
    pub incomplete_dependencies: std::collections::BTreeSet<String>,
    pub relation_sources: Vec<std::collections::BTreeSet<String>>,
}

fn analyze_with_bindings(
    sources: &BTreeMap<String, String>,
    bindings: &BTreeMap<String, String>,
) -> Analysis {
    let mut parsed = Vec::new();
    let mut notices = Vec::new();
    let mut digests = BTreeMap::new();
    let mut syntax_nodes = 0usize;
    for (path, data) in sources {
        if syntax_nodes >= 1_048_576 {
            notices.push(("snapshot_syntax_node_cap".into(), path.clone()));
            continue;
        }
        if let Some(source) = syntax::Source::parse(path, data) {
            if syntax_nodes + source.nodes.len() > 1_048_576 {
                notices.push(("snapshot_syntax_node_cap".into(), path.clone()));
                continue;
            }
            syntax_nodes += source.nodes.len();
            if source.has_parse_error {
                notices.push(("parse_error".into(), path.clone()));
            }
            if source.has_node_limit {
                notices.push(("syntax_node_cap".into(), path.clone()));
            }
            digests.insert(
                path.clone(),
                blake3::hash(data.as_bytes()).to_hex().to_string(),
            );
            parsed.push(Arc::new(source));
        }
    }
    let parsed = rust_macros::expand_sources(parsed, bindings);
    let program = syntax::Program::new(parsed, bindings.clone());
    let mut analyzer = engine::Analyzer::new(&program);
    let mut facts = analyzer.run();
    let (assembly_facts, assembly_omitted) = assembly::analyze(&program.sources);
    facts.extend(assembly_facts);
    analyzer.cap_facts(&mut facts);
    if assembly_omitted > 0 {
        notices.push(("assembly_fact_cap".into(), assembly_omitted.to_string()));
    }
    notices.extend(analyzer.notices);
    let (relations, omitted_relations) = model::connect(&facts, super::ENDPOINTS_PER_SNAPSHOT);
    if omitted_relations > 0 {
        notices.push(("relation_cap".into(), omitted_relations.to_string()));
    }
    let (dependencies, incomplete_dependencies) = program.dependency_graph();
    let relation_sources = relations
        .iter()
        .map(|r| program.relation_sources(r))
        .collect();
    Analysis {
        relations,
        facts,
        notices,
        sources: digests,
        omitted_relations,
        dependencies,
        incomplete_dependencies,
        relation_sources,
    }
}

mod dependencies;
mod generators;
mod manifests;
mod object_projections;
mod prototypes;
mod rust_deref;
mod rust_macros;
mod rust_names;
mod rust_types;
mod rust_values;
mod snapshot;
#[cfg(test)]
mod tests;
mod typescript;
pub(super) use snapshot::Snapshot;
mod render;
pub(super) use render::{Freshness, QueryState, RenderContext};
mod assembly;
mod callback_bindings;
mod conditional_syntax;
mod jvm;
mod polyglot;
mod polyglot_expressions;
mod polyglot_projections;
mod polyglot_statements;
mod rust_returns;
mod scala;

#[cfg(test)]
mod tests_frontend;
#[cfg(test)]
mod tests_public;
#[cfg(test)]
mod tests_reporting;
