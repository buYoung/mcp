# [feat] Prove a general related-terms dictionary offline

## Work Type
feat

## Current State (As-Is)
- [confirmed] Inspected on local `main` at `82cd08f4b`; local `main` is 10 commits behind the local `origin/main` ref `46995808c` (not fetched) — Evidence: `git rev-list --left-right --count HEAD...origin/main` → `0 10`.
- [confirmed] codemap-search has no related-term, synonym, or query-expansion support; search parses the raw query with Tantivy `QueryParser` over symbol, docstring, path-part, literal, definition-body, reference, and format-text fields — Evidence: `search_with_context_in_scope()` in `apps/codemap-search/src/index/ranking.rs` (`QueryParser::for_index`), `QueryTokens::parse()` in `apps/codemap-search/src/parser/tokenize.rs`.
- [confirmed] Indexed text fields use the Tantivy default `TEXT` analyzer (lowercasing, no stemming); symbol names are indexed together with their `split_identifier()` sub-tokens and owner names — Evidence: schema builder in `TantivySearchEngine::new()` and the `doc.add_text(self.symbol_field, …)` loop in `apps/codemap-search/src/index/engine.rs`.
- [confirmed] Vocabulary mismatch observed: `codemap-search search "watcher" --limit 5` ranks `apps/codemap-search/src/index/watcher.rs` first, `watchers` and `file change notification` do not return it in the top 5, and appending `notify watch watcher` restores it to rank 1 — Evidence: CLI runs from the repository root on 2026-10-06.
- [confirmed] Unbounded expansion degrades ranking: `event bus` returns `src/events/` sources, but `event bus emitter dispatch publish subscribe channel message queue worker` fills ranks 2–8 with experiment `*.lock.json` / `source-selection*.json` files — Evidence: same CLI session, `--limit 8`.
- [confirmed] Reusable CLI surfaces without source changes: `parse <file>` prints JSON symbols with `name`, `kind`, `range`, `docstring` (a directory argument fails with `IsADirectory`); `tokenize <identifier>` prints split tokens (`EventEmitterHub` → `event`, `emitter`, `hub`); `index <dir>` and `search <query> --limit N` print ranked file paths only — Evidence: `Commands` enum in `apps/codemap-search/src/main.rs` and command output.
- [confirmed] The package exposes a library target, and a standalone Rust crate already exists under experiments — Evidence: `apps/codemap-search/src/lib.rs`, `apps/codemap-search/experiments/regex-re2-poc/Cargo.toml` with its own `Cargo.lock`.
- [confirmed] Experiment convention: `event-navigation-poc` keeps Python scripts (`prepare.py`, `run.py`, `evaluate.py`), pinned `corpus.json`, `cases.json`, `results/`, and a Korean `README.ko.md`; `/experiments` is excluded from crate packaging — Evidence: `apps/codemap-search/experiments/event-navigation-poc/` listing, `exclude` in `apps/codemap-search/Cargo.toml`.
- [confirmed] Gold set exists: `benchmark/v2/data/dataset.json` holds 36 Grafana questions (12 `simple`, 12 `medium`, 12 `complex`) with Korean `prompt`, `entrypoint`, and `evidence[].path`, at `source_commit` `c6fad8695a96577eb466d425e6ac4a759ca30f47`; medium/complex prompts describe behavior without exact identifiers — Evidence: dataset keys and sampled prompts (`medium-go-1`, `complex-ts-3`, `complex-go-5`).
- [confirmed] The benchmark harness manages its own Grafana checkout through `prepare_grafana()` and refuses to overwrite a checkout with a different commit or modified tracked files — Evidence: `prepare_grafana(checkout, emit=emit)` call in `benchmark/resources.py`, `validate_grafana()` in `benchmark/ready.py`, `benchmark/README.md` 준비 section.
- [confirmed] Target product flow (user-agreed, for the deferred integration): user query → client calls `initial_instructions` with `task_query` (required string) and `terms` (required array of strings, usually 2+ items) → server returns related search terms → existing search flow continues. On `origin/main`, `initial_instructions` currently takes no arguments and Jev registration is a separate `register_task` tool — Evidence: `git show origin/main:apps/codemap-search/src/tools/mod.rs` (`"name": "initial_instructions"`, `"properties": {}`), `git show origin/main:apps/codemap-search/src/mcp/mod.rs` (`"register_task" =>` branch).
- [confirmed] Permitted public sources (checked 2026-10-06 via GitHub API and source pages): `abbrcode/abbreviations-in-code` MIT; `wuyichen24/software-engineering-abbreviations-acronyms` CC0-1.0; `github/explore` CC-BY-4.0 with 1,278 `topics/` entries carrying `aliases` and `related` front matter (e.g. `topics/pubsub/index.md` lists `event-bus`, `message-queue`, `observer-pattern`); Wikidata aliases CC0 (e.g. Q1071360 → `pub/sub`, `pubsub`) — Evidence: repository `license.spdx_id`, raw `topics/pubsub/index.md`.
- [confirmed] Rejected sources: Moby Thesaurus II gives general-English senses (`event` → accident, act, case; `queue` → braid, bun; `cache` → asylum; no `retry`); SWordNet public data is a Linux-kernel subset only; SEWordSim download link is dead; ConceptNet and Stack Overflow tag synonyms are CC BY-SA; the SCANL abbreviation artifact has no license; SEthesaurus data license is unverified — Evidence: Gutenberg #3202 `mthesaur.txt`, SWordNet `linux_id_100.csv`, GitHub API.
- [inferred] Identifier/docstring co-occurrence finds related words but misses synonyms that rarely share one identifier (e.g. `subscribe` vs `listen`) — Confirm by: Stage 2 sample review, counting how many unrelated-or-missing labels are synonym gaps.
- [inferred] A word's file count in the target repository can be approximated without source changes by counting `codemap-search search "<word>" --limit <large>` output lines — Confirm by: Stage 3 spot check against `rg -l -i -w <word>` counts.

## Desired Outcome (To-Be)
- A reproducible offline PoC under `apps/codemap-search/experiments/related-terms-poc/` builds a general, word-level related-terms dictionary from pinned open-source repositories plus permitted public sources, with no embeddings and no model calls at lookup time.
- Each dictionary entry maps a word or short phrase key to a related word with relation type (`abbreviation`, `synonym`, `related`), cross-repo support count, score, and source id; each key keeps at most 20 entries; every entry is traceable to a source and license.
- For each evaluation case, the PoC simulates the planned step 3 of the product flow: take a frozen `terms` list, look up the dictionary one hop, keep only words present in the target repository, drop overly common words, and render a related-search block with suggested queries.
- A Korean `README.ko.md` reports go/no-go against the user-approved thresholds with measured numbers, denominators, failure examples, and limits.

## Scope
### In Scope
- Select, pin, and clone a mining corpus of open-source repositories, excluding Grafana.
- Extract words with codemap-search's identifier splitting: symbol sub-tokens, owner names, and docstring words.
- Count co-occurrence within one identifier, between a symbol name and its docstring, between owner and member, and within one file; score pairs and apply the cross-repo support rule.
- Merge abbreviation and alias entries from the permitted sources with relation types and license records.
- Simulate the repository filter and block renderer on a separate Grafana clone at the pinned commit.
- Evaluate on all 36 benchmark questions: medium and complex cases measure effect, simple cases act as the identifier-query control.
- Review a seeded random sample of 200 dictionary pairs.
- Write `README.ko.md` and a third-party attribution file.
### Out of Scope
- [hard] Do not modify `apps/codemap-search/src/`, tool descriptions, tool schemas, or config keys.
- [hard] Do not modify `benchmark/` code or data; read `benchmark/v2/data/dataset.json` only, and do not write into the benchmark-managed Grafana checkout.
- [hard] Do not run `pnpm bench:start` or any paid model evaluation.
- [hard] Do not use embeddings or vector similarity.
- [hard] Do not include CC BY-SA, unlicensed, or license-unverified data in the dictionary: ConceptNet, Stack Overflow tag synonyms, SCANL, SEthesaurus data, SWordNet data, Moby Thesaurus.
- [hard] Do not commit cloned corpus repositories or raw intermediate data.
- [deferred] Server integration on `origin/main`: `initial_instructions` with required `task_query` string and required `terms` array, the related-search block in its response, and Tantivy `Searcher::doc_freq` replacing the PoC's CLI file-count approximation.
- [deferred] Codebase-specific dictionary learned from search routing (candidate → confirmed entries under `.codemap/`).
- [deferred] Ranking integration of expanded terms inside `search`.
- [deferred] Real-LLM adoption measurement of suggested queries.
- [deferred] Shared-neighbor synonym mining or model-generated synonym candidates; record as follow-up if Stage 2 shows synonyms are the main gap.
- [deferred] Korean ↔ English term mapping.

## Constraints
- Lookup uses only the precomputed dictionary and target-repository counts; counting statistics such as PMI or NPMI are allowed, vectors are not.
- Corpus: pin each repository commit in `corpus.json`, cover at least 6 languages and at least 5 application domains, exclude Grafana, and clone under `.codemap/poc/related-terms/corpus/` (ignored by the root `.gitignore` entry `.codemap/`).
- Generality rule: keep a related pair only when it appears in at least 10 distinct corpus repositories; the worker may tune this between 5 and 20 using only the Stage 2 sample review, and must record the final value.
- Never tune thresholds, filters, or scoring against Grafana evaluation results.
- Evaluation `terms`: derive each case's list from `prompt` only (never `evidence` or `entrypoint`), in English, 2–8 items, and freeze `cases.json` before the first evaluation run.
- Block limits: one hop only; at most 5 related words per term; drop words with 0 target-repo files and words found in more than 20% of target-repo files; words of 3 characters or fewer only through an `abbreviation` relation; at most 5 suggested queries of 2–3 words each; whole block at most 2048 bytes UTF-8.
- Do not expand identifier-shaped terms: a term with more than one `split_identifier()` token, or containing `::` or `.`, passes through unchanged.
- Thresholds are fixed by the user before evaluation: effect ≥ ⌈M/3⌉ recovered cases, sample precision ≥ 70%, block ≤ 2048 bytes.
- Commit the dictionary output only if it is at most 10 MB; otherwise keep it under `.codemap/poc/related-terms/` and commit stats, sample review, and the build command.

## Related Files / Entry Points
- `apps/codemap-search/experiments/related-terms-poc/` (proposed) — PoC home for scripts, `corpus.json`, `cases.json`, `sources/`, `results/`, `README.ko.md`.
- `apps/codemap-search/experiments/event-navigation-poc/` — copy its layout and Korean README conventions before creating files.
- `apps/codemap-search/experiments/regex-re2-poc/Cargo.toml` — precedent for a standalone experiment crate if the Rust extraction route is chosen.
- `apps/codemap-search/src/parser/tokenize.rs` — `split_identifier()` defines the sub-token rule that dictionary keys and the identifier-shape check must match.
- `apps/codemap-search/src/main.rs` — `Commands` enum: `parse`, `tokenize`, `index`, `search` used by the PoC.
- `apps/codemap-search/src/index/engine.rs` — read `TantivySearchEngine::new()` to know which fields and analyzer a search word can match; do not edit.
- `apps/codemap-search/src/index/ranking.rs` — read `search_with_context_in_scope()` to interpret CLI ranking; do not edit.
- `benchmark/v2/data/dataset.json` — source of the 36 evaluation cases and gold paths.
- `benchmark/resources.py` — `prepare_grafana()` call shows where the managed Grafana checkout lives; use it only to avoid writing there.
- `.gitignore` — confirm the `.codemap/` entry before placing clones and intermediates.
- `apps/codemap-search/src/tools/mod.rs` — current `initial_instructions` entry in `list_tools()`; context for the deferred integration only.

## Execution Plan
### Stage 1 — Freeze PoC inputs
- Starts when: A Grafana clone separate from the benchmark-managed checkout is available at `c6fad8695a96577eb466d425e6ac4a759ca30f47`, and the `codemap-search` binary used for the PoC is built from one recorded commit with its version and hash recorded.
- Work: Create the PoC directory; choose and pin the mining corpus; build `cases.json` from the dataset with frozen `terms`; fetch the permitted sources with license records.
- Deliverable: `corpus.json` (URL, commit, language, domain, license), clones under `.codemap/poc/related-terms/corpus/`, `cases.json` (id, difficulty, prompt, terms, gold paths), `sources/` manifests (URL, license, fetch date).
- Ends when:
  - [ ] `corpus.json` covers at least 6 languages and 5 domains and contains no Grafana repository.
  - [ ] `cases.json` has 36 cases, each with 2–8 English terms derived from `prompt` only and at least one gold path from `evidence[].path`.
  - [ ] Every source manifest records URL, license, and fetch date.
- Handoff: Stage 2 receives the pinned clones and source manifests; Stage 3 later receives `cases.json` and the Grafana clone.
- Replan when: The pinned Grafana commit cannot be cloned — stop and ask the user; disk limits block the corpus — shrink it while keeping 6 languages and 5 domains, and record the reduction.
- Worker decision: Exact repository choice within the corpus constraints; build the binary from `origin/main` (after `git fetch`, in a separate worktree so the current checkout is not switched) or from local `main`, and record which.

### Stage 2 — Build the general dictionary
- Starts when: Stage 1 clones and source manifests exist.
- Work: Extract words, count co-occurrence per unit, score pairs, apply the cross-repo support rule and the top-20 cap, then merge abbreviation and alias entries with relation types.
- Deliverable: Dictionary file (key, related, relation, support, score, source), `results/dictionary-stats.json` (keys, pairs, bytes, support threshold, extraction throughput), `results/sample-review.tsv` (200 seeded random pairs labeled related/unrelated in the code sense with a one-line reason, labeled by the worker against a written rubric saved next to the sample).
- Ends when:
  - [ ] One documented command regenerates the dictionary deterministically from Stage 1 inputs.
  - [ ] Sample precision is computed over 200 pairs with the recorded seed.
  - [ ] Stats report keys, pairs, size, and the final support threshold.
- Handoff: Stage 3 receives the dictionary and stats.
- Replan when: Sample precision stays below 50% after one threshold adjustment — stop and report the dominant failure pattern to the user before Stage 3; extraction is too slow to finish the corpus in one working session — switch extraction route or shrink the corpus and record why.
- Worker decision: Extraction route (per-file `codemap-search parse` from Python, or a standalone experiment crate using the library by path), PMI vs NPMI, support threshold within 5–20.

### Stage 3 — Simulate related-term output on Grafana
- Starts when: The Stage 2 dictionary exists and the separate Grafana clone is indexed with the recorded `codemap-search` binary.
- Work: For each case, run the baseline query (frozen terms joined) and record top-5 paths; render the related-search block under the block limits using target-repo counts; run each suggested query and record top-5 paths, block bytes, suggested-query count, and total extra result rows.
- Deliverable: `results/cases/<id>.json` (terms, block text, baseline hits, per-query hits) and `results/summary.json`.
- Ends when:
  - [ ] All 36 cases have baseline and treatment records.
  - [ ] The file-count approximation is spot-checked on at least 5 words against `rg -l -i -w <word>`, with differences recorded.
- Handoff: Stage 4 receives per-case results and the summary.
- Replan when: Most blocks are empty or contain only generic words — return to Stage 2 filters once, using only the sample review to adjust, then rerun; a 2048-byte block cannot hold at least one suggested query for 2+ terms — report it as a finding instead of raising the limit.
- Worker decision: Block layout within the limits; optionally up to 3 real identifier examples per term when obtainable from existing CLI output.

### Stage 4 — Report go/no-go
- Starts when: `results/summary.json` exists.
- Work: Write `README.ko.md` in plain Korean: method, corpus, sources and licenses, thresholds, per-threshold results with numbers and denominators, failure examples, limits, and the deferred integration notes; add the attribution file.
- Deliverable: `README.ko.md` and the attribution file listing `github/explore` (CC BY 4.0, with change notice) and `abbrcode/abbreviations-in-code` (MIT notice).
- Ends when:
  - [ ] Each threshold has a pass/fail line with the measured value and denominator.
- Handoff: Overall verification receives the PoC directory and report.
- Replan when: None — reporting only; earlier stages own result changes.

## Side Effect Checkpoints
- [ ] Apart from this brief, `git status --short` lists changes only under `apps/codemap-search/experiments/related-terms-poc/`.
- [ ] Nothing under `.codemap/poc/related-terms/` is tracked, and no cloned repository is staged.
- [ ] `git -C <benchmark-managed Grafana checkout> status --short` is unchanged from before the PoC.
- [ ] If an experiment crate was added, `cargo check --manifest-path apps/codemap-search/Cargo.toml` still passes and the main package `Cargo.toml` is unchanged.
- [ ] The dictionary has no entry whose source is CC BY-SA, unlicensed, or license-unverified.

## Acceptance Criteria
- [ ] Effect: with M = medium/complex cases whose baseline top 5 has no gold path, suggested queries put at least one gold path in a top 5 for at least ⌈M/3⌉ of those cases; if M = 0, the report marks the effect as inconclusive.
- [ ] Quality: precision over the 200 seeded sample pairs is at least 70%.
- [ ] Size: every rendered block is at most 2048 bytes UTF-8, with at most 5 suggested queries of 2–3 words.
- [ ] Control: no identifier-shaped term is expanded in any of the 36 cases.
- [ ] Reproducibility: the documented command sequence regenerates the dictionary and all results from `corpus.json`, `cases.json`, and source manifests.
- [ ] `README.ko.md` states go/no-go per threshold, the extra result rows per case as a token-cost proxy, and unverified limits.

## Open Questions
- [non-blocking] The evaluation-target answer "우선 PoC 만" is read as "use the Grafana benchmark questions only; add no other repository such as hicare-rpm-api." — Default: Grafana 36 questions only; Reconfirm before: writing the follow-up server-integration brief.
