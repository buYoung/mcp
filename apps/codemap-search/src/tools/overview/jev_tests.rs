use super::*;
use crate::jev::{Evaluation, Failure, Usage};
use serde_json::Value;
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::{Arc, atomic::{AtomicUsize, Ordering}}};

struct Fake { calls: AtomicUsize, questions: AtomicUsize, tied: bool }
impl Evaluator for Fake {
    fn evaluate<'a>(&'a self, request: EvaluationRequest) -> Pin<Box<dyn Future<Output=Result<Evaluation,Failure>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1,Ordering::SeqCst);
            self.questions.fetch_add(request.questions.len(),Ordering::SeqCst);
            let answers = request.questions.into_iter().map(|(id,question)| {
                let answer = match question {
                    crate::jev::Question::Score { instructions, .. } => {
                        let path = instructions["candidate"]["file_path"].as_str().unwrap();
                        let p = if self.tied { [0.25,0.25,0.25,0.25] }
                            else if path.contains("fit") { [0.0,0.0,0.0,1.0] }
                            else { [1.0,0.0,0.0,0.0] };
                        Answer::Score { score: p.iter().enumerate().map(|(i,v)| i as f64 * v).sum(),
                            legend:BTreeMap::new(), probabilities:p.iter().enumerate().map(|(i,v)| (i.to_string(),*v)).collect(), confidence:0.5 }
                    }
                    _ => Answer::Choice {choice:"implementation".into(), probabilities:BTreeMap::from([("implementation".into(),1.0),("unrelated".into(),0.0)]), confidence:1.0},
                };
                (id,answer)
            }).collect();
            Ok(Evaluation {model:crate::jev::MODEL.into(),answers,usage:Usage {input_tokens:10,output_tokens:2},request_ids:vec!["fake".into()],http_elapsed_ms:1,elapsed_ms:2})
        })
    }
}
fn file(path:&str) -> ExtractedFile {
    ExtractedFile { file_path:path.into(),total_lines:10,symbols:vec![ExtractedSymbol {name:"handler".into(),kind:"fn".into(),range:crate::parser::CodeRange { start_line:1,start_col:1,end_line:4,end_col:2 },docstring:None,owner:None,flags:crate::parser::SymbolFlags {has_todo:false,has_fixme:false,is_test:false,is_exported:false,is_deprecated:false}}],literals:vec![],docstrings:vec![],navigation:None }
}
fn prepared(files: Vec<ExtractedFile>) -> PreparedOverview {
    PreparedOverview {text:"# Root".into(),files,snapshot_id:123,is_eligible:true}
}
#[tokio::test]
async fn test_jev_all_candidates_qualified_before_limit_and_snapshot() {
    let fake = Arc::new(Fake {calls:AtomicUsize::new(0),questions:AtomicUsize::new(0),tied:false});
    let mut files = vec![file("none.rs");30];
    for (i,file) in files.iter_mut().enumerate() {file.file_path = if i >= 25 {format!("fit-{i}.rs")} else {format!("none-{i}.rs")};}
    let output = recommend(prepared(files), "find handler",fake.as_ref(),Policy::default(),100_000).await;
    assert_eq!(output.snapshot_id,123);
    assert_eq!(fake.calls.load(Ordering::SeqCst),2);
    assert_eq!(fake.questions.load(Ordering::SeqCst),35);
    assert_eq!(output.status,"applied");
    assert_eq!(output.text.matches("- `fit-").count(),5);
    assert!(!output.text.contains("none-0.rs"));
    assert!(output.text.contains("\"limit\":4"));
}
#[tokio::test]
async fn test_jev_no_match_and_tie_skip_roles() {
    for (tied,status) in [(false,"no_match"),(true,"insufficient_evidence")] {
        let fake = Fake {calls:AtomicUsize::new(0),questions:AtomicUsize::new(0),tied};
        let output = recommend(prepared(vec![file("none.rs")]),"task",&fake,Policy::default(),1024).await;
        assert_eq!(fake.calls.load(Ordering::SeqCst),1);
        assert_eq!(output.status,status);
        assert!(!output.text.contains("- `none.rs`"));
    }
}
#[tokio::test]
async fn test_jev_budget_restores_base_with_reason() {
    let fake = Fake {calls:AtomicUsize::new(0),questions:AtomicUsize::new(0),tied:false};
    let output = recommend(prepared(vec![file("fit.rs")]),"find handler",&fake,Policy::default(),120).await;
    assert_eq!(output.status,"fallback");
    assert_eq!(output.fallback_reason.as_deref(),Some("output_budget"));
    assert!(!output.text.contains("## Jev indexed recommendations"));
    assert!(output.text.starts_with("# Root"));
}

#[test]
fn test_jev_score_qualification_is_distribution_based() {
    let answer = Answer::Score {score:2.7,legend:BTreeMap::new(),probabilities:BTreeMap::from([("0".into(),0.1),("1".into(),0.1),("2".into(),0.1),("3".into(),0.7)]),confidence:0.2};
    assert!(max_score(&answer).unwrap().1);
    let _ = Value::Null;
}
