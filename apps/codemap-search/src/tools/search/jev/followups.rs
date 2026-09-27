//! Bounded discovery hints outside the rendered evidence, never asserted call edges.

use super::{simple_name, FilterInput, FilterResult, MatchState};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub(in crate::tools::search) fn append_call_candidates(
    text: &mut String,
    input: &FilterInput,
    result: &FilterResult,
    snapshot: &crate::index::PublishedIndexSnapshot,
    scope: Option<&str>,
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
        return;
    }
    let root = std::env::current_dir().unwrap_or_default();
    let exclusions = crate::callers::test_code::TestCodeFilter::from_config(&root);
    let codemap = snapshot.codemap();
    let mut groups: BTreeMap<&str, BTreeSet<(&str, usize)>> = BTreeMap::new();
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
            let entries = groups.entry(name).or_default();
            if entries.len() < config.caller_list_cap.min(super::MAX_LINKS_PER_QUESTION) {
                entries.insert((&file.file_path, call.range.start_line));
            } else {
                is_partial = true;
            }
        }
    }
    if groups.is_empty() {
        return;
    }
    let mut hints = "\n## Follow-up call candidates\nIndexed name matches outside the displayed source; target identity and task relevance are unverified. Verify with read/grep before claiming a connection.\n".to_string();
    let footer = "_Candidate list is partial; use grep for complete call-site enumeration._\n";
    'groups: for (name, entries) in groups {
        for (path, line) in entries {
            let row = crate::redact::source(&format!("- `{name}` candidate: {path}:{line}\n"))
                .into_owned();
            if hints.len() + row.len() + footer.len() > budget_bytes {
                is_partial = true;
                break 'groups;
            }
            hints.push_str(&row);
        }
    }
    if is_partial {
        hints.push_str(footer);
    }
    if hints.len() <= budget_bytes {
        text.push_str(&hints);
    }
}
