//! Cross-cutting snapshot symbol state plus call-site attribution: the per-name symbol
//! index built once from the codemap snapshot, and the attribution helpers that map a
//! scan hit to its enclosing definition and exclude same-named-definition ranges.

use std::collections::{HashMap, HashSet};

use crate::parser::{ExtractedFile, ExtractedSymbol};

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

pub(super) fn build_symbol_index(snapshot: &[ExtractedFile]) -> SymbolIndex<'_> {
    let mut by_name: HashMap<&str, Vec<(&ExtractedFile, &ExtractedSymbol)>> = HashMap::new();
    let mut fn_names: HashSet<String> = HashSet::new();
    let mut fn_def_counts: HashMap<String, usize> = HashMap::new();
    for file in snapshot {
        for sym in &file.symbols {
            by_name.entry(sym.name.as_str()).or_default().push((file, sym));
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
pub(super) fn enclosing_fn<'a>(file: &'a ExtractedFile, line: usize) -> Option<&'a ExtractedSymbol> {
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
            vec![sym("handler", "fn", 10, 10, None), sym("outer", "fn", 1, 50, None)],
        );
        let encl = enclosing_fn(&f, 10).unwrap();
        // The innermost (smallest span) wins: handler (10-10), not outer (1-50).
        assert_eq!(encl.name, "handler");
    }

    #[test]
    fn test_enclosing_fn_innermost_wins() {
        let f = file(
            "a.rs",
            vec![sym("outer", "fn", 1, 100, None), sym("inner", "fn", 40, 60, None)],
        );
        assert_eq!(enclosing_fn(&f, 50).unwrap().name, "inner");
        assert_eq!(enclosing_fn(&f, 5).unwrap().name, "outer");
        assert!(enclosing_fn(&f, 200).is_none());
    }
}
