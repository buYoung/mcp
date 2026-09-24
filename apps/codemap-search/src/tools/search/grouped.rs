//! Keep ranked snippet selection separate from its presentation order.
use crate::declarations;
use crate::parser::{ExtractedFile, ExtractedSymbol};
use std::collections::BTreeSet;

pub(super) struct Section {
    root: Option<ExtractedSymbol>,
    heading: String,
    text: String,
    shown: BTreeSet<(String, usize, usize)>,
    /// Every declaration row written into this section, in row order.
    members: Vec<BlockSymbol>,
    annotations: Vec<(BlockSymbol, std::ops::Range<usize>)>,
    pub anchors: Vec<(String, usize, usize)>,
    pub insertion: usize,
}

/// Identity of one indexed declaration as shown in the output (inclusive end line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockSymbol {
    pub name: String,
    pub kind: String,
    pub owner: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
}

impl BlockSymbol {
    pub(crate) fn from_symbol(symbol: &ExtractedSymbol) -> Self {
        Self {
            name: symbol.name.clone(),
            kind: symbol.kind.clone(),
            owner: symbol.owner.clone(),
            start_line: symbol.range.start_line,
            end_line: symbol.range.end_line_inclusive(),
        }
    }

    pub(crate) fn contains(&self, other: &BlockSymbol) -> bool {
        self.start_line <= other.start_line
            && other.end_line <= self.end_line
            && (self.start_line, self.end_line) != (other.start_line, other.end_line)
    }
}

/// One unit of the results body exactly as pushed: text plus, for a declaration body, the
/// declaration and the displayed line range. The structured filter works on these blocks
/// instead of re-parsing the rendered Markdown.
#[derive(Clone, Debug)]
pub struct SourceBlock {
    /// Evidence text. Callable fences and notices are held separately until selection.
    pub text: String,
    pub prefix: String,
    pub suffix: String,
    pub source_offset: Option<usize>,
    pub symbol: Option<BlockSymbol>,
    /// First and last displayed source line of a declaration body.
    pub displayed: Option<(usize, usize)>,
    /// The body was cut by the byte budget (its last displayed line may be incomplete).
    pub is_clipped: bool,
    /// The block is an omission note written by the structured filter, not source.
    pub is_note: bool,
}

impl SourceBlock {
    pub(crate) fn rendered_len(&self) -> usize {
        self.prefix.len() + self.text.len() + self.suffix.len()
    }

    fn write(&self, output: &mut String) {
        output.push_str(&self.prefix);
        output.push_str(&self.text);
        output.push_str(&self.suffix);
    }

    /// The body is complete when every line of the declaration is displayed uncut.
    pub(crate) fn is_complete_body(&self) -> bool {
        match (&self.symbol, self.displayed) {
            (Some(symbol), Some((first, last))) => {
                !self.is_clipped && first == symbol.start_line && last == symbol.end_line
            }
            _ => false,
        }
    }

    /// The numbered source lines of the block, without the fence or window notice.
    pub(crate) fn displayed_source(&self) -> String {
        self.text
            .lines()
            .filter(|line| line.contains('\u{2192}'))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(super) const PARTIAL_FILE_NOTICE: &str = "\n_Partial file output: per-file byte budget reached. Narrow the query or use `read` for the listed ranges._\n";

pub(crate) struct FileOutput {
    pub path: String,
    header: String,
    metadata: String,
    sections: Vec<Section>,
    current: Option<usize>,
    current_depth: usize,
    current_symbol: Option<BlockSymbol>,
    // Source blocks are the render plan; the large results string is assembled only
    // after selection in write_primary. Budgeting needs only its byte count.
    results_bytes: usize,
    blocks: Vec<SourceBlock>,
    indexed: Option<ExtractedFile>,
    has_impl_scopes: bool,
    base_bytes: usize,
    cap: usize,
    pub budget_hit: bool,
    pub source_span: Option<std::ops::Range<usize>>,
    first_result_source_offset: Option<usize>,
    pub first_source_byte: Option<usize>,
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
            current_symbol: None,
            results_bytes: 0,
            blocks: Vec::new(),
            indexed: indexed.cloned(),
            has_impl_scopes: false,
            base_bytes,
            cap,
            budget_hit: false,
            source_span: None,
            first_result_source_offset: None,
            first_source_byte: None,
        }
    }
    pub fn len(&self) -> usize {
        self.base_bytes
            + self.header.len()
            + self.metadata.len()
            + self.results_bytes
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
        self.push_block(SourceBlock {
            text: text.into(),
            prefix: String::new(),
            suffix: String::new(),
            source_offset,
            symbol: None,
            displayed: None,
            is_clipped: false,
            is_note: false,
        })
    }
    /// Plan a callable excerpt without assembling its final Markdown body.
    pub(crate) fn plan_source_for_symbol(
        &mut self,
        source: &str,
        notice: &str,
        symbol: &ExtractedSymbol,
        displayed: (usize, usize),
        is_clipped: bool,
    ) -> bool {
        let prefix = format!("{notice}```\n");
        let source_offset = Some(prefix.len());
        self.push_block(SourceBlock {
            text: source.into(),
            prefix,
            suffix: "\n```\n".into(),
            source_offset,
            symbol: Some(BlockSymbol::from_symbol(symbol)),
            displayed: Some(displayed),
            is_clipped,
            is_note: false,
        })
    }

    fn push_block(&mut self, block: SourceBlock) -> bool {
        if !self.fits(block.rendered_len()) {
            return false;
        }
        if self.first_result_source_offset.is_none() {
            self.first_result_source_offset = block
                .source_offset
                .filter(|offset| *offset < block.rendered_len())
                .map(|offset| self.results_bytes + offset);
        }
        self.results_bytes += block.rendered_len();
        self.blocks.push(block);
        true
    }
    pub(crate) fn blocks(&self) -> &[SourceBlock] {
        &self.blocks
    }
    pub(crate) fn indexed(&self) -> Option<&ExtractedFile> {
        self.indexed.as_ref()
    }
    /// Already prepared caller/callee evidence, including its precise/approximate labels.
    /// Borrow ranges instead of retaining another full annotation catalogue.
    pub(crate) fn annotation_for(&self, symbol: &BlockSymbol) -> Option<&str> {
        self.sections.iter().find_map(|section| {
            section
                .annotations
                .iter()
                .find(|(owner, _)| owner == symbol)
                .map(|(_, range)| &section.text[range.clone()])
        })
    }
    /// Every declaration row shown in the file's sections, in output order.
    pub(crate) fn shown_symbols(&self) -> Vec<BlockSymbol> {
        self.sections
            .iter()
            .flat_map(|section| section.members.iter().cloned())
            .collect()
    }
    /// Select blocks for which `replace` returns a note, then recompute the byte budget and
    /// the first-source offset from what remains. Anchors inside a replaced declaration are
    /// dropped so relation rendering does not claim evidence that is no longer displayed.
    /// Returns how many blocks were replaced.
    pub(crate) fn retain_blocks(
        &mut self,
        mut replace: impl FnMut(usize, &SourceBlock) -> Option<String>,
    ) -> usize {
        let mut removed_symbols: Vec<BlockSymbol> = Vec::new();
        let mut replaced = 0;
        for (index, block) in self.blocks.iter_mut().enumerate() {
            let Some(note) = replace(index, block) else {
                continue;
            };
            replaced += 1;
            if let Some(symbol) = block.symbol.take() {
                removed_symbols.push(symbol);
            }
            block.text = note;
            block.prefix.clear();
            block.suffix.clear();
            block.source_offset = None;
            block.displayed = None;
            block.is_clipped = false;
            block.is_note = true;
        }
        if replaced == 0 {
            return 0;
        }
        for section in &mut self.sections {
            section.anchors.retain(|(_, start, end)| {
                !removed_symbols
                    .iter()
                    .any(|symbol| symbol.start_line <= *start && *end <= symbol.end_line)
            });
        }
        self.results_bytes = 0;
        self.first_result_source_offset = None;
        for block in &self.blocks {
            if self.first_result_source_offset.is_none() {
                self.first_result_source_offset = block
                    .source_offset
                    .filter(|offset| *offset < block.rendered_len())
                    .map(|offset| self.results_bytes + offset);
            }
            self.results_bytes += block.rendered_len();
        }
        replaced
    }
    #[cfg(test)]
    pub(crate) fn anchors(&self) -> Vec<(String, usize, usize)> {
        self.sections
            .iter()
            .flat_map(|section| section.anchors.iter().cloned())
            .collect()
    }
    /// Bytes of source blocks (omission notes excluded) that `write_primary` delivered
    /// before the absolute text offset `limit`, for source observations.
    pub fn delivered_source_bytes(&self, limit: usize) -> usize {
        let Some(span) = &self.source_span else {
            return 0;
        };
        let mut offset = span.start;
        let mut delivered = 0;
        for block in &self.blocks {
            let start = offset;
            let end = offset + block.rendered_len();
            offset = end;
            if block.is_note {
                continue;
            }
            delivered += end.min(limit).saturating_sub(start.min(limit));
        }
        delivered
    }
    /// The bytes `write_primary` will append for this file.
    pub fn written_len(&self, is_partial_file: bool) -> usize {
        self.len()
            + if is_partial_file {
                PARTIAL_FILE_NOTICE.len()
            } else {
                0
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
        let section = &mut self.sections[index];
        let start = section.text.len();
        section.text.push_str(&text);
        if let Some(symbol) = &self.current_symbol {
            section
                .annotations
                .push((symbol.clone(), start..section.text.len()));
        }
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
                members: Vec::new(),
                annotations: Vec::new(),
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
            self.sections[index]
                .members
                .push(BlockSymbol::from_symbol(member));
        }
        self.current = Some(index);
        self.current_depth = chain.len() - 1;
        self.current_symbol = Some(BlockSymbol::from_symbol(symbol));
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
                    members: Vec::new(),
                    annotations: Vec::new(),
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
        for block in &self.blocks {
            block.write(text);
        }
        self.source_span = (self.results_bytes > 0).then_some(start..text.len());
        self.first_source_byte = self.first_result_source_offset.map(|offset| start + offset);
        if is_partial_file {
            // fits()/remaining_bytes() reserved the longer global cap footer, so
            // this local notice stays inside the file budget without hiding source.
            text.push_str(PARTIAL_FILE_NOTICE);
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
) {
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
    for (offset, relations) in insertions {
        text.insert_str(offset, &relations);
    }
}
