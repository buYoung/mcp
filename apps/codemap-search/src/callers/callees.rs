//! Callee discovery stage: re-reads a matched symbol's own body from disk and intersects
//! the identifiers it invokes with the snapshot's global `fn`-name set. Independent of the
//! workspace [`super::scan::ScanResult`] — it reads the symbol's source range directly.

use std::collections::HashSet;
use std::path::Path;

use crate::parser::ExtractedSymbol;

use super::scan::is_ident_char;
use super::symbols::SymbolIndex;
use super::{qualified_name, read_workspace_file};

/// Discover depth-1 callees of `sym`: names invoked as `identifier(` inside the symbol's
/// full source range that are in the snapshot's global `fn`-name set, excluding the
/// symbol's own name. Reads the symbol's full range from disk (not the display snippet).
pub(super) fn discover_callees(
    sym: &ExtractedSymbol,
    file_path: &str,
    fn_names: &HashSet<String>,
    root: &Path,
) -> Vec<String> {
    let content = match read_workspace_file(file_path, root) {
        Some(c) => c,
        None => return Vec::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = sym.range.start_line.saturating_sub(1);
    let end = sym.range.end_line.min(lines.len());
    if start >= end {
        return Vec::new();
    }
    let body = lines[start..end].join("\n");
    let mut found: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let begin = i;
            while i < bytes.len() && is_ident_char(bytes[i]) {
                i += 1;
            }
            let ident: String = bytes[begin..i].iter().collect();
            // Skip whitespace, then require `(` for a call.
            let mut j = i;
            while j < bytes.len() && (bytes[j] == ' ' || bytes[j] == '\t') {
                j += 1;
            }
            let is_call = j < bytes.len() && bytes[j] == '(';
            if is_call
                && ident != sym.name
                && fn_names.contains(&ident)
                && seen.insert(ident.clone())
            {
                found.push(ident);
            }
        } else {
            i += 1;
        }
    }
    found
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

/// Render the qualified form of a callee name when exactly one `fn` of that name exists in
/// the snapshot (unambiguous owner); otherwise the bare name.
pub(super) fn callee_display(name: &str, index: &SymbolIndex<'_>) -> String {
    let defs: Vec<_> = index
        .by_name
        .get(name)
        .map(|v| v.iter().filter(|(_, s)| s.kind == "fn").collect::<Vec<_>>())
        .unwrap_or_default();
    if defs.len() == 1 {
        let (file, sym) = defs[0];
        qualified_name(sym, &file.file_path)
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callers::fixtures::{file, sym};
    use crate::callers::symbols::build_symbol_index;

    #[test]
    fn test_callee_display_unambiguous_qualifies_ambiguous_bare() {
        let snapshot = vec![
            file("a.rs", vec![sym("alpha", "fn", 1, 3, Some("Engine"))]),
            file(
                "b.rs",
                vec![sym("beta", "fn", 1, 3, None), sym("beta", "fn", 5, 7, None)],
            ),
        ];
        let index = build_symbol_index(&snapshot);
        // alpha: exactly one fn def → qualified via owner.
        assert_eq!(callee_display("alpha", &index), "Engine::alpha");
        // beta: two defs → bare.
        assert_eq!(callee_display("beta", &index), "beta");
    }
}
