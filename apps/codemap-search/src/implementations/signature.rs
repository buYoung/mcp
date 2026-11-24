//! Whether two type spellings in two signatures denote one type across scopes, aliases,
//! qualification differences, language type aliases, and generic base substitutions.
use super::resolve::{encloses, normalized};
use super::*;
use crate::callers::resolution::SourceResolver;
use crate::parser::{CodeRange, ImportKind};
use std::cell::RefCell;
use std::collections::HashMap;

/// Resolution context of a written type fragment: the file, unit, owning type, namespace and
/// source range where that fragment appears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TypeScope {
    pub file: usize,
    pub unit: usize,
    pub owner: usize,
    pub namespace: String,
    pub range: CodeRange,
}

type CacheKey = (usize, usize, usize, usize, usize, String, usize);

/// What a type name in a signature refers to, as far as the indexed source shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TypeIdentity {
    /// A type declared in the indexed source.
    Indexed(usize),
    /// A name bound by explicit imports: each module and the name within it.
    Imported(Vec<(String, String)>),
    /// A globally qualified type path (e.g. PHP `\Ns\Type`, JVM `java.util.Date`, C++ `std::string`).
    Global(String),
    /// A type parameter declared on the enclosing type.
    TypeParam(String),
    /// A language-standard unqualified type name (e.g. Java `String`, Swift `Int`).
    Standard(String),
    /// A built-in language keyword/primitive type (canonicalized across exact aliases).
    Keyword(&'static str),
}

/// What a type name refers to and the declarations read to find that out, when it is known.
type Proven = Option<(TypeIdentity, Vec<Location>)>;

/// The type identities worked out so far for the signatures being compared, by scope, name, and
/// generic type-argument arity.
pub(super) struct Signatures<'a, 'r> {
    resolver: &'a SourceResolver<'r>,
    known: RefCell<HashMap<CacheKey, Proven>>,
}

impl<'a, 'r> Signatures<'a, 'r> {
    pub(super) fn new(resolver: &'a SourceResolver<'r>) -> Self {
        Self {
            resolver,
            known: RefCell::default(),
        }
    }
}

#[derive(Clone, Debug)]
enum TypeToken {
    Punct(char),
    Label(String),
    TypeName {
        name: String,
        arity: usize,
        scope: TypeScope,
    },
}

impl ImplementationIndex {
    pub(super) fn method_type_scope(&self, method: usize) -> TypeScope {
        let node = &self.methods[method];
        let owner = &self.types[node.owner];
        let is_in_type = node.file == owner.file
            || owner
                .parts
                .iter()
                .any(|(file, unit, _)| (*file, *unit) == (node.file, node.unit));
        let namespace = if is_in_type {
            self.typ(node.owner).namespace.clone()
        } else {
            self.method_unit(method).namespace.clone()
        };
        TypeScope {
            file: node.file,
            unit: node.unit,
            owner: node.owner,
            namespace,
            range: node.declaration.range.clone(),
        }
    }

    /// Whether `base_spelling` (written in `base_scope`, under `substitutions`) denotes the exact
    /// same type as `impl_spelling` (written in `impl_scope`), returning all declarations read as
    /// proof when it does.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn same_type_spelling(
        &self,
        language: &str,
        base_scope: &TypeScope,
        impl_scope: &TypeScope,
        base_spelling: &str,
        impl_spelling: &str,
        substitutions: &[TypeSubstitution],
        signatures: &Signatures<'_, '_>,
    ) -> Option<Vec<Location>> {
        if base_spelling == impl_spelling
            && substitutions.is_empty()
            && base_scope.file == impl_scope.file
            && base_scope.namespace == impl_scope.namespace
            && base_scope.owner == impl_scope.owner
        {
            return Some(Vec::new());
        }
        let left = tokenize_type(language, base_spelling, base_scope, substitutions, 0);
        let right = tokenize_type(language, impl_spelling, impl_scope, &[], 0);
        let mut i = 0usize;
        let mut j = 0usize;
        let mut evidence = Vec::new();
        let mut prev_csharp_ref = false;
        while i < left.len() || j < right.len() {
            // In C#, `#nullable enable` vs `#nullable disable` adds or omits `?` on reference
            // types (`string?` vs `string`, `object?` vs `object`, `byte[]?` vs `byte[]`,
            // `Uri?` vs `Uri`) without changing the CLR method signature. Value types (`int?`,
            // `DateTime?`, structs) keep `?` strictly significant.
            if language == "csharp" && prev_csharp_ref {
                let left_q = matches!(left.get(i), Some(TypeToken::Punct('?')));
                let right_q = matches!(right.get(j), Some(TypeToken::Punct('?')));
                if left_q != right_q {
                    if left_q {
                        i += 1;
                    } else {
                        j += 1;
                    }
                    prev_csharp_ref = false;
                    continue;
                }
            }
            let (Some(a), Some(b)) = (left.get(i), right.get(j)) else {
                return None;
            };
            match (a, b) {
                (TypeToken::Punct(ca), TypeToken::Punct(cb)) if ca == cb => {
                    prev_csharp_ref = language == "csharp" && *ca == ']';
                }
                (TypeToken::Label(la), TypeToken::Label(lb)) if la == lb => {
                    prev_csharp_ref = false;
                }
                (
                    TypeToken::TypeName {
                        name: na,
                        arity: aa,
                        scope: sa,
                    },
                    TypeToken::TypeName {
                        name: nb,
                        arity: ab,
                        scope: sb,
                    },
                ) if aa == ab => {
                    prev_csharp_ref = language == "csharp"
                        && (is_csharp_known_ref_type(na) || is_csharp_known_ref_type(nb));
                    let is_param_on = |owner: usize, spelling: &str| {
                        !spelling.contains(['.', ':', '\\'])
                            && self
                                .typ(owner)
                                .parameters
                                .iter()
                                .any(|param| param == spelling)
                    };
                    let same_scope_spelling = na == nb
                        && sa.file == sb.file
                        && sa.unit == sb.unit
                        && sa.namespace == sb.namespace
                        && (sa.owner == sb.owner
                            || (!is_param_on(sa.owner, na) && !is_param_on(sb.owner, nb)));
                    if !same_scope_spelling {
                        let (id_a, ev_a) = self.type_identity(sa, na, *aa, signatures)?;
                        let (id_b, ev_b) = self.type_identity(sb, nb, *ab, signatures)?;
                        if id_a != id_b {
                            return None;
                        }
                        evidence.extend(ev_a);
                        evidence.extend(ev_b);
                    }
                }
                _ => return None,
            }
            i += 1;
            j += 1;
        }
        Some(evidence)
    }

    fn type_identity(
        &self,
        scope: &TypeScope,
        name: &str,
        arity: usize,
        signatures: &Signatures<'_, '_>,
    ) -> Proven {
        let language = self.files[scope.file]
            .navigation
            .as_ref()?
            .implementations
            .as_ref()?
            .units[scope.unit]
            .language
            .as_str();
        if let Some(keyword) = canonical_keyword(language, name) {
            return Some((TypeIdentity::Keyword(keyword), Vec::new()));
        }
        if !name.contains(['.', ':', '\\'])
            && self
                .typ(scope.owner)
                .parameters
                .iter()
                .any(|param| param == name)
        {
            return Some((TypeIdentity::TypeParam(name.to_string()), Vec::new()));
        }
        // Go imports are strictly file-scoped and Go has no nested type declarations, so
        // non-type-parameter spellings resolve identically across every method in a unit.
        let key = if language == "go" {
            (
                scope.file,
                scope.unit,
                usize::MAX,
                0,
                0,
                name.to_string(),
                arity,
            )
        } else {
            (
                scope.file,
                scope.unit,
                scope.owner,
                scope.range.start_line,
                scope.range.start_col,
                name.to_string(),
                arity,
            )
        };
        if let Some(known) = signatures.known.borrow().get(&key) {
            return known.clone();
        }
        let identity = self.resolve_identity(scope, name, arity, signatures.resolver);
        signatures.known.borrow_mut().insert(key, identity.clone());
        identity
    }

    fn resolve_identity(
        &self,
        scope: &TypeScope,
        name: &str,
        arity: usize,
        resolver: &SourceResolver<'_>,
    ) -> Proven {
        let file = &self.files[scope.file];
        let unit = &file.navigation.as_ref()?.implementations.as_ref()?.units[scope.unit];
        let language = unit.language.as_str();
        let range = &scope.range;
        if let Some(keyword) = canonical_keyword(language, name) {
            return Some((TypeIdentity::Keyword(keyword), Vec::new()));
        }
        let path = normalized(name);
        if let Some(keyword) = canonical_keyword(language, &path) {
            return Some((TypeIdentity::Keyword(keyword), Vec::new()));
        }
        if !path.contains('.')
            && self
                .typ(scope.owner)
                .parameters
                .iter()
                .any(|param| param == &path)
        {
            return Some((TypeIdentity::TypeParam(path), Vec::new()));
        }
        // Try resolving to an indexed declaration first so a qualified reference (`models.Item`)
        // and an unqualified reference (`Item`) in the defining package share `Indexed(id)`.
        let reference = TypeReference {
            name: name.to_string(),
            arguments: vec![String::new(); arity],
            range: range.clone(),
        };
        if let Some((id, evidence)) =
            self.resolve_type(scope.file, scope.unit, &scope.namespace, &reference, resolver)
        {
            return Some((TypeIdentity::Indexed(id), evidence));
        }
        // A path anchored at the global scope (`\Ns\Type` in PHP, `::ns::Type` in C++) denotes
        // the same qualified type from every file.
        if name.starts_with('\\') || name.starts_with("::") {
            return Some((TypeIdentity::Global(path), Vec::new()));
        }
        let (head, tail) = path.split_once('.').unwrap_or((&path, ""));
        let bound = unit
            .imports
            .iter()
            .filter(|import| {
                import.entry.local_name == head
                    && import.entry.kind != ImportKind::Glob
                    && encloses(&import.scope, range)
            })
            .map(|import| &import.entry)
            .collect::<Vec<_>>();
        // For PHP/JVM explicit single-type imports of external types (`use Psr\...\Req;` or
        // `import java.util.Date;`), map to `Global(fqn)` so imported and fully-qualified
        // references (`\Psr\...\Req`, `java.util.Date`) agree.
        if matches!(language, "php" | "java" | "kotlin" | "scala" | "groovy") && !bound.is_empty() {
            let mut fqns = bound
                .iter()
                .filter(|entry| entry.kind == ImportKind::Named)
                .filter_map(|entry| {
                    let base = if language == "php" {
                        entry.source.as_deref()?
                    } else {
                        entry.imported_name.as_deref().or(entry.source.as_deref())?
                    };
                    let norm = normalized(base);
                    Some(if tail.is_empty() {
                        norm
                    } else {
                        format!("{norm}.{tail}")
                    })
                })
                .collect::<Vec<_>>();
            fqns.sort();
            fqns.dedup();
            if fqns.len() == 1 {
                return Some((TypeIdentity::Global(fqns.pop().unwrap()), Vec::new()));
            }
        }
        // One import statement can be recorded in more than one form, so every form counts.
        let imported = |member: Option<&str>| {
            let mut modules = bound
                .iter()
                .map(|entry| {
                    let module = entry.source.as_deref()?;
                    // A relative module names a file, so it is read from the importing file.
                    let module = if module.starts_with('.')
                        || language == "dart" && !module.contains(':')
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
        if !bound.is_empty() {
            return imported((!tail.is_empty()).then_some(tail));
        }
        let bare = path.rsplit('.').next().unwrap_or(&path);
        let is_declared = self.by_name.get(bare).is_some_and(|ids| !ids.is_empty())
            || file
                .symbols
                .iter()
                .any(|symbol| symbol.name == bare && crate::declarations::container(symbol));
        let has_import = |module: &str| {
            unit.imports.iter().any(|import| {
                import.entry.source.as_deref() == Some(module) && encloses(&import.scope, range)
            })
        };
        if is_qualified_standard(language, &path) {
            return Some((TypeIdentity::Global(path), Vec::new()));
        }
        if !is_declared && is_standard_type(language, &path, has_import) {
            if language == "csharp" {
                if let Some(keyword) = canonical_keyword("csharp", &format!("System.{path}")) {
                    return Some((TypeIdentity::Keyword(keyword), Vec::new()));
                }
            }
            return Some((TypeIdentity::Standard(path), Vec::new()));
        }
        None
    }
}

/// Tokenize a compact type spelling into punctuation, labels (`name:` / `ref:`), and scoped
/// type names (expanding any matching `substitutions` from a generic base clause).
fn tokenize_type(
    language: &str,
    spelling: &str,
    scope: &TypeScope,
    substitutions: &[TypeSubstitution],
    depth: usize,
) -> Vec<TypeToken> {
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
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        // Go `interface{}` is an exact alias for `any`.
        if language == "go" && spelling[at..].starts_with("interface{}") {
            tokens.push(TypeToken::TypeName {
                name: "interface{}".to_string(),
                arity: 0,
                scope: scope.clone(),
            });
            at += "interface{}".len();
            continue;
        }
        let start = at;
        let lead = separator(at);
        if !starts_name(at + lead) || lead > 0 && bytes[at] == b'.' {
            let ch = spelling[at..].chars().next().unwrap();
            tokens.push(TypeToken::Punct(ch));
            at += ch.len_utf8();
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
        let word = &spelling[start..at];
        let is_label = bytes.get(at) == Some(&b':') && bytes.get(at + 1) != Some(&b':');
        if is_label {
            tokens.push(TypeToken::Label(word.to_string()));
            at += 1; // consume the single ':'
            continue;
        }
        let arity = generic_arity(bytes, at);
        if depth < 8 && arity == 0 && !word.contains(['.', ':', '\\']) {
            if let Some(sub) = substitutions.iter().find(|sub| sub.parameter == word) {
                let sub_scope = TypeScope {
                    file: sub.file,
                    unit: sub.unit,
                    owner: sub.owner,
                    namespace: sub.namespace.clone(),
                    range: sub.range.clone(),
                };
                tokens.extend(tokenize_type(language, &sub.argument, &sub_scope, &[], depth + 1));
                continue;
            }
        }
        tokens.push(TypeToken::TypeName {
            name: word.to_string(),
            arity,
            scope: scope.clone(),
        });
    }
    tokens
}

fn is_csharp_known_ref_type(name: &str) -> bool {
    let bare = name.rsplit('.').next().unwrap_or(name);
    matches!(
        bare,
        "string" | "String" | "object" | "Object" | "dynamic" | "Uri" | "Type" | "Exception" | "Array"
    )
}

/// Canonical built-in keyword or language-exact type alias.
fn canonical_keyword(language: &str, name: &str) -> Option<&'static str> {
    if language == "go" {
        match name {
            "any" | "interface{}" => return Some("any"),
            "byte" | "uint8" => return Some("uint8"),
            "rune" | "int32" => return Some("int32"),
            _ => {}
        }
    }
    if language == "csharp" {
        match name {
            "string" | "System.String" => return Some("string"),
            "object" | "System.Object" => return Some("object"),
            "bool" | "System.Boolean" => return Some("bool"),
            "byte" | "System.Byte" => return Some("byte"),
            "sbyte" | "System.SByte" => return Some("sbyte"),
            "char" | "System.Char" => return Some("char"),
            "decimal" | "System.Decimal" => return Some("decimal"),
            "double" | "System.Double" => return Some("double"),
            "float" | "System.Single" => return Some("float"),
            "int" | "System.Int32" => return Some("int"),
            "uint" | "System.UInt32" => return Some("uint"),
            "long" | "System.Int64" => return Some("long"),
            "ulong" | "System.UInt64" => return Some("ulong"),
            "short" | "System.Int16" => return Some("short"),
            "ushort" | "System.UInt16" => return Some("ushort"),
            _ => {}
        }
    }
    match name {
        "void" => Some("void"),
        "bool" => Some("bool"),
        "boolean" => Some("boolean"),
        "byte" => Some("byte"),
        "char" => Some("char"),
        "short" => Some("short"),
        "int" => Some("int"),
        "long" => Some("long"),
        "float" => Some("float"),
        "double" => Some("double"),
        "number" => Some("number"),
        "string" => Some("string"),
        "str" => Some("str"),
        "u8" => Some("u8"),
        "u16" => Some("u16"),
        "u32" => Some("u32"),
        "u64" => Some("u64"),
        "i8" => Some("i8"),
        "i16" => Some("i16"),
        "i32" => Some("i32"),
        "i64" => Some("i64"),
        "usize" => Some("usize"),
        "isize" => Some("isize"),
        "uint" => Some("uint"),
        "uint8" => Some("uint8"),
        "uint16" => Some("uint16"),
        "uint32" => Some("uint32"),
        "uint64" => Some("uint64"),
        "int8" => Some("int8"),
        "int16" => Some("int16"),
        "int32" => Some("int32"),
        "int64" => Some("int64"),
        "float32" => Some("float32"),
        "float64" => Some("float64"),
        "complex64" => Some("complex64"),
        "complex128" => Some("complex128"),
        "rune" => Some("rune"),
        "error" => Some("error"),
        _ => match language {
            "go" => match name {
                "any" => Some("any"),
                "uintptr" => Some("uintptr"),
                "func" => Some("func"),
                "chan" => Some("chan"),
                "map" => Some("map"),
                "struct" => Some("struct"),
                "interface" => Some("interface"),
                _ => None,
            },
            "typescript" | "javascript" => match name {
                "any" => Some("any"),
                "unknown" => Some("unknown"),
                "never" => Some("never"),
                "undefined" => Some("undefined"),
                "null" => Some("null"),
                "object" => Some("object"),
                "symbol" => Some("symbol"),
                "bigint" => Some("bigint"),
                "true" => Some("true"),
                "false" => Some("false"),
                "this" => Some("this"),
                "readonly" => Some("readonly"),
                "keyof" => Some("keyof"),
                "typeof" => Some("typeof"),
                _ => None,
            },
            "csharp" => match name {
                "object" => Some("object"),
                "decimal" => Some("decimal"),
                "dynamic" => Some("dynamic"),
                "sbyte" => Some("sbyte"),
                "ushort" => Some("ushort"),
                "ulong" => Some("ulong"),
                "nint" => Some("nint"),
                "nuint" => Some("nuint"),
                "ref" => Some("ref"),
                "out" => Some("out"),
                "in" => Some("in"),
                "params" => Some("params"),
                "this" => Some("this"),
                _ => None,
            },
            "php" => match name {
                "array" => Some("array"),
                "callable" => Some("callable"),
                "iterable" => Some("iterable"),
                "object" => Some("object"),
                "mixed" => Some("mixed"),
                "null" => Some("null"),
                "false" => Some("false"),
                "true" => Some("true"),
                _ => None,
            },
            "rust" => match name {
                "Self" => Some("Self"),
                "mut" => Some("mut"),
                "dyn" => Some("dyn"),
                "impl" => Some("impl"),
                "u128" => Some("u128"),
                "i128" => Some("i128"),
                "f32" => Some("f32"),
                "f64" => Some("f64"),
                _ => None,
            },
            "swift" => match name {
                "Self" => Some("Self"),
                "Any" => Some("Any"),
                "inout" => Some("inout"),
                "some" => Some("some"),
                "any" => Some("any"),
                _ => None,
            },
            "kotlin" => match name {
                "suspend" => Some("suspend"),
                "vararg" => Some("vararg"),
                _ => None,
            },
            "dart" => match name {
                "dynamic" => Some("dynamic"),
                "covariant" => Some("covariant"),
                _ => None,
            },
            "cpp" => match name {
                "unsigned" => Some("unsigned"),
                "signed" => Some("signed"),
                "const" => Some("const"),
                _ => None,
            },
            _ => None,
        },
    }
}

/// Top-level comma-separated generic argument count inside `<...>` immediately following `at`.
fn generic_arity(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    if bytes.get(at) != Some(&b'<') {
        return 0;
    }
    at += 1;
    let mut angle = 1usize;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut commas = 0usize;
    let mut has_content = false;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'<' => {
                angle += 1;
                has_content = true;
            }
            b'>' => {
                angle -= 1;
                if angle == 0 {
                    return if has_content { commas + 1 } else { 0 };
                }
            }
            b'(' => {
                paren += 1;
                has_content = true;
            }
            b')' => paren = paren.saturating_sub(1),
            b'[' => {
                bracket += 1;
                has_content = true;
            }
            b']' => bracket = bracket.saturating_sub(1),
            b',' if angle == 1 && paren == 0 && bracket == 0 => {
                commas += 1;
                has_content = true;
            }
            b if !b.is_ascii_whitespace() => has_content = true,
            _ => {}
        }
        at += 1;
    }
    0
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
