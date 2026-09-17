//! Mask presentation copies only. Matching, parsing and persisted indexes keep original data.
use regex::Regex;
use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;
use std::sync::OnceLock;

const MARKER: &str = "[REDACTED]";

thread_local! {
    static IS_MCP_RESPONSE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct RequestGuard(bool);

pub(crate) fn begin_request() -> RequestGuard {
    RequestGuard(IS_MCP_RESPONSE.replace(true))
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        IS_MCP_RESPONSE.set(self.0);
    }
}

pub(crate) fn is_enabled() -> bool {
    IS_MCP_RESPONSE.get() && crate::config::get().is_redact_enabled
}

fn assignments() -> &'static Regex {
    static RULE: OnceLock<Regex> = OnceLock::new();
    RULE.get_or_init(|| Regex::new(r#"\b(?P<key>[A-Za-z_][A-Za-z0-9_.-]*)["']?[ \t]*(?::[ \t]*(?:&(?:'static[ \t]+)?str|String|string|str))?[ \t]*(?:=|:)[ \t]*"#).expect("built-in credential assignment rule"))
}

fn tokens() -> &'static [Regex] {
    static RULES: OnceLock<Vec<Regex>> = OnceLock::new();
    RULES.get_or_init(|| {
        [
            r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
            r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b",
            r"\bsk-[A-Za-z0-9_-]{16,}",
            r"\bAIza[A-Za-z0-9_-]{30,}",
            r"\bxox[baprs]-[A-Za-z0-9-]{10,}",
            r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("built-in token rule"))
        .collect()
    })
}

fn credentials() -> &'static [Regex] {
    static RULES: OnceLock<Vec<Regex>> = OnceLock::new();
    RULES.get_or_init(|| {
        [
            r#"(?i)\b(?:authorization|proxy[-_]authorization)["']?[ \t]*[:=][ \t]*["']?(?:Bearer|Basic)[ \t]+(?P<secret>[A-Za-z0-9_~+/=.-]+)"#,
            r"\bBearer[ \t]+(?P<secret>[A-Za-z0-9_~+/=.-]{8,})",
            r"(?i)\b[a-z][a-z0-9+.-]*://[^\s/:@]+:(?P<secret>[^\s/@]+)@",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("built-in credential rule"))
        .collect()
    })
}

fn private_keys() -> &'static Regex {
    static RULE: OnceLock<Regex> = OnceLock::new();
    RULE.get_or_init(|| {
        Regex::new(r"-----BEGIN (?P<kind>(?:(?:RSA|EC|DSA|OPENSSH|ENCRYPTED) )?PRIVATE KEY)-----")
            .expect("built-in private key rule")
    })
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    [
        "apikey",
        "token",
        "password",
        "passwd",
        "pwd",
        "secret",
        "secretkey",
        "privatekey",
        "accesskey",
        "accesskeyid",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
}

pub(crate) fn named_value<'a>(name: &str, value: &'a str) -> Cow<'a, str> {
    if is_enabled() && is_sensitive_key(name) {
        Cow::Owned(hidden(value))
    } else {
        source(value)
    }
}

/// Replacement never grows text, so existing byte ceilings remain valid after masking.
pub(crate) fn hidden(value: &str) -> String {
    value
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches(['\r', '\n']);
            let replacement = if body.len() >= MARKER.len() {
                MARKER.to_string()
            } else {
                "*".repeat(body.len())
            };
            replacement + &line[body.len()..]
        })
        .collect()
}

fn mask_ranges(text: &str, mut ranges: Vec<Range<usize>>) -> String {
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges.into_iter().filter(|range| range.start < range.end) {
        if let Some(last) = merged.last_mut().filter(|last| range.start <= last.end) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    let mut output = String::with_capacity(text.len());
    let mut start = 0;
    for range in merged {
        output.push_str(&text[start..range.start]);
        output.push_str(&hidden(&text[range.clone()]));
        start = range.end;
    }
    output.push_str(&text[start..]);
    output
}

enum Continuation {
    Value {
        should_mask_bare: bool,
    },
    Quote {
        delimiter: String,
        has_escapes: bool,
    },
    PrivateKey(String),
    Indented(usize),
}

/// Carries credential context across source lines, including lines outside a returned window.
#[derive(Default)]
pub(crate) struct LineRedactor {
    continuation: Option<Continuation>,
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

impl LineRedactor {
    fn mask_value(
        &mut self,
        line: &str,
        start: usize,
        should_mask_bare: bool,
        ranges: &mut Vec<Range<usize>>,
    ) -> usize {
        let value = &line[start..];
        if value.trim().is_empty() {
            self.continuation = Some(Continuation::Value { should_mask_bare });
            return line.len();
        }
        if let Some((opening, delimiter, has_escapes)) = quote(value) {
            let body_start = start + opening;
            if let Some(end) = closing_quote(&line[body_start..], &delimiter, has_escapes) {
                ranges.push(body_start..body_start + end);
                return body_start + end + delimiter.len();
            }
            ranges.push(body_start..line.len());
            self.continuation = Some(Continuation::Quote {
                delimiter,
                has_escapes,
            });
            return line.len();
        }
        if value
            .split('#')
            .next()
            .unwrap_or(value)
            .trim()
            .chars()
            .all(|c| matches!(c, '|' | '>' | '+' | '-' | '1'..='9'))
            && value.starts_with(['|', '>'])
        {
            let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            self.continuation = Some(Continuation::Indented(indent));
            return line.len();
        }
        let end = value
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '}' | ')' | '\"' | '\''))
            .unwrap_or(value.len());
        let candidate = &value[..end];
        // Keep code references and empty/boolean placeholders useful. Bare config values are secrets.
        if should_mask_bare
            && !candidate.is_empty()
            && !["null", "None", "nil", "true", "false"].contains(&candidate)
            && !candidate.starts_with(['$', '[', '{'])
            && !candidate.contains('(')
        {
            ranges.push(start..start + end);
        }
        start + end
    }

    pub(crate) fn line(&mut self, line: &str) -> String {
        let mut ranges = Vec::new();
        let mut cursor = 0;
        if let Some(state) = self.continuation.take() {
            match state {
                Continuation::Value { should_mask_bare } => {
                    let start = line.len() - line.trim_start_matches([' ', '\t']).len();
                    cursor = self.mask_value(line, start, should_mask_bare, &mut ranges);
                }
                Continuation::Quote {
                    delimiter,
                    has_escapes,
                } => {
                    if let Some(end) = closing_quote(line, &delimiter, has_escapes) {
                        ranges.push(0..end);
                        cursor = end + delimiter.len();
                    } else {
                        self.continuation = Some(Continuation::Quote {
                            delimiter,
                            has_escapes,
                        });
                        return hidden(line);
                    }
                }
                Continuation::PrivateKey(end_marker) => {
                    if let Some(end) = line.find(&end_marker) {
                        cursor = end + end_marker.len();
                        ranges.push(0..cursor);
                    } else {
                        self.continuation = Some(Continuation::PrivateKey(end_marker));
                        return hidden(line);
                    }
                }
                Continuation::Indented(indent) => {
                    let spaces = line.len() - line.trim_start_matches([' ', '\t']).len();
                    if line.trim().is_empty() || spaces > indent {
                        self.continuation = Some(Continuation::Indented(indent));
                        return format!("{}{}", &line[..spaces], hidden(&line[spaces..]));
                    }
                }
            }
        }
        for found in private_keys().captures_iter(&line[cursor..]) {
            let matched = found.get(0).unwrap();
            let start = cursor + matched.start();
            let content_start = cursor + matched.end();
            let end_marker = format!("-----END {}-----", &found["kind"]);
            if let Some(end) = line[content_start..].find(&end_marker) {
                ranges.push(start..content_start + end + end_marker.len());
            } else {
                ranges.push(start..line.len());
                self.continuation = Some(Continuation::PrivateKey(end_marker));
            }
        }
        for found in assignments().captures_iter(line) {
            let matched = found.get(0).unwrap();
            if matched.start() < cursor || !is_sensitive_key(&found["key"]) {
                continue;
            }
            let prefix = line[..matched.start()].trim();
            let should_mask_bare =
                prefix.is_empty() || prefix == "export" || prefix.ends_with(['\"', '\'', '{', ',']);
            cursor = self.mask_value(line, matched.end(), should_mask_bare, &mut ranges);
        }
        for rule in tokens() {
            ranges.extend(rule.find_iter(line).map(|found| found.range()));
        }
        for rule in credentials() {
            ranges.extend(
                rule.captures_iter(line)
                    .filter_map(|found| found.name("secret").map(|value| value.range())),
            );
        }
        mask_ranges(line, ranges)
    }
}

pub(crate) fn source(text: &str) -> Cow<'_, str> {
    if !is_enabled() {
        return Cow::Borrowed(text);
    }
    let mut redactor = LineRedactor::default();
    let masked: String = text
        .split_inclusive('\n')
        .map(|line| redactor.line(line))
        .collect();
    if masked == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(masked)
    }
}

fn response_text(text: &str) -> String {
    if !is_enabled() {
        return text.to_string();
    }
    // Source producers already mask with full-file context before slicing. Do not carry
    // quote/block state across formatted result groups, skipped lines or unrelated files.
    text.split_inclusive('\n')
        .map(|line| LineRedactor::default().line(line))
        .collect()
}

/// Mask final response strings without touching JSON-RPC ids, object keys or schema structure.
pub(crate) fn response(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = response_text(text),
        serde_json::Value::Array(items) => items.iter_mut().for_each(response),
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                if let serde_json::Value::String(text) = value {
                    *text = if is_sensitive_key(name) {
                        named_value(name, text).into_owned()
                    } else {
                        response_text(text)
                    };
                } else {
                    response(value);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_are_hidden_without_changing_safe_code_or_line_breaks() {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        let _request = begin_request();
        let cases = [
            (
                "{\"password\":\n  \"next-line-secret\"}",
                "next-line-secret",
            ),
            (
                "password = r\"\"\"\nraw-multiline-secret\n\"\"\"",
                "raw-multiline-secret",
            ),
            ("password = @\"first\"\"last-secret\";", "last-secret"),
            (
                "API_KEY=plain-secret-value\nSAFE=value\n",
                "plain-secret-value",
            ),
            (
                "const PASSWORD: &str = \"한글-비밀번호\";\r\n",
                "한글-비밀번호",
            ),
            (
                "{\"access_token\":\"arbitrary-credential\",\"port\":8080}",
                "arbitrary-credential",
            ),
            (
                "Authorization: Bearer bearer-value-12345",
                "bearer-value-12345",
            ),
            (
                "postgres://reader:database-password@localhost/app",
                "database-password",
            ),
            (
                "value = 'ghp_abcdefghijklmnopqrstuvwxyz0123456789'",
                "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            ),
            (
                "password = \"\"\"\nmultiline-secret-value\n\"\"\"\npublic = 'visible'\n",
                "multiline-secret-value",
            ),
            (
                "private_key: |\n  yaml-secret-value\npublic: visible\n",
                "yaml-secret-value",
            ),
            (
                "-----BEGIN PRIVATE KEY-----\npem-secret-value\n-----END PRIVATE KEY-----\n",
                "pem-secret-value",
            ),
            (
                "const PASSWORD: &str = r#\"raw-secret-value\"#;",
                "raw-secret-value",
            ),
            ("password='x'", "'x'"),
        ];
        for (input, secret) in cases {
            let masked = source(input);
            assert!(!masked.contains(secret), "credential remained visible");
            assert!(masked.len() <= input.len());
            assert_eq!(masked.matches('\n').count(), input.matches('\n').count());
            assert_eq!(masked.matches('\r').count(), input.matches('\r').count());
        }
        let safe = "const token = process.env.TOKEN;\nlet password = load_password();\nconst token_count = 12;\nconst message = \"hello\";\n// Basic conditional paths remain visible.\n";
        assert_eq!(source(safe), safe);
    }

    #[test]
    fn test_response_keeps_source_locations_and_protocol_structure() {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        let _request = begin_request();
        let original = "password = \"\"\"\ninterior-secret\n\"\"\"\npublic = 42\n";
        let rendered = source(original)
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{:>6}→{line}\n", i + 1))
            .collect::<String>();
        let mut value = serde_json::json!({
            "content": [{"type":"text", "text":rendered}],
            "other": {"code": -32602, "message": "API_KEY=error-secret-value"}
        });
        response(&mut value);
        let text = value["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("2→[REDACTED]") && text.contains("4→public = 42"),
            "{text}"
        );
        assert_eq!(value["content"][0]["type"], "text");
        assert_eq!(value["other"]["code"], -32602);
        assert!(!value.to_string().contains("interior-secret"));
        assert!(!value.to_string().contains("error-secret-value"));
        let first = value.clone();
        response(&mut value);
        assert_eq!(value, first);
    }

    #[test]
    fn test_configuration_layers_and_cli_do_not_silently_enable_or_disable_redaction() {
        let repo = tempfile::tempdir().unwrap();
        let global = tempfile::tempdir().unwrap();
        std::fs::create_dir(repo.path().join(".codemap")).unwrap();
        std::fs::write(
            global.path().join("config.toml"),
            "[tool_output]\nis_redact_enabled=false\n",
        )
        .unwrap();
        assert!(!crate::config::load(repo.path(), global.path()).is_redact_enabled);
        let repo_config = repo.path().join(".codemap/config.toml");
        std::fs::write(&repo_config, "[tool_output]\nis_redact_enabled=true\n").unwrap();
        assert!(crate::config::load(repo.path(), global.path()).is_redact_enabled);
        std::fs::write(&repo_config, "[tool_output]\nis_redact_enabled='invalid'\n").unwrap();
        assert!(!crate::config::load(repo.path(), global.path()).is_redact_enabled);

        let input = "password='unmasked-outside-mcp'";
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        assert_eq!(source(input), input);
        {
            let _request = begin_request();
            assert_ne!(source(input), input);
            let disabled = crate::config::ResolvedConfig {
                is_redact_enabled: false,
                ..Default::default()
            };
            let _disabled = crate::config::pin_test_config(disabled);
            assert_eq!(source(input), input);
        }
        assert_eq!(source(input), input);
    }
}
