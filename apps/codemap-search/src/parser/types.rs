use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl CodeRange {
    /// Inclusive last source line for display and line-window comparisons.
    /// Stored positions remain one-based with an exclusive end for syntax lookup.
    pub fn end_line_inclusive(&self) -> usize {
        if self.end_col == 1 && self.end_line > self.start_line {
            self.end_line - 1
        } else {
            self.end_line
        }
    }
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

/// One statically identifiable write or read endpoint for a typed collection.
/// Endpoints are joined only when both the resolved owning type and the
/// statically named field match; this is navigation evidence, not a runtime guarantee.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StaticCollectionEdgeKind {
    Producer,
    Consumer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StaticCollectionEdge {
    pub kind: StaticCollectionEdgeKind,
    pub collection_owner_type: String,
    pub collection_field: String,
    pub owner_expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_owner: Option<String>,
    /// Exact lexical declaration span for `source_owner`, when syntax proves it. This keeps
    /// same-named nested types in one file from being treated as a single owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_owner_range: Option<CodeRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub range: CodeRange,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_expansion: Option<MacroExpansionInfo>,
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
pub struct MacroExpansionInfo {
    pub notice: String,
    #[serde(default)]
    pub symbols: Vec<MacroSymbolOrigin>,
    #[serde(default)]
    pub inputs: Vec<MacroInputStamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_lines: Option<Vec<[usize; 2]>>,
}

impl MacroExpansionInfo {
    pub fn is_expanded_symbol(&self, symbol: &ExtractedSymbol) -> bool {
        self.symbols
            .iter()
            .any(|origin| origin.name == symbol.name && origin.range == symbol.range)
    }
    pub fn is_active_line(&self, line: usize) -> bool {
        self.active_lines.as_ref().is_none_or(|ranges| {
            ranges
                .iter()
                .any(|[start, end]| *start <= line && line <= *end)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MacroSymbolOrigin {
    pub name: String,
    pub range: CodeRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MacroInputStamp {
    pub path: String,
    pub modified_ns: u64,
    pub size_bytes: u64,
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

impl ExtractedFile {
    pub fn macro_expansion(&self) -> Option<&MacroExpansionInfo> {
        self.navigation.as_ref()?.macro_expansion.as_ref()
    }
}

/// `outer` strictly contains `inner` when `inner`'s line span sits inside
/// `outer`'s and the two spans are not identical — so a symbol never contains
/// itself and two symbols sharing a range never drop each other.
pub(crate) fn range_strictly_contains(outer: &CodeRange, inner: &CodeRange) -> bool {
    outer.start_line <= inner.start_line
        && inner.end_line <= outer.end_line
        && (outer.start_line < inner.start_line || inner.end_line < outer.end_line)
}
