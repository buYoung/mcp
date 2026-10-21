#[cfg(unix)]
use super::evidence::Dictionary;
use super::{prepare_dataset, LookupResult, RelatedTermsLookup};
use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;

fn create_dataset(path: &Path, related_word: &str) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(r#"
        CREATE TABLE sense_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO sense_meta VALUES ('schema_version','rt-sense/5'), ('dataset_id','shared-fixture'),
            ('input_sense_rule','{"direct":"head"}');
        CREATE TABLE terms (term_id INTEGER PRIMARY KEY, surface TEXT, word_key TEXT UNIQUE, kinds TEXT);
        CREATE TABLE fields (field TEXT PRIMARY KEY, label TEXT, scope TEXT);
        CREATE TABLE topics (topic TEXT PRIMARY KEY, label TEXT, scope TEXT, source_topic INTEGER);
        CREATE TABLE topic_fields (topic TEXT, field TEXT, rank INTEGER, PRIMARY KEY(topic,field));
        CREATE TABLE concepts (concept INTEGER PRIMARY KEY, name TEXT, topic TEXT);
        CREATE TABLE personas (persona INTEGER PRIMARY KEY, term_id INTEGER, topic TEXT, concept INTEGER, source_persona_count INTEGER);
        CREATE TABLE relation_kinds (kind_id INTEGER PRIMARY KEY, kind TEXT);
        CREATE TABLE relations (term_id INTEGER, rank INTEGER, word_key TEXT, surface_override TEXT, kind_id INTEGER, persona INTEGER);
        CREATE VIEW relation_texts AS SELECT term_id,rank,word_key,coalesce(surface_override,word_key) AS surface,kind,persona
            FROM relations JOIN relation_kinds USING(kind_id);
        CREATE TABLE input_senses (word_key TEXT, persona INTEGER);
        CREATE TABLE input_relations (input_word_key TEXT, source_persona INTEGER, related_word_key TEXT, surface TEXT, kind TEXT, rank INTEGER);
        INSERT INTO fields VALUES ('code','Code','fixture');
        INSERT INTO topics VALUES ('code','Code','fixture',1);
        INSERT INTO topic_fields VALUES ('code','code',0);
        INSERT INTO concepts VALUES (1,'Code','code');
        INSERT INTO terms VALUES (1,'token','token','[]');
        INSERT INTO personas VALUES (1,1,'code',1,1);
        INSERT INTO relation_kinds VALUES (1,'synonym');
    "#).unwrap();
    connection
        .execute(
            "INSERT INTO relations VALUES (1,0,?1,NULL,1,1)",
            [related_word],
        )
        .unwrap();
}

fn direct_words(answer: &LookupResult) -> Vec<&str> {
    answer
        .senses
        .iter()
        .flat_map(|sense| sense.direct.iter().map(String::as_str))
        .collect()
}

#[test]
fn test_related_terms_rejects_replaced_local_source() {
    let sources = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let first = sources.path().join("first.sqlite3");
    let second = sources.path().join("second.sqlite3");
    create_dataset(&first, "alphaword");
    create_dataset(&second, "betaword");
    let before = [fs::read(&first).unwrap(), fs::read(&second).unwrap()];
    let prepared = prepare_dataset(&first, home.path(), true).unwrap();
    let mut stale = RelatedTermsLookup::new(prepared.clone());
    // Deterministic interleaving: another preparation wins before the lazy open.
    let replacement = prepare_dataset(&second, home.path(), true).unwrap();
    assert_eq!(prepared.path(), replacement.path());
    assert_eq!(prepared.dataset_id(), replacement.dataset_id());
    let result = stale.lookup("token");
    assert!(
        result.is_err(),
        "a stale handle read another source: {result:?}"
    );
    let answer = RelatedTermsLookup::new(replacement)
        .lookup("token")
        .unwrap();
    assert_eq!(direct_words(&answer), ["betaword"]);
    let restored = prepare_dataset(&first, home.path(), true).unwrap();
    let answer = RelatedTermsLookup::new(restored).lookup("token").unwrap();
    assert_eq!(direct_words(&answer), ["alphaword"]);
    assert_eq!(
        [fs::read(&first).unwrap(), fs::read(&second).unwrap()],
        before
    );
}

#[test]
fn test_related_terms_rejects_reprepared_source_revision() {
    let source = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = source.path().join("dataset.sqlite3");
    create_dataset(&path, "alphaword");
    let prepared = prepare_dataset(&path, home.path(), true).unwrap();
    let original_bytes = fs::metadata(&path).unwrap().len();
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("UPDATE relations SET word_key='betaword'", [])
            .unwrap();
        // Force a size change: the assertion must not depend on filesystem clock precision.
        connection
            .execute(
                "INSERT INTO sense_meta VALUES ('revision',?1)",
                params!["x".repeat(original_bytes as usize * 2)],
            )
            .unwrap();
    }
    assert!(fs::metadata(&path).unwrap().len() > original_bytes);
    let revised_bytes = fs::read(&path).unwrap();
    let replacement = prepare_dataset(&path, home.path(), true).unwrap();
    let result = RelatedTermsLookup::new(prepared).lookup("token");
    assert!(
        result.is_err(),
        "a stale handle read a new revision: {result:?}"
    );
    let answer = RelatedTermsLookup::new(replacement)
        .lookup("token")
        .unwrap();
    assert_eq!(direct_words(&answer), ["betaword"]);
    assert_eq!(fs::read(&path).unwrap(), revised_bytes);
}

// Unix permits replacing the pathname while an existing SQLite reader retains its inode.
#[cfg(unix)]
#[test]
fn test_related_terms_dictionary_uses_the_lookup_connection() {
    let sources = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let first = sources.path().join("first.sqlite3");
    let second = sources.path().join("second.sqlite3");
    create_dataset(&first, "alphaword");
    create_dataset(&second, "betaword");
    let prepared = prepare_dataset(&first, home.path(), true).unwrap();
    let mut lookup = RelatedTermsLookup::new(prepared);
    assert_eq!(
        direct_words(&lookup.lookup("token").unwrap()),
        ["alphaword"]
    );
    prepare_dataset(&second, home.path(), true).unwrap();
    // Different raw input bypasses the answer cache and reads the existing connection.
    assert_eq!(
        direct_words(&lookup.lookup("TOKEN").unwrap()),
        ["alphaword"]
    );
    let dictionary = Dictionary::load(&mut lookup).unwrap();
    assert!(dictionary.keys.contains("alphaword"));
    assert!(!dictionary.keys.contains("betaword"));
    assert_eq!(dictionary.fields_by_word["alphaword"], ["code"]);
}
