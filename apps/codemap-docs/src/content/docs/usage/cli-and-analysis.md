---
title: CLI & Analysis
description: CLI subcommands and analyze index / analyze reads reference.
---

## CLI subcommands

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

## Index and read analysis (`analyze`)

`analyze index` inspects the committed index without re-indexing. `analyze reads` summarizes recorded MCP `read`, `search`, and `grep` calls. Run inside the target repository or pass `--path DIR`:

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --view summary --format json
```

| Option | Target | Description |
|---|---|---|
| `--path DIR` | Both | Repository directory to inspect |
| `--limit N`, `-n N` | Both | File rows per page (default `20`; `0` returns all) |
| `--offset N` | Both | File rows to skip for pagination |
| `--sort KEY`, `-s KEY` | Both | `index`: `stored` (stored index bytes, default), `size` (disk file bytes), `lines`, `symbols`, `literals`, `path`; `reads`: `reads` (count, default), `bytes` (`Results` bytes), `size` (disk file bytes), `last`, `path` |
| `--order asc\|desc` | Both | Ascending for `path` by default, descending for other keys |
| `--filter TEXT`, `-f TEXT` | Both | Case-sensitive path substring filter (literal, not glob) |
| `--view summary\|files\|full` | Both | `summary` (totals and per-language/tool breakdowns), `files` (totals and file rows), or `full` (all sections, default) |
| `--format table\|json` | Both | Human-readable tables (default) or compact JSON |
| `--language NAME`, `-l NAME` | `index` | Filter by indexed language |
| `--days N`, `-d N` | `reads` | Rolling `1–30` day window (default `7`) |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | Filter to a single tool |
| `--no-compare` | `reads` | Skip comparison against the previous window |

## MCP `analyze` tool

The MCP `analyze` tool runs the same query on the active workspace. It defaults to `view: "files"` and `limit: 10` (max `100`); pass the returned `page.next_offset` as `offset` to fetch the next page:

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

## Report sections and metrics

| Section | Contents |
|---|---|
| Index footprint (`index`) | Committed files, segments, disk size, stored index bytes, and call/reference counts |
| Languages and symbols (`index`) | Files, lines, symbols, exported symbols, literals, and docstrings per language |
| Files and freshness (`index`) | Indexed files with disk size, symbol/literal counts, and modified/missing status |
| Current/previous window (`reads`) | Calls, errors, unique files, `Reads`, `Response`/`Results` bytes, and delta vs previous window |
| Tool and daily activity (`reads`) | Per-tool calls, errors, latency, and UTC daily breakdown |
| Returned files (`reads`) | Per-file `Reads`, `Results` bytes, last seen timestamp, and repeat reads (2+ responses) |

- **`Reads`**: One count per file for each successful response that returned source lines or search snippets.
- **`Results` vs `Response`**: `Results` measures UTF-8 bytes of delivered file source and snippets; `Response` measures the full MCP response text, including headings, declarations, and omission notes.

Call metadata is stored for 30 days in `.codemap/analysis.sqlite3` without queries or source text. Start the server with `codemap-search mcp --no-call-log` to disable call logging.
