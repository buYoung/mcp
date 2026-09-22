use super::*;
use crate::jev::{DecisionFuture, MODEL};
use crate::parser::{CodeRange, ExtractedFile, SymbolFlags};
use std::sync::atomic::{AtomicUsize, Ordering};

fn symbol(name: &str, kind: &str, start: usize, end: usize) -> ExtractedSymbol {
    ExtractedSymbol {
        name: name.into(),
        kind: kind.into(),
        range: CodeRange {
            start_line: start,
            start_col: 1,
            end_line: end,
            end_col: 2,
        },
        owner: None,
        docstring: None,
        flags: SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: false,
            is_deprecated: false,
        },
    }
}

fn output(inputs: &[(&str, ExtractedSymbol, &str, Completeness)]) -> super::super::SearchOutput {
    let mut files = Vec::new();
    let mut text = String::from("# codemap-search\n");
    for (path, symbol, body, completeness) in inputs {
        let indexed = ExtractedFile {
            file_path: (*path).into(),
            total_lines: 20,
            symbols: vec![symbol.clone()],
            literals: vec![],
            docstrings: vec![],
            navigation: None,
        };
        let mut file = super::super::grouped::FileOutput::new(
            path,
            files.len() + 1,
            Some(&indexed),
            text.len(),
            100_000,
        );
        assert!(file.start_symbol(symbol, Some(body)));
        if !body.is_empty() {
            assert!(file.push_body(
                symbol,
                &format!("```\n{body}\n```\n"),
                4,
                body,
                *completeness == Completeness::Complete
            ));
        }
        // Policy fixtures supply the source verification state. E2E exercises actual digest checks.
        file.bodies.last_mut().unwrap().completeness = *completeness;
        file.write_primary(&mut text, false);
        files.push(file);
    }
    let prepared = PreparedEvidence::capture(&files, &text, usize::MAX, &[]);
    let source_files = files
        .iter()
        .filter_map(|file| {
            file.source_span
                .as_ref()
                .map(|span| crate::analyze::FileObservation {
                    path: file.path.clone(),
                    result_bytes: span.len() as u64,
                })
        })
        .collect();
    super::super::SearchOutput {
        text,
        source_files,
        prepared: Some(prepared),
    }
}

fn answers(prepared: &PreparedEvidence, probability: f64) -> BTreeMap<String, Answer> {
    prepared
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, body)| body.completeness == Completeness::Complete && body.span.is_some())
        .map(|(index, _)| {
            (
                format!("body{index:08}"),
                Answer::Noul { noul: probability },
            )
        })
        .collect()
}

#[test]
fn test_probability_boundary_and_threshold_replay() {
    let base = output(&[(
        "one.rs",
        symbol("sample", "function", 1, 3),
        "1→fn sample() {\n2→    42;\n3→}",
        Completeness::Complete,
    )]);
    let prepared = base.prepared.as_ref().unwrap();
    for probability in [0.0, 0.5, 0.69, 0.70, 0.71, 1.0] {
        let policy = retain(prepared, &answers(prepared, probability), 0.70).unwrap();
        assert_eq!(policy.keep[0], probability < 0.70);
    }
    let raw = answers(prepared, 0.80);
    assert!(!retain(prepared, &raw, 0.70).unwrap().keep[0]);
    assert!(retain(prepared, &raw, 0.90).unwrap().keep[0]);
    for invalid in [0.5, 0.0, 1.1, f64::NAN, f64::INFINITY] {
        assert!(retain(prepared, &raw, invalid).is_err());
    }
}

#[test]
fn test_partial_missing_oversized_unknown_and_rust_structure_are_protected() {
    let entries = [
        (
            "partial.rs",
            symbol("partial", "function", 1, 8),
            "1→fn partial() {",
            Completeness::Partial,
        ),
        (
            "missing.rs",
            symbol("missing", "function", 1, 3),
            "",
            Completeness::Missing,
        ),
        (
            "large.rs",
            symbol("large", "function", 1, 3),
            "1→fn large() { 42; }",
            Completeness::Oversized,
        ),
        (
            "unknown.rs",
            symbol("unknown", "future_kind", 1, 1),
            "1→unknown value",
            Completeness::Complete,
        ),
        (
            "record.rs",
            symbol("Record", "struct", 1, 3),
            "1→struct Record {\n2→ value: i32\n3→}",
            Completeness::Complete,
        ),
        (
            "impl.rs",
            symbol("Record", "impl", 1, 3),
            "1→impl Record {\n2→ fn method(){}\n3→}",
            Completeness::Complete,
        ),
        (
            "constant.rs",
            symbol("LIMIT", "constant", 1, 1),
            "1→const LIMIT: usize = 42;",
            Completeness::Complete,
        ),
    ];
    let base = output(&entries);
    let prepared = base.prepared.as_ref().unwrap();
    let policy = retain(prepared, &answers(prepared, 1.0), 0.70).unwrap();
    assert_eq!(policy.keep, vec![true; 7]);
    let rendered = render(&base, &policy, 100_000).unwrap();
    assert_eq!(rendered.text, base.text);
}

#[test]
fn test_displayed_dependency_closure_and_ambiguous_links() {
    let base = output(&[
        (
            "alpha.rs",
            symbol("alpha", "function", 1, 3),
            "1→fn alpha(){\n2→ beta();\n3→}",
            Completeness::Partial,
        ),
        (
            "beta.rs",
            symbol("beta", "function", 1, 3),
            "1→fn beta(){\n2→ gamma();\n3→}",
            Completeness::Complete,
        ),
        (
            "gamma.rs",
            symbol("gamma", "function", 1, 3),
            "1→fn gamma(){\n2→ 42;\n3→}",
            Completeness::Complete,
        ),
        (
            "other.rs",
            symbol("unrelated", "function", 1, 3),
            "1→fn unrelated(){\n2→ 17;\n3→}",
            Completeness::Complete,
        ),
    ]);
    let prepared = base.prepared.as_ref().unwrap();
    let policy = retain(prepared, &answers(prepared, 1.0), 0.70).unwrap();
    assert_eq!(policy.keep, vec![true, true, true, false]);
    assert_eq!(policy.reasons[2], RetentionReason::ConnectedEvidence);
    let duplicate = output(&[
        (
            "a.rs",
            symbol("same", "function", 1, 3),
            "1→fn same(){\n2→ 42;\n3→}",
            Completeness::Complete,
        ),
        (
            "b.rs",
            symbol("same", "function", 1, 3),
            "1→fn same(){\n2→ 17;\n3→}",
            Completeness::Complete,
        ),
    ]);
    let prepared = duplicate.prepared.as_ref().unwrap();
    assert!(retain(prepared, &answers(prepared, 1.0), 0.70)
        .unwrap()
        .keep
        .iter()
        .all(|keep| *keep));
}

#[test]
fn test_typescript_nested_method_and_enclosing_source_survive() {
    let mut base = output(&[
        (
            "nested.ts",
            symbol("Widget", "class", 1, 9),
            "1→class Widget {",
            Completeness::Partial,
        ),
        (
            "nested.ts",
            symbol("method", "method", 3, 5),
            "3→method() {\n4→ return 42;\n5→}",
            Completeness::Complete,
        ),
    ]);
    let prepared = base.prepared.as_mut().unwrap();
    prepared.bodies[0].has_other_declarations = true;
    let policy = retain(prepared, &answers(prepared, 1.0), 0.70).unwrap();
    assert_eq!(policy.keep, vec![true, true]);
}

#[test]
fn test_render_preserves_file_declarations_fences_and_post_filter_observations() {
    let base = output(&[
        (
            "keep.rs",
            symbol("kept", "function", 1, 3),
            "1→fn kept(){\n2→ 42;\n3→}",
            Completeness::Partial,
        ),
        (
            "omit.rs",
            symbol("omitted", "function", 5, 7),
            "5→fn omitted(){\n6→ 17;\n7→}",
            Completeness::Complete,
        ),
    ]);
    let prepared = base.prepared.as_ref().unwrap();
    let policy = retain(prepared, &answers(prepared, 1.0), 0.70).unwrap();
    let result = render(&base, &policy, 100_000).unwrap();
    assert!(result.text.contains("## 1. keep.rs"));
    assert!(result.text.contains("## 2. omit.rs"));
    assert!(result.text.contains("omitted (function) [L5-7]"));
    assert!(!result.text.contains("6→ 17"));
    assert!(result.text.contains("2→ 42"));
    assert_eq!(result.text.matches("```\n").count(), 2);
    assert_eq!(result.source_files.len(), 1);
    assert_eq!(result.source_files[0].path, "keep.rs");
    assert!(render(&base, &policy, 8).is_err());
    let keep = retain(prepared, &answers(prepared, 0.0), 0.70).unwrap();
    let roundtrip = render(&base, &keep, 100_000).unwrap();
    assert_eq!(roundtrip.text, base.text);
    assert_eq!(roundtrip.source_files.len(), base.source_files.len());
    assert!(roundtrip
        .source_files
        .iter()
        .zip(&base.source_files)
        .all(|(left, right)| left.path == right.path && left.result_bytes == right.result_bytes));
}

#[test]
fn test_malformed_or_incomplete_judgments_fail_whole_call() {
    let base = output(&[(
        "a.rs",
        symbol("body", "function", 1, 3),
        "1→fn body(){\n2→ 42;\n3→}",
        Completeness::Complete,
    )]);
    let prepared = base.prepared.as_ref().unwrap();
    assert!(retain(prepared, &BTreeMap::new(), 0.70).is_err());
    for probability in [-0.1, 1.1, f64::NAN] {
        assert!(retain(prepared, &answers(prepared, probability), 0.70).is_err());
    }
}

struct Fake {
    calls: AtomicUsize,
}
impl Evaluator for Fake {
    fn evaluate(
        &self,
        request: EvaluationRequest,
        _options: EvaluationOptions,
    ) -> DecisionFuture<'_, Result<Evaluation, Failure>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.state["task_query"], "Original intent");
            assert_eq!(request.state["search_arguments"]["query"], "lookup");
            assert_eq!(request.questions.len(), 1);
            for question in request.questions.values() {
                let Question::Noul { instructions, .. } = question else {
                    panic!("expected Noul")
                };
                assert_eq!(instructions["declaration"]["file_path"], "a.rs");
                assert!(instructions["displayed_body"]
                    .as_str()
                    .unwrap()
                    .contains("2→ 42"));
            }
            Ok(Evaluation {
                request_id: request.request_id,
                model: MODEL.into(),
                answers: request
                    .questions
                    .keys()
                    .map(|id| (id.clone(), Answer::Noul { noul: 0.8 }))
                    .collect(),
                metrics: Metrics::default(),
            })
        })
    }
}

#[tokio::test]
async fn test_only_complete_selected_evidence_is_sent_and_raw_policy_replays() {
    let base = output(&[
        (
            "a.rs",
            symbol("body", "function", 1, 3),
            "1→fn body(){\n2→ 42;\n3→}",
            Completeness::Complete,
        ),
        (
            "b.rs",
            symbol("partial", "function", 1, 3),
            "1→fn partial(){",
            Completeness::Partial,
        ),
    ]);
    let prepared = base.prepared.as_ref().unwrap();
    let fake = Fake {
        calls: AtomicUsize::new(0),
    };
    let result = evaluate(
        prepared,
        "Original intent",
        json!({"query":"lookup"}),
        0.70,
        &fake,
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert!(!result.retention.keep[0]);
    assert!(result.retention.keep[1]);
    assert!(
        retain(prepared, &result.raw.unwrap().answers, 0.90)
            .unwrap()
            .keep[0]
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}
