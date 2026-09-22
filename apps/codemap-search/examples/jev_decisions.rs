//! Independent Jev caller. `--mock` is the default; `--live` explicitly opts into paid HTTPS.
use codemap_search::jev::{Answer, EvaluationRequest, Evaluator, HttpsTransport, JevEvaluator, NoulCriteria, Policy, Question, Transport, TransportResponse, Usage};
use serde_json::{json, Value};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc, time::Duration};

struct MockTransport;
impl Transport for MockTransport {
    fn post<'a>(&'a self, _body: Vec<u8>, _timeout: Duration) -> Pin<Box<dyn Future<Output = Result<TransportResponse, codemap_search::jev::FailureKind>> + Send + 'a>> {
        Box::pin(async {
            Ok(TransportResponse { status: 200, body: json!({
                "model":"jev-1.13.0", "usage":{"input_tokens":42,"output_tokens":8},
                "answers":{
                    "score":{"type":"score","score":2.0,"confidence":1.0,"legend":{"0":"none","1":"some","2":"direct"},"probabilities":{"0":0.0,"1":0.0,"2":1.0}},
                    "choice":{"type":"choice","choice":"keep","confidence":1.0,"probabilities":{"keep":1.0,"omit":0.0}},
                    "noul":{"type":"noul","noul":0.9}
                }
            }).to_string().into_bytes() })
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let is_live = match std::env::args().nth(1).as_deref() {
        None | Some("--mock") => false,
        Some("--live") => true,
        _ => return Err("usage: jev_decisions [--mock|--live]".into()),
    };
    let transport: Arc<dyn Transport> = if is_live {
        let key = std::env::var("TYPESAFE_API_KEY").map_err(|_| "set TYPESAFE_API_KEY before --live")?;
        Arc::new(HttpsTransport::new(key, Policy::default().pool_idle_timeout).map_err(|_| "HTTPS setup failed")?)
    } else { Arc::new(MockTransport) };
    let mut questions = BTreeMap::new();
    questions.insert("score".into(), Question::Score {
        instructions: Value::String("How directly does `evidence` implement `task_query`?".into()),
        criteria: vec![json!("none"), json!("some"), json!("direct")],
    });
    questions.insert("choice".into(), Question::Choice {
        instructions: json!("Should `evidence` be kept for `task_query`?"),
        criteria: BTreeMap::from([("keep".into(), json!("relevant")), ("omit".into(), json!("unrelated"))]),
    });
    questions.insert("noul".into(), Question::Noul {
        instructions: json!("Is `evidence` related to `task_query`?"),
        criteria: Some(NoulCriteria { yes: json!("related"), no: json!("unrelated") }),
    });
    let result = JevEvaluator::new(transport, 3).evaluate(EvaluationRequest {
        task_query: "Find the customer support dispatch".into(),
        state: json!({"example":"support dispatch"}), questions,
        policy: Policy::default(), cancellation: None,
    }).await.map_err(|error| format!("evaluation failed: {:?}", error.kind))?;
    if !is_live {
        if !matches!(result.answers.get("score"), Some(Answer::Score { score, .. }) if *score == 2.0)
            || !matches!(result.answers.get("choice"), Some(Answer::Choice { choice, .. }) if choice == "keep")
            || !matches!(result.answers.get("noul"), Some(Answer::Noul { noul }) if *noul == 0.9)
            || result.usage != (Usage { input_tokens: 42, output_tokens: 8 }) {
            return Err("mock response did not match the expected three answers and usage".into());
        }
    }
    println!("Jev {}: {} answers, input={}, output={}, elapsed={}ms", if is_live { "live" } else { "mock" }, result.answers.len(), result.usage.input_tokens, result.usage.output_tokens, result.elapsed_ms);
    Ok(())
}
