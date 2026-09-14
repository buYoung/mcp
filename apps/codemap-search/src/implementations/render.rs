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
    fresh: HashMap<String, bool>,
    bytes: usize,
}
impl Eligibility<'_> {
    fn allows(&mut self, location: &Location) -> bool {
        if self.scope.is_some_and(|scope| {
            location.path != scope
                && !location
                    .path
                    .strip_prefix(scope)
                    .is_some_and(|tail| tail.starts_with('/'))
        }) {
            return false;
        }
        let fresh = if let Some(fresh) = self.fresh.get(&location.path) {
            *fresh
        } else {
            let path = self.root.join(&location.path);
            let is_fresh = self.fresh.len() < 1024
                && crate::workspace::walk_root_is_visible(&path, true)
                && self
                    .index
                    .proof_digests
                    .get(&location.path)
                    .is_some_and(|expected| {
                        std::fs::metadata(&path).is_ok_and(|metadata| {
                            let size = metadata.len();
                            if size > 8 * 1024 * 1024
                                || self.bytes.saturating_add(size as usize) > 64 * 1024 * 1024
                            {
                                return false;
                            }
                            self.bytes += size as usize;
                            std::fs::read(&path).is_ok_and(|bytes| digest(&bytes) == *expected)
                        })
                    });
            self.fresh.insert(location.path.clone(), is_fresh);
            is_fresh
        };
        fresh && !self.filter.is_excluded(&location.path, &location.range)
    }
    fn method(&mut self, id: usize) -> bool {
        let method = &self.index.methods[id];
        self.allows(&method.location) && self.allows(&self.index.location(method.owner))
    }
    fn link(&mut self, link: &Link) -> bool {
        self.method(link.declaration)
            && self.method(link.implementation)
            && link.evidence.iter().all(|location| self.allows(location))
    }
    fn call(&mut self, call: &Call) -> bool {
        self.allows(&call.location)
            && self.method(call.declaration)
            && call.evidence.iter().all(|location| self.allows(location))
    }
}

impl ImplementationIndex {
    fn display_method(&self, id: usize) -> String {
        let method = &self.methods[id];
        format!(
            "{}.{} — {}:{}",
            self.typ(method.owner).qualified_name,
            method.declaration.name,
            method.location.path,
            method.location.range.start_line
        )
    }

    pub(crate) fn for_paths(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
    ) -> String {
        self.for_paths_with_call_context(anchors, scope, cap, root, true)
    }

    pub(crate) fn for_paths_with_call_context(
        &self,
        anchors: &[(String, usize, usize)],
        scope: Option<&str>,
        cap: usize,
        root: &Path,
        should_include_calls: bool,
    ) -> String {
        if anchors.is_empty() || self.target_os != crate::config::get().analysis_target_os {
            return String::new();
        }
        let mut eligibility = Eligibility {
            index: self,
            root,
            scope,
            filter: crate::callers::test_code::TestCodeFilter::from_config(root),
            fresh: HashMap::new(),
            bytes: 0,
        };
        let mut methods = BTreeSet::new();
        let mut calls = BTreeSet::new();
        let mut inspected = 0;
        let mut capped = false;
        let paths = anchors
            .iter()
            .map(|(path, _, _)| path)
            .collect::<BTreeSet<_>>();
        for path in paths {
            for &id in self.methods_by_path.get(path).into_iter().flatten() {
                let method = &self.methods[id];
                if !method.declaration.is_abstract
                    && !self.forward.contains_key(&id)
                    && !self.reverse.contains_key(&id)
                {
                    continue;
                }
                let type_location = self.location(method.owner);
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
        }
        if methods.is_empty() && calls.is_empty() {
            return String::new();
        }
        let mut sections = Vec::new();
        let mut rows = 0;
        for id in methods {
            let method = &self.methods[id];
            let mut section = format!(
                "\n### {}{}\n",
                self.display_method(id),
                if method.declaration.is_abstract {
                    " [abstract declaration]"
                } else {
                    ""
                }
            );
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
                    self.display_method(link.implementation),
                    if link.is_pointer {
                        " [pointer receiver; *T method set]"
                    } else {
                        ""
                    }
                ));
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
                    "- implements/overrides declaration: {}\n",
                    self.display_method(link.declaration)
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
                    "- declaration reference: {} — {}:{}\n",
                    call.name, call.location.path, call.location.range.start_line
                ));
            }
            if related {
                sections.push(section);
            }
        }
        for id in calls {
            let call = &self.calls[id];
            let mut section = format!(
                "\n### {} — {}:{}\n- call declaration: {}\n",
                call.name,
                call.location.path,
                call.location.range.start_line,
                self.display_method(call.declaration)
            );
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
                section.push_str(&format!(
                    "- implementation candidate: {}{}\n",
                    self.display_method(link.implementation),
                    if link.is_pointer {
                        " [pointer receiver; *T method set]"
                    } else {
                        ""
                    }
                ));
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
}
