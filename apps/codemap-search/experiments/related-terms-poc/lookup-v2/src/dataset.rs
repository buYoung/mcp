//! Read-only reader for the generated `rt-dataset/1` SQLite artifact.
//!
//! The file is opened without create/write flags; a missing path is an error, never an
//! empty new database. Schema identity is validated before any lookup.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

pub const SCHEMA_VERSION: &str = "rt-dataset/1";
const REQUIRED_TABLES: &[&str] = &[
    "dataset_meta",
    "sources",
    "concepts",
    "expressions",
    "lexicalizations",
    "relations",
];
const REQUIRED_INDEXES: &[&str] = &[
    "expressions_by_token_key",
    "relations_by_from_expression",
    "lexicalizations_by_expression",
    "lexicalizations_by_concept",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetIdentity {
    pub dataset_id: String,
    pub schema_version: String,
    pub normalization_version: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ExpressionRow {
    pub expression_id: String,
    pub exact_key: String,
    pub token_key: String,
    pub surfaces: Vec<String>,
}

/// One relation leaving a matched expression, joined with the records that prove it.
#[derive(Debug, Clone)]
pub struct RelationRow {
    pub relation_id: String,
    pub relation_kind: String,
    pub evidence_kind: String,
    pub source_id: String,
    pub license_spdx: String,
    pub from_expression_id: String,
    pub to_expression_id: String,
    pub to_exact_key: String,
    pub to_token_key: String,
    pub to_surfaces: Vec<String>,
    pub from_surface: String,
    pub from_role: String,
    pub from_rank: i64,
    pub from_source_record: String,
    pub to_surface: String,
    pub to_source_record: String,
    pub from_concept_id: String,
    pub from_concept_kind: String,
    pub from_gloss: Option<String>,
    pub from_domain_labels: Vec<String>,
    pub to_concept_id: String,
    pub to_gloss: Option<String>,
}

pub struct Dataset {
    connection: Connection,
    pub identity: DatasetIdentity,
}

impl Dataset {
    pub fn open(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Err(format!("dataset unavailable: {} is not a file", path.display()));
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| format!("dataset unavailable: {error}"))?;
        let identity = validate(&connection, path)?;
        Ok(Self {
            connection,
            identity,
        })
    }

    pub fn expression_by_exact_key(&self, exact_key: &str) -> Result<Option<ExpressionRow>, String> {
        self.connection
            .prepare_cached(
                "SELECT expression_id, exact_key, token_key, surfaces_json FROM expressions WHERE exact_key = ?1",
            )
            .and_then(|mut statement| statement.query_row([exact_key], expression_row).optional())
            .map_err(|error| error.to_string())
    }

    pub fn expressions_by_token_key(&self, token_key: &str) -> Result<Vec<ExpressionRow>, String> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT expression_id, exact_key, token_key, surfaces_json FROM expressions \
                 WHERE token_key = ?1 AND token_key_lossy = 0 ORDER BY expression_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([token_key], expression_row)
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }

    pub fn concept_count(&self, expression_id: &str) -> Result<i64, String> {
        self.connection
            .prepare_cached("SELECT COUNT(DISTINCT concept_id) FROM lexicalizations WHERE expression_id = ?1")
            .and_then(|mut statement| statement.query_row([expression_id], |row| row.get(0)))
            .map_err(|error| error.to_string())
    }

    pub fn relations_from(&self, expression_id: &str) -> Result<Vec<RelationRow>, String> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT r.relation_id, r.relation_kind, r.evidence_kind, r.source_id, s.license_spdx,
                        r.from_expression_id, r.to_expression_id, te.exact_key, te.token_key, te.surfaces_json,
                        fl.surface, fl.role, fl.rank, fl.source_record, tl.surface, tl.source_record,
                        r.from_concept_id, fc.concept_kind, fc.gloss, fc.domain_labels_json,
                        r.to_concept_id, tc.gloss
                 FROM relations r
                 JOIN expressions te ON te.expression_id = r.to_expression_id
                 JOIN lexicalizations fl ON fl.lexicalization_id = r.from_lexicalization_id
                 JOIN lexicalizations tl ON tl.lexicalization_id = r.to_lexicalization_id
                 JOIN concepts fc ON fc.concept_id = r.from_concept_id
                 JOIN concepts tc ON tc.concept_id = r.to_concept_id
                 JOIN sources s ON s.source_id = r.source_id
                 WHERE r.from_expression_id = ?1
                 ORDER BY r.relation_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([expression_id], |row| {
                Ok(RelationRow {
                    relation_id: row.get(0)?,
                    relation_kind: row.get(1)?,
                    evidence_kind: row.get(2)?,
                    source_id: row.get(3)?,
                    license_spdx: row.get(4)?,
                    from_expression_id: row.get(5)?,
                    to_expression_id: row.get(6)?,
                    to_exact_key: row.get(7)?,
                    to_token_key: row.get(8)?,
                    to_surfaces: json_strings(&row.get::<_, String>(9)?),
                    from_surface: row.get(10)?,
                    from_role: row.get(11)?,
                    from_rank: row.get(12)?,
                    from_source_record: row.get(13)?,
                    to_surface: row.get(14)?,
                    to_source_record: row.get(15)?,
                    from_concept_id: row.get(16)?,
                    from_concept_kind: row.get(17)?,
                    from_gloss: row.get(18)?,
                    from_domain_labels: json_strings(&row.get::<_, String>(19)?),
                    to_concept_id: row.get(20)?,
                    to_gloss: row.get(21)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }

    /// Every expression with its stored keys, for the normalization conformance check.
    pub fn all_expressions(&self) -> Result<Vec<(String, String, bool)>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT exact_key, token_key, token_key_lossy FROM expressions ORDER BY expression_id")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? == 1)))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }

    pub fn all_surfaces(&self) -> Result<Vec<(String, String)>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT surface, expression_id FROM lexicalizations ORDER BY lexicalization_id")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }
}

fn expression_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExpressionRow> {
    Ok(ExpressionRow {
        expression_id: row.get(0)?,
        exact_key: row.get(1)?,
        token_key: row.get(2)?,
        surfaces: json_strings(&row.get::<_, String>(3)?),
    })
}

fn json_strings(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

fn validate(connection: &Connection, path: &Path) -> Result<DatasetIdentity, String> {
    let names: Vec<(String, String)> = connection
        .prepare("SELECT type, name FROM sqlite_master")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(|error| format!("dataset unavailable: {error}"))?;
    let has = |kind: &str, name: &str| names.iter().any(|(k, n)| k == kind && n == name);
    let missing: Vec<&str> = REQUIRED_TABLES
        .iter()
        .filter(|name| !has("table", name))
        .chain(REQUIRED_INDEXES.iter().filter(|name| !has("index", name)))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(format!("dataset incompatible: missing {missing:?}"));
    }
    let meta = |key: &str| -> Result<String, String> {
        connection
            .query_row("SELECT value FROM dataset_meta WHERE key = ?1", [key], |row| row.get(0))
            .map_err(|error| format!("dataset incompatible: dataset_meta.{key}: {error}"))
    };
    let identity = DatasetIdentity {
        dataset_id: meta("dataset_id")?,
        schema_version: meta("schema_version")?,
        normalization_version: meta("normalization_version")?,
        path: PathBuf::from(path).display().to_string(),
    };
    if identity.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "dataset incompatible: schema {} (reader supports {SCHEMA_VERSION})",
            identity.schema_version
        ));
    }
    if identity.normalization_version != crate::normalize::NORMALIZATION_VERSION {
        return Err(format!(
            "dataset incompatible: normalization {} (reader implements {})",
            identity.normalization_version,
            crate::normalize::NORMALIZATION_VERSION
        ));
    }
    Ok(identity)
}
