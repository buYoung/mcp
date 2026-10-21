//! Keys `rtk-1` of the `rt-related/1` dataset and the query decomposition of `related`.
//!
//! Dataset side: keys derive from a term's exact key (lower case), so they never depend on
//! which surface spelling a source used. `build_related_dataset.py` derives the same keys
//! and `related-check` proves both sides agree for every dataset term.
//!
//! Query side: the query keeps its casing, so camelCase/PascalCase boundaries split words
//! (`eventLoop` -> `event loop`); every ASCII character that is not a letter or digit
//! separates words, so attached symbols never block a match (`$eventLoop`).

pub const RELATED_KEYS_VERSION: &str = "rtk-1";
/// Phrases longer than this many words are only decomposed into spans of up to
/// `MAX_SPAN_WORDS` words, which keeps sub-phrase lookups bounded for long inputs.
pub const MAX_SPAN_WORDS: usize = 6;

/// A term whose symbols carry meaning: a leading dot (`.net`, `.csv`, `.env`) or `+`/`#`
/// (`c++`, `c#`, `f#`, `gtk+`). Protected terms match only by exact key, because their
/// symbol-free spelling is a different word (`net`, `c`).
pub fn is_protected(exact_key: &str) -> bool {
    let has_alphanumeric = exact_key.chars().any(|c| c.is_ascii_alphanumeric());
    has_alphanumeric && (exact_key.starts_with('.') || exact_key.contains('+') || exact_key.contains('#'))
}

/// `(match_key, joined_key)` of a dataset term: the exact key's alphanumeric runs joined by
/// one space, and the same runs concatenated. Protected and non-ASCII terms get empty keys
/// and are reachable by exact key only.
pub fn term_keys(exact_key: &str) -> (String, String) {
    if !exact_key.is_ascii() || is_protected(exact_key) {
        return (String::new(), String::new());
    }
    let words: Vec<&str> = exact_key
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    (words.join(" "), words.concat())
}

/// Splits one ASCII alphanumeric run at identifier boundaries and lower-cases the words.
/// A boundary is lower->Upper (`eventLoop`), or Upper/digit->Upper followed by a lower-case
/// letter (`HTTPServer`, `v2Engine`). Letters and digits otherwise stay together, so
/// numeronyms and versioned names remain one word (`k8s`, `i18n`, `utf8`, `http2`).
pub fn split_words(run: &str) -> Vec<String> {
    let chars: Vec<char> = run.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if !current.is_empty() && c.is_ascii_uppercase() {
            let previous = chars[index - 1];
            let is_next_lowercase = chars.get(index + 1).is_some_and(|next| next.is_ascii_lowercase());
            let is_boundary = previous.is_ascii_lowercase()
                || ((previous.is_ascii_uppercase() || previous.is_ascii_digit()) && is_next_lowercase);
            if is_boundary {
                words.push(current.to_ascii_lowercase());
                current.clear();
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current.to_ascii_lowercase());
    }
    words
}

/// Words of a query, in order, with the runs that could not become words.
#[derive(Debug, Default, PartialEq)]
pub struct QueryWords {
    pub words: Vec<String>,
    /// Runs containing a non-ASCII letter or digit. They are skipped whole and never
    /// reduced to their ASCII characters (`café` is not `caf`).
    pub skipped_non_ascii: Vec<String>,
}

/// Splits text into runs at every character that is not a letter or digit, then splits each
/// ASCII run with `split_words`.
pub fn query_words(text: &str) -> QueryWords {
    let mut result = QueryWords::default();
    for run in text.split(|c: char| !c.is_alphanumeric()).filter(|run| !run.is_empty()) {
        if run.is_ascii() {
            result.words.extend(split_words(run));
        } else {
            result.skipped_non_ascii.push(run.to_string());
        }
    }
    result
}

/// Removes leading and trailing ASCII characters that are neither letters, digits, nor a
/// protected term's symbols: a leading dot and trailing `+`/`#` are kept so `.net`, `c++`
/// and `c#` survive (`` `retry` `` -> `retry`, `$eventLoop` -> `eventLoop`).
pub fn strip_outer(text: &str) -> &str {
    let text = text.trim();
    let start = text
        .char_indices()
        .find(|&(_, c)| c.is_alphanumeric() || !c.is_ascii() || c == '.')
        .map_or(text.len(), |(index, _)| index);
    // A run of leading dots keeps only its last dot (`...net` is not a protected spelling).
    let start = match text[start..].find(|c: char| c != '.') {
        Some(offset) if offset > 1 => start + offset - 1,
        _ => start,
    };
    let rest = &text[start..];
    let end = rest
        .char_indices()
        .rev()
        .find(|&(_, c)| c.is_alphanumeric() || !c.is_ascii() || c == '+' || c == '#')
        .map_or(0, |(index, c)| index + c.len_utf8());
    &rest[..end]
}

/// Spellings inside a query that could be protected terms: every whitespace-separated
/// token's outer-stripped form that starts with a dot (`.csv`), and every alphanumeric run
/// directly followed by `+`/`#` characters (`c++` in `c++17`, `c#` in `C#/.NET`).
pub fn protected_candidates(text: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        let stripped = strip_outer(token);
        let dotted = stripped.split(['/', ',', ';']).filter(|part| part.starts_with('.'));
        for part in dotted {
            let part = strip_outer(part);
            if part.starts_with('.') && is_protected(&part.to_lowercase()) {
                candidates.push(part.to_lowercase());
            }
        }
    }
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_alphanumeric() {
            index += 1;
            continue;
        }
        let run_start = index;
        while index < chars.len() && chars[index].is_ascii_alphanumeric() {
            index += 1;
        }
        let symbols_start = index;
        while index < chars.len() && (chars[index] == '+' || chars[index] == '#') {
            index += 1;
        }
        if index > symbols_start {
            let candidate: String = chars[run_start..index].iter().collect::<String>().to_lowercase();
            candidates.push(candidate);
        }
    }
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.clone()));
    candidates
}

/// Every contiguous span `(start, end)` of `word_count` words, longest first, then left to
/// right. Spans longer than `MAX_SPAN_WORDS` are left out except the whole phrase.
pub fn spans(word_count: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    for length in (1..=word_count).rev() {
        if length < word_count && length > MAX_SPAN_WORDS {
            continue;
        }
        for start in 0..=(word_count - length) {
            result.push((start, start + length));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_terms_are_symbolic_names() {
        for term in [".net", ".csv", ".env", "c++", "c#", "f#", "gtk+", "notepad++", ".net core"] {
            assert!(is_protected(term), "{term}");
        }
        for term in ["net", "c", "node.js", "pub/sub", "event loop", "i/o", ".", "+", "#", "..."] {
            assert!(!is_protected(term), "{term}");
        }
    }

    #[test]
    fn term_keys_split_on_separators_only() {
        assert_eq!(term_keys("event loop"), ("event loop".into(), "eventloop".into()));
        assert_eq!(term_keys("event-loop"), ("event loop".into(), "eventloop".into()));
        assert_eq!(term_keys("node.js"), ("node js".into(), "nodejs".into()));
        assert_eq!(term_keys("addeventlistener"), ("addeventlistener".into(), "addeventlistener".into()));
        assert_eq!(term_keys("k8s"), ("k8s".into(), "k8s".into()));
        assert_eq!(term_keys("http/2"), ("http 2".into(), "http2".into()));
        assert_eq!(term_keys(".net"), (String::new(), String::new()));
        assert_eq!(term_keys("c++"), (String::new(), String::new()));
        assert_eq!(term_keys("café"), (String::new(), String::new()));
        assert_eq!(term_keys("!!!"), (String::new(), String::new()));
    }

    #[test]
    fn split_words_follows_identifier_boundaries() {
        let cases: &[(&str, &[&str])] = &[
            ("eventLoop", &["event", "loop"]),
            ("EventLoop", &["event", "loop"]),
            ("addEventListener", &["add", "event", "listener"]),
            ("HTTPServer", &["http", "server"]),
            ("XMLHttpRequest", &["xml", "http", "request"]),
            ("v2Engine", &["v2", "engine"]),
            ("oauth2Client", &["oauth2", "client"]),
            ("k8s", &["k8s"]),
            ("K8S", &["k8s"]),
            ("i18n", &["i18n"]),
            ("E2E", &["e2e"]),
            ("utf8", &["utf8"]),
            ("http2", &["http2"]),
            ("sha256", &["sha256"]),
            ("EVENT", &["event"]),
            ("a", &["a"]),
        ];
        for (input, expected) in cases {
            assert_eq!(split_words(input), expected.to_vec(), "{input}");
        }
    }

    #[test]
    fn query_words_treat_ascii_symbols_as_separators() {
        let words = |text: &str| query_words(text).words;
        let event_loop = vec!["event".to_string(), "loop".to_string()];
        for input in [
            "eventLoop", "$eventLoop", "event_loop", "EVENT_LOOP", "event-loop", "event.loop", "event loop",
            "@eventLoop", "#eventLoop", "eventLoop()", "`eventLoop`", "'eventLoop'", "\"eventLoop\"",
            "<eventLoop>", "[eventLoop]", "{eventLoop}", "eventLoop;", "eventLoop:", "::eventLoop",
            "eventLoop!", "eventLoop?", "*eventLoop*", "~eventLoop", "eventLoop%", "&eventLoop",
            "^eventLoop", "|eventLoop|", "=eventLoop", "event/loop", "event\\loop", "event,loop",
        ] {
            assert_eq!(words(input), event_loop, "{input}");
        }
    }

    #[test]
    fn non_ascii_runs_are_skipped_whole() {
        let result = query_words("café menu");
        assert_eq!(result.words, vec!["menu".to_string()]);
        assert_eq!(result.skipped_non_ascii, vec!["café".to_string()]);
        let result = query_words("naïve_bayes");
        assert_eq!(result.words, vec!["bayes".to_string()]);
        assert_eq!(result.skipped_non_ascii, vec!["naïve".to_string()]);
        // Non-ASCII punctuation and whitespace separate words like ASCII symbols do.
        assert_eq!(query_words("event\u{2014}loop").words, vec!["event".to_string(), "loop".to_string()]);
        assert_eq!(query_words("event\u{a0}loop").words, vec!["event".to_string(), "loop".to_string()]);
    }

    #[test]
    fn strip_outer_keeps_protected_symbols() {
        let cases = [
            ("`retry`", "retry"),
            ("$eventLoop", "eventLoop"),
            ("eventLoop()", "eventLoop"),
            ("--verbose", "verbose"),
            ("@Override", "Override"),
            ("#include", "include"),
            (".net", ".net"),
            ("'.csv'", ".csv"),
            ("c++", "c++"),
            ("C#", "C#"),
            ("gtk+", "gtk+"),
            ("...net", ".net"),
            ("café!", "café"),
            ("!!!", ""),
            ("", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(strip_outer(input), expected, "{input}");
        }
    }

    #[test]
    fn protected_candidates_find_symbolic_spellings() {
        assert_eq!(protected_candidates(".net"), vec![".net"]);
        assert_eq!(protected_candidates("read .CSV files"), vec![".csv"]);
        assert_eq!(protected_candidates("c++17"), vec!["c++"]);
        assert_eq!(protected_candidates("C#/.NET"), vec![".net", "c#"]);
        assert_eq!(protected_candidates("`gtk+`"), vec!["gtk+"]);
        assert!(protected_candidates("file.csv").is_empty());
        assert!(protected_candidates("req.body").is_empty());
        assert!(protected_candidates("eventLoop").is_empty());
    }

    #[test]
    fn spans_are_longest_first() {
        assert_eq!(spans(3), vec![(0, 3), (0, 2), (1, 3), (0, 1), (1, 2), (2, 3)]);
        assert_eq!(spans(1), vec![(0, 1)]);
        assert!(spans(0).is_empty());
        let long = spans(9);
        assert_eq!(long[0], (0, 9));
        assert!(long[1..].iter().all(|(start, end)| end - start <= MAX_SPAN_WORDS));
    }
}
