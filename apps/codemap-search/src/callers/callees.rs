//! Callee discovery stage: re-reads a matched symbol's own body from disk and intersects
//! the identifiers it invokes with the snapshot's global `fn`-name set. Independent of the
//! workspace [`super::scan::ScanResult`] — it reads the symbol's source range directly.

use std::collections::HashSet;
use std::path::Path;

use crate::parser::{CallSite, ExtractedFile, ExtractedSymbol};

use super::scan::is_ident_char;
use super::source::SourceSyntax;
use super::symbols::{
    definition_is_compatible, infer_owner_hint, lookup_by_owner_and_name,
    lookup_compatible_candidates, SymbolIndex,
};
use super::AnnotationRuntimeState;
use super::{qualified_name, read_workspace_file};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscoveredCallee {
    pub(super) name: String,
    pub(super) display: String,
    pub(super) is_precise: bool,
    pub(super) is_unresolved: bool,
}

/// Discover depth-1 callees of `sym`: names invoked as `identifier(` inside the symbol's
/// full source range that are in the snapshot's global `fn`-name set, excluding the
/// symbol's own name. Reads the symbol's full range from disk (not the display snippet).
pub(super) fn discover_callees(
    sym: &ExtractedSymbol,
    file_path: &str,
    index: &SymbolIndex<'_>,
    root: &Path,
) -> Vec<DiscoveredCallee> {
    let content = match read_workspace_file(file_path, root) {
        Some(c) => c,
        None => return Vec::new(),
    };
    let mut source = content.into_bytes();
    super::test_code::TestCodeFilter::from_config(root).mask_source(file_path, &mut source);
    let Some(syntax) = SourceSyntax::parse(file_path, &source) else {
        return Vec::new();
    };
    let content = String::from_utf8(source).ok().unwrap_or_default();
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let start = sym.range.start_line.saturating_sub(1);
    let end = sym.range.end_line_inclusive().min(lines.len());
    if start >= end {
        return Vec::new();
    }
    let mut body_start: usize = lines[..start].iter().map(|line| line.len()).sum();
    let mut body_end: usize = lines[..end].iter().map(|line| line.len()).sum();
    if sym.range.start_col > 0
        && sym.range.end_col > 0
        && (sym.range.start_line, sym.range.start_col) < (sym.range.end_line, sym.range.end_col)
    {
        body_start += sym.range.start_col - 1;
        let end_row = sym.range.end_line.saturating_sub(1).min(lines.len());
        body_end = lines[..end_row]
            .iter()
            .map(|line| line.len())
            .sum::<usize>()
            + sym.range.end_col
            - 1;
    }
    let Some(body) = content.get(body_start..body_end) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let bytes: Vec<(usize, char)> = body.char_indices().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if is_ident_start(bytes[i].1) {
            let begin = i;
            while i < bytes.len() && is_ident_char(bytes[i].1) {
                i += 1;
            }
            let ident: String = bytes[begin..i].iter().map(|(_, ch)| ch).collect();
            // Skip whitespace, then require `(` for a call.
            let mut j = i;
            while j < bytes.len() && bytes[j].1.is_whitespace() {
                j += 1;
            }
            let is_call = j < bytes.len() && bytes[j].1 == '(';
            let name_start = body_start + bytes[begin].0;
            let is_member = syntax.is_member_access(name_start..name_start + ident.len());
            if is_call
                && ident != sym.name
                && index.function_definition_count(&ident) > 0
                && syntax.is_code(name_start..name_start + ident.len())
                && !lookup_compatible_candidates(&ident, file_path, is_member, index).is_empty()
            {
                let display = callee_display(&ident, index, file_path, is_member);
                if seen.insert(display.clone()) {
                    found.push(DiscoveredCallee {
                        name: ident,
                        display,
                        is_precise: false,
                        is_unresolved: false,
                    });
                }
            }
        } else {
            i += 1;
        }
    }
    found
}

fn call_is_inside_symbol(call: &CallSite, sym: &ExtractedSymbol) -> bool {
    (sym.range.start_line, sym.range.start_col) <= (call.range.start_line, call.range.start_col)
        && (call.range.end_line, call.range.end_col) <= (sym.range.end_line, sym.range.end_col)
}

fn resolve_navigation_callee_display(
    call: &CallSite,
    call_file: &ExtractedFile,
    index: &SymbolIndex<'_>,
    navigation_context_enabled: bool,
    is_member: bool,
) -> DiscoveredCallee {
    if !navigation_context_enabled {
        return DiscoveredCallee {
            name: call.name.clone(),
            display: callee_display(&call.name, index, &call_file.file_path, is_member),
            is_precise: false,
            is_unresolved: false,
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
            let owner_candidates: Vec<_> = lookup_by_owner_and_name(&owner_hint, &call.name, index)
                .into_iter()
                .filter(|(file, sym)| {
                    definition_is_compatible(&call_file.file_path, is_member, &file.file_path, sym)
                })
                .collect();
            if owner_candidates.len() == 1 {
                let (file, sym) = owner_candidates[0];
                return DiscoveredCallee {
                    name: call.name.clone(),
                    display: definition_display(file, sym),
                    is_precise: true,
                    is_unresolved: false,
                };
            }
        }
    }

    let call_file_path = &call_file.file_path;
    let global = lookup_compatible_candidates(&call.name, call_file_path, is_member, index);
    let same_file: Vec<_> = global
        .iter()
        .copied()
        .filter(|(file, _)| file.file_path == *call_file_path)
        .collect();
    if same_file.len() == 1 {
        let (file, sym) = same_file[0];
        return DiscoveredCallee {
            name: call.name.clone(),
            display: definition_display(file, sym),
            is_precise: true,
            is_unresolved: false,
        };
    }
    if same_file.len() > 1 {
        return DiscoveredCallee {
            name: call.name.clone(),
            display: call.name.clone(),
            is_precise: false,
            is_unresolved: false,
        };
    }
    if global.len() == 1 {
        let (file, sym) = global[0];
        return DiscoveredCallee {
            name: call.name.clone(),
            display: definition_display(file, sym),
            is_precise: true,
            is_unresolved: false,
        };
    }
    DiscoveredCallee {
        name: call.name.clone(),
        display: call.name.clone(),
        is_precise: false,
        is_unresolved: false,
    }
}

pub(super) fn discover_callees_with_navigation(
    sym: &ExtractedSymbol,
    file: &ExtractedFile,
    index: &SymbolIndex<'_>,
    runtime_state: AnnotationRuntimeState,
    navigation_context_enabled: bool,
    root: &Path,
    resolver: &super::resolution::SourceResolver<'_>,
) -> Vec<DiscoveredCallee> {
    if super::resolution::supports(&file.file_path) {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        for call in resolver.calls(file) {
            if !call_is_inside_symbol(&call, sym) {
                continue;
            }
            let target = (!runtime_state.suppresses_navigation())
                .then(|| resolver.resolve_call(file, &call))
                .flatten();
            if target.is_some_and(|target| {
                target.file.file_path == file.file_path && target.symbol.range == sym.range
            }) {
                continue;
            }
            let callee = match target {
                Some(target) => DiscoveredCallee {
                    name: target.symbol.name.clone(),
                    display: definition_display(target.file, target.symbol),
                    is_precise: navigation_context_enabled && target.is_precise,
                    is_unresolved: false,
                },
                None => DiscoveredCallee {
                    name: call.name.clone(),
                    display: call.name,
                    is_precise: false,
                    is_unresolved: true,
                },
            };
            if seen.insert((callee.display.clone(), callee.is_unresolved)) {
                found.push(callee);
            }
        }
        found.sort_by_key(|callee| callee.is_unresolved);
        return found;
    }
    if runtime_state.suppresses_navigation() {
        return discover_callees(sym, &file.file_path, index, root);
    }

    let Some(navigation) = &file.navigation else {
        return discover_callees(sym, &file.file_path, index, root);
    };

    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let syntax = read_workspace_file(&file.file_path, root)
        .and_then(|source| SourceSyntax::parse(&file.file_path, source.as_bytes()));
    for call in &navigation.calls {
        if call.name == sym.name
            || index.function_definition_count(&call.name) == 0
            || !call_is_inside_symbol(call, sym)
        {
            continue;
        }
        let is_member = syntax
            .as_ref()
            .and_then(|syntax| syntax.is_member_call(call))
            .unwrap_or(call.receiver.is_some());
        if lookup_compatible_candidates(&call.name, &file.file_path, is_member, index).is_empty() {
            continue;
        }
        let callee = resolve_navigation_callee_display(
            call,
            file,
            index,
            navigation_context_enabled,
            is_member,
        );
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

fn definition_display(file: &ExtractedFile, sym: &ExtractedSymbol) -> String {
    format!(
        "{} — {}:{}",
        qualified_name(sym, &file.file_path),
        file.file_path,
        sym.range.start_line
    )
}

/// Include the definition location when exactly one `fn` of that name exists in the
/// snapshot. Ambiguous names retain the bare form instead of inventing a target.
pub(super) fn callee_display(
    name: &str,
    index: &SymbolIndex<'_>,
    file_path: &str,
    is_member: bool,
) -> String {
    let defs = lookup_compatible_candidates(name, file_path, is_member, index);
    if defs.len() == 1 {
        let (file, sym) = defs[0];
        definition_display(file, sym)
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
    fn test_lazy_exclusions_apply_to_source_resolved_callees() {
        use crate::parser::CodeExtractor;
        let source = "fn ordinary() { test_only(); }\n#[test]\nfn test_only() {}\nfn outer() {\n #[test]\n fn nested() { helper(); }\n}\nfn helper() {}\n";
        let (_directory, root) = crate::callers::fixtures::write_repo(&[("main.rs", source)]);
        let snapshot = vec![crate::parser::TreeSitterExtractor::new()
            .extract(source, "main.rs")
            .unwrap()];
        let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
        let index = build_symbol_index(&snapshot, Some(&filter));
        let resolver = super::super::resolution::SourceResolver::new(&snapshot, &root);
        for name in ["ordinary", "outer"] {
            let symbol = snapshot[0]
                .symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .unwrap();
            let callees = discover_callees_with_navigation(
                symbol,
                &snapshot[0],
                &index,
                AnnotationRuntimeState::default(),
                true,
                &root,
                &resolver,
            );
            assert!(
                callees.iter().all(|callee| callee.is_unresolved),
                "{name}: {callees:?}"
            );
            assert!(
                !callees.iter().any(|callee| callee.name == "helper"),
                "{name}: {callees:?}"
            );
        }
    }

    #[test]
    fn test_callee_display_unambiguous_qualifies_ambiguous_bare() {
        let snapshot = vec![
            file("a.rs", vec![sym("alpha", "fn", 1, 3, Some("Engine"))]),
            file(
                "b.rs",
                vec![sym("beta", "fn", 1, 3, None), sym("beta", "fn", 5, 7, None)],
            ),
        ];
        let index = build_symbol_index(&snapshot, None);
        // alpha: exactly one fn def → qualified via owner.
        assert_eq!(
            callee_display("alpha", &index, "a.rs", false),
            "Engine::alpha — a.rs:1"
        );
        // beta: two defs → bare.
        assert_eq!(callee_display("beta", &index, "a.rs", false), "beta");
    }
}
