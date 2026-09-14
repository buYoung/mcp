use super::LiveAnchor;
use crate::parser::{CodeRange, ExtractedFile, ExtractedSymbol};
use crate::tools::live_options::{LiveOptions, LiveView};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use tree_sitter::{Node, Parser, Point, Tree};

pub(super) fn callable(s: &ExtractedSymbol) -> bool {
    matches!(s.kind.as_str(), "fn" | "function" | "method")
}

fn container(s: &ExtractedSymbol) -> bool {
    matches!(
        s.kind.as_str(),
        "class" | "impl" | "struct" | "interface" | "trait" | "type" | "enum"
    )
}

fn bounds(r: &CodeRange) -> ((usize, usize), (usize, usize)) {
    ((r.start_line, r.start_col), (r.end_line, r.end_col))
}

fn contains(outer: &ExtractedSymbol, inner: &ExtractedSymbol) -> bool {
    let (a, b) = bounds(&outer.range);
    let (c, d) = bounds(&inner.range);
    a <= c && d <= b && (a != c || b != d)
}

pub(super) fn symbol_node<'a>(
    tree: &'a Tree,
    s: &ExtractedSymbol,
    source: &str,
) -> Option<Node<'a>> {
    let r = &s.range;
    let start = Point::new(
        r.start_line.saturating_sub(1),
        r.start_col.saturating_sub(1),
    );
    let end = Point::new(r.end_line.saturating_sub(1), r.end_col.saturating_sub(1));
    let mut node = tree.root_node().descendant_for_point_range(start, end)?;
    while !crate::parser::node_matches_source_range(node, source.as_bytes(), r) {
        node = node.parent()?;
    }
    // A value-less Go const spec has exactly the same range as its identifier.
    // Prefer the declaration carrying that name over the deepest matching token.
    let original = node;
    while node.child_by_field_name("name").is_none() {
        let Some(parent) = node.parent().filter(|parent| {
            crate::parser::node_matches_source_range(*parent, source.as_bytes(), r)
        }) else {
            return Some(original);
        };
        node = parent;
    }
    Some(node)
}

fn text(node: Node<'_>, source: &str) -> Option<String> {
    Some(
        node.utf8_text(source.as_bytes())
            .ok()?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn signature(s: &ExtractedSymbol, tree: Option<&Tree>, source: &str, is_go: bool) -> String {
    let Some(node) = tree.and_then(|t| symbol_node(t, s, source)) else {
        return s.name.clone();
    };
    if is_go && callable(s) {
        let parameters = node
            .child_by_field_name("parameters")
            .and_then(|n| text(n, source));
        if let Some(parameters) = parameters {
            let receiver = node.child_by_field_name("receiver").and_then(|receiver| {
                let mut cursor = receiver.walk();
                let value = receiver
                    .named_children(&mut cursor)
                    .find_map(|p| p.child_by_field_name("type").and_then(|n| text(n, source)));
                value
            });
            let result = node
                .child_by_field_name("result")
                .and_then(|n| text(n, source))
                .map(|x| format!(" {x}"))
                .unwrap_or_default();
            return format!(
                "{}{}{parameters}{result}",
                receiver.map(|x| format!("({x}).")).unwrap_or_default(),
                s.name
            );
        }
    }
    if matches!(s.kind.as_str(), "field" | "variable" | "const") {
        if let Some(typ) = node
            .child_by_field_name("type")
            .and_then(|n| text(n, source))
        {
            return format!("{} {typ}", s.name);
        }
    }
    s.name.clone()
}

pub(super) struct Outline {
    pub file: ExtractedFile,
    pub anchor_line: usize,
    pub parents: Vec<Option<usize>>,
    pub selected: BTreeSet<usize>,
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
        let symbols = &file.symbols;
        let parents: Vec<_> = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let lexical = symbols
                    .iter()
                    .enumerate()
                    .filter(|(j, c)| *j != i && contains(c, s))
                    .min_by_key(|(_, c)| {
                        (
                            c.range.end_line - c.range.start_line,
                            c.range.end_col.saturating_sub(c.range.start_col),
                        )
                    })
                    .map(|(j, _)| j);
                if lexical.is_some() {
                    return lexical;
                }
                s.owner.as_deref().and_then(|owner| {
                    let matches: Vec<_> = symbols
                        .iter()
                        .enumerate()
                        .filter(|(j, c)| {
                            *j != i
                                && container(c)
                                && c.name == owner
                                && !symbols
                                    .iter()
                                    .any(|outer| callable(outer) && contains(outer, c))
                        })
                        .map(|(j, _)| j)
                        .collect();
                    (matches.len() == 1).then(|| matches[0])
                })
            })
            .collect();
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
        for (i, _) in symbols.iter().enumerate().filter(|(_, s)| intersects(s)) {
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
                let visibility = if is_inside_callable && s.kind != "field" {
                    "local"
                } else if s.flags.is_exported {
                    "exported"
                } else {
                    "not exported"
                };
                let is_expanded = file
                    .macro_expansion()
                    .is_some_and(|info| info.is_expanded_symbol(s));
                let signature = if is_expanded {
                    s.name.clone()
                } else {
                    signature(s, tree.as_ref(), &source, ext == "go")
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
                    "{}{} [{kind}, {visibility}{}] — {}:{}-{}\n",
                    "  ".repeat(depth),
                    signature,
                    if is_expanded { ", macro expansion" } else { "" },
                    file.file_path,
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
                    file,
                    &selected,
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
            file: file.clone(),
            anchor_line: anchors
                .first()
                .and_then(|anchor| anchor.start_line)
                .unwrap_or(1),
            parents,
            selected,
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
}
