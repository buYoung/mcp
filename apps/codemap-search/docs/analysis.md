# Index and reading-activity reference

[한국어](./analysis.ko.md) | English

`analyze index` inspects the existing committed index without reparsing source or refreshing the index. `analyze reads` reports recorded MCP `read`, `search` and `grep` activity. Reports are generated on demand; byte counts are not token counts or physical disk reads.

## CLI

Run in the repository to inspect, or select it with `--path DIR`:

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --offset 20 --limit 20 --sort bytes
codemap-search analyze reads --view summary --format json
codemap-search analyze index --help
codemap-search analyze reads --help
```

| Option | Applies to | Behavior |
| --- | --- | --- |
| `--path DIR` | Both | Select the repository, its index configuration and usage database |
| `--limit N`, `-n N` | Both | File rows per page; default 20, `0` for all |
| `--offset N` | Both | Skip N file rows after filtering/sorting; follow the printed continuation |
| `--sort KEY`, `-s KEY` | Both | Index: `stored`, `size`, `lines`, `symbols`, `literals`, `path`; reads: `reads`, `bytes`, `size`, `last`, `path`. Defaults: `stored` and `reads`, respectively |
| `--order asc\|desc` | Both | Default: ascending paths, descending other values |
| `--filter TEXT`, `-f TEXT` | Both | Case-sensitive literal substring of paths, not a glob |
| `--view summary\|files\|full` | Both | Totals/groups, totals/files, or all detailed tables; default `full` |
| `--format table\|json` | Both | Human-readable tables (default) or compact JSON with shared column names |
| `--language NAME`, `-l NAME` | `index` | Filter by indexed language |
| `--days N`, `-d N` | `reads` | Rolling 1–30 days; default 7 |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | Restrict to one tool |
| `--no-compare` | `reads` | Omit the preceding equal-window comparison; windows above 15 days cannot be compared within 30-day retention |

## MCP

The `analyze` tool uses the same aggregation for the server's current workspace:

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

`target` is `index|reads`. Both accept `limit`, `offset`, `sort`, `order`, `filter` and `view`; index also accepts `language`, and reads accepts `days`, `tool` and `compare`.

MCP defaults to `view=files`, 10 file rows, sorting index by `stored` and reads by `bytes`. `limit` is 1–100; continue with `page.next_offset`. Request `view=full` for additional breakdowns.

Compact output uses a `summary` object and per-table `columns`/`rows` arrays. Keys identify byte/time units; numbers and `null` remain typed. Output stays within 8 KiB or a smaller `output.max_bytes`, trimming whole rows and marking `truncated`/`omitted_tables` while retaining totals and continuation. Sensitive strings are masked before JSON serialization. Analysis calls do not add their own usage observations.

## Report contents

| Section | Output | Measurement |
| --- | --- | --- |
| Index footprint | Committed files, segments, deleted documents, disk size, stored JSON size, static call/reference sites | Existing Tantivy snapshot aggregated in an in-memory SQLite database |
| Languages and symbols | Files, lines, symbols, exported symbols, literals and docstrings by language; test/documentation flags by symbol kind | Stored extraction metadata, which may differ from the full repository or current source |
| Files and freshness | Largest stored records with path, size, lines, symbols, literals, largest literal and changed/missing/unavailable state | Current filesystem metadata for sizes and mtime comparison only; no source parsing or index refresh |
| Current/previous window | Calls, errors, content responses, unique files, file reads, response/result volume and changes | Default: rolling 168 hours vs the preceding 168 hours; `n/a` when a baseline is absent |
| Tool and daily activity | Calls, errors, content responses, files, reads, response share, average/maximum processing time by tool; UTC daily trends | Recorded `read`/`search`/`grep` calls; the first and last calendar dates may be partial |
| Returned files | Path, latest recorded size, total/per-tool reads, result volume/share, active dates, last observation and repeat summary | One read per file per successful source-bearing response |

Totals cover all matching files, not just the displayed page. An activity path filter selects calls that returned a matching file: `Response` still measures the whole selected call, while `Results` includes only matching files. Errors without file observations do not match a path filter. Whole-index disk/segment metrics remain global when file filters are applied.

## Interpreting counts and bytes

- **`Reads`** counts files with returned source rows or search excerpts, once per file per successful response. Path-only and declaration/relation-only responses and failed calls contribute to calls/response volume but not reads. A file represented only by Jev omission notes is not a source read.
- **Repeated reads** are responses after a file's first appearance. They may cover different ranges or revisions; they are not evidence of wasted tokens.
- **`File size`** is the latest disk size recorded within the window. Unknown sizes are `?` and excluded from size totals.
- **`Results`** measures UTF-8 bytes in delivered per-file source/excerpt blocks, including their line/path prefixes. It is measured after source-stage masking and Jev body filtering, but before response-wide masking. Omitted bodies and Jev omission notes do not count as delivered source.
- **`Response`** measures final masked content text or error messages, including declarations, relations, headings and omission notes, but excluding JSON framing.
- **Processing time** covers server request handling, excluding SQLite recording and client/network time.

`find`, `overview`, `analyze` and internal indexing reads are not recorded. Client-side truncation and actual model consumption are not observable. Do not equate returned bytes with model tokens or assume every returned line was consumed.

## Recording, retention and failures

MCP records calls and file observations in `.codemap/analysis.sqlite3`, without query, source or response contents. **Retention is fixed at 30 days and cannot be extended.** Expired calls and their file observations are deleted together at MCP startup, recording, analysis, and every minute while MCP runs. If MCP is stopped, expired rows are removed at the next startup or analysis.

The database uses `secure_delete` and a deleted rollback journal to avoid retaining deleted rows in free pages or a persistent WAL. This policy applies to the managed database, not separate backups.

`codemap-search mcp --no-call-log` disables new recording while retaining cleanup of existing records. Recording/cleanup failures warn on stderr without failing MCP responses and retry on subsequent activity. The analysis command reports database access failures as errors. Recording gaps cannot be reconstructed, and comparisons do not guarantee continuous collection.

See [configuration](./configuration.md) for output limits and [Jev filtering](./configuration.md#optional-jev-decision-stages) for source omission behavior.
