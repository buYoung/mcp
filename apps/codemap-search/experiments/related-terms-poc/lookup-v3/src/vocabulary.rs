//! Index-time repository vocabulary: commit through the product engine, enumerate the live
//! postings of that exact committed generation, persist word → unique-live-file counts and
//! word → Snowball stem, and only then publish them. Serving lookup never calls into this module.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tantivy::collector::Count;
use tantivy::query::{AllQuery, BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption};
use tantivy::{DocId, DocSet, Index, ReloadPolicy, Term, TERMINATED};

use crate::stats::{
    hex, index_generation, PublishedStats, FIELD_POLICY, INDEX_DIR, PUBLISHED_FILE, STATS_FIELDS, STATS_FORMAT,
};

const STATS_DIR: &str = "stats";
const KEPT_STATS_FILES: usize = 3;

pub struct Vocabulary {
    pub counts: BTreeMap<String, u64>,
    pub live_files: u64,
    pub index_generation: String,
    pub index_opstamp: u64,
    pub segments: Vec<serde_json::Value>,
    /// Postings entries skipped because their document is deleted in this generation.
    pub deleted_postings_skipped: u64,
    /// Sum over fields of per-field live document frequencies (before the cross-field union).
    pub per_field_live_postings: u64,
    /// Workspace-relative paths of the live documents (one per indexed file).
    pub live_paths: Vec<String>,
}

fn stats_fields(index: &Index) -> Result<Vec<Field>, String> {
    let schema = index.schema();
    STATS_FIELDS
        .iter()
        .map(|name| schema.get_field(name).map_err(|error| format!("field {name}: {error}")))
        .collect()
}

/// Enumerate live postings of the committed generation in `index_dir`.
///
/// The generation is pinned by `meta.json` bytes read before and after the enumeration;
/// the reader's segments must equal that commit point, otherwise a concurrent writer moved
/// the index and the result is rejected instead of published.
pub fn materialize(index_dir: &Path) -> Result<Vocabulary, String> {
    let (generation_before, opstamp) = index_generation(index_dir)?;
    let meta: serde_json::Value = serde_json::from_slice(
        &std::fs::read(index_dir.join("meta.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let index = Index::open_in_dir(index_dir).map_err(|error| format!("index open failed: {error}"))?;
    let fields = stats_fields(&index)?;
    let file_path_field = index.schema().get_field("file_path").map_err(|error| error.to_string())?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|error: tantivy::TantivyError| error.to_string())?;
    let searcher = reader.searcher();

    let mut expected: Vec<(String, u32, u32)> = meta["segments"]
        .as_array()
        .ok_or("meta.json has no segments")?
        .iter()
        .map(|segment| {
            (
                segment["segment_id"].as_str().unwrap_or_default().replace('-', ""),
                segment["max_doc"].as_u64().unwrap_or_default() as u32,
                segment["deletes"]["num_deleted_docs"].as_u64().unwrap_or(0) as u32,
            )
        })
        .collect();
    let mut actual: Vec<(String, u32, u32)> = searcher
        .segment_readers()
        .iter()
        .map(|segment| (segment.segment_id().uuid_string(), segment.max_doc(), segment.num_deleted_docs()))
        .collect();
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(format!("reader segments {actual:?} differ from committed meta {expected:?}"));
    }

    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut live_files: u64 = 0;
    let mut live_paths: Vec<String> = Vec::new();
    let mut deleted_postings_skipped: u64 = 0;
    let mut per_field_live_postings: u64 = 0;
    for segment in searcher.segment_readers() {
        let alive = segment.alive_bitset();
        let is_alive = |doc: DocId| alive.is_none_or(|bitset| bitset.is_alive(doc));
        live_files += u64::from(segment.num_docs());
        // word -> live docs of this segment containing it in any searchable field.
        let mut segment_docs: HashMap<Vec<u8>, Vec<DocId>> = HashMap::new();
        for &field in &fields {
            let inverted = segment.inverted_index(field).map_err(|error| error.to_string())?;
            let mut terms = inverted.terms().stream().map_err(|error| error.to_string())?;
            while terms.advance() {
                let mut postings = inverted
                    .read_postings_from_terminfo(terms.value(), IndexRecordOption::Basic)
                    .map_err(|error| error.to_string())?;
                let mut docs = Vec::new();
                let mut doc = postings.doc();
                while doc != TERMINATED {
                    if is_alive(doc) {
                        docs.push(doc);
                    } else {
                        deleted_postings_skipped += 1;
                    }
                    doc = postings.advance();
                }
                if !docs.is_empty() {
                    per_field_live_postings += docs.len() as u64;
                    segment_docs.entry(terms.key().to_vec()).or_default().extend(docs);
                }
            }
        }
        for (word, mut docs) in segment_docs {
            docs.sort_unstable();
            docs.dedup();
            let word = String::from_utf8(word).map_err(|error| format!("non-UTF-8 term: {error}"))?;
            *counts.entry(word).or_default() += docs.len() as u64;
        }
        // Each live document is one file: verify there is exactly one live doc per path.
        let paths = segment.inverted_index(file_path_field).map_err(|error| error.to_string())?;
        let mut path_terms = paths.terms().stream().map_err(|error| error.to_string())?;
        while path_terms.advance() {
            let mut postings = paths
                .read_postings_from_terminfo(path_terms.value(), IndexRecordOption::Basic)
                .map_err(|error| error.to_string())?;
            let mut doc = postings.doc();
            while doc != TERMINATED {
                if is_alive(doc) {
                    live_paths.push(String::from_utf8_lossy(path_terms.key()).into_owned());
                }
                doc = postings.advance();
            }
        }
    }
    live_paths.sort();
    let distinct_paths = {
        let mut distinct = live_paths.clone();
        distinct.dedup();
        distinct.len() as u64
    };
    if live_paths.len() as u64 != live_files || distinct_paths != live_files {
        return Err(format!(
            "live path postings {} / distinct paths {distinct_paths} != live documents {live_files}",
            live_paths.len()
        ));
    }
    let (generation_after, _) = index_generation(index_dir)?;
    if generation_after != generation_before {
        return Err("index generation changed during materialization".to_string());
    }
    Ok(Vocabulary {
        counts,
        live_files,
        index_generation: generation_before,
        index_opstamp: opstamp,
        segments: meta["segments"].as_array().cloned().unwrap_or_default(),
        deleted_postings_skipped,
        per_field_live_postings,
        live_paths,
    })
}

/// Stem every stemmable vocabulary word once, at index time (`rt-stats/2`).
fn word_stems(counts: &BTreeMap<String, u64>) -> Vec<(String, String)> {
    let mut analyzer = crate::normalize::stem_analyzer();
    counts
        .keys()
        .filter(|word| crate::normalize::is_stemmable(word))
        .map(|word| (word.clone(), crate::normalize::stem(&mut analyzer, word)))
        .collect()
}

/// Write the stats SQLite file (temp + rename). Returns its path relative to the state dir.
fn persist(
    state_dir: &Path,
    vocabulary: &Vocabulary,
    stems: &[(String, String)],
) -> Result<(String, String, u64), String> {
    let directory = state_dir.join(STATS_DIR);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let name = format!("stats-{}.sqlite3", &vocabulary.index_generation[..16]);
    let temporary = directory.join(format!("{name}.tmp"));
    let _ = std::fs::remove_file(&temporary);
    {
        let mut connection = Connection::open(&temporary).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
                 CREATE TABLE stats_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE word_files (word TEXT PRIMARY KEY, file_count INTEGER NOT NULL) WITHOUT ROWID;
                 CREATE TABLE word_stems (word TEXT PRIMARY KEY, stem TEXT NOT NULL) WITHOUT ROWID;
                 CREATE INDEX word_stems_by_stem ON word_stems (stem);",
            )
            .map_err(|error| error.to_string())?;
        let transaction = connection.transaction().map_err(|error| error.to_string())?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO word_files (word, file_count) VALUES (?1, ?2)")
                .map_err(|error| error.to_string())?;
            for (word, count) in &vocabulary.counts {
                insert.execute((word, *count as i64)).map_err(|error| error.to_string())?;
            }
            let mut insert_stem = transaction
                .prepare("INSERT INTO word_stems (word, stem) VALUES (?1, ?2)")
                .map_err(|error| error.to_string())?;
            for (word, stem) in stems {
                insert_stem.execute((word, stem)).map_err(|error| error.to_string())?;
            }
            let mut meta = transaction
                .prepare("INSERT INTO stats_meta (key, value) VALUES (?1, ?2)")
                .map_err(|error| error.to_string())?;
            for (key, value) in [
                ("stats_format", STATS_FORMAT.to_string()),
                ("field_policy", FIELD_POLICY.to_string()),
                ("analyzer_policy", crate::normalize::ANALYZER_POLICY.to_string()),
                ("stem_policy", crate::normalize::STEM_POLICY.to_string()),
                ("stemmed_words", stems.len().to_string()),
                ("index_generation", vocabulary.index_generation.clone()),
                ("index_opstamp", vocabulary.index_opstamp.to_string()),
                ("live_files", vocabulary.live_files.to_string()),
                ("vocabulary_size", vocabulary.counts.len().to_string()),
            ] {
                meta.execute((key, value)).map_err(|error| error.to_string())?;
            }
        }
        transaction.commit().map_err(|error| error.to_string())?;
    }
    let final_path = directory.join(&name);
    std::fs::rename(&temporary, &final_path).map_err(|error| error.to_string())?;
    let bytes = std::fs::metadata(&final_path).map_err(|error| error.to_string())?.len();
    let digest = crate::stats::sha256_file(&final_path).map_err(|error| error.to_string())?;
    Ok((format!("{STATS_DIR}/{name}"), digest, bytes))
}

fn write_published(state_dir: &Path, published: &PublishedStats) -> Result<(), String> {
    let temporary = state_dir.join(format!("{PUBLISHED_FILE}.tmp"));
    let body = serde_json::to_vec_pretty(published).map_err(|error| error.to_string())?;
    std::fs::write(&temporary, body).map_err(|error| error.to_string())?;
    // rename(2) replaces published.json atomically: readers see the old or the new pair.
    std::fs::rename(&temporary, state_dir.join(PUBLISHED_FILE)).map_err(|error| error.to_string())
}

fn prune_stats_files(state_dir: &Path, keep: &str) {
    let Ok(entries) = std::fs::read_dir(state_dir.join(STATS_DIR)) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".sqlite3"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    files.sort();
    let removable = files.len().saturating_sub(KEPT_STATS_FILES);
    for (_, path) in files.into_iter().take(removable) {
        if !path.ends_with(keep) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn git_head(workspace: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace)
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// `rtpoc2 index`: refresh the private index from the cwd workspace, then materialize,
/// persist, and publish statistics for the committed generation.
pub fn index_and_publish(state_dir: &Path, should_fail_before_publish: bool) -> Result<serde_json::Value, String> {
    let workspace = std::env::current_dir().map_err(|error| error.to_string())?;
    let codemap_home = std::env::var_os("CODEMAP_HOME")
        .ok_or("CODEMAP_HOME must point to a private PoC directory (isolates global config)")?;
    if !PathBuf::from(&codemap_home).starts_with(state_dir) {
        return Err("CODEMAP_HOME must be inside --state so user configuration is not read".to_string());
    }
    std::fs::create_dir_all(state_dir).map_err(|error| error.to_string())?;
    codemap_search::workspace::ensure_index_root_is_not_user_home(&workspace)?;
    codemap_search::config::init(&workspace);
    let config = codemap_search::config::get();
    let index_dir = state_dir.join(INDEX_DIR);

    let indexing_started = Instant::now();
    let mut engine = codemap_search::index::TantivySearchEngine::new(&index_dir.to_string_lossy())?;
    let committed = engine.index_files_changed(&["."])?;
    drop(engine);
    let indexing_ms = indexing_started.elapsed().as_secs_f64() * 1000.0;

    let materialize_started = Instant::now();
    let vocabulary = materialize(&index_dir)?;
    let materialize_ms = materialize_started.elapsed().as_secs_f64() * 1000.0;
    let stem_started = Instant::now();
    let stems = word_stems(&vocabulary.counts);
    let stem_ms = stem_started.elapsed().as_secs_f64() * 1000.0;
    let persist_started = Instant::now();
    let (stats_file, stats_sha256, stats_bytes) = persist(state_dir, &vocabulary, &stems)?;
    let persist_ms = persist_started.elapsed().as_secs_f64() * 1000.0;
    // Private population evidence (repository paths never enter the shared dataset).
    std::fs::write(
        state_dir.join("indexed-files.json"),
        serde_json::to_vec_pretty(&vocabulary.live_paths).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let finalization = serde_json::json!({
        "index_refresh_committed": committed,
        "indexing_ms": indexing_ms,
        "materialize_ms": materialize_ms,
        "stem_ms": stem_ms,
        "persist_ms": persist_ms,
        "stats_bytes": stats_bytes,
        "deleted_postings_skipped": vocabulary.deleted_postings_skipped,
        "per_field_live_postings": vocabulary.per_field_live_postings,
        "segments": vocabulary.segments,
    });
    let published = PublishedStats {
        stats_format: STATS_FORMAT.to_string(),
        field_policy: FIELD_POLICY.to_string(),
        analyzer_policy: crate::normalize::ANALYZER_POLICY.to_string(),
        index_generation: vocabulary.index_generation.clone(),
        index_opstamp: vocabulary.index_opstamp,
        stats_file: stats_file.clone(),
        stats_sha256,
        live_files: vocabulary.live_files,
        vocabulary_size: vocabulary.counts.len() as u64,
        stem_policy: crate::normalize::STEM_POLICY.to_string(),
        stemmed_words: stems.len() as u64,
        workspace: serde_json::json!({
            "root": workspace.display().to_string(),
            "git_head": git_head(&workspace),
            "effective_config": {
                "max_file_size": config.max_file_size,
                "excluded_directories": config.excluded_directories,
                "use_git_exclude": config.use_git_exclude,
                "is_document_support_enabled": config.is_document_support_enabled,
                "is_shell_support_enabled": config.is_shell_support_enabled,
                "macro_expansion": format!("{:?}", config.macro_expansion),
            },
        }),
        finalization,
    };
    if should_fail_before_publish {
        // Failure injection for lifecycle evidence: the index and a stats file exist, but the
        // pair is never published, so readers must keep reporting the previous state as stale.
        return Err(format!(
            "injected failure before publication (index generation {} committed, {} not published)",
            &vocabulary.index_generation[..16],
            stats_file
        ));
    }
    write_published(state_dir, &published)?;
    prune_stats_files(state_dir, &stats_file);
    serde_json::to_value(&published).map_err(|error| error.to_string())
}

/// `rtpoc2 verify-stats`: recount every published word with the query engine (deletes are
/// honored by the collector), check that no live indexed term is missing, and re-stem the
/// published words against `word_stems`. This is an index-finalization audit, never part of
/// serving.
pub fn verify_published(state_dir: &Path) -> Result<serde_json::Value, String> {
    let started = Instant::now();
    let published: PublishedStats = serde_json::from_slice(
        &std::fs::read(state_dir.join(PUBLISHED_FILE)).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let index_dir = state_dir.join(INDEX_DIR);
    let (generation, _) = index_generation(&index_dir)?;
    if generation != published.index_generation {
        return Err("published statistics are stale; cannot audit a different generation".to_string());
    }
    let index = Index::open_in_dir(&index_dir).map_err(|error| error.to_string())?;
    let fields = stats_fields(&index)?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|error: tantivy::TantivyError| error.to_string())?;
    let searcher = reader.searcher();
    let stats = Connection::open_with_flags(
        state_dir.join(&published.stats_file),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|error| error.to_string())?;
    let published_counts: BTreeMap<String, u64> = stats
        .prepare("SELECT word, file_count FROM word_files")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)))?
                .collect()
        })
        .map_err(|error| error.to_string())?;
    let published_stems: BTreeMap<String, String> = stats
        .prepare("SELECT word, stem FROM word_stems")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect()
        })
        .map_err(|error| error.to_string())?;
    let expected_stems: BTreeMap<String, String> = word_stems(&published_counts).into_iter().collect();
    let stem_mismatches: Vec<serde_json::Value> = expected_stems
        .iter()
        .filter(|(word, stem)| published_stems.get(*word) != Some(*stem))
        .map(|(word, stem)| serde_json::json!({"word": word, "expected": stem, "published": published_stems.get(word)}))
        .collect();
    let unexpected_stem_rows = published_stems.keys().filter(|word| !expected_stems.contains_key(*word)).count();

    let count_files = |word: &str| -> Result<u64, String> {
        let clauses: Vec<(Occur, Box<dyn Query>)> = fields
            .iter()
            .map(|&field| {
                let query: Box<dyn Query> =
                    Box::new(TermQuery::new(Term::from_field_text(field, word), IndexRecordOption::Basic));
                (Occur::Should, query)
            })
            .collect();
        searcher
            .search(&BooleanQuery::new(clauses), &Count)
            .map(|count| count as u64)
            .map_err(|error| error.to_string())
    };

    let mut mismatches = Vec::new();
    for (word, &count) in &published_counts {
        let recount = count_files(word)?;
        if recount != count {
            mismatches.push(serde_json::json!({"word": word, "published": count, "recount": recount}));
        }
    }
    // Every dictionary term with a live document must be published; terms whose documents
    // are all deleted must not be.
    let mut missing_live_terms = Vec::new();
    let mut deleted_only_terms = 0u64;
    let mut seen: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
    let mut raw_doc_freq_exceeds_live = 0u64;
    for segment in searcher.segment_readers() {
        for &field in &fields {
            let inverted = segment.inverted_index(field).map_err(|error| error.to_string())?;
            let mut terms = inverted.terms().stream().map_err(|error| error.to_string())?;
            while terms.advance() {
                if !seen.insert(terms.key().to_vec()) {
                    continue;
                }
                let word = String::from_utf8_lossy(terms.key()).into_owned();
                if published_counts.contains_key(&word) {
                    continue;
                }
                if count_files(&word)? > 0 {
                    missing_live_terms.push(word);
                } else {
                    deleted_only_terms += 1;
                }
            }
        }
    }
    for (word, &count) in &published_counts {
        let raw: u64 = fields
            .iter()
            .map(|&field| searcher.doc_freq(&Term::from_field_text(field, word)).unwrap_or(0))
            .sum();
        raw_doc_freq_exceeds_live += u64::from(raw > count);
    }
    let live_docs = searcher.num_docs();
    let all_docs = searcher.search(&AllQuery, &Count).map_err(|error| error.to_string())? as u64;
    let is_pass = mismatches.is_empty()
        && missing_live_terms.is_empty()
        && live_docs == published.live_files
        && all_docs == published.live_files
        && stem_mismatches.is_empty()
        && unexpected_stem_rows == 0
        && published_stems.len() as u64 == published.stemmed_words;
    let mut digest = Sha256::new();
    for (word, count) in &published_counts {
        digest.update(format!("{word}\t{count}\n"));
    }
    Ok(serde_json::json!({
        "status": if is_pass { "pass" } else { "fail" },
        "index_generation": generation,
        "published_words": published_counts.len(),
        "recount_mismatches": mismatches.len(),
        "mismatch_examples": mismatches.into_iter().take(10).collect::<Vec<_>>(),
        "missing_live_terms": missing_live_terms.len(),
        "missing_examples": missing_live_terms.into_iter().take(10).collect::<Vec<_>>(),
        "deleted_only_terms_absent": deleted_only_terms,
        "words_where_raw_field_doc_freq_sum_exceeds_live_union": raw_doc_freq_exceeds_live,
        "denominator": {"published": published.live_files, "searcher_num_docs": live_docs, "all_query_count": all_docs},
        "stemmed_words": {"published": published.stemmed_words, "rows": published_stems.len(), "expected": expected_stems.len()},
        "stem_mismatches": stem_mismatches.len(),
        "stem_mismatch_examples": stem_mismatches.into_iter().take(10).collect::<Vec<_>>(),
        "unexpected_stem_rows": unexpected_stem_rows,
        "counts_sha256": hex(&digest.finalize()),
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
    }))
}

/// Raw per-field Tantivy `doc_freq` (includes deleted docs) next to the published live count.
pub fn raw_doc_freq(state_dir: &Path, words: &[String]) -> Result<serde_json::Value, String> {
    let index = Index::open_in_dir(state_dir.join(INDEX_DIR)).map_err(|error| error.to_string())?;
    let fields = stats_fields(&index)?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|error: tantivy::TantivyError| error.to_string())?;
    let searcher = reader.searcher();
    let mut rows = Vec::new();
    for word in words {
        let per_field: BTreeMap<&str, u64> = STATS_FIELDS
            .iter()
            .zip(&fields)
            .map(|(name, &field)| (*name, searcher.doc_freq(&Term::from_field_text(field, word)).unwrap_or(0)))
            .filter(|(_, count)| *count > 0)
            .collect();
        rows.push(serde_json::json!({"word": word, "raw_doc_freq_by_field": per_field}));
    }
    Ok(serde_json::json!({"segments": searcher.segment_readers().len(), "words": rows}))
}
