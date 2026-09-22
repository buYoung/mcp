use super::*;
use crate::jev::{Failure, Evaluation};
use crate::parser::{CodeRange, SymbolFlags};
use std::{future::Future, pin::Pin, sync::atomic::{AtomicUsize, Ordering}};

struct Fake { calls:AtomicUsize, probability:f64 }
impl Evaluator for Fake {
    fn evaluate<'a>(&'a self, request: EvaluationRequest) -> Pin<Box<dyn Future<Output=Result<Evaluation,Failure>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1,Ordering::SeqCst);
            let answers = request.questions.keys().map(|id| (id.clone(), Answer::Noul {noul:self.probability})).collect();
            Ok(Evaluation {model:crate::jev::MODEL.into(),answers,usage:Usage {input_tokens:5,output_tokens:1},request_ids:vec!["mock".into()],http_elapsed_ms:1,elapsed_ms:2})
        })
    }
}
fn symbol(name:&str) -> ExtractedSymbol {
    ExtractedSymbol {name:name.into(),kind:"fn".into(),range:CodeRange {start_line:1,start_col:1,end_line:1,end_col:20},owner:None,docstring:None,
        flags:SymbolFlags {has_todo:false,has_fixme:false,is_test:false,is_exported:false,is_deprecated:false} }
}
fn output() -> SearchOutput {
    let mut text = String::new();
    let mut files = Vec::new();
    let mut evidence = Vec::new();
    let mut source_files = Vec::new();
    for (file_index, (path, name, is_protected)) in [("a.rs","alpha",false),("b.rs","beta",true)].into_iter().enumerate() {
        let mut file = super::super::grouped::FileOutput::new(path,file_index+1,None,text.len(),10_000,true);
        let declaration = symbol(name);
        assert!(file.start_symbol(&declaration,None));
        let body = format!("1→ fn {name}() {{}}");
        let block = format!("```\n{body}\n```\n");
        assert!(file.push_source_segment(&block,4,&body,true));
        let span = file.source_segments[0].range.clone();
        file.write_primary(&mut text,false);
        source_files.push(crate::analyze::FileObservation {path:path.into(),result_bytes:block.len() as u64});
        evidence.push(SearchEvidence {file_path:path.into(),file_index,symbol:Some(declaration),
            result_range:span,block,body,context:String::new(),is_complete:true,is_code:true,is_protected});
        files.push(file);
    }
    let base_text = text.clone();
    let relation = "\n#### Related indexed context\n\n- connected source hint\n";
    text.insert_str(files[0].section_insertions()[0],relation);
    let render_plan = Some(RenderPlan {base_text,files,
        relation_insertions:vec![(0,0,relation.into())]});
    SearchOutput {text,source_files,evidence,render_plan,is_partial_or_stale:false}
}
#[tokio::test]
async fn test_jev_threshold_replay_and_protected_file_boundary() {
    let fake = Fake {calls:AtomicUsize::new(0),probability:0.8};
    let base = output();
    let result = filter(base,"original task",&json!({"query":"search terms"}),&fake,Policy::default(),0.70,10_000).await;
    assert_eq!(result.status,"applied");
    assert!(!result.output.text.contains("fn alpha() {}"));
    assert!(result.output.text.contains("## 1. a.rs") && result.output.text.contains("## 2. b.rs"));
    assert!(result.output.text.contains("fn beta() {}"));
    assert!(result.output.text.contains("#### Related indexed context")
        && result.output.text.contains("connected source hint"));
    assert_eq!(result.output.text.matches("```").count(),2);
    assert_eq!(result.output.source_files.len(),1);
    assert_eq!(result.output.source_files[0].path,"b.rs");
    let replay = retention(&output().evidence,&result.answers.unwrap().answers,0.90);
    assert_eq!(replay[&0],"uncertain_or_related");
    assert_eq!(replay[&1],"protected_dependency_or_kind");
    assert_eq!(fake.calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn test_jev_partial_and_missing_evidence_are_retained() {
    let fake = Fake {calls:AtomicUsize::new(0),probability:1.0};
    let mut base = output();
    base.evidence[0].is_complete = false;
    base.evidence[1].is_complete = false;
    let text = base.text.clone();
    let result = filter(base,"task",&json!({"query":"query"}),&fake,Policy::default(),0.70,10_000).await;
    assert_eq!(result.status,"bypassed");
    assert_eq!(result.output.text,text);
    assert_eq!(fake.calls.load(Ordering::SeqCst),0);
}
#[test]
fn test_jev_probability_boundaries() {
    let evidence = output();
    for (probability,expected) in [(0.0,"uncertain_or_related"),(0.5,"uncertain_or_related"),(0.69,"uncertain_or_related"),(0.70,"omit"),(0.71,"omit"),(1.0,"omit")] {
        let answers = BTreeMap::from([("body-0".into(),Answer::Noul {noul:probability})]);
        assert_eq!(retention(&evidence.evidence,&answers,0.70)[&0],expected);
        assert_eq!(retention(&evidence.evidence,&answers,0.70)[&1],"protected_dependency_or_kind");
    }
    assert!(!valid_threshold(f64::NAN) && !valid_threshold(0.5) && !valid_threshold(1.1));
}
