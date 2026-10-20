use super::helpers::{response_text, with_in_process_server};
use super::related_terms_tool::{configure, create_dataset};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::path::Path;

fn add_shared_words(path: &Path) {
    Connection::open(path).unwrap().execute_batch("
        UPDATE personas SET source_persona_count=100-persona WHERE term_id=1;
        INSERT INTO terms VALUES (9000,'context','context','[]'),(9003,'AbortSignal','abortsignal','[]');
        INSERT INTO personas VALUES (9001,9000,'topic7',7,1),(9002,9000,'topic8',8,1);
        INSERT INTO fields VALUES ('aux','Auxiliary','fixture');
        INSERT INTO topics VALUES ('aux','Auxiliary','fixture',99);
        INSERT INTO topic_fields VALUES ('aux','aux',0);
        INSERT INTO personas VALUES (9003,9003,'aux',NULL,1);
    ").unwrap();
}

fn fields(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("### "))
        .filter_map(|line| {
            line.split_once('[')?
                .1
                .split_once(']')
                .map(|(field, _)| field)
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_overlay_updates_after_edits_and_late_dataset_readiness() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    create_dataset(&dataset);
    add_shared_words(&dataset);
    configure(repo.path(), &dataset, "");
    fs::write(
        repo.path().join("first.rs"),
        "fn token() {}\nfn direct7x00() {}\nfn AbortSignal() {}\n",
    )
    .unwrap();
    fs::write(repo.path().join("second.rs"), "fn abort_signal() {}\n").unwrap();
    let prepared_dir = repo.path().join("related-terms/local/tool-fixture");
    fs::create_dir_all(&prepared_dir).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(prepared_dir.join("prepare.lock"))
        .unwrap();
    lock.lock().unwrap();
    let root = repo.path().to_path_buf();
    with_in_process_server(repo.path(), None, move |mut client| async move {
        client.call("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"overlay-test","version":"1"}})).await.unwrap();
        let waiting = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| text.contains("preparing local indexes")).await.unwrap();
        assert!(response_text(&waiting).contains("preparing local indexes"));
        // Complete the first index publish while dataset preparation is still blocked.
        let searched = client.call("tools/call", json!({"name":"search","arguments":{"query":"token"}})).await.unwrap();
        assert!(response_text(&searched).contains("token"), "{searched}");
        drop(lock);
        let first = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| {
            fields(text).first() == Some(&"field7") && text.contains("Codebase evidence:")
        }).await.unwrap();
        assert_eq!(fields(response_text(&first)).first(), Some(&"field7"), "{first}");
        assert!(response_text(&first).contains("showing 6 of 8"));
        // Production never removes senses, so the evidence note carries no removal count.
        assert!(!response_text(&first).contains("; removed "), "{first}");
        let prepared_before = fs::read(root.join("related-terms/local/tool-fixture/dataset.sqlite3")).unwrap();
        let aliases = client.call_tool_until("synonyms", json!({"terms":["AbortSignal"]}), |text| text.contains("input files: 2, occurrences: 2")).await.unwrap();
        assert!(response_text(&aliases).contains("input files: 2, occurrences: 2"), "{aliases}");

        // With watch disabled, only synonyms' ordinary supervisor refresh path drives this.
        fs::write(root.join("first.rs"), "fn token() {}\nfn direct2x00() {}\nfn AbortSignal() {}\n// changed source length\n").unwrap();
        let edited = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| {
            fields(text).first() == Some(&"field2") && text.contains("Codebase evidence:")
        }).await.unwrap();
        assert_eq!(fields(response_text(&edited)).first(), Some(&"field2"), "{edited}");
        fs::write(root.join("added.rs"), "fn context() {}\n").unwrap();
        let shared = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| {
            fields(text).get(..3) == Some(&["field2", "field7", "field8"][..])
        }).await.unwrap();
        let shared_text = response_text(&shared);
        assert_eq!(fields(shared_text).get(..3), Some(&["field2", "field7", "field8"][..]), "{shared_text}");
        assert_eq!(shared_text.matches("\"context\"").count(), 1, "{shared_text}");
        fs::remove_file(root.join("added.rs")).unwrap();
        let expected = response_text(&edited).to_string();
        let restored = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| text == expected).await.unwrap();
        assert_eq!(response_text(&restored), expected);
        assert_eq!(fs::read(root.join("related-terms/local/tool-fixture/dataset.sqlite3")).unwrap(), prepared_before);
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_overlay_disabled_does_not_prepare_evidence() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("source.rs"),
        "fn token() {}\nfn AbortSignal() {}\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join(".codemap")).unwrap();
    fs::write(repo.path().join(".codemap/config.toml"), "# codemap-config-version: 30\n[related_terms]\nenabled=false\ndataset_path='/does/not/exist'\n").unwrap();
    let root = repo.path().to_path_buf();
    with_in_process_server(repo.path(), None, move |mut client| async move {
        let list = client.call("tools/list", json!({})).await.unwrap();
        assert!(list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["name"] != "synonyms"));
        let search = client
            .call(
                "tools/call",
                json!({"name":"search","arguments":{"query":"token"}}),
            )
            .await
            .unwrap();
        assert_ne!(search["result"]["isError"], true);
        assert!(!response_text(&search).contains("Codebase evidence"));
        assert!(!root.join("related-terms").exists());
    })
    .await;
}
