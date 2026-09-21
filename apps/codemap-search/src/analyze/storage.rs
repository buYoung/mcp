//! SQLite observations contain counts and paths, never query text or source content.
use super::FileObservation;
use anyhow::Context;
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) const DATABASE_PATH: &str = ".codemap/analysis.sqlite3";
pub(super) const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const RETENTION_MS: i64 = 30 * DAY_MS;
const APPLICATION_ID: i64 = 0x434d4150;

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn open(path: &Path, should_create: bool) -> anyhow::Result<Connection> {
    if should_create {
        std::fs::create_dir_all(path.parent().context("Missing database directory")?)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    // Bound telemetry contention; a locked/unwritable database must not fail a tool response.
    connection.busy_timeout(Duration::from_millis(250))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "secure_delete", "ON")?;
    // A deleted rollback journal does not retain old observations in a long-lived WAL.
    connection.pragma_update(None, "journal_mode", "DELETE")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let application: i64 =
        transaction.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if version == 0 && application == 0 {
        let tables: i64 = transaction.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        anyhow::ensure!(
            tables == 0,
            "Not a codemap-search analysis database: {}",
            path.display()
        );
        transaction.execute_batch(
            "CREATE TABLE calls (
                id INTEGER PRIMARY KEY,
                timestamp_ms INTEGER NOT NULL,
                tool TEXT NOT NULL CHECK (tool IN ('read', 'search', 'grep')),
                is_error INTEGER NOT NULL CHECK (is_error IN (0, 1)),
                response_bytes INTEGER NOT NULL CHECK (response_bytes >= 0),
                elapsed_ms REAL NOT NULL CHECK (elapsed_ms >= 0)
            );
            CREATE INDEX calls_by_time ON calls(timestamp_ms);
            CREATE TABLE file_observations (
                call_id INTEGER NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                size_bytes INTEGER,
                result_bytes INTEGER NOT NULL CHECK (result_bytes >= 0),
                PRIMARY KEY (call_id, path)
            );
            CREATE INDEX observations_by_path ON file_observations(path, call_id);",
        )?;
        transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
        transaction.pragma_update(None, "user_version", 1)?;
    } else {
        anyhow::ensure!(
            version == 1 && application == APPLICATION_ID,
            "Unsupported analysis database schema/application: {version}/{application}"
        );
    }
    transaction.commit()?;
    Ok(connection)
}

pub(super) fn open_existing(path: &Path) -> anyhow::Result<Option<Connection>> {
    match std::fs::metadata(path) {
        Ok(_) => open(path, false).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn purge_expired(connection: &Connection, now_ms: i64) -> rusqlite::Result<usize> {
    connection.execute(
        "DELETE FROM calls WHERE timestamp_ms < ?1",
        [now_ms - RETENTION_MS],
    )
}

pub(crate) struct CallRecorder {
    root: PathBuf,
    connection: Option<Connection>,
    is_enabled: bool,
    has_reported_failure: bool,
}

impl CallRecorder {
    pub(crate) fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            root: root.canonicalize().unwrap_or(root),
            connection: None,
            is_enabled: true,
            has_reported_failure: false,
        }
    }

    pub(crate) fn set_enabled(&mut self, is_enabled: bool) {
        self.is_enabled = is_enabled;
    }

    fn report(&mut self, result: anyhow::Result<()>) {
        if let Err(error) = result {
            if !self.has_reported_failure {
                tracing::warn!(%error, "SQLite usage recording/retention failed; tool responses are unaffected; retrying on next activity");
            }
            self.has_reported_failure = true;
        } else {
            self.has_reported_failure = false;
        }
    }

    /// Also runs when new records are disabled, and never creates an unused database.
    pub(crate) fn maintain(&mut self) {
        let result = (|| {
            if self.connection.is_none() {
                self.connection = open_existing(&self.root.join(DATABASE_PATH))?;
            }
            if let Some(connection) = &self.connection {
                purge_expired(connection, now_ms())?;
            }
            Ok(())
        })();
        self.report(result);
    }

    pub(crate) fn record(
        &mut self,
        tool: &str,
        files: &[FileObservation],
        is_error: bool,
        response_bytes: usize,
        elapsed_ms: f64,
    ) {
        if !self.is_enabled || !matches!(tool, "read" | "search" | "grep") {
            return;
        }
        let result = self.append(tool, files, is_error, response_bytes, elapsed_ms);
        self.report(result);
    }

    fn append(
        &mut self,
        tool: &str,
        files: &[FileObservation],
        is_error: bool,
        response_bytes: usize,
        elapsed_ms: f64,
    ) -> anyhow::Result<()> {
        let mut observations = BTreeMap::<String, (Option<u64>, u64)>::new();
        if !is_error {
            for file in files {
                let resolved = self
                    .root
                    .join(crate::workspace::path_from_workspace_input(&file.path));
                let canonical = resolved.canonicalize().unwrap_or(resolved);
                let path = canonical
                    .strip_prefix(&self.root)
                    .unwrap_or(&canonical)
                    .to_string_lossy()
                    .replace('\\', "/");
                let size_bytes = std::fs::metadata(&canonical)
                    .ok()
                    .filter(|metadata| metadata.is_file())
                    .map(|metadata| metadata.len());
                let entry = observations.entry(path).or_insert((size_bytes, 0));
                entry.1 = entry.1.saturating_add(file.result_bytes);
            }
        }
        if self.connection.is_none() {
            self.connection = Some(open(&self.root.join(DATABASE_PATH), true)?);
        }
        let connection = self.connection.as_mut().unwrap();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let timestamp_ms = now_ms();
        purge_expired(&transaction, timestamp_ms)?;
        transaction.execute(
            "INSERT INTO calls (timestamp_ms, tool, is_error, response_bytes, elapsed_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timestamp_ms, tool, is_error, i64::try_from(response_bytes)?, elapsed_ms],
        )?;
        let call_id = transaction.last_insert_rowid();
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO file_observations (call_id, path, size_bytes, result_bytes) VALUES (?1, ?2, ?3, ?4)"
            )?;
            for (path, (size_bytes, result_bytes)) in observations {
                let size_bytes = size_bytes.map(i64::try_from).transpose()?;
                statement.execute(params![
                    call_id,
                    path,
                    size_bytes,
                    i64::try_from(result_bytes)?
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}
