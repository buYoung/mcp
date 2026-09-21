use super::options::{Options, View};
use super::report::{Cell, Dataset, Report};
use super::storage::DAY_MS;
use rusqlite::{params, Connection};
use serde_json::json;

// A path filter selects whole calls for response metrics, but only matching files for
// file metrics. EXISTS avoids multiplying call bytes when several returned files match.
const CALL_FILTER: &str = "c.timestamp_ms >= ?1 AND c.timestamp_ms < ?2
    AND (?3 IS NULL OR c.tool = ?3)
    AND (?4 IS NULL OR EXISTS (SELECT 1 FROM file_observations f
                              WHERE f.call_id = c.id AND instr(f.path, ?4) > 0))";
const FILE_SUMMARY: &str = "WITH observations AS (
    SELECT f.*, c.timestamp_ms, c.tool,
           row_number() OVER (PARTITION BY f.path ORDER BY c.timestamp_ms DESC, c.id DESC) AS recency
    FROM file_observations f JOIN calls c ON c.id = f.call_id
    WHERE c.timestamp_ms >= ?1 AND c.timestamp_ms < ?2
      AND (?3 IS NULL OR c.tool = ?3) AND (?4 IS NULL OR instr(f.path, ?4) > 0)
)
SELECT path, max(CASE WHEN recency = 1 THEN size_bytes END) AS size_bytes,
       count(*) AS reads, sum(tool = 'read') AS read_calls, sum(tool = 'search') AS search_calls,
       sum(tool = 'grep') AS grep_calls, sum(result_bytes) AS result_bytes,
       count(DISTINCT date(timestamp_ms / 1000, 'unixepoch')) AS active_days,
       strftime('%Y-%m-%d %H:%M:%S', max(timestamp_ms) / 1000, 'unixepoch') AS last_seen
FROM observations GROUP BY path";

#[derive(Default)]
struct Metrics {
    calls: i64,
    errors: i64,
    content_calls: i64,
    files: i64,
    reads: i64,
    response_bytes: i64,
    result_bytes: i64,
    average_ms: f64,
    maximum_ms: f64,
}

fn metrics(
    connection: &Connection,
    start_ms: i64,
    end_ms: i64,
    tool: Option<&str>,
    filter: Option<&str>,
) -> rusqlite::Result<Metrics> {
    let mut metrics = connection.query_row(
        &format!(
            "SELECT count(*), coalesce(sum(is_error), 0), coalesce(sum(response_bytes), 0),
                coalesce(avg(elapsed_ms), 0), coalesce(max(elapsed_ms), 0),
                coalesce(sum(EXISTS(SELECT 1 FROM file_observations f WHERE f.call_id = c.id)), 0)
         FROM calls c WHERE {CALL_FILTER}"
        ),
        params![start_ms, end_ms, tool, filter],
        |row| {
            Ok(Metrics {
                calls: row.get(0)?,
                errors: row.get(1)?,
                response_bytes: row.get(2)?,
                average_ms: row.get(3)?,
                maximum_ms: row.get(4)?,
                content_calls: row.get(5)?,
                ..Metrics::default()
            })
        },
    )?;
    (metrics.files, metrics.reads, metrics.result_bytes) = connection.query_row(
        "SELECT count(DISTINCT f.path), count(*), coalesce(sum(f.result_bytes), 0)
         FROM file_observations f JOIN calls c ON c.id = f.call_id
         WHERE c.timestamp_ms >= ?1 AND c.timestamp_ms < ?2 AND (?3 IS NULL OR c.tool = ?3)
           AND (?4 IS NULL OR instr(f.path, ?4) > 0)",
        params![start_ms, end_ms, tool, filter],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    Ok(metrics)
}

fn daily_rows(
    connection: &Connection,
    start_ms: i64,
    end_ms: i64,
    tool: Option<&str>,
    filter: Option<&str>,
) -> rusqlite::Result<Vec<Vec<Cell>>> {
    let mut statement = connection.prepare(&format!(
        "WITH RECURSIVE dates(day) AS (
            SELECT date(?1 / 1000, 'unixepoch')
            UNION ALL SELECT date(day, '+1 day') FROM dates WHERE day < date((?2 - 1) / 1000, 'unixepoch')
         ), activity AS (
            SELECT c.* FROM calls c WHERE {CALL_FILTER}
         ), daily_calls AS (
            SELECT date(timestamp_ms / 1000, 'unixepoch') AS day, count(*) AS calls,
                   sum(is_error) AS errors, sum(response_bytes) AS bytes FROM activity GROUP BY day
         ), daily_files AS (
            SELECT date(c.timestamp_ms / 1000, 'unixepoch') AS day, count(*) AS reads,
                   count(DISTINCT f.path) AS files
            FROM activity c JOIN file_observations f ON f.call_id = c.id
            WHERE ?4 IS NULL OR instr(f.path, ?4) > 0 GROUP BY day
         )
         SELECT d.day, coalesce(c.calls, 0), coalesce(c.errors, 0), coalesce(f.files, 0),
                coalesce(f.reads, 0), coalesce(c.bytes, 0)
         FROM dates d LEFT JOIN daily_calls c ON c.day = d.day LEFT JOIN daily_files f ON f.day = d.day
         ORDER BY d.day"
    ))?;
    let rows = statement
        .query_map(params![start_ms, end_ms, tool, filter], |row| {
            Ok(vec![
                row.get::<_, String>(0)?.into(),
                row.get::<_, i64>(1)?.into(),
                row.get::<_, i64>(2)?.into(),
                row.get::<_, i64>(3)?.into(),
                row.get::<_, i64>(4)?.into(),
                Cell::bytes(Some(row.get(5)?)),
            ])
        })?
        .collect();
    rows
}

fn compare(current: &Metrics, previous: &Metrics) -> Dataset {
    let rows = [
        ("calls", "Calls", current.calls, previous.calls, false),
        ("errors", "Errors", current.errors, previous.errors, false),
        (
            "content_responses",
            "Content responses",
            current.content_calls,
            previous.content_calls,
            false,
        ),
        (
            "files",
            "Unique files",
            current.files,
            previous.files,
            false,
        ),
        ("reads", "File reads", current.reads, previous.reads, false),
        (
            "response_bytes",
            "Response bytes",
            current.response_bytes,
            previous.response_bytes,
            true,
        ),
        (
            "result_bytes",
            "File result bytes",
            current.result_bytes,
            previous.result_bytes,
            true,
        ),
    ]
    .into_iter()
    .map(|(key, label, recent, before, is_size)| {
        let format = |value| {
            if is_size {
                Cell::bytes(Some(value))
            } else {
                Cell::from(value)
            }
        };
        vec![
            Cell {
                value: json!(key),
                display: label.into(),
            },
            format(before),
            format(recent),
            Cell::percent(
                (before > 0).then(|| (recent as f64 - before as f64) * 100.0 / before as f64),
            ),
        ]
    })
    .collect();
    Dataset::new(
        "comparison",
        "Comparison with preceding equal-length window",
        &[
            ("metric", "Metric", false),
            ("previous", "Previous", true),
            ("recent", "Recent", true),
            ("change_pct", "Change", true),
        ],
        rows,
    )
}

pub(super) fn collect(
    connection: Option<&Connection>,
    now_ms: i64,
    options: &Options,
) -> anyhow::Result<Report> {
    let mut report = Report::new(options, "ok");
    let tool = options.tool.map(|tool| tool.as_str());
    let filter = options.filter.as_deref();
    report.metadata["scope"] = json!({"filter": filter, "tool": tool});
    let Some(connection) = connection else {
        report.metadata["status"] = json!("no_records");
        report.notes.push("No SQLite observations yet; reconnect MCP with this version to start recording read/search/grep.".into());
        return Ok(report);
    };
    // All queries share one snapshot, released before any terminal or MCP output.
    let transaction = connection.unchecked_transaction()?;
    let window_ms = i64::from(options.days) * DAY_MS;
    let start_ms = now_ms - window_ms;
    let end_ms = now_ms + 1;
    let current = metrics(&transaction, start_ms, end_ms, tool, filter)?;
    let window: (String, String) = transaction.query_row(
        "SELECT datetime(?1 / 1000, 'unixepoch'), datetime(?2 / 1000, 'unixepoch')",
        params![start_ms, now_ms],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let earliest: Option<String> = transaction.query_row(&format!(
        "SELECT datetime(min(c.timestamp_ms) / 1000, 'unixepoch') FROM calls c WHERE {CALL_FILTER}"),
        params![now_ms - 30 * DAY_MS, end_ms, tool, filter], |row| row.get(0),
    )?;
    report.metadata["window"] = json!({"days": options.days, "from_utc": window.0, "to_utc": window.1,
        "earliest_retained_utc": earliest});
    if current.calls == 0 {
        report.metadata["status"] = json!("no_activity");
    }
    if options.should_compare && options.days <= 15 {
        let previous = metrics(&transaction, start_ms - window_ms, start_ms, tool, filter)?;
        report.tables.push(compare(&current, &previous));
    } else if options.should_compare {
        report.notes.push("Comparison unavailable: two equal windows would exceed the fixed 30-day retention. Use days<=15.".into());
    }
    let file_totals: (i64, i64, i64) = transaction.query_row(&format!(
        "SELECT coalesce(sum(reads > 1), 0), coalesce(sum(size_bytes), 0), coalesce(sum(size_bytes IS NULL), 0)
         FROM ({FILE_SUMMARY})"), params![start_ms, end_ms, tool, filter],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    report.summary = vec![
        ("calls", "Calls", current.calls.into()),
        ("errors", "Errors", current.errors.into()),
        (
            "content_responses",
            "Content responses",
            current.content_calls.into(),
        ),
        ("files", "Unique files", current.files.into()),
        ("reads", "File reads", current.reads.into()),
        (
            "response_bytes",
            "Response bytes",
            Cell::bytes(Some(current.response_bytes)),
        ),
        (
            "result_bytes",
            "File result bytes",
            Cell::bytes(Some(current.result_bytes)),
        ),
        (
            "avg_ms",
            "Average server ms",
            Cell::decimal(current.average_ms),
        ),
        (
            "max_ms",
            "Maximum server ms",
            Cell::decimal(current.maximum_ms),
        ),
        (
            "known_file_size_bytes",
            "Latest known file sizes",
            Cell::bytes(Some(file_totals.1)),
        ),
        ("unknown_sizes", "Unknown file sizes", file_totals.2.into()),
        ("repeated_files", "Repeated files", file_totals.0.into()),
        (
            "repeat_reads",
            "Reads after each file's first",
            (current.reads - current.files).into(),
        ),
    ];
    if options.view.has_groups() {
        let mut rows = Vec::new();
        for name in ["read", "search", "grep"]
            .into_iter()
            .filter(|name| tool.is_none_or(|tool| tool == *name))
        {
            let values = metrics(&transaction, start_ms, end_ms, Some(name), filter)?;
            rows.push(vec![
                name.into(),
                values.calls.into(),
                values.errors.into(),
                values.content_calls.into(),
                values.files.into(),
                values.reads.into(),
                Cell::bytes(Some(values.response_bytes)),
                Cell::share(values.response_bytes, current.response_bytes),
                Cell::decimal(values.average_ms),
                Cell::decimal(values.maximum_ms),
            ]);
        }
        report.tables.push(Dataset::new(
            "tools",
            "Tool activity",
            &[
                ("tool", "Tool", false),
                ("calls", "Calls", true),
                ("errors", "Errors", true),
                ("content_responses", "Content", true),
                ("files", "Files", true),
                ("reads", "Reads", true),
                ("response_bytes", "Response", true),
                ("share_pct", "Share", true),
                ("avg_ms", "Avg ms", true),
                ("max_ms", "Max ms", true),
            ],
            rows,
        ));
    }
    if options.view == View::Full {
        report.tables.push(Dataset::new(
            "daily",
            "Daily activity (UTC; edge dates may be partial)",
            &[
                ("date_utc", "Date", false),
                ("calls", "Calls", true),
                ("errors", "Errors", true),
                ("files", "Files", true),
                ("reads", "Reads", true),
                ("response_bytes", "Response", true),
            ],
            daily_rows(&transaction, start_ms, end_ms, tool, filter)?,
        ));
    }
    if options.view.has_files() {
        let mut statement = transaction.prepare(&format!(
            "{FILE_SUMMARY} ORDER BY {} LIMIT ?5 OFFSET ?6",
            options.reads_order(),
        ))?;
        let rows = statement
            .query_map(
                params![
                    start_ms,
                    end_ms,
                    tool,
                    filter,
                    options.sql_limit(),
                    options.sql_offset()
                ],
                |row| {
                    Ok(vec![
                        Cell::bytes(row.get(1)?),
                        row.get::<_, i64>(2)?.into(),
                        row.get::<_, i64>(3)?.into(),
                        row.get::<_, i64>(4)?.into(),
                        row.get::<_, i64>(5)?.into(),
                        Cell::bytes(Some(row.get(6)?)),
                        Cell::share(row.get(6)?, current.result_bytes),
                        row.get::<_, i64>(7)?.into(),
                        row.get::<_, String>(8)?.into(),
                        row.get::<_, String>(0)?.into(),
                    ])
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        report.page(options, current.files, rows.len());
        report.tables.push(Dataset::new(
            "files",
            "Returned files",
            &[
                ("size_bytes", "File size", true),
                ("reads", "Reads", true),
                ("read", "Read", true),
                ("search", "Search", true),
                ("grep", "Grep", true),
                ("result_bytes", "Results", true),
                ("share_pct", "Share", true),
                ("active_days", "Days", true),
                ("last_utc", "Last UTC", false),
                ("path", "Path", false),
            ],
            rows,
        ));
    }
    transaction.commit()?;
    report.notes.push("Reads count one file per successful content response; repeats may be different ranges/revisions, not wasted tokens.".into());
    report.notes.push("UTF-8 bytes: response=masked text/error including context, results=unmasked file blocks including row prefixes; no model token counts.".into());
    report.notes.push("Size=last observed in window (?/null=unknown); ms excludes recording/client time. UTC dates do not guarantee continuous collection; change is unavailable when previous=0.".into());
    if filter.is_some() {
        report.notes.push("Path filter selects calls returning a matching file; response bytes include the whole call, file metrics include only matching paths. Errors without file observations are excluded.".into());
    }
    Ok(report)
}
