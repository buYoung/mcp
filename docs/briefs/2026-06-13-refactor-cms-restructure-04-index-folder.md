# [refactor] Group index subsystem and introduce EngineSupervisor

## Work Type
refactor

## Current State (As-Is)
- The index subsystem is three flat files coupled through `IndexCommand` and engine ownership transfer: `apps/codemap-search/src/index.rs` (1,316 lines), `apps/codemap-search/src/indexer.rs` (218 lines, takes ownership of `TantivySearchEngine` via `spawn_indexer`), `apps/codemap-search/src/watcher.rs` (353 lines, emits `IndexCommand` per `watcher.rs:39`).
- `index.rs` mixes two concerns: the write path (`new` line 120, `index_files_changed` line 370, `apply_index_updates` line 448, `refresh_paths` line 597, `load_extracted_files` line 737) and the read path (`SearcherHandle` line 50, `SearcherHandle::search` lines 836-1035, ranking heuristics `is_discriminative_name` line 776, `term_hits_symbol_name` line 788, `symbol_matches_term` line 803, `partial_match_threshold` line 814, `is_test_like_path` line 819).
- Engine lifecycle supervision lives in the wrong module: `maybe_restart_indexer` and `maybe_trigger_refresh` (in `apps/codemap-search/src/mcp.rs`, methods on `McpServer`), `MAX_INDEXER_RESTART_ATTEMPTS` const, restart-attempt and refresh-debounce state fields, and the field-order drop contract (watcher must drop before indexer — documented in the `McpServer` struct comment) all manipulate index-subsystem types (`SearcherHandle`, `IndexerHandle`, `WatcherHandle`, `WatcherStatus`) yet live in the MCP server.
- Path helpers formerly in `index.rs` (`normalize_relative_path`, `relative_index_path`, `stored_index_key`) are in `workspace.rs` after child 01.

## Behavior Contract
- Locked: search ranking order and scores for identical index state; incremental reindex triggers; indexer auto-restart semantics (attempt cap, searcher/watcher rebuild); watcher-unhealthy fallback (request-triggered refresh stays active when `watch = false` or watch fails); shutdown sequence (watcher joined before indexer channel closes).
- Contract artifacts: `index.rs` in-file tests (basic indexing/search, ranking weights, incremental, corrupt recovery, format-version sidecar), e2e suite.
- Verification: `cargo test` passes unchanged; manual kill-the-indexer scenario optional (restart path is hard to drive in tests — preserve code shape exactly).

## Desired Outcome (To-Be)
- Directory `apps/codemap-search/src/index/` (proposed):
  - `mod.rs` — canonical re-exports (`SearchEngine`, `SearchResult`, `TantivySearchEngine`, `SearcherHandle`, `IndexerHandle`, `spawn_indexer`, watcher types, `EngineSupervisor`).
  - `engine.rs` — schema + write path (former `index.rs` minus read path).
  - `ranking.rs` — `SearcherHandle`, `SearcherHandle::search`, ranking heuristics.
  - `indexer.rs`, `watcher.rs` — moved as-is.
  - `supervisor.rs` — new `EngineSupervisor` struct owning `searcher: SearcherHandle`, `watcher: Option<WatcherHandle>`, `indexer: IndexerHandle`, restart-attempt counter, refresh-debounce instant, `Arc<WatcherStatus>`; **field order encodes the drop contract** (watcher before indexer) with the existing comment moved here; methods `ensure_alive()` (former `maybe_restart_indexer`), `trigger_refresh()` (former `maybe_trigger_refresh`), plus read accessors the server needs (`search`, `codemap_snapshot`, `is_dead`/`is_warming`/`last_error` passthroughs).
- `McpServer` holds one `EngineSupervisor` field instead of four handle/state fields; it calls `ensure_alive()`/`trigger_refresh()` at the search/overview dispatch sites exactly where the old methods were called.

## Scope
### In Scope
- Directory conversion, engine read/write split, supervisor extraction, `McpServer` field/call-site rewiring, `main.rs` wiring update (`Commands::Mcp` arm constructs the supervisor parts).
### Out of Scope
- [hard] No ranking changes, no schema changes, no new restart/refresh policies — supervision logic moves verbatim.
- [hard] Do not change `IndexCommand` variants or the indexer thread protocol.
- [deferred] Extracting search/overview tool bodies out of `mcp.rs` — child 05 (it consumes `EngineSupervisor` from this child).

## Related Files / Entry Points
- `apps/codemap-search/src/index.rs` — split source; read the `SearcherHandle` impl block boundary (around lines 776-1035) first to fix the read/write cut line.
- `apps/codemap-search/src/mcp.rs` — extraction source for supervision (`maybe_restart_indexer`, `maybe_trigger_refresh`, `MAX_INDEXER_RESTART_ATTEMPTS`, `McpServer` field block + drop-order comment).
- `apps/codemap-search/src/indexer.rs`, `apps/codemap-search/src/watcher.rs` — moved into `apps/codemap-search/src/index/` (proposed).
- `apps/codemap-search/src/main.rs` — `Commands::Mcp` arm (engine/searcher/indexer/watcher construction around lines 180-216) becomes supervisor construction.
- `apps/codemap-search/src/lib.rs` — `pub mod index;` resolves to directory; `pub mod indexer;`/`pub mod watcher;` declarations removed (now submodules).

## Side Effect Checkpoints
- [ ] Drop order verified: `EngineSupervisor` field order keeps watcher-join-before-indexer-channel-close semantics (the comment must travel with the fields).
- [ ] Restart path rebuilds searcher + indexer + watcher handles inside the supervisor and the server sees the fresh searcher (no stale handle held by `McpServer`).
- [ ] CLI `index`/`search`/`benchmark` subcommands (direct engine use, no supervisor) still compile and behave identically.
- [ ] `benchmark.rs` trait-object usage (`SearchEngine`) unaffected by the engine/ranking file split.

## Acceptance Criteria
- [ ] `index/` contains exactly `mod.rs`, `engine.rs`, `ranking.rs`, `indexer.rs`, `watcher.rs`, `supervisor.rs`.
- [ ] `McpServer` no longer owns raw `SearcherHandle`/`IndexerHandle`/`WatcherHandle` fields — only `EngineSupervisor` (plus non-engine state).
- [ ] `cargo test` passes; index test count unchanged.
- [ ] e2e suite passes, including any watch/no-watch scenarios it covers.

## Open Questions
- None — the supervisor boundary (engine supervision is an index concern, not an MCP concern) was locked during plan review; method names `ensure_alive`/`trigger_refresh` are bounded implementation choices.
