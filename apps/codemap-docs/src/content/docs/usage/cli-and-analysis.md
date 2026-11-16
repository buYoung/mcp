---
title: CLI & Analysis
description: Command-line subcommands and the analyze index / analyze reads reference for index footprint and recorded MCP reading activity.
---

## CLI subcommands

`codemap-search` provides direct CLI subcommands alongside the MCP server:

```text
codemap-search mcp [--no-call-log]
codemap-search codex-config [--server-name NAME]
codemap-search analyze index [--path DIR] [--sort stored|size|lines|symbols|literals|path] [OPTIONS]
codemap-search analyze reads [--path DIR] [--days 1..30] [--sort reads|bytes|size|last|path] [OPTIONS]
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

## Analyzing the index and reading activity

`analyze index` inspects the committed index without reparsing source or refreshing the index, while `analyze reads` summarizes recorded MCP `read`, `search`, and `grep` calls.

Run inside the target repository or pass `--path DIR`:

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --offset 20 --limit 20 --sort bytes
codemap-search analyze reads --view summary --format json
```

| Option | Applies to | Behavior |
|---|---|---|
| `--path DIR` | Both | Select the repository, its index configuration, and its usage database |
| `--limit N`, `-n N` | Both | File rows per page (default 20; `0` returns all) |
| `--offset N` | Both | Skip N file rows after filtering and sorting; use with the printed continuation offset |
| `--sort KEY`, `-s KEY` | Both | `index`: `stored`, `size`, `lines`, `symbols`, `literals`, `path` (default `stored`); `reads`: `reads`, `bytes`, `size`, `last`, `path` (default `reads`) |
| `--order asc\|desc` | Both | Default: ascending for `path`, descending for all other keys |
| `--filter TEXT`, `-f TEXT` | Both | Case-sensitive literal substring match on file paths (not a glob) |
| `--view summary\|files\|full` | Both | Totals and groups, totals and file rows, or all detail tables (default `full`) |
| `--format table\|json` | Both | Human-readable tables (default) or compact JSON sharing the same column names |
| `--language NAME`, `-l NAME` | `index` | Filter by indexed language |
| `--days N`, `-d N` | `reads` | Rolling 1–30 day window (default 7) |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | Restrict to a single tool |
| `--no-compare` | `reads` | Omit comparison against the preceding equal window (windows over 15 days cannot be compared within 30-day retention) |

## MCP `analyze` tool

The MCP `analyze` tool runs the same aggregation on the server's active workspace:

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

`target` must be `index` or `reads`. Both accept `limit`, `offset`, `sort`, `order`, `filter`, and `view`; `index` also accepts `language`, and `reads` accepts `days`, `tool`, and `compare`.

- **Defaults and pagination**: MCP defaults to `view=files` with 10 rows, sorting `index` by `stored` and `reads` by `bytes`. `limit` accepts `1–100` with `page.next_offset` for pagination; pass `view=full` for full breakdowns.
- **Format and caps**: Compact JSON returns a `summary` object and per-table `columns`/`rows` arrays within `8 KiB` (or a smaller `output.max_bytes`), trimming whole rows and setting `truncated` or `omitted_tables` while keeping totals and `page.next_offset`.
- **Privacy and logging**: Sensitive strings are masked before output, and `analyze` calls are never logged as usage observations.

## Report contents

| Section | Output | Measurement |
|---|---|---|
| Index footprint (`index`) | Committed files, segments, deleted documents, disk size, stored JSON size, and static call/reference sites | Committed index snapshot (read-only; no re-indexing) |
| Languages and symbols (`index`) | Files, lines, symbols, exported symbols, literals, and docstrings per language; test and documentation flags by symbol kind | Stored extraction metadata (may lag behind uncommitted edits or unindexed files) |
| Files and freshness (`index`) | Largest stored records with path, `File size`, lines, symbols, literals, largest literal, and changed, missing, or unavailable status | Filesystem metadata check (size and mtime) with no re-parsing or index refresh |
| Current/previous window (`reads`) | Calls, errors, content responses, unique files, `Reads`, `Response`/`Results` volume, and deltas | Default: rolling 168 hours vs the prior 168 hours (`n/a` if no baseline exists) |
| Tool and daily activity (`reads`) | Per-tool calls, errors, content responses, files, `Reads`, `Response` share, and mean/max processing time; UTC daily trends | Recorded `read`, `search`, and `grep` calls (boundary calendar dates may be partial) |
| Returned files (`reads`) | Path, latest `File size`, total and per-tool `Reads`, `Results` volume and share, active dates, last seen timestamp, and repeat summary | Counted as one read per file per successful source-bearing response |

Totals cover every matching file across the query, not just the current page. When filtering `reads` by path, `Response` still counts the full size of each matched call while `Results` sums only matching files (error responses without file hits do not match a path filter). Index disk and segment totals stay global even when file filters are applied.

## Interpreting counts and bytes

All byte metrics measure UTF-8 output volume rather than physical disk reads.

- **`Reads`**: Counted once per file for each successful response that returns source lines or search excerpts. Path-only, declaration/relation-only, Jev-omission-only, and failed calls count toward calls and response volume, not `Reads`.
- **Repeated reads**: Responses after a file's first appearance in the window. Repeated reads often inspect different line ranges or post-edit revisions and do not imply wasted tokens.
- **`File size`**: Latest recorded disk size within the window (`?` if unknown, excluded from size totals).
- **`Results`**: UTF-8 bytes of delivered per-file source or excerpt blocks (including line numbers and path prefixes), excluding omitted bodies, Jev omission notes, and deduplicated lines.
- **`Response`**: UTF-8 bytes of the final masked response text or error message, including headings, declarations, relations, and omission notes, excluding JSON-RPC framing.
- **Processing time**: Server request-handling time, excluding logging and client or network overhead.

`find`, `analyze`, and internal indexing reads are not logged. Because the server cannot observe client-side truncation or actual model token consumption, do not equate returned bytes with tokens or assume every returned line entered model context.

## Recording, retention, and failures

MCP stores call and file observations in `.codemap/analysis.sqlite3` without logging queries, source text, or response bodies. Retention is fixed at 30 days and cannot be extended: expired calls and their file observations are purged on startup, on new recordings, during analysis, and every minute while the server is running (or on the next startup or analysis if the server was stopped).

Start the server with `codemap-search mcp --no-call-log` to stop recording new calls while still running expiry cleanup. Recording or cleanup failures warn on `stderr` without interrupting MCP responses, whereas `analyze` commands report database access failures as errors.

See [configuration](/configuration/) for output limits and [optional Jev decision stages](/configuration/#optional-jev-decision-stages) for source omission behavior.
