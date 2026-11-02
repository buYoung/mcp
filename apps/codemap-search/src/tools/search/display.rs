//! Compact presentation copies after Jev has captured its unchanged evidence.
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(super) struct DisplayContext {
    notes: BTreeMap<String, usize>,
}

impl DisplayContext {
    pub fn compact(&mut self, text: &str) -> String {
        let mut output = String::with_capacity(text.len());
        let mut lines = text.split_inclusive('\n').peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            let indent = &line[..line.len() - line.trim_start().len()];
            if trimmed.starts_with("- _referenced in a non-call position") {
                let mut references: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
                let mut original = line.to_string();
                while let Some(next) = lines.peek() {
                    if next.len() - next.trim_start().len() <= indent.len() {
                        break;
                    }
                    let Some((location, _)) = next
                        .trim()
                        .strip_prefix("- ")
                        .and_then(|entry| entry.split_once(": `"))
                    else {
                        break;
                    };
                    let Some((path, number)) = location.rsplit_once(':') else {
                        break;
                    };
                    if number.parse::<usize>().is_err() {
                        break;
                    }
                    references.entry(path).or_default().push(number);
                    original.push_str(next);
                    lines.next();
                }
                if references.is_empty() {
                    output.push_str(line);
                } else {
                    let count: usize = references.values().map(Vec::len).sum();
                    let mut compact = format!("{line}{indent}  _All {count} returned locations retained; source excerpts omitted. Read listed lines to resolve target identity._\n");
                    for (path, numbers) in references {
                        compact.push_str(&format!("{indent}  - {path}:{}\n", numbers.join(",")));
                    }
                    output.push_str(if compact.len() < original.len() {
                        &compact
                    } else {
                        &original
                    });
                }
            } else if trimmed.starts_with("- _")
                && trimmed.contains("callee(s) unresolved in indexed source")
            {
                output.push_str(line);
                let mut names = Vec::new();
                while let Some(next) = lines.peek() {
                    if next.len() - next.trim_start().len() <= indent.len() {
                        break;
                    }
                    let Some(name) = next
                        .trim()
                        .strip_prefix("- ")
                        .and_then(|entry| entry.strip_suffix(" (unresolved)"))
                    else {
                        break;
                    };
                    names.push(name);
                    lines.next();
                }
                if !names.is_empty() {
                    output.push_str(&format!(
                        "{indent}  - {} (all unresolved)\n",
                        names.join(", ")
                    ));
                }
            } else if trimmed.starts_with("[Partial source analysis:")
                || trimmed.starts_with("- _(no direct caller observed")
            {
                let next_id = self.notes.len() + 1;
                if let Some(id) = self.notes.get(trimmed) {
                    output.push_str(&format!("{indent}[N{id} applies here]\n"));
                } else {
                    self.notes.insert(trimmed.into(), next_id);
                    output.push_str(&format!("{indent}[N{next_id}] {trimmed}\n"));
                }
            } else {
                output.push_str(line);
            }
        }
        output
    }
}

pub(super) const CALLEE_HEADERS: [&str; 3] = [
    "calls (depth 1, source/import resolution checked)",
    "calls (depth 1, tree-sitter precise where marked; fallback approximate)",
    "calls (depth 1, approximate, name-match only)",
];

pub(super) struct DeferredNote {
    pub range: std::ops::Range<usize>,
    pub file: usize,
    pub read: String,
}

pub(super) struct CalleeHeader {
    pub range: std::ops::Range<usize>,
    pub file: usize,
    pub kind: usize,
}

pub(super) struct NoteReplacement {
    pub range: std::ops::Range<usize>,
    pub text: String,
}

/// Inspect only renderer-owned annotation lines, never source, relations or candidates.
pub(super) fn callee_headers(annotation: &str, start: usize, file: usize) -> Vec<CalleeHeader> {
    let mut headers = Vec::new();
    let mut offset = start;
    for line in annotation.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some(header) = trimmed
            .strip_prefix("- _")
            .and_then(|s| s.strip_suffix(":_"))
        {
            if let Some(kind) = CALLEE_HEADERS
                .iter()
                .position(|candidate| *candidate == header)
            {
                let begin = offset + line.len() - line.trim_start().len() + "- _".len();
                headers.push(CalleeHeader {
                    range: begin..begin + header.len(),
                    file,
                    kind,
                });
            }
        }
        offset += line.len();
    }
    headers
}

/// Choose only shorter groups on the finalized text. These edits cannot change selection.
pub(super) fn plan_note_folding(
    text: &str,
    deferred: &[DeferredNote],
    headers: &[CalleeHeader],
    file_allowances: &[usize],
    definition_offset: Option<usize>,
) -> Vec<NoteReplacement> {
    const TITLE: &str = "# codemap-search\n";
    const DEFINITION: &str = "\n[D1] Body deferred by delivery budget, not a relevance judgment; listed reads restore unshown source.\n";
    let mut edits = Vec::new();
    let mut file_deltas = vec![0isize; file_allowances.len()];
    let definition_offset = definition_offset.filter(|offset| {
        offset
            .checked_sub(TITLE.len())
            .and_then(|start| text.get(start..*offset))
            == Some(TITLE)
            && deferred.iter().all(|note| *offset <= note.range.start)
    });
    if let Some(offset) = definition_offset.filter(|_| deferred.len() >= 2) {
        let replacements = deferred
            .iter()
            .map(|note| NoteReplacement {
                range: note.range.clone(),
                text: format!("_Body deferred [D1]; read {}._\n", note.read),
            })
            .collect::<Vec<_>>();
        let before: usize = deferred.iter().map(|note| note.range.len()).sum();
        let after: usize = replacements.iter().map(|edit| edit.text.len()).sum();
        if after + DEFINITION.len() < before {
            for (note, edit) in deferred.iter().zip(&replacements) {
                file_deltas[note.file] += edit.text.len() as isize - edit.range.len() as isize;
            }
            edits.push(NoteReplacement {
                range: offset..offset,
                text: DEFINITION.into(),
            });
            edits.extend(replacements);
        }
    }
    let mut seen = Vec::new();
    let mut next_label = 1;
    for header in headers {
        if seen.contains(&header.kind) {
            continue;
        }
        seen.push(header.kind);
        let group = headers
            .iter()
            .filter(|entry| entry.kind == header.kind)
            .collect::<Vec<_>>();
        if group.len() < 2 {
            continue;
        }
        let mut deltas = file_deltas.clone();
        let replacements = group
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let text = if index == 0 {
                    format!(
                        "calls [C{next_label}] {}",
                        CALLEE_HEADERS[entry.kind].strip_prefix("calls ").unwrap()
                    )
                } else {
                    format!("calls [C{next_label}]")
                };
                deltas[entry.file] += text.len() as isize - entry.range.len() as isize;
                NoteReplacement {
                    range: entry.range.clone(),
                    text,
                }
            })
            .collect::<Vec<_>>();
        let before: usize = group.iter().map(|entry| entry.range.len()).sum();
        let after: usize = replacements.iter().map(|edit| edit.text.len()).sum();
        if after >= before
            || deltas
                .iter()
                .zip(file_allowances)
                .any(|(delta, allowance)| *delta > 0 && *delta as usize > *allowance)
        {
            continue;
        }
        file_deltas = deltas;
        next_label += 1;
        edits.extend(replacements);
    }
    edits.sort_by_key(|edit| edit.range.start);
    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repeated_callee_headers_preserve_resolution_kind() {
        let mut text = String::new();
        let mut headers = Vec::new();
        for (file, kind) in [0, 1, 2, 0, 1, 2].into_iter().enumerate() {
            let annotation = format!(
                "  - _{}:_\n    - target — source.rs:2 (precise)\n",
                CALLEE_HEADERS[kind]
            );
            headers.extend(callee_headers(&annotation, text.len(), file));
            text.push_str(&annotation);
        }
        let apply = |edits: Vec<NoteReplacement>| {
            let mut result = text.clone();
            for edit in edits.into_iter().rev() {
                result.replace_range(edit.range, &edit.text);
            }
            result
        };
        let folded = apply(plan_note_folding(&text, &[], &headers, &[64; 6], None));
        assert!(folded.len() < text.len());
        let mut expanded = folded.clone();
        for (kind, original) in CALLEE_HEADERS.iter().enumerate() {
            let label = kind + 1;
            let definition = format!(
                "calls [C{label}] {}",
                original.strip_prefix("calls ").unwrap()
            );
            assert_eq!(folded.matches(&definition).count(), 1);
            expanded = expanded
                .replace(&definition, original)
                .replace(&format!("calls [C{label}]"), original);
        }
        assert_eq!(expanded, text);
        assert_eq!(
            apply(plan_note_folding(&text, &[], &headers[..3], &[64; 6], None)),
            text
        );
        // Each definition would grow a different full file: reject the entire group.
        assert_eq!(
            apply(plan_note_folding(&text, &[], &headers, &[0; 6], None)),
            text
        );
        assert_eq!(
            apply(plan_note_folding(&text, &[], &[], &[64; 6], None)),
            text
        );
        // Labels reset for each response, including after a rejected group.
        let mixed = apply(plan_note_folding(
            &text,
            &[],
            &headers,
            &[0, 64, 64, 64, 64, 64],
            None,
        ));
        assert!(mixed.contains("calls [C1] (depth 1, tree-sitter precise"));
        assert_eq!(mixed.matches(CALLEE_HEADERS[0]).count(), 2);
        assert_eq!(
            folded,
            apply(plan_note_folding(&text, &[], &headers, &[64; 6], None))
        );
    }
}

/// Masking and duplicate folding run after rendering and may lengthen short spans; this
/// reserve keeps a fitted search clear of the final `output.client` check.
const POST_RENDER_RESERVE_BYTES: usize = 512;

/// Resolved `output.client` limits, including built-in defaults, bound this presentation guard.
/// The Codex estimate is not a tokenizer or a guarantee for an arbitrarily large multi-tool cell.
pub(super) fn delivery_byte_cap(server_cap: usize) -> usize {
    crate::config::get()
        .client_output
        .delivery_byte_cap()
        .map_or(server_cap, |client_cap| {
            let reserve_bytes = POST_RENDER_RESERVE_BYTES.min(client_cap / 8);
            server_cap.min(client_cap - reserve_bytes)
        })
}
