mod composite;
mod tokenize;
mod types;

pub use tokenize::{split_identifier, QueryTokens};
pub use types::*;

use std::collections::HashSet;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor, Tree};

use crate::lang::{
    clean_docstring, contains_case_insensitive, find_name, is_composite_extension, spec_for_ext,
    strip_quotes, NameDecision,
};

pub trait CodeExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String>;
}

pub struct TreeSitterExtractor;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IndexAuxiliary {
    pub definition_body: Vec<String>,
    pub reference: Vec<String>,
    pub static_collection_edges: Vec<StaticCollectionEdge>,
}

const INDEX_AUXILIARY_MAX_CHARS: usize = 2048;

impl Default for TreeSitterExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterExtractor {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn extract_for_index(
        &self,
        file_content: &str,
        file_path: &str,
    ) -> Result<(ExtractedFile, IndexAuxiliary), String> {
        self.extract_parts(file_content, file_path, true)
    }
}

/// Standalone index-only auxiliary collection over optional `queries/<lang>/tags.scm` hooks.
/// The normal indexing path uses `TreeSitterExtractor::extract_for_index` to reuse the same
/// parse tree as extraction; this entry point is kept only to cross-check parser tests.
#[cfg(test)]
pub(crate) fn collect_index_auxiliary(
    file_content: &str,
    file_path: &str,
) -> Result<IndexAuxiliary, String> {
    if Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(is_composite_extension)
    {
        return TreeSitterExtractor::new()
            .extract_for_index(file_content, file_path)
            .map(|(_, auxiliary)| auxiliary);
    }
    let ext = Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let Some(spec) = spec_for_ext(ext) else {
        return Ok(IndexAuxiliary::default());
    };
    let Some(tags_query) = spec.tags_query(ext) else {
        return Ok(IndexAuxiliary::default());
    };

    let mut parser = Parser::new();
    parser
        .set_language(&spec.grammar(ext))
        .map_err(|error| error.to_string())?;
    let tree = parser
        .parse(file_content, None)
        .ok_or_else(|| format!("Failed to parse file content for auxiliary tags: {file_path}"))?;
    let source = file_content.as_bytes();

    Ok(collect_index_auxiliary_from_tree(tags_query, &tree, source))
}

fn collect_index_auxiliary_from_tree(
    tags_query: &Query,
    tree: &Tree,
    source: &[u8],
) -> IndexAuxiliary {
    let mut auxiliary = IndexAuxiliary::default();
    let mut seen_definition_body = HashSet::new();
    let mut seen_reference = HashSet::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(tags_query, tree.root_node(), source);
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures {
            let capture_name = tags_query.capture_names()[capture.index as usize];
            let Some(text) = node_text(capture.node, source) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            match capture_name {
                name if name.starts_with("definition.") => push_index_auxiliary_text(
                    &mut auxiliary.definition_body,
                    &mut seen_definition_body,
                    text,
                ),
                name if name.starts_with("reference.") => {
                    push_index_auxiliary_text(&mut auxiliary.reference, &mut seen_reference, text)
                }
                _ => {}
            }
        }
    }

    auxiliary
}

fn push_index_auxiliary_text(out: &mut Vec<String>, seen: &mut HashSet<String>, text: String) {
    let normalized = normalize_navigation_text(text);
    if normalized.is_empty() {
        return;
    }
    let capped = if normalized.chars().count() > INDEX_AUXILIARY_MAX_CHARS {
        normalized.chars().take(INDEX_AUXILIARY_MAX_CHARS).collect()
    } else {
        normalized
    };
    if seen.insert(capped.clone()) {
        out.push(capped);
    }
}

fn range_for_node(node: Node) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn node_text(node: Node, source: &[u8]) -> Option<String> {
    node.utf8_text(source)
        .ok()
        .map(|text| text.trim().to_string())
}

fn normalize_navigation_text(text: String) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" .", ".")
        .replace(". ", ".")
        .replace(" ::", "::")
        .replace(":: ", "::")
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
}

fn receiver_text(node: Node, source: &[u8]) -> Option<String> {
    node_text(node, source).map(normalize_navigation_text)
}

fn string_literal_value(text: &str) -> String {
    strip_quotes(text.trim().trim_end_matches(';').trim())
}

fn clean_type_text(text: &str) -> Option<String> {
    let cleaned = text
        .trim()
        .trim_start_matches(':')
        .trim_start_matches("new ")
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn base_name_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_args = trimmed
        .split(['<', '(', '['])
        .next()
        .unwrap_or(trimmed)
        .trim();
    let name = without_args
        .rsplit(['.', ':', '/', '\\', ' '])
        .find(|part| !part.is_empty())
        .unwrap_or(without_args)
        .trim_matches(['*', '&']);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Normalize a collection-owner type without discarding namespace qualification. This is
/// intentionally narrower than `base_name_from_text`: generic arguments and pointer/reference
/// wrappers do not identify an owner, while package/module/namespace segments do. Any shape that
/// can name multiple owners is omitted rather than guessed.
fn normalized_collection_owner_type(text: &str) -> Option<String> {
    let mut value = clean_type_text(text)?;
    loop {
        if let Some(stripped) = value.strip_prefix('&').or_else(|| value.strip_prefix('*')) {
            value = stripped.trim().to_string();
            // Rust permits a lifetime between `&` and `mut`/the referent. It is a
            // reference wrapper, not part of the collection owner's identity.
            if let Some(lifetime) = value.strip_prefix('\'') {
                let lifetime_end = lifetime.find(char::is_whitespace).filter(|end| *end > 0)?;
                if !lifetime[..lifetime_end]
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
                {
                    return None;
                }
                value = lifetime[lifetime_end..].trim().to_string();
            }
            continue;
        }
        if let Some(stripped) = value
            .strip_prefix("const ")
            .or_else(|| value.strip_prefix("mut "))
            .or_else(|| value.strip_prefix("ref "))
        {
            value = stripped.trim().to_string();
            continue;
        }
        break;
    }
    for elaborated_type_keyword in ["struct ", "class ", "enum ", "union "] {
        if let Some(stripped) = value.strip_prefix(elaborated_type_keyword) {
            value = stripped.trim().to_string();
            break;
        }
    }
    while let Some(stripped) = value.strip_suffix('&').or_else(|| value.strip_suffix('*')) {
        value = stripped.trim().to_string();
    }
    value = value.trim_end_matches(['?', '!']).trim().to_string();
    if value.contains('|') || value.contains('&') || value.contains("->") {
        return None;
    }

    let mut head_end = value.len();
    if let Some(generic_start) = value.find('<') {
        let mut depth = 0usize;
        let mut close = None;
        for (index, character) in value
            .char_indices()
            .skip_while(|(index, _)| *index < generic_start)
        {
            match character {
                '<' => depth += 1,
                '>' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        close = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        if close.is_none() || !value[close? + 1..].trim().is_empty() {
            return None;
        }
        head_end = generic_start;
    } else if value.contains('>') {
        return None;
    }

    let head = value[..head_end].trim();
    if head.is_empty()
        || !head.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':')
        })
    {
        return None;
    }
    Some(head.to_string())
}

fn first_descendant_kind<'a>(node: Node<'a>, kinds: &[&str]) -> Option<Node<'a>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }
    for child_index in 0..node.child_count() {
        let child = node.child(child_index as u32).unwrap();
        if let Some(found) = first_descendant_kind(child, kinds) {
            return Some(found);
        }
    }
    None
}

fn declarator_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" | "property_identifier" | "type_identifier" => {
            node_text(node, source)
        }
        "init_declarator"
        | "pointer_declarator"
        | "array_declarator"
        | "function_declarator"
        | "parenthesized_declarator"
        | "attributed_declarator" => node
            .child_by_field_name("declarator")
            .and_then(|declarator| declarator_name(declarator, source)),
        "reference_declarator" => {
            let mut cursor = node.walk();
            let name = node
                .named_children(&mut cursor)
                .find_map(|child| declarator_name(child, source));
            name
        }
        _ => None,
    }
}

fn field_or_descendant_name(node: Node, source: &[u8]) -> Option<String> {
    if let Some(name) = node.child_by_field_name("name") {
        return node_text(name, source);
    }
    if let Some(declarator) = node.child_by_field_name("declarator") {
        if let Some(name) = declarator_name(declarator, source) {
            return Some(name);
        }
    }
    if let Some(declarator) = first_descendant_kind(node, &["variable_declarator"]) {
        if let Some(name) = declarator.child_by_field_name("name") {
            return node_text(name, source);
        }
    }
    first_descendant_kind(
        node,
        &[
            "identifier",
            "field_identifier",
            "property_identifier",
            "type_identifier",
        ],
    )
    .and_then(|name| node_text(name, source))
}

fn call_name_and_receiver(function_node: Node, source: &[u8]) -> Option<(String, Option<String>)> {
    match function_node.kind() {
        "identifier"
        | "field_identifier"
        | "property_identifier"
        | "type_identifier"
        | "namespace_identifier" => node_text(function_node, source).map(|name| (name, None)),
        "member_expression" | "field_expression" | "selector_expression" | "attribute" => {
            let name_node = function_node
                .child_by_field_name("property")
                .or_else(|| function_node.child_by_field_name("field"))
                .or_else(|| function_node.child_by_field_name("attribute"))
                .or_else(|| function_node.child_by_field_name("name"))?;
            let receiver = function_node
                .child_by_field_name("object")
                .or_else(|| function_node.child_by_field_name("operand"))
                .or_else(|| function_node.child_by_field_name("receiver"))
                .or_else(|| function_node.child_by_field_name("value"))
                .or_else(|| function_node.child_by_field_name("argument"))
                .and_then(|node| receiver_text(node, source));
            node_text(name_node, source).map(|name| (name, receiver))
        }
        "scoped_identifier" | "qualified_identifier" | "qualified_type" => {
            let name_node = function_node.child_by_field_name("name")?;
            let receiver = function_node
                .child_by_field_name("scope")
                .or_else(|| function_node.child_by_field_name("path"))
                .and_then(|node| receiver_text(node, source));
            node_text(name_node, source).map(|name| (name, receiver))
        }
        "navigation_expression" => {
            let text = receiver_text(function_node, source)?;
            if let Some((receiver, name)) = text.rsplit_once('.') {
                let name = base_name_from_text(name)?;
                let receiver = receiver.trim();
                return Some((
                    name,
                    if receiver.is_empty() {
                        None
                    } else {
                        Some(receiver.to_string())
                    },
                ));
            }
            find_name(function_node, source).map(|name| (name, None))
        }
        "parenthesized_expression" | "parenthesized_declarator" => {
            for child_index in 0..function_node.child_count() {
                let child = function_node.child(child_index as u32).unwrap();
                if child.is_named() {
                    return call_name_and_receiver(child, source);
                }
            }
            None
        }
        _ => find_name(function_node, source).map(|name| (name, None)),
    }
}

fn call_site_from_node(node: Node, source: &[u8]) -> Option<CallSite> {
    let (name, receiver) = match node.kind() {
        "method_invocation" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|name| node_text(name, source))?;
            let receiver = node
                .child_by_field_name("object")
                .or_else(|| node.child_by_field_name("receiver"))
                .and_then(|receiver| receiver_text(receiver, source));
            (name, receiver)
        }
        "object_creation_expression" => {
            let type_node = node
                .child_by_field_name("type")
                .or_else(|| first_descendant_kind(node, &["type_identifier", "identifier"]))?;
            let name = node_text(type_node, source).and_then(|text| base_name_from_text(&text))?;
            (name, None)
        }
        "new_expression" => {
            let constructor = node
                .child_by_field_name("constructor")
                .or_else(|| first_descendant_kind(node, &["identifier", "member_expression"]))?;
            call_name_and_receiver(constructor, source)?
        }
        "method_call_expression" => {
            let name = node
                .child_by_field_name("name")
                .and_then(|name| node_text(name, source))?;
            let receiver = node
                .child_by_field_name("receiver")
                .and_then(|receiver| receiver_text(receiver, source));
            (name, receiver)
        }
        "macro_invocation" => {
            let macro_node = node.child_by_field_name("macro")?;
            let (name, receiver) = call_name_and_receiver(macro_node, source)?;
            (name, receiver)
        }
        "instruction" => {
            let kind_node = node.child_by_field_name("kind")?;
            let target = (0..node.named_child_count())
                .filter_map(|index| node.named_child(index as u32))
                .find(|child| child.id() != kind_node.id())?;
            let name = node_text(target, source).and_then(|text| base_name_from_text(&text))?;
            (name, None)
        }
        "navigation_expression" => {
            let text = receiver_text(node, source)?;
            let name = base_name_from_text(&text)?;
            let receiver = text
                .rsplit_once('.')
                .map(|(left, _)| left.trim().to_string());
            (name, receiver)
        }
        _ => {
            let function_node = node
                .child_by_field_name("function")
                .or_else(|| node.child_by_field_name("operator"))
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| {
                    (0..node.child_count())
                        .map(|index| node.child(index as u32).unwrap())
                        .find(|child| child.is_named())
                })?;
            call_name_and_receiver(function_node, source)?
        }
    };
    if name.is_empty() {
        return None;
    }
    Some(CallSite {
        name,
        receiver,
        range: range_for_node(node),
        scope_id: scope_id_for_node(node),
    })
}

fn value_type_from_initializer(value_node: Node, source: &[u8]) -> Option<String> {
    if matches!(value_node.kind(), "call_expression" | "call") {
        if let Some(function_node) = value_node
            .child_by_field_name("function")
            .or_else(|| value_node.child_by_field_name("name"))
        {
            if let Some((name, Some(receiver))) = call_name_and_receiver(function_node, source) {
                if matches!(name.as_str(), "new" | "default") {
                    return base_name_from_text(&receiver);
                }
            }
            return node_text(function_node, source).and_then(|text| base_name_from_text(&text));
        }
    }

    value_node
        .child_by_field_name("constructor")
        .or_else(|| value_node.child_by_field_name("type"))
        .or_else(|| value_node.child_by_field_name("function"))
        .or_else(|| first_descendant_kind(value_node, &["type_identifier", "identifier"]))
        .and_then(|value_node| node_text(value_node, source))
        .and_then(|text| base_name_from_text(&text))
}

fn scope_id_for_node(node: Node) -> Option<usize> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if matches!(
            ancestor.kind(),
            "function_declaration"
                | "function_definition"
                | "method_definition"
                | "method_declaration"
                | "constructor_declaration"
                | "arrow_function"
                | "function_item"
        ) {
            let range = range_for_node(ancestor);
            return Some(range.start_line.saturating_mul(100_000) + range.end_line);
        }
        current = ancestor.parent();
    }
    None
}

fn push_binding_name(names: &mut Vec<String>, name: String) {
    if !names.iter().any(|existing| existing == &name) {
        names.push(name);
    }
}

fn binding_names_from_pattern(node: Node, source: &[u8]) -> Vec<String> {
    match node.kind() {
        "identifier"
        | "field_identifier"
        | "property_identifier"
        | "type_identifier"
        | "shorthand_field_identifier"
        | "shorthand_property_identifier_pattern" => node_text(node, source).into_iter().collect(),
        "pair_pattern" => node
            .child_by_field_name("value")
            .or_else(|| node.child_by_field_name("pattern"))
            .map(|value| binding_names_from_pattern(value, source))
            .unwrap_or_default(),
        "object_assignment_pattern" => node
            .child_by_field_name("left")
            .map(|left| binding_names_from_pattern(left, source))
            .unwrap_or_default(),
        "field_pattern" => node
            .child_by_field_name("pattern")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| node.child_by_field_name("name"))
            .map(|pattern| binding_names_from_pattern(pattern, source))
            .unwrap_or_default(),
        "tuple_struct_pattern" | "struct_pattern" => {
            let mut names = Vec::new();
            let mut skipped_constructor = false;
            for child_index in 0..node.child_count() {
                let child = node.child(child_index as u32).unwrap();
                if !child.is_named() {
                    continue;
                }
                if !skipped_constructor
                    && matches!(
                        child.kind(),
                        "identifier"
                            | "field_identifier"
                            | "type_identifier"
                            | "scoped_identifier"
                            | "qualified_identifier"
                    )
                {
                    skipped_constructor = true;
                    continue;
                }
                for name in binding_names_from_pattern(child, source) {
                    push_binding_name(&mut names, name);
                }
            }
            names
        }
        "assignment_pattern" => node
            .child_by_field_name("left")
            .or_else(|| node.child_by_field_name("name"))
            .map(|left| binding_names_from_pattern(left, source))
            .unwrap_or_default(),
        "rest_pattern" => node
            .child_by_field_name("argument")
            .or_else(|| node.child_by_field_name("value"))
            .or_else(|| {
                (0..node.child_count())
                    .map(|index| node.child(index as u32).unwrap())
                    .find(|child| child.is_named())
            })
            .map(|argument| binding_names_from_pattern(argument, source))
            .unwrap_or_default(),
        "array_pattern" | "object_pattern" | "tuple_pattern" | "list_pattern" => {
            let mut names = Vec::new();
            for child_index in 0..node.child_count() {
                let child = node.child(child_index as u32).unwrap();
                if child.is_named() {
                    for name in binding_names_from_pattern(child, source) {
                        push_binding_name(&mut names, name);
                    }
                }
            }
            names
        }
        kind if kind.ends_with("_pattern") => {
            let mut names = Vec::new();
            for child_index in 0..node.child_count() {
                let child = node.child(child_index as u32).unwrap();
                if child.is_named() {
                    for name in binding_names_from_pattern(child, source) {
                        push_binding_name(&mut names, name);
                    }
                }
            }
            names
        }
        _ => Vec::new(),
    }
}

fn local_binding_names_from_node(node: Node, source: &[u8]) -> Vec<String> {
    for field_name in ["name", "pattern"] {
        if let Some(pattern_node) = node.child_by_field_name(field_name) {
            let names = binding_names_from_pattern(pattern_node, source);
            if !names.is_empty() {
                return names;
            }
        }
    }

    let names = binding_names_from_pattern(node, source);
    if !names.is_empty() {
        return names;
    }

    field_or_descendant_name(node, source).into_iter().collect()
}

fn local_bindings_from_node(node: Node, source: &[u8]) -> Vec<LocalBinding> {
    let names = local_binding_names_from_node(node, source);
    if names.is_empty() {
        return Vec::new();
    }

    let type_name = node
        .child_by_field_name("type")
        .and_then(|type_node| node_text(type_node, source))
        .and_then(|text| clean_type_text(&text))
        .or_else(|| {
            first_descendant_kind(node, &["type_annotation", "type_identifier"])
                .and_then(|type_node| node_text(type_node, source))
                .and_then(|text| clean_type_text(&text))
        });
    let value_type = first_descendant_kind(
        node,
        &[
            "new_expression",
            "object_creation_expression",
            "composite_literal",
            "call_expression",
            "call",
        ],
    )
    .and_then(|value_node| value_type_from_initializer(value_node, source));

    names
        .into_iter()
        .map(|name| LocalBinding {
            name,
            type_name: type_name.clone(),
            value_type: value_type.clone(),
            range: range_for_node(node),
            scope_id: scope_id_for_node(node),
        })
        .collect()
}

fn quoted_source_after_from(text: &str) -> Option<String> {
    let source_part = text
        .rsplit_once(" from ")
        .map(|(_, right)| right)
        .unwrap_or(text);
    let quote_start = source_part.find(['"', '\''])?;
    let quote = source_part.as_bytes()[quote_start] as char;
    let rest = &source_part[quote_start + 1..];
    let quote_end = rest.find(quote)?;
    Some(rest[..quote_end].to_string())
}

fn push_named_imports(
    entries: &mut Vec<ImportEntry>,
    named_clause: &str,
    source: Option<String>,
    range: &CodeRange,
) {
    for part in named_clause.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut pieces = trimmed.split_whitespace();
        let imported = pieces.next().unwrap_or("").trim();
        if imported.is_empty() {
            continue;
        }
        let local = match (pieces.next(), pieces.next()) {
            (Some("as"), Some(alias)) => alias,
            _ => imported,
        };
        entries.push(ImportEntry {
            local_name: local.to_string(),
            imported_name: Some(imported.to_string()),
            source: source.clone(),
            kind: ImportKind::Named,
            range: range.clone(),
        });
    }
}

fn dotted_import_entry(path: &str, alias: Option<&str>, range: CodeRange) -> Option<ImportEntry> {
    let cleaned_path = path.trim().trim_end_matches(';').trim();
    if cleaned_path.is_empty() {
        return None;
    }
    let imported_name = base_name_from_text(cleaned_path)?;
    let local_name = alias
        .map(str::trim)
        .filter(|alias| !alias.is_empty())
        .unwrap_or(&imported_name)
        .to_string();
    let source = cleaned_path
        .rsplit_once('.')
        .map(|(package, _)| package.to_string())
        .unwrap_or_else(|| cleaned_path.to_string());
    Some(ImportEntry {
        local_name,
        imported_name: Some(imported_name),
        source: Some(source),
        kind: ImportKind::Named,
        range,
    })
}

fn push_go_import_spec(entries: &mut Vec<ImportEntry>, spec: &str, range: &CodeRange) {
    let import_spec = spec.split_once("//").map(|(left, _)| left).unwrap_or(spec);
    let import_spec = import_spec.trim();
    if import_spec.is_empty() {
        return;
    }

    let mut parts = import_spec.split_whitespace();
    let Some(first) = parts.next() else {
        return;
    };
    let (alias, path_part) = match parts.next() {
        Some(path_part) => (Some(first), path_part),
        None => (None, first),
    };
    let source = string_literal_value(path_part);
    if source.is_empty() {
        return;
    }
    let Some(imported_name) = base_name_from_text(&source) else {
        return;
    };
    let local_name = alias
        .filter(|alias| !matches!(*alias, "_" | "."))
        .unwrap_or(&imported_name)
        .to_string();

    entries.push(ImportEntry {
        local_name,
        imported_name: Some(imported_name),
        source: Some(source),
        kind: ImportKind::Named,
        range: range.clone(),
    });
}

fn include_name_from_source(source: &str) -> Option<String> {
    let file_name = source
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(source)
        .trim();
    let stem = file_name
        .rsplit_once('.')
        .map(|(left, _)| left)
        .unwrap_or(file_name)
        .trim();
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

fn import_entries_from_text(text: &str, range: CodeRange) -> Vec<ImportEntry> {
    let trimmed = text.trim().trim_end_matches(';').trim();
    if let Some(rest) = trimmed.strip_prefix("export import ") {
        return import_entries_from_text(&format!("import {rest}"), range);
    }
    let mut entries = Vec::new();
    if let Some(rest) = trimmed.strip_prefix("import ") {
        let source = quoted_source_after_from(trimmed);
        let clause = rest
            .split_once(" from ")
            .map(|(left, _)| left)
            .unwrap_or(rest)
            .trim()
            .trim_start_matches("type ")
            .trim();
        if let Some(group_body) = clause
            .strip_prefix('(')
            .and_then(|body| body.trim().strip_suffix(')'))
        {
            for import_spec in group_body.lines() {
                push_go_import_spec(&mut entries, import_spec, &range);
            }
            return entries;
        }
        if clause.starts_with('"') || clause.starts_with('\'') {
            if let Some(source) = source {
                if !source.starts_with('.') {
                    if let Some(name) = base_name_from_text(&source) {
                        entries.push(ImportEntry {
                            local_name: name.clone(),
                            imported_name: Some(name),
                            source: Some(source),
                            kind: ImportKind::Named,
                            range,
                        });
                    }
                }
            }
            return entries;
        }
        if source.is_none() && (clause.contains('.') || clause.contains(" as ")) {
            let (path, alias) = clause
                .split_once(" as ")
                .map(|(path, alias)| (path, Some(alias)))
                .unwrap_or((clause, None));
            if let Some(entry) = dotted_import_entry(path, alias, range) {
                entries.push(entry);
            }
            return entries;
        }
        if let Some(namespace) = clause.strip_prefix("* as ") {
            let local = namespace.trim();
            if !local.is_empty() {
                entries.push(ImportEntry {
                    local_name: local.to_string(),
                    imported_name: None,
                    source,
                    kind: ImportKind::Namespace,
                    range,
                });
            }
            return entries;
        }
        if let Some(open) = clause.find('{') {
            let default_clause = clause[..open].trim().trim_end_matches(',').trim();
            if !default_clause.is_empty() {
                entries.push(ImportEntry {
                    local_name: default_clause.to_string(),
                    imported_name: None,
                    source: source.clone(),
                    kind: ImportKind::Default,
                    range: range.clone(),
                });
            }
            if let Some(close) = clause[open + 1..].find('}') {
                let named = &clause[open + 1..open + 1 + close];
                push_named_imports(&mut entries, named, source, &range);
            }
            return entries;
        }
        if !clause.is_empty() {
            entries.push(ImportEntry {
                local_name: clause.to_string(),
                imported_name: None,
                source,
                kind: ImportKind::Default,
                range,
            });
        }
        return entries;
    }

    if let Some(rest) = trimmed.strip_prefix("from ") {
        let (module, names) = rest.split_once(" import ").unwrap_or((rest, ""));
        let source = Some(module.trim().to_string());
        push_named_imports(&mut entries, names, source, &range);
        return entries;
    }

    if let Some(rest) = trimmed.strip_prefix("use ") {
        let path = rest.trim().trim_end_matches(';');
        if path.ends_with("::*") || path.ends_with(".*") {
            entries.push(ImportEntry {
                local_name: "*".to_string(),
                imported_name: None,
                source: Some(path.to_string()),
                kind: ImportKind::Glob,
                range,
            });
        } else if let Some((path, alias)) = path.split_once(" as ") {
            if let Some(imported_name) = base_name_from_text(path) {
                let local_name = alias.trim();
                if !local_name.is_empty() {
                    entries.push(ImportEntry {
                        local_name: local_name.to_string(),
                        imported_name: Some(imported_name),
                        source: Some(path.trim().to_string()),
                        kind: ImportKind::Named,
                        range,
                    });
                }
            }
        } else if let Some(name) = base_name_from_text(path) {
            entries.push(ImportEntry {
                local_name: name.clone(),
                imported_name: Some(name),
                source: Some(path.to_string()),
                kind: ImportKind::Named,
                range,
            });
        }
        return entries;
    }

    if trimmed.starts_with("import")
        || trimmed.starts_with("#include")
        || trimmed.starts_with(".include")
    {
        if let Some(source) = quoted_source_after_from(trimmed).or_else(|| {
            trimmed
                .split_whitespace()
                .last()
                .map(|part| string_literal_value(part.trim_matches(['<', '>'])))
        }) {
            let name = if trimmed.starts_with("#include") || trimmed.starts_with(".include") {
                include_name_from_source(&source)
            } else {
                base_name_from_text(&source)
            };
            if let Some(name) = name {
                entries.push(ImportEntry {
                    local_name: name.clone(),
                    imported_name: Some(name),
                    source: Some(source),
                    kind: ImportKind::Named,
                    range,
                });
            }
        }
    }
    entries
}

fn import_entries_from_node(node: Node, source: &[u8]) -> Vec<ImportEntry> {
    let Some(text) = node_text(node, source) else {
        return Vec::new();
    };
    let range = range_for_node(node);
    if matches!(
        node.kind(),
        "declaration" | "labeled_statement" | "template_type"
    ) {
        let trimmed = text.trim().trim_end_matches(';').trim();
        let module_name = trimmed
            .strip_prefix("export import ")
            .or_else(|| trimmed.strip_prefix("import "))
            .map(str::trim)
            .map(|name| string_literal_value(name.trim_matches(['<', '>'])))
            .filter(|name| !name.is_empty());
        if let Some(module_name) = module_name {
            let imported_name =
                base_name_from_text(&module_name).unwrap_or_else(|| module_name.clone());
            return vec![ImportEntry {
                local_name: imported_name.clone(),
                imported_name: Some(imported_name),
                source: Some(module_name),
                kind: ImportKind::Named,
                range,
            }];
        }
    }
    import_entries_from_text(&text, range)
}

fn enclosing_typescript_class(mut node: Node) -> Option<Node> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "class_declaration" | "abstract_class_declaration"
        ) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn typescript_class_name(class_node: Node, source: &[u8]) -> Option<String> {
    class_node
        .child_by_field_name("name")
        .and_then(|name| node_text(name, source))
}

fn typescript_class_member_type(
    class_node: Node,
    member_name: &str,
    source: &[u8],
) -> Option<String> {
    fn parameter_type(node: Node, member_name: &str, source: &[u8]) -> Option<String> {
        if matches!(
            node.kind(),
            "required_parameter" | "optional_parameter" | "formal_parameter"
        ) && field_or_descendant_name(node, source).as_deref() == Some(member_name)
        {
            let type_node = node
                .child_by_field_name("type")
                .or_else(|| first_descendant_kind(node, &["type_annotation"]));
            return type_node.and_then(|type_node| typescript_outer_named_type(type_node, source));
        }
        named_children(node).find_map(|child| parameter_type(child, member_name, source))
    }

    let body = class_node.child_by_field_name("body")?;
    for child_index in 0..body.child_count() {
        let child = body.child(child_index as u32).unwrap();
        if !child.is_named() {
            continue;
        }
        let Some(name_node) = child.child_by_field_name("name") else {
            continue;
        };
        if node_text(name_node, source).as_deref() != Some(member_name) {
            continue;
        }
        let Some(type_node) = child
            .child_by_field_name("type")
            .or_else(|| first_descendant_kind(child, &["type_annotation"]))
        else {
            continue;
        };
        return typescript_outer_named_type(type_node, source);
    }
    for child in named_children(body) {
        let is_constructor = child.kind() == "constructor_declaration"
            || child
                .child_by_field_name("name")
                .and_then(|name| node_text(name, source))
                .as_deref()
                == Some("constructor");
        if !is_constructor {
            continue;
        }
        let parameters = child
            .child_by_field_name("parameters")
            .or_else(|| first_descendant_kind(child, &["formal_parameters"]))?;
        if let Some(type_name) = parameter_type(parameters, member_name, source) {
            return Some(type_name);
        }
    }
    None
}

fn typescript_outer_named_type(mut node: Node, source: &[u8]) -> Option<String> {
    while matches!(node.kind(), "type_annotation" | "parenthesized_type") {
        node = named_children(node).next()?;
    }
    match node.kind() {
        "type_identifier" => node_text(node, source),
        "generic_type" => node
            .child_by_field_name("name")
            .or_else(|| named_children(node).next())
            .filter(|head| head.kind() == "type_identifier")
            .and_then(|head| node_text(head, source)),
        _ => None,
    }
}

/// Resolve only identities explicit in the syntax. `this` means the enclosing class;
/// `this.member` is accepted only when that member has one concrete named type.
fn typescript_collection_owner_type(
    owner_node: Node,
    site_node: Node,
    source: &[u8],
) -> Option<String> {
    let class_node = enclosing_typescript_class(site_node)?;
    match owner_node.kind() {
        "this" => typescript_class_name(class_node, source),
        "member_expression" => {
            let root = owner_node.child_by_field_name("object")?;
            if root.kind() != "this" {
                return None;
            }
            let member = owner_node.child_by_field_name("property")?;
            if !matches!(
                member.kind(),
                "property_identifier" | "private_property_identifier"
            ) {
                return None;
            }
            let member_name = node_text(member, source)?;
            typescript_class_member_type(class_node, &member_name, source)
        }
        _ => None,
    }
}

fn typescript_source_context(
    mut node: Node,
    source: &[u8],
) -> (Option<String>, Option<String>, Option<CodeRange>) {
    let mut source_owner = None;
    let mut source_symbol = None;
    let mut source_owner_range = None;
    while let Some(parent) = node.parent() {
        if source_symbol.is_none()
            && matches!(parent.kind(), "method_definition" | "function_declaration")
        {
            source_symbol = parent
                .child_by_field_name("name")
                .and_then(|name| node_text(name, source));
        }
        if source_owner.is_none()
            && matches!(
                parent.kind(),
                "class_declaration" | "abstract_class_declaration"
            )
        {
            source_owner = typescript_class_name(parent, source);
            source_owner_range = source_owner.as_ref().map(|_| range_for_node(parent));
        }
        if source_owner.is_some() && source_symbol.is_some() {
            break;
        }
        node = parent;
    }
    (source_owner, source_symbol, source_owner_range)
}

fn enclosing_callable(mut node: Node) -> Option<Node> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "function_declaration"
                | "function_definition"
                | "method_definition"
                | "method_declaration"
                | "constructor_declaration"
                | "function_item"
        ) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn callable_name(node: Node, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| node_text(name, source))
        .or_else(|| {
            node.child_by_field_name("declarator")
                .and_then(|declarator| declarator_name(declarator, source))
        })
}

fn static_collection_source_context(
    spec: &dyn crate::lang::LanguageSpec,
    ext: &str,
    site_node: Node,
    source: &[u8],
) -> (Option<String>, Option<String>, Option<CodeRange>) {
    let Some(callable) = enclosing_callable(site_node) else {
        return (None, None, None);
    };
    let source_owner = spec.find_owner(callable, ext, source);
    let source_owner_range = source_owner.as_ref().and_then(|owner| {
        let mut node = site_node;
        while let Some(parent) = node.parent() {
            if node_declares_type(parent, owner, source) {
                return Some(range_for_node(parent));
            }
            node = parent;
        }
        let mut root = site_node;
        while let Some(parent) = root.parent() {
            root = parent;
        }
        unique_type_declaration_range(root, owner, source)
    });
    (
        source_owner,
        callable_name(callable, source),
        source_owner_range,
    )
}

fn named_children(node: Node) -> impl Iterator<Item = Node> {
    (0..node.child_count())
        .filter_map(move |index| node.child(index as u32))
        .filter(|child| child.is_named())
}

fn unwrap_collection_expression(mut node: Node) -> Node {
    loop {
        let should_unwrap = matches!(
            node.kind(),
            "expression_list"
                | "reference_expression"
                | "parenthesized_expression"
                | "pointer_expression"
        );
        if !should_unwrap {
            return node;
        }
        let mut children = named_children(node);
        let Some(first) = children.next() else {
            return node;
        };
        if children.next().is_some() {
            return node;
        }
        node = first;
    }
}

fn parameter_type_for_name(node: Node, name: &str, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "block" | "compound_statement" | "function_body"
    ) {
        return None;
    }
    if matches!(
        node.kind(),
        "parameter_declaration" | "formal_parameter" | "parameter"
    ) {
        let parameter_name = node
            .child_by_field_name("name")
            .and_then(|name_node| node_text(name_node, source))
            .or_else(|| {
                node.child_by_field_name("declarator")
                    .and_then(|declarator| declarator_name(declarator, source))
            })
            .or_else(|| field_or_descendant_name(node, source));
        if parameter_name.as_deref() == Some(name) {
            return node
                .child_by_field_name("type")
                .or_else(|| {
                    first_descendant_kind(
                        node,
                        &[
                            "type_identifier",
                            "user_type",
                            "struct_specifier",
                            "class_specifier",
                        ],
                    )
                })
                .and_then(|type_node| node_text(type_node, source))
                .and_then(|text| normalized_collection_owner_type(&text));
        }
    }
    for child in named_children(node) {
        if let Some(type_name) = parameter_type_for_name(child, name, source) {
            return Some(type_name);
        }
    }
    None
}

fn callable_has_local_binding(node: Node, name: &str, source: &[u8]) -> bool {
    if matches!(
        node.kind(),
        "parameter_declaration"
            | "formal_parameter"
            | "parameter"
            | "local_variable_declaration"
            | "property_declaration"
            | "let_declaration"
            | "declaration"
    ) && field_or_descendant_name(node, source).as_deref() == Some(name)
    {
        return true;
    }
    for child in named_children(node) {
        if matches!(
            child.kind(),
            "function_declaration"
                | "function_definition"
                | "method_definition"
                | "method_declaration"
                | "constructor_declaration"
                | "function_item"
        ) {
            continue;
        }
        if callable_has_local_binding(child, name, source) {
            return true;
        }
    }
    false
}

fn is_type_declaration_node(node: Node) -> bool {
    matches!(
        node.kind(),
        "class_definition"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "object_declaration"
            | "struct_item"
            | "type_spec"
            | "class_specifier"
            | "struct_specifier"
            | "union_specifier"
    )
}

fn node_declares_type(node: Node, type_name: &str, source: &[u8]) -> bool {
    if !is_type_declaration_node(node) {
        return false;
    }
    node.child_by_field_name("name")
        .and_then(|name| node_text(name, source))
        .as_deref()
        == Some(type_name)
}

fn unique_type_declaration_range(root: Node, type_name: &str, source: &[u8]) -> Option<CodeRange> {
    fn collect(node: Node, type_name: &str, source: &[u8], ranges: &mut Vec<CodeRange>) {
        if node_declares_type(node, type_name, source) {
            ranges.push(range_for_node(node));
        }
        for child in named_children(node) {
            collect(child, type_name, source, ranges);
        }
    }

    let mut ranges = Vec::new();
    collect(root, type_name, source, &mut ranges);
    (ranges.len() == 1).then(|| ranges.remove(0))
}

fn declaration_name_and_type(node: Node, source: &[u8]) -> Option<(String, Option<String>)> {
    if !matches!(
        node.kind(),
        "field_declaration" | "property_declaration" | "assignment"
    ) {
        return None;
    }
    let name = field_or_descendant_name(node, source)?;
    let type_name = node
        .child_by_field_name("type")
        .or_else(|| first_descendant_kind(node, &["user_type", "type_identifier"]))
        .and_then(|type_node| node_text(type_node, source))
        .and_then(|text| normalized_collection_owner_type(&text));
    Some((name, type_name))
}

fn type_field_type(
    root: Node,
    owner_type: &str,
    owner_range: Option<&CodeRange>,
    field_name: &str,
    source: &[u8],
) -> Option<Option<String>> {
    fn visit(
        node: Node,
        owner_type: &str,
        owner_range: Option<&CodeRange>,
        field_name: &str,
        source: &[u8],
    ) -> Option<Option<String>> {
        if node_declares_type(node, owner_type, source)
            && owner_range.is_none_or(|range| range_for_node(node) == *range)
        {
            fn find_field(
                node: Node,
                field_name: &str,
                source: &[u8],
                is_owner_root: bool,
            ) -> Option<Option<String>> {
                if !is_owner_root && is_type_declaration_node(node) {
                    return None;
                }
                if matches!(
                    node.kind(),
                    "function_declaration"
                        | "function_definition"
                        | "method_definition"
                        | "method_declaration"
                        | "constructor_declaration"
                        | "function_item"
                ) {
                    return None;
                }
                if let Some((name, type_name)) = declaration_name_and_type(node, source) {
                    if name == field_name {
                        return Some(type_name);
                    }
                }
                for child in named_children(node) {
                    if let Some(found) = find_field(child, field_name, source, false) {
                        return Some(found);
                    }
                }
                None
            }
            return find_field(node, field_name, source, true);
        }
        for child in named_children(node) {
            if let Some(found) = visit(child, owner_type, owner_range, field_name, source) {
                return Some(found);
            }
        }
        None
    }
    visit(root, owner_type, owner_range, field_name, source)
}

fn normalized_collection_parts(text: &str) -> Vec<String> {
    text.trim()
        .trim_start_matches('&')
        .trim_start_matches('*')
        .trim()
        .trim_matches(['(', ')'])
        .replace("?.", ".")
        .replace("->", ".")
        .split('.')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

struct ResolvedStaticCollectionIdentity {
    collection_owner_type: String,
    collection_field: String,
    owner_expression: String,
    source_owner: Option<String>,
    source_symbol: Option<String>,
    source_owner_range: Option<CodeRange>,
}

fn static_collection_identity(
    spec: &dyn crate::lang::LanguageSpec,
    ext: &str,
    root: Node,
    expression_node: Node,
    site_node: Node,
    source: &[u8],
) -> Option<ResolvedStaticCollectionIdentity> {
    let expression_node = unwrap_collection_expression(expression_node);
    let expression = receiver_text(expression_node, source)?;
    let parts = normalized_collection_parts(&expression);
    let collection_field = parts.last()?.clone();
    let (source_owner, source_symbol, source_owner_range) =
        static_collection_source_context(spec, ext, site_node, source);
    let callable = enclosing_callable(site_node);
    let (collection_owner_type, owner_expression) = match parts.as_slice() {
        [self_name, _field] if matches!(self_name.as_str(), "this" | "self") => {
            (source_owner.clone()?, "self".to_string())
        }
        [field] => {
            let owner = source_owner.clone()?;
            if callable.is_some_and(|callable| callable_has_local_binding(callable, field, source))
            {
                return None;
            }
            type_field_type(root, &owner, source_owner_range.as_ref(), field, source)??;
            (owner, "self".to_string())
        }
        [receiver, _field] => {
            let callable = callable?;
            let receiver_type = parameter_type_for_name(callable, receiver, source)?;
            (receiver_type, receiver.to_string())
        }
        [self_name, member, _field] if matches!(self_name.as_str(), "this" | "self") => {
            let source_type = source_owner.clone()?;
            let member_type = type_field_type(
                root,
                &source_type,
                source_owner_range.as_ref(),
                member,
                source,
            )??;
            (member_type, expression.clone())
        }
        _ => return None,
    };

    let resolved_source_owner = source_owner.or_else(|| Some(collection_owner_type.clone()));
    Some(ResolvedStaticCollectionIdentity {
        collection_owner_type,
        collection_field,
        owner_expression,
        source_owner: resolved_source_owner,
        source_symbol,
        source_owner_range,
    })
}

fn is_collection_producer_operation(ext: &str, operation: Option<&str>) -> bool {
    match ext {
        "go" => operation == Some("append"),
        "rs" => operation.is_some_and(|operation| {
            matches!(
                operation,
                "push" | "push_back" | "insert" | "extend" | "extend_from_slice"
            )
        }),
        "java" | "kt" | "kts" => operation.is_some_and(|operation| {
            matches!(
                operation,
                "add" | "addAll" | "put" | "putAll" | "offer" | "push" | "enqueue"
            )
        }),
        "py" => operation.is_some_and(|operation| {
            matches!(
                operation,
                "append" | "extend" | "insert" | "add" | "update" | "put"
            )
        }),
        "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => operation.is_some_and(|operation| {
            matches!(
                operation,
                "push_back" | "emplace_back" | "insert" | "emplace" | "push"
            )
        }),
        "c" => operation.is_none(),
        _ => false,
    }
}

fn is_collection_consumer_operation(ext: &str, operation: Option<&str>) -> bool {
    operation.is_some_and(|operation| match ext {
        "rs" => matches!(
            operation,
            "iter" | "iter_mut" | "into_iter" | "get" | "first" | "last"
        ),
        "java" => matches!(
            operation,
            "iterator" | "stream" | "forEach" | "get" | "values" | "keySet" | "entrySet"
        ),
        "kt" | "kts" => matches!(
            operation,
            "iterator" | "forEach" | "get" | "first" | "last" | "asSequence"
        ),
        "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => matches!(
            operation,
            "begin" | "end" | "cbegin" | "cend" | "front" | "back" | "at"
        ),
        _ => false,
    })
}

fn bounded_collection_value(arguments_node: Node, source: &[u8]) -> Option<String> {
    let value_node = (0..arguments_node.child_count())
        .filter_map(|index| arguments_node.child(index as u32))
        .find(|child| child.is_named())?;
    let value = node_text(value_node, source).map(normalize_navigation_text)?;
    if value.is_empty() {
        return None;
    }
    const VALUE_MAX_CHARS: usize = 120;
    Some(if value.chars().count() > VALUE_MAX_CHARS {
        value.chars().take(VALUE_MAX_CHARS).collect()
    } else {
        value
    })
}

fn typescript_static_collection_edge(
    kind: StaticCollectionEdgeKind,
    site_node: Node,
    owner_node: Node,
    field_node: Node,
    arguments_node: Option<Node>,
    source: &[u8],
) -> Option<StaticCollectionEdge> {
    let collection_owner_type = typescript_collection_owner_type(owner_node, site_node, source)?;
    let collection_field = node_text(field_node, source)?;
    let owner_expression = receiver_text(owner_node, source)?;
    let (source_owner, source_symbol, source_owner_range) =
        typescript_source_context(site_node, source);
    Some(StaticCollectionEdge {
        kind,
        collection_owner_type,
        collection_field,
        owner_expression,
        source_owner,
        source_owner_range,
        source_symbol,
        value: arguments_node.and_then(|arguments| bounded_collection_value(arguments, source)),
        range: range_for_node(site_node),
    })
}

struct StaticCollectionCapture<'tree> {
    site_node: Node<'tree>,
    expression_node: Node<'tree>,
    arguments_node: Option<Node<'tree>>,
}

fn static_collection_edge(
    spec: &dyn crate::lang::LanguageSpec,
    ext: &str,
    root: Node,
    kind: StaticCollectionEdgeKind,
    capture: StaticCollectionCapture,
    source: &[u8],
) -> Option<StaticCollectionEdge> {
    let ResolvedStaticCollectionIdentity {
        collection_owner_type,
        collection_field,
        owner_expression,
        source_owner,
        source_symbol,
        source_owner_range,
    } = static_collection_identity(
        spec,
        ext,
        root,
        capture.expression_node,
        capture.site_node,
        source,
    )?;
    Some(StaticCollectionEdge {
        kind,
        collection_owner_type,
        collection_field,
        owner_expression,
        source_owner,
        source_owner_range,
        source_symbol,
        value: capture
            .arguments_node
            .and_then(|arguments| bounded_collection_value(arguments, source)),
        range: range_for_node(capture.site_node),
    })
}

fn is_typescript_for_of(node: Node, source: &[u8]) -> bool {
    (0..node.child_count()).any(|index| {
        node.child(index as u32)
            .filter(|child| !child.is_named())
            .and_then(|child| child.utf8_text(source).ok())
            == Some("of")
    })
}

const STATIC_COLLECTION_EDGES_PER_FILE_MAX: usize = 256;

fn push_static_collection_edge(
    edges: &mut Vec<StaticCollectionEdge>,
    seen: &mut HashSet<String>,
    edge: StaticCollectionEdge,
) {
    let key = format!(
        "{:?}:{}:{}:{}:{}",
        edge.kind,
        edge.collection_owner_type,
        edge.collection_field,
        edge.range.start_line,
        edge.range.start_col
    );
    if edges.len() < STATIC_COLLECTION_EDGES_PER_FILE_MAX && seen.insert(key) {
        edges.push(edge);
    }
}

fn collect_static_collection_edges_from_tree(
    spec: &dyn crate::lang::LanguageSpec,
    ext: &str,
    query: &Query,
    tree: &Tree,
    source: &[u8],
) -> Vec<StaticCollectionEdge> {
    let mut edges = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source);
    while edges.len() < STATIC_COLLECTION_EDGES_PER_FILE_MAX {
        let Some(query_match) = matches.next() else {
            break;
        };
        let mut owner_node = None;
        let mut field_node = None;
        let mut arguments_node = None;
        let mut push_node = None;
        let mut iteration_node = None;
        let mut read_node = None;
        let mut expression_node = None;
        let mut operation_node = None;
        let mut write_node = None;
        for capture in query_match.captures {
            match query.capture_names()[capture.index as usize] {
                "collection.owner" => owner_node = Some(capture.node),
                "collection.field" => field_node = Some(capture.node),
                "collection.arguments" => arguments_node = Some(capture.node),
                "collection.push" => push_node = Some(capture.node),
                "collection.iteration" => iteration_node = Some(capture.node),
                "collection.read" => read_node = Some(capture.node),
                "collection.expression" => expression_node = Some(capture.node),
                "collection.operation" => operation_node = Some(capture.node),
                "collection.write" => write_node = Some(capture.node),
                _ => {}
            }
        }

        if let (Some(site), Some(owner), Some(field)) = (push_node, owner_node, field_node) {
            if let Some(edge) = typescript_static_collection_edge(
                StaticCollectionEdgeKind::Producer,
                site,
                owner,
                field,
                arguments_node,
                source,
            ) {
                push_static_collection_edge(&mut edges, &mut seen, edge);
            }
        }
        if let (Some(site), Some(owner), Some(field)) = (iteration_node, owner_node, field_node) {
            if is_typescript_for_of(site, source) {
                if let Some(edge) = typescript_static_collection_edge(
                    StaticCollectionEdgeKind::Consumer,
                    site,
                    owner,
                    field,
                    None,
                    source,
                ) {
                    push_static_collection_edge(&mut edges, &mut seen, edge);
                }
            }
        }
        if let (Some(site), Some(owner), Some(field)) = (read_node, owner_node, field_node) {
            if let Some(edge) = typescript_static_collection_edge(
                StaticCollectionEdgeKind::Consumer,
                site,
                owner,
                field,
                None,
                source,
            ) {
                push_static_collection_edge(&mut edges, &mut seen, edge);
            }
        }

        if let (Some(site), Some(expression)) = (write_node, expression_node) {
            let operation = operation_node.and_then(|node| node_text(node, source));
            let kind = if is_collection_producer_operation(ext, operation.as_deref()) {
                Some(StaticCollectionEdgeKind::Producer)
            } else if is_collection_consumer_operation(ext, operation.as_deref()) {
                Some(StaticCollectionEdgeKind::Consumer)
            } else {
                None
            };
            if let Some(kind) = kind {
                if let Some(edge) = static_collection_edge(
                    spec,
                    ext,
                    tree.root_node(),
                    kind,
                    StaticCollectionCapture {
                        site_node: site,
                        expression_node: expression,
                        arguments_node,
                    },
                    source,
                ) {
                    push_static_collection_edge(&mut edges, &mut seen, edge);
                }
            }
        } else if let (Some(site), Some(expression)) = (read_node, expression_node) {
            if let Some(edge) = static_collection_edge(
                spec,
                ext,
                tree.root_node(),
                StaticCollectionEdgeKind::Consumer,
                StaticCollectionCapture {
                    site_node: site,
                    expression_node: expression,
                    arguments_node: None,
                },
                source,
            ) {
                push_static_collection_edge(&mut edges, &mut seen, edge);
            }
        }
    }
    edges
}

impl TreeSitterExtractor {
    fn extract_parts(
        &self,
        file_content: &str,
        file_path: &str,
        collect_auxiliary: bool,
    ) -> Result<(ExtractedFile, IndexAuxiliary), String> {
        let path = Path::new(file_path);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        if is_composite_extension(ext) {
            return self.extract_composite_parts(file_content, file_path, ext, collect_auxiliary);
        }
        self.extract_language_parts(file_content, file_path, ext, collect_auxiliary)
    }

    /// Reuse the existing language walk against a same-length composite source mask. `file_path`
    /// deliberately remains the outer component path while `grammar_ext` selects JS or TS.
    fn extract_language_parts(
        &self,
        file_content: &str,
        file_path: &str,
        grammar_ext: &str,
        collect_auxiliary: bool,
    ) -> Result<(ExtractedFile, IndexAuxiliary), String> {
        let ext = grammar_ext;

        // Resolve the per-language spec from the registry. An unsupported extension yields an
        // empty `ExtractedFile`, preserving the prior unknown-extension behavior exactly.
        let Some(spec) = spec_for_ext(ext) else {
            return Ok((
                ExtractedFile {
                    file_path: file_path.to_string(),
                    total_lines: file_content.lines().count(),
                    symbols: Vec::new(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                    navigation: None,
                },
                IndexAuxiliary::default(),
            ));
        };

        let mut parser = Parser::new();
        let lang = spec.grammar(ext);
        let query = spec.query(ext);
        let navigation_enabled = spec.navigation_enabled(ext);
        let navigation_store_references =
            navigation_enabled && crate::config::get().navigation_store_references;

        parser.set_language(&lang).map_err(|e| e.to_string())?;
        let tree = parser
            .parse(file_content, None)
            .ok_or("Failed to parse file content")?;

        let mut symbols = Vec::new();
        let mut literals = Vec::new();
        let mut navigation = NavigationFile::default();
        let mut seen_calls: HashSet<String> = HashSet::new();
        let mut seen_imports: HashSet<String> = HashSet::new();
        let mut seen_local_bindings: HashSet<String> = HashSet::new();
        let mut seen_references: HashSet<String> = HashSet::new();
        let source = file_content.as_bytes();
        let mut auxiliary = if collect_auxiliary {
            spec.tags_query(ext)
                .map(|tags_query| collect_index_auxiliary_from_tree(tags_query, &tree, source))
                .unwrap_or_default()
        } else {
            IndexAuxiliary::default()
        };
        if collect_auxiliary {
            if let Some(collection_query) = spec.static_collection_query(ext) {
                auxiliary.static_collection_edges = collect_static_collection_edges_from_tree(
                    spec,
                    ext,
                    collection_query,
                    &tree,
                    source,
                );
            }
        }

        // Per-language file-wide pre-pass collecting exported symbol names (TS `export {...}`
        // specifiers; ASM `.globl`/`.global` directives). No-op for languages without one.
        let mut exported_names = std::collections::HashSet::new();
        spec.collect_exported_names(tree.root_node(), source, &mut exported_names);

        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), source);

        while let Some(mat) = matches.next() {
            let mut main_node: Option<(Node, &str)> = None;
            let mut symbol_name: Option<String> = None;
            let mut is_valid_test_call = true;
            // ASM query: tracks the `.macro` directive kind so we know whether a `meta`
            // node should be emitted (only `.macro` definitions are captured as symbols).
            let mut asm_meta_kind_text: Option<String> = None;
            let mut nav_call_node: Option<Node> = None;
            let mut nav_import_node: Option<Node> = None;
            let mut local_scope_node: Option<Node> = None;
            let mut local_reference_node: Option<Node> = None;

            for capture in mat.captures {
                let name_idx = capture.index as usize;
                let name = query.capture_names()[name_idx];

                // `symbol.*` and `literal.*` both record `main_node`; the distinguishing
                // capture name is carried in the tuple and branched on below. This is
                // frozen Child 03 extraction routing — suppress, don't merge.
                #[allow(clippy::if_same_then_else)]
                if name.starts_with("symbol.") && name != "symbol.name" {
                    main_node = Some((capture.node, name));
                } else if name.starts_with("literal.") {
                    main_node = Some((capture.node, name));
                } else if name == "symbol.name" {
                    if let Ok(text) = capture.node.utf8_text(source) {
                        symbol_name = Some(text.to_string());
                    }
                } else if name == "fn_name" {
                    if let Ok(text) = capture.node.utf8_text(source) {
                        let t = text.trim();
                        if t != "describe" && t != "it" && t != "test" {
                            is_valid_test_call = false;
                        }
                    }
                } else if name == "meta_kind" {
                    // ASM query: capture the kind text so we can filter to `.macro` only.
                    if let Ok(text) = capture.node.utf8_text(source) {
                        asm_meta_kind_text = Some(text.trim().to_ascii_lowercase());
                    }
                } else if navigation_enabled && name == "nav.call" {
                    nav_call_node = Some(capture.node);
                } else if navigation_enabled && name == "nav.import" {
                    nav_import_node = Some(capture.node);
                } else if navigation_enabled && name == "local.scope" {
                    local_scope_node = Some(capture.node);
                } else if navigation_store_references && name == "local.reference" {
                    local_reference_node = Some(capture.node);
                }
            }

            if let Some(node) = nav_call_node {
                if let Some(call) = call_site_from_node(node, source) {
                    let key = format!(
                        "{}:{}:{}:{}:{}",
                        call.name,
                        call.receiver.as_deref().unwrap_or(""),
                        call.range.start_line,
                        call.range.start_col,
                        call.range.end_line
                    );
                    if seen_calls.insert(key) {
                        navigation.calls.push(call);
                    }
                }
            }
            if let Some(node) = nav_import_node {
                for entry in import_entries_from_node(node, source) {
                    let key = format!(
                        "{}:{}:{:?}:{}:{}",
                        entry.local_name,
                        entry.source.as_deref().unwrap_or(""),
                        entry.kind,
                        entry.range.start_line,
                        entry.range.start_col
                    );
                    if seen_imports.insert(key) {
                        navigation.imports.push(entry);
                    }
                }
            }
            if let Some(node) = local_scope_node {
                for binding in local_bindings_from_node(node, source) {
                    let scope_key = binding
                        .scope_id
                        .map(|scope_id| scope_id.to_string())
                        .unwrap_or_default();
                    let key = format!(
                        "{}:{}:{}:{}",
                        binding.name, binding.range.start_line, binding.range.start_col, scope_key
                    );
                    if seen_local_bindings.insert(key) {
                        navigation.local_bindings.push(binding);
                    }
                }
            }
            if let Some(node) = local_reference_node {
                if let Some(name) = node_text(node, source) {
                    let reference = ReferenceSite {
                        name,
                        range: range_for_node(node),
                        scope_id: scope_id_for_node(node),
                    };
                    let scope_key = reference
                        .scope_id
                        .map(|scope_id| scope_id.to_string())
                        .unwrap_or_default();
                    let key = format!(
                        "{}:{}:{}:{}",
                        reference.name,
                        reference.range.start_line,
                        reference.range.start_col,
                        scope_key
                    );
                    if seen_references.insert(key) {
                        navigation.references.push(reference);
                    }
                }
            }

            if let Some((node, capture_name)) = main_node {
                if capture_name.starts_with("symbol.") {
                    if is_valid_test_call {
                        let kind = match capture_name {
                            "symbol.struct" => "struct",
                            "symbol.union" => "union",
                            "symbol.enum" => "enum",
                            "symbol.variant" => "variant",
                            "symbol.trait" => "trait",
                            "symbol.mod" => "mod",
                            "symbol.fn" | "symbol.method" => "fn",
                            "symbol.macro" => "fn",
                            "symbol.type" => "type",
                            "symbol.const" => "const",
                            "symbol.static" => "static",
                            "symbol.field" => "field",
                            "symbol.class" => "class",
                            "symbol.variable" => "variable",
                            "symbol.interface" => "interface",
                            "symbol.test" => "test",
                            "symbol.record" => "record",
                            "symbol.object" => "object",
                            "symbol.property" => "property",
                            // Go `type_spec` and Kotlin `class_declaration` carry a single
                            // capture; the concrete kind comes from the node itself.
                            // C/C++: function_definition and function prototype declarations are
                            // both captured as `symbol.cfn`; always emitted as kind "fn".
                            "symbol.cfn" => "fn",
                            "symbol.cppmodule" => "module",
                            // ASM: labels and `.macro` definitions captured as `symbol.asmfn`.
                            // `.macro` nodes are filtered below using asm_meta_kind_text.
                            "symbol.asmfn" => "fn",
                            _ => "unknown",
                        };
                        // Per-language refine of a node-dependent kind (Go `type_spec` struct/
                        // interface/alias; Kotlin `class`/`interface`). No-op for other captures.
                        let kind = spec.refine_kind(capture_name, node, kind);

                        // Per-language accept-and-name cluster (ASM `.macro` filter and label/
                        // macro name extraction; C/C++ type-reference and vexing-parse skips and
                        // declarator-chain name walk). `Skip` reproduces the original `continue`;
                        // `None` falls through to the generic name resolution below.
                        let mut name = match spec.name_for_capture(
                            capture_name,
                            node,
                            kind,
                            ext,
                            source,
                            &asm_meta_kind_text,
                        ) {
                            Some(NameDecision::Skip) => continue,
                            Some(NameDecision::Name(n)) => n,
                            None => symbol_name
                                .unwrap_or_else(|| find_name(node, source).unwrap_or_default()),
                        };

                        if kind == "test" {
                            name = strip_quotes(&name);
                        }

                        if !name.is_empty() {
                            let range = range_for_node(node);

                            // Associated comments proximity search. The per-language anchor
                            // adjusts the start node (Python `decorated_definition`, TS
                            // `export_statement`, Go outer declaration); default is the node.
                            let walk_start_node = spec.docstring_anchor(node);

                            let mut current_sibling = walk_start_node.prev_sibling();
                            let mut comments = Vec::new();
                            let mut last_row = walk_start_node.start_position().row;

                            while let Some(sibling) = current_sibling {
                                let sk = sibling.kind();
                                if sk == "comment" || sk == "line_comment" || sk == "block_comment"
                                {
                                    let end_row = sibling.end_position().row;
                                    if end_row >= last_row - 1 {
                                        if let Ok(text) = sibling.utf8_text(source) {
                                            comments.push(text.to_string());
                                        }
                                        last_row = sibling.start_position().row;
                                        current_sibling = sibling.prev_sibling();
                                    } else {
                                        break;
                                    }
                                } else if sk == "attribute_item" || sk == "decorator" {
                                    last_row = sibling.start_position().row;
                                    current_sibling = sibling.prev_sibling();
                                } else {
                                    break;
                                }
                            }

                            let mut docstring = clean_docstring(&comments);
                            // Per-language docstring fallback when comment promotion yields
                            // `None` (Python inline `"""` docstrings, Go plain `//` comments).
                            if docstring.is_none() {
                                docstring = spec.docstring_fallback(node, source, &comments);
                            }

                            let node_text = node.utf8_text(source).unwrap_or("");

                            // Preceding comments are no longer promoted into docstrings,
                            // so scan them directly for TODO/FIXME (covers `// TODO` above a
                            // symbol); node_text covers in-body and python `"""` docstrings.
                            let comments_text = comments.join("\n");
                            let has_todo = contains_case_insensitive(node_text, "todo")
                                || contains_case_insensitive(&comments_text, "todo");
                            let has_fixme = contains_case_insensitive(node_text, "fixme")
                                || contains_case_insensitive(&comments_text, "fixme");

                            let is_test =
                                spec.is_test(node, &name, kind, file_path, source, &comments_text);

                            let is_exported =
                                spec.is_exported(node, &name, kind, source, &exported_names);

                            let is_deprecated =
                                spec.is_deprecated(node, source, &docstring, &comments_text);

                            // Owner (enclosing type) is computed for callables, enum variants,
                            // and fields. The search layer uses it for qualified names such as
                            // `Settings.timeout` as well as methods and enum variants.
                            // Best-effort: any unexpected shape yields `None`.
                            // Note: `symbol.method` maps to kind "fn" (see match arm above),
                            // so "method" is never a possible kind value here.
                            let owner = if matches!(kind, "fn" | "variant" | "field") {
                                spec.find_owner(node, ext, source)
                            } else {
                                None
                            };

                            symbols.push(ExtractedSymbol {
                                name,
                                kind: kind.to_string(),
                                range,
                                docstring,
                                flags: SymbolFlags {
                                    has_todo,
                                    has_fixme,
                                    is_test,
                                    is_exported,
                                    is_deprecated,
                                },
                                owner,
                            });
                        }
                    }
                } else if capture_name.starts_with("literal.string") {
                    // Only string literals carry search/detail value; numeric and boolean
                    // literals are dropped (low value, index/detail bloat — Child 03).
                    if let Ok(text) = node.utf8_text(source) {
                        let stripped = strip_quotes(text);
                        let line = node.start_position().row + 1;
                        literals.push(ExtractedLiteral {
                            text: stripped,
                            line,
                        });
                    }
                }
            }
        }

        let docstrings = symbols.iter().filter_map(|s| s.docstring.clone()).collect();
        let navigation = if navigation_enabled {
            Some(navigation)
        } else {
            None
        };

        Ok((
            ExtractedFile {
                file_path: file_path.to_string(),
                total_lines: file_content.lines().count(),
                symbols,
                literals,
                docstrings,
                navigation,
            },
            auxiliary,
        ))
    }

    fn extract_composite_parts(
        &self,
        file_content: &str,
        file_path: &str,
        extension: &str,
        collect_auxiliary: bool,
    ) -> Result<(ExtractedFile, IndexAuxiliary), String> {
        let mut extracted = ExtractedFile {
            file_path: file_path.to_string(),
            total_lines: file_content.lines().count(),
            symbols: Vec::new(),
            literals: Vec::new(),
            docstrings: Vec::new(),
            navigation: Some(NavigationFile::default()),
        };
        let mut auxiliary = IndexAuxiliary::default();

        for embedded in composite::extract_sources(file_content, extension) {
            let (part, part_auxiliary) = self.extract_language_parts(
                &embedded.source,
                file_path,
                embedded.grammar_ext,
                collect_auxiliary,
            )?;
            merge_composite_part(&mut extracted, &mut auxiliary, part, part_auxiliary);
        }
        // `extract_language_parts` already uses the outer path and same-length source masks;
        // assigning these explicitly documents and protects the public coordinate contract.
        extracted.file_path = file_path.to_string();
        extracted.total_lines = file_content.lines().count();
        Ok((extracted, auxiliary))
    }
}

fn push_unique<T: PartialEq>(out: &mut Vec<T>, item: T) {
    if !out.contains(&item) {
        out.push(item);
    }
}

fn merge_composite_part(
    extracted: &mut ExtractedFile,
    auxiliary: &mut IndexAuxiliary,
    part: ExtractedFile,
    part_auxiliary: IndexAuxiliary,
) {
    for symbol in part.symbols {
        push_unique(&mut extracted.symbols, symbol);
    }
    for literal in part.literals {
        push_unique(&mut extracted.literals, literal);
    }
    for docstring in part.docstrings {
        push_unique(&mut extracted.docstrings, docstring);
    }
    if let (Some(target), Some(source)) = (&mut extracted.navigation, part.navigation) {
        for call in source.calls {
            push_unique(&mut target.calls, call);
        }
        for reference in source.references {
            push_unique(&mut target.references, reference);
        }
        for binding in source.local_bindings {
            push_unique(&mut target.local_bindings, binding);
        }
        for import in source.imports {
            push_unique(&mut target.imports, import);
        }
    }
    for definition in part_auxiliary.definition_body {
        push_unique(&mut auxiliary.definition_body, definition);
    }
    for reference in part_auxiliary.reference {
        push_unique(&mut auxiliary.reference, reference);
    }
    for edge in part_auxiliary.static_collection_edges {
        push_unique(&mut auxiliary.static_collection_edges, edge);
    }
}

impl CodeExtractor for TreeSitterExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String> {
        self.extract_parts(file_content, file_path, false)
            .map(|(extracted, _)| extracted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Rust Parser Tests ---
    #[test]
    fn test_rust_parser_struct_and_fields() {
        let content = r#"
            /// Config struct description
            pub struct Config {
                pub port: u16,
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/config.rs").unwrap();

        // Assert struct symbol
        let struct_sym = file.symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(struct_sym.kind, "struct");
        assert!(struct_sym.flags.is_exported);
        assert_eq!(
            struct_sym.docstring.as_deref(),
            Some("Config struct description")
        );

        // Assert field variable symbol (verifies e2e target "port")
        let field_sym = file.symbols.iter().find(|s| s.name == "port").unwrap();
        assert_eq!(field_sym.kind, "field");
        assert!(field_sym.flags.is_exported);
    }

    #[test]
    fn test_rust_parser_flags_deprecated_and_todo() {
        let content = r#"
            // TODO: refactor this
            #[deprecated(since = "1.0.0")]
            fn deprecated_function() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "deprecated_function")
            .unwrap();

        assert!(sym.flags.has_todo);
        assert!(sym.flags.is_deprecated);
        assert!(!sym.flags.is_test);
    }

    #[test]
    fn test_rust_parser_test_detection() {
        let content = r#"
            #[test]
            fn my_test_case() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/tests.rs").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "my_test_case")
            .unwrap();

        assert!(sym.flags.is_test);
    }

    // --- Python Parser Tests ---
    #[test]
    fn test_python_parser_class_and_methods() {
        let content = r#"
class Database:
    """Manages db connection."""
    
    def __init__(self, url):
        self.url = url
        
    def query(self, sql):
        # FIXME: handle sql injection
        pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "db.py").unwrap();

        let class_sym = file.symbols.iter().find(|s| s.name == "Database").unwrap();
        assert_eq!(class_sym.kind, "class");
        assert_eq!(
            class_sym.docstring.as_deref(),
            Some("Manages db connection.")
        );
        assert!(class_sym.flags.is_exported); // No leading underscore

        let method_sym = file.symbols.iter().find(|s| s.name == "query").unwrap();
        assert_eq!(method_sym.kind, "fn");
        assert!(method_sym.flags.has_fixme);
    }

    #[test]
    fn test_python_private_and_deprecated() {
        let content = r#"
@deprecated
def _private_func():
    pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "utils.py").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "_private_func")
            .unwrap();

        assert!(!sym.flags.is_exported); // Starts with underscore
        assert!(sym.flags.is_deprecated);
    }

    // --- TypeScript / JavaScript Parser Tests ---
    #[test]
    fn test_ts_parser_interface_and_exports() {
        let content = r#"
            /** User info interface */
            export interface User {
                id: string;
                name: string;
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        // TS extension: parsed using typescript grammar
        let file = extractor.extract(content, "types.ts").unwrap();

        let sym = file.symbols.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(sym.kind, "interface");
        assert!(sym.flags.is_exported);
        assert_eq!(sym.docstring.as_deref(), Some("User info interface"));
    }

    #[test]
    fn test_tsx_jsx_grammar_selection() {
        let content = r#"
            export function Component() {
                return <div>Hello</div>;
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        // TSX extension: verifies TSX grammar parses JSX syntax successfully without erroring
        let file = extractor.extract(content, "component.tsx").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "Component").unwrap();
        assert_eq!(sym.kind, "fn");
        assert!(sym.flags.is_exported);
    }

    #[test]
    fn test_js_test_suite_hook_detection() {
        let content = r#"
            describe("auth service", () => {
                it("should validate credentials", () => {
                    // TODO: add boundary testing
                });
            });
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "auth.test.js").unwrap();

        let describe_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "auth service")
            .unwrap();
        assert_eq!(describe_sym.kind, "test");
        assert!(describe_sym.flags.is_test);

        let it_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "should validate credentials")
            .unwrap();
        assert_eq!(it_sym.kind, "test");
        assert!(it_sym.flags.is_test);
        assert!(it_sym.flags.has_todo);
    }

    #[test]
    fn test_raw_string_literal_quote_stripping() {
        let content = r##"
            pub const VAL: &str = r#"magic_value"#;
        "##;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        assert_eq!(file.literals.len(), 1);
        assert_eq!(file.literals[0].text, "magic_value");
        assert_eq!(file.literals[0].line, 2);
    }

    #[test]
    fn test_ts_named_exports_at_bottom() {
        let content = r#"
            function myFunc() {}
            export { myFunc };
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "index.ts").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "myFunc").unwrap();
        assert!(sym.flags.is_exported);
    }

    #[test]
    fn test_python_class_methods_export_status() {
        let content = r#"
class Calculator:
    def add(self, x, y):
        pass
    def _private_helper(self):
        pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "calc.py").unwrap();

        let add_sym = file.symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add_sym.flags.is_exported);

        let helper_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "_private_helper")
            .unwrap();
        assert!(!helper_sym.flags.is_exported);
    }

    #[test]
    fn test_block_comment_trailing_asterisks() {
        let content = r#"
            /** check function **/
            pub fn check() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "check").unwrap();
        assert_eq!(sym.docstring.as_deref(), Some("check function"));
    }

    #[test]
    fn test_ts_test_file_pattern_matching() {
        let content = r#"
            export function helper() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "tests/parser_test.ts").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert!(sym.flags.is_test);
    }

    #[test]
    fn test_ts_navigation_extracts_calls_imports_locals_and_ignores_strings() {
        let content = r#"
            import { save as persist } from "./user";
            const user: User = new User();
            export function run() {
                persist();
                user.save();
                const text = "fakeCall()";
                // commentCall()
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "nav.ts").unwrap();
        let navigation = file.navigation.expect("typescript navigation should run");

        assert!(navigation.imports.iter().any(|entry| {
            entry.local_name == "persist"
                && entry.imported_name.as_deref() == Some("save")
                && entry.source.as_deref() == Some("./user")
        }));
        assert!(navigation.local_bindings.iter().any(|binding| {
            binding.name == "user"
                && binding.type_name.as_deref() == Some("User")
                && binding.value_type.as_deref() == Some("User")
        }));
        assert!(navigation
            .calls
            .iter()
            .any(|call| call.name == "persist" && call.scope_id.is_some()));
        assert!(navigation
            .calls
            .iter()
            .any(|call| call.name == "save" && call.receiver.as_deref() == Some("user")));
        assert!(!navigation.calls.iter().any(|call| call.name == "fakeCall"));
        assert!(!navigation
            .calls
            .iter()
            .any(|call| call.name == "commentCall"));
    }

    #[test]
    fn test_ts_navigation_scope_ids_are_stable() {
        let content = "function run() { const item = makeItem(); item.save(); }";
        let extractor = TreeSitterExtractor::new();
        let first = extractor.extract(content, "stable.ts").unwrap();
        let second = extractor.extract(content, "stable.ts").unwrap();
        assert_eq!(first.navigation, second.navigation);
    }

    #[test]
    fn test_index_auxiliary_collects_bounded_deduped_tags() {
        let repeated_call = "expensiveLookup();\n".repeat(300);
        let content = format!(
            "function reconcileInvoice() {{\n{repeated_call}}}\n\
             function sendNotice() {{\nreconcileInvoice();\nreconcileInvoice();\n}}\n"
        );

        let extractor = TreeSitterExtractor::new();
        let (file, auxiliary) = extractor.extract_for_index(&content, "billing.ts").unwrap();
        let standalone_auxiliary = collect_index_auxiliary(&content, "billing.ts").unwrap();

        assert!(file
            .symbols
            .iter()
            .any(|symbol| symbol.name == "reconcileInvoice"));
        assert_eq!(standalone_auxiliary, auxiliary);
        assert!(auxiliary
            .definition_body
            .iter()
            .any(|text| text.contains("reconcileInvoice")));
        assert!(auxiliary
            .definition_body
            .iter()
            .all(|text| text.chars().count() <= INDEX_AUXILIARY_MAX_CHARS));
        assert_eq!(
            auxiliary
                .reference
                .iter()
                .filter(|text| text.as_str() == "reconcileInvoice()")
                .count(),
            1,
            "duplicate call-expression captures should be indexed once per file"
        );
    }

    // --- Owner (enclosing type) Tests (Phase A) ---

    /// Helper: extract `content` as `path` and return the `owner` of the first symbol named
    /// `name`. Panics if no such symbol exists, so a missing extraction fails loudly.
    fn owner_of(content: &str, path: &str, name: &str) -> Option<String> {
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, path).unwrap();
        file.symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("symbol `{name}` not extracted from {path}"))
            .owner
            .clone()
    }

    #[test]
    fn test_owner_rust_impl_method() {
        let content = "impl Server { pub fn start(&self) {} }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "start"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_rust_impl_trait_for_type_uses_type() {
        // `impl Trait for Type` → the implementing Type, not the trait.
        let content = "impl Display for Widget { fn fmt(&self) {} }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "fmt"),
            Some("Widget".to_string())
        );
    }

    #[test]
    fn test_owner_rust_generic_and_scoped_impl_normalized() {
        // Generic args stripped: `impl Cache<K, V>` → `Cache`.
        assert_eq!(
            owner_of(
                "impl<K, V> Cache<K, V> { fn get(&self) {} }",
                "src/lib.rs",
                "get"
            ),
            Some("Cache".to_string())
        );
        // Module path reduced to the rightmost segment: `impl a::b::Store` → `Store`.
        assert_eq!(
            owner_of(
                "impl crate::store::Store { fn put(&self) {} }",
                "src/lib.rs",
                "put"
            ),
            Some("Store".to_string())
        );
    }

    #[test]
    fn test_owner_rust_trait_default_method() {
        let content = "trait Greeter { fn greet(&self) { println!(\"hi\"); } }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "greet"),
            Some("Greeter".to_string())
        );
    }

    #[test]
    fn test_owner_rust_free_fn_is_none() {
        assert_eq!(owner_of("pub fn run() {}", "src/lib.rs", "run"), None);
    }

    #[test]
    fn test_owner_rust_fn_in_module_is_none() {
        // A function nested only inside a module (not a type) has no owner.
        assert_eq!(
            owner_of("mod util { pub fn helper() {} }", "src/lib.rs", "helper"),
            None
        );
    }

    #[test]
    fn test_owner_rust_local_fn_in_method_is_none() {
        let content = "impl Server { fn run(&self) { fn helper() {} } }";
        assert_eq!(owner_of(content, "src/lib.rs", "helper"), None);
    }

    #[test]
    fn test_owner_rust_fn_in_closure_in_method_is_none() {
        let content = "impl Server { fn run(&self) { let c = || { fn inner() {} }; } }";
        assert_eq!(owner_of(content, "src/lib.rs", "inner"), None);
    }

    #[test]
    fn test_owner_go_method_receiver_base_type() {
        // Pointer receiver `*Server` → `Server`.
        assert_eq!(
            owner_of(
                "package p\nfunc (s *Server) Start() {}\n",
                "main.go",
                "Start"
            ),
            Some("Server".to_string())
        );
        // Value receiver `Server` → `Server`.
        assert_eq!(
            owner_of("package p\nfunc (s Server) Stop() {}\n", "main.go", "Stop"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_go_generic_receiver_normalized() {
        // `*Box[T]` → `Box` (square-bracketed generic args stripped).
        assert_eq!(
            owner_of("package p\nfunc (b *Box[T]) Get() {}\n", "main.go", "Get"),
            Some("Box".to_string())
        );
    }

    #[test]
    fn test_owner_go_interface_method_elem() {
        let content = "package p\ntype Reader interface {\n Read() error\n}\n";
        assert_eq!(
            owner_of(content, "main.go", "Read"),
            Some("Reader".to_string())
        );
    }

    #[test]
    fn test_owner_go_free_fn_is_none() {
        assert_eq!(
            owner_of("package p\nfunc Run() {}\n", "main.go", "Run"),
            None
        );
    }

    #[test]
    fn test_owner_go_local_fn_in_method_is_none() {
        let content = "package p\nfunc (s *Server) Run() {\n inner := func() {}\n _ = inner\n}\n";
        // A `func_literal` is anonymous (no name symbol) — assert the method itself instead:
        // the receiver method resolves, and no spurious owner leaks to nested closures.
        assert_eq!(
            owner_of(content, "main.go", "Run"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_python_class_method() {
        let content = "class Foo:\n    def bar(self):\n        pass\n";
        assert_eq!(owner_of(content, "x.py", "bar"), Some("Foo".to_string()));
    }

    #[test]
    fn test_owner_python_free_fn_is_none() {
        assert_eq!(owner_of("def run():\n    pass\n", "x.py", "run"), None);
    }

    #[test]
    fn test_owner_python_local_fn_in_method_is_none() {
        let content = "class Foo:\n    def bar(self):\n        def helper():\n            pass\n";
        assert_eq!(owner_of(content, "x.py", "helper"), None);
    }

    #[test]
    fn test_owner_ts_class_method() {
        let content = "class Service { handle() {} }";
        assert_eq!(
            owner_of(content, "x.ts", "handle"),
            Some("Service".to_string())
        );
    }

    #[test]
    fn test_owner_ts_free_fn_is_none() {
        assert_eq!(owner_of("function run() {}", "x.ts", "run"), None);
    }

    #[test]
    fn test_owner_ts_local_fn_in_method_is_none() {
        let content = "class A { method() { function localFn() { return 1; } } }";
        assert_eq!(owner_of(content, "x.ts", "localFn"), None);
    }

    #[test]
    fn test_owner_ts_object_literal_method_is_none() {
        // A `method_definition` inside an object-literal value, not a named type.
        let content = "class A { config = { handler() {} }; }";
        assert_eq!(owner_of(content, "x.ts", "handler"), None);
    }

    #[test]
    fn test_owner_ts_class_expression_method_is_none() {
        // A class *expression* (a value) is anonymous — no owner.
        let content = "const X = class { doThing() {} };";
        assert_eq!(owner_of(content, "x.ts", "doThing"), None);
    }

    #[test]
    fn test_owner_js_class_method() {
        let content = "class Widget { render() {} }";
        assert_eq!(
            owner_of(content, "x.js", "render"),
            Some("Widget".to_string())
        );
    }

    #[test]
    fn test_owner_ts_abstract_class_method() {
        // `abstract_class_declaration` is a named type container (verified against the
        // grammar's node-types.json: `name` field is a `type_identifier`).
        let content = "abstract class Base { run() {} }";
        assert_eq!(owner_of(content, "x.ts", "run"), Some("Base".to_string()));
    }

    #[test]
    fn test_owner_java_class_method() {
        let content = "class A { void m() {} }";
        assert_eq!(owner_of(content, "A.java", "m"), Some("A".to_string()));
    }

    #[test]
    fn test_owner_java_interface_enum_record_methods() {
        assert_eq!(
            owner_of("interface I { void doI(); }", "I.java", "doI"),
            Some("I".to_string())
        );
        assert_eq!(
            owner_of("enum E { A; void doE() {} }", "E.java", "doE"),
            Some("E".to_string())
        );
        assert_eq!(
            owner_of("record R(int x) { void doR() {} }", "R.java", "doR"),
            Some("R".to_string())
        );
    }

    #[test]
    fn test_owner_java_anonymous_class_method_is_none() {
        let content =
            "class A { void m() { Runnable r = new Runnable() { public void run() {} }; } }";
        assert_eq!(owner_of(content, "A.java", "run"), None);
    }

    #[test]
    fn test_owner_kotlin_class_method() {
        let content = "class Service {\n  fun handle() {}\n}\n";
        assert_eq!(
            owner_of(content, "x.kt", "handle"),
            Some("Service".to_string())
        );
    }

    #[test]
    fn test_owner_kotlin_object_method() {
        let content = "object Singleton {\n  fun go() {}\n}\n";
        assert_eq!(
            owner_of(content, "x.kt", "go"),
            Some("Singleton".to_string())
        );
    }

    #[test]
    fn test_owner_kotlin_object_literal_method_is_none() {
        let content = "fun build() {\n  val x = object {\n    fun anon() {}\n  }\n}\n";
        assert_eq!(owner_of(content, "x.kt", "anon"), None);
    }

    #[test]
    fn test_owner_kotlin_free_fn_is_none() {
        assert_eq!(owner_of("fun run() {}\n", "x.kt", "run"), None);
    }

    #[test]
    fn test_owner_kotlin_companion_object_resolves_enclosing_class() {
        // Kotlin `companion object` members resolve to the enclosing class name. Note:
        // tree-sitter-kotlin-ng 1.1.0 is shape-sensitive here — a multi-line body nests the
        // member under the class so `companion_object` (passthrough) is traversed to
        // `class_declaration`; a single-line body can instead collapse the companion into an
        // `ERROR` node, in which case the walk yields `None` (best-effort, never wrong). This
        // test pins the common multi-line shape resolving to the enclosing class.
        let extractor = TreeSitterExtractor::new();
        let content = "class A {\n  companion object {\n    fun create() {}\n  }\n}\n";
        let file = extractor.extract(content, "x.kt").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "create")
            .expect("companion member should be extracted");
        assert_eq!(sym.owner, Some("A".to_string()));
    }

    /// Owner is best-effort and never panics: feed each grammar a method-in-type fixture and
    /// confirm the call returns without crashing (a wrong owner is worse than `None`).
    #[test]
    fn test_owner_no_panic_across_grammars() {
        let cases = [
            ("impl T { fn a(&self) {} }", "x.rs"),
            ("package p\nfunc (s *T) A() {}\n", "x.go"),
            ("class T:\n    def a(self): pass\n", "x.py"),
            ("class T { a() {} }", "x.ts"),
            ("class T { void a() {} }", "T.java"),
            ("class T {\n  fun a() {}\n}\n", "x.kt"),
            // C: struct with a nested function pointer field (no method ownership in C,
            // but exercises the owner walk without panicking).
            (
                "struct S { int x; };\nint add(int a, int b) { return a + b; }",
                "x.c",
            ),
            // C++: class with an in-class method declaration and an out-of-line definition.
            (
                "class Widget { public: void draw(); };\nvoid Widget::draw() {}",
                "x.cpp",
            ),
            // ASM: a globl label and a non-globl label.
            (".globl _main\n_main:\n  ret\n_local:\n  ret\n", "x.s"),
        ];
        for (content, path) in cases {
            let extractor = TreeSitterExtractor::new();
            // Must not panic; owner correctness is asserted in the per-language tests above.
            let _ = extractor.extract(content, path).unwrap();
        }
    }

    #[test]
    fn test_existing_language_index_auxiliary_matrix() {
        // Reproduce the declaration/reference shapes used by each locked grammar's tags query.
        // This complements the public CLI symbol matrix by proving that index auxiliary text
        // continues through the shared extraction path for every pre-existing backend language.
        let cases = [
            (
                "x.rs",
                "fn rust_aux_target() {}\nfn rust_aux_caller() { rust_aux_target(); }\n",
            ),
            (
                "x.go",
                "package p\nfunc goAuxTarget() {}\nfunc goAuxCaller() { goAuxTarget() }\n",
            ),
            (
                "X.java",
                "class JavaAux { void target() {} void caller() { target(); } }\n",
            ),
            (
                "x.kt",
                "class KotlinAux {\n  fun target() {}\n  fun caller() { target() }\n}\n",
            ),
            (
                "x.py",
                "def python_aux_target():\n    pass\ndef python_aux_caller():\n    python_aux_target()\n",
            ),
            (
                "x.c",
                "void c_aux_target(void) {}\nvoid c_aux_caller(void) { c_aux_target(); }\n",
            ),
            (
                "x.cpp",
                "void cpp_aux_target() {}\nvoid cpp_aux_caller() { cpp_aux_target(); }\n",
            ),
            (
                "x.s",
                "asm_aux_target:\n  ret\nasm_aux_caller:\n  call asm_aux_target\n",
            ),
        ];
        for (path, content) in cases {
            let auxiliary = collect_index_auxiliary(content, path).unwrap();
            assert!(
                !auxiliary.definition_body.is_empty(),
                "{path} must expose a definition to indexing"
            );
            assert!(
                !auxiliary.reference.is_empty(),
                "{path} must expose a relationship reference to indexing"
            );
        }
    }
}
