//! Standalone, offline-by-default use of the shared Jev evaluator.
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use codemap_search::jev::{
    Answer, BatchEvaluator, Cancellation, Evaluator, HttpTransport, JevError, Policy, Question,
    Transport, MODEL,
};
use serde_json::{json, Value};

struct MockTransport;

impl Transport for MockTransport {
    fn post<'a>(
        &'a self,
        request: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<Value, JevError>> + Send + 'a>> {
        Box::pin(async move {
            let wire: Value =
                serde_json::from_slice(&request).map_err(|_| JevError::InvalidRequest)?;
            let answers = wire["questions"]
                .as_object()
                .ok_or(JevError::InvalidRequest)?
                .iter()
                .map(|(id, question)| {
                    let answer = match question["type"].as_str() {
                        Some("score") => json!({
                            "type":"score","score":2.0,
                            "legend":{"0":"none","1":"weak","2":"useful","3":"direct"},
                            "probabilities":{"0":0.0,"1":0.0,"2":1.0,"3":0.0},
                            "confidence":1.0
                        }),
                        Some("choice") => json!({
                            "type":"choice","choice":"keep",
                            "probabilities":{"keep":1.0,"omit":0.0},
                            "confidence":1.0
                        }),
                        Some("noul") => json!({"type":"noul","noul":0.9}),
                        _ => return Err(JevError::InvalidRequest),
                    };
                    Ok((id.clone(), answer))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok(json!({
                "model":MODEL,"answers":answers,
                "usage":{"input_tokens":123,"output_tokens":9}
            }))
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let is_live = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [mode] if mode == "--mock" => false,
        [mode] if mode == "--live" => true,
        _ => return Err("Use --mock (default) or --live".into()),
    };
    let transport: Arc<dyn Transport> = if is_live {
        let api_key = std::env::var("TYPESAFE_API_KEY")
            .map_err(|_| "TYPESAFE_API_KEY must be set for --live")?;
        Arc::new(
            HttpTransport::new(api_key, Duration::from_secs(30)).map_err(|error| error.label())?,
        )
    } else {
        Arc::new(MockTransport)
    };
    let evaluator = BatchEvaluator::new(transport);
    let questions = BTreeMap::from([
        (
            "score".into(),
            Question::Score {
                instructions: json!("Rate the relevance of `candidate` to `task_query`."),
                criteria: vec![
                    json!("none"),
                    json!("weak"),
                    json!("useful"),
                    json!("direct"),
                ],
            },
        ),
        (
            "choice".into(),
            Question::Choice {
                instructions: json!("Choose whether `candidate` is useful for `task_query`."),
                criteria: BTreeMap::from([
                    ("keep".into(), json!("Useful")),
                    ("omit".into(), json!("Unrelated")),
                ]),
            },
        ),
        (
            "noul".into(),
            Question::Noul {
                instructions: json!("Is `candidate` unrelated to `task_query`?"),
                criteria: None,
            },
        ),
    ]);
    let result = evaluator
        .evaluate(
            codemap_search::jev::Request {
                state: json!({"task_query":"Find the route","candidate":"The route handler"}),
                questions,
            },
            Policy::default(),
            Cancellation::new(),
        )
        .await
        .map_err(|error| error.label())?;
    if !is_live {
        if !matches!(result.answers.get("score"), Some(Answer::Score { score, .. }) if *score == 2.0)
            || !matches!(result.answers.get("choice"), Some(Answer::Choice { choice, .. }) if choice == "keep")
            || !matches!(result.answers.get("noul"), Some(Answer::Noul { probability }) if *probability == 0.9)
            || result.usage.input_tokens != 123
            || result.usage.output_tokens != 9
        {
            return Err("Mock answer or usage did not match the fixture".into());
        }
    }
    println!(
        "model={} questions={} input_tokens={} output_tokens={} elapsed_ms={}",
        result.model,
        result.answers.len(),
        result.usage.input_tokens,
        result.usage.output_tokens,
        result.elapsed.as_millis()
    );
    Ok(())
}
