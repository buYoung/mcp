//! `rt-snake-parts/1`: snake_case part sequences of the indexed files (H14/H15, rtl-2.3).
//!
//! The index analyzer splits `is_test_file` into `is`, `test`, `file`, so `word_files` cannot
//! tell whether `test file` is ever written as `test_file`. `rtpoc2 snake-parts` (index time)
//! reads the live indexed files of the published generation once and records, for every run of
//! 2..=MAX_PARTS consecutive parts of a snake_case identifier, the number of files containing
//! it. Serving (`block`) reads only the resulting SQLite file, checked against the published
//! index generation, like the `rt-stats/2` statistics.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::stats::{sha256_file, RepositoryStats};

pub const FORMAT: &str = "rt-snake-parts/1";
pub const MAX_PARTS: usize = 4;
const RECORD_FILE: &str = "snake-parts.json";
const DIRECTORY: &str = "snake-parts";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Record {
    format: String,
    index_generation: String,
    file: String,
    sha256: String,
    max_parts: usize,
    files_scanned: u64,
    files_unreadable: u64,
    sequences: u64,
}

/// Lowercase part sequences (`test_file`, `is_test`, ...) of every snake_case identifier in
/// `text`: maximal `[A-Za-z0-9_]` runs that contain an underscore, split at underscores.
fn part_sequences(text: &[u8], sequences: &mut HashSet<String>) {
    for run in text.split(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_')) {
        if !run.contains(&b'_') {
            continue;
        }
        let parts: Vec<String> = run
            .split(|byte| *byte == b'_')
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).to_ascii_lowercase())
            .collect();
        for length in 2..=MAX_PARTS.min(parts.len()) {
            for window in parts.windows(length) {
                sequences.insert(window.join("_"));
            }
        }
    }
}

/// `rtpoc2 snake-parts --state DIR`: build the part-sequence table for the published generation.
pub fn build(state_dir: &Path) -> Result<serde_json::Value, String> {
    let repository = RepositoryStats::open(state_dir);
    let published = repository
        .published
        .as_ref()
        .filter(|_| repository.is_ready())
        .ok_or_else(|| format!("statistics not ready: {:?}", repository.state.status))?;
    let workspace = published.workspace["root"]
        .as_str()
        .map(PathBuf::from)
        .ok_or("published.json has no workspace root")?;
    let paths: Vec<String> = serde_json::from_slice(
        &std::fs::read(state_dir.join("indexed-files.json")).map_err(|error| format!("indexed-files.json: {error}"))?,
    )
    .map_err(|error| error.to_string())?;
    if paths.len() as u64 != published.live_files {
        return Err(format!("indexed-files.json lists {} files, published live_files {}", paths.len(), published.live_files));
    }
    // The table must describe the indexed contents: a file changed after the index commit
    // (meta.json) would add spellings the index never saw.
    let committed_at = std::fs::metadata(state_dir.join(crate::stats::INDEX_DIR).join("meta.json"))
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("index meta.json: {error}"))?;
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut files_unreadable = 0;
    for path in &paths {
        let file = workspace.join(path);
        if std::fs::metadata(&file).and_then(|metadata| metadata.modified()).is_ok_and(|modified| modified > committed_at) {
            return Err(format!("{path} changed after the index commit; re-run rtpoc2 index first"));
        }
        let Ok(bytes) = std::fs::read(&file) else {
            files_unreadable += 1;
            continue;
        };
        let mut sequences = HashSet::new();
        part_sequences(&bytes, &mut sequences);
        for sequence in sequences {
            *counts.entry(sequence).or_default() += 1;
        }
    }
    let directory = state_dir.join(DIRECTORY);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let name = format!("snake-parts-{}.sqlite3", &published.index_generation[..16]);
    let temporary = directory.join(format!("{name}.tmp"));
    let _ = std::fs::remove_file(&temporary);
    {
        let mut connection = Connection::open(&temporary).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
                 CREATE TABLE snake_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE snake_parts (sequence TEXT PRIMARY KEY, file_count INTEGER NOT NULL) WITHOUT ROWID;",
            )
            .map_err(|error| error.to_string())?;
        let transaction = connection.transaction().map_err(|error| error.to_string())?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO snake_parts (sequence, file_count) VALUES (?1, ?2)")
                .map_err(|error| error.to_string())?;
            for (sequence, count) in &counts {
                insert.execute((sequence, *count as i64)).map_err(|error| error.to_string())?;
            }
            let mut meta = transaction
                .prepare("INSERT INTO snake_meta (key, value) VALUES (?1, ?2)")
                .map_err(|error| error.to_string())?;
            for (key, value) in [
                ("format", FORMAT.to_string()),
                ("index_generation", published.index_generation.clone()),
                ("max_parts", MAX_PARTS.to_string()),
            ] {
                meta.execute((key, value)).map_err(|error| error.to_string())?;
            }
        }
        transaction.commit().map_err(|error| error.to_string())?;
    }
    std::fs::rename(&temporary, directory.join(&name)).map_err(|error| error.to_string())?;
    let file = format!("{DIRECTORY}/{name}");
    let record = Record {
        format: FORMAT.to_string(),
        index_generation: published.index_generation.clone(),
        sha256: sha256_file(&state_dir.join(&file)).map_err(|error| error.to_string())?,
        file,
        max_parts: MAX_PARTS,
        files_scanned: paths.len() as u64 - files_unreadable,
        files_unreadable,
        sequences: counts.len() as u64,
    };
    let body = serde_json::to_vec_pretty(&record).map_err(|error| error.to_string())?;
    let temporary_record = state_dir.join(format!("{RECORD_FILE}.tmp"));
    std::fs::write(&temporary_record, body).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary_record, state_dir.join(RECORD_FILE)).map_err(|error| error.to_string())?;
    serde_json::to_value(&record).map_err(|error| error.to_string())
}

/// Serving-side reader. `status` is `ready`, `missing`, `stale` (built for another index
/// generation), or `unavailable`; only a ready table answers lookups.
pub struct SnakeParts {
    pub status: &'static str,
    pub detail: Option<String>,
    pub sha256: Option<String>,
    connection: Option<Connection>,
}

impl SnakeParts {
    fn not_ready(status: &'static str, detail: String) -> Self {
        Self { status, detail: Some(detail), sha256: None, connection: None }
    }

    pub fn open(state_dir: &Path, repository: &RepositoryStats) -> Self {
        let Some(published) = repository.published.as_ref().filter(|_| repository.is_ready()) else {
            return Self::not_ready("unavailable", "repository statistics not ready".to_string());
        };
        let record: Record = match std::fs::read(state_dir.join(RECORD_FILE)) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(record) => record,
                Err(error) => return Self::not_ready("unavailable", format!("{RECORD_FILE} unreadable: {error}")),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Self::not_ready("missing", "rtpoc2 snake-parts was not run for this state".to_string())
            }
            Err(error) => return Self::not_ready("unavailable", error.to_string()),
        };
        if record.format != FORMAT || record.index_generation != published.index_generation {
            return Self::not_ready("stale", format!("{} for generation {}", record.format, record.index_generation));
        }
        let path = state_dir.join(&record.file);
        match sha256_file(&path) {
            Ok(actual) if actual == record.sha256 => {}
            Ok(_) => return Self::not_ready("unavailable", "snake-parts file hash differs from its record".to_string()),
            Err(error) => return Self::not_ready("unavailable", error.to_string()),
        }
        match Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX) {
            Ok(connection) => Self { status: "ready", detail: None, sha256: Some(record.sha256), connection: Some(connection) },
            Err(error) => Self::not_ready("unavailable", error.to_string()),
        }
    }

    /// Files whose snake_case identifiers contain `parts` consecutively; `None` when not ready.
    pub fn file_count(&self, parts: &[String]) -> Option<u64> {
        let connection = self.connection.as_ref()?;
        connection
            .prepare_cached("SELECT file_count FROM snake_parts WHERE sequence = ?1")
            .and_then(|mut statement| statement.query_row([parts.join("_")], |row| row.get::<_, i64>(0)).optional())
            .ok()
            .map(|count| count.unwrap_or(0) as u64)
    }
}
