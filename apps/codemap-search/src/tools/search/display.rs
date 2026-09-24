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

/// Only an explicitly configured Codex limit enables this presentation guard.
/// A 3.5-byte presentation allowance leaves room below Codex's approximate delivery limit;
/// it is not a tokenizer or a guarantee for an arbitrarily large multi-tool cell.
pub(super) fn delivery_byte_cap(server_cap: usize) -> usize {
    crate::config::get()
        .client_output
        .codex_output_token_limit
        .map_or(server_cap, |tokens| {
            server_cap.min(tokens.saturating_mul(7) / 2)
        })
}
