//! Conservative fallback for unsupported syntax, malformed regions and plain text.
use super::{detection::Detection, rules};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub(super) struct Candidate {
    pub key_start: usize,
    pub detection: Detection,
}

fn closing_quote(value: &str, delimiter: &str, has_escapes: bool) -> Option<usize> {
    let mut skip_until = 0;
    value.match_indices(delimiter).find_map(|(offset, _)| {
        if offset < skip_until {
            return None;
        }
        let escapes = value[..offset]
            .bytes()
            .rev()
            .take_while(|byte| *byte == b'\\')
            .count();
        if has_escapes && escapes % 2 != 0 {
            return None;
        }
        if delimiter.len() == 1 && value[offset + 1..].starts_with(delimiter) {
            // YAML/SQL single quotes and C# verbatim strings escape by doubling quotes.
            skip_until = offset + 2;
            return None;
        }
        Some(offset)
    })
}

fn quote(value: &str) -> Option<(usize, String, bool)> {
    for delimiter in ["\"\"\"", "'''", "\"", "'", "`"] {
        if value.starts_with(delimiter) {
            return Some((delimiter.len(), delimiter.into(), true));
        }
    }
    // Prefixes precede the complete delimiter, including Python triple quotes.
    if value.starts_with(['r', 'R', 'b', 'B', 'f', 'F', 'u', 'U', '@']) {
        for delimiter in ["\"\"\"", "'''", "\"", "'"] {
            if value[1..].starts_with(delimiter) {
                return Some((
                    1 + delimiter.len(),
                    delimiter.into(),
                    !value.starts_with(['r', 'R', '@']),
                ));
            }
        }
    }
    // Rust raw strings with hash-delimited closing quotes.
    if let Some(raw) = value.strip_prefix('r') {
        let hashes = raw.bytes().take_while(|byte| *byte == b'#').count();
        if raw[hashes..].starts_with('"') {
            return Some((hashes + 2, format!("\"{}", "#".repeat(hashes)), false));
        }
    }
    None
}

fn assignments() -> &'static Regex {
    static RULE: OnceLock<Regex> = OnceLock::new();
    RULE.get_or_init(|| Regex::new(r#"\b(?P<key>[A-Za-z_][A-Za-z0-9_.-]*)["']?[ \t]*(?::[ \t]*(?:&(?:'static[ \t]+)?str|String|string|str))?[ \t]*(?:=|:)[ \t]*"#).unwrap())
}

pub(super) fn detect(source: &str, path: Option<&Path>) -> Vec<Candidate> {
    let is_line_config = path.is_some_and(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ".env" || name.starts_with(".env."))
            || matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("env" | "ini" | "properties" | "cfg")
            )
    });
    let mut candidates = Vec::new();
    let mut consumed = 0;
    for found in assignments().captures_iter(source) {
        let matched = found.get(0).unwrap();
        if matched.start() < consumed || !rules::is_sensitive_key(&found["key"]) {
            continue;
        }
        let mut start = matched.end();
        if !is_line_config {
            start += source[start..].len() - source[start..].trim_start().len();
        }
        let value = &source[start..];
        let line_start = source[..matched.start()].rfind('\n').map_or(0, |at| at + 1);
        let prefix = source[line_start..matched.start()].trim();
        let line_end = source[start..]
            .find('\n')
            .map_or(source.len(), |at| start + at);
        let range = if let Some((opening, delimiter, has_escapes)) = quote(value) {
            let body_start = start + opening;
            let end = closing_quote(&source[body_start..], &delimiter, has_escapes)
                .map_or(source.len(), |at| body_start + at);
            consumed = (end + delimiter.len()).min(source.len());
            Some(body_start..end)
        } else if value.starts_with(['|', '>']) {
            let header = source[start..line_end]
                .split('#')
                .next()
                .unwrap_or("")
                .trim();
            if header
                .chars()
                .all(|c| matches!(c, '|' | '>' | '+' | '-' | '1'..='9'))
            {
                let indent = source[line_start..matched.start()]
                    .bytes()
                    .take_while(|c| matches!(c, b' ' | b'\t'))
                    .count();
                let body_start = (line_end + 1).min(source.len());
                let mut end = body_start;
                for line in source[body_start..].split_inclusive('\n') {
                    let spaces = line.len() - line.trim_start_matches([' ', '\t']).len();
                    if !line.trim().is_empty() && spaces <= indent {
                        break;
                    }
                    end += line.len();
                }
                consumed = end;
                Some(body_start..end)
            } else {
                None
            }
        } else if is_line_config {
            // ENV/INI punctuation and spaces can belong to a credential. Do not stop
            // at a code-expression separator such as a semicolon.
            let value = &source[start..line_end];
            let end = value.find('#').unwrap_or(value.len());
            let value = value[..end].trim_end();
            (!value.is_empty() && !value.starts_with('$')).then_some(start..start + value.len())
        } else {
            let end = value
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, ',' | ';' | '}' | ')' | '"' | '\'')
                })
                .unwrap_or(value.len());
            let bare = &value[..end];
            let should_mask_bare =
                prefix.is_empty() || prefix == "export" || prefix.ends_with(['"', '\'', '{', ',']);
            (should_mask_bare
                && !bare.is_empty()
                && !["null", "None", "nil", "true", "false"].contains(&bare)
                && !bare.starts_with(['$', '[', '{'])
                && !bare.contains('('))
            .then_some(start..start + end)
        };
        if let Some(range) = range.filter(|range| range.start < range.end) {
            candidates.push(Candidate {
                key_start: matched.start(),
                detection: Detection::field(range),
            });
        }
    }
    candidates
}
