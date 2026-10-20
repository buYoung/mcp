use super::helpers::{response_text, with_in_process_server};
use rusqlite::{params, Connection};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;

pub(super) fn create_dataset(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(r#"
        CREATE TABLE sense_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO sense_meta VALUES ('schema_version','rt-sense/5'), ('dataset_id','tool-fixture'),
            ('input_sense_rule','{"direct":"head"}');
        CREATE TABLE terms (term_id INTEGER PRIMARY KEY, surface TEXT, word_key TEXT UNIQUE, kinds TEXT);
        CREATE TABLE fields (field TEXT PRIMARY KEY, label TEXT, scope TEXT);
        CREATE TABLE topics (topic TEXT PRIMARY KEY, label TEXT, scope TEXT, source_topic INTEGER);
        CREATE TABLE topic_fields (topic TEXT, field TEXT, rank INTEGER, PRIMARY KEY(topic,field));
        CREATE TABLE concepts (concept INTEGER PRIMARY KEY, name TEXT, topic TEXT);
        CREATE TABLE personas (persona INTEGER PRIMARY KEY, term_id INTEGER, topic TEXT, concept INTEGER, source_persona_count INTEGER);
        CREATE TABLE relation_kinds (kind_id INTEGER PRIMARY KEY, kind TEXT);
        INSERT INTO relation_kinds VALUES (1,'synonym');
        CREATE TABLE relations (term_id INTEGER, rank INTEGER, word_key TEXT, surface_override TEXT, kind_id INTEGER, persona INTEGER,
            PRIMARY KEY(term_id,word_key)) WITHOUT ROWID;
        CREATE VIEW relation_texts AS SELECT term_id,rank,word_key,coalesce(surface_override,word_key) AS surface,kind,persona
            FROM relations JOIN relation_kinds USING(kind_id);
        CREATE TABLE input_senses (word_key TEXT, persona INTEGER, PRIMARY KEY(word_key,persona));
        CREATE TABLE input_relations (input_word_key TEXT, source_persona INTEGER, related_word_key TEXT, surface TEXT, kind TEXT, rank INTEGER);
        INSERT INTO terms VALUES (1,'token','token','[]');
    "#).unwrap();
    let mut id = 1000;
    for index in 1..=8 {
        let topic = format!("topic{index}");
        let field = format!("field{index}");
        connection
            .execute(
                "INSERT INTO fields VALUES (?1,?2,'fixture')",
                params![field, format!("분야 {index}")],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO topics VALUES (?1,?2,'fixture',?3)",
                params![topic, format!("Topic {index}"), index],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO topic_fields VALUES (?1,?2,0)",
                params![topic, field],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO concepts VALUES (?1,?2,?3)",
                params![index, format!("Concept {index}"), topic],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO personas VALUES (?1,1,?2,?1,2)",
                params![index, topic],
            )
            .unwrap();
        for word in 0..25 {
            connection
                .execute(
                    "INSERT INTO relations VALUES (1,?1,?2,NULL,1,?3)",
                    params![
                        index * 100 + word,
                        format!("direct{index}x{word:02}"),
                        index
                    ],
                )
                .unwrap();
        }
        for group in ["near", "topic"] {
            for word in 0..15 {
                id += 1;
                let key = format!("{group}{index}x{word:02}");
                connection
                    .execute("INSERT INTO terms VALUES (?1,?2,?2,'[]')", params![id, key])
                    .unwrap();
                let concept = (group == "near").then_some(index);
                connection
                    .execute(
                        "INSERT INTO personas VALUES (?1,?1,?2,?3,1)",
                        params![id, topic, concept],
                    )
                    .unwrap();
            }
        }
    }
}

fn add_input_parts(path: &Path) {
    Connection::open(path).unwrap().execute_batch("
        INSERT INTO fields VALUES ('field9','Extra field','fixture');
        INSERT INTO topics VALUES ('topic9','Shared topic','fixture',9);
        INSERT INTO topic_fields VALUES ('topic9','field9',0);
        INSERT INTO concepts VALUES (9,'Extra concept','topic9');
        INSERT INTO terms VALUES (9000,'zebra','zebra','[]'),(9001,'zulu','zulu','[]'),
            (9002,'token zebra zulu','tokenzebrazulu','[]');
        INSERT INTO personas VALUES (9000,9000,'topic9',9,1),(9001,9001,'topic9',9,1);
        INSERT INTO relations VALUES (9000,1,'zebraword',NULL,1,9000),(9001,1,'zuluword',NULL,1,9001);
    ").unwrap();
}

pub(super) fn configure(repo: &Path, dataset: &Path, extra: &str) {
    fs::create_dir_all(repo.join(".codemap")).unwrap();
    fs::write(repo.join(".codemap/config.toml"), format!(
        "# codemap-config-version: 30\n[index.refresh]\nwatch = false\nindex_staleness_ms = 1\n[related_terms]\nenabled = true\ndataset_path = {}\n{extra}\n",
        serde_json::to_string(&dataset.to_string_lossy()).unwrap(),
    )).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_disabled_has_no_side_effects() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path().to_path_buf();
    with_in_process_server(repo.path(), None, move |mut client| async move {
        let initialized = client.call("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rterms-test","version":"1"}})).await.unwrap();
        assert!(!initialized["result"]["instructions"].as_str().unwrap().contains("Use synonyms"));
        let list = client.call("tools/list", json!({})).await.unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().iter().all(|tool| tool["name"] != "synonyms"));
        let absent = client.call("tools/call", json!({"name":"synonyms","arguments":{"terms":["token"]}})).await.unwrap();
        assert_eq!(absent["error"]["code"], -32601);
        assert!(!root.join("related-terms").exists());
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_schema_groups_and_input_cap() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    create_dataset(&dataset);
    let before = fs::read(&dataset).unwrap();
    configure(repo.path(), &dataset, "");
    with_in_process_server(repo.path(), None, |mut client| async move {
        client.call("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rterms-test","version":"1"}})).await.unwrap();
        let list = client.call("tools/list", json!({})).await.unwrap();
        let tool = list["result"]["tools"].as_array().unwrap().iter().find(|tool| tool["name"] == "synonyms").unwrap();
        assert_eq!(tool["annotations"], json!({"readOnlyHint":true,"openWorldHint":false}));
        assert_eq!(tool["inputSchema"]["properties"]["terms"]["minItems"], 1);
        assert_eq!(tool["inputSchema"]["properties"]["terms"]["maxItems"], 16);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        let answer = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| text.contains("Senses (")).await.unwrap();
        let text = response_text(&answer);
        assert!(text.contains("Senses (showing 6 of 8)"), "{text}");
        assert_eq!(text.matches("### ").count(), 6);
        // Ready evidence appends word presence inside the same parentheses.
        assert!(text.contains("Direct (showing 20 of 25"), "{text}");
        assert!(text.contains("Near (showing 10 of 15"), "{text}");
        assert!(text.contains("Same topic (showing 10 of 15"), "{text}");
        assert!(text.len() <= 100_000);
        let missing = client.call("tools/call", json!({"name":"synonyms","arguments":{"terms":["zzzxmissingzz"]}})).await.unwrap();
        assert!(response_text(&missing).contains("Not in the dictionary"));
        let excessive = client.call("tools/call", json!({"name":"synonyms","arguments":{"terms":vec!["token";17]}})).await.unwrap();
        assert_eq!(excessive["result"]["isError"], true);
        assert!(response_text(&excessive).contains("16"));
        for args in [json!({"terms":[]}), json!({"terms":[42]}), json!({"terms":[" "]}), json!({"terms":["token"],"path":"elsewhere"}),
            json!({"terms":["token"],"field":"code","fields":["code"],"topic":"rust"})] {
            let result = client.call("tools/call", json!({"name":"synonyms","arguments":args})).await.unwrap();
            assert_eq!(result["result"]["isError"], true);
        }
    }).await;
    assert_eq!(fs::read(&dataset).unwrap(), before);
    assert!(repo
        .path()
        .join("related-terms/local/tool-fixture/ready.json")
        .is_file());
    assert!(!repo.path().join("related-terms/tool-fixture").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_balances_parts_and_merges_display_groups() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    create_dataset(&dataset);
    add_input_parts(&dataset);
    let before = fs::read(&dataset).unwrap();
    configure(repo.path(), &dataset, "");
    with_in_process_server(repo.path(), None, |mut client| async move {
        let result = client
            .call_tool_until("synonyms", json!({"terms":["token zebra zulu"]}), |text| {
                text.contains("Senses (")
            })
            .await
            .unwrap();
        let text = response_text(&result);
        assert!(text.starts_with("## "), "{text}");
        assert!(
            text.find("Dictionary ready").unwrap() < text.find("Looked up:").unwrap(),
            "{text}"
        );
        assert_eq!(text.matches("### ").count(), 6, "{text}");
        assert_eq!(text.matches("[field9]").count(), 1, "{text}");
        assert!(
            text.contains("\"zebraword\"") && text.contains("\"zuluword\""),
            "{text}"
        );
        for section in text.split("### ").skip(1) {
            assert!(
                section.contains("Direct (")
                    || section.contains("Near (")
                    || section.contains("Same topic ("),
                "{section}"
            );
        }
        let long = client
            .call_tool_until(
                "synonyms",
                json!({"terms":["token zebra zulu alpha beta gamma delta"]}),
                |text| text.contains("Senses ("),
            )
            .await
            .unwrap();
        let text = response_text(&long);
        assert!(text.contains("Lookup attempts omitted:"), "{text}");
        let attempts = text
            .lines()
            .find_map(|line| line.strip_prefix("Looked up: "))
            .unwrap();
        let attempts: Vec<String> = serde_json::from_str(&format!("[{attempts}]")).unwrap();
        assert_eq!(attempts.len(), 8);
        assert!(text.contains("Resolved inputs (parts):"), "{text}");
    })
    .await;
    assert_eq!(fs::read(&dataset).unwrap(), before);
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_balancing_preserves_supported_prefix() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    create_dataset(&dataset);
    add_input_parts(&dataset);
    configure(repo.path(), &dataset, "");
    let mut code = "fn token_zebra_zulu() {}\n".to_string();
    for field in 1..=6 {
        code.push_str(&format!("fn direct{field}x00() {{}}\n"));
    }
    fs::write(repo.path().join("source.rs"), code).unwrap();
    with_in_process_server(repo.path(), None, |mut client| async move {
        let result = client
            .call_tool_until("synonyms", json!({"terms":["token zebra zulu"]}), |text| {
                text.contains("Codebase evidence: moved back")
            })
            .await
            .unwrap();
        let text = response_text(&result);
        assert_eq!(text.matches("### ").count(), 6, "{text}");
        for field in 1..=6 {
            assert!(text.contains(&format!("[field{field}]")), "{text}");
        }
        assert!(
            !text.contains("[field9]"),
            "unsupported input parts must not displace supported senses: {text}"
        );
        assert!(!text.contains("; removed "), "{text}");
        assert!(
            text.find("Codebase evidence:").unwrap() < text.find("Looked up:").unwrap(),
            "{text}"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_keeps_shared_words_hidden_by_an_earlier_cap() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    create_dataset(&dataset);
    add_input_parts(&dataset);
    Connection::open(&dataset)
        .unwrap()
        .execute(
            "UPDATE relations SET word_key='direct1x24' WHERE term_id IN (9000,9001)",
            [],
        )
        .unwrap();
    let before = fs::read(&dataset).unwrap();
    configure(repo.path(), &dataset, "");
    let mut code = "fn token_zebra_zulu() {}\n".to_string();
    // All 25 words have evidence, so the shared word remains beyond the first 20.
    for word in 0..25 {
        code.push_str(&format!("fn direct1x{word:02}() {{}}\n"));
    }
    fs::write(repo.path().join("source.rs"), code).unwrap();
    with_in_process_server(repo.path(), None, |mut client| async move {
        let result = client
            .call_tool_until("synonyms", json!({"terms":["token zebra zulu"]}), |text| {
                text.contains("Codebase evidence: moved back")
            })
            .await
            .unwrap();
        let text = response_text(&result);
        assert!(
            text.contains("Direct (showing 20 of 25; 20 in code)"),
            "{text}"
        );
        let shared = text
            .split("### ")
            .find(|section| section.contains("[field9]"))
            .unwrap();
        assert!(
            shared.contains("Direct (showing 1 of 1; 1 in code): \"direct1x24\""),
            "{text}"
        );
        assert!(!shared.contains("already shown"), "{text}");
        assert_eq!(text.matches("\"direct1x24\"").count(), 1, "{text}");
        assert!(!text.contains("; removed "), "{text}");
    })
    .await;
    assert_eq!(fs::read(&dataset).unwrap(), before);
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_utf8_output_obeys_client_cap() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    create_dataset(&dataset);
    configure(repo.path(), &dataset, "[output.client]\npi_max_bytes = 512");
    with_in_process_server(repo.path(), None, |mut client| async move {
        client.call("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rterms-test","version":"1"}})).await.unwrap();
        let result = client.call_tool_until("synonyms", json!({"terms":["token","token"]}), |text| text.contains("Output truncated")).await.unwrap();
        assert_ne!(result["result"]["isError"], true, "{result}");
        let text = response_text(&result);
        assert!(text.len() <= 512, "{} bytes: {text}", text.len());
        assert!(text.contains("Output truncated"), "{text}");
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_downloading_does_not_block_search() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".codemap")).unwrap();
    fs::write(repo.path().join(".codemap/config.toml"), "# codemap-config-version: 30\n[index.refresh]\nwatch = false\n[related_terms]\nenabled = true\n").unwrap();
    let directory = repo.path().join("related-terms/rts5-75e02862daa29308");
    fs::create_dir_all(&directory).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("download.lock"))
        .unwrap();
    lock.lock().unwrap();
    with_in_process_server(repo.path(), None, |mut client| async move {
        client.call("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rterms-test","version":"1"}})).await.unwrap();
        let result = client.call("tools/call", json!({"name":"synonyms","arguments":{"terms":["token"]}})).await.unwrap();
        assert!(response_text(&result).contains("downloading"), "{result}");
        let search = tokio::time::timeout(Duration::from_secs(3), client.call("tools/call", json!({"name":"search","arguments":{"query":"token"}}))).await;
        assert!(search.is_ok(), "search must not wait for dataset preparation");
    }).await;
    // Keep the lock through server shutdown, so this test never accesses the public URL.
    drop(lock);
    assert!(!directory.join("dataset.sqlite3").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn test_related_terms_tool_unavailable_retries_local_preparation() {
    let repo = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let dataset = source.path().join("dataset.sqlite3");
    configure(repo.path(), &dataset, "");
    with_in_process_server(repo.path(), None, move |mut client| async move {
        client.call("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"rterms-test","version":"1"}})).await.unwrap();
        let failed = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| text.contains("unavailable")).await.unwrap();
        assert!(response_text(&failed).contains("unavailable"), "{failed}");
        create_dataset(&dataset);
        let ready = client.call_tool_until("synonyms", json!({"terms":["token"]}), |text| text.contains("Senses (")).await.unwrap();
        assert!(response_text(&ready).contains("Senses ("), "{ready}");
    }).await;
}
