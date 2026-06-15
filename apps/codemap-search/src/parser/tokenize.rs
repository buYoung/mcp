use std::collections::HashSet;

pub fn split_identifier(ident: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = ident.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];

        if c == '_' || c == '-' {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            continue;
        }

        let prev_is_lowercase = i > 0 && chars[i - 1].is_lowercase();
        let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let prev_is_uppercase = i > 0 && chars[i - 1].is_uppercase();

        let current_is_uppercase = c.is_uppercase();

        let next_is_lowercase = i + 1 < len && chars[i + 1].is_lowercase();

        let is_camel_boundary = (prev_is_lowercase || prev_is_digit) && current_is_uppercase;
        let is_acronym_boundary = prev_is_uppercase && current_is_uppercase && next_is_lowercase;
        let is_digit_boundary = prev_is_digit && c.is_alphabetic();

        let prev_is_uncased = i > 0
            && chars[i - 1].is_alphabetic()
            && !chars[i - 1].is_lowercase()
            && !chars[i - 1].is_uppercase();
        let current_is_uncased = c.is_alphabetic() && !c.is_lowercase() && !c.is_uppercase();
        let is_uncased_boundary = i > 0
            && ((prev_is_uncased
                && !current_is_uncased
                && (c.is_alphabetic() || c.is_ascii_digit()))
                || (!prev_is_uncased
                    && (chars[i - 1].is_alphabetic() || chars[i - 1].is_ascii_digit())
                    && current_is_uncased));

        if (is_camel_boundary || is_acronym_boundary || is_digit_boundary || is_uncased_boundary)
            && !current.is_empty()
        {
            tokens.push(current.to_lowercase());
            current.clear();
        }

        current.push(c);
    }

    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

#[derive(Debug, Clone)]
pub struct QueryTokens {
    raw_words: Vec<String>,
    words: Vec<String>,
    tokens: Vec<String>,
    raw_word_set: HashSet<String>,
    word_set: HashSet<String>,
    token_set: HashSet<String>,
    has_qualified_word: bool,
    search_text: String,
}

impl QueryTokens {
    pub fn parse(query: &str) -> Self {
        let mut raw_words = Vec::new();
        let mut words = Vec::new();
        let mut raw_word_set = HashSet::new();
        let mut word_set = HashSet::new();
        let mut tokens = Vec::new();
        let mut token_set = HashSet::new();
        let mut has_qualified_word = false;

        for raw_word in query.split_whitespace().filter_map(|word| {
            let lower = word.to_lowercase();
            (!lower.is_empty()).then_some(lower)
        }) {
            has_qualified_word |= raw_word
                .chars()
                .any(|c| !(c.is_alphanumeric() || c == '_' || c == '-'));
            push_unique(&mut raw_words, &mut raw_word_set, raw_word);
        }

        for word in query
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .filter_map(|word| {
                let lower = word.to_lowercase();
                (!lower.is_empty()).then_some(lower)
            })
        {
            push_unique(&mut words, &mut word_set, word.clone());
            push_unique(&mut tokens, &mut token_set, word.clone());
            for token in split_identifier(&word) {
                push_unique(&mut tokens, &mut token_set, token);
            }
        }

        let search_text = if tokens.is_empty() {
            query.to_string()
        } else {
            tokens.join(" ")
        };

        Self {
            raw_words,
            words,
            tokens,
            raw_word_set,
            word_set,
            token_set,
            has_qualified_word,
            search_text,
        }
    }

    pub fn raw_words(&self) -> &[String] {
        &self.raw_words
    }

    pub fn words(&self) -> &[String] {
        &self.words
    }

    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    pub fn search_text(&self) -> &str {
        &self.search_text
    }

    pub fn contains_word(&self, word: &str) -> bool {
        self.word_set.contains(word)
    }

    pub fn contains_raw_word(&self, word: &str) -> bool {
        self.raw_word_set.contains(word)
    }

    pub fn contains_token(&self, token: &str) -> bool {
        self.token_set.contains(token)
    }

    pub fn has_qualified_word(&self) -> bool {
        self.has_qualified_word
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

fn push_unique(values: &mut Vec<String>, seen: &mut HashSet<String>, value: String) {
    if seen.insert(value.clone()) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Sub-tokenization Tests ---
    #[test]
    fn test_split_identifier_cases() {
        assert_eq!(
            split_identifier("handleLoginError"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(
            split_identifier("handle_login_error"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(
            split_identifier("handle-login-error"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(split_identifier("HTTPClient"), vec!["http", "client"]);
        assert_eq!(split_identifier("v2Engine"), vec!["v2", "engine"]);
        assert_eq!(
            split_identifier("API2026version"),
            vec!["api2026", "version"]
        );
        assert_eq!(
            split_identifier("XMLHttpRequest"),
            vec!["xml", "http", "request"]
        );
        assert_eq!(split_identifier("HTMLElement"), vec!["html", "element"]);
        assert_eq!(split_identifier(""), Vec::<String>::new());
        assert_eq!(split_identifier("a"), vec!["a"]);
    }

    #[test]
    fn test_split_identifier_unicode_boundaries() {
        assert_eq!(split_identifier("한글MyName"), vec!["한글", "my", "name"]);
        assert_eq!(split_identifier("My한글Name"), vec!["my", "한글", "name"]);
        assert_eq!(split_identifier("한글_my_name"), vec!["한글", "my", "name"]);
    }
}
