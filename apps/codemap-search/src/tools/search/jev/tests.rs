//! Offline regressions for the search body filter. File outputs are built through the
//! grouped renderer's own API (rows, body blocks, anchors) and every evaluation runs
//! through `jev::mock::MockEvaluator`; no index, filesystem or network is involved. Fake
//! answers verify the payload and the Rust policy, never the model's accuracy.

use super::super::grouped::SourceBlock;
use super::*;
use crate::jev::mock::{answers, MockEvaluator};
use crate::jev::{CancelToken, EvaluationRequest, JevError, Usage};
use crate::parser::{
    CallSite, CodeRange, ExtractedFile, ExtractedSymbol, NavigationFile, SymbolFlags,
};
use std::collections::BTreeMap;
use std::time::Duration;

#[test]
fn identity_matching_advances_on_unicode_boundaries_after_a_substring_match() {
    let symbol = BlockSymbol {
        name: "증가".into(),
        kind: "fn".into(),
        owner: None,
        start_line: 1,
        end_line: 3,
    };
    assert!(identity_verified(
        "1→ // 증가량 is not the declaration\n2→ function 증가() {\n3→ }",
        &symbol
    ));
    assert!(!identity_verified(
        "1→ // 증가량 is only a longer identifier",
        &symbol
    ));
}

const TASK: &str = "how does the search tool cap output bytes?";
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

fn with_owner(mut symbol: ExtractedSymbol, owner: &str) -> ExtractedSymbol {
    symbol.owner = Some(owner.into());
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

fn indexed(path: &str, symbols: Vec<ExtractedSymbol>, calls: Vec<CallSite>) -> ExtractedFile {
    ExtractedFile {
        file_path: path.into(),
        total_lines: 60,
        symbols,
        literals: Vec::new(),
        docstrings: Vec::new(),
        navigation: Some(NavigationFile {
            calls,
            ..NavigationFile::default()
        }),
    }
}

fn output(path: &str, indexed: Option<&ExtractedFile>) -> FileOutput {
    FileOutput::new(path, 1, indexed, 0, 1 << 20)
}

fn numbered(start: usize, lines: &[&str]) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(offset, line)| format!("{}→ {line}", start + offset))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Push a declaration row plus its body block the way `plan_anchored_symbols` does. The
/// first displayed line names the declaration, as real source does.
fn push_body(
    file: &mut FileOutput,
    symbol: &ExtractedSymbol,
    displayed: (usize, usize),
    is_clipped: bool,
) {
    let lines: Vec<String> = (displayed.0..=displayed.1)
        .map(|line| {
            format!(
                "line {line} of {}: budget.fit(remaining_bytes, footer.len(), limit)",
                symbol.name
            )
        })
        .collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    push_lines(file, symbol, displayed, is_clipped, &lines);
}

fn push_lines(
    file: &mut FileOutput,
    symbol: &ExtractedSymbol,
    displayed: (usize, usize),
    is_clipped: bool,
    lines: &[&str],
) {
    assert!(file.start_symbol(symbol, Some("")));
    let mut body = numbered(displayed.0, lines);
    if is_clipped {
        body.push_str("\n… (truncated)");
    }
    assert!(file.plan_source_for_symbol(&body, "", symbol, displayed, is_clipped));
    file.anchor(
        displayed.0,
        displayed.1.saturating_sub(usize::from(is_clipped)),
    );
}

struct Fixture {
    cap: FileOutput,
    other: FileOutput,
}

/// Two displayed files:
/// - `cap.ts`: `applyCap` (complete, calls `renderTail` at L5), `renderTail` (complete,
///   unresolved call at L12), `helper` (window L24-30 of L22-30, calls `budget.fit()` at
///   L26), class `Budget` (complete, non-callable) with the nested method `Budget::fit`
///   (complete).
/// - `other.ts`: `unrelatedThing` (complete) and `clippedThing` (byte-clipped).
fn fixture() -> Fixture {
    let apply_cap = symbol("applyCap", "function", 1, 8);
    let render_tail = symbol("renderTail", "function", 10, 20);
    let helper = symbol("helper", "function", 22, 30);
    let budget = symbol("Budget", "class", 32, 40);
    let fit = with_owner(symbol("fit", "method", 34, 38), "Budget");
    let cap_index = indexed(
        "src/search/cap.ts",
        vec![
            apply_cap.clone(),
            render_tail.clone(),
            helper.clone(),
            budget.clone(),
            fit.clone(),
        ],
        vec![
            call("renderTail", None, 5),
            call("format", None, 12),
            call("fit", Some("budget"), 26),
        ],
    );
    let mut cap = output("src/search/cap.ts", Some(&cap_index));
    push_body(&mut cap, &apply_cap, (1, 8), false);
    push_body(&mut cap, &render_tail, (10, 20), false);
    push_body(&mut cap, &helper, (24, 30), false);
    push_body(&mut cap, &budget, (32, 40), false);
    push_body(&mut cap, &fit, (34, 38), false);

    let unrelated = symbol("unrelatedThing", "function", 1, 6);
    let clipped = symbol("clippedThing", "function", 8, 40);
    let mut other = output("src/search/other.ts", None);
    push_body(&mut other, &unrelated, (1, 6), false);
    push_body(&mut other, &clipped, (8, 20), true);
    Fixture { cap, other }
}

fn arguments() -> Value {
    json!({ "query": "cap output", "caller_context": false })
}

fn task(goal: &str) -> RegisteredTask {
    RegisteredTask {
        task_query: goal.into(),
        match_mode: MatchMode::All,
        questions: vec![crate::tools::task::TaskQuestion {
            id: "budget".into(),
            question: "Does the candidate implement or support the requested output byte cap?"
                .into(),
            when_true: "Related: supplied facts connect it to output budgeting.".into(),
            when_false:
                "Unrelated: supplied facts establish separate behavior, not merely missing context."
                    .into(),
        }],
    }
}

fn capture(fixture: &Fixture) -> FilterInput {
    FilterInput::capture(&task(TASK), arguments(), &[&fixture.cap, &fixture.other])
}

fn entity<'a>(input: &'a FilterInput, name: &str) -> (usize, &'a FilterEntity) {
    input
        .entities
        .iter()
        .enumerate()
        .find(|(_, entity)| entity.symbol.name == name)
        .unwrap_or_else(|| panic!("entity {name} missing"))
}

fn decision<'a>(
    result: &'a FilterResult,
    input: &FilterInput,
    name: &str,
) -> &'a RetentionDecision {
    let (index, _) = entity(input, name);
    &result.decisions[index]
}

/// A mock that answers each body question from the declaration name.
fn judge(probabilities: &[(&'static str, f64)]) -> MockEvaluator {
    let probabilities: BTreeMap<&'static str, f64> = probabilities.iter().copied().collect();
    MockEvaluator::new(move |request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|question| {
                let name = question.instructions()["candidate"]["name"]
                    .as_str()
                    .unwrap_or_default();
                let unrelated = probabilities.get(name).copied().unwrap_or(0.0);
                (question.id().clone(), answers::noul(1.0 - unrelated))
            })
            .collect())
    })
}

fn judgments(input: &FilterInput, probabilities: &[(&str, f64)]) -> Vec<BodyJudgment> {
    probabilities
        .iter()
        .map(|(name, unrelated)| {
            let (index, _) = entity(input, name);
            BodyJudgment {
                question_id: question_id(index, 0),
                entity: index,
                criterion: 0,
                group: 0,
                noul: NoulAnswer {
                    noul: 1.0 - *unrelated,
                },
            }
        })
        .collect()
}

fn applied_result(input: &FilterInput, decisions: Vec<RetentionDecision>) -> FilterResult {
    let with_body =
        |decision: &&RetentionDecision| input.entities[decision.entity].block_index.is_some();
    FilterResult {
        status: FilterStatus::Applied {
            bodies: input.body_count(),
            judged: 0,
            omitted: decisions
                .iter()
                .filter(with_body)
                .filter(|decision| !decision.is_retained)
                .count(),
            protected: 0,
            linked: 0,
        },
        decisions,
        judgments: Vec::new(),
        effective_threshold: DEFAULT_MIN_UNRELATED_PROBABILITY,
        entity_count: input.entities.len(),
        judgeable_count: input.judgeable().len(),
        evidence: input.evidence_summary(),
        rendered_omissions: 0,
        is_note_inline: false,
        usage: Usage::default(),
        timing: Timing::default(),
        requests: Vec::new(),
        evidence_version: EVIDENCE_VERSION,
        question_version: QUESTION_VERSION,
        policy_version: POLICY_VERSION,
    }
}

#[test]
fn threshold_accepts_only_finite_values_above_one_half_up_to_one() {
    assert!(validate_threshold(DEFAULT_MIN_UNRELATED_PROBABILITY).is_ok());
    assert!(validate_threshold(0.51).is_ok());
    assert!(validate_threshold(1.0).is_ok());
    for invalid in [
        0.5,
        0.0,
        -0.1,
        1.01,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        let error = validate_threshold(invalid).unwrap_err();
        assert!(
            error.contains("(0.5, 1.0]"),
            "threshold {invalid} must be rejected with the range, got {error}"
        );
    }
}

#[test]
fn capture_builds_entities_with_evidence_status_nesting_and_visible_call_links() {
    let fixture = fixture();
    let input = capture(&fixture);
    let names: Vec<&str> = input
        .entities
        .iter()
        .map(|entity| entity.symbol.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "applyCap",
            "renderTail",
            "helper",
            "Budget",
            "fit",
            "unrelatedThing",
            "clippedThing"
        ]
    );
    assert_eq!(input.file_count, 2);
    assert_eq!(input.body_count(), 7);
    assert_eq!(input.task.task_query, TASK);

    let (apply_cap, apply) = entity(&input, "applyCap");
    let (render_tail, render) = entity(&input, "renderTail");
    let (budget, budget_entity) = entity(&input, "Budget");
    let (_, fit) = entity(&input, "fit");
    let (_, helper) = entity(&input, "helper");
    let (_, unrelated) = entity(&input, "unrelatedThing");
    let (_, clipped) = entity(&input, "clippedThing");

    assert_eq!(apply.evidence, EvidenceStatus::Complete);
    assert!(apply.is_judgeable());
    assert!(!apply.is_masked);
    assert_eq!(apply.body.as_deref().unwrap().lines().count(), 8);
    assert!(apply
        .body
        .as_deref()
        .unwrap()
        .starts_with("1→ line 1 of applyCap"));
    assert_eq!(apply.outgoing, vec![render_tail]);
    assert_eq!(render.incoming, vec![apply_cap]);
    assert_eq!(
        render.outgoing,
        Vec::<usize>::new(),
        "unresolved `format` adds no link"
    );

    assert_eq!(helper.evidence, EvidenceStatus::PartialSource);
    assert!(!helper.is_judgeable());
    assert_eq!(helper.displayed, Some((24, 30)));
    // The call at L26 is inside the displayed window, so the link is visible even though
    // the window itself is never judged; `budget.fit()` resolves by unique simple name.
    assert_eq!(helper.outgoing, vec![4]);
    assert_eq!(fit.incoming, vec![2]);

    assert_eq!(budget_entity.evidence, EvidenceStatus::Complete);
    assert!(!budget_entity.is_callable);
    assert!(!budget_entity.is_judgeable());
    assert_eq!(fit.parent, Some(budget));
    assert_eq!(fit.qualified_name(), "Budget::fit");
    assert!(fit.is_judgeable());

    assert_eq!(unrelated.file_index, 1);
    assert_eq!(unrelated.block_index, Some(0));
    assert_eq!(clipped.evidence, EvidenceStatus::PartialSource);
    assert_eq!(clipped.block_index, Some(1));
    assert_eq!(input.judgeable(), vec![apply_cap, render_tail, 5]);
    assert_eq!(
        input.evidence_summary(),
        EvidenceSummary {
            complete: 4,
            partial: 2,
            no_source: 0,
            oversized: 0,
            identity_unverified: 0,
            masked_unavailable: 0,
            non_callable: 1,
        }
    );
}

#[test]
fn identity_is_verified_on_the_displayed_buffer_not_on_line_counts() {
    // The index says `moved` starts at L10 with 5 lines; the displayed buffer has the right
    // number of lines but names another declaration there (the file changed after
    // indexing). It is protected as unverified and never judged.
    let moved = symbol("moved", "function", 10, 14);
    let mut file = output("src/stale.ts", None);
    push_lines(
        &mut file,
        &moved,
        (10, 14),
        false,
        &[
            "export function somethingElse(a: number) {",
            "  return a + 1;",
            "}",
            "",
            "const movedValue = 3;",
        ],
    );
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let entity = &input.entities[0];
    assert_eq!(entity.evidence, EvidenceStatus::IdentityUnverified);
    assert!(!entity.is_judgeable());
    assert!(entity.body.is_none());
    assert_eq!(input.evidence_summary().identity_unverified, 1);
    let decisions = apply_policy(&input, &[], DEFAULT_MIN_UNRELATED_PROBABILITY);
    assert_eq!(
        decisions[0].reason,
        Some(RetentionReason::IncompleteEvidence(
            EvidenceStatus::IdentityUnverified
        ))
    );

    // The name must be a whole identifier within the first lines: `movedValue` is not
    // `moved`, `moved(` is.
    assert!(!identity_verified(
        "10→ const movedValue = 3;",
        &BlockSymbol::from_symbol(&moved)
    ));
    assert!(identity_verified(
        "10→ export const x = 1;\n11→ function moved(a) {",
        &BlockSymbol::from_symbol(&moved)
    ));
    assert!(!identity_verified(
        "10→ a\n11→ b\n12→ c\n13→ function moved() {}",
        &BlockSymbol::from_symbol(&moved)
    ));
    let method = with_owner(symbol("fit", "method", 34, 38), "Budget");
    assert!(identity_verified(
        "34→   fit(remaining: number): number {",
        &BlockSymbol::from_symbol(&method)
    ));
    assert!(!identity_verified(
        "34→   refit(remaining: number): number {",
        &BlockSymbol::from_symbol(&method)
    ));
}

#[test]
fn masking_beyond_the_signature_protects_a_body_while_partial_masking_is_judged() {
    let marker = crate::redact::MARKER;
    let secret = symbol("loadSecret", "function", 1, 4);
    let mut file = output("src/keys.ts", None);
    push_lines(
        &mut file,
        &secret,
        (1, 4),
        false,
        &[
            "function loadSecret() {",
            &format!("  {marker}"),
            &format!("  {marker} {marker}"),
            "}",
        ],
    );
    let partial = symbol("loadPartial", "function", 6, 12);
    push_lines(
        &mut file,
        &partial,
        (6, 12),
        false,
        &[
            "function loadPartial(client: Client, options: ConnectOptions): Connection {",
            &format!("  const key = {marker};"),
            "  const region = options.region ?? defaultRegion(client.profile);",
            "  const endpoint = resolveEndpoint(region, options.endpointOverride);",
            "  const connection = client.connect(key, endpoint, options.timeoutMs);",
            "  return connection;",
            "}",
        ],
    );
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let (secret_index, secret_entity) = entity(&input, "loadSecret");
    let (partial_index, partial_entity) = entity(&input, "loadPartial");
    assert_eq!(secret_entity.evidence, EvidenceStatus::MaskedUnavailable);
    assert!(secret_entity.is_masked);
    assert!(secret_entity.body.is_none());
    assert_eq!(partial_entity.evidence, EvidenceStatus::Complete);
    assert!(partial_entity.is_masked);
    assert_eq!(input.judgeable(), vec![partial_index]);
    assert_eq!(input.evidence_summary().masked_unavailable, 1);
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("loadPartial", 0.95)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert_eq!(
        decisions[secret_index].reason,
        Some(RetentionReason::IncompleteEvidence(
            EvidenceStatus::MaskedUnavailable
        ))
    );
    assert!(!decisions[partial_index].is_retained);
    assert!(is_masked_unavailable(&format!(
        "1→ f() {{\n2→ {marker}\n3→ }}"
    )));
    assert!(!is_masked_unavailable("1→ f() {\n2→ return 1;\n3→ }"));
    assert!(!is_masked_unavailable(&format!(
        "1→ f() {{\n2→ x = {marker}; y = 2\n3→ }}"
    )));
}

#[test]
fn capture_masks_secret_like_paths_and_the_intent_when_redaction_is_active() {
    let _scope = crate::redact::begin_request();
    let leaked = symbol("leaked", "function", 1, 3);
    let path = format!("src/{AWS_LIKE_TOKEN}/client.ts");
    let mut file = output(&path, None);
    push_body(&mut file, &leaked, (1, 3), false);
    let intent = format!("where is {AWS_LIKE_TOKEN} validated?");
    let input = FilterInput::capture(&task(&intent), arguments(), &[&file]);
    assert!(
        !input.entities[0].path.contains(AWS_LIKE_TOKEN),
        "path reached the evidence unmasked: {}",
        input.entities[0].path
    );
    assert!(
        !input.task.task_query.contains(AWS_LIKE_TOKEN),
        "{}",
        input.task.task_query
    );
    assert!(input.task.task_query.contains("validated"));
    let note = omission_note(&input.entities[0]);
    assert!(!note.contains(AWS_LIKE_TOKEN));
    let groups = questions::groups(&input, &input.judgeable(), &FilterPolicy::default()).unwrap();
    let state = serde_json::to_string(groups[0].request.state()).unwrap();
    assert!(!state.contains(AWS_LIKE_TOKEN), "{state}");
}

#[test]
fn policy_omits_bodies_at_or_above_the_threshold_and_keeps_bodies_below_it() {
    let fixture = fixture();
    let input = capture(&fixture);
    for (unrelated, expect_retained) in [
        (0.0, true),
        (0.5, true),
        (0.69, true),
        (0.70, false),
        (0.71, false),
        (1.0, false),
    ] {
        let decisions = apply_policy(
            &input,
            &judgments(&input, &[("unrelatedThing", unrelated)]),
            DEFAULT_MIN_UNRELATED_PROBABILITY,
        );
        let (index, _) = entity(&input, "unrelatedThing");
        let decision = &decisions[index];
        assert_eq!(
            decision.is_retained, expect_retained,
            "unrelated probability {unrelated} at threshold 0.70"
        );
        assert_eq!(
            decision.match_state,
            if unrelated >= 0.70 {
                MatchState::NoMatch
            } else if unrelated <= 0.30 {
                MatchState::Matched
            } else {
                MatchState::Uncertain
            }
        );
        if expect_retained {
            assert_eq!(
                decision.reason,
                Some(if decision.match_state == MatchState::Matched {
                    RetentionReason::JudgedRelated
                } else {
                    RetentionReason::Uncertain
                })
            );
        } else {
            assert_eq!(decision.reason, None);
        }
    }
}

#[test]
fn replaying_raw_judgments_with_another_threshold_changes_decisions_without_inference() {
    let fixture = fixture();
    let input = capture(&fixture);
    // `applyCap` and `renderTail` are linked, so both carry the same answer: neither can
    // rescue the other and the threshold alone decides.
    let judged = judgments(
        &input,
        &[
            ("unrelatedThing", 0.80),
            ("applyCap", 0.80),
            ("renderTail", 0.80),
        ],
    );
    let (unrelated_index, _) = entity(&input, "unrelatedThing");
    let (apply_index, _) = entity(&input, "applyCap");
    let at_seventy = apply_policy(&input, &judged, 0.70);
    assert!(!at_seventy[unrelated_index].is_retained);
    assert!(!at_seventy[apply_index].is_retained);
    let at_ninety = apply_policy(&input, &judged, 0.90);
    assert!(at_ninety[unrelated_index].is_retained);
    assert_eq!(
        at_ninety[unrelated_index].reason,
        Some(RetentionReason::Uncertain)
    );
    assert!(at_ninety[apply_index].is_retained);
    // The same answers replay to the same decisions.
    assert_eq!(apply_policy(&input, &judged, 0.70), at_seventy);
    // Replaying the newly registered composition changes decisions, never probabilities.
    let mut any_input = input.clone();
    any_input.task.match_mode = MatchMode::Any;
    let mut second = any_input.task.questions[0].clone();
    second.id = "support".into();
    any_input.task.questions.push(second);
    let mut any_answers = judged.clone();
    any_answers.push(BodyJudgment {
        question_id: question_id(unrelated_index, 1),
        entity: unrelated_index,
        criterion: 1,
        group: 0,
        noul: NoulAnswer { noul: 0.99 },
    });
    assert_eq!(
        apply_policy(&any_input, &any_answers, 0.70)[unrelated_index].match_state,
        MatchState::Matched
    );
    any_input.task.match_mode = MatchMode::All;
    assert_eq!(
        apply_policy(&any_input, &any_answers, 0.70)[unrelated_index].match_state,
        MatchState::NoMatch
    );
}

#[test]
fn policy_keeps_bodies_linked_to_nested_in_or_containing_retained_blocks() {
    let fixture = fixture();
    let input = capture(&fixture);
    let judged = judgments(
        &input,
        &[
            ("applyCap", 0.10),
            ("renderTail", 0.95),
            ("fit", 0.90),
            ("unrelatedThing", 0.99),
        ],
    );
    let decisions = apply_policy(&input, &judged, DEFAULT_MIN_UNRELATED_PROBABILITY);
    let (apply_cap, _) = entity(&input, "applyCap");
    let (budget, _) = entity(&input, "Budget");
    let by_name = |name: &str| {
        let (index, _) = entity(&input, name);
        &decisions[index]
    };
    assert_eq!(
        by_name("applyCap").reason,
        Some(RetentionReason::JudgedRelated)
    );
    assert_eq!(
        by_name("renderTail").reason,
        Some(RetentionReason::ConnectedToRetained(apply_cap)),
        "a body judged unrelated stays when a retained body visibly calls it"
    );
    assert_eq!(
        by_name("fit").reason,
        Some(RetentionReason::NestedInRetained(budget)),
        "a nested body stays while its enclosing block is displayed"
    );
    assert_eq!(by_name("Budget").reason, Some(RetentionReason::NotCallable));
    assert_eq!(
        by_name("helper").reason,
        Some(RetentionReason::IncompleteEvidence(
            EvidenceStatus::PartialSource
        ))
    );
    assert_eq!(
        by_name("clippedThing").reason,
        Some(RetentionReason::IncompleteEvidence(
            EvidenceStatus::PartialSource
        ))
    );
    assert!(!by_name("unrelatedThing").is_retained);
    assert_eq!(by_name("unrelatedThing").match_state, MatchState::NoMatch);

    // The reverse direction: an outer body judged unrelated keeps its lines when a related
    // declaration displayed inside it would otherwise be lost.
    let outer = symbol("outer", "function", 1, 20);
    let inner = symbol("inner", "function", 5, 9);
    let mut file = output("src/nested_fns.ts", None);
    push_body(&mut file, &outer, (1, 20), false);
    push_body(&mut file, &inner, (5, 9), false);
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let (outer_index, _) = entity(&input, "outer");
    let (inner_index, inner_entity) = entity(&input, "inner");
    assert_eq!(inner_entity.parent, Some(outer_index));
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("outer", 0.95), ("inner", 0.10)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert_eq!(
        decisions[inner_index].reason,
        Some(RetentionReason::JudgedRelated)
    );
    assert_eq!(
        decisions[outer_index].reason,
        Some(RetentionReason::ContainsRetained(inner_index))
    );
    assert!(decisions[outer_index].reason.as_ref().unwrap().is_link());
    // Both unrelated: both go.
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("outer", 0.95), ("inner", 0.95)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert!(!decisions[outer_index].is_retained);
    assert!(!decisions[inner_index].is_retained);
}

#[test]
fn policy_does_not_keep_a_body_through_an_omitted_neighbor_or_a_row_only_parent() {
    let fixture = fixture();
    let input = capture(&fixture);
    // Both ends of the visible link are unrelated: neither rescues the other.
    let judged = judgments(&input, &[("applyCap", 0.90), ("renderTail", 0.90)]);
    let decisions = apply_policy(&input, &judged, DEFAULT_MIN_UNRELATED_PROBABILITY);
    let (apply_cap, _) = entity(&input, "applyCap");
    let (render_tail, _) = entity(&input, "renderTail");
    assert!(!decisions[apply_cap].is_retained);
    assert!(!decisions[render_tail].is_retained);

    // A parent shown only as a row does not cover the child's lines, and a row-only child
    // does not protect its displayed parent.
    let outer = symbol("Outer", "class", 1, 20);
    let inner = with_owner(symbol("inner", "method", 5, 9), "Outer");
    let mut file = output("src/nested.ts", None);
    assert!(file.start_symbol(&outer, Some("")));
    push_body(&mut file, &inner, (5, 9), false);
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let (inner_index, inner_entity) = entity(&input, "inner");
    let (outer_index, outer_entity) = entity(&input, "Outer");
    assert_eq!(outer_entity.evidence, EvidenceStatus::NoSource);
    assert_eq!(inner_entity.parent, Some(outer_index));
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("inner", 0.95)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert!(!decisions[inner_index].is_retained);
    assert_eq!(
        decisions[outer_index].reason,
        Some(RetentionReason::NotCallable)
    );

    let holder = symbol("holder", "function", 1, 12);
    let local = symbol("localHelper", "function", 4, 6);
    let mut file = output("src/holder.ts", None);
    push_body(&mut file, &holder, (1, 12), false);
    assert!(file.start_symbol(&local, Some("")));
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let (holder_index, _) = entity(&input, "holder");
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("holder", 0.95)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert!(
        !decisions[holder_index].is_retained,
        "a row-only nested declaration has no displayed evidence to lose"
    );
}

#[test]
fn oversized_complete_bodies_are_protected_instead_of_judged() {
    let huge = symbol("huge", "function", 1, 3);
    let mut file = output("src/huge.ts", None);
    assert!(file.start_symbol(&huge, Some("")));
    let filler = "x".repeat(MAX_COMPLETE_BODY_BYTES);
    let text = format!("```\n1→ huge {filler}\n2→ b\n3→ c\n```\n");
    assert!(file.plan_source_for_symbol(
        text.trim_start_matches("```\n").trim_end_matches("\n```\n"),
        "",
        &huge,
        (1, 3),
        false
    ));
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    assert_eq!(input.entities[0].evidence, EvidenceStatus::Oversized);
    assert!(input.judgeable().is_empty());
    let decisions = apply_policy(&input, &[], DEFAULT_MIN_UNRELATED_PROBABILITY);
    assert_eq!(
        decisions[0].reason,
        Some(RetentionReason::IncompleteEvidence(
            EvidenceStatus::Oversized
        ))
    );
}

#[tokio::test]
async fn evaluate_sends_one_noul_question_per_complete_callable_body() {
    let fixture = fixture();
    let input = capture(&fixture);
    let evaluator = judge(&[("unrelatedThing", 0.99)]).with_usage(Usage::reported(296, 20));
    let deadline_at = Instant::now() + Duration::from_millis(2_000);
    let policy = FilterPolicy {
        deadline_at: Some(deadline_at),
        ..FilterPolicy::default()
    };
    let result = evaluate(&input, &evaluator, &policy).await;

    let requests = evaluator.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.task_query(), Some(TASK));
    assert_eq!(request.deadline_at, Some(deadline_at));
    assert_eq!(request.state["task_query"], json!(TASK));
    assert_eq!(request.state["search_arguments"], arguments());
    assert_eq!(
        request.state["filter"]["question_version"],
        json!(QUESTION_VERSION)
    );
    assert_eq!(
        request.state["filter"]["evidence_version"],
        json!(EVIDENCE_VERSION)
    );
    let ids: Vec<&str> = request.questions.keys().map(QuestionId::as_str).collect();
    assert_eq!(
        ids,
        ["b0.q0", "b1.q0", "b5.q0"],
        "only complete callable bodies are asked"
    );
    for question in request.questions.values() {
        assert_eq!(question["type"], json!("noul"));
        assert_eq!(
            question["instructions"]["question"],
            input.task.questions[0].question
        );
        assert_eq!(
            question["criteria"]["true"],
            input.task.questions[0].when_true
        );
        assert_eq!(
            question["criteria"]["false"],
            input.task.questions[0].when_false
        );
        assert!(question["instructions"]["candidate"].get("body").is_none());
    }
    let apply_cap = &request.state["candidates"]["b0"];
    assert_eq!(apply_cap["file_path"], json!("src/search/cap.ts"));
    assert_eq!(apply_cap["lines"], json!("L1-L8"));
    assert_eq!(apply_cap["is_masked"], json!(false));
    assert_eq!(apply_cap["calls_displayed"][0]["name"], "renderTail");
    assert_eq!(apply_cap["called_by_displayed"], json!([]));
    assert!(apply_cap["body"].as_str().unwrap().starts_with(
        "1→ line 1 of applyCap: budget.fit(remaining_bytes, footer.len(), limit)\n2→"
    ));
    assert_eq!(
        request.state["candidates"]["b1"]["called_by_displayed"][0]["name"],
        "applyCap"
    );
    assert!(
        request.state["candidates"].get("b4").is_none(),
        "the retained parent already supplies this body"
    );

    assert_eq!(
        result.status,
        FilterStatus::Applied {
            bodies: 7,
            judged: 3,
            omitted: 1,
            protected: 3,
            linked: 1,
        }
    );
    assert_eq!(result.judgments.len(), 3);
    assert_eq!(result.decisions.len(), 7);
    assert_eq!(result.omitted_entities().collect::<Vec<_>>(), vec![5]);
    assert_eq!(
        result.effective_threshold,
        DEFAULT_MIN_UNRELATED_PROBABILITY
    );
    assert_eq!(result.usage, Usage::reported(296, 20));
    assert_eq!(result.timing.request_count, 1);
    assert_eq!(result.requests.len(), 1);
    assert_eq!(result.evidence_version, EVIDENCE_VERSION);
    assert_eq!(result.question_version, QUESTION_VERSION);
    assert_eq!(result.policy_version, POLICY_VERSION);
    assert!(summary_note(&result).contains("1 of 7 planned bodies omitted"));
    assert!(summary_note(&result).contains("3 judged, 3 protected, 0 uncertain retained, 1 kept"));
    assert!(result
        .diagnostic()
        .starts_with("bodies=7 judged=3 omitted=1 rendered_omissions=0"));
}

#[tokio::test]
async fn evaluate_reports_linked_bodies_separately_from_protected_ones() {
    let fixture = fixture();
    let input = capture(&fixture);
    let evaluator = judge(&[
        ("applyCap", 0.10),
        ("renderTail", 0.95),
        ("fit", 0.90),
        ("unrelatedThing", 0.99),
    ]);
    let result = evaluate(&input, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(
        result.status,
        FilterStatus::Applied {
            bodies: 7,
            judged: 3,
            omitted: 1,
            protected: 3,
            linked: 2,
        }
    );
    assert!(decision(&result, &input, "renderTail").is_retained);
    assert!(decision(&result, &input, "fit").is_retained);
    assert!(!decision(&result, &input, "unrelatedThing").is_retained);
}

#[tokio::test]
async fn retention_rewrites_the_results_body_and_drops_omitted_anchors_only() {
    let fixture = fixture();
    let input = capture(&fixture);
    let evaluator = judge(&[("unrelatedThing", 0.99)]);
    let result = evaluate(&input, &evaluator, &FilterPolicy::default()).await;
    let outcome = FilterOutcome::from_result(&input, &result);
    assert!(outcome.replacement_for(0, 0).is_none());
    assert!(outcome.replacement_for(1, 1).is_none());
    assert_eq!(outcome.replacement_count(), 1);
    let note = outcome
        .replacement_for(1, 0)
        .expect("unrelatedThing is omitted");
    assert_eq!(
        note,
        "- _omitted body: L1-6 (function unrelatedThing) did not match the task questions; read src/search/other.ts offset 1 limit 6 to inspect._\n"
    );

    let Fixture { cap, mut other } = fixture;
    let mut untouched = String::new();
    let mut untouched_cap = cap;
    let before = untouched.len();
    untouched_cap.write_primary(&mut untouched, false);
    assert_eq!(untouched.len() - before, untouched_cap.written_len(false));
    assert!(untouched.contains("```\n1→ line 1 of applyCap"));

    assert_eq!(other.anchors().len(), 2);
    let replaced = other.retain_blocks(|block_index, _| outcome.replacement_for(1, block_index));
    assert_eq!(replaced, 1);
    assert_eq!(
        other.anchors(),
        vec![("src/search/other.ts".to_string(), 8, 19)],
        "only the omitted body's anchor is dropped"
    );
    let mut text = String::new();
    let expected_len = other.written_len(true);
    other.write_primary(&mut text, true);
    assert_eq!(text.len(), expected_len);
    assert!(text.contains("- Symbol: unrelatedThing (function) [L1-6]"));
    assert!(text.contains("- Symbol: clippedThing (function) [L8-40]"));
    assert!(!text.contains("line 1 of unrelatedThing"));
    assert!(text.contains(&note));
    assert!(text.contains("8→ line 8 of clippedThing"));
    assert!(text.ends_with(super::super::grouped::PARTIAL_FILE_NOTICE));
    let span = other.source_span.clone().expect("results are present");
    assert!(text[span.clone()].starts_with(&note));
    let first_source = other.first_source_byte.expect("a retained body remains");
    assert!(span.contains(&first_source));
    assert!(text[first_source..].starts_with("8→ line 8 of clippedThing"));
    // Source observations count the retained fence only, never the note.
    let retained_block = other.blocks()[1].rendered_len();
    assert_eq!(other.delivered_source_bytes(usize::MAX), retained_block);
    assert_eq!(other.delivered_source_bytes(span.end), retained_block);
    assert_eq!(other.delivered_source_bytes(span.start + note.len()), 0);
    assert_eq!(
        other.delivered_source_bytes(span.start + note.len() + 10),
        10
    );
    assert_eq!(
        untouched_cap.delivered_source_bytes(usize::MAX),
        untouched_cap.source_span.clone().unwrap().len()
    );
}

#[test]
fn retaining_every_block_keeps_the_output_byte_identical() {
    let Fixture { mut cap, .. } = fixture();
    let mut before = String::new();
    cap.write_primary(&mut before, false);
    let (span_before, first_before) = (cap.source_span.clone(), cap.first_source_byte);
    assert_eq!(cap.retain_blocks(|_, _| None), 0);
    let mut after = String::new();
    cap.write_primary(&mut after, false);
    assert_eq!(after, before);
    assert_eq!(cap.source_span, span_before);
    assert_eq!(cap.first_source_byte, first_before);
    assert_eq!(cap.anchors().len(), 5);
}

#[test]
fn rust_containers_constants_and_unknown_kinds_are_retained_without_judgment() {
    // Rust-shaped rows: an `impl` container with two methods, a `struct` with a displayed
    // body, a row-only `const`, and a kind the policy does not know.
    let container = symbol("Cache", "impl", 10, 40);
    let get = with_owner(symbol("get", "method", 12, 20), "Cache");
    let evict = with_owner(symbol("evict", "method", 22, 39), "Cache");
    let entry = symbol("Entry", "struct", 1, 8);
    let limit = symbol("LIMIT", "const", 42, 42);
    let widget = symbol("gadget", "widget", 44, 50);
    let index = indexed(
        "src/cache.rs",
        vec![
            entry.clone(),
            container.clone(),
            get.clone(),
            evict.clone(),
            limit.clone(),
            widget.clone(),
        ],
        vec![call("evict", Some("self"), 15)],
    );
    let mut file = output("src/cache.rs", Some(&index));
    push_body(&mut file, &entry, (1, 8), false);
    assert!(file.start_symbol(&container, Some("")));
    push_body(&mut file, &get, (12, 20), false);
    push_body(&mut file, &evict, (22, 39), false);
    assert!(file.start_symbol(&limit, Some("")));
    push_body(&mut file, &widget, (44, 50), false);
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let names: Vec<&str> = input
        .entities
        .iter()
        .map(|entity| entity.symbol.name.as_str())
        .collect();
    assert_eq!(names, ["Entry", "Cache", "get", "evict", "LIMIT", "gadget"]);
    let (get_index, get_entity) = entity(&input, "get");
    let (evict_index, evict_entity) = entity(&input, "evict");
    let (cache_index, cache_entity) = entity(&input, "Cache");
    assert_eq!(cache_entity.evidence, EvidenceStatus::NoSource);
    assert_eq!(get_entity.parent, Some(cache_index));
    assert_eq!(
        get_entity.outgoing,
        vec![evict_index],
        "`self.evict()` resolves to the sibling"
    );
    assert_eq!(evict_entity.incoming, vec![get_index]);
    assert_eq!(input.judgeable(), vec![get_index, evict_index]);

    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("get", 1.0), ("evict", 1.0)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    let reason = |name: &str| {
        let (index, _) = entity(&input, name);
        decisions[index].reason.clone()
    };
    assert_eq!(reason("Entry"), Some(RetentionReason::NotCallable));
    assert_eq!(reason("Cache"), Some(RetentionReason::NotCallable));
    assert_eq!(reason("LIMIT"), Some(RetentionReason::NotCallable));
    assert_eq!(
        reason("gadget"),
        Some(RetentionReason::NotCallable),
        "unknown kinds stay visible"
    );
    // The impl row does not display the method lines, so nesting protects nothing here
    // and both methods are omitted on their own forced judgments.
    assert_eq!(reason("get"), None);
    assert_eq!(reason("evict"), None);

    let result = applied_result(&input, decisions);
    let outcome = FilterOutcome::from_result(&input, &result);
    assert_eq!(
        file.retain_blocks(|block_index, _| outcome.replacement_for(0, block_index)),
        2
    );
    let mut text = String::new();
    file.write_primary(&mut text, false);
    assert!(text.contains("- Symbol: Cache (impl) [L10-40]"));
    assert!(text.contains("  - Symbol: get (method) [L12-20]"));
    assert!(text.contains("- Symbol: LIMIT (const) [L42-42]"));
    assert!(text.contains("1→ line 1 of Entry"));
    assert!(text.contains("44→ line 44 of gadget"));
    assert!(!text.contains("line 12 of get"));
    assert!(!text.contains("line 22 of evict"));
    assert!(text.contains(
        "(method Cache::get) did not match the task questions; read src/cache.rs offset 12 limit 9"
    ));
    assert_eq!(
        text.matches("```").count(),
        4,
        "two fences remain, both balanced"
    );
}

#[test]
fn a_body_no_larger_than_its_omission_note_is_never_omitted() {
    let tiny = symbol("tiny", "function", 1, 1);
    let mut file = output("src/tiny.ts", None);
    assert!(file.start_symbol(&tiny, Some("")));
    assert!(file.plan_source_for_symbol("1→ tiny()", "", &tiny, (1, 1), false));
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    assert_eq!(input.entities[0].body_bytes, "```\n1→ tiny()\n```\n".len());
    assert!(
        input.judgeable().is_empty(),
        "an isolated non-shrinkable body is bypassed before evaluation"
    );
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("tiny", 1.0)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert_eq!(decisions[0].reason, Some(RetentionReason::TooSmallToOmit));
    assert!(decisions[0].is_retained);
    assert!(decisions[0].reason.as_ref().unwrap().is_protection());
}

#[test]
fn the_status_line_is_inline_only_when_omissions_freed_the_room() {
    // One small omission (a body only slightly larger than its note) does not free enough
    // room for the status line: the omission note is written, the status line is not.
    let small = symbol("smallish", "function", 1, 2);
    let mut file = output("src/smallish.ts", None);
    push_lines(
        &mut file,
        &small,
        (1, 2),
        false,
        &[
            "export function smallish(input: string): string { return input.trim().toLowerCase().replace(/\\s+/g, ' ').slice(0, 64).padEnd(64, '.'); }",
            "// trailing comment that keeps this body larger than its omission note but not by much",
        ],
    );
    let input = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let decisions = apply_policy(
        &input,
        &judgments(&input, &[("smallish", 0.99)]),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    assert!(!decisions[0].is_retained, "{:?}", decisions[0]);
    let result = applied_result(&input, decisions);
    let outcome = FilterOutcome::from_result(&input, &result);
    assert_eq!(outcome.replacement_count(), 1);
    let freed = input.entities[0].body_bytes - omission_note(&input.entities[0]).len();
    assert!(freed < summary_note(&result).len(), "freed {freed}");
    assert!(outcome.inline_note.is_none());

    // Three omitted bodies free more than the status line needs: it is written inline.
    let fixture = fixture();
    let input = capture(&fixture);
    let decisions = apply_policy(
        &input,
        &judgments(
            &input,
            &[
                ("unrelatedThing", 0.99),
                ("applyCap", 0.99),
                ("renderTail", 0.99),
            ],
        ),
        DEFAULT_MIN_UNRELATED_PROBABILITY,
    );
    let result = applied_result(&input, decisions);
    let outcome = FilterOutcome::from_result(&input, &result);
    assert_eq!(outcome.replacement_count(), 3);
    let note = outcome.inline_note.as_deref().expect("room was freed");
    assert!(note.contains("3 of 7 planned bodies omitted"), "{note}");
    assert!(note.contains(POLICY_VERSION));
    assert!(note.contains("threshold 0.70"));

    // Bypass and fallback never write inline text.
    let bypassed = FilterResult::untouched(
        &input,
        FilterStatus::Bypassed("no_complete_bodies".into()),
        0.70,
        Instant::now(),
        Usage::default(),
        Timing::default(),
        Vec::new(),
    );
    let outcome = FilterOutcome::from_result(&input, &bypassed);
    assert_eq!(outcome.replacement_count(), 0);
    assert!(outcome.inline_note.is_none());
    assert!(summary_note(&bypassed).contains("bypassed (no_complete_bodies)"));
}

#[test]
fn retaining_every_block_clears_the_first_source_offset() {
    let only = symbol("only", "function", 1, 2);
    let mut file = output("src/only.ts", None);
    push_body(&mut file, &only, (1, 2), false);
    assert_eq!(file.retain_blocks(|_, _| Some("- _omitted_\n".into())), 1);
    let mut text = String::new();
    file.write_primary(&mut text, false);
    assert!(file.first_source_byte.is_none());
    assert!(file.source_span.is_some());
    assert!(file.anchors().is_empty());
    assert!(text.ends_with("\n### results\n- _omitted_\n"));
}

#[test]
fn source_blocks_report_completeness_from_the_displayed_range() {
    let block = |displayed: (usize, usize), is_clipped: bool| SourceBlock {
        text: "```\n1→ a\n```\n".into(),
        prefix: String::new(),
        suffix: String::new(),
        source_offset: Some(4),
        symbol: Some(BlockSymbol {
            name: "a".into(),
            kind: "function".into(),
            owner: None,
            start_line: 1,
            end_line: 3,
        }),
        displayed: Some(displayed),
        is_clipped,
        is_note: false,
    };
    assert!(block((1, 3), false).is_complete_body());
    assert!(!block((1, 3), true).is_complete_body());
    assert!(!block((1, 2), false).is_complete_body());
    assert!(!block((2, 3), false).is_complete_body());
    let notice = SourceBlock {
        text: "- source window: L1-2 of L1-3; remaining source omitted.\n```\n1→ a\n2→ b\n```\n"
            .into(),
        ..block((1, 2), false)
    };
    assert_eq!(notice.displayed_source(), "1→ a\n2→ b");
}

#[tokio::test]
async fn whole_call_failure_falls_back_and_keeps_every_body() {
    let fixture = fixture();
    let input = capture(&fixture);
    let evaluator = MockEvaluator::failing(JevError::RateLimited { status: 429 })
        .with_usage(Usage::reported(10, 0));
    let result = evaluate(&input, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(result.status, FilterStatus::Fallback("rate_limited".into()));
    assert!(result.decisions.is_empty());
    assert!(result.judgments.is_empty());
    assert_eq!(evaluator.request_count(), 1);
    let outcome = FilterOutcome::from_result(&input, &result);
    for file_index in 0..2 {
        for block_index in 0..5 {
            assert!(outcome.replacement_for(file_index, block_index).is_none());
        }
    }
    assert!(outcome.inline_note.is_none());
    assert_eq!(result.diagnostic(), "rate_limited");
}

#[tokio::test]
async fn incomplete_or_invalid_answers_fall_back_without_partial_application() {
    let fixture = fixture();
    let input = capture(&fixture);
    // Answers only the first question: the runtime reports the incomplete set as a failure.
    let evaluator = MockEvaluator::new(|request: &EvaluationRequest| {
        let first = request.questions()[0].id().clone();
        Ok(BTreeMap::from([(first, answers::noul(0.99))]))
    });
    let result = evaluate(&input, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(
        result.status,
        FilterStatus::Fallback("incomplete_answers".into())
    );
    assert!(result.decisions.is_empty());

    // A wrongly typed answer is a validation failure with the same fallback contract.
    let evaluator = MockEvaluator::new(|request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|question| (question.id().clone(), answers::score(&[0.5, 0.5])))
            .collect())
    });
    let result = evaluate(&input, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(
        result.status,
        FilterStatus::Fallback("incomplete_answers".into())
    );
    assert!(
        result.decisions.is_empty(),
        "a required typed answer failure restores the whole base output"
    );
}

#[tokio::test]
async fn forced_unrelated_answers_still_keep_protected_bodies() {
    let fixture = fixture();
    let input = capture(&fixture);
    let evaluator = judge(&[
        ("applyCap", 1.0),
        ("renderTail", 1.0),
        ("fit", 1.0),
        ("unrelatedThing", 1.0),
    ]);
    let result = evaluate(&input, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(
        result.status,
        FilterStatus::Applied {
            bodies: 7,
            judged: 3,
            omitted: 3,
            protected: 3,
            linked: 1,
        }
    );
    assert!(decision(&result, &input, "helper").is_retained);
    assert!(decision(&result, &input, "clippedThing").is_retained);
    assert!(decision(&result, &input, "Budget").is_retained);
    assert!(
        decision(&result, &input, "fit").is_retained,
        "nested in the displayed class block"
    );
    assert!(!decision(&result, &input, "applyCap").is_retained);
    assert!(!decision(&result, &input, "renderTail").is_retained);
    assert!(!decision(&result, &input, "unrelatedThing").is_retained);
}

#[tokio::test]
async fn bypasses_never_send_a_request() {
    let fixture = fixture();
    let input = capture(&fixture);
    let evaluator = judge(&[]);

    let blank = FilterInput::capture(&task("   "), arguments(), &[&fixture.cap]);
    let result = evaluate(&blank, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(
        result.status,
        FilterStatus::Bypassed("missing_task_query".into())
    );

    let invalid = FilterPolicy {
        min_unrelated_probability: 0.5,
        ..FilterPolicy::default()
    };
    let result = evaluate(&input, &evaluator, &invalid).await;
    assert_eq!(
        result.status,
        FilterStatus::Bypassed("invalid_threshold".into())
    );
    assert_eq!(result.effective_threshold, 0.5);

    let partial = symbol("partial", "function", 1, 9);
    let mut file = output("src/partial.ts", None);
    push_body(&mut file, &partial, (1, 5), false);
    let no_bodies = FilterInput::capture(&task(TASK), arguments(), &[&file]);
    let result = evaluate(&no_bodies, &evaluator, &FilterPolicy::default()).await;
    assert_eq!(
        result.status,
        FilterStatus::Bypassed("no_complete_bodies".into())
    );
    assert_eq!(result.judgeable_count, 0);
    assert_eq!(result.diagnostic(), "no_complete_bodies");

    assert_eq!(evaluator.request_count(), 0);
    assert!(result.decisions.is_empty());
}

#[tokio::test]
async fn a_cancelled_token_falls_back_before_any_body_is_touched() {
    let fixture = fixture();
    let input = capture(&fixture);
    let cancel = CancelToken::new();
    cancel.cancel();
    let evaluator = judge(&[("unrelatedThing", 0.99)]);
    let policy = FilterPolicy {
        cancel: Some(cancel),
        ..FilterPolicy::default()
    };
    let result = evaluate(&input, &evaluator, &policy).await;
    assert_eq!(result.status, FilterStatus::Fallback("cancelled".into()));
    assert!(result.decisions.is_empty());
}

#[test]
fn summary_note_names_the_policy_version_and_threshold() {
    let fixture = fixture();
    let input = capture(&fixture);
    let mut result = applied_result(&input, Vec::new());
    result.status = FilterStatus::Applied {
        bodies: 7,
        judged: 4,
        omitted: 2,
        protected: 3,
        linked: 1,
    };
    result.effective_threshold = 0.85;
    let note = summary_note(&result);
    assert!(note.contains("search-selection-policy/4-experimental"));
    assert!(note.contains("threshold 0.85"));
    assert!(note.contains("2 of 7 planned bodies omitted"));
    assert!(note.contains("1 kept through direct support or nesting"));
}
