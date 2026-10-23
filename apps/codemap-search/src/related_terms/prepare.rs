use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const INDEX_VERSION: u32 = 1;
const READY_MARKER: &str = "ready.json";
const INDEX_SQL: &str = "CREATE INDEX IF NOT EXISTS related_terms_by_word ON relations(word_key);
CREATE INDEX IF NOT EXISTS related_terms_by_persona ON relations(persona);
CREATE INDEX IF NOT EXISTS related_terms_personas_by_topic ON personas(topic);";

#[derive(Clone, Debug)]
pub struct PreparedDataset {
    path: PathBuf,
    dataset_id: String,
    source: Option<SourceIdentity>,
}

impl PreparedDataset {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn dataset_id(&self) -> &str {
        &self.dataset_id
    }

    pub(super) fn open(&self) -> Result<Connection, String> {
        let parent = self.path.parent().ok_or("prepared dataset has no parent")?;
        let _lock = prepare_lock(parent)?;
        let marker = read_marker(parent).ok_or("related-terms dataset is not ready")?;
        if marker.dataset_id != self.dataset_id || marker.index_version != INDEX_VERSION {
            return Err("related-terms prepared marker is incompatible".into());
        }
        if marker.source != self.source {
            return Err("prepared dataset source changed; prepare the dataset again".into());
        }
        let connection = open_readonly(&self.path)?;
        if dataset_identity(&connection)? != self.dataset_id {
            return Err("prepared dataset identity changed".into());
        }
        connection.set_prepared_statement_cache_capacity(32);
        Ok(connection)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct SourceIdentity {
    path: PathBuf,
    bytes: u64,
    modified_nanos: u128,
}

#[derive(Serialize, Deserialize)]
struct ReadyMarker {
    dataset_id: String,
    index_version: u32,
    source: Option<SourceIdentity>,
}

fn source_identity(path: &Path) -> Result<SourceIdentity, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let modified_nanos = metadata
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    Ok(SourceIdentity {
        path: path.to_path_buf(),
        bytes: metadata.len(),
        modified_nanos,
    })
}

fn read_marker(directory: &Path) -> Option<ReadyMarker> {
    serde_json::from_slice(&fs::read(directory.join(READY_MARKER)).ok()?).ok()
}

pub(super) fn open_readonly(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(30))
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

fn dataset_identity(connection: &Connection) -> Result<String, String> {
    let meta = |key| {
        connection
            .query_row(
                "SELECT value FROM sense_meta WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())
    };
    let schema = meta("schema_version")?;
    if schema != "rt-sense/5" {
        return Err(format!(
            "unsupported related-terms schema {schema:?}; expected rt-sense/5"
        ));
    }
    let id = meta("dataset_id")?;
    if id.is_empty()
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("invalid related-terms dataset_id".into());
    }
    Ok(id)
}

fn prepare_lock(directory: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(directory.join("prepare.lock"))
        .map_err(|e| e.to_string())?;
    for attempt in 0..3000 {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if attempt < 2999 => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(format!("cannot lock related-terms preparation: {e}")),
        }
    }
    Err("related-terms preparation is busy".into())
}

/// Blocking preparation belongs on the startup worker, never in the sequential MCP loop.
/// The source is copied before indexing; a user-owned source is never opened writable.
pub fn prepare_dataset(
    source: &Path,
    global_dir: &Path,
    is_user_owned: bool,
) -> Result<PreparedDataset, String> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let dataset_id = dataset_identity(&open_readonly(&source)?)?;
    let root = global_dir.join("related-terms");
    let directory = if is_user_owned {
        root.join("local").join(&dataset_id)
    } else {
        root.join(&dataset_id)
    };
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let path = directory.join("dataset.sqlite3");
    if is_user_owned && source == path {
        return Err("user source must not be the managed prepared copy".into());
    }
    let _lock = prepare_lock(&directory)?;
    let source_stamp = if is_user_owned {
        Some(source_identity(&source)?)
    } else {
        None
    };
    let old_marker = read_marker(&directory);
    if path.is_file()
        && old_marker.as_ref().is_some_and(|m| {
            m.dataset_id == dataset_id
                && m.index_version == INDEX_VERSION
                && m.source == source_stamp
        })
    {
        return Ok(PreparedDataset {
            path,
            dataset_id,
            source: source_stamp,
        });
    }
    let rebuild_existing = path.is_file()
        && old_marker
            .as_ref()
            .is_some_and(|m| m.dataset_id == dataset_id && m.source == source_stamp);
    // Invalidate readiness before changing a prepared generation. Readers take this same
    // lock when opening; existing readers retain their old SQLite file descriptor.
    match fs::remove_file(directory.join(READY_MARKER)) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
    fs::copy(
        if rebuild_existing { &path } else { &source },
        temporary.path(),
    )
    .map_err(|e| e.to_string())?;
    // A read-only source can be supplied; only our owned temporary receives write permission.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(temporary.path())
            .map_err(|e| e.to_string())?
            .permissions();
        permissions.set_readonly(false);
        fs::set_permissions(temporary.path(), permissions).map_err(|e| e.to_string())?;
    }
    let connection = Connection::open(temporary.path()).map_err(|e| e.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(30))
        .map_err(|e| e.to_string())?;
    if dataset_identity(&connection)? != dataset_id {
        return Err("dataset changed during preparation".into());
    }
    let rebuild = if old_marker
        .as_ref()
        .is_some_and(|marker| marker.index_version != INDEX_VERSION)
    {
        "DROP INDEX IF EXISTS related_terms_by_word; DROP INDEX IF EXISTS related_terms_by_persona; DROP INDEX IF EXISTS related_terms_personas_by_topic;"
    } else {
        ""
    };
    connection
        .execute_batch(&format!("BEGIN IMMEDIATE; {rebuild} {INDEX_SQL} COMMIT;"))
        .map_err(|e| e.to_string())?;
    connection.close().map_err(|(_, e)| e.to_string())?;
    if is_user_owned && source_stamp.as_ref() != Some(&source_identity(&source)?) {
        return Err("user dataset changed while it was being copied".into());
    }
    temporary.as_file_mut().flush().map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(&path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    File::open(&directory)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    let marker = ReadyMarker {
        dataset_id: dataset_id.clone(),
        index_version: INDEX_VERSION,
        source: source_stamp.clone(),
    };
    let mut ready = tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
    ready
        .write_all(&serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    ready.as_file().sync_all().map_err(|e| e.to_string())?;
    ready
        .persist(directory.join(READY_MARKER))
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    File::open(&directory)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(PreparedDataset {
        path,
        dataset_id,
        source: source_stamp,
    })
}
