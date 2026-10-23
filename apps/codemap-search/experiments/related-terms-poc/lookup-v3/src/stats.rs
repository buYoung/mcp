//! Serving-side access to published repository statistics.
//!
//! Lookup reads only `published.json`, the stats SQLite file it names, and the index's
//! `meta.json` commit point (to detect a newer, unpublished generation). It never opens
//! Tantivy segments, walks source files, recomputes counts, or stems the vocabulary: word
//! stems are read from the `word_stems` table written at index time.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};

/// `rt-stats/2` = `rt-stats/1` word counts plus the index-time `word_stems` table.
pub const STATS_FORMAT: &str = "rt-stats/2";
/// Union of every searchable TEXT field written by `TantivySearchEngine::apply_index_updates`.
/// `file_path` (raw STRING key) and stored-only fields are not searchable text.
pub const STATS_FIELDS: [&str; 7] = [
    "file_path_parts",
    "symbol",
    "docstring",
    "literal",
    "definition_body",
    "reference",
    "format_text",
];
pub const FIELD_POLICY: &str = "rts-fields-1:file_path_parts+symbol+docstring+literal+definition_body+reference+format_text";
pub const PUBLISHED_FILE: &str = "published.json";
pub const INDEX_DIR: &str = "index";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublishedStats {
    pub stats_format: String,
    pub field_policy: String,
    pub analyzer_policy: String,
    /// SHA-256 of the index `meta.json` the statistics were materialized from.
    pub index_generation: String,
    pub index_opstamp: u64,
    pub stats_file: String,
    pub stats_sha256: String,
    pub live_files: u64,
    pub vocabulary_size: u64,
    /// Absent in `rt-stats/1` publications, which are then reported as incompatible.
    #[serde(default)]
    pub stem_policy: String,
    #[serde(default)]
    pub stemmed_words: u64,
    pub workspace: serde_json::Value,
    pub finalization: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryStatus {
    Ready,
    /// Published statistics belong to an older committed index generation.
    Stale,
    /// No statistics were ever published for this state directory.
    Missing,
    /// Statistics exist but use another format/field/analyzer policy.
    Incompatible,
    /// Statistics or index metadata could not be read or failed integrity checks.
    Unavailable,
    /// The caller did not supply a repository state directory.
    NotConfigured,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepositoryState {
    pub status: RepositoryStatus,
    pub detail: Option<String>,
    pub published_generation: Option<String>,
    pub current_generation: Option<String>,
    pub live_files: Option<u64>,
    pub vocabulary_size: Option<u64>,
    pub field_policy: Option<String>,
    pub analyzer_policy: Option<String>,
}

pub struct RepositoryStats {
    pub state: RepositoryState,
    /// The publication record when one was read, ready or not (identity for reports).
    pub published: Option<PublishedStats>,
    connection: Option<Connection>,
}

impl RepositoryStats {
    pub fn not_configured() -> Self {
        Self::not_ready(RepositoryStatus::NotConfigured, None, None, None)
    }

    fn not_ready(
        status: RepositoryStatus,
        detail: Option<String>,
        published: Option<&PublishedStats>,
        current_generation: Option<String>,
    ) -> Self {
        Self {
            state: RepositoryState {
                status,
                detail,
                published_generation: published.map(|p| p.index_generation.clone()),
                current_generation,
                live_files: None,
                vocabulary_size: None,
                field_policy: published.map(|p| p.field_policy.clone()),
                analyzer_policy: published.map(|p| p.analyzer_policy.clone()),
            },
            published: published.cloned(),
            connection: None,
        }
    }

    /// Open the published statistics for `state_dir`, classifying every not-ready case.
    pub fn open(state_dir: &Path) -> Self {
        let published_path = state_dir.join(PUBLISHED_FILE);
        let published: PublishedStats = match std::fs::read(&published_path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(published) => published,
                Err(error) => {
                    return Self::not_ready(
                        RepositoryStatus::Unavailable,
                        Some(format!("published.json unreadable: {error}")),
                        None,
                        None,
                    )
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Self::not_ready(RepositoryStatus::Missing, Some("no published statistics".into()), None, None)
            }
            Err(error) => {
                return Self::not_ready(RepositoryStatus::Unavailable, Some(error.to_string()), None, None)
            }
        };
        if published.stats_format != STATS_FORMAT
            || published.field_policy != FIELD_POLICY
            || published.analyzer_policy != crate::normalize::ANALYZER_POLICY
            || published.stem_policy != crate::normalize::STEM_POLICY
        {
            return Self::not_ready(
                RepositoryStatus::Incompatible,
                Some(format!(
                    "published {}/{}/{}/{}; reader expects {STATS_FORMAT}/{FIELD_POLICY}/{}/{}",
                    published.stats_format,
                    published.field_policy,
                    published.analyzer_policy,
                    published.stem_policy,
                    crate::normalize::ANALYZER_POLICY,
                    crate::normalize::STEM_POLICY
                )),
                Some(&published),
                None,
            );
        }
        let current_generation = match index_generation(&state_dir.join(INDEX_DIR)) {
            Ok(generation) => generation,
            Err(error) => {
                return Self::not_ready(RepositoryStatus::Unavailable, Some(error), Some(&published), None)
            }
        };
        if current_generation.0 != published.index_generation {
            return Self::not_ready(
                RepositoryStatus::Stale,
                Some(format!(
                    "index committed opstamp {} after statistics for opstamp {} were published",
                    current_generation.1, published.index_opstamp
                )),
                Some(&published),
                Some(current_generation.0),
            );
        }
        let stats_path = state_dir.join(&published.stats_file);
        let integrity = sha256_file(&stats_path)
            .map_err(|error| format!("stats file unreadable: {error}"))
            .and_then(|actual| {
                (actual == published.stats_sha256)
                    .then_some(())
                    .ok_or_else(|| "stats file hash differs from published.json".to_string())
            })
            .and_then(|()| {
                Connection::open_with_flags(
                    &stats_path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )
                .map_err(|error| format!("stats file unopenable: {error}"))
            })
            .and_then(|connection| {
                let generation: String = connection
                    .query_row("SELECT value FROM stats_meta WHERE key = 'index_generation'", [], |row| row.get(0))
                    .map_err(|error| format!("stats meta unreadable: {error}"))?;
                (generation == published.index_generation)
                    .then_some(connection)
                    .ok_or_else(|| "stats file generation differs from published.json".to_string())
            });
        match integrity {
            Ok(connection) => Self {
                state: RepositoryState {
                    status: RepositoryStatus::Ready,
                    detail: None,
                    published_generation: Some(published.index_generation.clone()),
                    current_generation: Some(current_generation.0),
                    live_files: Some(published.live_files),
                    vocabulary_size: Some(published.vocabulary_size),
                    field_policy: Some(published.field_policy.clone()),
                    analyzer_policy: Some(published.analyzer_policy.clone()),
                },
                published: Some(published),
                connection: Some(connection),
            },
            Err(error) => Self::not_ready(
                RepositoryStatus::Unavailable,
                Some(error),
                Some(&published),
                Some(current_generation.0),
            ),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state.status, RepositoryStatus::Ready)
    }

    /// Unique live indexed files containing `word` in any searchable field. `None` when not ready.
    pub fn file_count(&self, word: &str) -> Option<u64> {
        let connection = self.connection.as_ref()?;
        connection
            .prepare_cached("SELECT file_count FROM word_files WHERE word = ?1")
            .and_then(|mut statement| statement.query_row([word], |row| row.get::<_, i64>(0)).optional())
            .ok()
            .map(|count| count.unwrap_or(0) as u64)
    }

    /// Repository file count of a term given its index tokens: a single token's own count; for
    /// several tokens, the concatenated identifier's count (`event loop` -> `eventloop`) when
    /// it is present, otherwise the smallest component count. `None` when not ready.
    pub fn term_file_count(&self, index_tokens: &[String]) -> Option<u64> {
        match index_tokens {
            [] => None,
            [only] => self.file_count(only),
            tokens => {
                let concatenated = self.file_count(&tokens.concat())?;
                if concatenated > 0 {
                    return Some(concatenated);
                }
                tokens.iter().map(|token| self.file_count(token)).min().flatten()
            }
        }
    }

    /// Whether `word` has an index-time stem row (an alphabetic live word of length >= 3).
    pub fn has_stemmed_word(&self, word: &str) -> bool {
        let Some(connection) = self.connection.as_ref() else {
            return false;
        };
        connection
            .prepare_cached("SELECT 1 FROM word_stems WHERE word = ?1")
            .and_then(|mut statement| statement.exists([word]))
            .unwrap_or(false)
    }

    /// Live words sharing `stem`, with their file counts, most frequent first.
    /// `None` when not ready.
    pub fn words_with_stem(&self, stem: &str) -> Option<Vec<(String, u64)>> {
        let connection = self.connection.as_ref()?;
        connection
            .prepare_cached(
                "SELECT s.word, f.file_count FROM word_stems s JOIN word_files f ON f.word = s.word \
                 WHERE s.stem = ?1 ORDER BY f.file_count DESC, s.word",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([stem], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)))?
                    .collect()
            })
            .ok()
    }
}

/// SHA-256 of the committed `meta.json` plus its opstamp. Reads one small file only.
pub fn index_generation(index_dir: &Path) -> Result<(String, u64), String> {
    let path: PathBuf = index_dir.join("meta.json");
    let bytes = std::fs::read(&path).map_err(|error| format!("index meta unreadable: {error}"))?;
    let opstamp = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|meta| meta.get("opstamp").and_then(|value| value.as_u64()))
        .ok_or_else(|| "index meta has no opstamp".to_string())?;
    Ok((hex(&Sha256::digest(&bytes)), opstamp))
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex(&Sha256::digest(&bytes)))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
