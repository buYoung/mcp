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
    /// C++ `using namespace Ns;` directives visible in this unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub using_namespaces: Vec<ScopedName>,
    /// C/C++ macro names defined in this file as empty or pure export/visibility attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub export_macros: Vec<String>,
    /// C++ template class/struct qualified names that declare partial or explicit specializations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub specialized_templates: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeReference {
    pub name: String,
    pub arguments: Vec<String>,
    pub range: CodeRange,
    /// Per-base preprocessor condition expressions (e.g. C# `#if HAVE_COMPONENT_MODEL`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDeclaration {
    pub name: String,
    pub typ: TypeReference,
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
    /// Exported as the module's default export (`export default class ...`), for script modules.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_default_export: bool,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_complete: bool,
    /// True when a non-Go type declaration has an intact header (`is_complete = true`) but its
    /// body contains a syntax/preprocessor parse error; suppresses overload-sensitive direct calls.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_body_error: bool,
    /// Declared as one of several parts of a type, as C# `partial` declarations are.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_partial: bool,
    /// Anonymous implementation owner (Java `new Base() { ... }`, Kotlin `object : Base { ... }`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_anonymous: bool,
    /// Typed instance fields declared on this type, for cross-file field-receiver and chained dispatch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldDeclaration>,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodDeclaration {
    pub name: String,
    pub range: CodeRange,
    pub parameters: Vec<String>,
    /// Type parameters declared on the method itself (e.g. `<T>`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_parameters: Vec<String>,
    pub return_type: String,
    pub qualifiers: String,
    pub is_abstract: bool,
    pub is_static: bool,
    pub is_private: bool,
    /// Visible only inside the declaring package or library, as a Java member without an
    /// access modifier and a Dart name with a leading underscore are.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_package_scoped: bool,
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
    /// Source range of the detached impl/extension block itself (used for explicit impl reporting).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<CodeRange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReceiverStep {
    Field(String),
    Call {
        name: String,
        argument_count: Option<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchCall {
    pub name: String,
    pub range: CodeRange,
    pub receiver: TypeReference,
    /// Optional member/call traversal steps from `receiver` (e.g. `self.field.m()` or `f().m()`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receiver_steps: Vec<ReceiverStep>,
    pub namespace: String,
    pub enclosing_symbol: Option<String>,
    pub argument_count: Option<usize>,
    pub conditions: Vec<String>,
    #[serde(default)]
    pub receiver_kind: ReceiverKind,
}

/// How a call's receiver relates to its type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReceiverKind {
    /// A value of the type: dispatch may reach an override.
    #[default]
    Value,
    /// The type itself: static members, never replaced at runtime.
    Type,
    /// The base of the enclosing type (`super`): the named member runs, not an override.
    Base,
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
