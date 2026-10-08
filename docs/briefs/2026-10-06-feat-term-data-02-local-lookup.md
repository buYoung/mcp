# [feat] 인덱싱 시점 어휘 통계와 로컬 데이터셋 조회 구현

## Work Type
feat

## Current State (As-Is)
- [confirmed] Authoring inspected `main` at `46995808c` on 2026-10-06. This child implements only an independent phase-1 experiment, not a new production MCP contract — Evidence: current Git revision and the user's selection of phase-1 detail plus later entry conditions.
- [confirmed] The product exposes library modules for parsing, indexing, and configuration. `TantivySearchEngine` publicly exposes its index, reader, schema, and field handles — Evidence: `apps/codemap-search/src/lib.rs` and the struct in `src/index/engine.rs`.
- [confirmed] `apply_index_updates()` reads/extracts changed files, deletes prior documents, adds one document per file, commits, and reconciles its cache. Full and path-scoped refreshes share this route — Evidence: `apps/codemap-search/src/index/engine.rs`, `apply_index_updates`, `index_files_changed_with_reload`, and `refresh_paths_with_reload`.
- [confirmed] `load_published_snapshot()` constructs derived state from committed documents. `publish_pass_result()` builds that state before publishing the reader and snapshot under the generation gate — Evidence: `apps/codemap-search/src/index/engine.rs` and `src/index/indexer.rs`, those functions. A new repository vocabulary table does not yet exist there.
- [confirmed] Searchable text comprises symbol/owner text and subtokens, docstrings, path parts, capped literals, definition bodies, references, and format text — Evidence: schema creation and document construction in `TantivySearchEngine::new()` and `apply_index_updates()`.
- [confirmed] The inspected Tantivy 0.26.2 `Searcher::doc_freq()` sums field-specific term metadata. Raw posting frequencies do not exclude deleted documents, and `doc_freq_given_deletes()` scans postings separately — Evidence: the installed dependency's `core/searcher.rs`, `index/inverted_index_reader.rs`, and `postings/segment_postings.rs`. Summing per-field frequencies is not an exact live-file union.
- [confirmed] The dependency exposes segment readers, term dictionaries, postings, and deleted-document checks — Evidence: the inspected Tantivy 0.26.2 `terms()`, `read_postings()`, `read_postings_from_terminfo()`, and `SegmentReader::is_deleted()` APIs. No PoC implementation or timing was executed during authoring.
- [confirmed] `split_identifier()` separates camel/snake/hyphen forms but does not split an ordinary phrase on whitespace. The CLI returned one token for `event bus` and two for `event-bus` during authoring — Evidence: `apps/codemap-search/src/parser/tokenize.rs` and the recorded CLI observations.
- [confirmed] Global config resolves through `CODEMAP_HOME` or `~/.codemap`; the product already uses bundled SQLite. The existing analysis database is writable telemetry, not a dataset store to repurpose — Evidence: `config::global_dir()`, `apps/codemap-search/Cargo.toml`, and `src/analyze/storage.rs`.
- [confirmed] Walking and indexing use cwd-relative workspace boundaries and ignore policies. Rooting a walk at an ignored `.codemap` descendant from the main repository can hide that population — Evidence: `workspace::walk_root_is_visible()`, `build_walker()`, and `engine::is_path_visible_to_walk()`. Index each isolated checkout from its own cwd rather than disabling protective exclusions.
- [inferred] Materializing an exact live-file vocabulary during index finalization should support lookup without rescanning source or postings, but its size, build cost, and recovery behavior are unmeasured — Confirm by: this child's lifecycle evidence and child 03's measurements.

## Desired Outcome (To-Be)
- A standalone Rust experiment opens the generated SQLite dataset locally and returns source-traceable candidates through a documented internal lookup interface.
- Repository word/file statistics are computed and persisted during indexing/finalization, before the experimental state becomes ready. Serving lookup only reads published data and cached counts.
- Lookup distinguishes unavailable, incompatible, stale, absent, and genuinely empty results; it never silently falls back to network access, model calls, a repository walk, or search-result counting.
- An exact handoff identifies the dataset version, repository generation, commands, result structure, and actual lifecycle verification for child 03.

## Scope
### In Scope
- Add an independent `publish = false` experiment crate under `apps/codemap-search/experiments/related-terms-poc/`, with its own workspace boundary and Cargo.lock, following the regex PoC precedent.
- Implement a read-only SQLite dataset reader, an indexing/finalization path for repository vocabulary, a persisted generation-aware statistics snapshot, and an offline lookup path.
- Reuse the product library and matching existing dependency versions without modifying the main crate's public/private visibility or production schema.
- Implement bounded diagnostics for candidate normalization, source relation kinds, repository presence, commonness policy, rejection reasons, and artifact compatibility.
- Select a bounded non-empty target workspace population and freeze it in `workspaces.json`, recording source URL/revision or reproducible local snapshot identity, selected paths, language/domain coverage, private checkout location, effective configuration, and any overlap with child 01's corpus.
- Record full rebuild, changed-file, deleted-file, restart, and failed-publication observations using isolated copies of real source files.
### Out of Scope
- [hard] Do not modify product source, main Cargo.toml/Cargo.lock, MCP schemas/descriptions, existing public CLI commands, or production configuration keys.
- [hard] Do not change `benchmark/`, write into its managed checkout, run paid evaluation, invoke LLM judgment, or use embeddings.
- [hard] Do not download a dataset, consult upstream APIs, execute searches to count files, or lazily scan postings/source from a serving lookup.
- [hard] Do not reuse `.codemap/analysis.sqlite3`, write into the user's global directory, or alter the user's actual working repository for lifecycle observations.
- [deferred] GitHub latest-dataset-release discovery and global installation. The user selected this future delivery direction, but PoC inputs are explicit local files only.
- [deferred] Production MCP exposure, query expansion/ranking changes, suggested-search block contracts, task registration changes, and automatic identifier rewriting.

## Constraints
- Consume `apps/codemap-search/experiments/related-terms-poc/results/dataset-handoff.json` exactly. Preserve its schema, semantic identifiers, source references, normalization version, and logical dataset identity.
- The corpus-independent SQLite dataset is read-only and shareable. Repository-specific words, file identities, paths, and counts remain in separate private workspace artifacts; never modify the shared dataset with local observations.
- Default statistics cover the union of the existing searchable text fields for each live indexed file. Count a file at most once for a word even when it occurs repeatedly or in several fields. Use the same live indexed-file population for the denominator.
- Build repository vocabulary independently of the current dataset's candidate list so a compatible dictionary update does not inherently require source reindexing. Record statistics normalization/field-policy versions and reject incompatible combinations instead of silently mixing them.
- Use the same analyzed token semantics as the indexed fields. Do not treat whitespace splitting of raw source, `rg -w`, stored uncapped literals, or stored extracted symbols alone as equivalent to the full search index.
- A viable no-product-change route is to enumerate live postings after committing the private index, union documents across fields, and persist the vocabulary before ready publication. This is index-finalization work, not a lookup fallback. Record full rebuild cost honestly.
- Keep phrase display forms intact. Return per-component token counts explicitly when exact phrase/co-occurrence counts were not materialized. Do not report a phrase file count as the sum/minimum of token counts or claim adjacency from repository-wide token presence.
- Statistics and the index must identify the same committed generation. Publish only complete artifacts. On a failed update, keep a provably matching prior pair with a stale notice or report not ready; never combine a new index with old counts as ready.
- Reconcile counts with committed live documents, including deletion and parse/encoding failure behavior. For incremental maintenance, retain enough prior per-file membership to subtract old contributions; a verified index-time full rebuild is an allowed first implementation.
- Persist vocabulary so restart loads metadata/counts rather than rebuilding them in the first lookup. An incompatible/missing artifact becomes unavailable until an explicit indexing action repairs it.
- Open the dictionary without create/write flags and use bound SQL parameters. Validate schema and data identity before lookup; a missing file must not turn into an empty newly created database. Do not apply SQLite `immutable=1` to any file that can change in place.
- Hold an artifact version for a run. Build replacements separately and switch only at a defined boundary; do not mutate a database beneath readers. Exact future downloader scheduling and release asset names remain unchosen.
- Keep raw expressions distinct from normalized lookup keys and identifier subtokens. Any experimental limits, commonness cutoffs, and relation ordering are versioned hypotheses frozen before child 03, not inherited approved values from the earlier brief.
- Empty results are legitimate. Do not pad output with unrelated candidates, infer synonyms from repository presence, compare uncalibrated source scores as probabilities, or perform transitive semantic expansion.
- Run indexing from each private checkout's cwd with an isolated `CODEMAP_HOME` and recorded effective configuration. Do not bypass `.codemap`, ignore, size, or source-format safeguards to make an accidentally empty corpus appear valid. Keep one index writer per workspace.

## Related Files / Entry Points
- `apps/codemap-search/experiments/related-terms-poc/results/dataset-handoff.json` (proposed) — verify child 01's exact dataset and normalized contract before implementing the reader.
- `apps/codemap-search/experiments/related-terms-poc/workspaces.json` (proposed) — pin the exact non-empty target population and reproducible checkout/snapshot route used for indexing and subsequent evaluation.
- `apps/codemap-search/experiments/related-terms-poc/Cargo.toml` (proposed) — create the standalone experiment crate and matching dependency lock.
- `apps/codemap-search/experiments/related-terms-poc/src/` (proposed) — colocate dataset reading, vocabulary materialization, and experimental command adapters by responsibility.
- `apps/codemap-search/experiments/regex-re2-poc/Cargo.toml` — copy only the independent-crate and `publish = false` conventions.
- `apps/codemap-search/src/index/engine.rs` — trace `apply_index_updates`, exposed reader/index fields, and `load_published_snapshot` before choosing finalization boundaries.
- `apps/codemap-search/src/index/indexer.rs` — preserve the existing commit/derived-state/publication principle without editing the production indexer.
- `apps/codemap-search/src/parser/tokenize.rs` — reuse identifier/query token behavior where applicable and keep phrase handling explicit.
- `apps/codemap-search/src/workspace.rs` — preserve walk eligibility and cwd boundaries in isolated input workspaces.
- `apps/codemap-search/src/config.rs` — inspect `global_dir`, `init`, and effective settings to isolate the PoC from user configuration.

## Execution Plan
### Stage 1 — 데이터셋 판독기와 로컬 실행 경계 구성
- Starts when: `apps/codemap-search/experiments/related-terms-poc/results/dataset-handoff.json` exists with verified artifact hashes, a valid non-empty SQLite dataset, attribution, and the canonical contract.
- Work: Build the independent Rust crate, verify read-only SQLite access and exact-key indexed lookup, and define the experimental output envelope without introducing production tool/config names. Select and pin disposable target checkouts/snapshots in workspaces.json, verify their non-empty eligible population, and isolate their configuration. Do not use a benchmark-managed checkout.
- No-op when: An existing experimental reader and index-time vocabulary pipeline already satisfy this entire brief on the handed-off dataset and have current lifecycle evidence with matching artifact identities.
- No-op handoff: Child 03 receives `apps/codemap-search/experiments/related-terms-poc/results/lookup-handoff.json` with verified existing commands/artifacts and complete lifecycle observations. A failed proof stops successors, returns to this child for bounded correction/re-verification, and updates the parent handoff before continuation.
- Deliverable: The local crate, its lockfile, `apps/codemap-search/experiments/related-terms-poc/workspaces.json`, and `apps/codemap-search/experiments/related-terms-poc/contracts/lookup.schema.json`, documenting candidate provenance, counts, readiness, generation, and errors.
- Verify: `Inspect the reader's open/query path and decode source-traceable records from the handed-off SQLite artifact`; Inputs: dataset-handoff.json, its SQLite file, the reader implementation, lookup.schema.json, and workspaces.json; Expected: a non-empty indexed lookup with preserved sense/source IDs, no writes/network calls, explicit missing/incompatible-artifact errors, and reproducible non-empty target workspaces.
- Ends when:
  - [ ] Dataset loading never creates or migrates a supplied SQLite file and the source contract survives the final returned record.
  - [ ] Serving lookup is separate from the indexing command and cannot silently initialize an index or acquire a writer.
- Handoff: Stage 2 receives the reader/output contract and isolated workspace route.
- Replan when: A required API is private or unavailable in the actual dependency version. Investigate a public committed-index route in this child; if product edits are unavoidable, return to the parent instead of changing the hard boundary.

### Stage 2 — 인덱싱 완료 전 어휘 통계 생성
- Starts when: The reader contract and non-empty isolated input workspace are established.
- Work: Implement materialization and persistence of exact per-word live-file counts and the denominator during index finalization. Bind the statistics to committed generation and policy identity, then make lookup consume only the published result.
- Deliverable: Private index/statistics artifacts under `.codemap/poc/related-terms/`, with `apps/codemap-search/experiments/related-terms-poc/results/index-stats.json` recording generation, non-zero populations, policy versions, timing, bytes, and comparison evidence.
- Verify: `Compare the published frequency table with an independent index-finalization enumeration of live file memberships`; Inputs: the private committed index, all selected text fields, published statistics, and index-stats.json; Expected: identical word/file counts and denominator, no cross-field/repetition double counting, and readiness only after complete statistics publication.
- Ends when:
  - [ ] A repeated word in one file contributes one, and the same word in separate live files contributes their unique file count.
  - [ ] Deleted documents are excluded even when their raw term metadata remains in a segment.
  - [ ] Phrase/component evidence and absent/unavailable states cannot be confused in the output.
- Handoff: Stage 3 receives the materializer, persisted generation-aware artifacts, and lookup wiring.
- Replan when: Full-field exactness, population visibility, or matching-generation publication cannot be established. Stop serving-ready claims and correct indexing in this child; never move counting into lookup to bypass the failure.

### Stage 3 — 갱신·재시작·오프라인 조회 검증
- Starts when: The initial private index and published statistics agree.
- Work: Observe add/change/delete and failure cases on scratch copies of existing corpus files. Restart against persisted state, execute repeated offline lookups, and record that serving calls neither read source/postings nor regenerate counts. Keep lifecycle probes separate, then recreate the pinned baseline targets and publish their matching index/statistics before the final handoff. Document exact experimental commands and pass required Cargo checks.
- Deliverable: `apps/codemap-search/experiments/related-terms-poc/results/lookup-handoff.json`, containing dataset-handoff reference, dataset/policy IDs, crate/source revision, build profile, `workspace_manifest_path`, `index_generation`, local index/statistics paths and hashes, effective configuration, `commands`, `output_schema_path`, `lifecycle_evidence_path`, `offline_lookup_evidence`, `technical_status`, and `limitations`.
- Verify: `cargo check --manifest-path apps/codemap-search/experiments/related-terms-poc/Cargo.toml && cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the standalone crate, its lockfile, and the unchanged main crate; Expected: both commands exit 0, with lifecycle and offline observations additionally recorded in the handoff rather than inferred from compilation.
- Ends when:
  - [ ] Change/delete observations update counts correctly, restart preserves matching state, and failed publication never exposes mismatched counts as ready.
  - [ ] Repeated lookups in a fresh offline process use the local dataset and persisted statistics only; a bounded call-path audit and execution observations support the claim.
  - [ ] README.ko.md contains the actual local commands and schema/normalization compatibility behavior.
- Handoff: Child 03 consumes `apps/codemap-search/experiments/related-terms-poc/results/lookup-handoff.json` only after its technical status is passing.
- Replan when: Any lifecycle or offline proof fails. Stop child 03, return to the responsible reader/indexing stage, correct and re-verify, then update parent topology/handoffs if the scope changed.

## Side Effect Checkpoints
- [ ] `git status --short` and `git diff --stat` show no product source, production Cargo, benchmark, or global-configuration changes; the historical untracked brief is preserved.
- [ ] `git ls-files .codemap/poc/related-terms` remains empty, and all destructive lifecycle operations are confined to recorded disposable copies, not original repositories.
- [ ] The SQLite dataset hash is unchanged by lookup and repository indexing. Local code vocabulary never enters canonical shared JSONL or its SQLite artifact.
- [ ] No new MCP tool, production CLI command, release publication, downloader, or existing analysis-database migration was introduced.
- [ ] The verification handoff distinguishes code inspection, executed observations, Cargo results, and unmeasured performance; no new package test suite, lint, or formatter setup was added.

## Acceptance Criteria
- [ ] A local experimental lookup returns genuine dataset records with preserved semantic/source evidence and uses a matching, non-empty index-time vocabulary snapshot for repository filtering.
- [ ] Initial build, modification, deletion, restart, and failed-publication evidence establishes unique live-file counting and matching generations without lookup-time reconstruction.
- [ ] A serving-path audit covering all new lookup entry points, plus offline execution evidence, shows no network, model calls, source walk, postings scan, search-result enumeration, or count recomputation in that path.
- [ ] Both recorded Cargo checks pass on the final experiment state, and child 03 receives a valid lookup handoff with reproducible commands and explicit limitations.

## Open Questions
- None — local access and index-time counting are user-decided. Production release/download timing, global asset layout, and public API names remain deferred rather than guessed.
