use super::*;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

fn overlaps(location: &Location, anchors: &[(String, usize, usize)]) -> bool {
    anchors.iter().any(|(path, start, end)| {
        path == &location.path
            && location.range.start_line <= *end
            && *start <= location.range.end_line_inclusive()
    })
}

struct Eligibility<'a> {
    index: &'a ImplementationIndex,
    root: &'a Path,
    scope: Option<&'a str>,
    filter: crate::callers::test_code::TestCodeFilter,
    ignore_probe: crate::workspace::IgnoreProbe,
    fresh: HashMap<String, bool>,
    bytes: usize,
}
impl Eligibility<'_> {
    fn in_scope(&self, path: &str) -> bool {
        !self.scope.is_some_and(|scope| {
            path != scope
                && !path
                    .strip_prefix(scope)
                    .is_some_and(|tail| tail.starts_with('/'))
        })
    }
    fn is_valid_evidence(&mut self, location: &Location) -> bool {
        let fresh = if let Some(fresh) = self.fresh.get(&location.path) {
            *fresh
        } else {
            let path = self.root.join(&location.path);
            let is_fresh = self.fresh.len() < 1024
                && self
                    .index
                    .is_digest_current_on_disk(
                        self.root,
                        &location.path,
                        Some(8 * 1024 * 1024),
                        Some((&mut self.bytes, 64 * 1024 * 1024)),
                    )
                    .unwrap_or(false)
                && if path.parent().is_none_or(|p| p.as_os_str().is_empty()) {
                    crate::workspace::walk_root_is_visible(&path, true)
                } else {
                    !self.ignore_probe.is_ignored(&path)
                };
            self.fresh.insert(location.path.clone(), is_fresh);
            is_fresh
        };
        fresh && !self.filter.is_excluded(&location.path, &location.range)
    }
    fn allows(&mut self, location: &Location) -> bool {
        self.in_scope(&location.path) && self.is_valid_evidence(location)
    }
    fn method(&mut self, id: usize) -> bool {
        let method = &self.index.methods[id];
        self.allows(&method.location) && self.is_valid_evidence(&self.index.declaring(id))
    }
    fn link(&mut self, link: &Link) -> bool {
        self.method(link.declaration)
            && self.method(link.implementation)
            && link
                .evidence
                .iter()
                .all(|location| self.is_valid_evidence(location))
    }
    fn explicit_impl(&mut self, item: &ExplicitImpl) -> bool {
        self.allows(&item.location)
            && self.is_valid_evidence(&self.index.location(item.contract))
            && item
                .evidence
                .iter()
                .all(|location| self.is_valid_evidence(location))
    }
    fn call(&mut self, call: &Call) -> bool {
        self.allows(&call.location)
            && self.method(call.declaration)
            && call
                .evidence
                .iter()
                .all(|location| self.is_valid_evidence(location))
    }
}

pub(crate) const SOURCE_TRAILER_HEADER: &str =
    "[Implementations and overrides outside this window (static analysis)]";

fn link_tags(link: &Link) -> String {
    let mut tags = String::new();
    if link.is_pointer {
        tags.push_str(" [pointer receiver; *T method set]");
    }
    if !link.conditions.is_empty() {
        tags.push_str(" [conditional: ");
        tags.push_str(&link.conditions.join(" && "));
        tags.push(']');
    }
    tags
}

impl ImplementationIndex {
    fn display_method(&self, id: usize, current_file: Option<&str>) -> String {
        let method = &self.methods[id];
        format!(
            "{}.{} — {}",
            self.typ(method.owner).qualified_name,
            method.declaration.name,
            crate::locations::display(
                &method.location.path,
                method.location.range.start_line,
                current_file
            )
        )
    }

    /// A row saying how many eligible methods look like implementations of `declaration` and
    /// are not listed, since the listed candidates then cover only the proven ones.
    fn unproven_note(&self, declaration: usize, eligibility: &mut Eligibility) -> Option<String> {
        let count = self
            .unproven
            .get(&declaration)?
            .iter()
            .filter(|method| eligibility.method(**method))
            .count();
        (count > 0).then(|| {
            format!(
                "- not source-proven: {count} same-named method(s) in types naming {} as a base; the candidates above may be incomplete.\n",
                self.typ(self.methods[declaration].owner).name
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn for_paths(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
    ) -> String {
        self.for_paths_with_call_context(anchors, scope, cap, root, true)
    }

    #[cfg(test)]
    pub(crate) fn for_paths_with_call_context(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
        should_include_calls: bool,
    ) -> String {
        self.for_paths_with_context(anchors, scope, cap, root, should_include_calls, None)
    }

    pub(crate) fn for_paths_with_context(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
        should_include_calls: bool,
        current_file: Option<&str>,
    ) -> String {
        if anchors.is_empty() || self.target_os != crate::config::get().analysis_target_os {
            return String::new();
        }
        let mut eligibility = Eligibility {
            index: self,
            root,
            scope,
            filter: crate::callers::test_code::TestCodeFilter::from_config(root),
            ignore_probe: crate::workspace::IgnoreProbe::default(),
            fresh: HashMap::new(),
            bytes: 0,
        };
        let mut methods = BTreeSet::new();
        let mut calls = BTreeSet::new();
        let mut explicit_contracts = BTreeSet::new();
        let mut explicit_anchors = BTreeSet::new();
        let mut inspected = 0;
        let mut capped = false;
        let paths = anchors
            .iter()
            .map(|(path, _, _)| path)
            .collect::<BTreeSet<_>>();
        for path in paths {
            for &id in self.methods_by_path.get(path).into_iter().flatten() {
                let method = &self.methods[id];
                let has_explicit = self.typ(method.owner).is_contract
                    && self.explicit_impls_by_contract.contains_key(&method.owner);
                if !method.declaration.is_abstract
                    && !self.forward.contains_key(&id)
                    && !self.reverse.contains_key(&id)
                    && !has_explicit
                {
                    continue;
                }
                let type_location = self.declaring(id);
                let header = Location {
                    path: type_location.path,
                    range: crate::parser::CodeRange {
                        end_line: type_location.range.start_line,
                        end_col: type_location.range.start_col + 1,
                        ..type_location.range
                    },
                };
                if overlaps(&method.location, anchors) || overlaps(&header, anchors) {
                    if inspected >= CANDIDATES_PER_QUERY {
                        capped = true;
                        break;
                    }
                    inspected += 1;
                    if !eligibility.method(id) {
                        continue;
                    }
                    methods.insert(id);
                }
            }
            for &id in self
                .calls_by_path
                .get(path)
                .into_iter()
                .flatten()
                .filter(|_| should_include_calls)
            {
                if !overlaps(&self.calls[id].location, anchors) {
                    continue;
                }
                if inspected >= CANDIDATES_PER_QUERY {
                    capped = true;
                    break;
                }
                inspected += 1;
                if eligibility.call(&self.calls[id]) {
                    calls.insert(id);
                }
            }
            for &id in self.explicit_impls_by_path.get(path).into_iter().flatten() {
                if !overlaps(&self.explicit_impls[id].location, anchors) {
                    continue;
                }
                if inspected >= CANDIDATES_PER_QUERY {
                    capped = true;
                    break;
                }
                inspected += 1;
                if eligibility.explicit_impl(&self.explicit_impls[id]) {
                    explicit_anchors.insert(id);
                }
            }
            // Marker traits (traits with 0 methods) that have external explicit impl blocks.
            for &contract in self.explicit_impls_by_contract.keys() {
                if self.types[contract].methods.is_empty()
                    && self.files[self.types[contract].file].file_path == *path
                {
                    let loc = self.location(contract);
                    if overlaps(&loc, anchors) && eligibility.allows(&loc) {
                        explicit_contracts.insert(contract);
                    }
                }
            }
        }
        if methods.is_empty()
            && calls.is_empty()
            && explicit_contracts.is_empty()
            && explicit_anchors.is_empty()
        {
            return String::new();
        }
        let mut sections = Vec::new();
        let mut rows = 0;
        for id in methods {
            let method = &self.methods[id];
            let mut section = format!("\n### {}", self.display_method(id, current_file));
            if method.declaration.is_abstract {
                section.push(' ');
                section.push_str(ABSTRACT_DECLARATION_LABEL);
            }
            section.push('\n');
            let mut related = false;
            for &link_id in self.forward.get(&id).into_iter().flatten() {
                let link = &self.links[link_id];
                if !eligibility.link(link) {
                    continue;
                }
                if rows >= OUTPUT_ROWS_PER_QUERY {
                    capped = true;
                    break;
                }
                rows += 1;
                related = true;
                section.push_str(&format!(
                    "- implementation candidate: {}{}\n",
                    self.display_method(link.implementation, current_file),
                    link_tags(link)
                ));
            }
            if self.typ(method.owner).is_contract {
                for &impl_id in self
                    .explicit_impls_by_contract
                    .get(&method.owner)
                    .into_iter()
                    .flatten()
                {
                    let item = &self.explicit_impls[impl_id];
                    if !eligibility.explicit_impl(item) {
                        continue;
                    }
                    if rows >= OUTPUT_ROWS_PER_QUERY {
                        capped = true;
                        break;
                    }
                    rows += 1;
                    related = true;
                    section.push_str(&format!(
                        "- explicit impl block: impl {} for {} — {}\n",
                        self.typ(item.contract).qualified_name,
                        item.owner_display,
                        crate::locations::display(
                            &item.location.path,
                            item.location.range.start_line,
                            current_file
                        )
                    ));
                }
            }
            if related {
                if let Some(note) = self.unproven_note(id, &mut eligibility) {
                    rows += 1;
                    section.push_str(&note);
                }
            }
            for &link_id in self.reverse.get(&id).into_iter().flatten() {
                let link = &self.links[link_id];
                if !eligibility.link(link) {
                    continue;
                }
                if rows >= OUTPUT_ROWS_PER_QUERY {
                    capped = true;
                    break;
                }
                rows += 1;
                related = true;
                section.push_str(&format!(
                    "- implements/overrides declaration: {}{}\n",
                    self.display_method(link.declaration, current_file),
                    link_tags(link)
                ));
            }
            if !related && method.declaration.is_abstract {
                section.push_str("- implementation: unresolved (no eligible source-proven candidate in this snapshot; imports, signatures or conditional/generic constraints may be unresolved).\n");
                related = true;
            }
            for &call_id in self
                .references
                .get(&id)
                .into_iter()
                .flatten()
                .filter(|_| should_include_calls)
            {
                let call = &self.calls[call_id];
                if !eligibility.call(call) {
                    continue;
                }
                if rows >= OUTPUT_ROWS_PER_QUERY {
                    capped = true;
                    break;
                }
                rows += 1;
                related = true;
                section.push_str(&format!(
                    "- declaration reference: {} — {}\n",
                    call.name,
                    crate::locations::display(
                        &call.location.path,
                        call.location.range.start_line,
                        current_file
                    )
                ));
            }
            if related {
                sections.push(section);
            }
        }
        for contract in explicit_contracts {
            let loc = self.location(contract);
            let mut section = format!(
                "\n### {} — {}\n",
                self.typ(contract).qualified_name,
                crate::locations::display(&loc.path, loc.range.start_line, current_file)
            );
            let mut related = false;
            for &impl_id in self
                .explicit_impls_by_contract
                .get(&contract)
                .into_iter()
                .flatten()
            {
                let item = &self.explicit_impls[impl_id];
                if !eligibility.explicit_impl(item) {
                    continue;
                }
                if rows >= OUTPUT_ROWS_PER_QUERY {
                    capped = true;
                    break;
                }
                rows += 1;
                related = true;
                section.push_str(&format!(
                    "- explicit impl block: impl {} for {} — {}\n",
                    self.typ(item.contract).qualified_name,
                    item.owner_display,
                    crate::locations::display(
                        &item.location.path,
                        item.location.range.start_line,
                        current_file
                    )
                ));
            }
            if related {
                sections.push(section);
            }
        }
        for impl_id in explicit_anchors {
            let item = &self.explicit_impls[impl_id];
            if rows >= OUTPUT_ROWS_PER_QUERY {
                capped = true;
                break;
            }
            rows += 1;
            let contract_loc = self.location(item.contract);
            sections.push(format!(
                "\n### impl {} for {} — {}\n- explicit impl block: implements contract {} — {}\n",
                self.typ(item.contract).qualified_name,
                item.owner_display,
                crate::locations::display(
                    &item.location.path,
                    item.location.range.start_line,
                    current_file
                ),
                self.typ(item.contract).qualified_name,
                crate::locations::display(
                    &contract_loc.path,
                    contract_loc.range.start_line,
                    current_file
                )
            ));
        }
        for id in calls {
            let call = &self.calls[id];
            let mut section = format!(
                "\n### {} — {}\n- call declaration: {}\n",
                call.name,
                crate::locations::display(
                    &call.location.path,
                    call.location.range.start_line,
                    current_file
                ),
                self.display_method(call.declaration, current_file)
            );
            let mut listed = false;
            for &link_id in self.forward.get(&call.declaration).into_iter().flatten() {
                let link = &self.links[link_id];
                if !eligibility.link(link) {
                    continue;
                }
                if rows >= OUTPUT_ROWS_PER_QUERY {
                    capped = true;
                    break;
                }
                rows += 1;
                listed = true;
                section.push_str(&format!(
                    "- implementation candidate: {}{}\n",
                    self.display_method(link.implementation, current_file),
                    link_tags(link)
                ));
            }
            if listed {
                if let Some(note) = self.unproven_note(call.declaration, &mut eligibility) {
                    rows += 1;
                    section.push_str(&note);
                }
            }
            section.push_str(
                "- runtime target: unresolved (receiver instance is not statically proven).\n",
            );
            sections.push(section);
        }
        if sections.is_empty() {
            return String::new();
        }
        let header="## Implementations\n\n_Source-proven declaration relationships. Candidates are not guaranteed runtime targets; only eligible indexed source is considered._\n";
        let note = "\n[Implementation context truncated; narrow the source window or workspace.]\n";
        if cap < header.len() + note.len() {
            return String::new();
        }
        let mut output = header.to_string();
        'sections: for section in sections {
            for line in section.split_inclusive('\n') {
                if output.len() + line.len() + note.len() > cap {
                    capped = true;
                    break 'sections;
                }
                output.push_str(line);
            }
        }
        if capped || self.omitted > 0 {
            output.push_str(note);
        }
        output
    }

    fn enclosing_caller_label(&self, path: &str, line: usize) -> Option<String> {
        let by_path = self.files_by_path.get_or_init(|| {
            self.files
                .iter()
                .enumerate()
                .map(|(idx, file)| (file.file_path.clone(), idx))
                .collect()
        });
        let file = self.files.get(*by_path.get(path)?)?;
        file.symbols
            .iter()
            .filter(|symbol| {
                matches!(
                    symbol.kind.as_str(),
                    "fn" | "method" | "function" | "constructor" | "init" | "def"
                ) && symbol.range.start_line <= line
                    && line <= symbol.range.end_line_inclusive()
            })
            .min_by_key(|symbol| symbol.range.end_line_inclusive() - symbol.range.start_line)
            .map(|symbol| {
                let name = symbol.name.split_whitespace().collect::<Vec<_>>().join(" ");
                match &symbol.owner {
                    Some(owner) => format!("{owner}.{name}"),
                    None => name,
                }
            })
    }

    /// Compact 1-line-per-declaration trailer for `read view=source` listing source-proven
    /// implementations, overrides, interface-dispatched callers, and Rust explicit impl blocks
    /// outside `(path, start..=end)`.
    pub(crate) fn source_trailer(
        &self,
        path: &str,
        start: usize,
        end: usize,
        root: &Path,
        byte_cap: usize,
    ) -> Option<String> {
        const ITEMS_PER_ROW: usize = 4;
        if byte_cap < SOURCE_TRAILER_HEADER.len() + 32
            || self.target_os != crate::config::get().analysis_target_os
        {
            return None;
        }
        let mut eligibility = Eligibility {
            index: self,
            root,
            scope: None,
            filter: crate::callers::test_code::TestCodeFilter::from_config(root),
            ignore_probe: crate::workspace::IgnoreProbe::default(),
            fresh: HashMap::new(),
            bytes: 0,
        };
        let anchors = [(path.to_string(), start, end)];
        let is_outside = |loc: &Location| {
            loc.path != path || loc.range.end_line_inclusive() < start || loc.range.start_line > end
        };
        let mut lines: Vec<String> = Vec::new();
        let mut matched_methods: Vec<usize> = self
            .methods_by_path
            .get(path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&id| {
                let method = &self.methods[id];
                let has_explicit = self.typ(method.owner).is_contract
                    && self.explicit_impls_by_contract.contains_key(&method.owner);
                if !self.forward.contains_key(&id)
                    && !self.reverse.contains_key(&id)
                    && !self.references.contains_key(&id)
                    && !has_explicit
                {
                    return false;
                }
                let type_loc = self.declaring(id);
                let header = Location {
                    path: type_loc.path,
                    range: crate::parser::CodeRange {
                        end_line: type_loc.range.start_line,
                        end_col: type_loc.range.start_col + 1,
                        ..type_loc.range
                    },
                };
                overlaps(&method.location, &anchors) || overlaps(&header, &anchors)
            })
            .collect();
        matched_methods.sort_by_key(|&id| {
            (
                self.methods[id].location.range.start_line,
                self.methods[id].location.range.start_col,
                id,
            )
        });
        matched_methods.dedup();
        let mut emitted_explicit_contracts = std::collections::HashSet::new();
        let mut emitted_forward_decls = std::collections::HashSet::new();
        for id in matched_methods {
            if !eligibility.method(id) {
                continue;
            }
            let method = &self.methods[id];
            let label = format!(
                "{}.{}",
                self.typ(method.owner).qualified_name,
                method.declaration.name
            );
            let mut forward_items = Vec::new();
            for &link_id in self.forward.get(&id).into_iter().flatten() {
                let link = &self.links[link_id];
                if is_outside(&self.methods[link.implementation].location) && eligibility.link(link)
                {
                    let target = &self.methods[link.implementation];
                    let loc = crate::locations::display(
                        &target.location.path,
                        target.location.range.start_line,
                        Some(path),
                    );
                    forward_items.push(format!(
                        "{}.{} ({loc}){}",
                        self.typ(target.owner).qualified_name,
                        target.declaration.name,
                        link_tags(link)
                    ));
                }
            }
            if !forward_items.is_empty() {
                emitted_forward_decls.insert(id);
            }
            if !forward_items.is_empty() {
                let total = forward_items.len();
                let shown = forward_items
                    .iter()
                    .take(ITEMS_PER_ROW)
                    .cloned()
                    .collect::<Vec<_>>();
                let suffix = if total > ITEMS_PER_ROW {
                    format!(", +{} more", total - ITEMS_PER_ROW)
                } else {
                    String::new()
                };
                lines.push(format!(
                    "- {label} implemented by: {}{suffix}",
                    shown.join(", ")
                ));
            }
            let mut reverse_items = Vec::new();
            for &link_id in self.reverse.get(&id).into_iter().flatten() {
                let link = &self.links[link_id];
                if is_outside(&self.methods[link.declaration].location) && eligibility.link(link) {
                    let target = &self.methods[link.declaration];
                    let loc = crate::locations::display(
                        &target.location.path,
                        target.location.range.start_line,
                        Some(path),
                    );
                    reverse_items.push(format!(
                        "{}.{} ({loc}){}",
                        self.typ(target.owner).qualified_name,
                        target.declaration.name,
                        link_tags(link)
                    ));
                }
            }
            if !reverse_items.is_empty() {
                let total = reverse_items.len();
                let shown = reverse_items.iter().take(2).cloned().collect::<Vec<_>>();
                let suffix = if total > 2 {
                    format!(", +{} more", total - 2)
                } else {
                    String::new()
                };
                lines.push(format!("- {label} overrides: {}{suffix}", shown.join(", ")));
            }
            // Indirect callers dispatching through implemented contracts/interfaces/traits.
            let mut dispatched_callers: Vec<String> = Vec::new();
            let mut seen_dispatched: std::collections::HashSet<(String, usize)> =
                std::collections::HashSet::new();
            for &link_id in self.reverse.get(&id).into_iter().flatten() {
                let link = &self.links[link_id];
                if !eligibility.link(link) {
                    continue;
                }
                let contract_method = &self.methods[link.declaration];
                let via_label = format!(
                    "{}.{}",
                    self.typ(contract_method.owner).qualified_name,
                    contract_method.declaration.name
                );
                for &call_id in self.references.get(&link.declaration).into_iter().flatten() {
                    let call = &self.calls[call_id];
                    if !is_outside(&call.location) || !eligibility.call(call) {
                        continue;
                    }
                    let key = (call.location.path.clone(), call.location.range.start_line);
                    if !seen_dispatched.insert(key) {
                        continue;
                    }
                    let loc = crate::locations::display(
                        &call.location.path,
                        call.location.range.start_line,
                        Some(path),
                    );
                    let entry = match self
                        .enclosing_caller_label(&call.location.path, call.location.range.start_line)
                    {
                        Some(caller) => format!("{caller} ({loc} via {via_label})"),
                        None => format!("{loc} (via {via_label})"),
                    };
                    dispatched_callers.push(entry);
                }
            }
            if !dispatched_callers.is_empty() {
                let total = dispatched_callers.len();
                let shown = dispatched_callers
                    .iter()
                    .take(ITEMS_PER_ROW)
                    .cloned()
                    .collect::<Vec<_>>();
                let suffix = if total > ITEMS_PER_ROW {
                    format!(", +{} more", total - ITEMS_PER_ROW)
                } else {
                    String::new()
                };
                lines.push(format!(
                    "- {label} called via contract: {}{suffix}",
                    shown.join(", ")
                ));
            }
            // Direct callers invoking this contract/interface/abstract method.
            let mut contract_callers: Vec<String> = Vec::new();
            let mut seen_contract_calls: std::collections::HashSet<(String, usize)> =
                std::collections::HashSet::new();
            for &call_id in self.references.get(&id).into_iter().flatten() {
                let call = &self.calls[call_id];
                if !is_outside(&call.location) || !eligibility.call(call) {
                    continue;
                }
                let key = (call.location.path.clone(), call.location.range.start_line);
                if !seen_contract_calls.insert(key) {
                    continue;
                }
                let loc = crate::locations::display(
                    &call.location.path,
                    call.location.range.start_line,
                    Some(path),
                );
                let entry = match self
                    .enclosing_caller_label(&call.location.path, call.location.range.start_line)
                {
                    Some(caller) => format!("{caller} ({loc})"),
                    None => loc,
                };
                contract_callers.push(entry);
            }
            if !contract_callers.is_empty() {
                let total = contract_callers.len();
                let shown = contract_callers
                    .iter()
                    .take(ITEMS_PER_ROW)
                    .cloned()
                    .collect::<Vec<_>>();
                let suffix = if total > ITEMS_PER_ROW {
                    format!(", +{} more", total - ITEMS_PER_ROW)
                } else {
                    String::new()
                };
                lines.push(format!("- {label} called at: {}{suffix}", shown.join(", ")));
            }
            if self.typ(method.owner).is_contract && emitted_explicit_contracts.insert(method.owner)
            {
                let mut explicit_items = Vec::new();
                for &impl_id in self
                    .explicit_impls_by_contract
                    .get(&method.owner)
                    .into_iter()
                    .flatten()
                {
                    let item = &self.explicit_impls[impl_id];
                    if is_outside(&item.location) && eligibility.explicit_impl(item) {
                        let loc = crate::locations::display(
                            &item.location.path,
                            item.location.range.start_line,
                            Some(path),
                        );
                        explicit_items.push(format!("for {} ({loc})", item.owner_display));
                    }
                }
                if !explicit_items.is_empty() {
                    let total = explicit_items.len();
                    let shown = explicit_items
                        .iter()
                        .take(ITEMS_PER_ROW)
                        .cloned()
                        .collect::<Vec<_>>();
                    let suffix = if total > ITEMS_PER_ROW {
                        format!(", +{} more", total - ITEMS_PER_ROW)
                    } else {
                        String::new()
                    };
                    lines.push(format!(
                        "- {} explicit impls: {}{suffix}",
                        self.typ(method.owner).qualified_name,
                        shown.join(", ")
                    ));
                }
            }
        }
        // Marker traits (0 methods) overlapping this window.
        let mut marker_contracts: Vec<usize> = self
            .explicit_impls_by_contract
            .keys()
            .copied()
            .filter(|&contract| {
                self.types[contract].methods.is_empty()
                    && !emitted_explicit_contracts.contains(&contract)
                    && self.files[self.types[contract].file].file_path == path
                    && overlaps(&self.location(contract), &anchors)
            })
            .collect();
        marker_contracts.sort_by_key(|&c| self.typ(c).range.start_line);
        for contract in marker_contracts {
            if !eligibility.allows(&self.location(contract)) {
                continue;
            }
            let mut explicit_items = Vec::new();
            for &impl_id in self
                .explicit_impls_by_contract
                .get(&contract)
                .into_iter()
                .flatten()
            {
                let item = &self.explicit_impls[impl_id];
                if is_outside(&item.location) && eligibility.explicit_impl(item) {
                    let loc = crate::locations::display(
                        &item.location.path,
                        item.location.range.start_line,
                        Some(path),
                    );
                    explicit_items.push(format!("for {} ({loc})", item.owner_display));
                }
            }
            if !explicit_items.is_empty() {
                let total = explicit_items.len();
                let shown = explicit_items
                    .iter()
                    .take(ITEMS_PER_ROW)
                    .cloned()
                    .collect::<Vec<_>>();
                let suffix = if total > ITEMS_PER_ROW {
                    format!(", +{} more", total - ITEMS_PER_ROW)
                } else {
                    String::new()
                };
                lines.push(format!(
                    "- {} explicit impls: {}{suffix}",
                    self.typ(contract).qualified_name,
                    shown.join(", ")
                ));
            }
        }
        // Polymorphic/interface/virtual call sites inside this window whose concrete
        // implementations live outside the window.
        let mut window_calls: Vec<usize> = self
            .calls_by_path
            .get(path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&call_id| overlaps(&self.calls[call_id].location, &anchors))
            .collect();
        window_calls.sort_by_key(|&call_id| {
            let loc = &self.calls[call_id].location.range;
            (loc.start_line, loc.start_col, call_id)
        });
        let mut seen_call_decls = std::collections::HashSet::new();
        for call_id in window_calls {
            let call = &self.calls[call_id];
            if !eligibility.call(call)
                || emitted_forward_decls.contains(&call.declaration)
                || !seen_call_decls.insert(call.declaration)
            {
                continue;
            }
            let decl = &self.methods[call.declaration];
            let mut impl_items = Vec::new();
            for &link_id in self.forward.get(&call.declaration).into_iter().flatten() {
                let link = &self.links[link_id];
                if is_outside(&self.methods[link.implementation].location) && eligibility.link(link)
                {
                    let target = &self.methods[link.implementation];
                    let loc = crate::locations::display(
                        &target.location.path,
                        target.location.range.start_line,
                        Some(path),
                    );
                    impl_items.push(format!(
                        "{}.{} ({loc}){}",
                        self.typ(target.owner).qualified_name,
                        target.declaration.name,
                        link_tags(link)
                    ));
                }
            }
            if impl_items.is_empty() {
                continue;
            }
            let decl_label = format!(
                "{}.{}",
                self.typ(decl.owner).qualified_name,
                decl.declaration.name
            );
            let decl_loc = if is_outside(&decl.location) {
                format!(
                    " ({})",
                    crate::locations::display(
                        &decl.location.path,
                        decl.location.range.start_line,
                        Some(path),
                    )
                )
            } else {
                String::new()
            };
            let total = impl_items.len();
            let shown = impl_items
                .iter()
                .take(ITEMS_PER_ROW)
                .cloned()
                .collect::<Vec<_>>();
            let suffix = if total > ITEMS_PER_ROW {
                format!(", +{} more", total - ITEMS_PER_ROW)
            } else {
                String::new()
            };
            lines.push(format!(
                "- {decl_label}{decl_loc} implemented by: {}{suffix}",
                shown.join(", ")
            ));
        }
        if lines.is_empty() {
            return None;
        }
        let mut out = String::from(SOURCE_TRAILER_HEADER);
        let mut emitted = 0usize;
        let total_lines = lines.len();
        for (idx, line) in lines.iter().enumerate() {
            let remaining = total_lines - idx;
            let more_note = format!("\n- +{remaining} more declaration(s) (view=relations)");
            let reserve = if idx + 1 < total_lines {
                more_note.len()
            } else {
                0
            };
            if out.len() + 1 + line.len() + reserve > byte_cap {
                if emitted == 0 {
                    return None;
                }
                let omit = format!("\n- +{remaining} more declaration(s) (view=relations)");
                if out.len() + omit.len() <= byte_cap {
                    out.push_str(&omit);
                }
                break;
            }
            out.push('\n');
            out.push_str(line);
            emitted += 1;
        }
        (emitted > 0).then_some(out)
    }
}
