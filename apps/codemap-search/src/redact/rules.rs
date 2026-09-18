use super::detection::{Detection, DetectionKind};
use regex::Regex;
use std::sync::OnceLock;

struct Rule {
    id: &'static str,
    kind: DetectionKind,
    pattern: Regex,
}

fn catalog() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        use DetectionKind::{Credential, Token};
        [
            ("token.aws-access-key", Token, r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
            ("token.github", Token, r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b"),
            ("token.openai", Token, r"\bsk-[A-Za-z0-9_-]{16,}"),
            ("token.google", Token, r"\bAIza[A-Za-z0-9_-]{30,}"),
            ("token.slack", Token, r"\bxox[baprs]-[A-Za-z0-9-]{10,}"),
            ("token.jwt", Token, r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"),
            ("token.stripe", Token, r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,}"),
            ("token.gitlab", Token, r"\bglpat-[A-Za-z0-9_-]{20,}"),
            ("token.npm", Token, r"\bnpm_[A-Za-z0-9]{20,}"),
            ("token.sendgrid", Token, r"\bSG\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{20,}"),
            ("credential.authorization", Credential, r#"(?i)\b(?:authorization|proxy[-_]authorization)["']?[ \t]*[:=][ \t]*["']?(?:Bearer|Basic)[ \t]+(?P<secret>[A-Za-z0-9_~+/=.-]+)"#),
            ("credential.bearer", Credential, r"\bBearer[ \t]+(?P<secret>[A-Za-z0-9_~+/=.-]{8,})"),
            ("credential.url-password", Credential, r"(?i)\b[a-z][a-z0-9+.-]*://[^\s/:@]+:(?P<secret>[^\s/@]+)@"),
        ].into_iter().map(|(id, kind, pattern)| Rule {
            id, kind, pattern: Regex::new(pattern).expect("built-in redaction rule"),
        }).collect()
    })
}

pub(super) fn is_sensitive_key(key: &str) -> bool {
    let normalized = crate::config::redact::normalize_field(key);
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
        || crate::config::get()
            .redact
            .sensitive_fields
            .contains(&normalized)
}

fn matches(
    text: &str,
    id: &str,
    kind: DetectionKind,
    pattern: &Regex,
    detections: &mut Vec<Detection>,
) {
    detections.extend(pattern.captures_iter(text).filter_map(|found| {
        let value = found
            .name("secret")
            .filter(|value| value.start() < value.end())
            .or_else(|| found.get(0))?;
        (value.start() < value.end()).then(|| Detection {
            range: value.range(),
            rule_id: id.into(),
            kind,
        })
    }));
}

pub(super) fn detect(text: &str) -> Vec<Detection> {
    let mut detections = Vec::new();
    for rule in catalog() {
        matches(text, rule.id, rule.kind, &rule.pattern, &mut detections);
    }
    for rule in &crate::config::get().redact.rules {
        matches(
            text,
            &rule.id,
            DetectionKind::Custom,
            &rule.pattern,
            &mut detections,
        );
    }
    static PEM: OnceLock<Regex> = OnceLock::new();
    let pem = PEM.get_or_init(|| {
        Regex::new(r"-----BEGIN (?P<kind>(?:(?:RSA|EC|DSA|OPENSSH|ENCRYPTED) )?PRIVATE KEY)-----")
            .unwrap()
    });
    for found in pem.captures_iter(text) {
        let begin = found.get(0).unwrap();
        let end_marker = format!("-----END {}-----", &found["kind"]);
        let end = text[begin.end()..]
            .find(&end_marker)
            .map_or(text.len(), |offset| begin.end() + offset + end_marker.len());
        detections.push(Detection {
            range: begin.start()..end,
            rule_id: "private-key.pem".into(),
            kind: DetectionKind::PrivateKey,
        });
    }
    detections
}
