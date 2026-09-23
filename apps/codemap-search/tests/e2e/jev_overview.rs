//! Unbounded overview evidence through the actual serializer, batcher and MCP adapter.
use super::helpers::{create_mock_repo, response_text, with_in_process_server};
use codemap_search::jev::mock::{MockTransport, ScriptedResponse};
use codemap_search::jev::{EvaluatorConfig, JevEvaluator};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

#[tokio::test]
async fn monorepo_overview_sends_every_full_file_row_in_bounded_score_batches() {
    let mut owned = vec![(
        ".codemap/config.toml".into(),
        "[analysis.jev]\noverview_enabled=true\n".into(),
    )];
    for i in 0..70 {
        let workspace = if i < 35 { "apps/one" } else { "packages/two" };
        let source = (0..9)
            .map(|j| format!("export function budget{i}_{j}() {{ return 'PRIVATE_BODY'; }}\n"))
            .collect::<String>();
        owned.push((format!("{workspace}/src/f{i:02}.ts"), source));
    }
    let borrowed: Vec<_> = owned
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect();
    let temp = create_mock_repo(&borrowed).unwrap();
    let transport = Arc::new(MockTransport::with_handler(|body| {
        let answers: serde_json::Map<String, Value> = body["questions"].as_object().unwrap().iter().map(|(id, question)| {
            assert_eq!(question["type"], "score", "no declaration-role stage");
            let is_related = question["instructions"]["candidate"]["file_path"] == "packages/two/src/f69.ts";
            let answer = if is_related { json!({"type":"score","score":3.0,"probabilities":{"0":0.0,"1":0.0,"2":0.0,"3":1.0}}) }
                else { json!({"type":"score","score":0.0,"probabilities":{"0":1.0,"1":0.0,"2":0.0,"3":0.0}}) };
            (id.clone(), answer)
        }).collect();
        ScriptedResponse::ok(
            json!({"model":"jev-1.13.0","answers":answers,"usage":{"input_tokens":20,"output_tokens":5}}).to_string(),
        )
    }));
    let posts = transport.clone();
    let evaluator = Arc::new(
        JevEvaluator::new(
            transport,
            EvaluatorConfig {
                max_batch_bytes: 16_000,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    with_in_process_server(temp.path(), Some(evaluator), |mut client| async move {
        let response = client
            .call_tool_until(
                "overview",
                json!({"task_query":"locate budget69"}),
                |text| text.contains("## Recommended files"),
            )
            .await
            .unwrap();
        assert!(response["error"].is_null(), "{response}");
        let text = response_text(&response);
        assert!(
            text.contains("## Workspace Scopes"),
            "ordinary monorepo response stays unchanged: {text}"
        );
        assert!(
            text.contains(
                "Evaluated all 70 indexed files in this snapshot; 1 qualified, showing 1."
            ),
            "{text}"
        );
        assert!(text.contains("### 1. packages/two/src/f69.ts"));
        assert!(!text.contains("role:"));
        let posts = posts.posts();
        assert!(
            posts.len() > 1,
            "the full input must really cross a batch boundary"
        );
        let mut paths = BTreeSet::new();
        for post in posts {
            assert!(post.bytes.len() <= 16_000);
            assert_eq!(post.body["state"]["task_query"], "locate budget69");
            assert!(
                !serde_json::to_string(&post.body["state"])
                    .unwrap()
                    .contains("budget69_8"),
                "the whole overview is not repeated in shared state"
            );
            for question in post.body["questions"].as_object().unwrap().values() {
                let candidate = &question["instructions"]["candidate"];
                let path = candidate["file_path"].as_str().unwrap();
                assert!(
                    paths.insert(path.to_string()),
                    "each ordinary file has one question"
                );
                let row = candidate["overview_text"].as_str().unwrap();
                let number = path
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .trim_start_matches('f')
                    .trim_end_matches(".ts")
                    .parse::<usize>()
                    .unwrap();
                assert!(
                    row.contains(&format!("budget{number}_8")),
                    "the four-symbol display cap must not affect evidence: {row}"
                );
                assert!(!row.contains("PRIVATE_BODY"));
                assert!(candidate.get("declarations").is_none());
            }
        }
        assert_eq!(paths.len(), 70);
        assert!(paths.contains("apps/one/src/f00.ts") && paths.contains("packages/two/src/f69.ts"));
    })
    .await;
}
