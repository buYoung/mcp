//! Offline Jev answers for the MCP end-to-end tests.
//!
//! This transport is compiled only into debug builds and is selected only by the
//! `CODEMAP_TEST_JEV_SCRIPT` environment variable when the server starts, so neither an MCP
//! request nor a config file can select it and release builds cannot answer from a script.
//! The real evaluator still validates, batches, budgets, and times every request; only the
//! HTTPS exchange is replaced. The script is a JSON file re-read for every request, so a test
//! can change answers or delays while the server runs:
//!
//! ```json
//! {"delay_ms": 0, "status": 200, "record_path": "/abs/requests.jsonl",
//!  "rules": [{"type": "noul", "contains": "format_size", "noul": 0.8},
//!            {"type": "score", "probabilities": [1, 0, 0, 0]},
//!            {"type": "choice", "choice": "entry"}]}
//! ```
//!
//! Each question takes the answer of the first rule whose `type` matches and whose optional
//! `contains` text occurs in the question's JSON. A question without a matching rule gets no
//! answer, which the evaluator rejects like a provider omission. A `status` other than 200
//! answers every request with that status. `record_path` receives one JSON line per request.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::jev::{BoxFuture, JevTransport, TransportError, TransportErrorKind, TransportResponse};

const SCRIPT_ENV: &str = "CODEMAP_TEST_JEV_SCRIPT";

pub(super) struct ScriptedTransport {
    script_path: PathBuf,
}

impl ScriptedTransport {
    pub(super) fn from_environment() -> Option<Self> {
        let script_path = PathBuf::from(std::env::var_os(SCRIPT_ENV)?);
        tracing::warn!("{SCRIPT_ENV} is set: Jev requests receive scripted offline answers");
        Some(Self { script_path })
    }

    fn script(&self) -> Result<Script, String> {
        let text = std::fs::read_to_string(&self.script_path)
            .map_err(|error| format!("cannot read the Jev test script: {error}"))?;
        serde_json::from_str(&text).map_err(|error| format!("invalid Jev test script: {error}"))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Script {
    #[serde(default)]
    delay_ms: u64,
    #[serde(default = "success_status")]
    status: u16,
    #[serde(default)]
    record_path: Option<PathBuf>,
    #[serde(default)]
    rules: Vec<Rule>,
}

fn success_status() -> u16 {
    200
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    #[serde(rename = "type")]
    question_type: String,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default)]
    noul: Option<f64>,
    /// Score level probabilities, lowest level first.
    #[serde(default)]
    probabilities: Option<Vec<f64>>,
    #[serde(default)]
    choice: Option<String>,
}

impl Rule {
    fn matches(&self, question_type: &str, question_json: &str) -> bool {
        self.question_type == question_type
            && self
                .contains
                .as_deref()
                .is_none_or(|text| question_json.contains(text))
    }

    fn answer(&self, question: &Value) -> Value {
        match self.question_type.as_str() {
            "noul" => json!({"type": "noul", "noul": self.noul.unwrap_or(0.0)}),
            "score" => {
                let probabilities = self.probabilities.clone().unwrap_or_default();
                let score: f64 = probabilities
                    .iter()
                    .enumerate()
                    .map(|(level, probability)| level as f64 * probability)
                    .sum();
                let confidence = probabilities.iter().copied().fold(0.0, f64::max);
                let probabilities: Map<String, Value> = probabilities
                    .iter()
                    .enumerate()
                    .map(|(level, probability)| (level.to_string(), json!(probability)))
                    .collect();
                json!({"type": "score", "score": score, "probabilities": probabilities, "confidence": confidence})
            }
            _ => {
                let choice = self.choice.clone().unwrap_or_default();
                let probabilities: Map<String, Value> = question["criteria"]
                    .as_object()
                    .into_iter()
                    .flat_map(|options| options.keys())
                    .map(|option| {
                        (
                            option.clone(),
                            json!(if *option == choice { 1.0 } else { 0.0 }),
                        )
                    })
                    .collect();
                json!({"type": "choice", "choice": choice, "probabilities": probabilities, "confidence": 1.0})
            }
        }
    }
}

impl JevTransport for ScriptedTransport {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            let failure = |detail: String| TransportError {
                kind: TransportErrorKind::Other,
                detail,
            };
            let script = self.script().map_err(failure)?;
            let request: Value = serde_json::from_slice(&body)
                .map_err(|error| failure(format!("request body is not JSON: {error}")))?;
            if let Some(record_path) = &script.record_path {
                let mut line = serde_json::to_vec(&request).expect("JSON values serialize");
                line.push(b'\n');
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(record_path)
                    .and_then(|mut file| file.write_all(&line))
                    .map_err(|error| failure(format!("cannot record the request: {error}")))?;
            }
            tokio::time::sleep(Duration::from_millis(script.delay_ms)).await;
            if script.status != 200 {
                return Ok(TransportResponse {
                    status: script.status,
                    body: b"{}".to_vec(),
                });
            }
            let mut answers = Map::new();
            for (question_id, question) in request["questions"].as_object().into_iter().flatten() {
                let question_type = question["type"].as_str().unwrap_or_default();
                let question_json = question.to_string();
                if let Some(rule) = script
                    .rules
                    .iter()
                    .find(|rule| rule.matches(question_type, &question_json))
                {
                    answers.insert(question_id.clone(), rule.answer(question));
                }
            }
            let response = json!({
                "model": request["model"],
                "answers": answers,
                "usage": {"input_tokens": body.len() / 4, "output_tokens": 8 * question_count(&request)},
            });
            Ok(TransportResponse {
                status: 200,
                body: serde_json::to_vec(&response).expect("JSON values serialize"),
            })
        })
    }
}

/// Deterministic output usage: a fixed amount per question in the request.
fn question_count(request: &Value) -> usize {
    request["questions"].as_object().map_or(0, Map::len)
}
