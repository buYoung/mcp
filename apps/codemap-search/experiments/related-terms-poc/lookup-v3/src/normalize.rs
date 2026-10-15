//! Query normalization `rtn-2` (see contracts/dataset.md) and the repository-statistics
//! analyzer. Dataset keys are produced by the Python builder; `normalize-check` proves this
//! module derives the same keys for every dataset expression.

use tantivy::tokenizer::{
    Language, LowerCaser, RawTokenizer, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer,
};

pub const NORMALIZATION_VERSION: &str = "rtn-2";
/// The analyzer of every searchable codemap-search TEXT field (Tantivy `default`).
pub const ANALYZER_POLICY: &str = "tantivy-0.26.2/default:simple+remove_long(40)+lowercase";
/// Which vocabulary words get a `word_stems` row and how the stem is computed.
pub const STEM_POLICY: &str = "tantivy-0.26.2/stemmer:english(snowball);words=ascii-alphabetic,min-length-3";
const TOKEN_SEPARATORS: &[char] = &['_', '-', '.', '/', ':'];
const REMOVE_LONG_TOKEN_BYTES: usize = 40;
const MIN_STEMMED_WORD_LENGTH: usize = 3;

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryKeys {
    pub exact_key: String,
    pub token_key: String,
    pub token_key_lossy: bool,
    /// Terms the index analyzer would produce for this text; used only for repository
    /// statistics, never for dictionary matching.
    pub index_tokens: Vec<String>,
}

impl QueryKeys {
    pub fn parse(input: &str) -> Self {
        let (token_key, token_key_lossy) = token_key(input);
        Self {
            exact_key: exact_key(input),
            token_key,
            token_key_lossy,
            index_tokens: index_tokens(input),
        }
    }
}

/// Lowercase, collapse whitespace runs, trim. Punctuation is preserved.
pub fn exact_key(surface: &str) -> String {
    surface
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Alphanumeric runs split by `split_identifier`, joined with one space. The flag is true
/// when a character other than whitespace/`_-./:` was dropped or no token remains.
/// Non-ASCII text has no token key (exact matching only), because the Python builder
/// cannot reproduce Rust's Unicode Alphabetic property for combining marks.
pub fn token_key(surface: &str) -> (String, bool) {
    if !surface.is_ascii() {
        return (String::new(), true);
    }
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut is_lossy = false;
    for c in surface.chars() {
        if c.is_alphanumeric() {
            current.push(c);
            continue;
        }
        if !(c.is_whitespace() || TOKEN_SEPARATORS.contains(&c)) {
            is_lossy = true;
        }
        if !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    let tokens: Vec<String> = parts
        .iter()
        .flat_map(|part| codemap_search::parser::split_identifier(part))
        .collect();
    let is_lossy = is_lossy || tokens.is_empty();
    (tokens.join(" "), is_lossy)
}

/// The same analyzer chain codemap-search registers as `default` for TEXT fields.
pub fn index_analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(REMOVE_LONG_TOKEN_BYTES))
        .filter(LowerCaser)
        .build()
}

pub fn index_tokens(text: &str) -> Vec<String> {
    let mut analyzer = index_analyzer();
    let mut stream = analyzer.token_stream(text);
    let mut tokens = Vec::new();
    while stream.advance() {
        tokens.push(stream.token().text.clone());
    }
    tokens
}

/// Index words that get a stem: lowercase ASCII letters only, at least three of them.
/// Index words are already lowercased by the index analyzer.
pub fn is_stemmable(word: &str) -> bool {
    word.len() >= MIN_STEMMED_WORD_LENGTH && word.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// Tantivy's Snowball English stemmer over one whole word (no further tokenization).
pub fn stem_analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(RawTokenizer::default())
        .filter(Stemmer::new(Language::English))
        .build()
}

pub fn stem(analyzer: &mut TextAnalyzer, word: &str) -> String {
    let mut stream = analyzer.token_stream(word);
    if stream.advance() {
        stream.token().text.clone()
    } else {
        String::new()
    }
}
