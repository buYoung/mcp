use crate::parser::{CodeRange, ImportEntry};
use serde::{Deserialize, Serialize};

/// Compact, source-derived facts. These travel with the symbol snapshot; no source text is kept.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationFile {
    pub source_digest: String,
    pub units: Vec<ImplementationUnit>,
    pub omitted: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationUnit {
    pub language: String,
    pub namespace: String,
    pub imports: Vec<ScopedImport>,
    pub shadows: Vec<ScopedName>,
    pub types: Vec<TypeDeclaration>,
    pub implementations: Vec<ImplementationBlock>,
    pub calls: Vec<DispatchCall>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeReference {
    pub name: String,
    pub arguments: Vec<String>,
    pub range: CodeRange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedImport {
    pub entry: ImportEntry,
    pub scope: CodeRange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedName {
    pub name: String,
    pub scope: CodeRange,
    pub range: CodeRange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDeclaration {
    pub name: String,
    pub qualified_name: String,
    pub range: CodeRange,
    pub namespace: String,
    pub scope: CodeRange,
    pub parameters: Vec<String>,
    pub bases: Vec<TypeReference>,
    pub methods: Vec<MethodDeclaration>,
    pub is_contract: bool,
    pub is_exported: bool,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_complete: bool,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodDeclaration {
    pub name: String,
    pub range: CodeRange,
    pub parameters: Vec<String>,
    pub return_type: String,
    pub qualifiers: String,
    pub is_abstract: bool,
    pub is_static: bool,
    pub is_private: bool,
    pub is_final: bool,
    pub is_virtual: bool,
    pub is_override: bool,
    pub is_pointer_receiver: bool,
    pub has_known_signature: bool,
    pub conditions: Vec<String>,
}

/// Rust impl blocks, Go receiver declarations and Swift extensions have a separate owner site.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationBlock {
    pub owner: TypeReference,
    pub contracts: Vec<TypeReference>,
    pub methods: Vec<MethodDeclaration>,
    pub namespace: String,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchCall {
    pub name: String,
    pub range: CodeRange,
    pub receiver: TypeReference,
    pub namespace: String,
    pub enclosing_symbol: Option<String>,
    pub argument_count: Option<usize>,
    pub conditions: Vec<String>,
}

pub(crate) fn digest(source: &[u8]) -> String {
    blake3::hash(source).to_hex().to_string()
}

impl ImplementationFile {
    pub(crate) fn merge(&mut self, other: Self) {
        self.omitted += other.omitted;
        self.units.extend(other.units);
    }
}
