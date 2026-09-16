//! Source-only expansion of the deliberately narrow declarative macro subset.
//! Macro definitions are parsed as data. No target build or procedural macro is
//! run, and every expanded span maps back to original selected source bytes.
use super::model::Location;
use super::rust_names::use_paths;
use super::syntax::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct Segment {
    pub start: usize,
    pub end: usize,
    pub origin: usize,
    pub is_linear: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Expansion {
    pub start: usize,
    pub end: usize,
    pub origin: usize,
    pub definition: Option<Location>,
}
struct Edit {
    start: usize,
    end: usize,
    origin: usize,
    chunks: Vec<(String, usize, bool)>,
    definition: Option<Location>,
}
impl Source {
    pub fn original_offset(&self, offset: usize) -> usize {
        self.segments
            .iter()
            .rev()
            .find(|s| s.start <= offset && offset < s.end)
            .map(|s| s.origin + if s.is_linear { offset - s.start } else { 0 })
            .unwrap_or(offset)
    }
    pub fn offset_location(&self, offset: usize) -> Location {
        let offset = offset.min(self.data.len());
        let row = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        Location {
            path: self.path.clone(),
            line: row + 1,
            column: offset - self.line_starts[row] + 1,
        }
    }
    pub fn expansion(&self, id: NodeId) -> Option<&Expansion> {
        let start = self.nodes[id].start;
        self.expansions
            .iter()
            .find(|e| e.start <= start && start < e.end)
    }
    pub fn expansion_identity(&self, id: NodeId) -> String {
        self.expansion(id)
            .map(|e| {
                if e.definition.is_some() {
                    format!(
                        ":macro@{}:node@{}",
                        e.origin,
                        self.nodes[id].start - e.start
                    )
                } else {
                    format!(":macro@{}", e.origin)
                }
            })
            .unwrap_or_default()
    }
    fn apply_macro_edits(&mut self, mut edits: Vec<Edit>) -> bool {
        if edits.is_empty() || edits.len() > 256 {
            return false;
        }
        edits.sort_by_key(|e| e.start);
        if edits.windows(2).any(|pair| pair[0].end > pair[1].start) {
            return false;
        }
        let bytes = self.parsed_data.len() as isize
            + edits
                .iter()
                .map(|e| {
                    e.chunks.iter().map(|c| c.0.len()).sum::<usize>() as isize
                        - (e.end - e.start) as isize
                })
                .sum::<isize>();
        if bytes < 0 || bytes as usize > super::super::SOURCE_BYTES_PER_FILE {
            return false;
        }
        let old = if self.segments.is_empty() {
            vec![Segment {
                start: 0,
                end: self.parsed_data.len(),
                origin: 0,
                is_linear: true,
            }]
        } else {
            self.segments.clone()
        };
        let mut data = String::new();
        let mut segments = Vec::new();
        let mut expansions = Vec::new();
        let mut cursor = 0;
        fn append(
            data: &mut String,
            segments: &mut Vec<Segment>,
            text: &str,
            origin: usize,
            is_linear: bool,
        ) {
            if text.is_empty() {
                return;
            }
            let start = data.len();
            data.push_str(text);
            segments.push(Segment {
                start,
                end: data.len(),
                origin,
                is_linear,
            });
        }
        fn unchanged(
            source: &Source,
            old: &[Segment],
            data: &mut String,
            segments: &mut Vec<Segment>,
            start: usize,
            end: usize,
        ) {
            for segment in old {
                let left = start.max(segment.start);
                let right = end.min(segment.end);
                if left < right {
                    append(
                        data,
                        segments,
                        &source.parsed_data[left..right],
                        segment.origin
                            + if segment.is_linear {
                                left - segment.start
                            } else {
                                0
                            },
                        segment.is_linear,
                    );
                }
            }
        }
        for edit in &edits {
            unchanged(self, &old, &mut data, &mut segments, cursor, edit.start);
            let start = data.len();
            for (text, origin, linear) in &edit.chunks {
                append(&mut data, &mut segments, text, *origin, *linear);
            }
            expansions.push(Expansion {
                start,
                end: data.len(),
                origin: edit.origin,
                definition: edit.definition.clone(),
            });
            cursor = edit.end;
        }
        unchanged(
            self,
            &old,
            &mut data,
            &mut segments,
            cursor,
            self.parsed_data.len(),
        );
        let shifted = |offset: usize| -> usize {
            (offset as isize
                + edits
                    .iter()
                    .filter(|e| e.end <= offset)
                    .map(|e| {
                        e.chunks.iter().map(|c| c.0.len()).sum::<usize>() as isize
                            - (e.end - e.start) as isize
                    })
                    .sum::<isize>()) as usize
        };
        for old in &self.expansions {
            expansions.push(Expansion {
                start: shifted(old.start),
                end: shifted(old.end),
                origin: old.origin,
                definition: old.definition.clone(),
            });
        }
        expansions.sort_by_key(|e| e.start);
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let Some(tree) = parser.parse(&data, None) else {
            return false;
        };
        if tree.root_node().has_error() {
            return false;
        }
        self.parsed_data = Arc::new(data);
        self.segments = segments;
        self.expansions = expansions;
        self.nodes.clear();
        self.collect_node(tree.root_node(), None, 0);
        self.module_bindings = self.lexical_bindings(0);
        true
    }
}
fn definition(source: &Source, id: NodeId) -> Option<(String, NodeId, Vec<NodeId>)> {
    let rules: Vec<_> = source.nodes[id]
        .children
        .iter()
        .copied()
        .filter(|n| source.nodes[*n].kind == "macro_rule")
        .collect();
    if rules.len() != 1 {
        return None;
    }
    let pattern = source.child(rules[0], &["left"])?;
    let body = source.child(rules[0], &["right"])?;
    let pattern = source.text(Some(pattern));
    let regex = regex::Regex::new(r"^\(\s*\$(\w+)\s*:\s*ident\s*\)$").unwrap();
    let captures = regex.captures(pattern)?;
    let variable = format!("${}", &captures[1]);
    let parts = source.walk(body, false);
    if parts
        .iter()
        .any(|n| source.nodes[*n].kind.contains("repetition"))
    {
        return None;
    }
    let variables: Vec<_> = parts
        .into_iter()
        .filter(|n| source.nodes[*n].kind == "metavariable")
        .collect();
    Some((variable, body, variables))
}
fn invocation_argument(source: &Source, id: NodeId) -> Option<NodeId> {
    let tokens = source.first(id, &["token_tree"])?;
    let values: Vec<_> = source.nodes[tokens]
        .children
        .iter()
        .copied()
        .filter(|n| {
            !matches!(
                source.nodes[*n].kind.as_str(),
                "line_comment" | "block_comment"
            )
        })
        .collect();
    (values.len() == 1
        && matches!(
            source.nodes[values[0]].kind.as_str(),
            "identifier" | "type_identifier"
        ))
    .then(|| values[0])
}
fn expand_items(source: &mut Source) {
    let mut definitions: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
    for &id in &source.nodes[0].children {
        if source.nodes[id].kind == "macro_definition" {
            definitions
                .entry(source.text(source.child(id, &["name"])).into())
                .or_default()
                .push(id);
        }
    }
    let mut edits = Vec::new();
    for &statement in &source.nodes[0].children {
        let invocation = if source.nodes[statement].kind == "expression_statement"
            && source.nodes[statement].children.len() == 1
        {
            source.nodes[statement].children[0]
        } else {
            statement
        };
        if source.nodes[invocation].kind != "macro_invocation" {
            continue;
        }
        let name = source.text(source.child(invocation, &["macro"]));
        let candidates: Vec<_> = definitions
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(|n| source.nodes[*n].start < source.nodes[invocation].start)
            .collect();
        if candidates.len() != 1 {
            continue;
        }
        let Some((variable, body, variables)) = definition(source, candidates[0]) else {
            continue;
        };
        let Some(argument) = invocation_argument(source, invocation) else {
            continue;
        };
        if variables.is_empty() || variables.iter().any(|n| source.text(Some(*n)) != variable) {
            continue;
        }
        let mut cursor = source.nodes[body].start + 1;
        let mut chunks = Vec::new();
        for id in variables {
            chunks.push((
                source.parsed_data[cursor..source.nodes[id].start].to_owned(),
                source.original_offset(cursor),
                true,
            ));
            chunks.push((
                source.text(Some(argument)).into(),
                source.original_offset(source.nodes[argument].start),
                true,
            ));
            cursor = source.nodes[id].end;
        }
        chunks.push((
            source.parsed_data[cursor..source.nodes[body].end - 1].into(),
            source.original_offset(cursor),
            true,
        ));
        let expanded = chunks.iter().map(|c| c.0.as_str()).collect::<String>();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let Some(tree) = parser.parse(&expanded, None) else {
            continue;
        };
        let mut walk = tree.root_node().walk();
        if tree.root_node().has_error()
            || tree.root_node().named_child_count() == 0
            || tree.root_node().named_children(&mut walk).any(|n| {
                !matches!(
                    n.kind(),
                    "impl_item" | "line_comment" | "block_comment" | "attribute_item"
                )
            })
        {
            continue;
        }
        edits.push(Edit {
            start: source.nodes[statement].start,
            end: source.nodes[statement].end,
            origin: source.original_offset(source.nodes[invocation].start),
            chunks,
            definition: None,
        });
    }
    source.apply_macro_edits(edits);
}
fn statement_safe(text: &str, argument: &str, alias: &str) -> bool {
    let data = format!("fn __source_template() {{{text}}}");
    let Some(source) = Source::parse("__source_template.rs", &data) else {
        return false;
    };
    if source.has_parse_error {
        return false;
    }
    if source.nodes[0].children.len() != 1 {
        return false;
    }
    let Some(body) = source.child(source.nodes[0].children[0], &["body"]) else {
        return false;
    };
    if source.nodes[body].children.is_empty() {
        return false;
    }
    for &node in &source.nodes[body].children {
        if matches!(
            source.nodes[node].kind.as_str(),
            "line_comment" | "block_comment"
        ) {
            continue;
        }
        let pattern = source.child(node, &["pattern"]);
        if source.nodes[node].kind != "let_declaration"
            || pattern.is_none_or(|n| {
                source.nodes[n].kind != "identifier" || source.text(Some(n)) != argument
            })
        {
            return false;
        }
    }
    for id in source.walk(body, false) {
        let kind = source.nodes[id].kind.as_str();
        if matches!(
            kind,
            "macro_invocation"
                | "macro_definition"
                | "closure_expression"
                | "function_item"
                | "mod_item"
                | "use_declaration"
        ) {
            return false;
        }
        if !matches!(kind, "identifier" | "type_identifier") || source.text(Some(id)) == argument {
            continue;
        }
        let mut ancestor = source.nodes[id].parent;
        let mut allowed = false;
        while let Some(parent) = ancestor {
            if !matches!(
                source.nodes[parent].kind.as_str(),
                "scoped_identifier"
                    | "scoped_type_identifier"
                    | "generic_type"
                    | "generic_function"
            ) {
                break;
            }
            let text = source.text(Some(parent));
            if text.starts_with("::core::")
                || text.starts_with("::std::")
                || text.starts_with("::alloc::")
                || text.starts_with(&format!("{alias}::"))
            {
                allowed = true;
                break;
            }
            ancestor = source.nodes[parent].parent;
        }
        if !allowed {
            return false;
        }
    }
    true
}
pub(crate) fn expand_sources(
    sources: Vec<Arc<Source>>,
    bindings: &BTreeMap<String, String>,
) -> Vec<Arc<Source>> {
    let mut sources: Vec<Source> = sources.into_iter().map(|s| (*s).clone()).collect();
    for source in sources.iter_mut().filter(|s| s.language == "rust") {
        expand_items(source);
    }
    let templates = sources.clone();
    let mut definitions: BTreeMap<(usize, String), Vec<NodeId>> = BTreeMap::new();
    let mut exported: BTreeMap<(usize, String), Vec<NodeId>> = BTreeMap::new();
    for (index, source) in templates
        .iter()
        .enumerate()
        .filter(|(_, s)| s.language == "rust")
    {
        let mut is_exported = false;
        for &id in &source.nodes[0].children {
            if matches!(
                source.nodes[id].kind.as_str(),
                "line_comment" | "block_comment"
            ) {
                continue;
            }
            if source.nodes[id].kind == "attribute_item" {
                is_exported = source.text(Some(id)).contains("macro_export");
                continue;
            }
            if source.nodes[id].kind == "macro_definition" {
                let name = source.text(source.child(id, &["name"])).to_owned();
                definitions
                    .entry((index, name.clone()))
                    .or_default()
                    .push(id);
                if is_exported {
                    exported.entry((index, name)).or_default().push(id);
                }
            }
            is_exported = false;
        }
    }
    for (index, source) in sources
        .iter_mut()
        .enumerate()
        .filter(|(_, s)| s.language == "rust")
    {
        let mut imports: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for &id in &source.nodes[0].children {
            if source.nodes[id].kind == "use_declaration" {
                for (alias, path) in use_paths(source, source.child(id, &["argument"]), "") {
                    imports.entry(alias).or_default().push(path);
                }
            }
        }
        let nested: std::collections::BTreeSet<_> = source
            .walk(0, false)
            .iter()
            .filter(|id| {
                source.nodes[**id].kind == "macro_definition"
                    && source.nodes[**id].parent != Some(0)
            })
            .map(|id| source.text(source.child(*id, &["name"])).to_owned())
            .collect();
        let mut edits = Vec::new();
        let mut generated = BTreeMap::new();
        for invocation in source.walk(0, false) {
            if source.nodes[invocation].kind != "macro_invocation"
                || source.expansion(invocation).is_some()
            {
                continue;
            }
            let Some(statement) = source.nodes[invocation]
                .parent
                .filter(|n| source.nodes[*n].kind == "expression_statement")
            else {
                continue;
            };
            if source.nodes[statement]
                .parent
                .is_none_or(|n| source.nodes[n].kind != "block")
            {
                continue;
            }
            let name = source.text(source.child(invocation, &["macro"]));
            if nested.contains(name) {
                continue;
            }
            let mut candidates: Vec<_> = definitions
                .get(&(index, name.into()))
                .into_iter()
                .flatten()
                .copied()
                .filter(|n| templates[index].nodes[*n].start < source.nodes[invocation].start)
                .map(|n| (index, n))
                .collect();
            if candidates.is_empty() {
                let paths = imports.get(name).cloned().unwrap_or_else(|| {
                    if name.contains("::") {
                        vec![name.into()]
                    } else {
                        Vec::new()
                    }
                });
                if paths.len() != 1 {
                    continue;
                }
                let parts: Vec<_> = paths[0].split("::").collect();
                if parts.len() != 2 {
                    continue;
                }
                let Some(path) = bindings.get(parts[0]) else {
                    continue;
                };
                let Some(target) = templates.iter().position(|s| &s.path == path) else {
                    continue;
                };
                candidates.extend(
                    exported
                        .get(&(target, parts[1].into()))
                        .into_iter()
                        .flatten()
                        .map(|n| (target, *n)),
                );
            }
            if candidates.len() != 1 {
                continue;
            }
            let (target, id) = candidates[0];
            let template = &templates[target];
            let Some((variable, body, variables)) = definition(template, id) else {
                continue;
            };
            let Some(argument) = invocation_argument(source, invocation)
                .filter(|n| source.nodes[*n].kind == "identifier")
            else {
                continue;
            };
            let argument = source.text(Some(argument));
            if variables.iter().any(|n| {
                !matches!(template.text(Some(*n)), "$crate") && template.text(Some(*n)) != variable
            }) {
                continue;
            }
            let alias = format!("__source_macro_crate_{}", edits.len());
            if source.parsed_data.contains(&alias) {
                continue;
            }
            if variables
                .iter()
                .any(|n| template.text(Some(*n)) == "$crate")
                && !bindings.values().any(|p| p == &template.path)
            {
                continue;
            }
            let mut expanded = String::new();
            let mut cursor = template.nodes[body].start + 1;
            for variable in variables {
                expanded.push_str(&template.parsed_data[cursor..template.nodes[variable].start]);
                expanded.push_str(if template.text(Some(variable)) == "$crate" {
                    &alias
                } else {
                    argument
                });
                cursor = template.nodes[variable].end;
            }
            expanded.push_str(&template.parsed_data[cursor..template.nodes[body].end - 1]);
            if !statement_safe(&expanded, argument, &alias) {
                continue;
            }
            let origin = source.original_offset(source.nodes[invocation].start);
            let definition = Some(template.location(id));
            edits.push(Edit {
                start: source.nodes[statement].start,
                end: source.nodes[statement].end,
                origin,
                chunks: vec![(expanded, origin, false)],
                definition,
            });
            generated.insert(alias, template.path.clone());
        }
        if source.apply_macro_edits(edits) {
            source.generated_imports = generated;
        }
    }
    sources.into_iter().map(Arc::new).collect()
}
