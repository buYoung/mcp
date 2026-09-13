use std::collections::HashSet;
use std::sync::OnceLock;

use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, has_annotation, path_indicates_test, LanguageSpec, NameDecision};
use crate::parser::{CodeRange, ImportEntry, ImportKind, ReferenceSite};

const GROOVY_QUERY_STR: &str = concat!(
    include_str!("../../queries/groovy/symbols.scm"),
    "\n",
    include_str!("../../queries/groovy/navigation.scm"),
    "\n",
    include_str!("../../queries/groovy/references.scm")
);
const GRADLE_QUERY_STR: &str = concat!(
    include_str!("../../queries/groovy/symbols.scm"),
    "\n",
    include_str!("../../queries/groovy/gradle_navigation.scm"),
    "\n",
    include_str!("../../queries/groovy/gradle.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/groovy/tags.scm");

fn query(ext: &str) -> &'static Query {
    static GROOVY_QUERY: OnceLock<Query> = OnceLock::new();
    static GRADLE_QUERY: OnceLock<Query> = OnceLock::new();
    let slot = if ext == "gradle" {
        &GRADLE_QUERY
    } else {
        &GROOVY_QUERY
    };
    slot.get_or_init(|| {
        let source = if ext == "gradle" {
            GRADLE_QUERY_STR
        } else {
            GROOVY_QUERY_STR
        };
        Query::new(&super::bundled_grammars::GROOVY.into(), source)
            .expect("Failed to compile Groovy query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&super::bundled_grammars::GROOVY.into(), TAGS_QUERY_STR)
            .expect("Failed to compile Groovy tags query")
    })
}

fn range_for_node(node: Node<'_>) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn quoted_literals(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let quote = bytes[index];
        if quote != b'\'' && quote != b'"' {
            index += 1;
            continue;
        }
        let start = index + 1;
        index = start;
        while index < bytes.len() && bytes[index] != quote {
            if bytes[index] == b'\\' {
                index += 1;
            }
            index += 1;
        }
        if index < bytes.len() {
            let value = &text[start..index];
            if !value.contains('$') {
                values.push(value.to_string());
            }
        }
        index += 1;
    }
    values
}

fn normalized_invocation(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn quoted_method_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.has_error() {
        return None;
    }
    let text = node.utf8_text(source).ok()?;
    let quote = if text.starts_with("\"\"\"") {
        "\"\"\""
    } else if text.starts_with("'''") {
        "'''"
    } else if text.starts_with('"') {
        "\""
    } else {
        "'"
    };
    let body = text.strip_prefix(quote)?.strip_suffix(quote)?;
    let mut encoded = String::from("\"");
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next()? {
                escaped @ ('\'' | '$') => encoded.push(escaped),
                escaped @ ('"' | '\\' | 'b' | 'f' | 'n' | 'r' | 't' | 'u') => {
                    encoded.push('\\');
                    encoded.push(escaped);
                }
                first @ '0'..='7' => {
                    let mut value = first.to_digit(8)?;
                    for _ in 0..if first <= '3' { 2 } else { 1 } {
                        let Some(next) = chars.peek().and_then(|ch| ch.to_digit(8)) else {
                            break;
                        };
                        value = value * 8 + next;
                        chars.next();
                    }
                    encoded.push_str(&format!("\\u{value:04x}"));
                }
                _ => return None,
            }
        } else {
            // An interpolated name is dynamic, even if grammar recovery accepts it.
            if ch == '$'
                && quote.starts_with('"')
                && chars
                    .peek()
                    .is_some_and(|ch| *ch == '{' || *ch == '_' || ch.is_alphabetic())
            {
                return None;
            }
            let escaped = serde_json::to_string(&ch.to_string()).ok()?;
            encoded.push_str(&escaped[1..escaped.len() - 1]);
        }
    }
    encoded.push('"');
    serde_json::from_str(&encoded).ok()
}

fn gradle_task_name(text: &str) -> Option<String> {
    let normalized = normalized_invocation(text);
    let is_task_declaration = normalized.starts_with("task ")
        || normalized.starts_with("task(")
        || normalized.starts_with("tasks.register")
        || normalized.starts_with("tasks.create");
    is_task_declaration
        .then(|| quoted_literals(&normalized).into_iter().next())
        .flatten()
}

fn is_within_gradle_block(node: Node<'_>, source: &[u8], block_name: &str) -> bool {
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if matches!(current.kind(), "method_invocation" | "juxt_function_call")
            && current.utf8_text(source).is_ok_and(|text| {
                let normalized = normalized_invocation(text);
                normalized.starts_with(&format!("{block_name} "))
                    || normalized.starts_with(&format!("{block_name}("))
                    || normalized.starts_with(&format!("{block_name} {{"))
            })
        {
            return true;
        }
        ancestor = current.parent();
    }
    false
}

fn gradle_references(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let text = node.utf8_text(source).unwrap_or_default();
    let normalized = normalized_invocation(text);
    let literals = quoted_literals(&normalized);
    let mut references = Vec::new();
    if [
        "dependsOn",
        "mustRunAfter",
        "shouldRunAfter",
        "finalizedBy",
        "tasks.named",
    ]
    .iter()
    .any(|name| normalized.contains(name))
    {
        references.extend(literals);
    } else if is_within_gradle_block(node, source, "plugins")
        && (normalized.starts_with("id ") || normalized.starts_with("id("))
        && !literals.is_empty()
    {
        references.push(literals[0].clone());
    } else if is_within_gradle_block(node, source, "dependencies") {
        references.extend(
            literals
                .into_iter()
                .filter(|literal| literal.split(':').count() == 3),
        );
    }
    references
}

fn import_path(text: &str) -> Option<String> {
    let body = text.trim().strip_prefix("import ")?.trim();
    if body.is_empty() || body.contains('$') {
        return None;
    }
    Some(
        body.strip_prefix("static ")
            .unwrap_or(body)
            .trim_end_matches(';')
            .trim_end_matches(".*")
            .to_string(),
    )
}

pub(crate) struct GroovySpec;

impl LanguageSpec for GroovySpec {
    fn language_name(&self) -> &'static str {
        "groovy"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["gradle"]
    }

    fn grammar(&self, _ext: &str) -> Language {
        super::bundled_grammars::GROOVY.into()
    }

    fn query(&self, ext: &str) -> &'static Query {
        query(ext)
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["groovy", "gradle"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn always_store_references(&self, ext: &str) -> bool {
        ext == "gradle"
    }

    fn capture_is_valid(&self, capture_name: &str, node: Node<'_>, source: &[u8]) -> bool {
        if capture_name == "symbol.gradle_target" {
            return node
                .utf8_text(source)
                .ok()
                .and_then(gradle_task_name)
                .is_some();
        }
        true
    }

    fn refine_kind(&self, capture_name: &str, node: Node, kind: &'static str) -> &'static str {
        if capture_name == "symbol.class" {
            let mut cursor = node.walk();
            if node
                .children(&mut cursor)
                .any(|child| child.kind() == "trait")
            {
                return "trait";
            }
        }
        kind
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node<'_>,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        if matches!(capture_name, "symbol.method" | "symbol.fn") {
            let name = node.child_by_field_name("name")?;
            if matches!(name.kind(), "string_literal" | "character_literal") {
                return Some(
                    quoted_method_name(name, source)
                        .map(NameDecision::Name)
                        .unwrap_or(NameDecision::Skip),
                );
            }
        }
        (capture_name == "symbol.gradle_target").then(|| {
            node.utf8_text(source)
                .ok()
                .and_then(gradle_task_name)
                .map(NameDecision::Name)
                .unwrap_or(NameDecision::Skip)
        })
    }

    fn symbol_kind_for_capture(
        &self,
        capture_name: &str,
        node: Node<'_>,
        source: &[u8],
        default_kind: &'static str,
    ) -> String {
        if capture_name == "symbol.gradle_target" {
            "target".to_string()
        } else if capture_name == "symbol.class"
            && node
                .utf8_text(source)
                .is_ok_and(|text| text.trim_start().starts_with("trait "))
        {
            "trait".to_string()
        } else {
            default_kind.to_string()
        }
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let text = node.utf8_text(source).ok()?;
        let Some(path) = import_path(text) else {
            return Some(Vec::new());
        };
        let local_name = path.rsplit('.').next().unwrap_or(&path).to_string();
        Some(vec![ImportEntry {
            local_name,
            imported_name: None,
            source: Some(path),
            kind: ImportKind::Namespace,
            range: range_for_node(node),
        }])
    }

    fn reference_sites_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ReferenceSite>> {
        Some(
            gradle_references(node, source)
                .into_iter()
                .map(|name| ReferenceSite {
                    name,
                    range: range_for_node(node),
                    scope_id: None,
                })
                .collect(),
        )
    }

    fn is_import_line(&self, line: &str) -> bool {
        line.trim_start().starts_with("import ")
    }

    fn is_test(
        &self,
        node: Node<'_>,
        name: &str,
        _kind: &str,
        file_path: &str,
        source: &[u8],
        _comments_text: &str,
    ) -> bool {
        path_indicates_test(file_path)
            || file_path.contains("/test/")
            || name.ends_with("Spec")
            || ["Test", "Unroll"]
                .iter()
                .any(|annotation| has_annotation(node, annotation, source))
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        let text = node.utf8_text(source).unwrap_or_default();
        !text
            .split_whitespace()
            .any(|token| matches!(token, "private" | "protected"))
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        has_annotation(node, "Deprecated", source)
            || node
                .utf8_text(source)
                .is_ok_and(|text| text.contains("@Deprecated"))
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["method_declaration", "closure", "function_definition"]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "record_declaration",
            "enum_declaration",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_body", "interface_body", "enum_body"]
    }
}
