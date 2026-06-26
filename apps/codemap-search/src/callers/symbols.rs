//! Cross-cutting snapshot symbol state plus call-site attribution: the per-name symbol
//! index built once from the codemap snapshot, and the attribution helpers that map a
//! scan hit to its enclosing definition and exclude same-named-definition ranges.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser::{ExtractedFile, ExtractedSymbol, LocalBinding};

use super::scan::ScanHit;

/// A per-symbol view of where every snapshot symbol of a given name lives, used to
/// resolve a bare callee name to its qualified form and to count definitions.
pub(super) struct SymbolIndex<'a> {
    /// name → all snapshot symbols (any kind) carrying that name.
    pub(super) by_name: HashMap<&'a str, Vec<(&'a ExtractedFile, &'a ExtractedSymbol)>>,
    /// Global set of `fn` names (callee intersection target).
    pub(super) fn_names: HashSet<String>,
    /// name → count of `fn` definitions (common-name threshold input).
    pub(super) fn_def_counts: HashMap<String, usize>,
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

pub(super) fn build_symbol_index(snapshot: &[ExtractedFile]) -> SymbolIndex<'_> {
    let mut by_name: HashMap<&str, Vec<(&ExtractedFile, &ExtractedSymbol)>> = HashMap::new();
    let mut fn_names: HashSet<String> = HashSet::new();
    let mut fn_def_counts: HashMap<String, usize> = HashMap::new();
    for file in snapshot {
        for sym in &file.symbols {
            by_name
                .entry(sym.name.as_str())
                .or_default()
                .push((file, sym));
            if sym.kind == "fn" {
                fn_names.insert(sym.name.clone());
                *fn_def_counts.entry(sym.name.clone()).or_insert(0) += 1;
            }
        }
    }
    SymbolIndex {
        by_name,
        fn_names,
        fn_def_counts,
    }
}

pub(super) fn build_navigation_index(snapshot: &[ExtractedFile]) -> NavigationIndex {
    let mut calls_by_name: HashMap<String, Vec<CallSiteAddress>> = HashMap::new();
    let mut files_by_path = HashMap::new();
    for (file_index, file) in snapshot.iter().enumerate() {
        files_by_path.insert(file.file_path.clone(), file_index);
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

pub(super) fn lookup_same_file_candidates<'a>(
    name: &str,
    file_path: &str,
    index: &'a SymbolIndex<'a>,
) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)> {
    index
        .by_name
        .get(name)
        .map(|defs| {
            defs.iter()
                .copied()
                .filter(|(file, sym)| file.file_path == file_path && is_callable_symbol(sym))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn lookup_global_callable_candidates<'a>(
    name: &str,
    index: &'a SymbolIndex<'a>,
) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)> {
    index
        .by_name
        .get(name)
        .map(|defs| {
            defs.iter()
                .copied()
                .filter(|(_, sym)| is_callable_symbol(sym))
                .collect()
        })
        .unwrap_or_default()
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
    const EXTENSIONS: [&str; 4] = ["ts", "tsx", "js", "jsx"];
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(normalize_relative_path(&base));
    } else {
        for ext in EXTENSIONS {
            candidates.push(normalize_relative_path(&base.with_extension(ext)));
        }
        for ext in EXTENSIONS {
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
    index.by_name.get(name).is_some_and(|defs| {
        defs.iter().any(|(file, def)| {
            def.kind == "fn"
                && file.file_path == hit.file_path
                && def.range.start_line <= hit.line_number
                && hit.line_number <= def.range.end_line
        })
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
        let (start, end) = (sym.range.start_line, sym.range.end_line);
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
}
