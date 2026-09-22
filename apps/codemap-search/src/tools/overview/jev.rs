//! Root-only recommendation over a single, captured codemap publication.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::jev::{Answer, Cancellation, Evaluator, Policy, Question, Request, Usage};
use crate::parser::{ExtractedFile, ExtractedSymbol};

use super::OverviewPreparation;

const QUESTION_VERSION: &str = "overview-fragment-v1";
const POLICY_VERSION: &str = "overview-qualification-v1-experimental";
const MAX_RECOMMENDATIONS: usize = 24;
const MAX_ADDITION_BYTES: usize = 32 * 1024;

#[derive(Debug)]
pub struct RecommendationResult {
    pub text: String,
    pub status: &'static str,
    pub usage: Usage,
    pub elapsed: Duration,
    pub snapshot_identity: usize,
    pub fragment_answers: BTreeMap<String, Answer>,
    pub role_answers: BTreeMap<String, Answer>,
    pub question_version: &'static str,
    pub policy_version: &'static str,
    pub evaluated_files: usize,
    pub evaluated_fragments: usize,
}

#[derive(Debug)]
pub struct RecommendationFailure {
    pub reason: &'static str,
    pub usage: Option<Usage>,
    pub elapsed: Duration,
}

struct Fragment {
    file_index: usize,
    id: String,
    evidence: Value,
}

fn masked(value: &str) -> String {
    crate::redact::source(value).into_owned()
}

fn fragments(files: &[ExtractedFile]) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    for (file_index, file) in files.iter().enumerate() {
        let mut evidence = Vec::new();
        for symbols in file.symbols.chunks(12) {
            evidence.push(json!({
                "file_path":file.file_path,
                "total_lines":file.total_lines,
                "declarations":symbols.iter().map(|symbol| json!({
                    "kind":symbol.kind,"name":symbol.name,"owner":symbol.owner,
                    "start_line":symbol.range.start_line,
                    "end_line":symbol.range.end_line_inclusive(),
                    "docstring":symbol.docstring
                })).collect::<Vec<_>>()
            }));
        }
        for docstrings in file.docstrings.chunks(12) {
            evidence.push(json!({
                "file_path":file.file_path,
                "total_lines":file.total_lines,
                "file_docstrings":docstrings
            }));
        }
        if let Some(navigation) = &file.navigation {
            for calls in navigation.calls.chunks(24) {
                evidence.push(json!({
                    "file_path":file.file_path,
                    "total_lines":file.total_lines,
                    "possible_calls":calls.iter().map(|call| json!({
                        "name":call.name,"line":call.range.start_line
                    })).collect::<Vec<_>>()
                }));
            }
        }
        if evidence.is_empty() {
            evidence.push(json!({
                "file_path":file.file_path,"total_lines":file.total_lines,
                "indexed_evidence":"No declarations or docs in this indexed file."
            }));
        }
        for (part_index, item) in evidence.into_iter().enumerate() {
            fragments.push(Fragment {
                file_index,
                id: format!("f{file_index}_{part_index}"),
                evidence: json!({"candidate": masked(&item.to_string())}),
            });
        }
    }
    fragments
}

fn score_question(evidence: Value) -> Question {
    Question::Score {
        instructions: json!({
            "judgment":"How useful is `candidate` for implementing or locating the behavior requested in `task_query`? Judge only the indexed evidence. Include direct implementation, configuration, callers, consumers and failure handling. Do not assume that absence of words proves irrelevance.",
            "candidate":evidence["candidate"]
        }),
        criteria: vec![
            json!("No useful evidence for the requested behavior."),
            json!("Tangential background or generic wrapper."),
            json!("Important supporting implementation, configuration, caller or consumer."),
            json!("Direct implementation of the requested behavior."),
        ],
    }
}

fn role_question(file: &ExtractedFile, symbol: &ExtractedSymbol) -> Question {
    let candidate = json!({
        "file_path":file.file_path,"kind":symbol.kind,"name":symbol.name,
        "owner":symbol.owner,"docstring":symbol.docstring,
        "start_line":symbol.range.start_line,
        "end_line":symbol.range.end_line_inclusive()
    });
    Question::Choice {
        instructions: json!({
            "judgment":"Choose one representative navigation role for `declaration` in relation to `task_query`. A role is a hint, not a verified or exhaustive classification.",
            "declaration":masked(&candidate.to_string())
        }),
        criteria: BTreeMap::from([
            (
                "implementation".into(),
                json!("Direct behavior implementation."),
            ),
            (
                "configuration".into(),
                json!("Configuration or contract controlling the behavior."),
            ),
            (
                "caller".into(),
                json!("Caller or entry point reaching the behavior."),
            ),
            ("consumer".into(), json!("Consumer or downstream effect.")),
            (
                "unrelated".into(),
                json!("No supported relation in the indexed evidence."),
            ),
        ]),
    }
}

fn failure(reason: &'static str, usage: Option<Usage>, started: Instant) -> RecommendationFailure {
    RecommendationFailure {
        reason,
        usage,
        elapsed: started.elapsed(),
    }
}

pub async fn recommend(
    preparation: OverviewPreparation,
    task_query: &str,
    evaluator: &dyn Evaluator,
    policy: Policy,
    cancellation: Cancellation,
    output_cap: Option<usize>,
) -> Result<RecommendationResult, RecommendationFailure> {
    let started = Instant::now();
    let Some(snapshot) = preparation.snapshot else {
        return Err(failure(
            "index_not_ready_or_non_root",
            Some(Usage::default()),
            started,
        ));
    };
    let snapshot_identity = preparation.snapshot_identity.unwrap_or_default();
    let candidates = fragments(&snapshot);
    let questions = candidates
        .iter()
        .map(|fragment| {
            (
                fragment.id.clone(),
                score_question(fragment.evidence.clone()),
            )
        })
        .collect();
    let state = json!({"task_query":masked(task_query)});
    let scores = evaluator
        .evaluate(
            Request {
                state: state.clone(),
                questions,
            },
            policy.clone(),
            cancellation.clone(),
        )
        .await
        .map_err(|error| failure(error.label(), None, started))?;
    let mut best_scores: Vec<Option<f64>> = vec![None; snapshot.len()];
    let mut has_tie = false;
    let has_usable_indexed_evidence = snapshot.iter().any(|file| {
        !file.symbols.is_empty()
            || !file.docstrings.is_empty()
            || file
                .navigation
                .as_ref()
                .is_some_and(|navigation| !navigation.calls.is_empty())
    });
    for fragment in &candidates {
        let Some(Answer::Score {
            score,
            probabilities,
            ..
        }) = scores.answers.get(&fragment.id)
        else {
            return Err(failure("invalid_score_answer", Some(scores.usage), started));
        };
        let positive = probabilities["2"] + probabilities["3"];
        let negative = probabilities["0"] + probabilities["1"];
        has_tie |= (positive - negative).abs() <= 1e-9;
        if positive > negative {
            let current = &mut best_scores[fragment.file_index];
            *current = Some(current.map_or(*score, |old| old.max(*score)));
        }
    }
    let mut selected: Vec<usize> = best_scores
        .iter()
        .enumerate()
        .filter_map(|(index, score)| score.map(|_| index))
        .collect();
    selected.sort_by(|&left, &right| {
        best_scores[right]
            .unwrap()
            .total_cmp(&best_scores[left].unwrap())
            .then_with(|| snapshot[left].file_path.cmp(&snapshot[right].file_path))
    });
    selected.truncate(MAX_RECOMMENDATIONS);
    let status = if !selected.is_empty() {
        "matched"
    } else if has_tie || !has_usable_indexed_evidence {
        "insufficient_evidence"
    } else {
        "no_match"
    };
    let mut usage = scores.usage;
    let mut role_answers = BTreeMap::new();
    if !selected.is_empty() {
        let mut role_questions = BTreeMap::new();
        for &file_index in &selected {
            let file = &snapshot[file_index];
            for (symbol_index, symbol) in file.symbols.iter().enumerate() {
                role_questions.insert(
                    format!("r{file_index}_{symbol_index}"),
                    role_question(file, symbol),
                );
            }
        }
        if !role_questions.is_empty() {
            let roles = evaluator
                .evaluate(
                    Request {
                        state,
                        questions: role_questions,
                    },
                    policy,
                    cancellation,
                )
                .await
                .map_err(|error| failure(error.label(), Some(usage), started))?;
            usage.input_tokens = usage.input_tokens.saturating_add(roles.usage.input_tokens);
            usage.output_tokens = usage
                .output_tokens
                .saturating_add(roles.usage.output_tokens);
            role_answers = roles.answers;
        }
    }
    let mut addition = format!(
        "\n\n## Jev indexed recommendations\n\nrecommendation_status={status}; question_version={QUESTION_VERSION}; policy_version={POLICY_VERSION}; evaluated_files={}; evaluated_fragments={}. Indexed evidence alone cannot establish that source behavior is absent. Use search/read/grep/find to continue.\n",
        snapshot.len(), candidates.len()
    );
    for file_index in selected {
        let file = &snapshot[file_index];
        let path = masked(&file.file_path);
        addition.push_str(&format!(
            "\n- `{path}` — indexed relevance score {:.3}; read {read}\n",
            best_scores[file_index].unwrap(),
            read = json!({"file_path":file.file_path,"offset":1,"limit":file.total_lines.min(180).max(1),"view":"source"})
        ));
        let mut supported = file
            .symbols
            .iter()
            .enumerate()
            .filter_map(|(symbol_index, symbol)| {
                let id = format!("r{file_index}_{symbol_index}");
                match role_answers.get(&id) {
                    Some(Answer::Choice {
                        choice,
                        probabilities,
                        ..
                    }) if choice != "unrelated" => Some((
                        probabilities.get(choice).copied().unwrap_or(0.0),
                        symbol,
                        choice,
                    )),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        supported.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.range.start_line.cmp(&b.1.range.start_line))
        });
        for (_, symbol, role) in supported.into_iter().take(2) {
            let start = symbol.range.start_line;
            let end = symbol.range.end_line_inclusive();
            let calls = file
                .navigation
                .as_ref()
                .map(|navigation| {
                    navigation
                        .calls
                        .iter()
                        .filter(|call| {
                            start <= call.range.start_line && call.range.start_line <= end
                        })
                        .take(3)
                        .map(|call| masked(&call.name))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            addition.push_str(&format!(
                "  - {} {} [L{start}-{end}] — role: {role}; possible calls: {}; read {}\n",
                masked(&symbol.kind), masked(&symbol.name),
                if calls.is_empty() { "none shown".into() } else { calls.join(", ") },
                json!({"file_path":file.file_path,"offset":start,"limit":end.saturating_sub(start).saturating_add(1).min(180),"view":"source"})
            ));
        }
    }
    if addition.len() > MAX_ADDITION_BYTES
        || output_cap.is_some_and(|cap| preparation.text.len().saturating_add(addition.len()) > cap)
    {
        return Err(failure("output_limit", Some(usage), started));
    }
    Ok(RecommendationResult {
        text: format!("{}{addition}", preparation.text),
        status,
        usage,
        elapsed: started.elapsed(),
        snapshot_identity,
        fragment_answers: scores.answers,
        role_answers,
        question_version: QUESTION_VERSION,
        policy_version: POLICY_VERSION,
        evaluated_files: snapshot.len(),
        evaluated_fragments: candidates.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::jev::{Evaluation, JevError};
    use crate::parser::{CodeRange, SymbolFlags};

    struct FakeEvaluator {
        calls: AtomicUsize,
    }

    struct DelayedEvaluator(FakeEvaluator);

    impl Evaluator for DelayedEvaluator {
        fn evaluate<'a>(
            &'a self,
            request: Request,
            policy: Policy,
            cancellation: Cancellation,
        ) -> Pin<Box<dyn Future<Output = Result<Evaluation, JevError>> + Send + 'a>> {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                self.0.evaluate(request, policy, cancellation).await
            })
        }
    }

    impl Evaluator for FakeEvaluator {
        fn evaluate<'a>(
            &'a self,
            request: Request,
            _policy: Policy,
            _cancellation: Cancellation,
        ) -> Pin<Box<dyn Future<Output = Result<Evaluation, JevError>> + Send + 'a>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let answers = request
                    .questions
                    .into_iter()
                    .map(|(id, question)| {
                        let answer = match question {
                            Question::Score { instructions, .. } => {
                                let is_fit = instructions.to_string().contains("fit_");
                                let is_tie = instructions.to_string().contains("tie_");
                                let probabilities = if is_tie {
                                    BTreeMap::from([
                                        ("0".into(), 0.25),
                                        ("1".into(), 0.25),
                                        ("2".into(), 0.25),
                                        ("3".into(), 0.25),
                                    ])
                                } else if is_fit {
                                    BTreeMap::from([
                                        ("0".into(), 0.0),
                                        ("1".into(), 0.0),
                                        ("2".into(), 0.0),
                                        ("3".into(), 1.0),
                                    ])
                                } else {
                                    BTreeMap::from([
                                        ("0".into(), 1.0),
                                        ("1".into(), 0.0),
                                        ("2".into(), 0.0),
                                        ("3".into(), 0.0),
                                    ])
                                };
                                Answer::Score {
                                    score: if is_tie {
                                        1.5
                                    } else if is_fit {
                                        3.0
                                    } else {
                                        0.0
                                    },
                                    probabilities,
                                    confidence: 1.0,
                                }
                            }
                            Question::Choice { criteria, .. } => {
                                let probabilities = criteria
                                    .keys()
                                    .map(|key| {
                                        (
                                            key.clone(),
                                            if key == "implementation" { 1.0 } else { 0.0 },
                                        )
                                    })
                                    .collect();
                                Answer::Choice {
                                    choice: "implementation".into(),
                                    probabilities,
                                    confidence: 1.0,
                                }
                            }
                            Question::Noul { .. } => return Err(JevError::InvalidRequest),
                        };
                        Ok((id, answer))
                    })
                    .collect::<Result<BTreeMap<_, _>, JevError>>()?;
                Ok(Evaluation {
                    model: crate::jev::MODEL.into(),
                    answers,
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens: 1,
                    },
                    elapsed: Duration::ZERO,
                    batches: Vec::new(),
                })
            })
        }
    }

    fn file(path: String, with_symbol: bool) -> ExtractedFile {
        ExtractedFile {
            file_path: path,
            total_lines: 10,
            symbols: if with_symbol {
                vec![ExtractedSymbol {
                    name: "handler".into(),
                    kind: "fn".into(),
                    range: CodeRange {
                        start_line: 2,
                        start_col: 1,
                        end_line: 4,
                        end_col: 2,
                    },
                    docstring: None,
                    owner: None,
                    flags: SymbolFlags {
                        has_todo: false,
                        has_fixme: false,
                        is_test: false,
                        is_exported: true,
                        is_deprecated: false,
                    },
                }]
            } else {
                Vec::new()
            },
            literals: Vec::new(),
            docstrings: Vec::new(),
            navigation: None,
        }
    }

    fn preparation(files: Vec<ExtractedFile>) -> OverviewPreparation {
        OverviewPreparation {
            text: "# Root Codemap Overview".into(),
            snapshot: Some(Arc::new(files)),
            snapshot_identity: Some(42),
        }
    }

    #[tokio::test]
    async fn evaluates_the_complete_catalog_before_limiting_recommendations() {
        let evaluator = FakeEvaluator {
            calls: AtomicUsize::new(0),
        };
        let mut files = (0..30)
            .map(|index| file(format!("fit_{index:02}.rs"), false))
            .collect::<Vec<_>>();
        files.push(file("bad.rs".into(), false));
        let result = recommend(
            preparation(files),
            "Find handler",
            &evaluator,
            Policy::default(),
            Cancellation::new(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.evaluated_files, 31);
        assert_eq!(result.evaluated_fragments, 31);
        assert_eq!(result.text.matches("indexed relevance score").count(), 24);
        assert!(!result.text.contains("`bad.rs` — indexed relevance"));
        assert_eq!(evaluator.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn no_match_skips_role_evaluation_and_preserves_base_overview() {
        let evaluator = FakeEvaluator {
            calls: AtomicUsize::new(0),
        };
        let result = recommend(
            preparation(vec![file("bad.rs".into(), true)]),
            "Find handler",
            &evaluator,
            Policy::default(),
            Cancellation::new(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.status, "no_match");
        assert_eq!(result.text.matches("indexed relevance score").count(), 0);
        assert!(result.text.starts_with("# Root Codemap Overview"));
        assert_eq!(evaluator.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn tied_or_absent_index_evidence_is_insufficient() {
        let evaluator = FakeEvaluator {
            calls: AtomicUsize::new(0),
        };
        let tied = recommend(
            preparation(vec![file("tie_a.rs".into(), true)]),
            "Find handler",
            &evaluator,
            Policy::default(),
            Cancellation::new(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(tied.status, "insufficient_evidence");
        let absent = recommend(
            preparation(vec![file("bad.rs".into(), false)]),
            "Find handler",
            &evaluator,
            Policy::default(),
            Cancellation::new(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(absent.status, "insufficient_evidence");
    }

    #[tokio::test]
    async fn output_cap_falls_back_after_complete_evaluation() {
        let evaluator = FakeEvaluator {
            calls: AtomicUsize::new(0),
        };
        let result = recommend(
            preparation(vec![file("fit_a.rs".into(), true)]),
            "Find handler",
            &evaluator,
            Policy::default(),
            Cancellation::new(),
            Some(40),
        )
        .await
        .unwrap_err();
        assert_eq!(result.reason, "output_limit");
        assert_eq!(evaluator.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn held_snapshot_cannot_mix_with_a_new_publication_during_evaluation() {
        let old = Arc::new(vec![file("fit_old.rs".into(), false)]);
        let publication = Arc::new(std::sync::Mutex::new(Arc::clone(&old)));
        let preparation = OverviewPreparation {
            text: "# Old root".into(),
            snapshot: Some(old),
            snapshot_identity: Some(42),
        };
        let evaluator = Arc::new(DelayedEvaluator(FakeEvaluator {
            calls: AtomicUsize::new(0),
        }));
        let task = tokio::spawn({
            let evaluator = Arc::clone(&evaluator);
            async move {
                recommend(
                    preparation,
                    "Find handler",
                    evaluator.as_ref(),
                    Policy::default(),
                    Cancellation::new(),
                    None,
                )
                .await
                .unwrap()
            }
        });
        tokio::time::sleep(Duration::from_millis(5)).await;
        *publication.lock().unwrap() = Arc::new(vec![file("fit_new.rs".into(), false)]);
        let result = task.await.unwrap();
        assert_eq!(result.snapshot_identity, 42);
        assert!(result.text.contains("fit_old.rs"));
        assert!(!result.text.contains("fit_new.rs"));
    }
}
