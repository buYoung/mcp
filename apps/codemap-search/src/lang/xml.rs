//! XML-family language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::format_support::{clean, is_recoverable, named_children};
use super::{LanguageSpec, NameDecision};

const QUERY_SOURCE: &str = include_str!("../../queries/xml/symbols.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_xml::LANGUAGE_XML.into(), QUERY_SOURCE)
            .expect("Failed to compile XML query")
    })
}

fn attribute_parts(node: Node<'_>, source: &[u8]) -> Option<(String, Vec<String>)> {
    let children = named_children(node);
    let name = children.first().map(|child| clean(*child, source))?;
    let value = children.get(1).map(|child| clean(*child, source))?;
    Some((name, value.split_whitespace().map(str::to_string).collect()))
}

pub(crate) struct XmlSpec;

impl LanguageSpec for XmlSpec {
    fn language_name(&self) -> &'static str {
        "xml"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_xml::LANGUAGE_XML.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[
            "xml", "xsd", "xsl", "xslt", "plist", "csproj", "props", "targets",
        ]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn indexes_format_text(&self) -> bool {
        true
    }

    fn capture_is_valid(&self, capture: &str, node: Node<'_>, source: &[u8]) -> bool {
        is_recoverable(node)
            && (capture != "symbol.property"
                || attribute_parts(node, source).is_some_and(|(name, values)| {
                    matches!(name.as_str(), "id" | "class") && !values.is_empty()
                }))
    }

    fn refine_kind(&self, capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        if capture == "symbol.type" {
            "tag"
        } else {
            "attribute"
        }
    }

    fn name_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        _meta: &Option<String>,
    ) -> Option<NameDecision> {
        if capture == "symbol.type" {
            return named_children(node)
                .into_iter()
                .find(|child| child.kind() == "Name")
                .map(|name| NameDecision::Name(clean(name, source)));
        }
        attribute_parts(node, source)
            .and_then(|(_, values)| values.into_iter().next())
            .map(NameDecision::Name)
    }

    fn additional_symbol_names_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        source: &[u8],
        _primary_name: &str,
    ) -> Vec<String> {
        if capture != "symbol.property" {
            return Vec::new();
        }
        attribute_parts(node, source)
            .map(|(_, values)| values.into_iter().skip(1).collect())
            .unwrap_or_default()
    }
}
