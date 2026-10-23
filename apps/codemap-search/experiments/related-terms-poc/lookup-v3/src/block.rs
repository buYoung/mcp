//! `rtpoc2 block`: the feature-shaped related-terms block for one set of search terms.
//!
//! Each non-identifier term goes through `lookup` (dictionary final terms on the whole term);
//! morphology runs on the term itself (one index token) or on each eligible index token of a
//! phrase. The block then derives suggested queries, a budget-matching set of control queries
//! built from the original terms only, and a byte-capped plain-text rendering. The task query
//! is recorded only: nothing here interprets it or calls a model.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use serde::Serialize;

use crate::dataset::Dataset;
use crate::lookup::{self, LookupOutput, LookupPolicy};
use crate::morphology::MorphologyWord;
use crate::normalize::QueryKeys;
use crate::snake_parts::SnakeParts;
use crate::stats::RepositoryStats;

pub const OUTPUT_SCHEMA: &str = "rt-block/2";

/// Block limits of an rtl-2.1 policy file (H8–H12). Policies without them cannot build blocks.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BlockPolicy {
    pub block_dictionary_terms_per_term: usize,
    pub block_morphology_words_per_term: usize,
    pub block_gloss_max_chars: usize,
    pub block_suggested_query_limit: usize,
    pub block_query_min_words: usize,
    pub block_query_max_words: usize,
    pub block_anchor_words_max: usize,
    pub max_block_bytes: usize,
    /// H11: phrase index tokens shorter than this get no morphology.
    pub block_phrase_morphology_min_token_length: usize,
    /// H11: phrase index tokens that are English function words get no morphology.
    pub block_phrase_morphology_stopwords: Vec<String>,
    /// H11: morphology types the block may show; other types stay in the lookup output only.
    pub block_morphology_types: Vec<String>,
    /// H11 (rtl-2.2): inside a phrase, keep only singular/plural variants of a token.
    #[serde(default)]
    pub block_phrase_morphology_number_only: bool,
    /// H13 (rtl-2.2): how suggested queries relate to control queries.
    #[serde(default)]
    pub block_query_design: QueryDesign,
    /// H14 (rtl-2.3): phrase terms also list identifier spellings verified in the repository.
    #[serde(default)]
    pub block_identifier_forms_per_term: usize,
    /// H14: phrases with more index tokens than this get no identifier forms.
    #[serde(default)]
    pub block_identifier_form_max_tokens: usize,
    /// H14/H15: also accept snake_case spellings found in `rt-snake-parts/1`.
    #[serde(default)]
    pub block_identifier_snake_case: bool,
    /// H15: at most this many identifiers joining two input terms per block.
    #[serde(default)]
    pub block_cross_identifier_forms: usize,
    /// H15: two terms with more index tokens than this together are not joined.
    #[serde(default)]
    pub block_cross_identifier_max_tokens: usize,
    /// H17 (rtl-2.4): every related term of a single-token term is searched alone.
    #[serde(default)]
    pub block_single_token_standalone: bool,
}

/// H13: how suggested queries are built relative to the control queries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryDesign {
    /// H9/H12: suggested queries from each term's replacements; control queries separately.
    #[default]
    PerTermReplacements,
    /// Each suggested query is one control query with one related term appended.
    PairedAppend,
    /// Each suggested query is one control query with its term replaced by one related term.
    PairedSubstitute,
}

/// H13: one control query and the suggested query built from it. H16 standalone pairs search
/// an identifier alone against the words it was built from.
#[derive(Debug, Serialize)]
pub struct QueryPair {
    pub term: String,
    pub related: String,
    pub control: String,
    pub suggested: String,
}

impl BlockPolicy {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("policy unreadable: {error}"))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("policy has no {OUTPUT_SCHEMA} block limits (rtl-2.1 needed): {error}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockMode {
    Dictionary,
    Morphology,
    Both,
    /// H12: no related terms; the suggested queries are the control queries.
    ControlTermsOnly,
}

impl BlockMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "dictionary" => Ok(Self::Dictionary),
            "morphology" => Ok(Self::Morphology),
            "both" => Ok(Self::Both),
            "control_terms_only" => Ok(Self::ControlTermsOnly),
            _ => Err(format!(
                "--mode must be dictionary, morphology, both, or control_terms_only (got {value})"
            )),
        }
    }

    fn includes_dictionary(self) -> bool {
        matches!(self, Self::Dictionary | Self::Both)
    }

    fn includes_morphology(self) -> bool {
        matches!(self, Self::Morphology | Self::Both)
    }
}

#[derive(Debug, Serialize)]
pub struct DictionaryExpansion {
    pub term: String,
    pub relation_kind: Option<String>,
    pub gloss: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MorphologyExpansion {
    /// The index token the variant replaces (the whole term for a single-token term).
    pub token: String,
    pub word: String,
    #[serde(rename = "type")]
    pub morphology_type: &'static str,
    pub file_count: u64,
    /// The term with `token` replaced by `word` (just `word` for a single-token term).
    pub replacement: String,
    #[serde(skip)]
    is_token_absent: bool,
    #[serde(skip)]
    token_file_count: u64,
    #[serde(skip)]
    token_position: usize,
}

/// H11 trace: how one index token of a phrase was handled.
#[derive(Debug, Serialize)]
pub struct MorphologyToken {
    pub token: String,
    pub file_count: Option<u64>,
    pub is_absent_in_repository: bool,
    /// `expanded`, or why morphology was skipped: `too_short` or `stopword`.
    pub handling: &'static str,
    /// The token's morphology status (see `Morphology::status`) when expanded.
    pub morphology_status: Option<&'static str>,
    /// Stem-family words found for the token, before the type filter (uncapped).
    pub words_found: usize,
}

#[derive(Debug, Serialize)]
pub struct BlockTerm {
    pub input: String,
    /// `expanded` (looked up) or `passthrough` (kept as written).
    pub handling: &'static str,
    pub reason: Option<&'static str>,
    pub index_tokens: Vec<String>,
    pub lookup_status: Option<&'static str>,
    pub match_kinds: Vec<&'static str>,
    /// Single-token terms: the lookup morphology status. Phrases: `ok`, `empty`,
    /// `no_eligible_token`, or `repository_not_ready` over `morphology_tokens`.
    pub morphology_status: Option<&'static str>,
    /// See `RepositoryStats::term_file_count`; used for absence and anchor choice.
    pub repository_file_count: Option<u64>,
    pub query_absent_in_repository: bool,
    /// The index token with the smallest positive file count (used when this term is an
    /// anchor that does not fit a query whole).
    pub rarest_token: Option<String>,
    pub dictionary: Vec<DictionaryExpansion>,
    pub morphology: Vec<MorphologyExpansion>,
    /// H14: identifier spellings of a phrase found in the repository vocabulary, best first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub identifier_forms: Vec<IdentifierForm>,
    /// Phrases only: per-token morphology handling.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub morphology_tokens: Vec<MorphologyToken>,
    /// How many expansions `block_text` shows for this term after the H10 byte cap, counted in
    /// rendered order (morphology first when the term is absent from the repository, else
    /// dictionary first).
    pub shown_expansions: usize,
}

impl BlockTerm {
    /// Replacement texts in rank order: morphology first for a term absent from the
    /// repository, dictionary first otherwise.
    fn replacements(&self) -> Vec<&str> {
        let dictionary = self.dictionary.iter().map(|expansion| expansion.term.as_str());
        let morphology = self.morphology.iter().map(|expansion| expansion.replacement.as_str());
        if self.query_absent_in_repository {
            morphology.chain(dictionary).collect()
        } else {
            dictionary.chain(morphology).collect()
        }
    }

    /// Related terms in rendered order (H14 identifier forms first, then the `replacements`
    /// order): a morphology variant appends only its variant word; an H16 snake_case form and,
    /// with H17 `single_token_standalone`, every related term of a single-token term is
    /// searched alone.
    fn related(&self, single_token_standalone: bool) -> Vec<Related<'_>> {
        let is_single_token_standalone = single_token_standalone && self.index_tokens.len() == 1;
        let identifiers = self.identifier_forms.iter().map(|form| Related {
            appended: &form.identifier,
            replacement: &form.identifier,
            is_standalone: form.spelling == SNAKE,
        });
        let dictionary = self.dictionary.iter().map(move |expansion| Related {
            appended: &expansion.term,
            replacement: &expansion.term,
            is_standalone: is_single_token_standalone,
        });
        let morphology = self.morphology.iter().map(move |expansion| Related {
            appended: &expansion.word,
            replacement: &expansion.replacement,
            is_standalone: is_single_token_standalone,
        });
        if self.query_absent_in_repository {
            identifiers.chain(morphology).chain(dictionary).collect()
        } else {
            identifiers.chain(dictionary).chain(morphology).collect()
        }
    }

    /// Rendered expansions, in the same rank order as `replacements`.
    fn rendered_expansions(&self) -> Vec<String> {
        let dictionary = self.dictionary.iter().map(|expansion| {
            let kind = expansion.relation_kind.as_deref().unwrap_or("related");
            match &expansion.gloss {
                Some(gloss) => format!("{} [{kind}: \"{gloss}\"]", expansion.term),
                None => format!("{} [{kind}]", expansion.term),
            }
        });
        let morphology = self
            .morphology
            .iter()
            .map(|expansion| format!("{} [{}]", expansion.replacement, expansion.morphology_type));
        let identifiers = self.identifier_forms.iter().map(|form| format!("{} [{}]", form.identifier, form.tag()));
        if self.query_absent_in_repository {
            identifiers.chain(morphology).chain(dictionary).collect()
        } else {
            identifiers.chain(dictionary).chain(morphology).collect()
        }
    }
}

/// One related term of a block term, as used by H13/H16 query pairs.
struct Related<'a> {
    /// Text appended to the control query (paired_append).
    appended: &'a str,
    /// Text replacing the whole term (paired_substitute).
    replacement: &'a str,
    /// H16: searched alone, paired with the term's own words alone.
    is_standalone: bool,
}

const CAMEL: &str = "camel";
const SNAKE: &str = "snake";

/// H14: one identifier spelling of a phrase term that exists in the repository.
#[derive(Debug, Serialize)]
pub struct IdentifierForm {
    /// The spelling used in queries and the block: camelCase (`search parser` -> `searchParser`)
    /// or snake_case (`test file` -> `test_file`).
    pub identifier: String,
    /// The key found in the repository: the lowercase concatenated word (`searchparser`, in
    /// `word_files`) or the underscore-joined part sequence (`test_file`, in `rt-snake-parts/1`).
    pub index_token: String,
    pub file_count: u64,
    /// True when every token kept its input number (no singular/plural change).
    pub is_exact: bool,
    /// `camel` or `snake` (H14 rtl-2.3).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub spelling: &'static str,
}

impl IdentifierForm {
    /// Block tag: a camelCase form is a whole repository word, a snake_case form a run of parts
    /// of repository identifiers (`test_file` inside `is_test_file`).
    fn tag(&self) -> &'static str {
        if self.spelling == SNAKE {
            "identifier part in repo"
        } else {
            "identifier in repo"
        }
    }
}

/// H15: an identifier that joins two input terms (`sync` + `external alertmanager` ->
/// `syncExternalAlertmanager`) and exists in the repository.
#[derive(Debug, Serialize)]
pub struct CrossIdentifierForm {
    /// The two input terms, in identifier order.
    pub terms: [String; 2],
    #[serde(flatten)]
    pub form: IdentifierForm,
}

/// The token itself and its regular singular/plural spellings (see `is_number_inflection`).
fn number_spellings(token: &str) -> Vec<String> {
    let mut spellings = vec![token.to_string()];
    let mut candidates = vec![format!("{token}s"), format!("{token}es")];
    if let Some(stem) = token.strip_suffix("ies") {
        candidates.push(format!("{stem}y"));
    }
    if let Some(stem) = token.strip_suffix('y') {
        candidates.push(format!("{stem}ies"));
    }
    for suffix in ["es", "s"] {
        if let Some(stem) = token.strip_suffix(suffix) {
            candidates.push(stem.to_string());
        }
    }
    for candidate in candidates {
        if !candidate.is_empty() && is_number_inflection(token, &candidate) && !spellings.contains(&candidate) {
            spellings.push(candidate);
        }
    }
    spellings
}

fn camel_case(parts: &[String]) -> String {
    parts
        .iter()
        .enumerate()
        .map(|(position, part)| {
            if position == 0 {
                part.clone()
            } else {
                let mut chars = part.chars();
                chars.next().map(|first| first.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
            }
        })
        .collect()
}

/// H14/H15 verification of index tokens: every combination of input or regular singular/plural
/// spellings whose concatenation is a repository word (camelCase form) or, with
/// `block_identifier_snake_case`, whose underscore-joined parts occur in repository snake_case
/// identifiers (snake_case form). Tokens must be ASCII alphanumeric.
fn verified_forms(
    repository: &RepositoryStats,
    snake_parts: &SnakeParts,
    policy: &BlockPolicy,
    index_tokens: &[String],
) -> Vec<IdentifierForm> {
    if !index_tokens.iter().all(|token| !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric())) {
        return Vec::new();
    }
    let mut combinations: Vec<Vec<String>> = vec![Vec::new()];
    for token in index_tokens {
        combinations = combinations
            .into_iter()
            .flat_map(|prefix| {
                number_spellings(token).into_iter().map(move |spelling| {
                    let mut next = prefix.clone();
                    next.push(spelling);
                    next
                })
            })
            .collect();
    }
    let mut forms = Vec::new();
    for parts in combinations {
        let is_exact = parts.as_slice() == index_tokens;
        let index_token = parts.concat();
        if let Some(file_count) = repository.file_count(&index_token).filter(|&count| count > 0) {
            forms.push(IdentifierForm { identifier: camel_case(&parts), index_token, file_count, is_exact, spelling: CAMEL });
        }
        if policy.block_identifier_snake_case {
            if let Some(file_count) = snake_parts.file_count(&parts).filter(|&count| count > 0) {
                let snake = parts.join("_");
                forms.push(IdentifierForm { identifier: snake.clone(), index_token: snake, file_count, is_exact, spelling: SNAKE });
            }
        }
    }
    forms
}

/// H14: identifier forms of a phrase with 2..=`block_identifier_form_max_tokens` index tokens.
/// The exact spelling ranks first, then by file count descending; at most
/// `block_identifier_forms_per_term`.
fn identifier_forms(
    repository: &RepositoryStats,
    snake_parts: &SnakeParts,
    policy: &BlockPolicy,
    index_tokens: &[String],
) -> Vec<IdentifierForm> {
    if policy.block_identifier_forms_per_term == 0
        || index_tokens.len() < 2
        || index_tokens.len() > policy.block_identifier_form_max_tokens
    {
        return Vec::new();
    }
    let mut forms = verified_forms(repository, snake_parts, policy, index_tokens);
    forms.sort_by(|left, right| {
        right
            .is_exact
            .cmp(&left.is_exact)
            .then_with(|| right.file_count.cmp(&left.file_count))
            .then_with(|| left.identifier.cmp(&right.identifier))
    });
    forms.dedup_by(|right, left| right.identifier == left.identifier);
    forms.truncate(policy.block_identifier_forms_per_term);
    forms
}

/// The spelling-independent key of a form (`syncExternalAlertmanager` = `sync_external_alertmanager`).
fn form_key(identifier: &str) -> String {
    identifier.replace('_', "").to_ascii_lowercase()
}

/// H15: identifiers joining two different expanded input terms (either order) with at most
/// `block_cross_identifier_max_tokens` index tokens together, verified like H14. The rarest
/// (smallest file count) first, then by identifier; forms equal to an H14 form or an earlier
/// form are dropped; at most `block_cross_identifier_forms`.
fn cross_identifier_forms(
    repository: &RepositoryStats,
    snake_parts: &SnakeParts,
    policy: &BlockPolicy,
    terms: &[BlockTerm],
) -> Vec<CrossIdentifierForm> {
    if policy.block_cross_identifier_forms == 0 {
        return Vec::new();
    }
    let mut expanded: Vec<&BlockTerm> = Vec::new();
    for term in terms.iter().filter(|term| term.handling == "expanded" && !term.index_tokens.is_empty()) {
        // A repeated term (same index tokens) is joined once.
        if !expanded.iter().any(|kept| kept.index_tokens == term.index_tokens) {
            expanded.push(term);
        }
    }
    let mut forms = Vec::new();
    for (first_position, first) in expanded.iter().enumerate() {
        for (second_position, second) in expanded.iter().enumerate() {
            let token_count = first.index_tokens.len() + second.index_tokens.len();
            if first_position == second_position || token_count > policy.block_cross_identifier_max_tokens {
                continue;
            }
            let tokens: Vec<String> = first.index_tokens.iter().chain(&second.index_tokens).cloned().collect();
            for form in verified_forms(repository, snake_parts, policy, &tokens) {
                forms.push(CrossIdentifierForm { terms: [first.input.clone(), second.input.clone()], form });
            }
        }
    }
    forms.sort_by(|left, right| {
        left.form
            .file_count
            .cmp(&right.form.file_count)
            .then_with(|| left.form.identifier.cmp(&right.form.identifier))
            .then_with(|| left.terms.cmp(&right.terms))
    });
    let mut seen: HashSet<String> = terms
        .iter()
        .flat_map(|term| term.identifier_forms.iter().map(|form| form_key(&form.identifier)))
        .collect();
    forms.retain(|cross| seen.insert(form_key(&cross.form.identifier)));
    forms.truncate(policy.block_cross_identifier_forms);
    forms
}

#[derive(Debug, Serialize)]
pub struct BlockOmissions {
    pub expansions: usize,
    /// H15 cross-term identifiers dropped for size (absent when zero).
    #[serde(skip_serializing_if = "is_zero")]
    pub cross_identifier_forms: usize,
    pub queries: usize,
    pub term_lines: usize,
    pub is_byte_truncated: bool,
}

fn is_zero(count: &usize) -> bool {
    *count == 0
}

/// Identifier-shaped terms are searched as written, never expanded.
fn is_identifier_shaped(term: &str) -> bool {
    term.contains("::") || term.contains('.') || codemap_search::parser::split_identifier(term).len() > 1
}

fn trim_gloss(gloss: &str, max_chars: usize) -> String {
    let collapsed = gloss.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let kept: String = collapsed.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{}...", kept.trim_end())
}

fn dictionary_expansions(output: &LookupOutput, policy: &BlockPolicy) -> Vec<DictionaryExpansion> {
    let mut finals: Vec<_> = output.candidates.iter().filter(|c| c.outcome == "final").collect();
    finals.sort_by_key(|candidate| candidate.final_rank);
    finals
        .into_iter()
        .take(policy.block_dictionary_terms_per_term)
        .map(|candidate| DictionaryExpansion {
            term: candidate.exact_key.clone(),
            relation_kind: candidate.primary_kind.clone(),
            gloss: candidate
                .evidence
                .iter()
                .filter(|e| !e.is_policy_excluded)
                .find_map(|e| e.gloss.as_deref())
                .map(|gloss| trim_gloss(gloss, policy.block_gloss_max_chars)),
        })
        .collect()
}

fn morphology_type_rank(morphology_type: &str) -> u8 {
    match morphology_type {
        "inflection" => 0,
        "derivation" => 1,
        _ => 2,
    }
}

/// H11 order inside a term, over the types listed in `block_morphology_types`: variants of
/// tokens absent from the repository first, then inflection → derivation → same_root, then
/// variants of the rarer token (smaller token file count), then variant file count
/// descending; capped per term.
fn rank_morphology(expansions: Vec<MorphologyExpansion>, policy: &BlockPolicy) -> Vec<MorphologyExpansion> {
    let mut expansions: Vec<MorphologyExpansion> = expansions
        .into_iter()
        .filter(|expansion| policy.block_morphology_types.iter().any(|kind| kind == expansion.morphology_type))
        .collect();
    expansions.sort_by(|left, right| {
        (!left.is_token_absent, morphology_type_rank(left.morphology_type), left.token_file_count)
            .cmp(&(!right.is_token_absent, morphology_type_rank(right.morphology_type), right.token_file_count))
            .then_with(|| right.file_count.cmp(&left.file_count))
            .then_with(|| left.token_position.cmp(&right.token_position))
            .then_with(|| left.word.cmp(&right.word))
    });
    expansions.truncate(policy.block_morphology_words_per_term);
    expansions
}

fn morphology_expansion(
    word: &MorphologyWord,
    token: &str,
    token_position: usize,
    is_token_absent: bool,
    token_file_count: Option<u64>,
    replacement: String,
) -> MorphologyExpansion {
    MorphologyExpansion {
        token: token.to_string(),
        word: word.word.clone(),
        morphology_type: word.morphology_type,
        file_count: word.file_count,
        replacement,
        is_token_absent,
        token_file_count: token_file_count.unwrap_or(0),
        token_position,
    }
}

/// H11: the block filters morphology by type before capping, so it reads the whole stem family
/// (no `morphology_limit`) whenever the policy enables morphology at all.
fn morphology_pool_limit(lookup_policy: &LookupPolicy) -> usize {
    lookup_policy.morphology_limit.map_or(0, |_| usize::MAX)
}

/// The block's morphology status: `empty` when the type filter left nothing to show.
fn shown_morphology_status(status: &'static str, expansions: &[MorphologyExpansion]) -> &'static str {
    if status == "ok" && expansions.is_empty() {
        "empty"
    } else {
        status
    }
}

/// True when one word is the regular English plural of the other: `-es` after s, x, z, ch,
/// sh; consonant + `y` → `-ies`; `-s` otherwise.
fn is_number_inflection(left: &str, right: &str) -> bool {
    let is_plural_of = |plural: &str, singular: &str| {
        let takes_es = ["s", "x", "z", "ch", "sh"].iter().any(|ending| singular.ends_with(ending));
        let takes_ies = singular
            .strip_suffix('y')
            .and_then(|stem| stem.chars().last())
            .is_some_and(|before| !"aeiou".contains(before));
        match plural.strip_prefix(singular) {
            Some("es") => takes_es,
            Some("s") => !takes_es && !takes_ies,
            _ => takes_ies && plural.strip_prefix(&singular[..singular.len() - 1]) == Some("ies"),
        }
    };
    is_plural_of(left, right) || is_plural_of(right, left)
}

/// H11: morphology on each eligible index token of a phrase. Each variant replaces that
/// token (every occurrence) in the lowercase index-token text of the phrase.
fn phrase_morphology(
    repository: &RepositoryStats,
    lookup_policy: &LookupPolicy,
    policy: &BlockPolicy,
    index_tokens: &[String],
) -> (Vec<MorphologyToken>, Vec<MorphologyExpansion>, &'static str) {
    let mut tokens = Vec::new();
    let mut expansions = Vec::new();
    let limit = morphology_pool_limit(lookup_policy);
    let mut seen: HashSet<&str> = HashSet::new();
    for (position, token) in index_tokens.iter().enumerate() {
        if !seen.insert(token.as_str()) {
            continue;
        }
        let file_count = repository.file_count(token);
        let mut record = MorphologyToken {
            token: token.clone(),
            file_count,
            is_absent_in_repository: repository.is_ready() && file_count == Some(0),
            handling: "expanded",
            morphology_status: None,
            words_found: 0,
        };
        if token.chars().count() < policy.block_phrase_morphology_min_token_length {
            record.handling = "too_short";
        } else if policy.block_phrase_morphology_stopwords.contains(token) {
            record.handling = "stopword";
        } else {
            let morphology = crate::morphology::morphology(repository, lookup_policy, &QueryKeys::parse(token), limit);
            record.morphology_status = Some(morphology.status);
            record.words_found = morphology.words.len();
            for word in &morphology.words {
                if policy.block_phrase_morphology_number_only && !is_number_inflection(token, &word.word) {
                    continue;
                }
                let replacement = index_tokens
                    .iter()
                    .map(|other| if other == token { word.word.as_str() } else { other.as_str() })
                    .collect::<Vec<_>>()
                    .join(" ");
                expansions.push(morphology_expansion(word, token, position, record.is_absent_in_repository, file_count, replacement));
            }
        }
        tokens.push(record);
    }
    let expansions = rank_morphology(expansions, policy);
    let status = if !repository.is_ready() {
        "repository_not_ready"
    } else if tokens.iter().all(|token| token.handling != "expanded") {
        "no_eligible_token"
    } else if expansions.is_empty() {
        "empty"
    } else {
        "ok"
    };
    (tokens, expansions, status)
}

/// The index token with the smallest positive file count (earliest on ties).
fn rarest_token(repository: &RepositoryStats, index_tokens: &[String]) -> Option<String> {
    index_tokens
        .iter()
        .enumerate()
        .filter_map(|(position, token)| {
            repository
                .file_count(token)
                .filter(|&count| count > 0)
                .map(|count| (count, position, token))
        })
        .min()
        .map(|(_, _, token)| token.clone())
}

fn block_term(
    dataset: &Dataset,
    repository: &RepositoryStats,
    snake_parts: &SnakeParts,
    lookup_policy: &LookupPolicy,
    policy: &BlockPolicy,
    mode: BlockMode,
    input: &str,
) -> Result<BlockTerm, String> {
    let index_tokens = crate::normalize::index_tokens(input);
    let repository_file_count = repository.term_file_count(&index_tokens);
    let mut term = BlockTerm {
        input: input.to_string(),
        handling: "passthrough",
        reason: Some("identifier_shaped"),
        rarest_token: rarest_token(repository, &index_tokens),
        index_tokens,
        lookup_status: None,
        match_kinds: Vec::new(),
        morphology_status: None,
        repository_file_count,
        query_absent_in_repository: repository.is_ready() && repository_file_count == Some(0),
        dictionary: Vec::new(),
        morphology: Vec::new(),
        morphology_tokens: Vec::new(),
        identifier_forms: Vec::new(),
        shown_expansions: 0,
    };
    if is_identifier_shaped(input) {
        return Ok(term);
    }
    let output = lookup::lookup(dataset, repository, lookup_policy, input)?;
    term.handling = "expanded";
    term.reason = None;
    term.lookup_status = Some(output.status);
    term.match_kinds = output.matches.iter().map(|m| m.match_kind).collect();
    if mode.includes_dictionary() {
        term.dictionary = dictionary_expansions(&output, policy);
    }
    if term.index_tokens.len() > 1 {
        let (tokens, expansions, status) = phrase_morphology(repository, lookup_policy, policy, &term.index_tokens);
        term.morphology_tokens = tokens;
        term.morphology_status = Some(status);
        if mode.includes_morphology() {
            term.morphology = expansions;
            term.identifier_forms = identifier_forms(repository, snake_parts, policy, &term.index_tokens);
        }
    } else {
        // A single-token term runs H6/H7 morphology on the whole term and replaces itself.
        let morphology = crate::morphology::morphology(
            repository,
            lookup_policy,
            &QueryKeys::parse(input),
            morphology_pool_limit(lookup_policy),
        );
        let token = morphology.query_word.clone().unwrap_or_default();
        let token_file_count = repository.file_count(&token);
        let expansions: Vec<MorphologyExpansion> = morphology
            .words
            .iter()
            .map(|word| {
                morphology_expansion(
                    word,
                    &token,
                    0,
                    morphology.query_absent_in_repository,
                    token_file_count,
                    word.word.clone(),
                )
            })
            .collect();
        let expansions = rank_morphology(expansions, policy);
        term.morphology_status = Some(shown_morphology_status(morphology.status, &expansions));
        if mode.includes_morphology() {
            term.morphology = expansions;
        }
    }
    Ok(term)
}

fn query_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}

/// An input term considered as an anchor: its text and its rarest single index token.
struct Anchor<'a> {
    text: &'a str,
    rarest_token: Option<&'a str>,
}

/// One replacement plus up to `block_anchor_words_max` anchors, or `None` when no anchor
/// fits. An anchor too long for `block_query_max_words` is replaced by its rarest single
/// token; an anchor repeating a query word is skipped.
fn compose_query(replacement: &str, anchors: &[Anchor<'_>], policy: &BlockPolicy) -> Option<String> {
    let mut parts = vec![replacement.to_string()];
    let mut words: Vec<String> = query_words(replacement);
    let mut anchors_used = 0;
    for anchor in anchors {
        if anchors_used == policy.block_anchor_words_max {
            break;
        }
        let mut text = anchor.text;
        if words.len() + query_words(text).len() > policy.block_query_max_words {
            match anchor.rarest_token {
                Some(token) => text = token,
                None => continue,
            }
        }
        let anchor_words = query_words(text);
        if words.len() + anchor_words.len() > policy.block_query_max_words
            || anchor_words.iter().any(|word| words.contains(word))
        {
            continue;
        }
        words.extend(anchor_words);
        parts.push(text.to_string());
        anchors_used += 1;
    }
    (anchors_used > 0 && words.len() >= policy.block_query_min_words).then(|| parts.join(" "))
}

/// H9 term order: expanded terms absent from the repository first, then input order.
fn expansion_order(terms: &[BlockTerm]) -> Vec<usize> {
    let mut expanded: Vec<usize> = (0..terms.len()).filter(|&position| terms[position].handling == "expanded").collect();
    expanded.sort_by_key(|&position| !terms[position].query_absent_in_repository);
    expanded
}

/// H9 anchor order for each term: the other input terms by smallest positive repository file
/// count (ties by input order).
fn anchors_by_term(terms: &[BlockTerm]) -> Vec<Vec<Anchor<'_>>> {
    let mut anchor_order: Vec<(u64, usize)> = terms
        .iter()
        .enumerate()
        .filter_map(|(position, term)| {
            term.repository_file_count
                .filter(|&count| count > 0)
                .map(|count| (count, position))
        })
        .collect();
    anchor_order.sort();
    (0..terms.len())
        .map(|position| {
            anchor_order
                .iter()
                .filter(|(_, anchor)| *anchor != position)
                .map(|(_, anchor)| Anchor {
                    text: terms[*anchor].input.as_str(),
                    rarest_token: terms[*anchor].rarest_token.as_deref(),
                })
                .collect()
        })
        .collect()
}

/// Appends `query` unless its lowercase word set was already used; false once at the limit.
fn push_unique(queries: &mut Vec<String>, seen: &mut HashSet<BTreeSet<String>>, query: String, limit: usize) -> bool {
    if seen.insert(query_words(&query).into_iter().collect()) {
        queries.push(query);
    }
    queries.len() < limit
}

/// H9: each expanded term's replacements, in term order, combined with the term's first
/// anchors. Duplicate queries (same lowercase word set) are dropped; at most
/// `block_suggested_query_limit` queries.
fn suggested_queries(terms: &[BlockTerm], policy: &BlockPolicy) -> Vec<String> {
    let anchors = anchors_by_term(terms);
    let mut queries = Vec::new();
    let mut seen: HashSet<BTreeSet<String>> = HashSet::new();
    if policy.block_suggested_query_limit == 0 {
        return queries;
    }
    for position in expansion_order(terms) {
        for replacement in terms[position].replacements() {
            let Some(query) = compose_query(replacement, &anchors[position], policy) else {
                continue;
            };
            if !push_unique(&mut queries, &mut seen, query, policy.block_suggested_query_limit) {
                return queries;
            }
        }
    }
    queries
}

/// H12 control candidates of one term: the H9 query with the term itself, the term with each
/// single anchor, then the term alone when its word count is within the query limits.
fn control_candidates(
    terms: &[BlockTerm],
    anchors: &[Vec<Anchor<'_>>],
    position: usize,
    policy: &BlockPolicy,
    single_anchor: &BlockPolicy,
) -> Vec<String> {
    let input = terms[position].input.as_str();
    let mut list: Vec<String> = compose_query(input, &anchors[position], policy).into_iter().collect();
    list.extend(
        (0..anchors[position].len()).filter_map(|start| compose_query(input, &anchors[position][start..=start], single_anchor)),
    );
    let word_count = query_words(input).len();
    if word_count >= policy.block_query_min_words && word_count <= policy.block_query_max_words {
        list.push(input.to_string());
    }
    list
}

/// H13: (control, suggested) query pairs. H16 (rtl-2.3): H15 cross-term identifiers come first,
/// each searched alone against its two terms' words. Then only terms with related terms take
/// part, in H9 term order; their H12 control candidates are used round-robin across terms, and
/// each suggested query appends (paired_append) or swaps in (paired_substitute) the term's next
/// usable related term, except that an H16 standalone form (snake_case) is searched alone
/// against the term alone. A related term is usable when the suggested query adds a new word and
/// repeats no earlier suggestion or control; a control query is skipped when it repeats an
/// earlier word set or no related term of its term is usable; at most
/// block_suggested_query_limit pairs.
fn paired_queries(terms: &[BlockTerm], cross: &[CrossIdentifierForm], policy: &BlockPolicy, design: QueryDesign) -> Vec<QueryPair> {
    let anchors = anchors_by_term(terms);
    let single_anchor = BlockPolicy { block_anchor_words_max: 1, ..policy.clone() };
    // A term without H12 control candidates (no anchor, too few words) can still take part
    // through an H16 standalone form, whose control is the term alone.
    let single_token_standalone = policy.block_single_token_standalone;
    let lists: Vec<(usize, Vec<String>, bool)> = expansion_order(terms)
        .into_iter()
        .filter(|&position| !terms[position].related(single_token_standalone).is_empty())
        .filter_map(|position| {
            let list = control_candidates(terms, &anchors, position, policy, &single_anchor);
            if !list.is_empty() {
                Some((position, list, false))
            } else {
                let has_standalone =
                    terms[position].related(single_token_standalone).iter().any(|related| related.is_standalone);
                has_standalone.then(|| (position, vec![terms[position].input.clone()], true))
            }
        })
        .collect();
    let mut pairs = Vec::new();
    let mut used = vec![0usize; terms.len()];
    let mut seen_control: HashSet<BTreeSet<String>> = HashSet::new();
    let mut seen_suggested: HashSet<BTreeSet<String>> = HashSet::new();
    for form in cross {
        if pairs.len() >= policy.block_suggested_query_limit {
            return pairs;
        }
        let control = form.terms.join(" ");
        let control_words: BTreeSet<String> = query_words(&control).into_iter().collect();
        let suggested_words: BTreeSet<String> = query_words(&form.form.identifier).into_iter().collect();
        if suggested_words.is_subset(&control_words)
            || seen_control.contains(&control_words)
            || seen_suggested.contains(&suggested_words)
        {
            continue;
        }
        seen_control.insert(control_words);
        seen_suggested.insert(suggested_words);
        pairs.push(QueryPair {
            term: form.terms.join(" + "),
            related: form.form.identifier.clone(),
            control,
            suggested: form.form.identifier.clone(),
        });
    }
    let rounds = lists.iter().map(|(_, list, _)| list.len()).max().unwrap_or(0);
    for round in 0..rounds {
        for (position, list, is_standalone_only) in &lists {
            if pairs.len() >= policy.block_suggested_query_limit {
                return pairs;
            }
            let Some(control) = list.get(round) else {
                continue;
            };
            let term = &terms[*position];
            let related = term.related(single_token_standalone);
            let control_words: BTreeSet<String> = query_words(control).into_iter().collect();
            // H17: when every related term is searched alone, the round's H12 control is unused.
            let is_round_control_unused = single_token_standalone && related.iter().all(|related| related.is_standalone);
            if seen_control.contains(&control_words) && !is_round_control_unused {
                continue;
            }
            // The term's related terms are tried in turn from the next unused one; the control
            // is skipped only when none adds a new word or every result repeats a suggestion.
            let chosen = (0..related.len()).find_map(|offset| {
                let index = (used[*position] + offset) % related.len();
                let candidate = &related[index];
                if *is_standalone_only && !candidate.is_standalone {
                    return None;
                }
                let (pair_control, suggested) = if candidate.is_standalone {
                    (term.input.clone(), candidate.appended.to_string())
                } else {
                    let suggested = match design {
                        QueryDesign::PairedSubstitute => format!("{}{}", candidate.replacement, &control[term.input.len()..]),
                        _ => format!("{control} {}", candidate.appended),
                    };
                    (control.clone(), suggested)
                };
                let pair_control_words: BTreeSet<String> = query_words(&pair_control).into_iter().collect();
                let suggested_words: BTreeSet<String> = query_words(&suggested).into_iter().collect();
                let is_usable = suggested_words != pair_control_words
                    && !suggested_words.is_subset(&pair_control_words)
                    && !seen_suggested.contains(&suggested_words)
                    && (!candidate.is_standalone || !seen_control.contains(&pair_control_words));
                is_usable.then_some((index, candidate.appended, pair_control, pair_control_words, suggested, suggested_words))
            });
            let Some((index, appended, pair_control, pair_control_words, suggested, suggested_words)) = chosen else {
                continue;
            };
            seen_control.insert(pair_control_words);
            seen_suggested.insert(suggested_words);
            used[*position] = index + 1;
            pairs.push(QueryPair {
                term: term.input.clone(),
                related: appended.to_string(),
                control: pair_control,
                suggested,
            });
        }
    }
    pairs
}

/// H12: control queries use the input terms only. Each expanded term (H9 term order) has
/// control candidates in this order: the H9 query with the term itself as the replacement,
/// the term with each single anchor (anchor order), then the term alone when it has at least
/// block_query_min_words words. Candidates are taken round-robin across terms (every term's
/// first candidate, then every term's second, ...) so the control can spend as many searches
/// as the suggested queries.
fn control_queries(terms: &[BlockTerm], policy: &BlockPolicy) -> Vec<String> {
    let anchors = anchors_by_term(terms);
    let single_anchor = BlockPolicy { block_anchor_words_max: 1, ..policy.clone() };
    let candidates: Vec<Vec<String>> = expansion_order(terms)
        .into_iter()
        .map(|position| control_candidates(terms, &anchors, position, policy, &single_anchor))
        .collect();
    let mut queries = Vec::new();
    let mut seen: HashSet<BTreeSet<String>> = HashSet::new();
    if policy.block_suggested_query_limit == 0 {
        return queries;
    }
    let rounds = candidates.iter().map(Vec::len).max().unwrap_or(0);
    for round in 0..rounds {
        for list in &candidates {
            let Some(query) = list.get(round) else {
                continue;
            };
            if !push_unique(&mut queries, &mut seen, query.clone(), policy.block_suggested_query_limit) {
                return queries;
            }
        }
    }
    queries
}

fn render_block(
    mode: BlockMode,
    terms: &[BlockTerm],
    expansions: &[Vec<String>],
    kept: &[usize],
    cross: &[String],
    queries: &[String],
) -> String {
    let mode_label = match mode {
        BlockMode::Dictionary => "dictionary",
        BlockMode::Morphology => "morphology",
        BlockMode::Both => "dictionary+morphology",
        BlockMode::ControlTermsOnly => "control: input terms only",
    };
    let mut text = format!("Related search terms ({mode_label}; unverified for this task)\n");
    for ((term, expansions), &kept) in terms.iter().zip(expansions).zip(kept) {
        let label = if term.query_absent_in_repository {
            format!("{} (absent in repo)", term.input)
        } else {
            term.input.clone()
        };
        let body = if term.handling == "passthrough" {
            "identifier, search as written".to_string()
        } else if mode == BlockMode::ControlTermsOnly {
            "search as written".to_string()
        } else if expansions.is_empty() {
            "no related terms".to_string()
        } else if kept == 0 {
            "(omitted for size)".to_string()
        } else {
            expansions[..kept].join("; ")
        };
        text.push_str(&format!("{label}: {body}\n"));
    }
    if !cross.is_empty() {
        text.push_str(&format!("Identifiers joining two terms: {}\n", cross.join("; ")));
    }
    if !queries.is_empty() {
        text.push_str(&format!("Suggested queries: {}\n", queries.join(" | ")));
    }
    text
}

fn truncate_to_bytes(text: &mut String, max_bytes: usize) {
    let mut end = max_bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
}

/// H15 rendering of a cross-term identifier: `syncExternalAlertmanager (sync + external alertmanager)`.
fn rendered_cross_form(cross: &CrossIdentifierForm) -> String {
    let part = if cross.form.spelling == SNAKE { ", identifier part" } else { "" };
    format!("{} ({} + {}{part})", cross.form.identifier, cross.terms[0], cross.terms[1])
}

/// Render within `max_block_bytes`: drop the lowest-ranked expansion (latest rank position,
/// later term on ties) until it fits, then H15 cross-term identifiers from the end, then
/// queries from the end, then term lines from the end, and finally cut bytes. The result never
/// exceeds the cap. Returns the kept expansions per term and the kept cross-term identifiers.
fn capped_block(
    mode: BlockMode,
    terms: &[BlockTerm],
    cross: &[CrossIdentifierForm],
    queries: &[String],
    max_bytes: usize,
) -> (String, BlockOmissions, Vec<usize>, usize) {
    let expansions: Vec<Vec<String>> = terms.iter().map(BlockTerm::rendered_expansions).collect();
    let cross: Vec<String> = cross.iter().map(rendered_cross_form).collect();
    let mut kept: Vec<usize> = expansions.iter().map(Vec::len).collect();
    let mut kept_cross = cross.len();
    let mut kept_queries = queries.len();
    let mut kept_terms = terms.len();
    let mut omissions = BlockOmissions {
        expansions: 0,
        cross_identifier_forms: 0,
        queries: 0,
        term_lines: 0,
        is_byte_truncated: false,
    };
    loop {
        let mut text = render_block(
            mode,
            &terms[..kept_terms],
            &expansions[..kept_terms],
            &kept[..kept_terms],
            &cross[..kept_cross],
            &queries[..kept_queries],
        );
        if text.len() <= max_bytes {
            kept[kept_terms..].fill(0);
            return (text, omissions, kept, kept_cross);
        }
        let lowest_ranked = (0..kept_terms).filter(|&position| kept[position] > 0).max_by_key(|&position| (kept[position], position));
        if let Some(position) = lowest_ranked {
            kept[position] -= 1;
            omissions.expansions += 1;
        } else if kept_cross > 0 {
            kept_cross -= 1;
            omissions.cross_identifier_forms += 1;
        } else if kept_queries > 0 {
            kept_queries -= 1;
            omissions.queries += 1;
        } else if kept_terms > 0 {
            kept_terms -= 1;
            omissions.term_lines += 1;
        } else {
            truncate_to_bytes(&mut text, max_bytes);
            omissions.is_byte_truncated = true;
            return (text, omissions, kept, kept_cross);
        }
    }
}

pub fn build_block(
    dataset: &Dataset,
    repository: &RepositoryStats,
    snake_parts: &SnakeParts,
    lookup_policy: &LookupPolicy,
    policy: &BlockPolicy,
    mode: BlockMode,
    task_query: &str,
    inputs: &[String],
) -> Result<serde_json::Value, String> {
    let mut terms: Vec<BlockTerm> = inputs
        .iter()
        .map(|input| block_term(dataset, repository, snake_parts, lookup_policy, policy, mode, input.trim()))
        .collect::<Result<_, _>>()?;
    let cross = if mode.includes_morphology() {
        cross_identifier_forms(repository, snake_parts, policy, &terms)
    } else {
        Vec::new()
    };
    let (queries, control, pairs) = match (mode, policy.block_query_design) {
        (BlockMode::ControlTermsOnly, _) => {
            let control = control_queries(&terms, policy);
            (control.clone(), control, Vec::new())
        }
        (_, QueryDesign::PerTermReplacements) => {
            (suggested_queries(&terms, policy), control_queries(&terms, policy), Vec::new())
        }
        (_, design) => {
            let pairs = paired_queries(&terms, &cross, policy, design);
            let queries = pairs.iter().map(|pair| pair.suggested.clone()).collect();
            let control = pairs.iter().map(|pair| pair.control.clone()).collect();
            (queries, control, pairs)
        }
    };
    let (block_text, omissions, kept, kept_cross) = capped_block(mode, &terms, &cross, &queries, policy.max_block_bytes);
    // Only the queries block_text shows are suggested (and only their pairs and controls).
    let shown_queries = queries.len() - omissions.queries;
    let mut queries = queries;
    let mut control = control;
    let mut pairs = pairs;
    queries.truncate(shown_queries);
    pairs.truncate(shown_queries);
    if mode == BlockMode::ControlTermsOnly || policy.block_query_design != QueryDesign::PerTermReplacements {
        control.truncate(shown_queries);
    }
    for (term, shown) in terms.iter_mut().zip(kept) {
        term.shown_expansions = if mode == BlockMode::ControlTermsOnly { 0 } else { shown };
    }
    let block_bytes = block_text.len();
    let published = repository.published.as_ref();
    let mut block = serde_json::json!({
        "schema": OUTPUT_SCHEMA,
        "dataset": dataset.identity,
        "policy": {"policy_id": lookup_policy.policy_id, "policy_sha256": lookup_policy.sha256},
        "stats": {
            "status": repository.state.status,
            "detail": repository.state.detail,
            "stats_format": published.map(|p| &p.stats_format),
            "index_generation": published.map(|p| &p.index_generation),
            "stats_file": published.map(|p| &p.stats_file),
            "stats_sha256": published.map(|p| &p.stats_sha256),
            "stem_policy": published.map(|p| &p.stem_policy),
            "live_files": repository.state.live_files,
        },
        "task_query": task_query,
        "task_query_use": "recorded_only",
        "mode": mode,
        "terms": terms,
        "query_design": policy.block_query_design,
        "suggested_queries": queries,
        "control_queries": control,
        "query_pairs": pairs,
        "block_text": block_text,
        "block_bytes": block_bytes,
        "max_block_bytes": policy.max_block_bytes,
        "block_omissions": omissions,
    });
    // rtl-2.3 keys only; earlier policies keep their exact output.
    if policy.block_identifier_snake_case {
        block["stats"]["snake_parts"] = serde_json::json!({
            "status": snake_parts.status, "detail": snake_parts.detail, "sha256": snake_parts.sha256,
        });
    }
    if policy.block_cross_identifier_forms > 0 {
        block["cross_identifier_forms"] = serde_json::to_value(&cross).map_err(|error| error.to_string())?;
        block["shown_cross_identifier_forms"] = serde_json::json!(if mode == BlockMode::ControlTermsOnly { 0 } else { kept_cross });
    }
    Ok(block)
}
