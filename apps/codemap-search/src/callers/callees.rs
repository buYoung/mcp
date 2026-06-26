//! Callee discovery stage: re-reads a matched symbol's own body from disk and intersects
//! the identifiers it invokes with the snapshot's global `fn`-name set. Independent of the
//! workspace [`super::scan::ScanResult`] — it reads the symbol's source range directly.

use std::collections::HashSet;
use std::path::Path;

use crate::parser::{CallSite, ExtractedFile, ExtractedSymbol};

use super::scan::is_ident_char;
use super::symbols::{
    infer_owner_hint, lookup_by_owner_and_name, lookup_global_callable_candidates,
    lookup_same_file_candidates, SymbolIndex,
};
use super::AnnotationRuntimeState;
use super::{qualified_name, read_workspace_file};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscoveredCallee {
    pub(super) name: String,
    pub(super) display: String,
    pub(super) is_precise: bool,
}

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

fn call_is_inside_symbol(call: &CallSite, sym: &ExtractedSymbol) -> bool {
    sym.range.start_line <= call.range.start_line && call.range.end_line <= sym.range.end_line
}

fn resolve_navigation_callee_display(
    call: &CallSite,
    call_file: &ExtractedFile,
    index: &SymbolIndex<'_>,
    navigation_context_enabled: bool,
) -> DiscoveredCallee {
    if !navigation_context_enabled {
        return DiscoveredCallee {
            name: call.name.clone(),
            display: callee_display(&call.name, index),
            is_precise: false,
        };
    }

    if let Some(receiver) = call.receiver.as_deref() {
        // fallback: `this`, optional chains, destructured calls, factory-return receivers,
        // and interface dispatch all need scope/type data beyond the local binding list.
        if let Some(owner_hint) = call_file
            .navigation
            .as_ref()
            .and_then(|navigation| infer_owner_hint(receiver, &navigation.local_bindings))
        {
            let owner_candidates = lookup_by_owner_and_name(&owner_hint, &call.name, index);
            if owner_candidates.len() == 1 {
                let (file, sym) = owner_candidates[0];
                return DiscoveredCallee {
                    name: call.name.clone(),
                    display: qualified_name(sym, &file.file_path),
                    is_precise: true,
                };
            }
        }
    }

    let call_file_path = &call_file.file_path;
    let same_file = lookup_same_file_candidates(&call.name, call_file_path, index);
    if same_file.len() == 1 {
        let (file, sym) = same_file[0];
        return DiscoveredCallee {
            name: call.name.clone(),
            display: qualified_name(sym, &file.file_path),
            is_precise: true,
        };
    }
    if same_file.len() > 1 {
        return DiscoveredCallee {
            name: call.name.clone(),
            display: call.name.clone(),
            is_precise: false,
        };
    }
    let global = lookup_global_callable_candidates(&call.name, index);
    if global.len() == 1 {
        let (file, sym) = global[0];
        return DiscoveredCallee {
            name: call.name.clone(),
            display: qualified_name(sym, &file.file_path),
            is_precise: true,
        };
    }
    DiscoveredCallee {
        name: call.name.clone(),
        display: call.name.clone(),
        is_precise: false,
    }
}

pub(super) fn discover_callees_with_navigation(
    sym: &ExtractedSymbol,
    file: &ExtractedFile,
    index: &SymbolIndex<'_>,
    runtime_state: AnnotationRuntimeState,
    navigation_context_enabled: bool,
    root: &Path,
) -> Vec<DiscoveredCallee> {
    if runtime_state.suppresses_navigation() {
        return discover_callees(sym, &file.file_path, &index.fn_names, root)
            .into_iter()
            .map(|name| DiscoveredCallee {
                display: callee_display(&name, index),
                name,
                is_precise: false,
            })
            .collect();
    }

    let Some(navigation) = &file.navigation else {
        return discover_callees(sym, &file.file_path, &index.fn_names, root)
            .into_iter()
            .map(|name| DiscoveredCallee {
                display: callee_display(&name, index),
                name,
                is_precise: false,
            })
            .collect();
    };

    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for call in &navigation.calls {
        if call.name == sym.name
            || !index.fn_names.contains(&call.name)
            || !call_is_inside_symbol(call, sym)
        {
            continue;
        }
        let callee =
            resolve_navigation_callee_display(call, file, index, navigation_context_enabled);
        let key = format!("{}:{}:{}", callee.name, callee.display, callee.is_precise);
        if seen.insert(key) {
            found.push(callee);
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
