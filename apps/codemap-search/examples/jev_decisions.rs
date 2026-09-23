//! Direct check of the Jev decision runtime, independent of MCP, the index, the workspace
//! and the application config.
//!
//! ```text
//! cargo run --example jev_decisions -- --mock   # default: replay examples/jev_decisions.mock.json offline
//! cargo run --example jev_decisions -- --live   # operator-run: one real request, needs TYPESAFE_API_KEY
//! ```
//!
//! Mock mode never reads credentials or opens a connection. It asserts the fixture answers
//! (Score 2.0, Choice `keep`, Noul 0.9) and the fixture usage, and exits with status 1 on any
//! mismatch. Live mode posts the same request once, prints the typed answers, usage and
//! timing, and asserts nothing about the values: provider answers are not deterministic.

use codemap_search::jev::mock::{MockTransport, ScriptedResponse};
use codemap_search::jev::{
    EvaluationOutcome, EvaluationRequest, Evaluator, EvaluatorConfig, HttpsSettings, JevError,
    JevEvaluator, NoulCriteria, Question, QuestionId, SecretString, TASK_QUERY_FIELD,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::process::ExitCode;
use std::sync::Arc;

const FIXTURE: &str = include_str!("jev_decisions.mock.json");
const API_KEY_ENV: &str = "TYPESAFE_API_KEY";
const RELEVANCE: &str = "relevance";
const BODY: &str = "body";
const HELPS_TASK: &str = "helps_task";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Mock,
    Live,
}

fn parse_mode(args: &[String]) -> Result<Mode, String> {
    match args {
        [] => Ok(Mode::Mock),
        [flag] if flag == "--mock" => Ok(Mode::Mock),
        [flag] if flag == "--live" => Ok(Mode::Live),
        other => Err(format!(
            "unrecognized arguments {other:?}; use --mock (default) or --live"
        )),
    }
}

fn question_id(name: &str) -> QuestionId {
    QuestionId::new(name).expect("example question ids are valid")
}

/// The request both modes send: a caller-built JSON state (the explicit task intent under
/// the adapters' `task_query` field plus a presentation copy of one candidate) and one
/// question of each primitive over that state. The runtime adds nothing to the state.
fn build_request() -> Result<EvaluationRequest, JevError> {
    // The task intent is explicit and local to this call. Adapters receive it from their
    // caller the same way and never read it from anywhere else.
    let task_query = "Where does the search tool decide whether a result keeps its source body?";
    let mut state = Map::new();
    state.insert(TASK_QUERY_FIELD.into(), json!(task_query));
    state.insert(
        "candidate".into(),
        json!({
            "path": "src/tools/search/mod.rs",
            "symbol": "run_inner_with_metadata",
            "kind": "fn",
            "summary": "Runs the BM25 query, groups hits per file and renders numbered source fences before the output cap.",
        }),
    );
    let relevance = Question::score(
        question_id(RELEVANCE),
        json!("Rate how much the `candidate` file helps with the behavior described in `task_query`. Use only the fields of `candidate`."),
        vec![
            json!("No relevant evidence."),
            json!("Weak evidence: shares vocabulary or general utility code."),
            json!("Strong evidence: implements part of the requested behavior."),
            json!("Direct match: the behavior is decided here."),
        ],
    )?;
    let body = Question::choice(
        question_id(BODY),
        json!("Decide whether a reader working on `task_query` should keep the source body of `candidate` in the tool output."),
        BTreeMap::from([
            ("keep".to_string(), json!("Keep the body: it is likely needed.")),
            ("omit".to_string(), json!("Omit the body: the summary is enough.")),
        ]),
    )?;
    let helps_task = Question::noul(
        question_id(HELPS_TASK),
        json!("Would reading `candidate` help complete `task_query`?"),
        Some(NoulCriteria {
            when_true: json!("Reading it moves the task forward."),
            when_false: json!("Reading it would be a detour."),
        }),
    )?;
    EvaluationRequest::new(Value::Object(state), vec![relevance, body, helps_task])
}

struct Expected {
    score: f64,
    choice: String,
    noul: f64,
    input_tokens: u64,
    output_tokens: u64,
}

fn load_fixture() -> Result<(Vec<u8>, Expected), String> {
    let fixture: Value =
        serde_json::from_str(FIXTURE).map_err(|error| format!("fixture is not JSON: {error}"))?;
    let response = serde_json::to_vec(&fixture["response"])
        .map_err(|error| format!("fixture response does not serialize: {error}"))?;
    let expected = &fixture["expected"];
    let number = |key: &str| {
        expected[key]
            .as_f64()
            .ok_or_else(|| format!("fixture expected.{key} must be a number"))
    };
    let count = |key: &str| {
        expected[key]
            .as_u64()
            .ok_or_else(|| format!("fixture expected.{key} must be an integer"))
    };
    let choice = expected["body_choice"]
        .as_str()
        .ok_or("fixture expected.body_choice must be a string")?
        .to_string();
    Ok((
        response,
        Expected {
            score: number("relevance_score")?,
            choice,
            noul: number("helps_task_noul")?,
            input_tokens: count("input_tokens")?,
            output_tokens: count("output_tokens")?,
        },
    ))
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "unknown".to_string(), |value| value.to_string())
}

fn print_outcome(outcome: &EvaluationOutcome) {
    println!("model: {}", outcome.model);
    if let Some(score) = outcome.score(&question_id(RELEVANCE)) {
        let distribution: Vec<String> = score
            .probabilities
            .iter()
            .map(|probability| format!("{probability:.2}"))
            .collect();
        println!(
            "{RELEVANCE} (score): {:.2}  probabilities=[{}]  confidence={}",
            score.score,
            distribution.join(", "),
            score.confidence.map_or_else(
                || "n/a".to_string(),
                |confidence| format!("{confidence:.2}")
            )
        );
    }
    if let Some(choice) = outcome.choice(&question_id(BODY)) {
        println!(
            "{BODY} (choice): {}  P(keep)={:.2} P(omit)={:.2}",
            choice.choice,
            choice.probability("keep"),
            choice.probability("omit")
        );
    }
    if let Some(noul) = outcome.noul(&question_id(HELPS_TASK)) {
        println!("{HELPS_TASK} (noul): {:.2}", noul.noul);
    }
    let usage = outcome.usage;
    println!(
        "usage: input_tokens={} output_tokens={} (responses with usage: {}, without: {})",
        optional(usage.input_tokens),
        optional(usage.output_tokens),
        usage.reported_responses,
        usage.unreported_responses
    );
    println!(
        "timing: elapsed={}ms http={}ms queue_wait={}ms requests={}",
        outcome.timing.elapsed.as_millis(),
        outcome.timing.http.as_millis(),
        outcome.timing.queue_wait.as_millis(),
        outcome.timing.request_count
    );
    for identity in &outcome.requests {
        println!(
            "request {}: {} question(s), {} bytes, sha256={}, http_status={}",
            identity.batch_index,
            identity.question_ids.len(),
            identity.request_bytes,
            identity.request_sha256,
            identity
                .http_status
                .map_or_else(|| "n/a".to_string(), |status| status.to_string())
        );
    }
}

fn check(outcome: &EvaluationOutcome, expected: &Expected) -> Vec<String> {
    let mut failures = Vec::new();
    match outcome.score(&question_id(RELEVANCE)) {
        Some(score) if (score.score - expected.score).abs() < 1e-9 => {}
        Some(score) => failures.push(format!(
            "{RELEVANCE}: score {} != expected {}",
            score.score, expected.score
        )),
        None => failures.push(format!("{RELEVANCE}: missing score answer")),
    }
    match outcome.choice(&question_id(BODY)) {
        Some(choice) if choice.choice == expected.choice => {}
        Some(choice) => failures.push(format!(
            "{BODY}: choice {} != expected {}",
            choice.choice, expected.choice
        )),
        None => failures.push(format!("{BODY}: missing choice answer")),
    }
    match outcome.noul(&question_id(HELPS_TASK)) {
        Some(noul) if (noul.noul - expected.noul).abs() < 1e-9 => {}
        Some(noul) => failures.push(format!(
            "{HELPS_TASK}: noul {} != expected {}",
            noul.noul, expected.noul
        )),
        None => failures.push(format!("{HELPS_TASK}: missing noul answer")),
    }
    if outcome.usage.input_tokens != Some(expected.input_tokens)
        || outcome.usage.output_tokens != Some(expected.output_tokens)
    {
        failures.push(format!(
            "usage: {}/{} != expected {}/{}",
            optional(outcome.usage.input_tokens),
            optional(outcome.usage.output_tokens),
            expected.input_tokens,
            expected.output_tokens
        ));
    }
    if outcome.timing.request_count != 1 {
        failures.push(format!(
            "requests: {} != expected 1",
            outcome.timing.request_count
        ));
    }
    failures
}

async fn run(mode: Mode) -> Result<(), String> {
    let request =
        build_request().map_err(|error| format!("request construction failed: {error}"))?;
    match mode {
        Mode::Mock => {
            let (response, expected) = load_fixture()?;
            let transport = Arc::new(MockTransport::scripted(vec![ScriptedResponse::ok(
                response,
            )]));
            let evaluator = JevEvaluator::new(transport, EvaluatorConfig::default())
                .map_err(|error| format!("evaluator construction failed: {error}"))?;
            println!("mode: mock (fixture replay; no credentials, no network)");
            let outcome = evaluator.evaluate(request).await.map_err(|failure| {
                format!("evaluation failed: {failure} [{}]", failure.error.kind())
            })?;
            print_outcome(&outcome);
            let failures = check(&outcome, &expected);
            if failures.is_empty() {
                println!("checks: passed (score, choice, noul, usage, request count)");
                Ok(())
            } else {
                for failure in &failures {
                    eprintln!("check failed: {failure}");
                }
                Err(format!("{} check(s) failed", failures.len()))
            }
        }
        Mode::Live => {
            let api_key = std::env::var(API_KEY_ENV)
                .ok()
                .filter(|key| !key.trim().is_empty())
                .ok_or_else(|| format!("live mode needs the {API_KEY_ENV} environment variable"))?;
            let evaluator = JevEvaluator::https(
                SecretString::new(api_key),
                EvaluatorConfig::default(),
                HttpsSettings::default(),
            )
            .map_err(|error| format!("evaluator construction failed: {error}"))?;
            println!("mode: live (one request to the provider; answers are printed, not asserted)");
            let outcome = evaluator.evaluate(request).await.map_err(|failure| {
                format!(
                    "evaluation failed: {failure} [{}]; requests attempted: {}",
                    failure.error.kind(),
                    failure.requests.len()
                )
            })?;
            print_outcome(&outcome);
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match parse_mode(&args) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("runtime setup failed: {error}");
            return ExitCode::from(2);
        }
    };
    match runtime.block_on(run(mode)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}
