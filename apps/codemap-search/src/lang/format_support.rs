//! Shared helpers for query-driven non-programming-language specs.

use tree_sitter::Node;

use crate::parser::{CodeRange, ImportEntry, ImportKind, ReferenceSite};

pub(super) fn text(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").trim().to_string()
}

pub(super) fn clean(node: Node<'_>, source: &[u8]) -> String {
    text(node, source)
        .trim_matches(|character| matches!(character, '"' | '\'' | '`' | '[' | ']'))
        .to_string()
}

pub(super) fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

pub(super) fn first_named(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node).into_iter().next()
}

pub(super) fn descendants<'tree>(node: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
    let mut found = Vec::new();
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if current.kind() == kind {
            found.push(current);
        }
        let mut children = named_children(current);
        children.reverse();
        stack.extend(children);
    }
    found
}

pub(super) fn first_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    descendants(node, kind).into_iter().next()
}

pub(super) fn nearest_ancestor<'tree>(node: Node<'tree>, kinds: &[&str]) -> Node<'tree> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if kinds.contains(&candidate.kind()) {
            return candidate;
        }
        current = candidate.parent();
    }
    node
}

pub(super) fn is_recoverable(boundary: Node<'_>) -> bool {
    if has_recovery_marker(boundary) {
        return false;
    }
    let mut ancestor = boundary.parent();
    while let Some(node) = ancestor {
        if node.parent().is_none() {
            return true;
        }
        if node.is_error() || node.is_missing() {
            return false;
        }
        ancestor = node.parent();
    }
    true
}

fn has_recovery_marker(node: Node<'_>) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    (0..node.child_count()).any(|index| node.child(index as u32).is_some_and(has_recovery_marker))
}

pub(super) fn range(node: Node<'_>) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

pub(super) fn import(node: Node<'_>, source: &[u8]) -> ImportEntry {
    let name = clean(node, source);
    ImportEntry {
        local_name: name.clone(),
        imported_name: None,
        source: Some(name),
        kind: ImportKind::Default,
        range: range(node),
    }
}

pub(super) fn reference(node: Node<'_>, source: &[u8]) -> ReferenceSite {
    ReferenceSite {
        name: clean(node, source),
        range: range(node),
        scope_id: None,
    }
}
