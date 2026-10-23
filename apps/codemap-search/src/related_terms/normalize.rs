use crate::parser::split_identifier;

pub fn canonical_surface(text: &str) -> String {
    text.split_whitespace()
        .map(canonical_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn canonical_token(token: &str) -> String {
    if token.eq_ignore_ascii_case(".net") {
        return token.into();
    }
    let mut bare = token;
    if let Some(prefix) = bare.strip_suffix("()") {
        if prefix.ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '!') {
            bare = prefix;
        }
    }
    if let Some(prefix) = bare.strip_suffix('!') {
        if prefix.ends_with(|c: char| c.is_alphanumeric() || c == '_') {
            bare = prefix;
        }
    }
    if let Some(inner) = bare.strip_prefix("__").and_then(|s| s.strip_suffix("__")) {
        if inner.starts_with(|c: char| c.is_ascii_alphabetic())
            && inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            bare = inner;
        }
    }
    if let Some(inner) = bare.strip_prefix(['$', '@']) {
        if inner.starts_with(|c: char| c.is_ascii_alphabetic()) {
            bare = inner;
        }
    }
    if let Some(inner) = bare.strip_prefix('.') {
        if inner.chars().count() >= 2
            && inner.starts_with(|c: char| c.is_ascii_alphanumeric())
            && inner
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-'))
        {
            bare = inner;
        }
    }
    if bare.is_empty() {
        token.into()
    } else {
        bare.into()
    }
}

pub fn exact_key(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn word_key(text: &str) -> String {
    let text = exact_key(&canonical_surface(text));
    let chars: Vec<char> = text.chars().collect();
    let mut key = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() || matches!(chars[i], '_' | '-') {
            let start = i;
            while i < chars.len() && (chars[i].is_whitespace() || matches!(chars[i], '_' | '-')) {
                i += 1;
            }
            if start == 0
                || i == chars.len()
                || !chars[start - 1].is_ascii_alphanumeric()
                || !chars[i].is_ascii_alphanumeric()
            {
                key.extend(&chars[start..i]);
            }
        } else {
            key.push(chars[i]);
            i += 1;
        }
    }
    key
}

pub(super) fn input_parts(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|run| !run.is_empty() && run.is_ascii())
        .flat_map(split_identifier)
        .collect()
}

pub(super) fn inflection_bases(word: &str) -> Vec<String> {
    if !word.bytes().all(|b| b.is_ascii_lowercase()) {
        return Vec::new();
    }
    let mut bases = Vec::new();
    for suffix in ["s", "es", "ies", "ing", "ed"] {
        let Some(stem) = word.strip_suffix(suffix) else {
            continue;
        };
        match suffix {
            "s" if word.ends_with("ss") => continue,
            "ies" => bases.push(format!("{stem}y")),
            "ing" | "ed" => {
                bases.push(format!("{stem}e"));
                bases.push(stem.into());
                let bytes = stem.as_bytes();
                if bytes.len() >= 2
                    && bytes[bytes.len() - 1] == bytes[bytes.len() - 2]
                    && !b"aeiou".contains(&bytes[bytes.len() - 1])
                {
                    bases.push(stem[..stem.len() - 1].into());
                }
            }
            _ => bases.push(stem.into()),
        }
    }
    bases.retain(|b| b.len() >= 3 && b != word);
    bases.sort();
    bases.dedup();
    bases
}

pub(super) fn spelling_alternative(key: &str) -> Option<&'static str> {
    match key {
        "backhannel" => Some("backchannel"),
        "polices" => Some("policies"),
        "acquring" => Some("acquiring"),
        "cirds" => Some("cidrs"),
        "mata" => Some("meta"),
        "asterick" => Some("asterisk"),
        "certficate" => Some("certificate"),
        "comitted" => Some("committed"),
        "conut" => Some("count"),
        "replicy" => Some("policy"),
        "uncontented" => Some("uncontended"),
        _ => None,
    }
}
