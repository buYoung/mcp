//! Markdown document language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Query};

use super::LanguageSpec;

const QUERY_SOURCE: &str = include_str!("../../queries/markdown/symbols.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_md::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Markdown query")
    })
}

pub(crate) struct MarkdownSpec;

impl LanguageSpec for MarkdownSpec {
    fn language_name(&self) -> &'static str {
        "document"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_md::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "mdx"]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["markdown"]
    }

    fn caller_scan_enabled(&self) -> bool {
        false
    }

    fn indexes_format_text(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_and_inline_queries_compile() {
        let _ = query();
        Query::new(
            &tree_sitter_md::INLINE_LANGUAGE.into(),
            include_str!("../../queries/markdown/inline.scm"),
        )
        .expect("Failed to compile Markdown inline query");
    }
}
