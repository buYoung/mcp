//! Serving lookup: local dataset + published repository statistics only.
//!
//! Stages (each rejected candidate keeps its stage and reason):
//! dictionary → normalization → repository presence → commonness → deduplication → limit.
//!
//! `rtl-2` rules (lemma fallback, phrase evidence, file-count ordering, morphology) are
//! enabled only by their policy fields; an `rtl-1` policy file keeps the `rtl-1` behavior
//! and the `rt-lookup/1` envelope.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::dataset::{Dataset, ExpressionRow, RelationRow};
use crate::morphology::{inflection_bases, Morphology};
use crate::normalize::QueryKeys;
use crate::stats::{hex, RepositoryState, RepositoryStats};

pub const OUTPUT_SCHEMA: &str = "rt-lookup/1";
/// `rt-lookup/1` plus the optional `rtl-2` fields (lemma fallback, phrase evidence,
/// ordering file count, morphology).
pub const OUTPUT_SCHEMA_V2: &str = "rt-lookup/2";
const LEMMA_FALLBACK_MATCH_KIND: &str = "lemma_fallback";

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LookupPolicy {
    pub policy_id: String,
    pub included_relation_kinds: Vec<String>,
    pub kind_priority: HashMap<String, u32>,
    pub commonness_max_file_fraction: f64,
    pub final_limit: usize,
    // rtl-2 fields. Each is absent from rtl-1, and its default keeps the rtl-1 behavior.
    /// H5: `lemma_fallback` listed here enables inflection stripping after exact/token misses.
    #[serde(default)]
    pub match_kinds: Vec<String>,
    /// H1 change: multi-token candidates need a concatenated-identifier occurrence, or one of
    /// these relation kinds with every component present.
    #[serde(default)]
    pub phrase_evidence_relation_kinds: Option<Vec<String>>,
    /// H4 change: order by repository file count after kind priority and sense rank.
    #[serde(default)]
    pub order_by_repository_file_count: bool,
    /// H6/H7: emit a separate `morphology` list of at most this many repository words.
    #[serde(default)]
    pub morphology_limit: Option<usize>,
    #[serde(default)]
    pub morphology_prefixes: Vec<String>,
    #[serde(default)]
    pub morphology_min_prefix_remainder_length: usize,
    #[serde(skip)]
    pub sha256: String,
}

impl LookupPolicy {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("policy unreadable: {error}"))?;
        let mut policy: Self = serde_json::from_slice(&bytes).map_err(|error| format!("policy invalid: {error}"))?;
        policy.sha256 = hex(&Sha256::digest(&bytes));
        Ok(policy)
    }

    fn is_lemma_fallback_enabled(&self) -> bool {
        self.match_kinds.iter().any(|kind| kind == LEMMA_FALLBACK_MATCH_KIND)
    }

    fn uses_rtl2_rules(&self) -> bool {
        self.is_lemma_fallback_enabled()
            || self.phrase_evidence_relation_kinds.is_some()
            || self.order_by_repository_file_count
            || self.morphology_limit.is_some()
    }
}

#[derive(Debug, Serialize)]
pub struct Match {
    pub expression_id: String,
    pub exact_key: String,
    pub token_key: String,
    pub surfaces: Vec<String>,
    pub match_kind: &'static str,
    pub concepts: i64,
}

#[derive(Debug, Serialize)]
pub struct Evidence {
    pub relation_id: String,
    pub relation_kind: String,
    pub evidence_kind: String,
    pub source_id: String,
    pub license: String,
    pub matched_expression_id: String,
    pub from_surface: String,
    pub from_source_record: String,
    pub to_surface: String,
    pub to_source_record: String,
    pub from_concept_id: String,
    pub to_concept_id: String,
    pub concept_kind: String,
    pub gloss: Option<String>,
    pub to_gloss: Option<String>,
    pub domain_labels: Vec<String>,
    /// WordNet sense order of the query word for this sense (0 = most frequent); 0 otherwise.
    pub query_sense_rank: i64,
    pub is_policy_excluded: bool,
}

#[derive(Debug, Serialize)]
pub struct ComponentCount {
    pub token: String,
    pub file_count: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RepositoryEvidence {
    /// Per index-token live-file counts. A phrase has no materialized phrase count.
    pub components: Vec<ComponentCount>,
    pub phrase_file_count: Option<u64>,
    pub presence: &'static str,
    pub single_token_file_fraction: Option<f64>,
    /// rtl-2 H1: file count of the index tokens joined without a separator (`eventloop`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concatenated_file_count: Option<u64>,
    /// rtl-2 H1: `concatenated` or `components_only` for a kept multi-token candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase_evidence: Option<&'static str>,
    /// rtl-2 H4: the file count used for ordering (see `RepositoryStats::term_file_count`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordering_file_count: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct Candidate {
    pub expression_id: String,
    pub exact_key: String,
    pub surfaces: Vec<String>,
    pub token_key: String,
    pub index_tokens: Vec<String>,
    pub primary_kind: Option<String>,
    pub relation_kinds: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub repository: Option<RepositoryEvidence>,
    pub outcome: &'static str,
    pub stage: &'static str,
    pub reason: Option<&'static str>,
    pub merged_into: Option<String>,
    pub final_rank: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct StageCounts {
    pub dictionary_relations: usize,
    pub normalized_candidates: usize,
    pub after_normalization: usize,
    pub after_repository_presence: usize,
    pub after_commonness: usize,
    pub after_deduplication: usize,
    pub final_candidates: usize,
}

/// rtl-2 H5 trace: inflection-stripped exact keys tried after the exact/token stages missed.
#[derive(Debug, Serialize)]
pub struct LemmaFallback {
    pub tried: Vec<String>,
    pub matched: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LookupOutput {
    pub schema: &'static str,
    pub input: String,
    pub normalized: QueryKeys,
    pub dataset_id: String,
    pub policy_id: String,
    pub policy_sha256: String,
    pub repository: RepositoryState,
    pub status: &'static str,
    pub matches: Vec<Match>,
    pub candidates: Vec<Candidate>,
    pub final_terms: Vec<String>,
    pub stage_counts: StageCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lemma_fallback: Option<LemmaFallback>,
    /// rtl-2 H6/H7: repository morphology, never part of `candidates` or `final_terms`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub morphology: Option<Morphology>,
    pub lookup_us: u128,
}

/// Exact keys to try for H5: the last word of the exact key replaced by each inflection base
/// (`event loops` -> `event loop`), in rule order.
fn lemma_keys(exact_key: &str) -> Vec<String> {
    let (head, last) = match exact_key.rsplit_once(' ') {
        Some((head, last)) => (format!("{head} "), last),
        None => (String::new(), exact_key),
    };
    inflection_bases(last)
        .into_iter()
        .map(|base| format!("{head}{base}"))
        .collect()
}

/// H5: the first inflection-stripped exact key that names a dictionary expression.
fn lemma_fallback(dataset: &Dataset, exact_key: &str) -> Result<(LemmaFallback, Option<ExpressionRow>), String> {
    let mut tried = Vec::new();
    for key in lemma_keys(exact_key) {
        tried.push(key.clone());
        if let Some(expression) = dataset.expression_by_exact_key(&key)? {
            return Ok((
                LemmaFallback {
                    tried,
                    matched: Some(key),
                },
                Some(expression),
            ));
        }
    }
    Ok((LemmaFallback { tried, matched: None }, None))
}

/// H1 change: why a multi-token candidate counts as present in the repository, if it does.
fn phrase_evidence(
    evidence: &[Evidence],
    component_kinds: &[String],
    presence: &str,
    concatenated_file_count: Option<u64>,
) -> Option<&'static str> {
    if concatenated_file_count.unwrap_or(0) > 0 {
        Some("concatenated")
    } else if presence == "all_components_present"
        && evidence
            .iter()
            .any(|e| !e.is_policy_excluded && component_kinds.contains(&e.relation_kind))
    {
        Some("components_only")
    } else {
        None
    }
}

pub fn lookup(
    dataset: &Dataset,
    repository: &RepositoryStats,
    policy: &LookupPolicy,
    input: &str,
) -> Result<LookupOutput, String> {
    let started = Instant::now();
    let keys = QueryKeys::parse(input);

    // Dictionary stage: exact key first, then normalized token key for other expressions.
    let mut matches = Vec::new();
    if let Some(expression) = dataset.expression_by_exact_key(&keys.exact_key)? {
        let concepts = dataset.concept_count(&expression.expression_id)?;
        matches.push(Match {
            expression_id: expression.expression_id,
            exact_key: expression.exact_key,
            token_key: expression.token_key,
            surfaces: expression.surfaces,
            match_kind: "exact",
            concepts,
        });
    }
    if !keys.token_key_lossy && !keys.token_key.is_empty() {
        for expression in dataset.expressions_by_token_key(&keys.token_key)? {
            if matches.iter().all(|m| m.expression_id != expression.expression_id) {
                let concepts = dataset.concept_count(&expression.expression_id)?;
                matches.push(Match {
                    expression_id: expression.expression_id,
                    exact_key: expression.exact_key,
                    token_key: expression.token_key,
                    surfaces: expression.surfaces,
                    match_kind: "token_normalized",
                    concepts,
                });
            }
        }
    }
    let mut lemma_trace = None;
    let mut lemma_keys_matched: Option<QueryKeys> = None;
    if matches.is_empty() && policy.is_lemma_fallback_enabled() {
        let (trace, expression) = lemma_fallback(dataset, &keys.exact_key)?;
        if let Some(expression) = expression {
            let concepts = dataset.concept_count(&expression.expression_id)?;
            lemma_keys_matched = Some(QueryKeys::parse(&expression.exact_key));
            matches.push(Match {
                expression_id: expression.expression_id,
                exact_key: expression.exact_key,
                token_key: expression.token_key,
                surfaces: expression.surfaces,
                match_kind: LEMMA_FALLBACK_MATCH_KIND,
                concepts,
            });
        }
        lemma_trace = Some(trace);
    }
    let mut relations: Vec<RelationRow> = Vec::new();
    for matched in &matches {
        relations.extend(dataset.relations_from(&matched.expression_id)?);
    }
    let dictionary_relations = relations.len();
    let matched_ids: Vec<&str> = matches.iter().map(|m| m.expression_id.as_str()).collect();

    // Normalization stage: one candidate per target expression, evidence kept per relation.
    let mut by_target: BTreeMap<String, Vec<RelationRow>> = BTreeMap::new();
    for relation in relations {
        by_target.entry(relation.to_expression_id.clone()).or_default().push(relation);
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    for (expression_id, rows) in by_target {
        let first = &rows[0];
        let evidence: Vec<Evidence> = rows
            .iter()
            .map(|row| Evidence {
                relation_id: row.relation_id.clone(),
                relation_kind: row.relation_kind.clone(),
                evidence_kind: row.evidence_kind.clone(),
                source_id: row.source_id.clone(),
                license: row.license_spdx.clone(),
                matched_expression_id: row.from_expression_id.clone(),
                from_surface: row.from_surface.clone(),
                from_source_record: row.from_source_record.clone(),
                to_surface: row.to_surface.clone(),
                to_source_record: row.to_source_record.clone(),
                from_concept_id: row.from_concept_id.clone(),
                to_concept_id: row.to_concept_id.clone(),
                concept_kind: row.from_concept_kind.clone(),
                gloss: row.from_gloss.clone(),
                to_gloss: (row.to_concept_id != row.from_concept_id).then(|| row.to_gloss.clone()).flatten(),
                domain_labels: row.from_domain_labels.clone(),
                query_sense_rank: if row.from_role == "wordnet_member" { row.from_rank } else { 0 },
                is_policy_excluded: !policy.included_relation_kinds.contains(&row.relation_kind),
            })
            .collect();
        let mut relation_kinds: Vec<String> = evidence.iter().map(|e| e.relation_kind.clone()).collect();
        relation_kinds.sort();
        relation_kinds.dedup();
        let primary_kind = evidence
            .iter()
            .filter(|e| !e.is_policy_excluded)
            .min_by_key(|e| policy.kind_priority.get(&e.relation_kind).copied().unwrap_or(u32::MAX))
            .map(|e| e.relation_kind.clone());
        let index_tokens = crate::normalize::index_tokens(&first.to_exact_key);
        let is_same_as_lemma = lemma_keys_matched.as_ref().is_some_and(|lemma| {
            (!lemma.token_key.is_empty() && first.to_token_key == lemma.token_key) || index_tokens == lemma.index_tokens
        });
        let (outcome, reason) = if primary_kind.is_none() {
            ("rejected", Some("excluded_relation_kind"))
        } else if matched_ids.contains(&expression_id.as_str())
            || (!keys.token_key.is_empty() && first.to_token_key == keys.token_key)
            || index_tokens == keys.index_tokens
            || is_same_as_lemma
        {
            ("rejected", Some("same_as_query"))
        } else if index_tokens.is_empty() {
            ("rejected", Some("no_index_tokens"))
        } else {
            ("pending", None)
        };
        candidates.push(Candidate {
            expression_id,
            exact_key: first.to_exact_key.clone(),
            surfaces: first.to_surfaces.clone(),
            token_key: first.to_token_key.clone(),
            index_tokens,
            primary_kind,
            relation_kinds,
            evidence,
            repository: None,
            outcome,
            stage: "normalization",
            reason,
            merged_into: None,
            final_rank: None,
        });
    }
    let normalized_candidates = candidates.len();
    let pending = |candidates: &[Candidate]| candidates.iter().filter(|c| c.outcome == "pending").count();
    let after_normalization = pending(&candidates);

    // Repository presence: component counts from the published statistics only.
    let is_ready = repository.is_ready();
    for candidate in candidates.iter_mut().filter(|c| c.outcome == "pending" || c.outcome == "rejected") {
        let components: Vec<ComponentCount> = candidate
            .index_tokens
            .iter()
            .map(|token| ComponentCount {
                token: token.clone(),
                file_count: repository.file_count(token),
            })
            .collect();
        let presence = if !is_ready {
            "unknown"
        } else if components.iter().all(|c| c.file_count.unwrap_or(0) > 0) {
            if components.len() > 1 { "all_components_present" } else { "present" }
        } else {
            "component_absent"
        };
        let single_token_file_fraction = match (&components[..], repository.state.live_files) {
            ([only], Some(live)) if live > 0 => only.file_count.map(|count| count as f64 / live as f64),
            _ => None,
        };
        let phrase_kinds = policy
            .phrase_evidence_relation_kinds
            .as_ref()
            .filter(|_| components.len() > 1);
        let concatenated_file_count = phrase_kinds.and_then(|_| repository.file_count(&candidate.index_tokens.concat()));
        let phrase_evidence =
            phrase_kinds.and_then(|kinds| phrase_evidence(&candidate.evidence, kinds, presence, concatenated_file_count));
        let ordering_file_count = if policy.order_by_repository_file_count {
            repository.term_file_count(&candidate.index_tokens)
        } else {
            None
        };
        candidate.repository = Some(RepositoryEvidence {
            components,
            phrase_file_count: None,
            presence,
            single_token_file_fraction,
            concatenated_file_count,
            phrase_evidence,
            ordering_file_count,
        });
        if candidate.outcome == "pending" && is_ready {
            if phrase_kinds.is_some() {
                if phrase_evidence.is_none() {
                    candidate.outcome = "rejected";
                    candidate.stage = "repository_presence";
                    candidate.reason = Some("phrase_without_identifier_evidence");
                }
            } else if presence == "component_absent" {
                candidate.outcome = "rejected";
                candidate.stage = "repository_presence";
                candidate.reason = Some("absent_in_repository");
            }
        }
    }
    let after_repository_presence = if is_ready { pending(&candidates) } else { 0 };

    // Commonness: only single-token candidates have a defined file fraction.
    if is_ready {
        for candidate in candidates.iter_mut().filter(|c| c.outcome == "pending") {
            let fraction = candidate.repository.as_ref().and_then(|r| r.single_token_file_fraction);
            if fraction.is_some_and(|fraction| fraction > policy.commonness_max_file_fraction) {
                candidate.outcome = "rejected";
                candidate.stage = "commonness";
                candidate.reason = Some("too_common");
            }
        }
    }
    let after_commonness = if is_ready { pending(&candidates) } else { 0 };

    // Order surviving candidates, then deduplicate by the index tokens a search would use.
    let order_key = |candidate: &Candidate| {
        let kind = candidate
            .primary_kind
            .as_ref()
            .and_then(|kind| policy.kind_priority.get(kind).copied())
            .unwrap_or(u32::MAX);
        let sense_rank = candidate
            .evidence
            .iter()
            .filter(|e| !e.is_policy_excluded)
            .map(|e| e.query_sense_rank)
            .min()
            .unwrap_or(i64::MAX);
        let support = candidate.evidence.iter().filter(|e| !e.is_policy_excluded).count();
        // Constant 0 unless the policy orders by repository file count (rtl-1 order unchanged).
        let file_count = candidate
            .repository
            .as_ref()
            .and_then(|r| r.ordering_file_count)
            .unwrap_or(0);
        (
            kind,
            sense_rank,
            std::cmp::Reverse(file_count),
            std::cmp::Reverse(support),
            candidate.exact_key.clone(),
        )
    };
    candidates.sort_by_key(|candidate| (candidate.outcome != "pending", order_key(candidate)));
    let mut final_terms = Vec::new();
    let mut after_deduplication = 0;
    if is_ready {
        let mut seen: HashMap<Vec<String>, String> = HashMap::new();
        for candidate in candidates.iter_mut().filter(|c| c.outcome == "pending") {
            if let Some(kept) = seen.get(&candidate.index_tokens) {
                candidate.outcome = "rejected";
                candidate.stage = "deduplication";
                candidate.reason = Some("duplicate_index_tokens");
                candidate.merged_into = Some(kept.clone());
                continue;
            }
            seen.insert(candidate.index_tokens.clone(), candidate.exact_key.clone());
            after_deduplication += 1;
            if final_terms.len() < policy.final_limit {
                candidate.outcome = "final";
                candidate.stage = "final";
                candidate.final_rank = Some(final_terms.len() + 1);
                final_terms.push(candidate.exact_key.clone());
            } else {
                candidate.outcome = "rejected";
                candidate.stage = "limit";
                candidate.reason = Some("over_limit");
            }
        }
    } else {
        for candidate in candidates.iter_mut().filter(|c| c.outcome == "pending") {
            candidate.outcome = "not_evaluated";
            candidate.stage = "repository_presence";
            candidate.reason = Some("repository_not_ready");
        }
    }

    let morphology = policy
        .morphology_limit
        .map(|limit| crate::morphology::morphology(repository, policy, &keys, limit));
    let status = if matches.is_empty() {
        "no_dictionary_entry"
    } else if dictionary_relations == 0 {
        "no_relations"
    } else if !is_ready {
        "repository_not_ready"
    } else if final_terms.is_empty() {
        "empty_after_filters"
    } else {
        "ok"
    };
    Ok(LookupOutput {
        schema: if policy.uses_rtl2_rules() { OUTPUT_SCHEMA_V2 } else { OUTPUT_SCHEMA },
        input: input.to_string(),
        normalized: keys,
        dataset_id: dataset.identity.dataset_id.clone(),
        policy_id: policy.policy_id.clone(),
        policy_sha256: policy.sha256.clone(),
        repository: repository.state.clone(),
        status,
        matches,
        stage_counts: StageCounts {
            dictionary_relations,
            normalized_candidates,
            after_normalization,
            after_repository_presence,
            after_commonness,
            after_deduplication,
            final_candidates: final_terms.len(),
        },
        candidates,
        final_terms,
        lemma_fallback: lemma_trace,
        morphology,
        lookup_us: started.elapsed().as_micros(),
    })
}
