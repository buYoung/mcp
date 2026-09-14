use super::LiveAnchor;
use crate::declarations::{add_impl_containers, live_signature, parents, visibility};
pub(super) use crate::declarations::{callable, container, symbol_node};
use crate::parser::{ExtractedFile, ExtractedSymbol};
use crate::tools::live_options::{LiveOptions, LiveView};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use tree_sitter::Parser;

pub(super) struct Outline {
    pub file: ExtractedFile,
    pub anchor_line: usize,
    pub parents: Vec<Option<usize>>,
    pub selected: BTreeSet<usize>,
    pub focused: BTreeSet<usize>,
    pub rows: Vec<String>,
    pub order: Vec<usize>,
    pub references: BTreeMap<usize, Vec<String>>,
}

impl Outline {
    pub fn new(
        file: &ExtractedFile,
        anchors: &[&LiveAnchor],
        resolver: &crate::callers::resolution::SourceResolver<'_>,
        options: LiveOptions,
    ) -> Self {
        let mut source = std::fs::read(&file.file_path).unwrap_or_default();
        let root = std::env::current_dir().unwrap_or_default();
        crate::callers::test_code::TestCodeFilter::from_config(&root)
            .mask_source(&file.file_path, &mut source);
        let source = String::from_utf8(source).unwrap_or_default();
        let path = Path::new(&file.file_path);
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        let tree = crate::lang::spec_for_path(path).and_then(|spec| {
            let mut parser = Parser::new();
            parser.set_language(&spec.grammar(ext)).ok()?;
            crate::parser::parse_source(&mut parser, source.as_bytes()).ok()
        });
        let mut file = file.clone();
        if ext == "rs" {
            if let Some(tree) = tree.as_ref() {
                add_impl_containers(&mut file, tree, &source);
            }
        }
        let symbols = &file.symbols;
        let parents = parents(symbols);
        let mut selected = BTreeSet::new();
        let intersects = |s: &ExtractedSymbol| {
            anchors.iter().any(|a| {
                // File-only grep results have no line window and describe the whole file.
                a.start_line.zip(a.end_line).is_none_or(|(lo, hi)| {
                    s.range.start_line <= hi && lo <= s.range.end_line_inclusive()
                })
            })
        };
        let mut roots = BTreeSet::new();
        let focused: BTreeSet<_> = symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| intersects(symbol))
            .map(|(i, _)| i)
            .collect();
        for &i in &focused {
            let mut current = Some(i);
            while let Some(j) = current {
                // Free functions and other top-level declarations are scope roots too.
                if container(&symbols[j]) || parents[j].is_none() {
                    roots.insert(j);
                    break;
                }
                current = parents[j];
            }
        }
        for i in 0..symbols.len() {
            let mut current = Some(i);
            while let Some(j) = current {
                if roots.contains(&j) {
                    selected.insert(i);
                    break;
                }
                // Members themselves belong to the group; their bodies do not.
                if j != i && callable(&symbols[j]) {
                    break;
                }
                current = parents[j];
            }
        }
        selected.extend(&focused);
        if options.view == LiveView::Relations {
            selected.retain(|&i| callable(&symbols[i]) && intersects(&symbols[i]));
        }
        // Parent context is retained for readable owner -> member grouping.
        for i in selected.clone() {
            let mut current = parents[i];
            while let Some(j) = current {
                selected.insert(j);
                current = parents[j];
            }
        }
        let rows = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut current = parents[i];
                let mut depth = 0;
                let mut is_inside_callable = false;
                while let Some(j) = current {
                    depth += 1;
                    is_inside_callable |= callable(&symbols[j]);
                    current = parents[j];
                }
                let kind = if callable(s) {
                    if s.owner.is_some() {
                        "method"
                    } else {
                        "function"
                    }
                } else {
                    s.kind.as_str()
                };
                let visibility = visibility(s, tree.as_ref(), &source, ext, is_inside_callable)
                    .map(|value| format!(", {value}"))
                    .unwrap_or_default();
                let is_expanded = file
                    .macro_expansion()
                    .is_some_and(|info| info.is_expanded_symbol(s));
                let signature = if is_expanded {
                    s.name.clone()
                } else {
                    live_signature(s, tree.as_ref(), &source, ext == "go")
                };
                let signature = if signature.chars().count() > 400 {
                    format!(
                        "{}… [signature shortened]",
                        signature.chars().take(400).collect::<String>()
                    )
                } else {
                    signature
                };
                format!(
                    "{}- {} [{kind}{visibility}{}] — L{}-{}\n",
                    "  ".repeat(depth),
                    signature,
                    if is_expanded { ", macro expansion" } else { "" },
                    s.range.start_line,
                    s.range.end_line_inclusive()
                )
            })
            .collect();
        let references = tree
            .as_ref()
            .filter(|_| options.should_include_relations())
            .map(|tree| {
                super::references::collect_with_resolver(
                    &file,
                    &focused,
                    tree,
                    &source,
                    Some(resolver),
                )
            })
            .unwrap_or_default();
        let mut children = vec![Vec::new(); symbols.len() + 1];
        for (i, p) in parents.iter().enumerate() {
            children[p.unwrap_or(symbols.len())].push(i);
        }
        for group in &mut children {
            group.sort_by_key(|&i| {
                (
                    symbols[i].range.start_line,
                    symbols[i].range.start_col,
                    &symbols[i].name,
                    &symbols[i].kind,
                )
            });
        }
        fn visit(parent: usize, children: &[Vec<usize>], out: &mut Vec<usize>) {
            for &i in &children[parent] {
                out.push(i);
                visit(i, children, out);
            }
        }
        let mut order = Vec::new();
        visit(symbols.len(), &children, &mut order);
        Self {
            file,
            anchor_line: anchors
                .first()
                .and_then(|anchor| anchor.start_line)
                .unwrap_or(1),
            parents,
            selected,
            focused,
            rows,
            order,
            references,
        }
    }

    pub fn chain(&self, i: usize) -> Vec<usize> {
        let mut chain = vec![i];
        let mut current = self.parents[i];
        while let Some(j) = current {
            chain.push(j);
            current = self.parents[j];
        }
        chain.reverse();
        chain
    }

    pub fn section_anchors(
        &self,
        root: usize,
        anchors: &[&LiveAnchor],
    ) -> Vec<(String, usize, usize)> {
        let mut ranges = Vec::new();
        for &i in &self.focused {
            if self.chain(i).first() != Some(&root) {
                continue;
            }
            let range = &self.file.symbols[i].range;
            for anchor in anchors {
                let start = range.start_line.max(anchor.start_line.unwrap_or(1));
                let end = range
                    .end_line_inclusive()
                    .min(anchor.end_line.unwrap_or(usize::MAX));
                if start <= end {
                    ranges.push((start, end));
                }
            }
        }
        ranges.sort_unstable();
        let mut merged: Vec<(String, usize, usize)> = Vec::new();
        for (start, end) in ranges {
            if let Some(last) = merged
                .last_mut()
                .filter(|last| start <= last.2.saturating_add(1))
            {
                last.2 = last.2.max(end);
            } else {
                merged.push((self.file.file_path.clone(), start, end));
            }
        }
        merged
    }
}
