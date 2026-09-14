use super::*;
use crate::parser::ExtractedFile;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct EventIndex {
    stamp: String,
    sources: Arc<HashMap<String, String>>,
    endpoints: Arc<Vec<EventEndpoint>>,
    by_path: HashMap<String, Vec<usize>>,
    by_key: HashMap<String, Vec<usize>>,
    by_route: HashMap<(String, String, String, String), Vec<usize>>,
    unavailable_sources: usize,
    omitted_endpoints: usize,
    is_enabled: bool,
    has_rust_sources: bool,
}

fn under_scope(path: &str, scope: Option<&str>) -> bool {
    scope.is_none_or(|scope| {
        path == scope
            || path
                .strip_prefix(scope)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}
fn display(location: &EventLocation) -> String {
    match location.name.as_deref() {
        Some(name) => format!(
            "{name} — {}:{}",
            location.file_path, location.range.start_line
        ),
        None => format!("{}:{}", location.file_path, location.range.start_line),
    }
}
fn overlaps(location: &EventLocation, anchors: &[(String, usize, usize)]) -> bool {
    anchors.is_empty()
        || anchors.iter().any(|(path, start, end)| {
            path == &location.file_path
                && location.range.start_line <= *end
                && *start <= location.range.end_line_inclusive()
        })
}

fn endpoint_overlaps(endpoint: &EventEndpoint, anchors: &[(String, usize, usize)]) -> bool {
    overlaps(&endpoint.location, anchors)
        || endpoint
            .api_definition
            .as_ref()
            .is_some_and(|location| overlaps(location, anchors))
        || endpoint
            .handler
            .as_ref()
            .is_some_and(|location| overlaps(location, anchors))
        || endpoint
            .key_evidence
            .iter()
            .any(|location| overlaps(location, anchors))
        || endpoint.bus.as_ref().is_some_and(|bus| {
            bus.locations
                .iter()
                .any(|location| overlaps(location, anchors))
        })
}

impl EventIndex {
    pub(crate) fn indexed_sources(&self) -> &HashMap<String, String> {
        &self.sources
    }

    pub fn build(files: &[ExtractedFile], inputs: EventInputs) -> Self {
        let started = std::time::Instant::now();
        let cfg = crate::config::get();
        let stamp = super::config_stamp();
        let unavailable_sources = inputs.unavailable;
        let sources = Arc::new(inputs.into_sources());
        let mut extraction = if cfg.event_navigation.is_enabled {
            super::extract::extract(
                files,
                &sources,
                &std::env::current_dir().unwrap_or_default(),
            )
        } else {
            super::extract::Extraction {
                endpoints: Vec::new(),
                omitted: 0,
                parse_failures: 0,
            }
        };
        extraction.endpoints.sort_by(|a, b| {
            a.location
                .file_path
                .cmp(&b.location.file_path)
                .then(
                    a.location
                        .range
                        .start_line
                        .cmp(&b.location.range.start_line),
                )
                .then(a.location.range.start_col.cmp(&b.location.range.start_col))
        });
        let mut by_path: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_route: HashMap<(String, String, String, String), Vec<usize>> = HashMap::new();
        for (i, endpoint) in extraction.endpoints.iter().enumerate() {
            let mut paths = HashSet::from([endpoint.location.file_path.as_str()]);
            if let Some(location) = &endpoint.api_definition {
                paths.insert(&location.file_path);
            }
            if let Some(handler) = &endpoint.handler {
                paths.insert(&handler.file_path);
            }
            for location in &endpoint.key_evidence {
                paths.insert(&location.file_path);
            }
            if let Some(bus) = &endpoint.bus {
                for location in &bus.locations {
                    paths.insert(&location.file_path);
                }
            }
            for path in paths {
                by_path.entry(path.into()).or_default().push(i);
            }
            if let Some(key) = &endpoint.event_key {
                by_key.entry(key.clone()).or_default().push(i);
            }
            if let Some((bus, key, target, channel)) = endpoint.key() {
                by_route
                    .entry((bus.into(), key.into(), target.into(), channel.into()))
                    .or_default()
                    .push(i);
            }
        }
        tracing::debug!(
            elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
            event_count = extraction.endpoints.len(),
            source_count = sources.len(),
            unavailable_sources,
            "built event navigation snapshot"
        );
        let has_rust_sources = sources.keys().any(|path| path.ends_with(".rs"));
        Self {
            stamp,
            sources,
            endpoints: Arc::new(extraction.endpoints),
            by_path,
            by_key,
            by_route,
            unavailable_sources: unavailable_sources + extraction.parse_failures,
            omitted_endpoints: extraction.omitted,
            is_enabled: cfg.event_navigation.is_enabled,
            has_rust_sources,
        }
    }

    pub fn for_key(&self, key: &str, scope: Option<&str>, cap: usize, root: &Path) -> String {
        self.render(
            self.by_key.get(key).map(Vec::as_slice).unwrap_or(&[]),
            0,
            scope,
            cap,
            root,
            None,
        )
    }
    pub fn for_paths(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
    ) -> String {
        if anchors.is_empty() {
            return String::new();
        }
        let mut selected = BTreeSet::new();
        let mut routes = BTreeSet::new();
        let mut candidate_omissions = 0;
        let mut inspected = 0;
        let paths: BTreeSet<_> = anchors.iter().map(|(path, _, _)| path).collect();
        for path in paths {
            let candidates = self.by_path.get(path).map(Vec::as_slice).unwrap_or(&[]);
            let remaining = super::CANDIDATES_PER_QUERY.saturating_sub(inspected);
            candidate_omissions += candidates.len().saturating_sub(remaining);
            for &index in candidates.iter().take(remaining) {
                inspected += 1;
                let endpoint = &self.endpoints[index];
                if endpoint_overlaps(endpoint, anchors) {
                    selected.insert(index);
                    if let Some((bus, key, target, channel)) = endpoint.key() {
                        routes.insert((
                            bus.to_string(),
                            key.to_string(),
                            target.to_string(),
                            channel.to_string(),
                        ));
                    }
                }
            }
        }
        if selected.is_empty() {
            return String::new();
        }
        for route in routes {
            let candidates = self.by_route.get(&route).map(Vec::as_slice).unwrap_or(&[]);
            let remaining = super::CANDIDATES_PER_QUERY.saturating_sub(inspected);
            candidate_omissions += candidates.len().saturating_sub(remaining);
            for &index in candidates.iter().take(remaining) {
                inspected += 1;
                selected.insert(index);
            }
        }
        self.render(
            &selected.into_iter().collect::<Vec<_>>(),
            candidate_omissions,
            scope,
            cap,
            root,
            Some(anchors),
        )
    }

    fn render(
        &self,
        indices: &[usize],
        candidate_omissions: usize,
        scope: Option<&str>,
        cap: usize,
        root: &Path,
        anchors: Option<&[(String, usize, usize)]>,
    ) -> String {
        if anchors.is_none() && cap < 512 {
            return "[Event output budget unavailable; narrow the source request.]\n"
                .chars()
                .take(cap)
                .collect();
        }
        if !crate::config::get().event_navigation.is_enabled {
            if anchors.is_some() {
                return String::new();
            }
            return "## Event relationships\n\n[Event navigation disabled; set event_navigation.is_enabled=true and wait for indexing.]\n".into();
        }
        if !self.is_enabled || self.stamp != super::config_stamp() {
            if anchors.is_some() {
                return String::new();
            }
            return "## Event relationships\n\n[Event settings/target/exclusions changed; a matching indexed generation is not ready. No stale event links are shown.]\n".into();
        }
        let filter = crate::callers::test_code::TestCodeFilter::from_config(root);
        let mut freshness = HashMap::new();
        let mut eligible = Vec::new();
        let mut filtered = 0;
        let mut stale = 0;
        let mut source_bytes = 0;
        let mut source_files = 0;
        let mut source_budget_omissions = 0;
        for &index in indices.iter().take(super::CANDIDATES_PER_QUERY) {
            let endpoint = &self.endpoints[index];
            if !under_scope(&endpoint.location.file_path, scope) {
                continue;
            }
            let mut locations = vec![&endpoint.location];
            locations.extend(endpoint.dependencies.iter());
            if let Some(handler) = &endpoint.handler {
                locations.push(handler);
            }
            let is_fresh = locations.iter().all(|location| {
                *freshness
                    .entry(location.file_path.clone())
                    .or_insert_with(|| {
                        let Some(expected) = self.sources.get(&location.file_path) else {
                            return false;
                        };
                        if source_files >= super::SOURCE_FILES_PER_QUERY
                            || source_bytes + expected.len() > super::SOURCE_BYTES_PER_QUERY
                        {
                            source_budget_omissions += 1;
                            return false;
                        }
                        source_files += 1;
                        source_bytes += expected.len();
                        let path = root.join(&location.file_path);
                        std::fs::metadata(&path).is_ok_and(|m| m.len() == expected.len() as u64)
                            && std::fs::read(path).is_ok_and(|bytes| bytes == expected.as_bytes())
                    })
            });
            if !is_fresh {
                stale += 1;
                continue;
            }
            if locations.iter().any(|location| {
                filter.is_excluded(&location.file_path, &location.range)
                    || !crate::workspace::walk_root_is_visible(
                        &root.join(&location.file_path),
                        true,
                    )
            }) {
                filtered += 1;
                continue;
            }
            eligible.push(endpoint);
        }
        let total = eligible.len();
        // Automatic context needs a current, allowed endpoint or supporting
        // definition in the requested lines, not just a surviving route peer.
        if anchors.is_some_and(|anchors| {
            !eligible
                .iter()
                .any(|endpoint| endpoint_overlaps(endpoint, anchors))
        }) {
            return String::new();
        }
        if cap < 512 {
            return "[Event output budget unavailable; narrow the source request.]\n"
                .chars()
                .take(cap)
                .collect();
        }
        let header="## Event relationships\n\n_Static registration routes, separate from direct calls. Delivery, registration order and removal timing are not guaranteed._\n";
        let mut out = header.to_string();
        if self.has_rust_sources {
            out.push_str(&format!(
                "\n_Rust analysis target_os={} (explicit configuration; no host inference)._\n",
                crate::config::get()
                    .analysis_target_os
                    .as_deref()
                    .unwrap_or("<neutral>")
            ));
        }
        let mut append = |row: &str| {
            if out.len() + row.len() + 512.min(cap / 2) <= cap {
                out.push_str(row);
                true
            } else {
                false
            }
        };
        let mut groups: BTreeMap<(String, String, String, String), Vec<&EventEndpoint>> =
            BTreeMap::new();
        let mut unresolved = Vec::new();
        for endpoint in eligible.into_iter().take(super::ENDPOINTS_PER_QUERY) {
            if let Some((bus, key, target, channel)) = endpoint.key() {
                groups
                    .entry((bus.into(), key.into(), target.into(), channel.into()))
                    .or_default()
                    .push(endpoint);
            } else {
                unresolved.push(endpoint);
            }
        }
        let mut was_capped = false;
        let mut rendered_endpoints = 0;
        for ((identity, key, target, channel), endpoints) in groups {
            let bus = endpoints[0].bus.as_ref().unwrap();
            let section=format!("\n### Event {key:?}\n- bus: `{identity}` — {}{}\n- target: `{target}`; channel: `{channel}`\n",bus.description,if bus.is_configured_assumption {" [configured bus assumption]"}else{""});
            if !append(&section) {
                was_capped = true;
                break;
            }
            for endpoint in endpoints {
                let mut row = String::new();
                row.push_str(&format!(
                    "- {}: {}{}\n  - API: `{}`{}\n",
                    endpoint.role.label(),
                    display(&endpoint.location),
                    endpoint
                        .enclosing_symbol
                        .as_ref()
                        .map(|s| format!(" in {s}"))
                        .unwrap_or_default(),
                    endpoint.api_identity,
                    if endpoint.is_api_configured {
                        " [user rule]"
                    } else {
                        " [built-in rule]"
                    }
                ));
                row.push_str(&format!("  - rule: `{}`\n", endpoint.api_rule));
                if let Some(location) = &endpoint.api_definition {
                    row.push_str(&format!("  - API definition: {}\n", display(location)));
                }
                if endpoint.is_target_configured {
                    row.push_str(
                        "  - target supplied by API rule [configured target assumption]\n",
                    );
                }
                if endpoint.is_key_configured {
                    row.push_str("  - key supplied by API rule [configured key assumption]\n");
                } else if let Some(evidence) = endpoint
                    .key_evidence
                    .iter()
                    .find(|location| location.name.is_some())
                    .or_else(|| endpoint.key_evidence.first())
                {
                    row.push_str(&format!("  - key evidence: {}\n", display(evidence)));
                }
                if let Some(handler) = &endpoint.handler {
                    row.push_str(&format!("  - handler definition: {}\n", display(handler)));
                }
                if let Some(reason) = &endpoint.handler_reason {
                    row.push_str(&format!("  - handler unresolved: {reason}\n"));
                }
                if endpoint.is_once {
                    row.push_str(
                        "  - once-only registration; execution order is not established\n",
                    );
                }
                for condition in &endpoint.conditions {
                    row.push_str(&format!("  - conditional: {condition}\n"));
                }
                if endpoint.role == EventRole::Unsubscribe {
                    row.push_str(
                        "  - removal observed; remaining registrations are not simulated\n",
                    );
                }
                if !append(&row) {
                    was_capped = true;
                    break;
                }
                rendered_endpoints += 1;
            }
            if was_capped {
                break;
            }
        }
        for endpoint in unresolved {
            if was_capped {
                break;
            }
            let reasons = if endpoint.is_inactive {
                "statically inactive condition".into()
            } else {
                endpoint.unresolved_reasons.join("; ")
            };
            let mut row = format!(
                "\n- unresolved {}: {} — {reasons}\n  - API: `{}`; key: {:?}\n",
                endpoint.role.label(),
                display(&endpoint.location),
                endpoint.api_identity,
                endpoint.event_key
            );
            if let Some(handler) = &endpoint.handler {
                row.push_str(&format!("  - handler definition: {}\n", display(handler)));
            }
            if let Some(reason) = &endpoint.handler_reason {
                row.push_str(&format!("  - handler unresolved: {reason}\n"));
            }
            row.push_str(&format!(
                "  - rule: `{}`; target: {:?}; channel: `{}`\n",
                endpoint.api_rule, endpoint.target, endpoint.channel
            ));
            if endpoint.is_once {
                row.push_str("  - once-only registration; execution order is not established\n");
            }
            if endpoint.role == EventRole::Unsubscribe {
                row.push_str("  - removal observed; remaining registrations are not simulated\n");
            }
            for condition in &endpoint.conditions {
                row.push_str(&format!("  - conditional: {condition}\n"));
            }
            if !append(&row) {
                was_capped = true;
                break;
            }
            rendered_endpoints += 1;
        }
        if total == 0 {
            append("\n[No eligible event endpoints in this bounded indexed scope. Unknown/unconfigured API receivers are not joined by method spelling.]\n");
        }
        let query_omissions = candidate_omissions
            + indices.len().saturating_sub(super::CANDIDATES_PER_QUERY)
            + total.saturating_sub(super::ENDPOINTS_PER_QUERY);
        if self.unavailable_sources > 0
            || self.omitted_endpoints > 0
            || query_omissions > 0
            || filtered > 0
            || stale > 0
        {
            out.push_str(&format!("\n[Event coverage: {} unavailable/capped source inputs, {} extraction omissions, {query_omissions} query candidate/endpoint omissions, {filtered} test/exclusion-filtered endpoints, {stale} stale/unverified endpoints hidden ({source_budget_omissions} source-budget omissions).]\n",self.unavailable_sources,self.omitted_endpoints));
        }
        if was_capped {
            out.push_str(&format!("\n[Event output cap reached: {} eligible endpoint(s) not rendered. Narrow event_key, source path or workspace scope.]\n",total.min(super::ENDPOINTS_PER_QUERY).saturating_sub(rendered_endpoints)));
        }
        if out.len() > cap {
            return "[Event output budget unavailable; narrow the source request.]\n"
                .chars()
                .take(cap)
                .collect();
        }
        out
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
