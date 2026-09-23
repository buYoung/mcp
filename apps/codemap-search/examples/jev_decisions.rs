//! Calls the codemap-search Jev runtime directly: no MCP server, index, workspace, or
//! application config.
//!
//! `--mock` (the default) answers from a fixed mixed-response fixture without credentials
//! or network access, and exits nonzero unless it reads Score=2.0, Choice=keep, Noul=0.9,
//! and the fixture usage. `--live` sends the same questions to TypeSafe with the key in
//! `TYPESAFE_API_KEY`; it is a billed, operator-run check and never part of CI.
use std::process::ExitCode;

use codemap_search::jev::{
    ApiKey, BoxFuture, ChoiceOption, Evaluation, EvaluationRequest, Evaluator, JevEvaluator,
    JevTransport, NoulCriteria, Question, TransportError, TransportPolicy, TransportResponse,
    Usage,
};
use serde_json::{json, Value};

const SCORE_ID: &str = "upload_retry_relevance";
const CHOICE_ID: &str = "upload_retry_visibility";
const NOUL_ID: &str = "upload_retry_unrelated";
const FIXTURE_USAGE: Usage = Usage {
    input_tokens: 412,
    output_tokens: 36,
};
const FIXTURE: &str = r#"{
  "model": "jev-1.13.0",
  "answers": {
    "upload_retry_relevance": {
      "type": "score",
      "score": 2.0,
      "legend": {"0": "No useful evidence.", "1": "Tangential background.", "2": "Important supporting evidence.", "3": "Directly implements the requested behavior."},
      "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9, "3": 0.05},
      "confidence": 0.84
    },
    "upload_retry_visibility": {
      "type": "choice",
      "choice": "keep",
      "probabilities": {"keep": 0.8, "omit": 0.2},
      "confidence": 0.62
    },
    "upload_retry_unrelated": {"type": "noul", "noul": 0.9}
  },
  "usage": {"input_tokens": 412, "output_tokens": 36}
}"#;

/// Returns the fixture for every request; the evaluator still validates it against the
/// questions it sent.
struct FixtureTransport;

impl JevTransport for FixtureTransport {
    fn post(&self, _body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async {
            Ok(TransportResponse {
                status: 200,
                body: FIXTURE.as_bytes().to_vec(),
            })
        })
    }
}

fn state() -> Value {
    json!({
        "task_query": "Where does the client retry a failed upload?",
        "declaration": {
            "path": "src/upload.rs",
            "signature": "fn retry_upload(attempts: u32) -> Result<(), UploadError>",
            "doc": "Retries a failed upload with exponential backoff."
        }
    })
}

fn questions() -> Vec<Question> {
    let score = Question::score(
        SCORE_ID,
        json!("How useful is `declaration` for answering `task_query`?"),
        vec![
            json!("No useful evidence."),
            json!("Tangential background."),
            json!("Important supporting evidence."),
            json!("Directly implements the requested behavior."),
        ],
    );
    let choice = Question::choice(
        CHOICE_ID,
        json!("Should `declaration` stay visible while answering `task_query`?"),
        vec![
            ChoiceOption::new(
                "keep",
                json!("It is evidence for the task, or its relevance is uncertain."),
            ),
            ChoiceOption::new("omit", json!("It is clearly unrelated to the task.")),
        ],
    );
    let noul = Question::noul(
        NOUL_ID,
        json!("Is `declaration` unrelated to `task_query`?"),
        Some(NoulCriteria::new(
            Some(json!(
                "The declaration plays no part in the requested behavior."
            )),
            Some(json!(
                "The declaration is related, or its relevance is uncertain."
            )),
        )),
    );
    [score, choice, noul]
        .into_iter()
        .map(|question| question.expect("example questions are valid"))
        .collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None | Some("--mock") => run_mock().await,
        Some("--live") => run_live().await,
        Some(_) => {
            eprintln!("usage: jev_decisions [--mock | --live]");
            ExitCode::from(2)
        }
    }
}

async fn run_mock() -> ExitCode {
    let evaluator = Evaluator::new(FixtureTransport, TransportPolicy::default())
        .expect("the default policy is valid");
    let evaluation = match evaluator
        .evaluate(EvaluationRequest::new(state(), questions()))
        .await
    {
        Ok(evaluation) => evaluation,
        Err(failure) => {
            eprintln!(
                "mock evaluation failed ({}): {failure}",
                failure.error.label()
            );
            return ExitCode::FAILURE;
        }
    };
    print_evaluation(&evaluation);
    let mismatches = fixture_mismatches(&evaluation);
    if mismatches.is_empty() {
        println!("mock check passed: Score=2.0, Choice=keep, Noul=0.9, fixture usage");
        ExitCode::SUCCESS
    } else {
        for mismatch in mismatches {
            eprintln!("mismatch: {mismatch}");
        }
        ExitCode::FAILURE
    }
}

fn fixture_mismatches(evaluation: &Evaluation) -> Vec<String> {
    let mut mismatches = Vec::new();
    match evaluation.score(SCORE_ID) {
        Some(answer) if (answer.score - 2.0).abs() < 1e-9 => {}
        other => mismatches.push(format!("Score expected 2.0, got {other:?}")),
    }
    match evaluation.choice(CHOICE_ID) {
        Some(answer) if answer.choice == "keep" => {}
        other => mismatches.push(format!("Choice expected keep, got {other:?}")),
    }
    match evaluation.noul(NOUL_ID) {
        Some(answer) if (answer.noul - 0.9).abs() < 1e-9 => {}
        other => mismatches.push(format!("Noul expected 0.9, got {other:?}")),
    }
    if evaluation.usage != FIXTURE_USAGE {
        mismatches.push(format!(
            "usage expected {FIXTURE_USAGE:?}, got {:?}",
            evaluation.usage
        ));
    }
    mismatches
}

async fn run_live() -> ExitCode {
    let Ok(raw_key) = std::env::var("TYPESAFE_API_KEY") else {
        eprintln!("--live needs TYPESAFE_API_KEY in the environment");
        return ExitCode::from(2);
    };
    let evaluator = match ApiKey::new(raw_key)
        .and_then(|api_key| Evaluator::https(&api_key, TransportPolicy::default()))
    {
        Ok(evaluator) => evaluator,
        Err(error) => {
            eprintln!("live setup failed ({}): {error}", error.label());
            return ExitCode::FAILURE;
        }
    };
    match evaluator
        .evaluate(EvaluationRequest::new(state(), questions()))
        .await
    {
        Ok(evaluation) => {
            print_evaluation(&evaluation);
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!(
                "live evaluation failed ({}): {failure}; usage input={} output={}",
                failure.error.label(),
                failure.usage.input_tokens,
                failure.usage.output_tokens
            );
            ExitCode::FAILURE
        }
    }
}

fn print_evaluation(evaluation: &Evaluation) {
    println!("model: {}", evaluation.model);
    for (question_id, answer) in &evaluation.answers {
        println!("{question_id}: {answer:?}");
    }
    println!(
        "usage: input_tokens={} output_tokens={}",
        evaluation.usage.input_tokens, evaluation.usage.output_tokens
    );
    println!(
        "timing: elapsed_ms={} http_ms={} requests={}",
        evaluation.timing.elapsed.as_millis(),
        evaluation.timing.http_elapsed.as_millis(),
        evaluation.requests.len()
    );
}
