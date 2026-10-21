# [feat] Rust related-terms lookup on the sense dataset alone

## Work Type
feat

## Current State (As-Is)
- [confirmed] As of `e473b5f48` on `feat/related-terms-lookup` (inspected 2026-10-17), the only lookup implementation is the Python PoC `apps/codemap-search/experiments/related-terms-poc/sense_lookup.py` (`rt-sense-lookup/2`) — Evidence: `DEFAULT_SENSE_DATASET` points at `datasets/rts5-75e02862daa29308/dataset.sqlite3`; `argparse` flags `--exact-first`, `--evidence-inflections`, `--split-senses-by-topic`, `--similarity-order` all default to on.
- [confirmed] The PoC resolves input words through the related-terms dataset `rtr1-930791c83bea6132` (about 430MiB, local only under `.codemap/poc/related-terms/thesaurus-v1/datasets/`) and the `rtpoc3` binary — Evidence: `sense_lookup.py` flags `--dataset` (help text "rt-related/1 SQLite used by rtpoc3 for input resolution") and `--rtpoc3`; `docs/related-terms/README.md` section "바로 조회하기".
- [confirmed] A 41MiB `rtr1` subset (the sense dataset's words plus the English inflected forms that inflection evidence reads, 263,163 entries) gave answers identical to full `rtr1` for 3,253 checked words — Evidence: `docs/related-terms/sense-mapping-log/13-related-dataset-dependency.md`, the sentence starting "뜻 데이터셋에 있는 단어(표제어, 관련어, 입력 단어 17만 6천 개)와, 굴절형 근거가 읽는 영어 굴절형만 남긴 rtr1 부분집합" and the row "줄인 판(벡터 없음)".
- [inferred] Matching against the sense dataset's own word keys alone, without the extra inflected forms, changes the resolution of some inflected inputs (for example `blocked`, `retried`) — Confirm by: the 176,351-word comparison in Stage 3 plus the 16 out-of-dictionary inputs listed in the stage-13 log.
- [confirmed] Base-form evidence in the PoC comes from `rtr1` relations; the sense dataset carries its own `form` relations (18,140) that can replace it — Evidence: `docs/related-terms/codemap-integration.md` section 4, row "기본형 다툼".
- [confirmed] Sense order in the PoC uses int8 768-dim vectors from `rtr1`; turning them off lowers first-sense hit by 1.7 percentage points — Evidence: `docs/related-terms/codemap-integration.md` section 4, row "뜻 정렬".
- [confirmed] The dataset uses the compact `rt-sense/5` layout: `relations(term_id, rank, word_key, surface_override, kind_id, persona)` with `PRIMARY KEY (term_id, word_key) WITHOUT ROWID`, `relation_kinds`, view `relation_texts`; only indexes `personas_by_mapping` and `personas_by_concept` remain — Evidence: `sqlite_master` of `datasets/rts5-75e02862daa29308/dataset.sqlite3`; `docs/related-terms/sense-mapping-log/16-compact-storage.md` section 2.
- [confirmed] Row counts: `terms` 18,566, `relations` 592,793, `input_senses` 334,187, `personas` 44,424, `topics` 2,545, `fields` 39 — Evidence: `select count(*)` on the rts5 file.
- [confirmed] Normalization rules live in `term_canonical.py` (`canonical_surface`, `exact_key`, `word_key`; regexes `SIGIL_PREFIX`, `MACRO_SUFFIX`, `CALL_SUFFIX`, `DUNDER`, `INNER_SEPARATORS`), syntax-only input parts in `syntax_parts.py`, and 11 confirmed spelling aliases in `spelling_aliases.py` `CONFIRMED_SPELLINGS` — Evidence: those module-level names.
- [confirmed] codemap-search already ships an identifier splitter and SQLite: `split_identifier()` in `apps/codemap-search/src/parser/tokenize.rs`; `rusqlite = { version = "0.40.2", features = ["bundled"] }` in `apps/codemap-search/Cargo.toml` — Evidence: those declarations.
- [confirmed] The MCP loop handles requests one at a time on a current-thread runtime, so one slow lookup blocks every other tool — Evidence: `apps/codemap-search/AGENTS.md` section "Sequential protocol with off-thread indexing".
- [confirmed] Cargo examples are an existing non-shipped route for developer programs — Evidence: `apps/codemap-search/examples/jev_decisions.rs`; `exclude` in `apps/codemap-search/Cargo.toml` also drops `/experiments` from the published crate.
- [inferred] Replacing `rtr1` base-form evidence with `form` relations changes the chosen base form for some inputs — Confirm by: the 176,351-word comparison in Stage 3.

## Desired Outcome (To-Be)
- A Rust lookup module inside codemap-search answers a batch of input terms using only an `rt-sense/5` SQLite file: no `rtr1`, no `rtpoc3`, no vectors, no Python.
- The lookup reads nothing at process start; the first call opens the database and prepares statements, and later calls reuse them plus an in-process cache.
- The lookup accepts identifiers (`appVersion`) and multi-word expressions (`abort signal`) and splits identifiers itself.
- A preparation step builds the three lookup indexes on a local, writable copy and records that they are ready, without ever modifying a file the user supplied; concurrent processes never see a half-copied or half-indexed file.
- Each answer carries, per sense, both the display groups (a word shown once) and the sense's full word set before de-duplication, so later filtering can judge each sense on its own words.
- Answers match the PoC run with `--no-similarity-order` for all 176,351 classified words, except differences traced to the base-form or input-match source.
- Measured latency and a per-call term cap are recorded for the MCP tool child.

## Scope
### In Scope
- New module `apps/codemap-search/src/related_terms/` with normalization, input resolution, sense collection, sense ordering, related-word collection, and preparation (indexes + ready marker).
- Behavior rules re-implemented from the PoC: normalization (`term_canonical.py`), syntax-only parts (`syntax_parts.py`), the 11 spelling aliases, whole expression → base form (`-s`, `-es`, `-ies`, `-ing`, `-ed`) → sub-expression matching, "inputs with their own senses do not widen to a base form", base-form ties decided from `form` relations and inflected incoming relations.
- Sense collection from own senses, `input_senses`, and `input_relations`; ordering: review-only senses last, own senses first, generation count, then topic word count.
- Related-word groups: direct relations, direct relations without a sense, near terms (same concept), same topic; a word shown once is not repeated in display groups, while each sense also keeps its full word set.
- Lookup indexes `relations(word_key)`, `relations(persona)`, `personas(topic)` and a ready marker holding dataset id and index version, built under a cross-process preparation lock.
- Developer example `apps/codemap-search/examples/related_terms_dump.rs` that prints one JSON line per input for comparison.
- Comparison script under `apps/codemap-search/experiments/related-terms-poc/sense-eval/` that runs the PoC and the example over the same words and classifies differences.
- Latency measurement and the per-call term cap.
- Evidence log `docs/related-terms/integration-log/01-lookup.md`.
### Out of Scope
- [hard] Do not port Python code line by line; implement from the documented behavior.
- [hard] Do not read `rtr1`, call `rtpoc3`, or add embedding vectors.
- [hard] Do not modify `apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3` or any other tracked dataset file; build indexes on a scratch copy.
- [hard] Do not add tests; verification is the comparison script and the latency log.
- [hard] Do not register an MCP tool, add config keys, or add a public CLI subcommand.
- [hard] Do not change the PoC lookup scripts (`sense_lookup.py`, `term_canonical.py`, `spelling_aliases.py`, `syntax_parts.py`).
- [deferred] Vector-based sense order, headword spelling (K7), sense order (K9), symbol-input noise (stage 15) — quality phase after integration.

## Constraints
- Rust module names, types, and constants follow `apps/codemap-search/AGENTS.md` conventions; lower layers return `Result<T, String>` or `Option`.
- Reuse `split_identifier()` for identifier splitting instead of a second splitter.
- Preparation copies a user-supplied file (`is_user_owned = true`) into `<global_dir>/related-terms/local/<dataset_id>/` before adding indexes, never into the download folder `<global_dir>/related-terms/<dataset_id>/`; `dataset_id` comes from `sense_meta.dataset_id`. The marker of a copy records the source path, size, and modification time, and a changed source is copied again.
- Preparation holds a cross-process lock (`prepare.lock` in the target folder, same pattern as `lock_registry()` in `apps/codemap-search/src/mcp/global_instructions/registry.rs`) from copying through writing the marker; a copy is written under a temporary name and renamed into place only when complete.
- Readers open a prepared file only after its ready marker exists; set a SQLite busy timeout so a reader waiting on another process's index build does not fail at once.
- Preparation rejects any file whose `sense_meta.schema_version` is not `rt-sense/5`, with a message naming the found version.
- Use `CREATE INDEX IF NOT EXISTS` so an interrupted preparation can rerun; write the ready marker only after all three indexes exist.
- The marker's index version is a constant in the binary; when a later binary has a different index version, preparation rebuilds the indexes on the existing file and rewrites the marker, without asking for a new download. A marker with the current version means no work.
- Add a dependency only if the existing crates cannot do the job, and explain it in the evidence log.
- Run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after Rust changes.
- The evidence log is Korean in "~다" style, like other `docs/related-terms` documents.

## Related Files / Entry Points
- `apps/codemap-search/experiments/related-terms-poc/sense_lookup.py` — behavior reference; read the lookup flow and the `--format json` answer shape first.
- `apps/codemap-search/experiments/related-terms-poc/term_canonical.py` — key rules to re-implement (`word_key`, `canonical_surface`, `exact_key`).
- `apps/codemap-search/experiments/related-terms-poc/syntax_parts.py` — syntax-only input parts the lookup skips or splits.
- `apps/codemap-search/experiments/related-terms-poc/spelling_aliases.py` — the 11 `CONFIRMED_SPELLINGS` to carry as constants.
- `apps/codemap-search/experiments/related-terms-poc/verify_sense_migration.py` — word population ("every word the base classifies") and difference classes to mirror in the comparison.
- `apps/codemap-search/experiments/related-terms-poc/sense-eval/score_eval_set.py` — scores expected-field hits on the 224 eval words.
- `apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3` — input dataset; copy before indexing.
- `apps/codemap-search/src/parser/tokenize.rs` — `split_identifier()` to reuse.
- `apps/codemap-search/src/lib.rs` — register the new module.
- `apps/codemap-search/src/mcp/global_instructions/registry.rs` — `lock_registry()` pattern for the preparation lock.
- `docs/related-terms/sense-mapping-log/13-related-dataset-dependency.md` — what the 41MiB subset contained and the out-of-dictionary inputs to re-check.
- `apps/codemap-search/src/related_terms/` (proposed) — new lookup module.
- `apps/codemap-search/examples/related_terms_dump.rs` (proposed) — comparison driver, modeled on `examples/jev_decisions.rs`.
- `apps/codemap-search/experiments/related-terms-poc/sense-eval/compare_rust_lookup.py` (proposed) — comparison script.
- `docs/related-terms/codemap-integration.md` — design sections 3 and 4 that this child implements.
- `docs/related-terms/integration-log/01-lookup.md` (proposed) — evidence log handed to the MCP tool child.

## Execution Plan
### Stage 1 — Pin the reference answers
- Starts when: The rts5 file exists at `apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3` with SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`, and the local `rtr1` dataset and `rtpoc3` binary that `sense_lookup.py` needs are present.
- Work: Record the starting commit (`git rev-parse HEAD`) for the side-effect checks, then produce the PoC reference answers for every word the rts5 dataset classifies (own senses or `input_senses`), using `sense_lookup.py --no-similarity-order --format json` with the other defaults, and fix the answer fields the comparison will read (resolved inputs, ordered senses as field and topic, ordered words per group).
- No-op when: None — the Rust lookup does not exist yet, so the comparison cannot already pass.
- No-op handoff: None — no no-change route exists for this child.
- Deliverable: A reference answer file outside the repository plus the word list, with the count recorded for the evidence log.
- Verify: `python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py token --no-similarity-order --format json`; Inputs: the rts5 dataset (CLI default); Expected: exit 0 and a JSON answer listing at least one sense for `token`.
- Ends when:
  - [ ] The word list has 176,351 entries and every entry has a reference answer.
- Handoff: Stage 2 receives the reference file, the word list, and the fixed answer fields.
- Replan when: The PoC cannot run because `rtr1` or `rtpoc3` is missing; stop and ask the user where the local copies are instead of rebuilding them.

### Stage 2 — Build the lookup and preparation
- Starts when: Stage 1 has fixed the answer fields.
- Work: Implement the Rust module: normalization, identifier splitting through `split_identifier()`, input resolution against the sense dataset's own word keys, base-form ties from `form` relations, the 11 spelling aliases, sense collection and ordering, related-word groups, preparation with the three indexes and the ready marker, lazy open with prepared statements, batch input, and an in-process cache. Add the example that prints one JSON line per input.
- Deliverable: Module `apps/codemap-search/src/related_terms/` (proposed) and example `apps/codemap-search/examples/related_terms_dump.rs` (proposed).
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the workspace with the new module; Expected: exit 0.
- Ends when:
  - [ ] `cargo run --release --manifest-path apps/codemap-search/Cargo.toml --example related_terms_dump -- <scratch copy> token appVersion "abort signal"` prints three JSON lines, and `appVersion` shows both the whole expression and its words as looked-up inputs.
  - [ ] Running preparation twice on the same scratch copy leaves exactly the three new indexes and one ready marker, and a second run makes no schema change.
  - [ ] Preparation on a file flagged as user-supplied leaves that file's SHA-256 unchanged and creates the indexed copy under `<global_dir>/related-terms/local/<dataset_id>/`.
  - [ ] Two preparations started at the same time on the same source leave one indexed copy, one marker, and `PRAGMA quick_check` returns `ok`.
  - [ ] The example's JSON shows, for a word whose senses share a related word, that word in only one display group but in the full word set of every sense that has it.
- Handoff: Stage 3 receives the example binary and a prepared scratch copy.
- Replan when: An answer field cannot be reproduced without `rtr1` data beyond base-form evidence; record the field and stop for a parent replan.
- Worker decision: File split inside `src/related_terms/`, cache size and eviction, marker file name and format.

### Stage 3 — Compare all words with the PoC
- Starts when: Stage 2's example runs on a prepared scratch copy.
- Work: Write the comparison script, run both lookups over the 176,351 words, and classify each answer as identical, resolution-differs (difference traced to the base form or the matched input word), or other. Fix the Rust lookup until "other" is empty, then score both sides on the 224 eval words with expected fields.
- Deliverable: Comparison report outside the repository and script `apps/codemap-search/experiments/related-terms-poc/sense-eval/compare_rust_lookup.py` (proposed).
- Verify: `python3 apps/codemap-search/experiments/related-terms-poc/sense-eval/compare_rust_lookup.py`; Inputs: the Stage 1 reference file, the example binary, the prepared scratch copy; Expected: exit 0 and a report whose class counts sum to 176,351 with "other" = 0.
- Ends when:
  - [ ] Every resolution-differs answer names the input word and both chosen base forms or matched words.
  - [ ] Expected-field hits on the 224 eval words are recorded for the PoC (`--no-similarity-order`) and for Rust.
- Handoff: Stage 4 receives the report counts, examples per class, and eval-set hits.
- Replan when: Resolution differences change the expected-field hit count on the eval set by more than zero words; report the words to the parent before continuing, because it changes the quality baseline the later children compare against.

### Stage 4 — Measure latency and hand off
- Starts when: Stage 3 reports "other" = 0.
- Work: Measure cold first call (open plus statement preparation), single-term p50/p95/max over the 240 eval-set identifiers and a fixed-seed 1,000-word sample, and full-call time for batches; choose a per-call term cap from the measured worst case. Write the evidence log with the API entry points, comparison results, eval hits, latency, cap, and any added dependency with its reason.
- Deliverable: Evidence log `docs/related-terms/integration-log/01-lookup.md` (proposed) listing the module path, the preparation and lookup function names, the result type (display groups and per-sense full word sets), the preparation lock and folder layout, the comparison class counts, the eval hits, the latency table, and the per-call term cap.
- Verify: `bounded inspection of docs/related-terms/integration-log/01-lookup.md`; Inputs: the log file; Expected: it contains the class counts with "other" = 0, a latency table with p50/p95/max in milliseconds, and one integer per-call term cap.
- Ends when:
  - [ ] `cargo check --manifest-path apps/codemap-search/Cargo.toml` exits 0 on the final state.
- Handoff: The MCP tool child (`docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`) receives `docs/related-terms/integration-log/01-lookup.md` and the module `apps/codemap-search/src/related_terms/`.
- Replan when: Single-term p95 exceeds the time of a `search` call on the same machine; investigate missing index use before handing off.

## Side Effect Checkpoints
- [ ] `shasum -a 256 apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3` still prints `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced` after all stages.
- [ ] `git diff --stat <starting commit> -- apps/codemap-search/experiments/related-terms-poc/datasets/ apps/codemap-search/experiments/related-terms-poc/sense_lookup.py apps/codemap-search/experiments/related-terms-poc/term_canonical.py apps/codemap-search/experiments/related-terms-poc/spelling_aliases.py apps/codemap-search/experiments/related-terms-poc/syntax_parts.py` is empty, using the commit recorded in Stage 1, so staged or committed changes are caught too.
- [ ] `cargo package --list --manifest-path apps/codemap-search/Cargo.toml` lists no `.sqlite3` file.
- [ ] `codemap-search mcp` start-up does not open the dataset: the new module is not called from `main.rs` or `src/mcp/` in this child.
- [ ] Existing e2e tests still pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests` exits 0.

## Acceptance Criteria
- [ ] The comparison report covers 176,351 words with "other" = 0, and every resolution-differs answer is explained in the evidence log.
- [ ] The Rust lookup answers `token`, `appVersion`, and `abort signal` from an `rt-sense/5` file with no `rtr1` path, no `rtpoc3` path, and no vector data configured.
- [ ] Preparation never changes the SHA-256 of a user-supplied file and creates exactly `relations(word_key)`, `relations(persona)`, and `personas(topic)` on the copy.
- [ ] The evidence log records cold-call time, single-term p50/p95/max, and the chosen per-call term cap.

## Open Questions
- None — the user decided dataset-only Rust lookup, no vectors in phase 1, and no tests for this child; remaining choices are bounded in Constraints and Worker decision.
