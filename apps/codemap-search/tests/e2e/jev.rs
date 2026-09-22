use super::helpers::{create_mock_repo, McpClient};
use serde_json::{json, Value};

fn text(response: &Value) -> &str { response["result"]["content"][0]["text"].as_str().unwrap_or("") }

#[tokio::test]
async fn test_jev_keyless_modes_preserve_baseline_and_request_intent() {
    let temp = create_mock_repo(&[
        (".codemap/config.toml", "[update]\nconfig_auto_update=false\n[analysis.jev]\noverview_enabled=true\nsearch_filter_enabled=true\napi_key_env='CODEMAP_JEV_TEST_NO_KEY'\nsearch_filter_min_unrelated_probability=0.90\n"),
        ("src/handler.rs", "pub fn target_handler() { println!(\"source_marker\"); }"),
    ]).unwrap();
    let mut client = McpClient::spawn(temp.path()).await.unwrap();
    let listing = client.send_request("tools/list",json!({})).await.unwrap();
    let tools = listing["result"]["tools"].as_array().unwrap();
    let search = tools.iter().find(|item| item["name"] == "search").unwrap();
    let overview = tools.iter().find(|item| item["name"] == "overview").unwrap();
    assert_eq!(search["inputSchema"]["required"],json!(["query"]));
    assert_eq!(search["annotations"]["openWorldHint"],true);
    assert_eq!(overview["annotations"]["openWorldHint"],true);
    assert_eq!(search["inputSchema"]["properties"]["task_query"]["type"],"string");
    assert_eq!(overview["inputSchema"]["properties"]["task_query"]["type"],"string");

    let args = json!({"query":"target_handler","caller_context":false});
    let base = client.send_tool_until("search",args.clone(),|text| text.contains("target_handler") && text.contains("source_marker")).await.unwrap();
    let evaluated = client.send_request("tools/call",json!({"name":"search","arguments":{"query":"target_handler","caller_context":false,"task_query":"Find the handler"}})).await.unwrap();
    assert_eq!(text(&base),text(&evaluated));
    assert_eq!(evaluated["result"]["_meta"]["jev"]["outcome"],"bypassed");
    assert_eq!(evaluated["result"]["_meta"]["jev"]["reason"],"missing_api_key");
    let forgotten = client.send_request("tools/call",json!({"name":"search","arguments":args})).await.unwrap();
    assert_eq!(text(&base),text(&forgotten));
    assert_eq!(forgotten["result"]["_meta"]["jev"]["reason"],"missing_task_query");
    let root = client.send_tool_until("overview",json!({"task_query":"Find the handler"}),|text| text.contains("handler.rs")).await.unwrap();
    assert_eq!(root["result"]["_meta"]["jev"]["reason"],"missing_api_key");
    let old_root = client.send_request("tools/call",json!({"name":"overview","arguments":{}})).await.unwrap();
    assert_eq!(text(&root),text(&old_root));
    for (name,args) in [("search",json!({"query":"handler","task_query":2})),("overview",json!({"task_query":false}))] {
        let response = client.send_request("tools/call",json!({"name":name,"arguments":args})).await.unwrap();
        assert_eq!(response["error"]["code"],-32602);
    }
}
