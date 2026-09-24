//! Capture one-hop supporting evidence without spending a search-wide prefix budget.

use super::*;
use std::collections::BTreeSet;

impl FilterEntity {
    pub(super) fn direct_callers(&self) -> Vec<usize> {
        self.incoming
            .iter()
            .take(MAX_LINKS_PER_QUESTION)
            .copied()
            .chain(self.indexed_callers.iter().map(|(index, _)| *index))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(super) fn direct_support(&self) -> Vec<usize> {
        self.direct_callers()
            .into_iter()
            .chain(self.outgoing.iter().take(MAX_LINKS_PER_QUESTION).copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

/// Reuse the ordinary read path's scope checks and full-file secret masking. Neither
/// this read nor its result invokes Jev, expands call chains, or changes agent output.
fn supporting_source(caller: &FilterEntity, file: &FileOutput) -> Option<String> {
    if let Some(body) = &caller.body {
        return Some(body.clone());
    }
    let first = caller.symbol.start_line;
    let last = caller.symbol.end_line;
    let output = crate::tools::read::read_file_with_metadata(&json!({
        "file_path": file.path, "offset": first,
        "limit": last.saturating_sub(first) + 1, "view": "source", "expand": "none",
    }))
    .ok()?;
    if output.text.len() > MAX_COMPLETE_BODY_BYTES
        || !identity_verified(&output.text, &caller.symbol)
        || is_masked_unavailable(&output.text)
    {
        return None;
    }
    let last_returned = output
        .text
        .lines()
        .filter_map(|line| line.split_once('→')?.0.trim().parse::<usize>().ok())
        .next_back()?;
    (last_returned == last).then_some(output.text)
}

impl FilterInput {
    pub(crate) fn add_supporting_evidence(
        &mut self,
        files: &[&FileOutput],
        snapshot: &crate::index::PublishedIndexSnapshot,
        scope: Option<&str>,
        should_include_events: bool,
        is_snapshot_fresh: bool,
        policy: &FilterPolicy,
    ) {
        self.is_snapshot_fresh = is_snapshot_fresh;
        if !is_snapshot_fresh {
            return;
        }
        let should_include_callers = self
            .search_arguments
            .get("caller_context")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let indices = self.judgeable();
        let callers: BTreeSet<_> = indices
            .iter()
            .filter(|_| should_include_callers)
            .flat_map(|index| self.entities[*index].direct_support())
            .collect();
        for index in callers {
            let caller = &self.entities[index];
            if let Some(source) = supporting_source(caller, files[caller.file_index]) {
                self.supporting_sources.insert(index, source);
            }
        }
        let root = std::env::current_dir().unwrap_or_default();
        for index in indices {
            // Keep direct support intact; do not silently cut it to make a negative
            // judgment fit. Preflight also accounts for the goal and every criterion.
            if !questions::candidate_fits(self, index, policy) {
                self.entities[index].is_context_clipped = true;
                continue;
            }
            let entity = &self.entities[index];
            let file = files[entity.file_index];
            let annotation =
                crate::redact::source(file.annotation_for(&entity.symbol).unwrap_or(""))
                    .into_owned();
            self.entities[index].supporting_context = annotation;
            if !questions::candidate_fits(self, index, policy) {
                self.entities[index].supporting_context.clear();
                self.entities[index].is_context_clipped = true;
                continue;
            }
            if should_include_events {
                let entity = &self.entities[index];
                let mut anchors: Vec<_> = entity
                    .direct_callers()
                    .into_iter()
                    .filter(|_| should_include_callers)
                    .map(|caller| {
                        let caller = &self.entities[caller];
                        (
                            files[caller.file_index].path.clone(),
                            caller.symbol.start_line,
                            caller.symbol.end_line,
                        )
                    })
                    .collect();
                anchors.push((
                    file.path.clone(),
                    entity.symbol.start_line,
                    entity.symbol.end_line,
                ));
                let events = snapshot.events().for_paths_with_context(
                    &anchors,
                    scope,
                    MAX_COMPLETE_BODY_BYTES,
                    &root,
                    Some(&file.path),
                    None,
                );
                let events = crate::redact::source(&events);
                let old_len = self.entities[index].supporting_context.len();
                if !events.is_empty() {
                    self.entities[index].supporting_context.push('\n');
                    self.entities[index].supporting_context.push_str(&events);
                }
                if !questions::candidate_fits(self, index, policy) {
                    self.entities[index].supporting_context.truncate(old_len);
                    self.entities[index].is_context_clipped = true;
                }
            }
        }
    }
}
