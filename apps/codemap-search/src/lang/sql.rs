//! SQL language spec. SQL is intentionally limited to declaration and literal extraction;
//! it has no caller/navigation or static-collection semantics.

use std::sync::OnceLock;
use tree_sitter::{Language, Query};

use super::LanguageSpec;

const SQL_QUERY_STR: &str = include_str!("../../queries/sql/symbols.scm");

fn get_sql_query() -> &'static Query {
    static SQL_QUERY: OnceLock<Query> = OnceLock::new();
    SQL_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_sequel::LANGUAGE.into(), SQL_QUERY_STR)
            .expect("Failed to compile SQL query")
    })
}

pub(crate) struct SqlSpec;

impl LanguageSpec for SqlSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_sequel::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_sql_query()
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sql"]
    }

    fn is_import_line(&self, _line: &str) -> bool {
        false
    }
}
