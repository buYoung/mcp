//! Supporting evidence from the same immutable buffer as a live read/grep response.
//! Local dependencies are bounded; unavailable runtime/imported dependencies preserve
//! the candidate instead of treating missing communication context as a negative answer.
use super::CapturedFile;
use crate::parser::{CallSite, ExtractedSymbol};
use crate::tools::search::jev::{
    is_callable_kind, simple_name, MAX_COMPLETE_BODY_BYTES, MAX_LINKS_PER_QUESTION,
};
use std::collections::HashSet;
use std::fmt::Write;

#[derive(Default)]
pub(super) struct SupportingContext {
    pub text: String,
    pub has_missing: bool,
    pub is_clipped: bool,
}

fn contains(symbol: &ExtractedSymbol, line: usize) -> bool {
    symbol.range.start_line <= line && line <= symbol.range.end_line_inclusive()
}

fn unique(mut indices: impl Iterator<Item = usize>) -> Option<usize> {
    let first = indices.next()?;
    indices.next().is_none().then_some(first)
}

fn call_owner(file: &CapturedFile, call: &CallSite) -> Option<usize> {
    file.file
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, symbol)| {
            is_callable_kind(&symbol.kind) && contains(symbol, call.range.start_line)
        })
        .min_by_key(|(_, symbol)| symbol.range.end_line - symbol.range.start_line)
        .map(|(index, _)| index)
}

fn local_target(file: &CapturedFile, call: &CallSite, owner: usize) -> Option<usize> {
    let receiver = call.receiver.as_deref().unwrap_or("");
    let wanted_owner = if matches!(receiver, "this" | "self" | "Self" | "$this") {
        file.file.symbols[owner].owner.as_deref()
    } else if receiver.is_empty() {
        None
    } else {
        Some(receiver)
    };
    // No unique-name fallback across unrelated receivers: an unresolved member may
    // belong to an injected service even when a local method has the same name.
    unique(
        file.file
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| {
                is_callable_kind(&symbol.kind)
                    && simple_name(&symbol.name) == simple_name(&call.name)
                    && symbol.owner.as_deref() == wanted_owner
            })
            .map(|(index, _)| index),
    )
}

fn is_local_value_call(file: &CapturedFile, call: &CallSite, owner: usize) -> bool {
    let Some(receiver) = call.receiver.as_deref() else {
        return false;
    };
    let root = receiver
        .split(['.', '[', ':', '?', '('])
        .next()
        .unwrap_or("");
    if matches!(root, "this" | "self" | "Self" | "$this") {
        return false;
    }
    let symbol = &file.file.symbols[owner];
    file.file.navigation.as_ref().is_some_and(|navigation| {
        navigation.local_bindings.iter().any(|binding| {
            binding.name == root
                && contains(symbol, binding.range.start_line)
                && binding.range.start_line <= call.range.start_line
        })
    })
}

fn append_range(
    output: &mut SupportingContext,
    file: &CapturedFile,
    lines: &[&str],
    start: usize,
    end: usize,
    role: &str,
    seen: &mut HashSet<(usize, usize)>,
) {
    if !seen.insert((start, end)) {
        return;
    }
    let Some(slice) = start.checked_sub(1).and_then(|first| lines.get(first..end)) else {
        output.has_missing = true;
        return;
    };
    let mut source = String::new();
    for (offset, line) in slice.iter().enumerate() {
        let _ = writeln!(source, "{}→{line}", start + offset);
    }
    if crate::tools::search::jev::is_masked_unavailable(&source) {
        output.has_missing = true;
        return;
    }
    let excerpt = format!(
        "\n{role}: {}:L{start}-L{end}\n{source}",
        crate::redact::source(&file.path)
    );
    if output.text.len().saturating_add(excerpt.len()) > MAX_COMPLETE_BODY_BYTES {
        output.has_missing = true;
        output.is_clipped = true;
        return;
    }
    output.text.push_str(&excerpt);
}

pub(super) fn capture(file: &CapturedFile, candidate: usize) -> SupportingContext {
    let mut output = SupportingContext::default();
    let Some(navigation) = file.file.navigation.as_ref() else {
        output.has_missing = true;
        return output;
    };
    let lines: Vec<_> = file.source.split('\n').collect();
    let mut ranges = HashSet::new();
    let mut pending = vec![candidate];
    let mut visited = HashSet::new();
    // Capture direct callers as well as callees. A leaf helper may implement a flow
    // through its callers without naming that flow in its own body.
    for call in navigation
        .calls
        .iter()
        .filter(|call| simple_name(&call.name) == simple_name(&file.file.symbols[candidate].name))
    {
        if let Some(owner) = call_owner(file, call) {
            if owner != candidate
                && !pending.contains(&owner)
                && local_target(file, call, owner) == Some(candidate)
            {
                pending.push(owner);
                if pending.len() > MAX_LINKS_PER_QUESTION * 2 + 1 {
                    output.has_missing = true;
                    output.is_clipped = true;
                    break;
                }
            }
        }
    }
    while let Some(index) = pending.pop() {
        if !visited.insert(index) {
            continue;
        }
        if visited.len() > MAX_LINKS_PER_QUESTION * 2 + 1 {
            output.has_missing = true;
            output.is_clipped = true;
            break;
        }
        let symbol = &file.file.symbols[index];
        if index != candidate {
            let (start, end) = if is_callable_kind(&symbol.kind) {
                let Some(bounds) = file
                    .bounds
                    .iter()
                    .find(|bounds| bounds.symbol_index == index)
                else {
                    output.has_missing = true;
                    continue;
                };
                (bounds.start, bounds.end)
            } else {
                (symbol.range.start_line, symbol.range.end_line_inclusive())
            };
            append_range(
                &mut output,
                file,
                &lines,
                start,
                end,
                "Same-file supporting declaration",
                &mut ranges,
            );
        }
        if let Some(owner) = symbol.owner.as_deref() {
            if let Some(container) = file.file.symbols.iter().find(|container| {
                !is_callable_kind(&container.kind)
                    && container.name == owner
                    && contains(container, symbol.range.start_line)
            }) {
                append_range(
                    &mut output,
                    file,
                    &lines,
                    container.range.start_line,
                    container.range.start_line,
                    "Enclosing declaration (header only)",
                    &mut ranges,
                );
            }
        }
        for reference in navigation
            .references
            .iter()
            .filter(|reference| contains(symbol, reference.range.start_line))
        {
            // An import declaration identifies a dependency but does not supply its
            // implementation. Do not let a negative score erase that uncertainty.
            for import in navigation
                .imports
                .iter()
                .filter(|import| import.local_name == reference.name)
            {
                output.has_missing = true;
                append_range(
                    &mut output,
                    file,
                    &lines,
                    import.range.start_line,
                    import.range.end_line_inclusive(),
                    "Imported dependency (implementation unavailable)",
                    &mut ranges,
                );
            }
            let is_local = navigation.local_bindings.iter().any(|binding| {
                binding.name == reference.name && contains(symbol, binding.range.start_line)
            });
            if !is_local {
                let mut definitions =
                    file.file
                        .symbols
                        .iter()
                        .enumerate()
                        .filter(|(other, definition)| {
                            *other != index
                                && definition.name == reference.name
                                && !contains(symbol, definition.range.start_line)
                                && (definition.owner.is_none() || definition.owner == symbol.owner)
                        });
                if let Some((definition, _)) = definitions.next() {
                    if definitions.next().is_none() {
                        pending.push(definition);
                    } else {
                        output.has_missing = true;
                    }
                } else {
                    for binding in navigation.local_bindings.iter().filter(|binding| {
                        binding.name == reference.name
                            && !contains(symbol, binding.range.start_line)
                    }) {
                        output.has_missing = true;
                        append_range(
                            &mut output,
                            file,
                            &lines,
                            binding.range.start_line,
                            binding.range.end_line_inclusive(),
                            "Outer binding (scope dependency)",
                            &mut ranges,
                        );
                    }
                }
            }
        }
        for call in navigation
            .calls
            .iter()
            .filter(|call| contains(symbol, call.range.start_line))
        {
            if let Some(target) = local_target(file, call, index) {
                pending.push(target);
            } else if !is_local_value_call(file, call, index) {
                output.has_missing = true;
            }
        }
    }
    output
}
