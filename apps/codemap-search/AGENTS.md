# AGENTS.md

## 1. Overview

`codemap-search` is a Rust MCP/CLI binary that provides a self-contained codemap, Tantivy BM25 search, caller/callee annotations, and embedded read/find/grep tools without requiring external runtime binaries.

## 2. Folder Structure

- `Cargo.toml`: Rust package metadata and dependencies for clap, tokio, tree-sitter grammars, Tantivy, notify, tracing, TOML config, and ripgrep library crates.
- `README.md`: user-facing installation, MCP registration, tool, config, and logging reference.
- `docs/`: benchmark reports, design notes, and configuration references; keep implementation changes aligned with current design docs.
- `src/main.rs`: CLI entry. It initializes stderr logging/config, dispatches CLI subcommands, and in `mcp` mode scaffolds config, creates the Tantivy engine, starts indexer/watcher, and runs the MCP loop.
- `src/mcp.rs`: hand-written JSON-RPC/MCP protocol loop, tool schemas, request dispatch, search/overview rendering, warming/stale-result notices, path safety, output caps, and indexer auto-restart.
- `src/index.rs`: Tantivy schema, query parsing recovery, incremental indexing, mtime cache, extraction-format sidecar versioning, corrupt-index rebuild, ranking, and result selection.
- `src/indexer.rs`: background indexing thread, capacity-1 refresh channel, status flags, immutable codemap snapshot publication, and clean shutdown join.
- `src/watcher.rs`: notify watcher, fixed-window debounce, git HEAD hint handling, event filtering, path-scoped refreshes, full refresh escalation, and watcher health signaling.
- `src/parser.rs`: tree-sitter extraction for supported languages, symbol/literal/docstring capture, owner inference, flags, ranges, and identifier splitting.
- `src/codemap.rs`: root/folder/file codemap views, significant-symbol filtering, directory aggregation, and `llms-txt` rendering.
- `src/callers.rs`: depth-1 caller/callee annotation scan, common-name ambiguity labels, budget handling, and failure isolation.
- `src/tools/`: live filesystem `read`, `find`, and `grep` implementations plus shared walkers, glob matching, ignore rules, path resolution, aliases, and argument helpers.
- `tests/e2e/`: end-to-end coverage for MCP protocol, parser, config, search, tools, watcher, codemap, benchmark, and cross-feature scenarios.

## 3. Core Behaviors & Patterns

- **Sequential protocol, off-thread indexing**: MCP runs as a single-client line reader on Tokio `current_thread`. Heavy indexing runs on the named `codemap-indexer` OS thread, so request handling keeps a read-only `SearcherHandle` and never owns the Tantivy writer.
- **Snapshot search, live filesystem tools**: `search` and `overview` read committed index/codemap snapshots and may report warming or stale status. `read`, `find`, and `grep` read disk directly, so they are the fallback for just-edited files and exact enumeration.
- **Single-writer ownership**: `TantivySearchEngine` moves into the indexer thread. The indexer publishes whole `Arc<Vec<ExtractedFile>>` snapshots after successful passes, which prevents partially updated codemap views.
- **Refresh routing**: request fallback sends full `Refresh`; watcher ordinary edits send `RefreshPaths`; git HEAD changes, notify rescan/overflow, oversized batches, and watcher failures escalate to full refresh. The capacity-1 channel coalesces redundant full refreshes.
- **Watcher health controls fallback**: a healthy watcher suppresses request-triggered tree walks. If watch is disabled, fails to start, exits, or the indexer dies, health flips off and request-time refresh resumes.
- **Search detail is budgeted and hybrid**: BM25 ranks over path parts, symbols/owners, docstrings, and capped literals. Top files render snippets, line numbers, literals, and optional caller/callee notes; the tail renders compact ranked rows within byte/count caps.
- **Config is never-exit**: repo/global `.codemap/config.toml` layers merge per key as `repo > global > default`; invalid files, unknown keys, and type mismatches warn to stderr and fall back. Configured excluded directories augment built-ins instead of replacing them.
- **Recovery is explicit**: Tantivy query parsing catches panics; corrupt or outdated index formats rebuild via `codemap.format`; dead indexers can auto-restart on later requests up to a fixed cap.
- **Safety and output bounds are shared**: cwd containment, ignore-aware walkers, source-extension filters, max file size, read output caps, grep max columns, detail byte budgets, and caller annotation sub-budgets are enforced before rendering large results.

## 4. Conventions

- **Naming**: Rust modules/functions use `snake_case`, types/traits use `PascalCase`, constants use `UPPER_SNAKE_CASE`, and tests use `test_*` names.
- **Module ownership**: keep protocol dispatch in `mcp.rs`, indexing in `index.rs`/`indexer.rs`, watch behavior in `watcher.rs`, extraction in `parser.rs`, codemap rendering in `codemap.rs`, caller annotations in `callers.rs`, and public read/find/grep entrypoints under `src/tools/`.
- **Result shapes**: MCP-facing helpers return `Result<String, (i64, String)>` when they need JSON-RPC error codes. Lower-level code usually returns `Result<T, String>` or `Option` and lets `mcp.rs` map failures to protocol responses.
- **Diagnostics**: stdout is reserved for JSON-RPC frames in MCP mode. Logs, config warnings, parse/index failures, and progress diagnostics go to stderr through `tracing`, `eprintln!`, or explicit stderr writes.
- **Config shape**: TOML keys are `snake_case` and normalize into `ResolvedConfig`. `excluded_directories` is unioned with built-ins; built-in junk/VCS/index directories cannot be removed by config.
- **Tool aliases**: MCP tools accept observed agent aliases where implemented (`read` accepts `file_path`/`path`/`file`/`query`, line windows accept `offset`/`limit` plus start/end aliases, `grep` accepts `glob`/`include`/`file_pattern`). Canonical names win.
- **Path display**: output paths are normalized to forward slashes and are cwd-relative when possible; opt-in out-of-root `find` results fall back to absolute paths.
- **Output budgets**: new rendering must obey existing caps (`read_output_byte_cap`, `search_detail_byte_cap`, `annotation_sub_budget`, `grep_max_columns`, list limits) and emit truncation notes instead of silently dropping context.
- **Comments**: comments document operational constraints and measured tradeoffs, especially watcher/indexer drop order, panic recovery, sidecar version bumps, ignore semantics, and agent-observed parameter behavior.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after Rust changes in this package.
