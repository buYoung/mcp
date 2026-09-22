//! Keep ranked snippet selection separate from its presentation order.
use crate::declarations;
use crate::parser::{ExtractedFile, ExtractedSymbol};
use std::collections::BTreeSet;

pub(super) struct Section {
    root: Option<ExtractedSymbol>,
    heading: String,
    text: String,
    shown: BTreeSet<(String, usize, usize)>,
    pub anchors: Vec<(String, usize, usize)>,
    pub insertion: usize,
}

pub(super) struct FileOutput {
    pub path: String,
    header: String,
    metadata: String,
    sections: Vec<Section>,
    current: Option<usize>,
    current_depth: usize,
    results: String,
    indexed: Option<ExtractedFile>,
    has_impl_scopes: bool,
    base_bytes: usize,
    cap: usize,
    pub budget_hit: bool,
    pub source_span: Option<std::ops::Range<usize>>,
    first_result_source_offset: Option<usize>,
    pub first_source_byte: Option<usize>,
    pub bodies: Vec<super::jev::BodyEvidence>,
    pub source_segments: Vec<super::jev::SourceSegment>,
    is_current_source: bool,
    has_checked_source: bool,
}

impl FileOutput {
    pub fn new(
        path: &str,
        number: usize,
        indexed: Option<&ExtractedFile>,
        base_bytes: usize,
        cap: usize,
    ) -> Self {
        Self {
            path: path.into(),
            header: format!("\n## {number}. {path}\n\n"),
            metadata: String::new(),
            sections: Vec::new(),
            current: None,
            current_depth: 0,
            results: String::new(),
            indexed: indexed.cloned(),
            has_impl_scopes: false,
            base_bytes,
            cap,
            budget_hit: false,
            source_span: None,
            first_result_source_offset: None,
            first_source_byte: None,
            bodies: Vec::new(),
            source_segments: Vec::new(),
            is_current_source: false,
            has_checked_source: false,
        }
    }
    pub fn len(&self) -> usize {
        self.base_bytes
            + self.header.len()
            + self.metadata.len()
            + self.results.len()
            + "\n### results\n".len()
            + self
                .sections
                .iter()
                .map(|section| section.heading.len() + section.text.len())
                .sum::<usize>()
    }
    pub fn can_fit(&self, extra: usize) -> bool {
        self.len()
            .saturating_add(extra)
            .saturating_add(super::search_cap_footer(self.cap).len())
            <= self.cap
    }
    pub fn remaining_bytes(&self) -> usize {
        self.cap
            .saturating_sub(self.len() + super::search_cap_footer(self.cap).len())
    }
    fn fits(&mut self, extra: usize) -> bool {
        let fits = self.can_fit(extra);
        self.budget_hit |= !fits;
        fits
    }
    pub fn push_str(&mut self, text: &str) {
        if !self.fits(text.len()) {
            return;
        }
        if let Some(index) = self.current {
            self.sections[index].text.push_str(text);
        } else {
            self.metadata.push_str(text);
        }
    }
    pub fn push_source(&mut self, text: &str, source_offset: Option<usize>) -> bool {
        if !self.fits(text.len()) {
            return false;
        }
        if self.first_result_source_offset.is_none() {
            self.first_result_source_offset = source_offset
                .filter(|offset| *offset < text.len())
                .map(|offset| self.results.len() + offset);
        }
        if let Some(offset) = source_offset.filter(|offset| *offset < text.len()) {
            self.source_segments.push(super::jev::SourceSegment { path: self.path.clone(), span: self.results.len()..self.results.len()+text.len(), first_source_byte: self.results.len()+offset, body_index: None });
        }
        self.results.push_str(text);
        true
    }
    pub fn push_body(&mut self, symbol: &ExtractedSymbol, text: &str, source_offset: usize, displayed_body: &str, is_complete: bool) -> bool {
        let start = self.results.len();
        if !self.push_source(text, Some(source_offset)) { return false; }
        if let Some(index) = self.bodies.iter().position(|body| body.symbol.name == symbol.name && body.symbol.range == symbol.range && body.symbol.kind == symbol.kind && body.symbol.owner == symbol.owner) {
            let body=&mut self.bodies[index];
            body.span=Some(start..self.results.len());
            body.displayed_body=displayed_body.to_string();
            body.completeness=if displayed_body.len()>super::jev::MAX_BODY_BYTES { super::jev::Completeness::Oversized }
                else if is_complete && self.is_current_source { super::jev::Completeness::Complete }
                else { super::jev::Completeness::Partial };
            if let Some(segment)=self.source_segments.last_mut() { segment.body_index=Some(index); }
        }
        true
    }
    pub fn body_context(&mut self, symbol: &ExtractedSymbol, context: &str) {
        if let Some(body)=self.bodies.iter_mut().find(|body|body.symbol.name==symbol.name && body.symbol.range==symbol.range && body.symbol.kind==symbol.kind && body.symbol.owner==symbol.owner) {
            body.displayed_context.push_str(context);
        }
    }
    pub fn push_annotation(&mut self, text: &str) -> bool {
        let Some(index) = self.current else {
            return false;
        };
        let prefix = "  ".repeat(self.current_depth);
        let text = text
            .split_inclusive('\n')
            .map(|line| {
                if line.trim().is_empty() {
                    line.into()
                } else {
                    format!("{prefix}{line}")
                }
            })
            .collect::<String>();
        if !self.can_fit(text.len()) {
            return false;
        }
        self.sections[index].text.push_str(&text);
        true
    }
    pub fn start_symbol(&mut self, symbol: &ExtractedSymbol, source: Option<&str>) -> bool {
        let owned_source = (source.is_none()
            && !self.has_impl_scopes
            && symbol.owner.is_some()
            && self.path.ends_with(".rs"))
        .then(|| std::fs::read_to_string(&self.path).ok())
        .flatten();
        let source = source.or(owned_source.as_deref());
        if let (false,Some(file),Some(source))=(self.has_checked_source,&self.indexed,source) {
            self.has_checked_source=true;
            self.is_current_source=file.navigation.as_ref().and_then(|navigation|navigation.implementations.as_ref())
                .is_some_and(|facts|facts.source_digest==crate::implementations::digest(source.as_bytes()));
        }
        if !self.has_impl_scopes && self.path.ends_with(".rs") && source.is_some() {
            self.has_impl_scopes = true;
            if let (Some(file), Some(source)) = (&mut self.indexed, source) {
                let is_current = file
                    .navigation
                    .as_ref()
                    .and_then(|nav| nav.implementations.as_ref())
                    .is_some_and(|facts| {
                        facts.source_digest == crate::implementations::digest(source.as_bytes())
                    });
                if is_current {
                    let mut parser = tree_sitter::Parser::new();
                    if parser
                        .set_language(&tree_sitter_rust::LANGUAGE.into())
                        .is_ok()
                    {
                        if let Ok(tree) =
                            crate::parser::parse_source(&mut parser, source.as_bytes())
                        {
                            declarations::add_impl_containers(file, &tree, source);
                        }
                    }
                }
            }
        }
        let mut chain = self
            .indexed
            .as_ref()
            .map(|file| {
                file.symbols
                    .iter()
                    .filter(|parent| {
                        (declarations::container(parent) || declarations::callable(parent))
                            && declarations::contains(parent, symbol)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        chain.sort_by_key(|parent| {
            (
                parent.range.start_line,
                parent.range.start_col,
                std::cmp::Reverse(parent.range.end_line),
                std::cmp::Reverse(parent.range.end_col),
            )
        });
        if chain.is_empty() {
            if let (Some(file), Some(owner)) = (&self.indexed, &symbol.owner) {
                let parents = file
                    .symbols
                    .iter()
                    .filter(|parent| declarations::container(parent) && parent.name == *owner)
                    .collect::<Vec<_>>();
                if parents.len() == 1 {
                    chain.push(parents[0].clone());
                }
            }
        }
        chain.push(symbol.clone());
        let root = chain[0].clone();
        let existing = self.sections.iter().position(|section| {
            section.root.as_ref().is_some_and(|candidate| {
                candidate.name == root.name
                    && candidate.kind == root.kind
                    && candidate.range == root.range
            })
        });
        let heading = format!(
            "\n### {} {}\n\n",
            declarations::kind_label(&root),
            root.name
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(400)
                .collect::<String>()
        );
        let rows = chain
            .iter()
            .enumerate()
            .filter(|(_, member)| {
                existing.is_none_or(|i| {
                    !self.sections[i].shown.contains(&(
                        member.name.clone(),
                        member.range.start_line,
                        member.range.start_col,
                    ))
                })
            })
            .map(|(depth, member)| {
                (
                    member,
                    format!(
                        "{}- Symbol: {} ({}) [L{}-{}]\n",
                        "  ".repeat(depth),
                        member.name,
                        member.kind,
                        member.range.start_line,
                        member.range.end_line_inclusive()
                    ),
                )
            })
            .collect::<Vec<_>>();
        let extra = rows.iter().map(|(_, row)| row.len()).sum::<usize>()
            + if existing.is_none() { heading.len() } else { 0 };
        if !self.fits(extra) {
            return false;
        }
        // The renderer walks source order. A repeated owner can be revisited by
        // detached Go methods; append to its existing section without moving it.
        let index = existing.unwrap_or_else(|| {
            self.sections.push(Section {
                root: Some(root),
                heading,
                text: String::new(),
                shown: BTreeSet::new(),
                anchors: Vec::new(),
                insertion: 0,
            });
            self.sections.len() - 1
        });
        for (member, row) in rows {
            self.sections[index].text.push_str(&row);
            self.sections[index].shown.insert((
                member.name.clone(),
                member.range.start_line,
                member.range.start_col,
            ));
        }
        self.current = Some(index);
        self.current_depth = chain.len() - 1;
        for member in chain {
            if !self.bodies.iter().any(|body|body.symbol.name==member.name && body.symbol.range==member.range && body.symbol.kind==member.kind && body.symbol.owner==member.owner) {
                // Snippets contain whole physical lines. A same-line sibling can be
                // present even when its AST span is outside this declaration.
                let has_other_declarations=self.indexed.as_ref().is_some_and(|file|file.symbols.iter().any(|other| {
                    let is_same=other.name==member.name && other.kind==member.kind && other.range==member.range && other.owner==member.owner;
                    !is_same && !declarations::contains(other,&member)
                        && other.range.start_line<=member.range.end_line_inclusive()
                        && member.range.start_line<=other.range.end_line_inclusive()
                }));
                self.bodies.push(super::jev::BodyEvidence {path:self.path.clone(),symbol:member,span:None,displayed_body:String::new(),displayed_context:String::new(),completeness:super::jev::Completeness::Missing,has_other_declarations});
            }
        }
        true
    }
    pub fn anchor(&mut self, start: usize, end: usize) {
        if let Some(index) = self.current {
            self.sections[index]
                .anchors
                .push((self.path.clone(), start, end));
        }
    }
    pub fn literal_anchor(&mut self, line: usize) {
        let owner = self.indexed.as_ref().and_then(|file| {
            file.symbols
                .iter()
                .filter(|symbol| {
                    symbol.range.start_line <= line && line <= symbol.range.end_line_inclusive()
                })
                .min_by_key(|symbol| symbol.range.end_line - symbol.range.start_line)
                .cloned()
        });
        if let Some(owner) = owner {
            if self.start_symbol(&owner, None) {
                self.anchor(line, line);
            }
        } else {
            let existing = self
                .sections
                .iter()
                .position(|section| section.root.is_none());
            let heading = "\n### file scope\n\n";
            if existing.is_none() && !self.fits(heading.len()) {
                return;
            }
            let index = existing.unwrap_or_else(|| {
                self.sections.push(Section {
                    root: None,
                    heading: heading.into(),
                    text: String::new(),
                    shown: BTreeSet::new(),
                    anchors: Vec::new(),
                    insertion: 0,
                });
                self.sections.len() - 1
            });
            self.current = Some(index);
            self.anchor(line, line);
        }
    }
    pub fn write_primary(&mut self, text: &mut String, is_partial_file: bool) {
        text.push_str(&self.header);
        text.push_str(&self.metadata);
        for section in &mut self.sections {
            text.push_str(&section.heading);
            text.push_str(&section.text);
            section.insertion = text.len();
        }
        text.push_str("\n### results\n");
        let start = text.len();
        text.push_str(&self.results);
        self.source_span = (!self.results.is_empty()).then_some(start..text.len());
        self.first_source_byte = self.first_result_source_offset.map(|offset| start + offset);
        for body in &mut self.bodies { if let Some(span)=&mut body.span {span.start+=start;span.end+=start;} }
        for segment in &mut self.source_segments {segment.span.start+=start;segment.span.end+=start;segment.first_source_byte+=start;}
        if is_partial_file {
            // fits()/remaining_bytes() reserved the longer global cap footer, so
            // this local notice stays inside the file budget without hiding source.
            text.push_str("\n_Partial file output: per-file byte budget reached. Narrow the query or use `read` for the listed ranges._\n");
        }
    }
}

pub(super) fn append_relations(
    text: &mut String,
    files: &[FileOutput],
    snapshot: &crate::index::PublishedIndexSnapshot,
    scope: Option<&str>,
    should_include_calls: bool,
    should_include_events: bool,
) -> Vec<(usize,usize)> {
    let cap = crate::config::get().search_detail_byte_cap;
    let mut remaining = cap
        .saturating_sub(text.len())
        .min(cap / 2)
        .min(crate::tools::live_symbols::PAYLOAD_BYTE_CAP);
    let root = std::env::current_dir().unwrap_or_default();
    let mut insertions = Vec::new();
    let mut shown_routes = crate::events::ShownRoutes::default();
    for (file_number, file) in files.iter().enumerate() {
        let anchors = file
            .sections
            .iter()
            .flat_map(|section| section.anchors.iter().cloned())
            .collect::<Vec<_>>();
        let file_cap = remaining / (files.len() - file_number);
        if file_cap < 256 || anchors.is_empty() {
            continue;
        }
        let eligible = file
            .sections
            .iter()
            .filter(|section| !section.anchors.is_empty())
            .count();
        let mut available = file_cap;
        for (position, section) in file
            .sections
            .iter()
            .filter(|section| !section.anchors.is_empty())
            .enumerate()
        {
            let section_cap = available / (eligible - position);
            if section_cap < 256 {
                continue;
            }
            let result_paths = std::collections::HashSet::from([file.path.as_str()]);
            let mut relations = super::render::render_static_collection_edges(
                &section.anchors,
                snapshot,
                snapshot.records_for_result_paths(&result_paths),
                scope,
                section_cap,
            );
            let other_cap = section_cap.saturating_sub(relations.len() + 32);
            let events = if should_include_events {
                snapshot.events().for_paths_with_context(
                    &section.anchors,
                    scope,
                    other_cap,
                    &root,
                    Some(&file.path),
                    Some(&mut shown_routes),
                )
            } else {
                String::new()
            };
            let implementations = snapshot.implementations().for_paths_with_context(
                &section.anchors,
                scope,
                other_cap.saturating_sub(events.len()),
                &root,
                should_include_calls,
                Some(&file.path),
            );
            for part in [events, implementations] {
                if !part.is_empty() {
                    relations.push('\n');
                    relations.push_str(&part);
                }
            }
            let mut nested = String::new();
            for line in relations.split_inclusive('\n') {
                if line.starts_with("## ") || line.starts_with("### ") {
                    nested.push_str("##");
                }
                nested.push_str(line);
            }
            if nested.len() <= section_cap {
                available -= nested.len();
                remaining -= nested.len();
                insertions.push((section.insertion, nested));
            }
        }
    }
    insertions.sort_by_key(|(offset, _)| std::cmp::Reverse(*offset));
    let offsets=insertions.iter().map(|(offset,text)|(*offset,text.len())).collect();
    for (offset, relations) in insertions {
        text.insert_str(offset, &relations);
    }
    offsets
}
