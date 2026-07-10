# AGENTS.md

## 1. Overview

`codemap-search` is a Rust MCP/CLI binary that provides self-contained code navigation: workspace codemap views, Tantivy-backed BM25 search, caller/callee annotations, and embedded read/find/grep tools. It owns both the local MCP protocol contract and the indexing/runtime machinery needed to keep results useful without external binaries.

## 2. Ownership Map

### Stable Ownership Boundaries

- **MCP protocol and tool contract boundary**: Start in `src/mcp/mod.rs` and `src/tools/mod.rs` when changing JSON-RPC behavior, tool schemas, annotations, or tool descriptions. They own initialize negotiation, `tools/list`, `tools/call`, read-only annotations, active workspace scope, and the JSON content/error envelope; verify through `tests/e2e/mcp.rs`.
- **Search and codemap snapshot boundary**: Start in `src/tools/search/`, `src/tools/overview.rs`, and `src/index/supervisor.rs` when changing search/overview behavior. These paths own snapshot-backed navigation, warming/stale/dead-indexer notices, monorepo scoping, detail/tail rendering, caller/callee budgets, and output caps; verify through search, codemap, and cross-feature e2e tests.
- **Index refresh boundary**: Start in `src/index/indexer.rs`, `src/index/watcher.rs`, and `src/index/supervisor.rs` when changing freshness or recovery. They own the single Tantivy writer, capacity-1 refresh channel, path-scoped refreshes, watcher health fallback, indexer auto-restart, and shutdown order; preserve watcher-before-indexer drop semantics.
- **Configuration boundary**: Start in `src/config.rs`, `src/config_locale.rs`, and `docs/configuration.md` when changing config keys, defaults, generated templates, or schema migration. They own repo/global/default precedence, never-exit validation, locale-selected comments, additive config sync, and filesystem permission policies; verify with config tests and docs alignment.
- **Filesystem tool safety boundary**: Start in `src/workspace.rs` and `src/tools/read.rs`, `find.rs`, or `grep.rs` when changing live filesystem tools. These files own cwd containment, allowed external roots, ignore-aware walkers, aliases, file-size/output limits, and path display; verify through tools e2e tests.
- **Language extraction boundary**: Start in `src/lang/` and `src/parser/` when adding language support or changing symbol extraction. They own tree-sitter grammar registration, source-extension allowlists, symbol/literal/docstring extraction, owner inference, and parse-time flags consumed by search and codemap.

### Active Change Routes

- **Config schema-sync route**: Within **Configuration boundary**, start in `src/config.rs` and `docs/configuration.md` when adding or moving a config key. Recent changes concentrate here; update `CONFIG_VERSION`, migrations, parser normalization, generated template comments, README/config docs, and tests together so automatic repo config sync remains additive and non-destructive.
- **Navigation guidance route**: Within **MCP protocol and tool contract boundary**, start in `src/tools/mod.rs` and `src/tools/instructions/` when changing tool selection guidance. Preserve read-only/open-world annotations, monorepo-specific descriptions, `initial_instructions`, and filesystem permission text because clients use these fields to decide whether and how to call tools.
- **Search rendering route**: Within **Search and codemap snapshot boundary**, start in `src/tools/search/mod.rs` when changing ranked output. Recent changes touch detail byte caps, directory diversity, read suggestions, literal rendering, and caller/callee context; keep truncation notes and narrowed-read hints explicit.

## 3. Core Behaviors & Patterns

- **Sequential protocol with off-thread indexing**: The MCP loop is a single-client Tokio current-thread line reader. Indexing runs on the named `codemap-indexer` OS thread, so request handling owns only read-side handles and never blocks on the Tantivy writer.
- **Snapshot search, live filesystem tools**: `search` and `overview` read committed index/codemap snapshots and may warn about warming, stale, or frozen state. `read`, `find`, and `grep` read disk directly, so they remain the fallback for just-edited files and exact enumeration.
- **Single-writer refresh model**: `TantivySearchEngine` moves into the indexer thread. The indexer publishes whole `Arc<Vec<ExtractedFile>>` codemap snapshots after successful passes, preventing partially updated overview output.
- **Watcher-first freshness**: A healthy watcher sends path-scoped `RefreshPaths` for ordinary edits and escalates to full `Refresh` for git HEAD changes, notify overflow/rescan, oversized batches, or watcher errors. While healthy, request-triggered refresh is suppressed; when unhealthy, debounced request fallback resumes.
- **Explicit recovery and notices**: Dead indexers can auto-restart up to a fixed cap; corrupt index formats rebuild through `TantivySearchEngine::new`; background errors are surfaced in search/overview output rather than hidden.
- **Budgeted hybrid search output**: BM25 ranks over path parts, symbols, owners, docs, and capped literals. Top-ranked files render snippets, symbols, read suggestions, and optional caller/callee notes; remaining matches become compact ranked rows within byte/count caps.
- **Never-exit config loading**: Missing files, parse errors, unknown keys, and type mismatches warn to stderr and fall back per key. Built-in excluded directories and filesystem permission defaults remain protective unless explicitly widened by config.
- **Centralized path and ignore handling**: `workspace.rs` owns cwd containment, lenient canonicalization, ignore-aware walkers, `.codemapignore`, source-extension filtering, minified bundle filtering, and display-path normalization for multiple tools.

## 4. Conventions

- **Naming**: Rust modules/functions use `snake_case`, types/traits use `PascalCase`, constants use `UPPER_SNAKE_CASE`, and tests use `test_*` names.
- **Module ownership**: Keep protocol dispatch in `mcp/`, indexing in `index/`, watch behavior in `index/watcher.rs`, extraction in `parser/` and `lang/`, codemap rendering in `codemap/`, caller annotations in `callers/`, and live filesystem tools under `tools/`.
- **Result shapes**: MCP-facing helpers return `Result<String, (i64, String)>` when JSON-RPC error codes matter. Lower layers generally return `Result<T, String>` or `Option` and let the protocol/tool layer map failures.
- **Diagnostics**: stdout is reserved for JSON-RPC frames in MCP mode. Logs, config warnings, parse/index failures, and progress diagnostics go to stderr via `tracing`, `eprintln!`, or explicit stderr writes.
- **Config shape**: TOML keys are `snake_case`; sectioned keys under `[update]`, `[index]`, `[refresh]`, `[search]`, `[tool_output]`, `[filesystem_permissions]`, and `[caller_context]` are canonical. Legacy top-level keys may be accepted but should not be the documented primary form.
- **Tool aliases**: Preserve agent-observed aliases where implemented, such as `file_path`/`path`/`file`/`query` and line-window start/end aliases. Canonical names should win when aliases collide.
- **Path display**: Output paths are normalized to forward slashes and are cwd-relative when possible; opt-in out-of-root filesystem-tool results fall back to absolute paths.
- **Output budgets**: New rendering must obey existing caps such as `read_output_byte_cap`, `search_detail_byte_cap`, `annotation_sub_budget`, `grep_max_columns`, and list limits. Emit truncation or narrowing notes rather than silently dropping context.
- **Comments**: Use comments for operational constraints and measured tradeoffs: watcher/indexer drop order, panic recovery, config migration, output caps, ignore semantics, and agent-observed parameter behavior.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after Rust changes in this package.
