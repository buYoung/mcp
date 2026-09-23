//! Offline regressions for the root recommendation adapter: every evaluation runs through
//! `jev::mock::MockEvaluator`; no index, filesystem or network is involved. Fake answers
//! verify the payload the model receives and the Rust policy over the answers; they say
//! nothing about the model's accuracy.

use super::*;
use crate::jev::mock::{answers, Gate, MockEvaluator};
use crate::jev::EvaluationRequest;
use crate::parser::{CallSite, CodeRange, NavigationFile, SymbolFlags};
use std::sync::Arc;

const TASK: &str = "where are search result bodies filtered before rendering?";
const AWS_LIKE_TOKEN: &str = "AKIAABCDEFGHIJKLMNOP";

fn range(start: usize, end: usize) -> CodeRange {
    CodeRange {
        start_line: start,
        start_col: 1,
        end_line: end,
        end_col: 2,
    }
}

fn symbol(name: &str, kind: &str, start: usize, end: usize) -> ExtractedSymbol {
    ExtractedSymbol {
        name: name.into(),
        kind: kind.into(),
        range: range(start, end),
        docstring: None,
        flags: SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: true,
            is_deprecated: false,
        },
        owner: None,
    }
}

fn with_doc(mut symbol: ExtractedSymbol, doc: &str) -> ExtractedSymbol {
    symbol.docstring = Some(doc.into());
    symbol
}

fn with_owner(mut symbol: ExtractedSymbol, owner: &str) -> ExtractedSymbol {
    symbol.owner = Some(owner.into());
    symbol
}

fn private(mut symbol: ExtractedSymbol) -> ExtractedSymbol {
    symbol.flags.is_exported = false;
    symbol
}

fn test_only(mut symbol: ExtractedSymbol) -> ExtractedSymbol {
    symbol.flags.is_test = true;
    symbol
}

fn call(name: &str, receiver: Option<&str>, line: usize) -> CallSite {
    CallSite {
        name: name.into(),
        receiver: receiver.map(Into::into),
        range: range(line, line),
        scope_id: None,
    }
}

fn file(
    path: &str,
    total_lines: usize,
    symbols: Vec<ExtractedSymbol>,
    docstrings: Vec<&str>,
    calls: Vec<CallSite>,
) -> ExtractedFile {
    ExtractedFile {
        file_path: path.into(),
        total_lines,
        symbols,
        literals: Vec::new(),
        docstrings: docstrings.into_iter().map(Into::into).collect(),
        navigation: (!calls.is_empty()).then(|| NavigationFile {
            calls,
            ..NavigationFile::default()
        }),
    }
}

fn catalog() -> Vec<ExtractedFile> {
    vec![
        file(
            "src/search/filter.rs",
            120,
            vec![
                with_doc(
                    symbol("filter_bodies", "fn", 10, 60),
                    "Drops unrelated bodies after ranking.",
                ),
                symbol("Retention", "struct", 62, 68),
                with_owner(symbol("apply", "fn", 70, 100), "Retention"),
            ],
            vec!["Search body filtering."],
            vec![
                call("rank", None, 20),
                call("apply", Some("self"), 30),
                call("trim_all", None, 80),
            ],
        ),
        file(
            "src/search/rank.rs",
            80,
            vec![symbol("rank", "fn", 5, 40)],
            vec![],
            vec![call("helper", None, 12)],
        ),
        file(
            "src/util/strings.rs",
            30,
            vec![
                symbol("trim_all", "fn", 1, 10),
                private(symbol("local_scratch", "let", 3, 5)),
                private(symbol("helper", "fn", 12, 20)),
                test_only(symbol("test_trim", "fn", 22, 30)),
                symbol("util", "mod", 1, 30),
            ],
            vec![],
            vec![],
        ),
    ]
}

fn input_from(files: &[ExtractedFile]) -> RootInput {
    RootInput::capture(TASK, 7, files).expect("valid input")
}

fn declaration_index(input: &RootInput, name: &str) -> usize {
    input
        .declarations()
        .iter()
        .position(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("declaration {name} is captured"))
}

/// A Choice distribution over the eight roles: `role` takes what `unrelated` leaves after
/// 0.02 for each other positive option; `insufficient_evidence` stays at zero.
fn role_distribution(role: &str, unrelated_probability: f64) -> Vec<(&'static str, f64)> {
    let filler_count = ROLES
        .iter()
        .filter(|(option, _)| !is_negative_role(option) && *option != role)
        .count() as f64;
    ROLES
        .iter()
        .map(|(option, _)| {
            let probability = if *option == "unrelated" {
                unrelated_probability
            } else if *option == "insufficient_evidence" {
                0.0
            } else if *option == role {
                1.0 - unrelated_probability - 0.02 * filler_count
            } else {
                0.02
            };
            (*option, probability)
        })
        .collect()
}

fn negative_distribution(option: &'static str) -> Vec<(&'static str, f64)> {
    ROLES
        .iter()
        .map(|(candidate, _)| (*candidate, if *candidate == option { 0.86 } else { 0.02 }))
        .collect()
}

/// Scores by `file_path`, roles by declaration name as `(role, P(unrelated))`.
fn scripted(
    scores: BTreeMap<&'static str, [f64; 4]>,
    roles: BTreeMap<&'static str, (&'static str, f64)>,
) -> MockEvaluator {
    MockEvaluator::new(move |request: &EvaluationRequest| {
        let mut answers = BTreeMap::new();
        for question in request.questions() {
            let instructions = question.instructions();
            let answer = if let Some(evidence) = instructions.get("candidate") {
                let path = evidence["file_path"]
                    .as_str()
                    .expect("fragment names its file");
                let distribution = scores.get(path).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]);
                answers::score(&distribution)
            } else {
                let name = instructions["declaration"]["name"]
                    .as_str()
                    .expect("role question names its declaration");
                let (role, unrelated) = roles.get(name).copied().unwrap_or(("unrelated", 0.86));
                let distribution = if is_negative_role(role) {
                    negative_distribution(role)
                } else {
                    role_distribution(role, unrelated)
                };
                answers::choice(role, &distribution)
            };
            answers.insert(question.id().clone(), answer);
        }
        Ok(answers)
    })
}

fn qualified(score: usize) -> [f64; 4] {
    match score {
        3 => [0.0, 0.0, 0.1, 0.9],
        2 => [0.0, 0.1, 0.8, 0.1],
        _ => [0.0, 0.2, 0.5, 0.3],
    }
}

const UNRELATED: [f64; 4] = [0.9, 0.1, 0.0, 0.0];
const TIED: [f64; 4] = [0.3, 0.2, 0.4, 0.1];

fn policy() -> RecommendationPolicy {
    RecommendationPolicy::default()
}

fn ranked_paths(result: &RecommendationResult) -> Vec<&str> {
    result
        .ranking
        .iter()
        .map(|file| file.path.as_str())
        .collect()
}

// ---------------------------------------------------------------------------------------
// Activation and capture
// ---------------------------------------------------------------------------------------

#[test]
fn activation_applies_only_to_ready_root_requests() {
    assert_eq!(root_activation(true, None, false, false, 3), Ok(()));
    assert_eq!(
        root_activation(true, Some("markdown"), false, false, 3),
        Ok(())
    );
    assert_eq!(
        root_activation(false, None, false, false, 3),
        Err("not_root_scope")
    );
    assert_eq!(
        root_activation(true, Some("llms-txt"), false, false, 3),
        Err("unsupported_format")
    );
    assert_eq!(
        root_activation(true, None, true, false, 3),
        Err("index_warming")
    );
    assert_eq!(
        root_activation(true, None, false, true, 3),
        Err("indexer_dead")
    );
    assert_eq!(
        root_activation(true, None, false, false, 0),
        Err("empty_index")
    );
}

#[test]
fn capture_projects_every_file_once_and_links_calls_by_name() {
    let input = input_from(&catalog());
    assert_eq!(input.snapshot_id(), 7);
    assert_eq!(input.snapshot_file_count(), 3);
    assert_eq!(input.task_query(), TASK);
    let paths: Vec<&str> = input
        .files()
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            "src/search/filter.rs",
            "src/search/rank.rs",
            "src/util/strings.rs"
        ]
    );
    let names: Vec<&str> = input
        .declarations()
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect();
    // `mod` entries and function-local declarations are not candidates; private top-level
    // and test-flagged declarations are (the flag travels as evidence).
    assert_eq!(
        names,
        vec![
            "filter_bodies",
            "Retention",
            "apply",
            "rank",
            "trim_all",
            "helper",
            "test_trim"
        ]
    );
    let test_trim = &input.declarations()[declaration_index(&input, "test_trim")];
    assert!(test_trim.is_test);
    assert!(test_trim.identity_line().contains("[exported, test]"));
    let apply = declaration_index(&input, "apply");
    assert_eq!(
        input.declarations()[apply].qualified_name(),
        "Retention::apply"
    );
    assert_eq!(input.declarations()[apply].end_line, 100);

    let filter_bodies = declaration_index(&input, "filter_bodies");
    let outgoing = input.outgoing_calls(filter_bodies);
    assert_eq!(outgoing.len(), 2);
    assert_eq!(outgoing[0].name, "rank");
    assert_eq!(outgoing[0].target, Some(declaration_index(&input, "rank")));
    // `self.apply` resolves to the unique `apply` callable even across owners when it is
    // the only candidate in the catalog.
    assert_eq!(outgoing[1].target, Some(apply));
    assert_eq!(
        input.outgoing_calls(apply)[0].target,
        Some(declaration_index(&input, "trim_all"))
    );
    assert_eq!(
        input.possible_callers(declaration_index(&input, "rank")),
        &[(filter_bodies, 20)]
    );
    // The call at rank.rs:12 targets the private helper in another file.
    assert_eq!(
        input.possible_callers(declaration_index(&input, "helper")),
        &[(declaration_index(&input, "rank"), 12)]
    );
    assert_eq!(
        input.files()[0].call_names,
        vec!["apply", "rank", "trim_all"]
    );
    assert_eq!(input.files()[0].call_name_count, 3);
    assert_eq!(input.files()[0].docs, vec!["Search body filtering."]);
    assert!(input.files().iter().all(|file| file.has_usable_evidence));
    assert!(input.files()[2]
        .declarations
        .contains(&declaration_index(&input, "helper")));
}

#[test]
fn capture_masks_secret_like_evidence_including_the_intent() {
    let _scope = crate::redact::begin_request();
    let files = vec![file(
        "src/config.rs",
        40,
        vec![with_doc(
            symbol("load", "fn", 1, 20),
            &format!("Loads the key {AWS_LIKE_TOKEN} from the environment."),
        )],
        vec![&format!("Module docs mention {AWS_LIKE_TOKEN} too.")],
        vec![],
    )];
    let intent = format!("where is {AWS_LIKE_TOKEN} validated?");
    let input = RootInput::capture(&intent, 1, &files).unwrap();
    assert!(
        !input.task_query().contains(AWS_LIKE_TOKEN),
        "{}",
        input.task_query()
    );
    assert!(input.task_query().contains("validated"));
    let fragments = input.fragments().unwrap();
    let encoded = serde_json::to_string(&fragments[0].evidence).unwrap();
    assert!(!encoded.contains(AWS_LIKE_TOKEN), "{encoded}");
    assert!(input.declarations()[0]
        .doc
        .as_deref()
        .is_some_and(|doc| doc.contains("Loads the key")));
    let state = serde_json::to_string(&shared_state(&input)).unwrap();
    assert!(!state.contains(AWS_LIKE_TOKEN), "{state}");
}

#[test]
fn capture_requires_explicit_task_intent() {
    assert_eq!(
        RootInput::capture("   ", 1, &catalog()).err(),
        Some("missing_task_query")
    );
}

#[test]
fn fragments_split_large_files_under_the_byte_limit_and_cover_every_declaration() {
    let symbols: Vec<ExtractedSymbol> = (0..200)
        .map(|index| {
            with_doc(
                symbol(
                    &format!("routine_{index:03}"),
                    "fn",
                    index * 5 + 1,
                    index * 5 + 4,
                ),
                &"documentation ".repeat(15),
            )
        })
        .collect();
    let files = vec![file(
        "src/big.rs",
        1_000,
        symbols,
        vec!["File docs."],
        vec![call("x", None, 2)],
    )];
    let input = input_from(&files);
    let fragments = input.fragments().unwrap();
    assert!(
        fragments.len() >= 4,
        "expected several parts, got {}",
        fragments.len()
    );
    let mut seen = Vec::new();
    for (position, fragment) in fragments.iter().enumerate() {
        assert_eq!(fragment.part, position);
        assert_eq!(fragment.id.as_str(), format!("f0p{position}"));
        assert!(!fragment.is_path_only);
        let evidence = &fragment.evidence;
        assert!(
            encoded_len(evidence) <= FRAGMENT_BYTE_LIMIT,
            "part {position} is {} bytes",
            encoded_len(evidence)
        );
        assert_eq!(evidence["part"], position + 1);
        assert_eq!(evidence["parts"], fragments.len());
        assert_eq!(evidence["file_path"], "src/big.rs");
        assert_eq!(evidence["evidence_available"], true);
        if position == 0 {
            assert_eq!(evidence["docs"], serde_json::json!(["File docs."]));
            assert_eq!(evidence["calls"], serde_json::json!(["x"]));
            assert_eq!(evidence["call_name_count"], 1);
        } else {
            assert!(evidence.get("docs").is_none());
            assert!(evidence.get("calls").is_none());
        }
        for line in evidence["declarations"].as_array().unwrap() {
            let line = line.as_str().unwrap();
            assert!(line.contains("(fn) L"), "{line}");
            assert!(line.contains("[exported] — documentation"), "{line}");
            seen.push(line.split(' ').next().unwrap().to_string());
        }
    }
    let expected: Vec<String> = input
        .declarations()
        .iter()
        .map(|declaration| declaration.name.clone())
        .collect();
    assert_eq!(
        seen, expected,
        "every declaration appears exactly once, in order"
    );
}

#[test]
fn oversized_documentation_continues_across_parts_and_impossible_identities_fail() {
    // One declaration whose docstring alone is larger than a fragment: it continues over
    // several parts, each repeating the declaration identity and its position.
    let files = vec![file(
        "src/long_doc.rs",
        50,
        vec![with_doc(
            symbol("documented", "fn", 1, 40),
            &"words ".repeat(4_000),
        )],
        vec![],
        vec![],
    )];
    let input = input_from(&files);
    let fragments = input.fragments().unwrap();
    assert!(fragments.len() >= 3, "{}", fragments.len());
    let mut chunks = Vec::new();
    for fragment in &fragments {
        assert!(encoded_len(&fragment.evidence) <= FRAGMENT_BYTE_LIMIT);
        for line in fragment.evidence["declarations"].as_array().unwrap() {
            let line = line.as_str().unwrap();
            assert!(
                line.starts_with("documented (fn) L1-40 [exported] — (continued "),
                "{line}"
            );
            chunks.push(line.to_string());
        }
    }
    assert!(chunks[0].contains(&format!("(continued 1/{})", chunks.len())));
    assert!(chunks
        .last()
        .unwrap()
        .contains(&format!("(continued {0}/{0})", chunks.len())));
    let rejoined: String = chunks
        .iter()
        .map(|chunk| {
            chunk
                .split_once("(continued ")
                .unwrap()
                .1
                .split_once(") ")
                .unwrap()
                .1
        })
        .collect();
    assert_eq!(rejoined.len(), "words ".repeat(4_000).trim_end().len());

    // An identity that cannot fit a fragment by itself is a projection failure, and the
    // whole recommendation falls back without a request.
    let files = vec![file(
        "src/absurd.rs",
        5,
        vec![symbol(&"n".repeat(FRAGMENT_BYTE_LIMIT + 1), "fn", 1, 2)],
        vec![],
        vec![],
    )];
    let input = input_from(&files);
    assert!(matches!(
        input.fragments(),
        Err(ProjectionError { file_index: 0, .. })
    ));
    let evaluator = scripted(BTreeMap::new(), BTreeMap::new());
    let result = futures_block_on(async { recommend(&input, &evaluator, &policy()).await });
    assert_eq!(
        result.status,
        RecommendationStatus::Fallback("projection_incomplete".into())
    );
    assert_eq!(evaluator.request_count(), 0);
    assert!(result.rendered.unwrap().contains("projection_incomplete"));
}

fn futures_block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[tokio::test]
async fn files_without_usable_evidence_are_sent_as_path_only_and_never_prove_absence() {
    let _scope = crate::redact::begin_request();
    let path_only = file("assets/data.json", 3, vec![], vec![], vec![]);
    let fully_masked = file("src/keys.rs", 9, vec![], vec![AWS_LIKE_TOKEN], vec![]);
    let input = input_from(&[path_only.clone(), fully_masked.clone()]);
    assert!(input.files().iter().all(|file| !file.has_usable_evidence));
    let fragments = input.fragments().unwrap();
    assert_eq!(fragments.len(), 2);
    for fragment in &fragments {
        assert!(fragment.is_path_only);
        assert_eq!(fragment.evidence["evidence_available"], false);
        assert!(fragment.evidence["evidence_note"]
            .as_str()
            .unwrap()
            .contains("only the path and line count"));
    }
    // Nothing usable anywhere: insufficient evidence without a request.
    let evaluator = scripted(BTreeMap::new(), BTreeMap::new());
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::InsufficientEvidence);
    assert_eq!(evaluator.request_count(), 0);
    assert_eq!(result.coverage.path_only_fragments, 2);
    assert_eq!(result.coverage.questions, 0);
    assert!(result.rendered.unwrap().contains("tied or unavailable"));

    // Mixed: the path-only fragment is asked (with its limitation stated) and an unhelpful
    // answer for it does not turn a complete no-match into `no_match`.
    let mut files = catalog();
    files.push(path_only);
    let input = input_from(&files);
    let evaluator = scripted(BTreeMap::new(), BTreeMap::new());
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::InsufficientEvidence);
    assert_eq!(evaluator.request_count(), 1);
    assert_eq!(result.coverage.questions, 4);
    assert_eq!(result.coverage.path_only_fragments, 1);
    let asked = &evaluator.requests()[0].questions;
    let data = asked
        .values()
        .find(|question| question["instructions"]["candidate"]["file_path"] == "assets/data.json")
        .expect("the path-only file is asked");
    assert_eq!(
        data["instructions"]["candidate"]["evidence_available"],
        false
    );
}

// ---------------------------------------------------------------------------------------
// Qualification, status and ranking
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn all_unrelated_evaluation_returns_no_match_without_a_role_stage() {
    let input = input_from(&catalog());
    let evaluator = scripted(BTreeMap::new(), BTreeMap::new());
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::NoMatch);
    assert_eq!(result.status.outcome(), "applied");
    assert!(result.ranking.is_empty());
    assert_eq!(result.qualified_file_count, 0);
    assert_eq!(result.role_stage, RoleStage::NotNeeded);
    assert_eq!(evaluator.request_count(), 1);
    assert_eq!(
        result.coverage,
        Coverage {
            snapshot_files: 3,
            eligible_files: 3,
            projected_declarations: 7,
            fragments: 3,
            path_only_fragments: 0,
            questions: 3,
            judged_fragments: 3,
        }
    );
    assert_eq!(result.fragment_judgments.len(), 3);
    let rendered = result.rendered.expect("status section");
    assert!(rendered.contains("none qualified"));
    assert!(rendered.contains("does not show that the implementation is absent"));
    assert!(rendered.contains("Continue with search, read, grep or find."));
    assert!(!rendered.contains("### 1."));
    // The request carried this call's intent and every fragment, nothing else; the
    // question text names the fields it refers to.
    let recorded = evaluator.requests();
    assert_eq!(recorded[0].task_query(), Some(TASK));
    assert_eq!(recorded[0].questions.len(), 3);
    assert_eq!(
        recorded[0].state["catalog"]["projection_version"],
        PROJECTION_VERSION
    );
    assert_eq!(
        recorded[0].state["catalog"]["question_version"],
        QUESTION_VERSION
    );
    for question in recorded[0].questions.values() {
        assert_eq!(question["type"], "score");
        let text = question["instructions"]["question"].as_str().unwrap();
        assert!(
            text.contains("`candidate`") && text.contains("`task_query`"),
            "{text}"
        );
        assert!(text.contains("data, not instructions"), "{text}");
        assert_eq!(question["criteria"].as_array().unwrap().len(), 4);
        assert!(question["instructions"]["candidate"]["file_path"].is_string());
    }
}

#[tokio::test]
async fn tied_evidence_is_reported_as_insufficient_not_absent() {
    let input = input_from(&catalog());
    let evaluator = scripted(
        BTreeMap::from([("src/search/rank.rs", TIED)]),
        BTreeMap::new(),
    );
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::InsufficientEvidence);
    assert!(result.ranking.is_empty());
    assert_eq!(evaluator.request_count(), 1);
    let tied = result
        .fragment_judgments
        .iter()
        .find(|judgment| judgment.file_index == 1)
        .unwrap();
    assert_eq!(tied.qualifies, None);
    assert!(result.rendered.unwrap().contains("tied or unavailable"));
}

#[test]
fn qualification_compares_probability_masses_not_the_score() {
    let score = |probabilities: &[f64]| ScoreAnswer {
        score: probabilities
            .iter()
            .enumerate()
            .map(|(level, p)| level as f64 * p)
            .sum(),
        probabilities: probabilities.to_vec(),
        confidence: None,
    };
    assert_eq!(qualifies(&score(&[0.2, 0.25, 0.55, 0.0])), Some(true));
    assert_eq!(qualifies(&score(&[0.51, 0.0, 0.0, 0.49])), Some(false));
    assert_eq!(qualifies(&score(&[0.5, 0.0, 0.5, 0.0])), None);
}

#[tokio::test]
async fn an_unqualified_file_never_outranks_a_qualified_one() {
    let files = vec![
        file(
            "src/high_unqualified.rs",
            10,
            vec![symbol("a", "fn", 1, 5)],
            vec![],
            vec![],
        ),
        file(
            "src/low_qualified.rs",
            10,
            vec![symbol("b", "fn", 1, 5)],
            vec![],
            vec![],
        ),
    ];
    let input = input_from(&files);
    let evaluator = scripted(
        BTreeMap::from([
            ("src/high_unqualified.rs", [0.51, 0.0, 0.0, 0.49]), // score 1.47
            ("src/low_qualified.rs", [0.2, 0.25, 0.55, 0.0]),    // score 1.35
        ]),
        BTreeMap::from([("b", ("implementation", 0.2))]),
    );
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(ranked_paths(&result), vec!["src/low_qualified.rs"]);
    assert!((result.ranking[0].max_score - 1.35).abs() < 1e-9);
    assert_eq!(result.ranking[0].role_status, FileRoleStatus::Evaluated);
    assert_eq!(result.ranking[0].declarations[0].role, "implementation");
}

#[tokio::test]
async fn more_than_24_qualified_files_keep_the_best_24_by_score_then_path() {
    let mut files = Vec::new();
    let mut scores = BTreeMap::new();
    let paths: Vec<String> = (0..30).map(|index| format!("src/f{index:02}.rs")).collect();
    for (index, path) in paths.iter().enumerate() {
        files.push(file(
            path,
            20,
            vec![symbol(&format!("fn_{index}"), "fn", 1, 10)],
            vec![],
            vec![],
        ));
    }
    let leaked: Vec<&'static str> = paths
        .iter()
        .map(|path| Box::leak(path.clone().into_boxed_str()) as &'static str)
        .collect();
    for (index, path) in leaked.iter().enumerate().take(26) {
        let level3 = index as f64 * 0.02;
        // Score rises with the index; f24 and f25 share a distribution to exercise the tie.
        let distribution = if index == 24 || index == 25 {
            [0.0, 0.1, 0.9 - 24.0 * 0.02, 24.0 * 0.02]
        } else {
            [0.0, 0.1, 0.9 - level3, level3]
        };
        scores.insert(*path, distribution);
    }
    scores.insert(leaked[26], [0.51, 0.0, 0.0, 0.49]);
    scores.insert(leaked[27], UNRELATED);
    scores.insert(leaked[28], UNRELATED);
    scores.insert(leaked[29], UNRELATED);
    let input = input_from(&files);
    let evaluator = scripted(scores, BTreeMap::new());
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(result.qualified_file_count, 26);
    assert_eq!(result.ranking.len(), MAX_RECOMMENDED_FILES);
    let ranked = ranked_paths(&result);
    assert_eq!(ranked[0], "src/f24.rs");
    assert_eq!(ranked[1], "src/f25.rs");
    assert_eq!(ranked[2], "src/f23.rs");
    assert_eq!(ranked[23], "src/f02.rs");
    assert!(!ranked.contains(&"src/f26.rs"));
    assert!(!ranked.contains(&"src/f00.rs"));
    // Every recommended path and declaration maps to the prepared input; roles were asked
    // for exactly the 24 selected files and none was supported.
    for file in &result.ranking {
        assert_eq!(input.files()[file.file_index].path, file.path);
        assert!(file.declarations.is_empty(), "no roles were scripted");
        assert_eq!(file.role_status, FileRoleStatus::Evaluated);
    }
    assert_eq!(result.role_stage, RoleStage::Applied { questions: 24 });
    assert_eq!(result.role_evaluated_declarations.len(), 24);
    assert_eq!(result.role_candidates_omitted, 0);
    let rendered = result.rendered.unwrap();
    assert!(rendered.contains("26 qualified, showing 24"));
    assert!(rendered.contains("### 24. src/f02.rs"));
}

#[tokio::test]
async fn fragmented_file_qualifies_on_any_part_and_ranks_by_its_maximum() {
    let mut symbols: Vec<ExtractedSymbol> = (0..120)
        .map(|index| {
            with_doc(
                symbol(
                    &format!("routine_{index:03}"),
                    "fn",
                    index * 5 + 1,
                    index * 5 + 4,
                ),
                &"documentation ".repeat(15),
            )
        })
        .collect();
    symbols.push(symbol("last_one", "fn", 900, 905));
    let files = vec![
        file("src/big.rs", 1_000, symbols, vec![], vec![]),
        file(
            "src/small.rs",
            10,
            vec![symbol("s", "fn", 1, 5)],
            vec![],
            vec![],
        ),
    ];
    let input = input_from(&files);
    let part_count = input
        .fragments()
        .unwrap()
        .iter()
        .filter(|fragment| fragment.file_index == 0)
        .count();
    assert!(part_count >= 2);
    let evaluator = MockEvaluator::new(move |request: &EvaluationRequest| {
        let mut answers = BTreeMap::new();
        for question in request.questions() {
            let instructions = question.instructions();
            let answer = if let Some(evidence) = instructions.get("candidate") {
                let is_last_part = evidence["part"] == evidence["parts"];
                if evidence["file_path"] == "src/big.rs" && is_last_part {
                    answers::score(&qualified(3))
                } else if evidence["file_path"] == "src/small.rs" {
                    answers::score(&qualified(2))
                } else {
                    answers::score(&UNRELATED)
                }
            } else {
                answers::choice("unrelated", &negative_distribution("unrelated"))
            };
            answers.insert(question.id().clone(), answer);
        }
        Ok(answers)
    });
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(ranked_paths(&result), vec!["src/big.rs", "src/small.rs"]);
    assert!((result.ranking[0].max_score - 2.9).abs() < 1e-9);
    assert_eq!(result.coverage.fragments, part_count + 1);
    assert_eq!(result.coverage.judged_fragments, part_count + 1);
    // The big file has more candidates than the role bound: the rest is reported.
    assert_eq!(
        result.role_stage,
        RoleStage::Applied {
            questions: MAX_ROLE_CANDIDATES_PER_FILE + 1
        }
    );
    assert_eq!(
        result.role_candidates_omitted,
        121 - MAX_ROLE_CANDIDATES_PER_FILE
    );
}

// ---------------------------------------------------------------------------------------
// Roles and rendering
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn roles_attach_at_most_two_positive_declarations_with_read_windows() {
    let mut files = catalog();
    files[0].symbols.push(symbol("long_tail", "fn", 101, 700));
    files[0].total_lines = 800;
    let input = input_from(&files);
    let evaluator = scripted(
        BTreeMap::from([
            ("src/search/filter.rs", qualified(3)),
            ("src/search/rank.rs", qualified(2)),
        ]),
        BTreeMap::from([
            ("filter_bodies", ("implementation", 0.1)),
            ("apply", ("consumer", 0.3)),
            ("long_tail", ("caller", 0.2)),
            ("Retention", ("contract", 0.05)),
            ("rank", ("unrelated", 0.9)),
        ]),
    );
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(evaluator.request_count(), 2);
    let recorded = evaluator.requests();
    // The role stage names only declarations of the selected files (leaves first), with
    // the question text referring to the named fields.
    let asked: Vec<String> = recorded[1]
        .questions
        .values()
        .map(|question| {
            let text = question["instructions"]["question"].as_str().unwrap();
            assert!(
                text.contains("`declaration`") && text.contains("`task_query`"),
                "{text}"
            );
            assert_eq!(question["criteria"].as_object().unwrap().len(), ROLES.len());
            question["instructions"]["declaration"]["name"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(asked.len(), 4);
    assert!(asked
        .iter()
        .all(|name| ["filter_bodies", "apply", "long_tail", "rank"].contains(&name.as_str())));
    assert!(
        !asked.contains(&"Retention".to_string()),
        "containers are asked only without leaves"
    );
    assert!(
        !asked.contains(&"trim_all".to_string()),
        "unselected files are never asked"
    );
    assert_eq!(
        recorded[1].state["catalog"]["question_version"],
        QUESTION_VERSION
    );

    let filter = &result.ranking[0];
    assert_eq!(filter.path, "src/search/filter.rs");
    assert_eq!(filter.role_status, FileRoleStatus::Evaluated);
    assert_eq!(filter.declarations.len(), MAX_DECLARATIONS_PER_FILE);
    let roles: Vec<(&str, &str)> = filter
        .declarations
        .iter()
        .map(|role| {
            (
                input.declarations()[role.declaration].name.as_str(),
                role.role.as_str(),
            )
        })
        .collect();
    // Ordered by positive mass: implementation 0.9, caller 0.8; consumer 0.7 is dropped.
    assert_eq!(
        roles,
        vec![("filter_bodies", "implementation"), ("long_tail", "caller")]
    );
    assert!((filter.declarations[0].positive_mass - 0.9).abs() < 1e-9);
    assert!((filter.declarations[0].negative_mass - 0.1).abs() < 1e-9);
    let rank = &result.ranking[1];
    assert!(
        rank.declarations.is_empty(),
        "an unrelated-only file stays visible without roles"
    );
    assert_eq!(rank.role_status, FileRoleStatus::Evaluated);

    let rendered = result.rendered.unwrap();
    assert!(rendered.contains("### 1. src/search/filter.rs · relevance 2.90/3"));
    assert!(rendered.contains("- filter_bodies (fn) L10–60 · role: implementation"));
    assert!(rendered.contains("doc: Drops unrelated bodies after ranking."));
    assert!(rendered.contains("calls: rank @L20, self.apply @L30"));
    assert!(rendered.contains("read: read({\"file_path\":\"src/search/filter.rs\",\"limit\":51,\"offset\":10,\"view\":\"source\"})"));
    assert!(rendered.contains("- long_tail (fn) L101–700 · role: caller"));
    assert!(rendered.contains("\"limit\":180,\"offset\":101"));
    assert!(rendered.contains("the declaration continues to L700"));
    assert!(rendered.contains("### 2. src/search/rank.rs"));
    assert!(rendered.contains("No declaration-level role was supported"));
    assert_eq!(result.role_judgments.len(), 4);
    assert_eq!(result.usage.reported_responses, 2);
    assert_eq!(result.timing.request_count, 2);
}

#[test]
fn a_negative_option_tying_with_the_top_probability_suppresses_the_role() {
    let choice = |chosen: &str, pairs: &[(&str, f64)]| ChoiceAnswer {
        choice: chosen.into(),
        probabilities: pairs
            .iter()
            .map(|(option, probability)| ((*option).to_string(), *probability))
            .collect(),
        confidence: None,
    };
    // Clear positive winner.
    let role = representative_role(&choice(
        "implementation",
        &[("implementation", 0.6), ("caller", 0.2), ("unrelated", 0.2)],
    ))
    .expect("positive role");
    assert_eq!(role.role, "implementation");
    assert!((role.positive_mass - 0.8).abs() < 1e-9);
    // A negative option within the tie tolerance of the top: no role is shown.
    assert!(representative_role(&choice(
        "implementation",
        &[
            ("implementation", 0.45),
            ("unrelated", 0.45),
            ("caller", 0.10)
        ],
    ))
    .is_none());
    assert!(representative_role(&choice(
        "caller",
        &[
            ("caller", 0.40),
            ("insufficient_evidence", 0.395),
            ("consumer", 0.205)
        ],
    ))
    .is_none());
    // Negative choices never attach.
    assert!(representative_role(&choice(
        "unrelated",
        &[("unrelated", 0.9), ("implementation", 0.1)],
    ))
    .is_none());
    assert!(representative_role(&choice(
        "insufficient_evidence",
        &[("insufficient_evidence", 0.5), ("implementation", 0.5)],
    ))
    .is_none());
}

#[tokio::test]
async fn role_stage_failure_keeps_the_complete_file_ranking() {
    let input = input_from(&catalog());
    let evaluator = MockEvaluator::new(|request: &EvaluationRequest| {
        let is_role_stage = request
            .questions()
            .iter()
            .any(|question| question.instructions().get("declaration").is_some());
        if is_role_stage {
            return Err(JevError::RateLimited { status: 429 });
        }
        Ok(request
            .questions()
            .iter()
            .map(|question| (question.id().clone(), answers::score(&qualified(2))))
            .collect())
    });
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(result.ranking.len(), 3);
    assert_eq!(
        result.role_stage,
        RoleStage::Fallback("rate_limited".into())
    );
    assert!(result.ranking.iter().all(|file| {
        file.declarations.is_empty()
            && file.role_status == FileRoleStatus::Unavailable("rate_limited".into())
    }));
    assert!(result.role_evaluated_declarations.is_empty());
    let rendered = result.rendered.unwrap();
    assert!(rendered.contains("Declaration roles are unavailable for this call (rate_limited)"));
    assert!(rendered.contains("### 3."));
    assert!(!rendered.contains("No declaration-level role was supported"));
    assert_eq!(evaluator.request_count(), 2, "the role stage was attempted");
}

#[tokio::test]
async fn file_stage_failure_preserves_the_base_overview_with_a_bounded_reason() {
    let input = input_from(&catalog());
    let evaluator = MockEvaluator::failing(JevError::DeadlineExceeded {
        deadline: Duration::from_secs(1),
    });
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(
        result.status,
        RecommendationStatus::Fallback("deadline_exceeded".into())
    );
    assert_eq!(result.status.label(), "fallback:deadline_exceeded");
    assert_eq!(result.status.outcome(), "fallback");
    assert!(result.ranking.is_empty());
    assert!(result.fragment_judgments.is_empty());
    assert_eq!(result.role_stage, RoleStage::NotNeeded);
    assert_eq!(result.coverage.questions, 3);
    assert_eq!(result.coverage.judged_fragments, 0);
    let rendered = result.rendered.unwrap();
    assert!(rendered.contains("Unavailable: the evaluation failed (deadline_exceeded)"));
    assert!(rendered.contains("no ranking was produced"));
    assert_eq!(evaluator.request_count(), 1);
}

#[tokio::test]
async fn output_budget_bypasses_before_sending_or_truncates_whole_entries() {
    let input = input_from(&catalog());
    let scores = BTreeMap::from([
        ("src/search/filter.rs", qualified(3)),
        ("src/search/rank.rs", qualified(2)),
        ("src/util/strings.rs", qualified(1)),
    ]);
    let full = recommend(
        &input,
        &scripted(scores.clone(), BTreeMap::new()),
        &policy(),
    )
    .await;
    let full_text = full.rendered.clone().unwrap();
    assert_eq!(full.rendered_file_count, 3);

    let truncated = recommend(
        &input,
        &scripted(scores.clone(), BTreeMap::new()),
        &RecommendationPolicy {
            output_budget_bytes: Some(full_text.len() - 1),
            ..policy()
        },
    )
    .await;
    assert_eq!(truncated.status, RecommendationStatus::Matched);
    assert_eq!(truncated.rendered_file_count, 2);
    assert_eq!(
        truncated.ranking.len(),
        3,
        "the structured ranking stays complete"
    );
    let text = truncated.rendered.unwrap();
    assert!(text.len() < full_text.len());
    assert!(text.contains("1 further recommended file(s) omitted"));
    assert!(!text.contains("### 3."));

    // Room for a status-only section but not for the matched header: the evaluation runs,
    // the ranking is complete, and nothing is rendered.
    let minimum = minimum_section_bytes(&input);
    let evaluator = scripted(scores.clone(), BTreeMap::new());
    let bypassed = recommend(
        &input,
        &evaluator,
        &RecommendationPolicy {
            output_budget_bytes: Some(minimum),
            ..policy()
        },
    )
    .await;
    assert_eq!(
        bypassed.status,
        RecommendationStatus::Bypassed("insufficient_output_room".into())
    );
    assert!(bypassed.rendered.is_none());
    assert_eq!(bypassed.ranking.len(), 3);
    assert_eq!(evaluator.request_count(), 2);

    // No room for any section at all: nothing is sent.
    let evaluator = scripted(scores, BTreeMap::new());
    let early = recommend(
        &input,
        &evaluator,
        &RecommendationPolicy {
            output_budget_bytes: Some(minimum - 1),
            ..policy()
        },
    )
    .await;
    assert_eq!(
        early.status,
        RecommendationStatus::Bypassed("insufficient_output_room".into())
    );
    assert_eq!(evaluator.request_count(), 0);
    assert!(early.ranking.is_empty());
    assert!(early.rendered.is_none());
}

#[tokio::test]
async fn empty_catalog_is_bypassed_without_a_request() {
    let input = input_from(&[]);
    let evaluator = scripted(BTreeMap::new(), BTreeMap::new());
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(
        result.status,
        RecommendationStatus::Bypassed("empty_index".into())
    );
    assert_eq!(evaluator.request_count(), 0);
}

// ---------------------------------------------------------------------------------------
// Snapshot identity, deadlines, replay
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn a_refreshed_snapshot_during_evaluation_cannot_enter_the_result() {
    let gate = Gate::new();
    let input = Arc::new(input_from(&catalog()));
    let evaluator = Arc::new(
        scripted(
            BTreeMap::from([("src/search/filter.rs", qualified(3))]),
            BTreeMap::new(),
        )
        .with_gate(Arc::clone(&gate)),
    );
    let running = tokio::spawn({
        let input = Arc::clone(&input);
        let evaluator = Arc::clone(&evaluator);
        async move { recommend(&input, &*evaluator, &policy()).await }
    });
    gate.entered().await;
    // A newer generation is published while the request is in flight.
    let mut refreshed = catalog();
    refreshed.push(file(
        "src/search/new_file.rs",
        5,
        vec![symbol("n", "fn", 1, 3)],
        vec![],
        vec![],
    ));
    refreshed[0].file_path = "src/search/renamed.rs".into();
    let _newer = RootInput::capture(TASK, 8, &refreshed).unwrap();
    gate.release();
    let result = running.await.unwrap();
    assert_eq!(result.snapshot_id, 7);
    assert_eq!(ranked_paths(&result), vec!["src/search/filter.rs"]);
    let recorded = evaluator.requests();
    let asked: Vec<String> = recorded[0]
        .questions
        .values()
        .map(|question| {
            question["instructions"]["candidate"]["file_path"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert!(!asked
        .iter()
        .any(|path| path.contains("renamed") || path.contains("new_file")));
}

#[tokio::test(start_paused = true)]
async fn one_absolute_deadline_bounds_both_stages_together() {
    let input = input_from(&catalog());
    let evaluator = scripted(
        BTreeMap::from([("src/search/filter.rs", qualified(3))]),
        BTreeMap::from([("filter_bodies", ("implementation", 0.1))]),
    )
    .with_delay(Duration::from_millis(500));
    let cancel = CancelToken::new();
    let deadline_at = Instant::now() + Duration::from_secs(2);
    let result = recommend(
        &input,
        &evaluator,
        &RecommendationPolicy {
            deadline_at: Some(deadline_at),
            cancel: Some(cancel),
            output_budget_bytes: None,
        },
    )
    .await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    let recorded = evaluator.requests();
    assert_eq!(recorded.len(), 2);
    // Both stages receive the very same instant; nothing re-adds a duration.
    assert_eq!(recorded[0].deadline_at, Some(deadline_at));
    assert_eq!(recorded[1].deadline_at, Some(deadline_at));
    assert_eq!(result.timing.elapsed, Duration::from_secs(1));

    // Too little budget left for the role stage: the ranking stands, roles are skipped and
    // every file says so.
    let slow = scripted(
        BTreeMap::from([("src/search/filter.rs", qualified(3))]),
        BTreeMap::new(),
    )
    .with_delay(Duration::from_millis(1_200));
    let result = recommend(
        &input,
        &slow,
        &RecommendationPolicy {
            deadline_at: Some(Instant::now() + Duration::from_secs(2)),
            ..policy()
        },
    )
    .await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(
        result.role_stage,
        RoleStage::Skipped("deadline_budget_exhausted".into())
    );
    assert_eq!(slow.request_count(), 1);
    assert_eq!(
        result.ranking[0].role_status,
        FileRoleStatus::Unavailable("deadline_budget_exhausted".into())
    );
    assert!(result
        .rendered
        .unwrap()
        .contains("Declaration roles are unavailable for this call (deadline_budget_exhausted)"));

    // A deadline already spent before the role stage: the adapter never builds the second
    // request. (The mock evaluator does not enforce deadlines itself; the real runtime's
    // pre-dispatch refusal for the first stage is covered in `jev::tests`.)
    let spent = scripted(
        BTreeMap::from([("src/search/filter.rs", qualified(3))]),
        BTreeMap::new(),
    );
    let result = recommend(
        &input,
        &spent,
        &RecommendationPolicy {
            deadline_at: Some(Instant::now() - Duration::from_millis(1)),
            ..policy()
        },
    )
    .await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(
        result.role_stage,
        RoleStage::Skipped("deadline_budget_exhausted".into())
    );
    assert_eq!(spent.request_count(), 1);
}

#[tokio::test]
async fn stored_judgments_replay_a_changed_policy_without_inference() {
    let input = input_from(&catalog());
    let fragments = input.fragments().unwrap();
    let evaluator = scripted(
        BTreeMap::from([
            ("src/search/filter.rs", qualified(3)),
            ("src/util/strings.rs", qualified(1)),
        ]),
        BTreeMap::from([
            ("filter_bodies", ("implementation", 0.1)),
            ("apply", ("consumer", 0.3)),
        ]),
    );
    let result = recommend(&input, &evaluator, &policy()).await;
    assert_eq!(evaluator.request_count(), 2);
    // Re-rank from the raw judgments only.
    let ranking = rank_files(&input, &fragments, &result.fragment_judgments).unwrap();
    assert_eq!(ranking.status, RecommendationStatus::Matched);
    assert_eq!(ranking.qualified_file_count, 2);
    assert_eq!(
        ranking
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        vec!["src/search/filter.rs", "src/util/strings.rs"]
    );
    let roles = choose_roles(&input, 0, &result.role_judgments);
    assert_eq!(roles.len(), 2);
    assert_eq!(roles[0].role, "implementation");
    assert_eq!(evaluator.request_count(), 2, "replay made no request");

    // A partial judgment set never produces a ranking.
    let only_high: Vec<FragmentJudgment> = result
        .fragment_judgments
        .iter()
        .filter(|judgment| judgment.file_index == 0)
        .cloned()
        .collect();
    assert_eq!(rank_files(&input, &fragments, &only_high), Err(2));

    // A replayed policy that admits a file outside the evaluated role set gives that file
    // no synthesized role: it is `NotEvaluated`, the others keep their roles.
    let mut replayed = ranking.files.clone();
    replayed.push(RankedFile {
        file_index: 1,
        path: input.files()[1].path.clone(),
        max_score: 0.0,
        declarations: Vec::new(),
        role_status: FileRoleStatus::NotEvaluated,
    });
    attach_roles(
        &input,
        &mut replayed,
        &result.role_judgments,
        &result.role_evaluated_declarations,
    );
    assert_eq!(replayed[0].role_status, FileRoleStatus::Evaluated);
    assert_eq!(replayed[0].declarations.len(), 2);
    assert_eq!(replayed[1].role_status, FileRoleStatus::Evaluated);
    assert_eq!(replayed[2].path, "src/search/rank.rs");
    assert_eq!(replayed[2].role_status, FileRoleStatus::NotEvaluated);
    assert!(replayed[2].declarations.is_empty());
    assert_eq!(evaluator.request_count(), 2);
}
