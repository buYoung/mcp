//! Source-backed selection of overview rows. The evidence is private to Jev; only
//! the existing declaration rows are removed, without adding source to the response.
use super::{context, external, CapturedFile};
use crate::parser::{ExtractedFile, ExtractedSymbol};
use crate::tools::search::jev::{
    self, BlockSymbol, EvidenceStatus, FilterEntity, FilterInput, FilterPolicy, FilterResult,
    SelectionUnit,
};
use crate::tools::task::RegisteredTask;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DeclarationKey {
    path: String,
    name: String,
    kind: String,
    start: usize,
    end: usize,
}

impl DeclarationKey {
    pub(crate) fn new(path: &str, name: &str, kind: &str, start: usize, end: usize) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            kind: kind.into(),
            start,
            end,
        }
    }

    pub(crate) fn from_symbol(path: &str, symbol: &ExtractedSymbol) -> Self {
        Self::new(
            path,
            &symbol.name,
            &symbol.kind,
            symbol.range.start_line,
            symbol.range.end_line_inclusive(),
        )
    }
}

pub(crate) struct Plan {
    pub input: FilterInput,
    keys: Vec<DeclarationKey>,
    captured: Vec<CapturedFile>,
}

impl Plan {
    pub(crate) fn add_indexed_context(
        &mut self,
        engine: &crate::index::EngineSupervisor,
        policy: &FilterPolicy,
    ) {
        external::augment(&mut self.input, engine, &self.captured, policy);
    }

    pub(crate) fn omitted(&self, result: &FilterResult) -> HashSet<DeclarationKey> {
        result
            .decisions
            .iter()
            .filter(|decision| !decision.is_retained)
            .map(|decision| self.keys[decision.entity].clone())
            .collect()
    }
}

pub(crate) fn capture(
    files: &[&ExtractedFile],
    task: &RegisteredTask,
    arguments: &serde_json::Value,
    policy: &FilterPolicy,
) -> Plan {
    let root = std::env::current_dir().unwrap_or_default();
    let exclusions = crate::callers::test_code::TestCodeFilter::from_config(&root);
    let mut plan = Plan {
        input: FilterInput {
            selection_unit: SelectionUnit::Declaration,
            evidence_version: "overview-task-evidence/1",
            task: task.masked(),
            search_arguments: super::masked_value(arguments),
            entities: Vec::new(),
            file_count: files.len(),
            is_snapshot_fresh: true,
            supporting_sources: HashMap::new(),
        },
        keys: Vec::new(),
        captured: Vec::new(),
    };
    for file in files {
        if plan.captured.len() >= jev::MAX_LINKS_PER_QUESTION || external::has_stopped(policy) {
            break;
        }
        if exclusions.is_file_excluded(&file.file_path) {
            continue;
        }
        let Some(captured) = external::load_indexed(file, &exclusions) else {
            continue;
        };
        let file_index = plan.captured.len();
        let lines: Vec<_> = captured.source.split('\n').collect();
        for symbol in crate::codemap::significant_symbols(&file.symbols) {
            let mut entity = declaration_entity(file_index, file, symbol);
            if !external::has_stopped(policy)
                && !exclusions.is_excluded(&file.file_path, &symbol.range)
            {
                capture_body(&mut entity, &captured, symbol, &lines);
            }
            entity.block_index = Some(plan.keys.len());
            plan.keys
                .push(DeclarationKey::from_symbol(&file.file_path, symbol));
            plan.input.entities.push(entity);
        }
        plan.captured.push(captured);
    }
    let entities = &mut plan.input.entities;
    for index in 0..entities.len() {
        entities[index].parent = (0..entities.len())
            .filter(|&other| {
                other != index
                    && entities[other].file_index == entities[index].file_index
                    && entities[other].symbol.contains(&entities[index].symbol)
            })
            .min_by_key(|&other| entities[other].line_count());
    }
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, entity) in entities
        .iter()
        .enumerate()
        .filter(|(_, entity)| entity.is_callable)
    {
        by_name
            .entry(jev::simple_name(&entity.symbol.name).into())
            .or_default()
            .push(index);
    }
    for (file_index, captured) in plan.captured.iter().enumerate() {
        for call in captured
            .file
            .navigation
            .iter()
            .flat_map(|navigation| &navigation.calls)
        {
            let owner = entities
                .iter()
                .enumerate()
                .filter(|(_, entity)| {
                    entity.file_index == file_index
                        && entity.is_callable
                        && entity.symbol.start_line <= call.range.start_line
                        && call.range.start_line <= entity.symbol.end_line
                })
                .min_by_key(|(_, entity)| entity.line_count())
                .map(|(index, _)| index);
            let Some(owner) = owner else {
                continue;
            };
            if let Some(target) = jev::resolve_call(entities, &by_name, call, owner) {
                if target != owner && !entities[owner].outgoing.contains(&target) {
                    entities[owner].outgoing.push(target);
                    entities[target].incoming.push(owner);
                }
            } else {
                entities[owner].unresolved_calls += 1;
            }
        }
    }
    plan
}

fn declaration_entity(
    file_index: usize,
    file: &ExtractedFile,
    symbol: &ExtractedSymbol,
) -> FilterEntity {
    let start = symbol.range.start_line;
    let end = symbol.range.end_line_inclusive();
    FilterEntity {
        file_index,
        path: crate::redact::source(&file.file_path).into_owned(),
        symbol: BlockSymbol {
            name: crate::redact::source(&symbol.name).into_owned(),
            kind: symbol.kind.clone(),
            owner: symbol
                .owner
                .as_deref()
                .map(|owner| crate::redact::source(owner).into_owned()),
            start_line: start,
            end_line: end,
        },
        block_index: None,
        displayed: Some((start, end)),
        evidence: EvidenceStatus::NoSource,
        is_callable: jev::is_callable_kind(&symbol.kind),
        is_masked: false,
        body_bytes: 0,
        body: None,
        outgoing: Vec::new(),
        incoming: Vec::new(),
        indexed_callers: Vec::new(),
        omitted_indexed_callers: 0,
        parent: None,
        unresolved_calls: 0,
        supporting_context: String::new(),
        is_context_clipped: false,
        has_missing_context: false,
    }
}

fn capture_body(
    entity: &mut FilterEntity,
    captured: &CapturedFile,
    symbol: &ExtractedSymbol,
    lines: &[&str],
) {
    let Some(index) = captured.file.symbols.iter().position(|current| {
        current.name == symbol.name
            && current.kind == symbol.kind
            && current.owner == symbol.owner
            && current.range == symbol.range
    }) else {
        return;
    };
    let bounds = captured
        .bounds
        .iter()
        .find(|bounds| bounds.symbol_index == index);
    let start = bounds.map_or(symbol.range.start_line, |bounds| bounds.start);
    let end = symbol.range.end_line_inclusive();
    let Some(source) = start.checked_sub(1).and_then(|first| lines.get(first..end)) else {
        return;
    };
    let body: String = source
        .iter()
        .enumerate()
        .map(|(offset, line)| format!("{}→{line}\n", start + offset))
        .collect();
    entity.symbol.start_line = start;
    entity.displayed = Some((start, end));
    entity.body_bytes = body.len();
    entity.is_masked = body.contains(crate::redact::MARKER);
    entity.evidence = if body.len() > jev::MAX_COMPLETE_BODY_BYTES {
        EvidenceStatus::Oversized
    } else if (entity.is_callable && bounds.is_none())
        || !jev::identity_verified(&body, &entity.symbol)
    {
        EvidenceStatus::IdentityUnverified
    } else if jev::is_masked_unavailable(&body) {
        EvidenceStatus::MaskedUnavailable
    } else {
        EvidenceStatus::Complete
    };
    if entity.evidence == EvidenceStatus::Complete {
        let support = context::capture(captured, index);
        entity.body = Some(body);
        entity.supporting_context = support.text;
        entity.is_context_clipped = support.is_clipped;
        entity.has_missing_context = support.has_missing;
    }
}
