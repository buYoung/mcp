use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolFlags {
    pub has_todo: bool,
    pub has_fixme: bool,
    pub is_test: bool,
    pub is_exported: bool,
    pub is_deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: String,
    pub range: CodeRange,
    pub docstring: Option<String>,
    pub flags: SymbolFlags,
    /// The enclosing *type* of a member (the class for class-nested languages, the `impl`
    /// type for Rust, the receiver type for Go). `None` for free functions, top-level
    /// symbols, and functions nested only inside a module/namespace or a function/closure/
    /// lambda scope. Additive and best-effort: a wrong owner is worse than `None`, so any
    /// unexpected parse shape yields `None`. `#[serde(default)]` keeps pre-upgrade docs
    /// (which lack this field) deserializable during the one-time reindex transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedLiteral {
    pub text: String,
    /// 1-based source line where the literal starts (matches `read` line numbers).
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CallSite {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    pub range: CodeRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSite {
    pub name: String,
    pub range: CodeRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalBinding {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    pub range: CodeRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportKind {
    Named,
    Default,
    Namespace,
    Glob,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntry {
    pub local_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub kind: ImportKind,
    pub range: CodeRange,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NavigationFile {
    #[serde(default)]
    pub calls: Vec<CallSite>,
    #[serde(default)]
    pub references: Vec<ReferenceSite>,
    #[serde(default)]
    pub local_bindings: Vec<LocalBinding>,
    #[serde(default)]
    pub imports: Vec<ImportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFile {
    pub file_path: String,
    #[serde(default)]
    pub total_lines: usize,
    pub symbols: Vec<ExtractedSymbol>,
    pub literals: Vec<ExtractedLiteral>,
    pub docstrings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<NavigationFile>,
}

/// `outer` strictly contains `inner` when `inner`'s line span sits inside
/// `outer`'s and the two spans are not identical — so a symbol never contains
/// itself and two symbols sharing a range never drop each other.
pub(crate) fn range_strictly_contains(outer: &CodeRange, inner: &CodeRange) -> bool {
    outer.start_line <= inner.start_line
        && inner.end_line <= outer.end_line
        && (outer.start_line < inner.start_line || inner.end_line < outer.end_line)
}
