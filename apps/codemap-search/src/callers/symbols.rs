//! Cross-cutting snapshot symbol state plus call-site attribution: the per-name symbol
//! index built once from the codemap snapshot, and the attribution helpers that map a
//! scan hit to its enclosing definition and exclude same-named-definition ranges.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser::{ExtractedFile, ExtractedSymbol, LocalBinding};

use super::scan::ScanHit;

/// A per-symbol view of where every snapshot symbol of a given name lives, used to
/// resolve a bare callee name to its qualified form and to count definitions.
pub(super) struct SymbolIndex<'a> {
    by_name: HashMap<&'a str, SymbolDefinitions<'a>>,
    test_filter: Option<&'a super::test_code::TestCodeFilter>,
    macro_files: HashMap<&'a str, &'a crate::parser::MacroExpansionInfo>,
}

struct SymbolDefinitions<'a> {
    candidates: Vec<(&'a ExtractedFile, &'a ExtractedSymbol)>,
    included: OnceCell<Vec<usize>>,
}

impl<'a> SymbolIndex<'a> {
    pub(super) fn can_attribute_calls(&self, path: &str) -> bool {
        !self.macro_files.contains_key(path)
    }
    pub(super) fn is_active_line(&self, path: &str, line: usize) -> bool {
        self.macro_files
            .get(path)
            .is_none_or(|info| info.is_active_line(line))
    }
    pub(super) fn includes(&self, path: &str, range: &crate::parser::CodeRange) -> bool {
        self.test_filter
            .is_none_or(|filter| !filter.is_excluded(path, range))
    }

    pub(super) fn definitions(&self, name: &str) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)> {
        let Some(definitions) = self.by_name.get(name) else {
            return Vec::new();
        };
        if self.test_filter.is_none() {
            return definitions.candidates.clone();
        }
        definitions
            .included
            .get_or_init(|| {
                definitions
                    .candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(index, (file, symbol))| {
                        self.includes(&file.file_path, &symbol.range)
                            .then_some(index)
                    })
                    .collect()
            })
            .iter()
            .map(|&index| definitions.candidates[index])
            .collect()
    }

    pub(super) fn function_definition_count(&self, name: &str) -> usize {
        self.definitions(name)
            .iter()
            .filter(|(_, symbol)| symbol.kind == "fn")
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallSiteAddress {
    pub(super) file_index: usize,
    pub(super) call_index: usize,
}

#[derive(Default)]
pub(super) struct NavigationIndex {
    pub(super) calls_by_name: HashMap<String, Vec<CallSiteAddress>>,
    pub(super) files_by_path: HashMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceHintResolution {
    UnsupportedSourceForm,
    SourceUnresolved,
}

pub(super) fn build_symbol_index<'a>(
    snapshot: &'a [ExtractedFile],
    test_filter: Option<&'a super::test_code::TestCodeFilter>,
) -> SymbolIndex<'a> {
    let mut by_name: HashMap<&str, SymbolDefinitions<'_>> = HashMap::new();
    for file in snapshot {
        for symbol in file
            .symbols
            .iter()
            .filter(|symbol| is_callable_symbol(symbol))
        {
            by_name
                .entry(&symbol.name)
                .or_insert_with(|| SymbolDefinitions {
                    candidates: Vec::new(),
                    included: OnceCell::new(),
                })
                .candidates
                .push((file, symbol));
        }
    }
    SymbolIndex {
        by_name,
        test_filter,
        macro_files: snapshot
            .iter()
            .filter_map(|file| {
                Some((
                    file.file_path.as_str(),
                    file.navigation.as_ref()?.macro_expansion.as_ref()?,
                ))
            })
            .collect(),
    }
}

pub(super) fn build_navigation_index(snapshot: &[ExtractedFile]) -> NavigationIndex {
    let mut calls_by_name: HashMap<String, Vec<CallSiteAddress>> = HashMap::new();
    let mut files_by_path = HashMap::new();
    for (file_index, file) in snapshot.iter().enumerate() {
        files_by_path.insert(file.file_path.clone(), file_index);
        if file.macro_expansion().is_some() {
            continue;
        }
        if let Some(navigation) = &file.navigation {
            for (call_index, call) in navigation.calls.iter().enumerate() {
                calls_by_name
                    .entry(call.name.clone())
                    .or_default()
                    .push(CallSiteAddress {
                        file_index,
                        call_index,
                    });
            }
        }
    }
    NavigationIndex {
        calls_by_name,
        files_by_path,
    }
}

fn is_callable_symbol(sym: &ExtractedSymbol) -> bool {
    sym.kind == "fn" || sym.kind == "method"
}

pub(super) fn lookup_global_callable_candidates<'a>(
    name: &str,
    index: &'a SymbolIndex<'a>,
) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)> {
    index.definitions(name)
}

/// Name-only fallback must not join unrelated language runtimes, or Rust/C-family
/// object member access to a free function. Module-qualified calls remain eligible.
pub(super) fn definition_is_compatible(
    source_path: &str,
    is_member_access: bool,
    definition_path: &str,
    symbol: &ExtractedSymbol,
) -> bool {
    let language = language_family(Path::new(source_path));
    language == language_family(Path::new(definition_path))
        && !(is_member_access
            && matches!(language, Some("rust" | "c_family"))
            && symbol.owner.is_none())
}

pub(super) fn language_family(path: &Path) -> Option<&'static str> {
    crate::lang::spec_for_path(path).map(|spec| match spec.language_name() {
        "javascript" | "typescript" | "vue" | "astro" | "svelte" => "ecmascript",
        "c" | "cpp" => "c_family",
        language => language,
    })
}

pub(super) fn lookup_compatible_candidates<'a>(
    name: &str,
    source_path: &str,
    is_member_access: bool,
    index: &'a SymbolIndex<'a>,
) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)> {
    lookup_global_callable_candidates(name, index)
        .into_iter()
        .filter(|(file, symbol)| {
            definition_is_compatible(source_path, is_member_access, &file.file_path, symbol)
        })
        .collect()
}

pub(super) fn infer_owner_hint(receiver: &str, locals: &[LocalBinding]) -> Option<String> {
    locals
        .iter()
        .find(|binding| binding.name == receiver)
        .and_then(|binding| {
            binding
                .value_type
                .clone()
                .or_else(|| binding.type_name.clone())
        })
}

pub(super) fn lookup_by_owner_and_name<'a>(
    owner: &str,
    name: &str,
    index: &'a SymbolIndex<'a>,
) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)> {
    lookup_global_callable_candidates(name, index)
        .into_iter()
        .filter(|(_, sym)| sym.owner.as_deref() == Some(owner))
        .collect()
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            std::path::Component::ParentDir => Some("..".to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn source_candidate_paths(
    importing_file: &str,
    source: &str,
) -> Result<Vec<String>, SourceHintResolution> {
    if !(source.starts_with("./") || source.starts_with("../")) {
        return Err(SourceHintResolution::UnsupportedSourceForm);
    }
    let base_dir = Path::new(importing_file)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let base = base_dir.join(source);
    const SCRIPT_EXTENSIONS: [&str; 4] = ["ts", "tsx", "js", "jsx"];
    let extensions: &[&str] = match Path::new(importing_file)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("cs") => &["cs"],
        Some("php") => &["php"],
        Some("rb") => &["rb"],
        Some("lua") => &["lua"],
        _ => &SCRIPT_EXTENSIONS,
    };
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(normalize_relative_path(&base));
    } else {
        for ext in extensions {
            candidates.push(normalize_relative_path(&base.with_extension(ext)));
        }
        for ext in extensions {
            candidates.push(normalize_relative_path(&base.join(format!("index.{ext}"))));
        }
    }
    Ok(candidates)
}

pub(super) fn lookup_source_hint_candidates<'a>(
    name: &str,
    importing_file: &str,
    source: &str,
    snapshot: &'a [ExtractedFile],
    navigation_index: &NavigationIndex,
) -> Result<Vec<(&'a ExtractedFile, &'a ExtractedSymbol)>, SourceHintResolution> {
    let candidates = source_candidate_paths(importing_file, source)?;
    let matched_files: Vec<usize> = candidates
        .iter()
        .filter_map(|candidate| navigation_index.files_by_path.get(candidate).copied())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if matched_files.len() != 1 {
        return Err(SourceHintResolution::SourceUnresolved);
    }
    let file_index = matched_files[0];
    let file = &snapshot[file_index];
    Ok(file
        .symbols
        .iter()
        .filter(|sym| sym.name == name && is_callable_symbol(sym) && sym.flags.is_exported)
        .map(|sym| (file, sym))
        .collect())
}

/// Whether a hit falls inside the line range of ANY `fn` definition carrying `name` in the
/// hit's own file. A definition header (`fn name(`) classifies as a call site, and a call
/// inside a same-named body is (self-)recursion — both must be filtered from caller lists.
/// For a unique name this is exactly the old own-range exclusion; for a common name it also
/// covers the sibling definitions.
pub(super) fn is_within_same_named_fn(hit: &ScanHit, name: &str, index: &SymbolIndex<'_>) -> bool {
    index.definitions(name).iter().any(|(file, def)| {
        def.kind == "fn"
            && file.file_path == hit.file_path
            && def.range.start_line <= hit.line_number
            && hit.line_number <= def.range.end_line_inclusive()
    })
}

/// The innermost `fn`-scope symbol whose inclusive line range contains `line` in `file`.
/// Smallest span wins (innermost nesting), tie-broken by `range_strictly_contains`. The
/// inclusive test (`start <= line <= end`) keeps single-line callables attributable.
pub(super) fn enclosing_fn(file: &ExtractedFile, line: usize) -> Option<&ExtractedSymbol> {
    let mut best: Option<&ExtractedSymbol> = None;
    for sym in &file.symbols {
        if sym.kind != "fn" {
            continue;
        }
        let (start, end) = (sym.range.start_line, sym.range.end_line_inclusive());
        if start <= line && line <= end {
            best = match best {
                None => Some(sym),
                Some(current) => {
                    // Prefer the strictly-inner one; on equal spans keep the first found.
                    if crate::parser::range_strictly_contains(&current.range, &sym.range) {
                        Some(sym)
                    } else {
                        Some(current)
                    }
                }
            };
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callers::fixtures::{file, sym};

    #[test]
    fn test_enclosing_fn_inclusive_single_line() {
        // A one-line arrow function: start == end. The inclusive test must attribute a call
        // on that exact line to it (the strict-contains test would drop it).
        let f = file(
            "a.ts",
            vec![
                sym("handler", "fn", 10, 10, None),
                sym("outer", "fn", 1, 50, None),
            ],
        );
        let encl = enclosing_fn(&f, 10).unwrap();
        // The innermost (smallest span) wins: handler (10-10), not outer (1-50).
        assert_eq!(encl.name, "handler");
    }

    #[test]
    fn test_enclosing_fn_innermost_wins() {
        let f = file(
            "a.rs",
            vec![
                sym("outer", "fn", 1, 100, None),
                sym("inner", "fn", 40, 60, None),
            ],
        );
        assert_eq!(enclosing_fn(&f, 50).unwrap().name, "inner");
        assert_eq!(enclosing_fn(&f, 5).unwrap().name, "outer");
        assert!(enclosing_fn(&f, 200).is_none());
    }

    #[test]
    fn relative_import_candidates_follow_the_importing_language() {
        for (importing_file, expected, unexpected) in [
            ("src/use.cs", "src/target.cs", "src/target.ts"),
            ("src/use.php", "src/target.php", "src/target.ts"),
            ("src/use.rb", "src/target.rb", "src/target.ts"),
            ("src/use.lua", "src/target.lua", "src/target.ts"),
        ] {
            let candidates = source_candidate_paths(importing_file, "./target").unwrap();
            assert!(candidates.iter().any(|candidate| candidate == expected));
            assert!(!candidates.iter().any(|candidate| candidate == unexpected));
        }
    }
}
