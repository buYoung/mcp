//! Supporting evidence from the same immutable buffer as a live read/grep response.
//! Local dependencies and declaration contracts inform Jev's relevance judgment.
//! Gaps describe evidence coverage; they do not decide retention on the server.
use super::CapturedFile;
use crate::parser::{CallSite, ExtractedSymbol};
use crate::tools::search::jev::{
    is_callable_kind, simple_name, MAX_COMPLETE_BODY_BYTES, MAX_LINKS_PER_QUESTION,
};
use std::collections::{HashSet, VecDeque};
use std::fmt::Write;

#[derive(Default)]
pub(super) struct SupportingContext {
    pub text: String,
    pub has_missing: bool,
    pub is_clipped: bool,
    notes: HashSet<&'static str>,
}

impl SupportingContext {
    pub(super) fn note_gap(&mut self, reason: &'static str) {
        self.has_missing = true;
        if !self.notes.insert(reason) {
            return;
        }
        let note = format!("\nEvidence gap: {reason}\n");
        if self.text.len().saturating_add(note.len()) <= MAX_COMPLETE_BODY_BYTES {
            self.text.push_str(&note);
        } else {
            self.is_clipped = true;
        }
    }
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
    if seen
        .iter()
        .any(|&(first, last)| first <= start && end <= last)
    {
        return;
    }
    let Some(slice) = start.checked_sub(1).and_then(|first| lines.get(first..end)) else {
        output.note_gap("A supporting source range is unavailable.");
        return;
    };
    let mut source = String::new();
    for (offset, line) in slice.iter().enumerate() {
        let _ = writeln!(source, "{}→{line}", start + offset);
    }
    if crate::tools::search::jev::is_masked_unavailable(&source) {
        output.note_gap("Redaction left a supporting excerpt unavailable.");
        return;
    }
    let excerpt = format!(
        "\n{role}: {}:L{start}-L{end}\n{source}",
        crate::redact::source(&file.path)
    );
    if output.text.len().saturating_add(excerpt.len()) > MAX_COMPLETE_BODY_BYTES {
        output.note_gap("Supporting excerpts exceed the evidence byte budget.");
        output.is_clipped = true;
        return;
    }
    output.text.push_str(&excerpt);
    seen.insert((start, end));
}

fn append_contract(
    output: &mut SupportingContext,
    file: &CapturedFile,
    index: usize,
    lines: &[&str],
    seen: &mut HashSet<(usize, usize)>,
) {
    let Some((start, end)) = file.declaration_headers.get(index).copied().flatten() else {
        output.note_gap("A declaration header could not be captured.");
        return;
    };
    append_range(
        output,
        file,
        lines,
        start,
        end,
        "Declaration contract (body not expanded)",
        seen,
    );
}

fn append_owner_contract(
    output: &mut SupportingContext,
    file: &CapturedFile,
    index: usize,
    lines: &[&str],
    seen: &mut HashSet<(usize, usize)>,
) {
    let symbol = &file.file.symbols[index];
    let container = if crate::declarations::container(symbol) {
        Some(index)
    } else {
        file.file
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, parent)| {
                crate::declarations::container(parent)
                    && symbol.owner.as_deref() == Some(parent.name.as_str())
                    && crate::declarations::contains(parent, symbol)
            })
            .min_by_key(|(_, parent)| parent.range.end_line - parent.range.start_line)
            .map(|(index, _)| index)
    };
    let Some(container) = container else { return };
    append_contract(output, file, container, lines, seen);
    let parent = &file.file.symbols[container];
    let referenced_names: HashSet<_> = file
        .file
        .navigation
        .iter()
        .flat_map(|navigation| &navigation.references)
        .filter(|reference| contains(symbol, reference.range.start_line))
        .map(|reference| simple_name(&reference.name))
        .chain(
            file.file
                .navigation
                .iter()
                .flat_map(|navigation| &navigation.calls)
                .filter(|call| contains(symbol, call.range.start_line))
                .map(|call| simple_name(&call.name)),
        )
        .collect();
    let mut members: Vec<_> = file
        .file
        .symbols
        .iter()
        .enumerate()
        .filter(|(member, child)| {
            *member != index
                && child.owner.as_deref() == Some(parent.name.as_str())
                && crate::declarations::contains(parent, child)
                && !file.file.symbols.iter().any(|scope| {
                    is_callable_kind(&scope.kind)
                        && crate::declarations::contains(scope, child)
                        && crate::declarations::contains(parent, scope)
                })
        })
        .collect();
    // Stable ordering keeps the ordinary declaration order within each tier, but a
    // referenced member must not lose its contract to earlier unrelated siblings.
    members.sort_by_key(|(_, child)| !referenced_names.contains(simple_name(&child.name)));
    for (ordinal, (member, _)) in members.into_iter().enumerate() {
        if ordinal == MAX_LINKS_PER_QUESTION * 2 {
            output.note_gap("The enclosing member-contract list is bounded.");
            output.is_clipped = true;
            break;
        }
        append_contract(output, file, member, lines, seen);
    }
}

/// Source for a cross-file candidate, with its imports and enclosing member contracts.
/// The caller establishes the candidate location; this does not prove runtime dispatch.
pub(super) fn supporting_declaration(file: &CapturedFile, index: usize) -> SupportingContext {
    let mut output = SupportingContext::default();
    let symbol = &file.file.symbols[index];
    let lines: Vec<_> = file.source.split('\n').collect();
    let mut seen = HashSet::new();
    if is_callable_kind(&symbol.kind) {
        if let Some(bounds) = file
            .bounds
            .iter()
            .find(|bounds| bounds.symbol_index == index)
        {
            append_range(
                &mut output,
                file,
                &lines,
                bounds.start,
                bounds.end,
                "Supporting callable",
                &mut seen,
            );
        } else {
            output.note_gap("The supporting callable body could not be captured.");
        }
    } else if !crate::declarations::container(symbol) {
        append_range(
            &mut output,
            file,
            &lines,
            symbol.range.start_line,
            symbol.range.end_line_inclusive(),
            "Supporting declaration",
            &mut seen,
        );
    }
    append_contract(&mut output, file, index, &lines, &mut seen);
    append_owner_contract(&mut output, file, index, &lines, &mut seen);
    if let Some(navigation) = &file.file.navigation {
        // Bind only the supplied source, not every unrelated method in the file.
        // Glob imports cannot be resolved by a single local spelling and stay visible.
        let in_excerpt = |line| {
            seen.iter()
                .any(|&(start, end)| start <= line && line <= end)
        };
        let referenced_names: HashSet<_> = navigation
            .references
            .iter()
            .filter(|reference| in_excerpt(reference.range.start_line))
            .map(|reference| reference.name.as_str())
            .chain(
                navigation
                    .calls
                    .iter()
                    .filter(|call| in_excerpt(call.range.start_line))
                    .map(|call| {
                        call.receiver
                            .as_deref()
                            .unwrap_or(&call.name)
                            .split(['.', '[', ':', '?', '('])
                            .next()
                            .unwrap_or("")
                    }),
            )
            .collect();
        for import in navigation.imports.iter().filter(|import| {
            referenced_names.contains(import.local_name.as_str())
                || import.local_name.is_empty()
                || matches!(import.kind, crate::parser::ImportKind::Glob)
        }) {
            append_range(
                &mut output,
                file,
                &lines,
                import.range.start_line,
                import.range.end_line_inclusive(),
                "Import binding (not target identity proof)",
                &mut seen,
            );
        }
    }
    output
}

/// Lexical member evidence, not a retention link or a runtime dataflow claim. Match
/// the enclosing container span as well as its name so same-named types do not mix.
fn member_support(file: &CapturedFile, candidate: usize) -> Vec<usize> {
    let symbol = &file.file.symbols[candidate];
    let container = |index: usize| {
        let child = &file.file.symbols[index];
        file.file
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, parent)| {
                crate::declarations::container(parent)
                    && child.owner.as_deref() == Some(parent.name.as_str())
                    && crate::declarations::contains(parent, child)
            })
            .min_by_key(|(_, parent)| parent.range.end_line - parent.range.start_line)
            .map(|(index, _)| index)
    };
    let Some(owner) = container(candidate) else {
        return Vec::new();
    };
    let own_uses: Vec<_> = file
        .self_member_uses
        .iter()
        .filter(|usage| contains(symbol, usage.range.start_line))
        .collect();
    let mut support = Vec::new();
    for usage in &file.self_member_uses {
        if contains(symbol, usage.range.start_line) {
            continue;
        }
        let is_reference =
            usage.name == simple_name(&symbol.name) && is_callable_kind(&symbol.kind);
        let shares_assignment = own_uses
            .iter()
            .any(|own| own.name == usage.name && (own.is_assignment || usage.is_assignment));
        if !is_reference && !shares_assignment {
            continue;
        }
        let other = file
            .file
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, other)| {
                is_callable_kind(&other.kind) && contains(other, usage.range.start_line)
            })
            .min_by_key(|(_, other)| other.range.end_line - other.range.start_line)
            .map(|(index, _)| index);
        if let Some(other) = other.filter(|&index| container(index) == Some(owner)) {
            if !support.contains(&other) {
                support.push(other);
            }
        }
    }
    support
}

pub(super) fn capture(file: &CapturedFile, candidate: usize) -> SupportingContext {
    let mut output = SupportingContext::default();
    let Some(navigation) = file.file.navigation.as_ref() else {
        output.note_gap("Navigation observations are unavailable for this source.");
        return output;
    };
    let lines: Vec<_> = file.source.split('\n').collect();
    let mut ranges = HashSet::new();
    // Process the candidate's own bindings first, then direct support before deeper
    // dependencies. A caller's dependency chain must not consume the budget first.
    let mut pending = VecDeque::from([candidate]);
    let mut visited = HashSet::new();
    let member_support = member_support(file, candidate);
    if member_support.len() > MAX_LINKS_PER_QUESTION {
        output.note_gap("Same-container member uses exceed the link budget.");
        output.is_clipped = true;
    }
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
                pending.push_back(owner);
                if pending.len() > MAX_LINKS_PER_QUESTION * 2 + 1 {
                    output.note_gap("The same-file caller list exceeds the link budget.");
                    output.is_clipped = true;
                    break;
                }
            }
        }
    }
    while let Some(index) = pending.pop_front() {
        if !visited.insert(index) {
            continue;
        }
        if visited.len() > MAX_LINKS_PER_QUESTION * 2 + 1 {
            output.note_gap("Same-file dependencies exceed the link budget.");
            output.is_clipped = true;
            break;
        }
        let symbol = &file.file.symbols[index];
        if file.reference_gaps.iter().any(|gap| {
            gap.start_line <= symbol.range.end_line_inclusive()
                && symbol.range.start_line <= gap.end_line_inclusive()
        }) {
            output.note_gap("Some dependency syntax was not resolved by the local extractor.");
        }
        if index != candidate {
            let (start, end) = if is_callable_kind(&symbol.kind) {
                let Some(bounds) = file
                    .bounds
                    .iter()
                    .find(|bounds| bounds.symbol_index == index)
                else {
                    output.note_gap("A supporting callable has no verified body boundary.");
                    continue;
                };
                (bounds.start, bounds.end)
            } else if crate::declarations::container(symbol) {
                append_contract(&mut output, file, index, &lines, &mut ranges);
                append_owner_contract(&mut output, file, index, &lines, &mut ranges);
                continue;
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
        append_owner_contract(&mut output, file, index, &lines, &mut ranges);
        for reference in navigation
            .references
            .iter()
            .filter(|reference| contains(symbol, reference.range.start_line))
        {
            // Imports are evidence of bindings. Cross-file augmentation may add the
            // declaration, but this observation alone does not supply an implementation.
            for import in navigation
                .imports
                .iter()
                .filter(|import| import.local_name == reference.name)
            {
                output.note_gap("Import bindings alone do not establish dependency behavior; inspect any supplied cross-file evidence.");
                append_range(
                    &mut output,
                    file,
                    &lines,
                    import.range.start_line,
                    import.range.end_line_inclusive(),
                    "Imported dependency binding",
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
                        pending.push_back(definition);
                    } else {
                        output.note_gap(
                            "A referenced same-file name has multiple possible declarations.",
                        );
                    }
                } else {
                    for binding in navigation.local_bindings.iter().filter(|binding| {
                        binding.name == reference.name
                            && !contains(symbol, binding.range.start_line)
                    }) {
                        output.note_gap("An outer binding's runtime value or scope is unresolved.");
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
                pending.push_back(target);
            } else if !is_local_value_call(file, call, index) {
                output.note_gap("Some non-local call targets are unresolved; inspect any supplied cross-file evidence.");
            }
        }
        if index == candidate {
            // Direct calls/import dependencies are queued first. Member-use evidence
            // must not displace the implementation that a wrapper actually invokes.
            pending.extend(member_support.iter().copied().take(MAX_LINKS_PER_QUESTION));
        }
    }
    output
}
