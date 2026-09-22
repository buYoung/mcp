//! Experimental root-index recommendation. Never reads source or changes a published snapshot.
use super::PreparedOverview;
use crate::jev::{Answer, Evaluation, EvaluationRequest, Evaluator, Policy, Usage};
use crate::parser::{ExtractedFile, ExtractedSymbol};
use serde_json::json;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "jev_tests.rs"]
mod jev_tests;

pub const QUESTION_VERSION: &str = "overview-fragment-v1/role-v1";
pub const POLICY_VERSION: &str = "overview-qualification-v1-experimental";
const MAX_FILES: usize = 24;
const SYMBOLS_PER_FRAGMENT: usize = 20;

pub struct Recommendation {
    pub text: String,
    pub status: &'static str,
    pub usage: Usage,
    pub http_elapsed_ms: u128,
    pub elapsed_ms: u128,
    /// Retain the raw answers and the snapshot identity so a policy can be replayed offline.
    pub score_answers: Option<Evaluation>,
    pub role_answers: Option<Evaluation>,
    pub snapshot_id: usize,
    pub fallback_reason: Option<String>,
}

fn mask(input: &str) -> String { crate::redact::source(input).into_owned() }
fn evidence(symbol: &ExtractedSymbol) -> serde_json::Value {
    json!({"name":mask(&symbol.name), "owner":symbol.owner.as_deref().map(mask),
        "kind":mask(&symbol.kind), "start_line":symbol.range.start_line,
        "end_line":symbol.range.end_line_inclusive(),
        "doc":symbol.docstring.as_deref().map(mask)})
}
fn fragment(file: &ExtractedFile, symbols: &[ExtractedSymbol]) -> serde_json::Value {
    json!({"file_path":mask(&file.file_path),
        "file_docs":file.docstrings.iter().map(|doc| mask(doc)).collect::<Vec<_>>(),
        "declarations":symbols.iter().map(evidence).collect::<Vec<_>>()})
}
fn score_question(fragment: serde_json::Value) -> crate::jev::Question {
    crate::jev::Question::Score {
        instructions: json!({"candidate":fragment,
            "judgment":"Using only `candidate`, how directly does this indexed file fragment support the behavior requested in `task_query`? Treat paths and declarations as navigation evidence, not source proof."}),
        criteria: vec![
            json!("No useful indexed evidence for the task"),
            json!("Tangential background or generic wrapper"),
            json!("Important supporting implementation, configuration, caller or consumer"),
            json!("Direct implementation of the requested behavior"),
        ],
    }
}
fn role_question(file: &ExtractedFile, symbol: &ExtractedSymbol) -> crate::jev::Question {
    crate::jev::Question::Choice {
        instructions: json!({"candidate_file":mask(&file.file_path), "declaration":evidence(symbol),
            "judgment":"Select one representative navigation role for `declaration` in `candidate_file` with respect to `task_query`. The label is not exhaustive proof. Choose unrelated if unsupported."}),
        criteria: BTreeMap::from([
            ("implementation".into(), json!("Direct behavior implementation")),
            ("support".into(), json!("Supporting contract, configuration or failure handling")),
            ("caller".into(), json!("Potential caller or entry point")),
            ("consumer".into(), json!("Potential downstream consumer")),
            ("unrelated".into(), json!("No supported role in the indexed evidence")),
        ]),
    }
}
fn max_score(answer: &Answer) -> Option<(f64, bool, bool)> {
    let Answer::Score { score, probabilities, .. } = answer else { return None };
    let relevant = probabilities.get("2")? + probabilities.get("3")?;
    let other = probabilities.get("0")? + probabilities.get("1")?;
    Some((*score, relevant > other, (relevant - other).abs() <= f64::EPSILON))
}

/// Evaluate all projected root files, then only the selected declarations. An incomplete
/// inference at either stage discards the whole addition; the base overview stays intact.
pub async fn recommend(
    prepared: PreparedOverview, task_query: &str, evaluator: &dyn Evaluator, policy: Policy,
    output_cap: usize,
) -> Recommendation {
    let started = std::time::Instant::now();
    let mut result = Recommendation { text: prepared.text, status: "bypassed", usage: Usage::default(),
        http_elapsed_ms: 0, elapsed_ms: 0, score_answers: None, role_answers: None,
        snapshot_id: prepared.snapshot_id, fallback_reason:None };
    if !prepared.is_eligible || task_query.trim().is_empty() { return result; }
    let mut ids = BTreeMap::new();
    let mut questions = BTreeMap::new();
    for (file_index, file) in prepared.files.iter().enumerate() {
        // Even files without symbols get a path/doc fragment. Every eligible indexed file
        // participates; no BM25, path list, source read or pre-ranking is involved.
        let chunks: Vec<&[ExtractedSymbol]> = if file.symbols.is_empty() {
            vec![&[]]
        } else { file.symbols.chunks(SYMBOLS_PER_FRAGMENT).collect() };
        for (fragment_index, symbols) in chunks.into_iter().enumerate() {
            let id = format!("file-{file_index}-fragment-{fragment_index}");
            questions.insert(id.clone(), score_question(fragment(file, symbols)));
            ids.insert(id, file_index);
        }
    }
    if questions.is_empty() {
        result.status = "insufficient_evidence";
        result.elapsed_ms = started.elapsed().as_millis();
        return result;
    }
    let expected_scores: Vec<String> = questions.keys().cloned().collect();
    let mut policy = policy;
    let score = evaluator.evaluate(EvaluationRequest {
        task_query: mask(task_query), state: json!({"scope":"root indexed metadata"}),
        questions, policy, cancellation: None,
    }).await;
    let score = match score {
        Ok(score) => score,
        Err(error) => {
            result.status = "fallback";
            result.fallback_reason = Some(format!("{:?}",error.kind));
            result.usage = error.usage;
            result.http_elapsed_ms = error.http_elapsed_ms;
            result.elapsed_ms = started.elapsed().as_millis();
            add_note(&mut result.text, output_cap, &format!("Jev recommendation fallback ({:?}); use search/read/grep/find for live evidence.", error.kind));
            return result;
        }
    };
    result.usage = score.usage;
    result.http_elapsed_ms = score.http_elapsed_ms;
    if score.answers.keys().cloned().collect::<Vec<_>>() != expected_scores {
        result.status = "fallback";
        result.fallback_reason = Some("incomplete_scores".into());
        result.score_answers = Some(score);
        result.elapsed_ms = started.elapsed().as_millis();
        return result;
    }
    let mut ranking: Vec<(usize, f64)> = Vec::new();
    let mut is_tied = false;
    for (id, &file_index) in &ids {
        if let Some((value, qualified, tied)) = score.answers.get(id).and_then(max_score) {
            is_tied |= tied;
            if qualified {
                if let Some((_, best)) = ranking.iter_mut().find(|(index, _)| *index == file_index) {
                    *best = (*best).max(value);
                } else { ranking.push((file_index, value)); }
            }
        } else {
            result.status = "fallback";
            result.fallback_reason = Some("incomplete_scores".into());
            result.score_answers = Some(score);
            add_note(&mut result.text, output_cap, "Jev recommendation fallback (incomplete scores); use search/read/grep/find.");
            result.elapsed_ms = started.elapsed().as_millis();
            return result;
        }
    }
    ranking.sort_by(|(a, x), (b, y)| y.total_cmp(x).then_with(|| prepared.files[*a].file_path.cmp(&prepared.files[*b].file_path)));
    ranking.truncate(MAX_FILES);
    if ranking.is_empty() {
        let has_usable_evidence = prepared.files.iter().any(|file| !file.symbols.is_empty() || !file.docstrings.is_empty());
        result.status = if is_tied || !has_usable_evidence { "insufficient_evidence" } else { "no_match" };
        add_note(&mut result.text, output_cap,
            &format!("Jev recommendation_status={}; indexed evidence did not establish a recommendation. Continue with search/read/grep/find.", result.status));
        result.score_answers = Some(score);
        result.elapsed_ms = started.elapsed().as_millis();
        return result;
    }
    // The second stage is dependent on the selected files; no unused declarations are sent.
    let mut roles = BTreeMap::new();
    let mut role_ids = BTreeMap::new();
    for (file_index, _) in &ranking {
        for (symbol_index, symbol) in prepared.files[*file_index].symbols.iter().enumerate() {
            let id = format!("role-{file_index}-{symbol_index}");
            roles.insert(id.clone(), role_question(&prepared.files[*file_index], symbol));
            role_ids.insert(id, (*file_index, symbol_index));
        }
    }
    let role_result = if roles.is_empty() { None } else {
        let expected_roles: Vec<String> = roles.keys().cloned().collect();
        policy.deadline = policy.deadline.saturating_sub(started.elapsed());
        let answer = evaluator.evaluate(EvaluationRequest { task_query: mask(task_query),
            state: json!({"scope":"selected indexed declarations"}), questions: roles,
            policy, cancellation: None }).await;
        match answer {
            Ok(answer) => {
                if answer.answers.keys().cloned().collect::<Vec<_>>() != expected_roles
                    || answer.answers.values().any(|value| !matches!(value, Answer::Choice { choice, probabilities, .. }
                        if probabilities.contains_key(choice) && probabilities.contains_key("unrelated"))) {
                    result.status = "fallback";
                    result.fallback_reason = Some("invalid_roles".into());
                    result.score_answers = Some(score);
                    result.role_answers = Some(answer);
                    result.elapsed_ms = started.elapsed().as_millis();
                    return result;
                }
                result.usage.input_tokens += answer.usage.input_tokens;
                result.usage.output_tokens += answer.usage.output_tokens;
                result.http_elapsed_ms += answer.http_elapsed_ms;
                Some(answer)
            }
            Err(error) => {
                result.status = "fallback";
                result.fallback_reason = Some(format!("{:?}",error.kind));
                result.score_answers = Some(score);
                result.usage.input_tokens += error.usage.input_tokens;
                result.usage.output_tokens += error.usage.output_tokens;
                result.http_elapsed_ms += error.http_elapsed_ms;
                add_note(&mut result.text, output_cap, &format!("Jev recommendation fallback ({:?}); use search/read/grep/find.", error.kind));
                result.elapsed_ms = started.elapsed().as_millis();
                return result;
            }
        }
    };
    let mut addition = format!("\n\n## Jev indexed recommendations\n\nrecommendation_status=matched; policy={POLICY_VERSION}; question={QUESTION_VERSION}. Indexed navigation hints only; verify with read.\n");
    for (file_index, value) in &ranking {
        let file = &prepared.files[*file_index];
        let mut row = format!("- `{}` (indexed score {:.2})\n", mask(&file.file_path), value);
        let mut selected: Vec<(usize, &str, f64)> = role_result.iter().flat_map(|answer| answer.answers.iter())
            .filter_map(|(id, answer)| {
                let &(index, symbol_index) = role_ids.get(id)?;
                if index != *file_index { return None; }
                let Answer::Choice { choice, probabilities, .. } = answer else { return None };
                (choice != "unrelated" && probabilities.get(choice)? > probabilities.get("unrelated")?)
                    .then_some((symbol_index, choice.as_str(), *probabilities.get(choice)?))
            }).collect();
        selected.sort_by(|a,b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
        for (symbol_index, role, _) in selected.into_iter().take(2) {
            let symbol = &file.symbols[symbol_index];
            let start = symbol.range.start_line;
            let end = symbol.range.end_line_inclusive();
            let read = json!({"file_path":file.file_path,"offset":start,
                "limit":end.saturating_sub(start).saturating_add(1).min(180)});
            row.push_str(&format!("  - {role}: {} ({}) [L{start}-{end}]; read {read}\n",
                mask(&symbol.name), mask(&symbol.kind)));
            if let Some(doc) = &symbol.docstring {
                row.push_str(&format!("    - indexed doc: {}\n", mask(doc).chars().take(200).collect::<String>()));
            }
            if let Some(navigation) = &file.navigation {
                let calls: Vec<_> = navigation.calls.iter()
                    .filter(|call| start <= call.range.start_line && call.range.start_line <= end)
                    .take(3).map(|call| mask(&call.name)).collect();
                if !calls.is_empty() {
                    row.push_str(&format!("    - possible indexed calls: {}\n", calls.join(", ")));
                }
                let callers: Vec<_> = navigation.calls.iter()
                    .filter(|call| call.name == symbol.name)
                    .take(3).map(|call| format!("L{}", call.range.start_line)).collect();
                if !callers.is_empty() {
                    row.push_str(&format!("    - possible same-file callers: {}\n", callers.join(", ")));
                }
            }
        }
        if result.text.len() + addition.len() + row.len() > output_cap {
            result.status = "fallback";
            result.fallback_reason = Some("output_budget".into());
            result.score_answers = Some(score);
            result.role_answers = role_result;
            add_note(&mut result.text, output_cap, "Jev recommendation fallback (output budget); narrow the task or raise the overview cap.");
            result.elapsed_ms = started.elapsed().as_millis();
            return result;
        }
        addition.push_str(&row);
    }
    result.text.push_str(&addition);
    result.status = "applied";
    result.score_answers = Some(score);
    result.role_answers = role_result;
    result.elapsed_ms = started.elapsed().as_millis();
    result
}
fn add_note(text: &mut String, cap: usize, note: &str) {
    let addition = format!("\n\n[{note}]");
    if text.len() + addition.len() <= cap { text.push_str(&addition); }
}
