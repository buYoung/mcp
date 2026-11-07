//! Bounded discovery hints outside the rendered evidence, never asserted call edges.
//! Jev also judges the callables around the first call sites with the user's task:
//! clearly unrelated callers are dropped and related ones are marked and listed first.

use super::{
    is_callable_kind, judge, simple_name, BlockSymbol, Evaluator, EvidenceStatus, FilterEntity,
    FilterInput, FilterPolicy, FilterResult, JudgedBodies, MatchState, Unit,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Callers judged per search, in one extra judging pass.
const MAX_JUDGED_CALLERS: usize = 8;

/// Call sites of the matched callables' names outside the displayed source.
pub(in crate::tools::search) struct CallCandidates {
    groups: BTreeMap<String, BTreeSet<(String, usize)>>,
    is_partial: bool,
    /// Task match of the callable around a call site, keyed by its path and line.
    judged: HashMap<(String, usize), MatchState>,
}

pub(in crate::tools::search) fn call_candidates(
    input: &FilterInput,
    result: &FilterResult,
    snapshot: &crate::index::PublishedIndexSnapshot,
    scope: Option<&str>,
) -> Option<CallCandidates> {
    let config = crate::config::get();
    let names: HashSet<_> = result
        .decisions
        .iter()
        // Retention also covers row-only and unavailable source. Those declarations
        // have no positive task evidence and must not seed unrelated name expansion.
        .filter(|decision| decision.match_state == MatchState::Matched)
        .map(|decision| &input.entities[decision.entity])
        .filter(|entity| entity.is_callable)
        .map(|entity| simple_name(&entity.symbol.name))
        .collect();
    if names.is_empty() {
        return None;
    }
    let root = crate::workspace::root_dir().unwrap_or_default();
    let exclusions = crate::callers::test_code::TestCodeFilter::from_config(&root);
    let codemap = snapshot.codemap();
    let mut groups: BTreeMap<String, BTreeSet<(String, usize)>> = BTreeMap::new();
    let mut inspected = 0;
    let mut is_partial = false;
    'files: for file in codemap.iter() {
        if scope.is_some_and(|scope| {
            !super::super::monorepo::path_is_under_scope(&file.file_path, scope)
        }) || exclusions.is_file_excluded(&file.file_path)
        {
            continue;
        }
        let Some(navigation) = &file.navigation else {
            continue;
        };
        for call in &navigation.calls {
            let name = simple_name(&call.name);
            if !names.contains(name) {
                continue;
            }
            if inspected >= config.navigation_callsite_budget {
                is_partial = true;
                break 'files;
            }
            inspected += 1;
            if exclusions.is_excluded(&file.file_path, &call.range)
                || input.entities.iter().any(|entity| {
                    entity.path == file.file_path
                        && entity.displayed.is_some_and(|(first, last)| {
                            first <= call.range.start_line && call.range.end_line <= last
                        })
                })
            {
                continue;
            }
            let entries = groups.entry(name.to_string()).or_default();
            if entries.len() < config.caller_list_cap.min(super::MAX_LINKS_PER_QUESTION) {
                entries.insert((file.file_path.clone(), call.range.start_line));
            } else {
                is_partial = true;
            }
        }
    }
    (!groups.is_empty()).then(|| CallCandidates {
        groups,
        is_partial,
        judged: HashMap::new(),
    })
}

impl CallCandidates {
    /// Judge the callables around the first call sites with the user's task. Their
    /// cost joins `result`; a failed or unavailable judgment leaves every row unjudged.
    pub(in crate::tools::search) async fn judge(
        &mut self,
        input: &FilterInput,
        result: &mut FilterResult,
        snapshot: &crate::index::PublishedIndexSnapshot,
        evaluator: &dyn Evaluator,
        policy: &FilterPolicy,
    ) {
        let started = tokio::time::Instant::now();
        let codemap = snapshot.codemap();
        let mut entities: Vec<FilterEntity> = Vec::new();
        let mut sites: Vec<(usize, (String, usize))> = Vec::new();
        'groups: for entries in self.groups.values() {
            for (path, line) in entries {
                let Some(symbol) = codemap
                    .iter()
                    .find(|file| file.file_path == *path)
                    .and_then(|file| {
                        file.symbols
                            .iter()
                            .filter(|symbol| {
                                is_callable_kind(&symbol.kind)
                                    && symbol.range.start_line <= *line
                                    && *line <= symbol.range.end_line_inclusive()
                            })
                            .min_by_key(|symbol| {
                                symbol.range.end_line_inclusive() - symbol.range.start_line
                            })
                    })
                else {
                    continue;
                };
                let symbol = BlockSymbol::from_symbol(symbol);
                let masked_path = crate::redact::source(path).into_owned();
                if let Some(index) = entities
                    .iter()
                    .position(|entity| entity.path == masked_path && entity.symbol == symbol)
                {
                    sites.push((index, (path.clone(), *line)));
                    continue;
                }
                if entities.len() >= MAX_JUDGED_CALLERS {
                    break 'groups;
                }
                let Some(body) = super::evidence::callable_source(path, &symbol) else {
                    continue;
                };
                entities.push(FilterEntity {
                    file_index: entities.len(),
                    path: masked_path,
                    symbol,
                    block_index: None,
                    displayed: None,
                    evidence: EvidenceStatus::Complete,
                    is_callable: true,
                    body_bytes: body.len(),
                    body: Some(body),
                    outgoing: Vec::new(),
                    incoming: Vec::new(),
                    parent: None,
                    is_requested: false,
                    is_match_view: false,
                    unit: Unit::Declaration,
                });
                sites.push((entities.len() - 1, (path.clone(), *line)));
            }
        }
        let Ok(threshold) = super::validate_threshold(policy.min_unrelated_probability) else {
            return;
        };
        if entities.is_empty() {
            return;
        }
        let callers = FilterInput {
            entities,
            ..input.clone()
        };
        let indices: Vec<usize> = (0..callers.entities.len()).collect();
        let Some(JudgedBodies {
            judgments,
            usage,
            timing,
            requests,
            failure,
        }) = judge(&callers, &indices, evaluator, policy, started).await
        else {
            return;
        };
        result.usage = result.usage + usage;
        result.timing.http += timing.http;
        result.timing.queue_wait += timing.queue_wait;
        result.timing.request_count += timing.request_count;
        result.requests.extend(requests);
        if failure.is_some() {
            return;
        }
        let states = super::compose(&callers, &judgments, threshold);
        for (index, site) in sites {
            self.judged.insert(site, states[index]);
        }
    }
}

pub(in crate::tools::search) fn append_call_candidates(
    text: &mut String,
    candidates: &CallCandidates,
) {
    let config = crate::config::get();
    let budget_bytes = config.search_detail_byte_cap;
    let budget_bytes = super::super::display::delivery_byte_cap(budget_bytes)
        .saturating_sub(text.len())
        .min(config.annotation_sub_budget)
        .min(4096);
    if budget_bytes < 512 {
        return;
    }
    let state = |site: &(String, usize)| candidates.judged.get(site).copied();
    let mut rows: Vec<(bool, &str, &(String, usize))> = candidates
        .groups
        .iter()
        .flat_map(|(name, entries)| entries.iter().map(move |site| (name.as_str(), site)))
        .filter(|(_, site)| state(site) != Some(MatchState::NoMatch))
        .map(|(name, site)| (state(site) == Some(MatchState::Matched), name, site))
        .collect();
    if rows.is_empty() {
        return;
    }
    // Related callers first; the sort is stable, so name and location order stays within each.
    rows.sort_by_key(|(is_related, _, _)| !is_related);
    let mut hints = if candidates.judged.is_empty() {
        "\n## Follow-up call candidates\nIndexed name matches outside the displayed source; target identity and task relevance are unverified. Verify with read/grep before claiming a connection.\n".to_string()
    } else {
        "\n## Follow-up call candidates\nIndexed name matches outside the displayed source; target identity is unverified. Jev judged the enclosing callers of the first matches against the user's task: related ones are marked and listed first, unrelated ones are left out. Verify with read/grep before claiming a connection.\n".to_string()
    };
    let footer = "_Candidate list is partial; use grep for complete call-site enumeration._\n";
    let mut is_partial = candidates.is_partial;
    for (is_related, name, (path, line)) in rows {
        let mark = if is_related { " (Jev: related)" } else { "" };
        let row = crate::redact::source(&format!("- `{name}` candidate: {path}:{line}{mark}\n"))
            .into_owned();
        if hints.len() + row.len() + footer.len() > budget_bytes {
            is_partial = true;
            break;
        }
        hints.push_str(&row);
    }
    if is_partial {
        hints.push_str(footer);
    }
    if hints.len() <= budget_bytes {
        text.push_str(&hints);
    }
}
