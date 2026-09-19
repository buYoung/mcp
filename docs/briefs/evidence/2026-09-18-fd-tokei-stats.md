# Indexed tokei statistics handoff — revised 2026-09-19

## Revision and decision

- Repository: `/Users/buyong/workspace/private/buyong-mcp`
- Base revision: `9edd5699955152a3d49db642493a940522901148` (`9edd56999`)
- Starting `git status --short`: empty.
- Original slice added an `overview.include_stats` request option. The user later changed the contract: remove the request option, include statistics on every root overview by default, and allow disabling through configuration.
- Final contract: repository-root and monorepo workspace-root `overview` calls append statistics unless `[tool_output].is_overview_stats_enabled = false` is set. Non-root folders and file views remain statistics-free.

## Changed files

- `apps/codemap-search/Cargo.toml`
- `apps/codemap-search/Cargo.lock`
- `apps/codemap-search/src/config.rs`
- `apps/codemap-search/src/config_template.toml`
- `apps/codemap-search/src/config_template.ko.toml`
- `apps/codemap-search/src/tools/mod.rs`
- `apps/codemap-search/src/tools/overview.rs`
- `apps/codemap-search/src/tools/overview/stats.rs`
- `apps/codemap-search/src/codemap/monorepo.rs`
- `apps/codemap-search/src/index/engine.rs`
- `apps/codemap-search/src/index/indexer.rs`
- `apps/codemap-search/src/tools/instructions/tools/overview.md`
- `apps/codemap-search/src/tools/instructions/tools/overview.monorepo.md`
- `apps/codemap-search/docs/configuration.md`
- `apps/codemap-search/docs/configuration.ko.md`
- `apps/codemap-search/README.md`
- `apps/codemap-search/README.ko.md`

## Dependency and scope contract

- Selected dependency: `tokei = { version = "15.0.0", default-features = false }`.
- `parse_from_slice`, `from_file_extension`, and `CodeStats::summarise` operate on in-memory byte buffers for indexed files. Disk reads are checked against the published generation's existing modification-time stamps. No tokei executable is installed or executed.
- The candidate population is the same published codemap snapshot used by the root overview. Tokei never performs an independent repository walk.
- The request schema no longer contains `include_stats`; `overview` accepts `path` and `format`. Repository roots and selectable monorepo workspace roots attach statistics automatically.
- Statistics apply when the resolved path is the repository root or a selectable monorepo workspace root. Other folder and file views preserve their existing output.
- In a monorepo, the repository root keeps its workspace scope list and appends one aggregate statistics section for the full indexed physical-file population. A workspace-root overview appends one statistics section for that workspace only. Selection uses the existing `WorkspaceCatalog`, including its conventional project roots and selectable top-level source roots; it does not add manifest-based project discovery.

## Configuration contract

- Configuration schema version is now **17**.
- New key: `[tool_output].is_overview_stats_enabled`.
- Default: `true`, satisfying the user’s “root overview always shows statistics” decision.
- Disable with:
  ```toml
  [tool_output]
  is_overview_stats_enabled = false
  ```
- The key follows existing repo → global → default precedence and is included in English/Korean templates, migration blocks, and documentation.
- Migration adds the key as a commented block to older schema-16 files, preserving existing values and comments. The built-in default applies unless explicitly overridden.

## Counting convention

- For each physical indexed file:
  1. Compare its current nanosecond modification-time stamp with the stamp from the same committed document as the published codemap.
  2. Reuse valid numeric cache entries; otherwise read its source bytes once within the configured file limit and request budget. Verify size and modification time before and after reading.
  3. Classify with `tokei::LanguageType::from_file_extension`.
  4. Count with `parse_from_slice`.
  5. Call `summarise()` so embedded language blobs fold once into the physical file’s language bucket.
- `total = code + comments + blanks`.
- Generated macro-expansion buffers are not separate physical files.
- Duplicate indexed paths are deduplicated.
- Codemap language labels are used when available; the tokei language name is the fallback.
- Rows are bounded to 30 languages and state any omission.
- The output explicitly reports scope, population, indexed/counted/unavailable coverage, collection state, and the counting convention.
- A request counts at most 256 uncached files and reads at most 64 MiB of their declared sizes, with one extra byte per read to detect growth. Remaining uncached files are pending; subsequent calls reuse completed counts. A file larger than 64 MiB is an explicit coverage gap even if the configured index size cap is larger.

## Cache and freshness contract

- Cache type: process-local `OnceLock<Mutex<HashMap<CacheKey, FileStats>>>`.
- Cache ownership: `apps/codemap-search/src/tools/overview/stats.rs`.
- Cache key includes workspace identity, published snapshot pointer, indexed path, file size, nanosecond source modification time, codemap language, tokei language, and counting mode.
- Cache values contain numeric counts only; raw source is not retained.
- Cache entries are bounded by the active published file population and retained only for the current workspace and snapshot. Cache hits validate the current metadata against that generation without rereading source bytes.
- Missing, unreadable, oversized, unsupported, or concurrently changed files are explicit coverage gaps.
- A missing indexed stamp or a current stamp that differs from the published stamp is an explicit coverage gap. Existing stored stamps are copied into `PublishedIndexSnapshot` in memory; no index schema or stored document format changes.
- No statistics are persisted to disk or loaded on restart.

## Read-only and write-boundary trace

1. Repository/project root `overview` resolves its path and reads the published snapshot.
2. `stats::render` receives the published snapshot.
3. Uncached physical files already present in that scope are read within the request budget after source-stamp validation.
4. Tokei counts the in-memory bytes.
5. The renderer appends only counts and coverage metadata.
6. No new call reaches file writes, subprocess execution, report export, persistent cache, or a tokei CLI.
7. Snapshot loading now carries existing source stamps in memory. Index writes, watcher lifecycle, and response-redaction behavior are unchanged.
8. `ExtractedFile::total_lines`, persisted documents, and Tantivy schemas are unchanged.

## Verification

All commands ran from `/Users/buyong/workspace/private/buyong-mcp`.

The following results preceded the final absolute-path, source-stamp, and collection-budget corrections. Final revalidation is recorded below.

- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo check --manifest-path apps/codemap-search/Cargo.toml`
  - Result: exit 0; `Finished dev profile ... in 6.90s`.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml config::`
  - Result: exit 0; 32 library config tests and 9 e2e config tests passed.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::codemap::`
  - Result: exit 0; 15 passed, 0 failed, 194 filtered out.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::`
  - Result: exit 0; 35 passed, 1 ignored, 173 filtered out.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp::`
  - Result: exit 101; 23 passed, 3 failed, 183 filtered out.
  - The failures are the same pre-existing search rendering failures documented in the find handoff and reproduce at base revision `9edd56999`. They are unrelated to root statistics or the config toggle.
- `git diff --check`
  - Result: exit 0.

## Manual stdio observations

Observed against `apps/codemap-search/target/debug/codemap-search mcp`.

### Default root output

In a temporary copy of `apps/codemap-search/tests/fixtures/event_navigation`:

- Root `overview` with no request options contained `## Indexed Statistics`.
- Coverage: 7 indexed, 7 counted, 0 unavailable.
- TypeScript: 29 code, 1 comment, 0 blanks, 30 total (6 files).
- TOML: 14 code, 0 comments, 0 blanks, 14 total (1 file).
- Total: 43 code, 1 comment, 0 blanks, 44 total (7 counted files).

### Config-disabled root output

The same temporary fixture with:

```toml
# codemap-config-version: 17
[tool_output]
is_overview_stats_enabled = false
```

produced a root `overview` response without `## Indexed Statistics`.

### Monorepo project-root output

A temporary monorepo with `apps/alpha`, `packages/beta`, and one nested file under `apps/alpha` produced:

- `overview` with no path:
  - `## Indexed Statistics`
  - Scope: `root (all indexed files)`
  - Coverage: 5 indexed, 5 counted, 0 unavailable.
  - TOML: 28 code, 0 comments, 0 blanks, 28 total (2 files).
  - TypeScript: 18 code, 0 comments, 0 blanks, 18 total (3 files).
  - Total: 46 code, 0 comments, 0 blanks, 46 total (5 counted files).
- `overview` with `path="apps/alpha"`:
  - `## Indexed Statistics`
  - Scope: `workspace root apps/alpha`
  - Coverage: 3 indexed, 3 counted, 0 unavailable.
  - TOML: 14 code, 0 comments, 0 blanks, 14 total (1 file).
  - TypeScript: 12 code, 0 comments, 0 blanks, 12 total (2 files).
  - Total: 26 code, 0 comments, 0 blanks, 26 total (3 counted files).
- `overview` with `path="apps/alpha/nested"`:
  - No statistics section; it is a folder below a workspace root, not a workspace root itself.

### Monorepo root output

A temporary monorepo with `apps/alpha` and `packages/beta`, each containing an indexed TypeScript and TOML file, produced:

- The existing `Workspace Scopes` section listing `apps/alpha` and `packages/beta`.
- One aggregate root statistics section:
  - Coverage: 4 indexed, 4 counted, 0 unavailable.
  - TOML: 28 code, 0 comments, 0 blanks, 28 total (2 files).
  - TypeScript: 12 code, 0 comments, 0 blanks, 12 total (2 files).
  - Total: 40 code, 0 comments, 0 blanks, 40 total (4 counted files).

This confirms that monorepo workspace navigation remains intact while root statistics cover the full indexed physical population.

## 최종 재검증 — 2026-09-19

The final scope/freshness corrections were checked from `/Users/buyong/workspace/private/buyong-mcp`:

- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo check --manifest-path apps/codemap-search/Cargo.toml`: exit 0, 2.98s. An earlier check caught a private-module type path; the final source uses the public `PublishedIndexSnapshot` re-export.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::codemap::`: 15 passed, 0 failed, 3.14s.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::`: 35 passed, 1 ignored, 0 failed, 13.50s.
- `PATH=/Users/buyong/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp::`: 23 passed, 3 failed, 148.17s. The failed names and assertions match the pre-existing failures reproduced at `9edd56999` in the find handoff.

Manual observations used `python3 -` to send JSON-RPC lines to the compiled `codemap-search mcp` binary. The temporary monorepo copied the existing `tests/fixtures/event_navigation/known.ts` and `config.toml` into `apps/alpha` and `packages/beta`, plus one more `known.ts` under `apps/alpha/nested`. Its root config set `config_auto_update=false`, `watch=false`, and `index_staleness_ms=86400000` to keep the observed generation stable.

| Request | Observed statistics |
| --- | --- |
| No path, absolute repository path, or root `format="llms-txt"` | One section; 5 indexed, 5 counted; 46 total lines |
| `path="apps/alpha"`, `path="alpha"`, `file_path="alpha"`, absolute project path, or `path="apps\\alpha"` | One section scoped to `apps/alpha`; 3 indexed, 3 counted; 26 total lines |
| `path="packages/beta"` | One section scoped to `packages/beta`; 2 indexed, 2 counted; 20 total lines |
| `path="apps/alpha/nested"` or `path="apps/alpha/known.ts"` | No statistics section |
| All the preceding requests with `is_overview_stats_enabled=false` | No statistics section |

After incrementing only the temporary `apps/alpha/known.ts` modification time by one second, the unchanged published generation returned `3 indexed, 2 counted, 1 unavailable`, `Collection: partial`, and `source changed since the published snapshot`. The counted total fell to 20 lines; the stale file was not silently included through its earlier cache entry.

`tools/list` still returned six tools, `overview` properties were exactly `format` and `path`, and both `overview` and `find` kept `readOnlyHint=true` / `openWorldHint=false`.

### 수집 예산과 재사용

A second bounded observation copied the 300 smallest tracked `.rs`, `.ts`, `.py`, `.js`, `.toml`, and `.json` files from `apps/codemap-search/tests/fixtures`, `apps/codemap-search/src`, `apps/mcp-server/src`, and `apps/scout/src`, ordered by `(size, path)`, into an isolated temporary workspace. The input was 716,960 bytes; all 300 files were indexed. Its config kept the published generation stable as above.

- First ready root response: 300 indexed, 256 counted, 0 unavailable, 44 pending; 24,212 code + 187 comments + 554 blanks = 24,953 total.
- Next root response: 300 indexed, 300 counted, 0 unavailable, complete; 26,761 code + 354 comments + 787 blanks = 27,902 total.
- Repeated root response: the same complete coverage and numeric totals.

This observes collection progressing across requests. Cache reuse is also established by the inspected path: validated cache hits return before decrementing the uncached-file budget. No speedup is inferred from these values.

Earlier attempts are not counted as successful budget verification: a 260-file source-only copy did not finish initial indexing within 45 seconds, and a fixture-only selection stopped before server startup because it contained fewer than 260 eligible tracked files.

## Limitations

- No performance speedup is claimed or measured.
- Unsupported tokei languages remain explicit coverage gaps.
- Whole-repository counting of non-indexed files remains out of scope.
- Source freshness follows the existing index's nanosecond modification-time convention. Edits that deliberately preserve the stored timestamp are not reliably detectable from that evidence; cache hits also compare size. The feature adds no persisted content fingerprint.
- New collection is bounded, but scope filtering and metadata validation still visit the selected indexed population.
- Global MCP acceptance is still blocked by the same three pre-existing search rendering failures recorded in the find handoff.
