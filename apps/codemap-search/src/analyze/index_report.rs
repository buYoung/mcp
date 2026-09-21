//! Inspect one committed Tantivy snapshot. SQL aggregation uses a disposable in-memory database.
use super::options::{Options, View};
use super::report::{Cell, Dataset, Report};
use crate::parser::ExtractedFile;
use anyhow::Context;
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;
use tantivy::schema::Value;
use tantivy::{DocAddress, Index, IndexReader, ReloadPolicy, TantivyDocument};

fn source_metadata(
    root: &Path,
    path: &str,
    indexed_mtime_ns: Option<u64>,
    is_mcp: bool,
) -> (Option<i64>, &'static str) {
    let path = root.join(crate::workspace::path_from_workspace_input(path));
    if is_mcp
        && crate::workspace::resolve_for_filesystem_tool(
            &path.to_string_lossy(),
            crate::workspace::FilesystemTool::Read,
        )
        .is_err()
    {
        return (None, "restricted");
    }
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            let mtime_ns = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_nanos() as u64);
            let state = match (indexed_mtime_ns, mtime_ns) {
                (Some(indexed), Some(current)) if indexed == current => "unchanged",
                (Some(_), Some(_)) => "changed",
                _ => "unknown",
            };
            (i64::try_from(metadata.len()).ok(), state)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, "missing"),
        _ => (None, "unavailable"),
    }
}

fn directory_size(path: &Path) -> (i64, usize) {
    let mut bytes = 0i64;
    let mut unavailable = 0;
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        match entry {
            Ok(entry) if entry.file_type().is_file() => match entry.metadata() {
                Ok(metadata) => {
                    bytes = bytes.saturating_add(i64::try_from(metadata.len()).unwrap_or(i64::MAX))
                }
                Err(_) => unavailable += 1,
            },
            Err(_) => unavailable += 1,
            _ => {}
        }
    }
    (bytes, unavailable)
}

pub(super) fn collect(root: &Path, index_path: &Path, options: &Options) -> anyhow::Result<Report> {
    let mut report = Report::new(options, "ok");
    report.metadata["scope"] = json!({"filter": options.filter, "language": options.language});
    if !index_path.join("meta.json").try_exists()? {
        report.metadata["status"] = json!("index_missing");
        report
            .notes
            .push("No committed index yet; start MCP or run codemap-search index first.".into());
        return Ok(report);
    }
    // Never use TantivySearchEngine::new here: it can create, upgrade or rebuild an index.
    let index = Index::open_in_dir(index_path).context("Opening the existing Tantivy index")?;
    let schema = index.schema();
    let extracted_field = schema.get_field("extracted_json")?;
    let mtime_field = schema.get_field("mtime")?;
    let path_field = schema.get_field("file_path")?;
    let reader: IndexReader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    let searcher = reader.searcher();
    let mut database = Connection::open_in_memory()?;
    database.execute_batch(
        "CREATE TABLE index_files (
            path TEXT PRIMARY KEY, language TEXT NOT NULL, size_bytes INTEGER, lines INTEGER NOT NULL,
            symbols INTEGER NOT NULL, exported INTEGER NOT NULL, literals INTEGER NOT NULL,
            docstrings INTEGER NOT NULL, calls INTEGER NOT NULL, references_count INTEGER NOT NULL,
            payload_bytes INTEGER NOT NULL, largest_literal_bytes INTEGER NOT NULL, state TEXT NOT NULL
         );
         CREATE TABLE symbol_counts (kind TEXT NOT NULL, symbols INTEGER NOT NULL, exported INTEGER NOT NULL,
                                     tests INTEGER NOT NULL, documented INTEGER NOT NULL);"
    )?;
    let transaction = database.transaction()?;
    {
        let mut insert_file = transaction.prepare(
            "INSERT INTO index_files VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        )?;
        let mut insert_kind =
            transaction.prepare("INSERT INTO symbol_counts VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for (segment_ordinal, segment) in searcher.segment_readers().iter().enumerate() {
            for document_id in segment.doc_ids_alive() {
                let document = searcher
                    .doc::<TantivyDocument>(DocAddress::new(segment_ordinal as u32, document_id))?;
                let path = document
                    .get_first(path_field)
                    .and_then(|value| value.as_str())
                    .context("Stored document is missing file_path")?;
                let language =
                    crate::lang::language_name_for_path(Path::new(path)).unwrap_or("unknown");
                if options
                    .filter
                    .as_ref()
                    .is_some_and(|filter| !path.contains(filter))
                    || options
                        .language
                        .as_deref()
                        .is_some_and(|filter| filter != language)
                {
                    continue;
                }
                let json = document
                    .get_first(extracted_field)
                    .and_then(|value| value.as_str())
                    .context("Stored document is missing extracted_json")?;
                let extracted: ExtractedFile =
                    serde_json::from_str(json).context("Decoding stored extraction metadata")?;
                let indexed_mtime_ns = document
                    .get_first(mtime_field)
                    .and_then(|value| value.as_u64());
                let (size_bytes, state) =
                    source_metadata(root, &extracted.file_path, indexed_mtime_ns, options.is_mcp);
                let mut kinds = BTreeMap::<&str, (i64, i64, i64, i64)>::new();
                for symbol in &extracted.symbols {
                    let counts = kinds.entry(&symbol.kind).or_default();
                    counts.0 += 1;
                    counts.1 += i64::from(symbol.flags.is_exported);
                    counts.2 += i64::from(symbol.flags.is_test);
                    counts.3 += i64::from(
                        symbol
                            .docstring
                            .as_ref()
                            .is_some_and(|value| !value.trim().is_empty()),
                    );
                }
                let exported = kinds.values().map(|counts| counts.1).sum::<i64>();
                for (kind, counts) in kinds {
                    insert_kind.execute(params![kind, counts.0, counts.1, counts.2, counts.3])?;
                }
                let calls = extracted
                    .navigation
                    .as_ref()
                    .map_or(0, |navigation| navigation.calls.len());
                let references = extracted
                    .navigation
                    .as_ref()
                    .map_or(0, |navigation| navigation.references.len());
                let largest_literal_bytes = extracted
                    .literals
                    .iter()
                    .map(|literal| literal.text.len())
                    .max()
                    .unwrap_or(0);
                insert_file.execute(params![
                    extracted.file_path,
                    language,
                    size_bytes,
                    extracted.total_lines as i64,
                    extracted.symbols.len() as i64,
                    exported,
                    extracted.literals.len() as i64,
                    extracted.docstrings.len() as i64,
                    calls as i64,
                    references as i64,
                    json.len() as i64,
                    largest_literal_bytes as i64,
                    state
                ])?;
            }
        }
    }
    transaction.commit()?;

    let totals: Vec<i64> = database.query_row(
        "SELECT count(*), coalesce(sum(size_bytes), 0), coalesce(sum(payload_bytes), 0),
                coalesce(sum(calls), 0), coalesce(sum(references_count), 0), coalesce(sum(size_bytes IS NULL), 0),
                coalesce(sum(lines), 0), coalesce(sum(symbols), 0), coalesce(sum(exported), 0),
                coalesce(sum(literals), 0), coalesce(sum(docstrings), 0) FROM index_files",
        [], |row| (0..11).map(|column| row.get(column)).collect(),
    )?;
    let (disk_bytes, unavailable) = directory_size(index_path);
    let deleted_documents = searcher
        .segment_readers()
        .iter()
        .map(|segment| i64::from(segment.max_doc() - segment.num_docs()))
        .sum::<i64>();
    report.summary = vec![
        ("files", "Matching files", totals[0].into()),
        ("lines", "Indexed physical lines", totals[6].into()),
        ("symbols", "Symbols", totals[7].into()),
        ("exported", "Exported symbols", totals[8].into()),
        ("literals", "Literals", totals[9].into()),
        ("docstrings", "Docstring entries", totals[10].into()),
        (
            "source_bytes",
            "Current source sizes",
            Cell::bytes(Some(totals[1])),
        ),
        (
            "stored_json_bytes",
            "Stored extraction JSON",
            Cell::bytes(Some(totals[2])),
        ),
        (
            "unknown_sizes",
            "Unavailable source sizes",
            totals[5].into(),
        ),
        ("call_sites", "Static call sites", totals[3].into()),
        (
            "reference_sites",
            "Static reference sites",
            totals[4].into(),
        ),
        (
            "index_files",
            "Whole-index files",
            (searcher.num_docs() as i64).into(),
        ),
        (
            "index_disk_bytes",
            "Whole-index disk size",
            Cell::bytes(Some(disk_bytes)),
        ),
        (
            "index_unavailable_entries",
            "Unavailable index entries",
            (unavailable as i64).into(),
        ),
        (
            "index_segments",
            "Whole-index segments",
            (searcher.segment_readers().len() as i64).into(),
        ),
        (
            "index_deleted_documents",
            "Whole-index deleted documents",
            deleted_documents.into(),
        ),
    ];
    if totals[0] == 0 {
        report.metadata["status"] = json!("no_matches");
    }
    if options.view.has_groups() {
        let mut statement = database.prepare(
            "SELECT language, count(*), sum(lines), sum(symbols), sum(exported), sum(literals), sum(docstrings),
                    coalesce(sum(size_bytes), 0), sum(payload_bytes) FROM index_files GROUP BY language ORDER BY count(*) DESC, language"
        )?;
        let rows = statement
            .query_map([], |row| {
                let mut values = vec![Cell::from(row.get::<_, String>(0)?)];
                for column in 1..7 {
                    values.push(row.get::<_, i64>(column)?.into());
                }
                values.push(Cell::bytes(Some(row.get(7)?)));
                values.push(Cell::bytes(Some(row.get(8)?)));
                Ok(values)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        report.tables.push(Dataset::new(
            "languages",
            "Indexed languages",
            &[
                ("language", "Language", false),
                ("files", "Files", true),
                ("lines", "Lines", true),
                ("symbols", "Symbols", true),
                ("exported", "Exported", true),
                ("literals", "Literals", true),
                ("docstrings", "Docs", true),
                ("source_bytes", "Source size", true),
                ("stored_json_bytes", "Stored JSON", true),
            ],
            rows,
        ));
        let mut statement = database
            .prepare("SELECT state, count(*) FROM index_files GROUP BY state ORDER BY state")?;
        let rows = statement
            .query_map([], |row| {
                Ok(vec![
                    row.get::<_, i64>(1)?.into(),
                    row.get::<_, String>(0)?.into(),
                ])
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        report.tables.push(Dataset::new(
            "freshness",
            "Source freshness (mtime only)",
            &[("files", "Files", true), ("state", "State", false)],
            rows,
        ));
    }
    if options.view == View::Full {
        let mut statement = database.prepare(
            "SELECT kind, sum(symbols), sum(exported), sum(tests), sum(documented) FROM symbol_counts GROUP BY kind ORDER BY sum(symbols) DESC, kind"
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(vec![
                    row.get::<_, String>(0)?.into(),
                    row.get::<_, i64>(1)?.into(),
                    row.get::<_, i64>(2)?.into(),
                    row.get::<_, i64>(3)?.into(),
                    row.get::<_, i64>(4)?.into(),
                ])
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        report.tables.push(Dataset::new(
            "symbol_kinds",
            "Indexed symbol kinds",
            &[
                ("kind", "Kind", false),
                ("symbols", "Symbols", true),
                ("exported", "Exported", true),
                ("tests", "Test flags", true),
                ("documented", "Documented", true),
            ],
            rows,
        ));
    }
    if options.view.has_files() {
        let mut statement = database.prepare(&format!(
            "SELECT size_bytes, lines, symbols, literals, largest_literal_bytes, payload_bytes, state, path
             FROM index_files ORDER BY {} LIMIT ?1 OFFSET ?2", options.index_order(),
        ))?;
        let rows = statement
            .query_map(params![options.sql_limit(), options.sql_offset()], |row| {
                Ok(vec![
                    Cell::bytes(row.get(0)?),
                    row.get::<_, i64>(1)?.into(),
                    row.get::<_, i64>(2)?.into(),
                    row.get::<_, i64>(3)?.into(),
                    Cell::bytes(Some(row.get(4)?)),
                    Cell::bytes(Some(row.get(5)?)),
                    row.get::<_, String>(6)?.into(),
                    row.get::<_, String>(7)?.into(),
                ])
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        report.page(options, totals[0], rows.len());
        report.tables.push(Dataset::new(
            "files",
            "Indexed files",
            &[
                ("size_bytes", "File size", true),
                ("lines", "Lines", true),
                ("symbols", "Symbols", true),
                ("literals", "Literals", true),
                ("max_literal_bytes", "Max literal", true),
                ("stored_json_bytes", "Stored JSON", true),
                ("state", "State", false),
                ("path", "Path", false),
            ],
            rows,
        ));
    }
    report.notes.push(
        "File metrics follow filters; index_* metrics cover the whole committed index.".into(),
    );
    report.notes.push("Counts come from stored extraction; size/mtime use current metadata. Unchanged mtime does not prove identical content.".into());
    report.notes.push("Stored JSON is uncompressed metadata, not compressed index size or model tokens; ?/null size means unavailable.".into());
    Ok(report)
}
