//! Whether a type name spelled the same in two signatures denotes one type in both.
use super::resolve::{encloses, normalized};
use super::*;
use crate::callers::resolution::SourceResolver;
use crate::parser::ImportKind;
use std::cell::RefCell;
use std::collections::HashMap;

/// What a type name in a signature refers to, as far as the indexed source shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TypeIdentity {
    /// A type declared in the indexed source.
    Indexed(usize),
    /// A name bound by explicit imports: each module and the name within it.
    Imported(Vec<(String, String)>),
    /// A name the language gives one meaning in every file.
    Standard,
}

/// What a type name refers to and the declarations read to find that out, when it is known.
type Proven = Option<(TypeIdentity, Vec<Location>)>;

/// The type identities worked out so far for the signatures being compared, by method and name.
pub(super) struct Signatures<'a, 'r> {
    resolver: &'a SourceResolver<'r>,
    known: RefCell<HashMap<(usize, String), Proven>>,
}

impl<'a, 'r> Signatures<'a, 'r> {
    pub(super) fn new(resolver: &'a SourceResolver<'r>) -> Self {
        Self {
            resolver,
            known: RefCell::default(),
        }
    }
}

impl ImplementationIndex {
    /// Whether every type name in `spelling`, written the same in both methods, is one type
    /// in both: a keyword, one indexed declaration, one import or a name the language defines.
    pub(super) fn same_types(
        &self,
        base: usize,
        implementation: usize,
        spelling: &str,
        signatures: &Signatures<'_, '_>,
    ) -> bool {
        let language = self.unit(self.methods[base].owner).language.as_str();
        type_names(spelling).into_iter().all(|name| {
            is_keyword_type(language, name)
                || self
                    .type_identity(base, name, signatures)
                    .zip(self.type_identity(implementation, name, signatures))
                    .is_some_and(|((first, _), (second, _))| first == second)
        })
    }

    /// The declarations that `same_types` read to accept the signature of `method`.
    pub(super) fn signature_evidence(
        &self,
        method: usize,
        signatures: &Signatures<'_, '_>,
    ) -> Vec<Location> {
        let declaration = &self.methods[method].declaration;
        declaration
            .parameters
            .iter()
            .chain(std::iter::once(&declaration.return_type))
            .flat_map(|spelling| type_names(spelling))
            .filter_map(|name| {
                signatures
                    .known
                    .borrow()
                    .get(&(method, name.to_string()))?
                    .clone()
            })
            .flat_map(|(_, evidence)| evidence)
            .collect()
    }

    fn type_identity(&self, method: usize, name: &str, signatures: &Signatures<'_, '_>) -> Proven {
        let key = (method, name.to_string());
        if let Some(known) = signatures.known.borrow().get(&key) {
            return known.clone();
        }
        let identity = self.resolve_identity(method, name, signatures.resolver);
        signatures.known.borrow_mut().insert(key, identity.clone());
        identity
    }

    fn resolve_identity(&self, method: usize, name: &str, resolver: &SourceResolver<'_>) -> Proven {
        let node = &self.methods[method];
        let file = &self.files[node.file];
        let unit = &file.navigation.as_ref()?.implementations.as_ref()?.units[node.unit];
        let range = &node.declaration.range;
        // A path that starts at the global scope reads the same from every file.
        if name.starts_with('\\') || name.starts_with("::") {
            return Some((TypeIdentity::Standard, Vec::new()));
        }
        let path = normalized(name);
        let (head, tail) = path.split_once('.').unwrap_or((&path, ""));
        let bound = unit
            .imports
            .iter()
            .map(|import| &import.entry)
            .zip(unit.imports.iter().map(|import| &import.scope))
            .filter(|(entry, scope)| {
                entry.local_name == head && entry.kind != ImportKind::Glob && encloses(scope, range)
            })
            .map(|(entry, _)| entry)
            .collect::<Vec<_>>();
        // One import statement can be recorded in more than one form, so every form counts.
        let imported = |member: Option<&str>| {
            let mut modules = bound
                .iter()
                .map(|entry| {
                    let module = entry.source.as_deref()?;
                    // A relative module names a file, so it is read from the importing file.
                    let module = if module.starts_with('.')
                        || unit.language == "dart" && !module.contains(':')
                    {
                        relative_path(&file.file_path, module)?
                    } else {
                        module.to_string()
                    };
                    let member = member.unwrap_or(match entry.kind {
                        ImportKind::Default => "default",
                        _ => entry.imported_name.as_deref().unwrap_or(head),
                    });
                    Some((module, member.to_string()))
                })
                .collect::<Option<Vec<_>>>()?;
            modules.sort();
            modules.dedup();
            Some((TypeIdentity::Imported(modules), Vec::new()))
        };
        // An import that qualifies the name decides it: the rest of the path lies in that module.
        if !tail.is_empty() && !bound.is_empty() {
            return imported(Some(tail));
        }
        // Every part of a type declared in parts lies in the namespace of the type.
        let owner = &self.types[node.owner];
        let is_in_type = node.file == owner.file
            || owner
                .parts
                .iter()
                .any(|(file, unit, _)| (*file, *unit) == (node.file, node.unit));
        let namespace = if is_in_type {
            self.typ(node.owner).namespace.as_str()
        } else {
            unit.namespace.as_str()
        };
        let reference = TypeReference {
            name: name.to_string(),
            arguments: Vec::new(),
            range: range.clone(),
        };
        if let Some((id, evidence)) =
            self.resolve_type(node.file, node.unit, namespace, &reference, resolver)
        {
            return Some((TypeIdentity::Indexed(id), evidence));
        }
        if !bound.is_empty() {
            return imported(None);
        }
        let bare = path.rsplit('.').next().unwrap_or(&path);
        let is_declared = self.by_name.get(bare).is_some_and(|ids| !ids.is_empty())
            || file.symbols.iter().any(|symbol| symbol.name == bare);
        let has_import = |module: &str| {
            unit.imports
                .iter()
                .any(|import| import.entry.source.as_deref() == Some(module))
        };
        (is_qualified_standard(&unit.language, &path)
            || !is_declared && is_standard_type(&unit.language, &path, has_import))
        .then(|| (TypeIdentity::Standard, Vec::new()))
    }
}

/// The type names a compact signature fragment spells, each with its qualifying path. A word
/// followed by a single colon labels a parameter and is not a type.
fn type_names(spelling: &str) -> Vec<&str> {
    let bytes = spelling.as_bytes();
    let starts_name = |at: usize| {
        bytes
            .get(at)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_' || !byte.is_ascii())
    };
    let separator = |at: usize| match bytes.get(at) {
        Some(b'.' | b'\\') => 1,
        Some(b':') if bytes.get(at + 1) == Some(&b':') => 2,
        _ => 0,
    };
    let mut names = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let lead = separator(at);
        if !starts_name(at + lead) || lead > 0 && bytes[at] == b'.' {
            // Digits and the rest of a number or a word that began with one are skipped whole.
            at += 1;
            while bytes.get(at - 1).is_some_and(u8::is_ascii_digit)
                && bytes
                    .get(at)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                at += 1;
            }
            continue;
        }
        at += lead;
        loop {
            while bytes.get(at).is_some_and(|byte| {
                byte.is_ascii_alphanumeric() || *byte == b'_' || !byte.is_ascii()
            }) {
                at += 1;
            }
            let next = separator(at);
            if next == 0 || !starts_name(at + next) {
                break;
            }
            at += next;
        }
        let is_label = bytes.get(at) == Some(&b':') && bytes.get(at + 1) != Some(&b':');
        if !is_label {
            names.push(&spelling[start..at]);
        }
    }
    names
}

/// Words a signature spells that no declaration can rename: built-in value types and the
/// keywords that stand beside a type.
fn is_keyword_type(language: &str, name: &str) -> bool {
    matches!(
        name,
        "void"
            | "bool"
            | "boolean"
            | "byte"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "number"
            | "string"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "usize"
            | "isize"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "float32"
            | "float64"
            | "complex64"
            | "complex128"
            | "rune"
            | "error"
    ) || match language {
        "go" => matches!(
            name,
            "any" | "uintptr" | "func" | "chan" | "map" | "struct" | "interface"
        ),
        "typescript" | "javascript" => matches!(
            name,
            "any"
                | "unknown"
                | "never"
                | "undefined"
                | "null"
                | "object"
                | "symbol"
                | "bigint"
                | "true"
                | "false"
                | "this"
                | "readonly"
                | "keyof"
                | "typeof"
        ),
        "csharp" => matches!(
            name,
            "object"
                | "decimal"
                | "dynamic"
                | "sbyte"
                | "ushort"
                | "ulong"
                | "nint"
                | "nuint"
                | "ref"
                | "out"
                | "in"
                | "params"
                | "this"
        ),
        "php" => matches!(
            name,
            "array" | "callable" | "iterable" | "object" | "mixed" | "null" | "false" | "true"
        ),
        "rust" => matches!(
            name,
            "Self" | "mut" | "dyn" | "impl" | "u128" | "i128" | "f32" | "f64"
        ),
        "swift" => matches!(name, "Self" | "Any" | "inout" | "some" | "any"),
        "kotlin" => matches!(name, "suspend" | "vararg"),
        "dart" => matches!(name, "dynamic" | "covariant"),
        "cpp" => matches!(name, "unsigned" | "signed" | "const"),
        _ => false,
    }
}

/// A qualified name whose first part the language reserves, so no file can rebind it.
fn is_qualified_standard(language: &str, path: &str) -> bool {
    language == "cpp" && path.starts_with("std.")
}

/// Type names a language defines in every file without an import. A declaration of the same
/// name in the indexed source takes its place, so callers check that there is none.
/// `has_import` tells whether the file imports a module by that exact name.
fn is_standard_type(language: &str, path: &str, has_import: impl Fn(&str) -> bool) -> bool {
    let java = || {
        matches!(
            path,
            "Object"
                | "String"
                | "CharSequence"
                | "Number"
                | "Integer"
                | "Long"
                | "Short"
                | "Byte"
                | "Character"
                | "Boolean"
                | "Double"
                | "Float"
                | "Void"
                | "Class"
                | "Enum"
                | "Iterable"
                | "Comparable"
                | "Runnable"
                | "Thread"
                | "Throwable"
                | "Exception"
                | "RuntimeException"
                | "Error"
                | "StringBuilder"
                | "AutoCloseable"
        )
    };
    match language {
        "java" | "groovy" => java(),
        "kotlin" => matches!(
            path,
            "Any"
                | "Unit"
                | "Nothing"
                | "Int"
                | "Long"
                | "Short"
                | "Byte"
                | "Char"
                | "Boolean"
                | "Double"
                | "Float"
                | "String"
                | "CharSequence"
                | "Number"
                | "Array"
                | "IntArray"
                | "LongArray"
                | "ByteArray"
                | "CharArray"
                | "BooleanArray"
                | "DoubleArray"
                | "FloatArray"
                | "ShortArray"
                | "Throwable"
                | "Comparable"
                | "Enum"
                | "Pair"
                | "Triple"
                | "List"
                | "MutableList"
                | "Set"
                | "MutableSet"
                | "Map"
                | "MutableMap"
                | "Collection"
                | "MutableCollection"
                | "Iterable"
                | "Iterator"
                | "Sequence"
        ),
        "scala" => {
            java()
                || matches!(
                    path,
                    "Any"
                        | "AnyRef"
                        | "AnyVal"
                        | "Unit"
                        | "Nothing"
                        | "Null"
                        | "Int"
                        | "Char"
                        | "Option"
                        | "List"
                        | "Seq"
                        | "Map"
                        | "Set"
                        | "Array"
                        | "Vector"
                        | "Either"
                )
        }
        "swift" => matches!(
            path,
            "Int"
                | "Int8"
                | "Int16"
                | "Int32"
                | "Int64"
                | "UInt"
                | "UInt8"
                | "UInt16"
                | "UInt32"
                | "UInt64"
                | "Double"
                | "Float"
                | "Bool"
                | "String"
                | "Character"
                | "Substring"
                | "Array"
                | "Dictionary"
                | "Set"
                | "Optional"
                | "AnyObject"
                | "Void"
                | "Error"
                | "Never"
                | "Result"
        ),
        "dart" => matches!(
            path,
            "num"
                | "String"
                | "Object"
                | "Null"
                | "Never"
                | "List"
                | "Map"
                | "Set"
                | "Iterable"
                | "Iterator"
                | "Future"
                | "Stream"
                | "Duration"
                | "DateTime"
                | "Uri"
                | "Function"
                | "Type"
                | "Symbol"
                | "Exception"
                | "Error"
                | "StackTrace"
                | "Pattern"
                | "RegExp"
                | "Comparable"
                | "BigInt"
                | "Record"
                | "Enum"
        ),
        "rust" => matches!(path, "String" | "Vec" | "Option" | "Result" | "Box"),
        "cpp" => matches!(
            path,
            "size_t"
                | "ptrdiff_t"
                | "int8_t"
                | "int16_t"
                | "int32_t"
                | "int64_t"
                | "uint8_t"
                | "uint16_t"
                | "uint32_t"
                | "uint64_t"
                | "intptr_t"
                | "uintptr_t"
                | "wchar_t"
                | "char8_t"
                | "char16_t"
                | "char32_t"
        ),
        // `System` is in scope only where the file imports it.
        "csharp" => {
            has_import("System")
                && matches!(
                    path,
                    "Object"
                        | "String"
                        | "Type"
                        | "Exception"
                        | "Array"
                        | "Enum"
                        | "Delegate"
                        | "Attribute"
                        | "Action"
                        | "Func"
                        | "IDisposable"
                        | "IComparable"
                        | "IEquatable"
                        | "IFormatProvider"
                        | "DateTime"
                        | "DateTimeOffset"
                        | "TimeSpan"
                        | "Guid"
                        | "Uri"
                        | "Nullable"
                        | "EventArgs"
                        | "EventHandler"
                        | "Int32"
                        | "Int64"
                        | "Boolean"
                        | "Double"
                        | "Decimal"
                        | "Byte"
                        | "Char"
                )
        }
        // Script globals are not listed: a bare name that no file imports or declares is one.
        "typescript" | "javascript" => !path.contains('.'),
        _ => false,
    }
}
