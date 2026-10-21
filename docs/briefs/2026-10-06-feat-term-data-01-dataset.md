# [feat] 의미와 출처를 보존하는 유의어 데이터셋 구축

## Work Type
feat

## Current State (As-Is)
- [confirmed] Authoring inspected `main` at `46995808c` on 2026-10-06. The earlier `docs/briefs/2026-10-06-feat-related-terms-poc.md` is an untracked proposal, not evidence that its assumptions or thresholds are implemented or approved — Evidence: `git status --short`, `git log -1`, and that document's `Constraints` and `Acceptance Criteria`.
- [confirmed] The user selected JSONL as canonical data and SQLite as the generated lookup artifact. The user selected local-only PoC access, with future dataset releases managed on GitHub Releases and downloaded into a global directory — Evidence: the 2026-10-06 authoring decision answers. Publishing and downloading are not this child's work.
- [confirmed] Open English WordNet 2025 core JSON is available at `https://github.com/globalwordnet/english-wordnet/releases/download/2025-edition/english-wordnet-2025-json.zip`. Authoring downloaded 9,986,555 bytes and verified SHA-256 `7d749f6e2c39e6970e4997839dcf6e42fd281f3c2fae0171d2192bae8cfa4b51` against release metadata. The ZIP contains 73 JSON files totaling 72,404,635 uncompressed bytes — Evidence: the bounded ZIP inspection and `2025-edition` release asset metadata.
- [confirmed] The same core archive contains 107,519 synsets, of which 44,961 have multiple distinct members. Memberships total 185,129, and flattening each synset into directed member pairs would produce 285,546 pairs before cross-synset deduplication — Evidence: enumeration of objects containing `members` and `definition` in the verified archive. These are raw-resource counts, not code-domain coverage or PoC quality results.
- [confirmed] Bulk synset JSON is keyed by synset ID, whereas the live lemma API places an `id` inside each returned object. Separate `entries-*.json` files are present in the archive — Evidence: the verified archive's first `adj.all.json` object keys, ZIP file listing, and `https://en-word.net/api/lemma/delete`. Inspect the entry/form/sense linkage during import rather than assuming the API and bulk shapes are interchangeable.
- [confirmed] The verified WordNet archive has a digital-information sense connecting `delete` and `erase`, but its `retry` member belongs to a court-rehearing sense. No exact case-folded synset member was found for `deserialize`, `enqueue`, `dequeue`, or `memoize` in that archive — Evidence: the authoring archive scan. This is an exploratory probe, not a representative coverage score.
- [confirmed] WordNet's release license requires attribution under CC BY 4.0 and preservation of the underlying Princeton WordNet notices — Evidence: `https://raw.githubusercontent.com/globalwordnet/english-wordnet/2025-edition/LICENSE.md` and `WNDB_License.txt` at the same tag.
- [confirmed] GitHub Explore's tree `c8d7bc516928a4f17845b767884a80cc53affd57` contains 1,278 `topics/*/index.md` files totaling 853,774 bytes. Not every topic has aliases: `authentication` has none in the inspected front matter, while `authorization` lists `authz`; `pubsub` separates `aliases` from `related` — Evidence: the GitHub tree API and inspected blobs. This tree ID is not a repository commit pin.
- [confirmed] Wikidata Q1349935 revision 2552902218 lists `message dispatcher`, `message loop`, `message pump`, and `run loop` as English aliases of `event loop`; Q1376168 revision 2543005439 has no English aliases for `memoization`. Q1127410 revision 2515651660 preserves a software-serialization concept and aliases including `serialize` and `data serialization` — Evidence: the authoring `wbgetentities` response with `info`, `labels`, `aliases`, and `descriptions`.
- [confirmed] Wikidata structured entity data is CC0, but other wiki text can be CC BY-SA. GitHub Explore is CC BY 4.0, abbrcode is MIT, and the software-engineering abbreviations repository is CC0-1.0 — Evidence: `https://www.wikidata.org/wiki/Wikidata:Licensing`, repository license responses, and original license files inspected during research. Recheck licenses at the actual frozen revisions.
- [confirmed] Abbreviation sources contain ambiguous mappings such as `res` to response/result and `dir` to direction/directory. Their recommendation marks are naming guidance, not probability estimates — Evidence: `https://github.com/abbrcode/abbreviations-in-code` README. The other abbreviation source is `https://github.com/wuyichen24/software-engineering-abbreviations-acronyms`.
- [inferred] Explicit sense/alias sources can seed a useful dataset, but their combined software-domain coverage and precision remain unknown. Co-occurrence alone cannot establish synonymy — Confirm by: this child's source audit and child 03's frozen lookup review, with synonyms, aliases, abbreviations, and unverified associations measured separately.

## Desired Outcome (To-Be)
- A reproducible local builder emits canonical, sense-preserving JSONL and a generated SQLite dataset without lookup-time model calls, embeddings, or upstream API access.
- Every usable relation retains source-record identity, semantic scope, relation kind, and license evidence. Unsupported associations remain distinguishable from source-supported synonym relations.
- An addressable dataset handoff supplies child 02 with exact local artifact paths, hashes, schema and normalization versions, population counts, and reconstruction commands.

## Scope
### In Scope
- Implement source acquisition, frozen manifests, normalization, source-specific importers, a SQLite schema/build step, attribution, and build documentation under `apps/codemap-search/experiments/related-terms-poc/`.
- Audit the five source families: Open English WordNet core, GitHub Explore, Wikidata structured entities, abbrcode, and software-engineering abbreviations. Freeze selected records and explain exclusions instead of assuming each source is equally useful.
- Preserve lexical forms, senses/concepts, typed relationships, and many-to-many evidence. Keep generic-language meanings distinguishable from code-domain evidence.
- Inspect a bounded, pinned public-code corpus for vocabulary and usage evidence when needed. Record language/domain coverage and repository-family deduplication. Retain mined co-occurrences only as unverified associations, not automatically accepted synonyms.
- Generate `contracts/dataset.schema.json`, `contracts/dataset.md`, `schema.sql`, `sources.lock.json`, `build_dataset.py`, and an initial `README.ko.md` within the experiment. These are proposed internal artifact names, not product API names.
### Out of Scope
- [hard] Do not edit product `apps/codemap-search/src/`, its Cargo files, MCP tool names/schemas/descriptions, or configuration keys.
- [hard] Do not change `benchmark/`, write into its managed Grafana checkout, run `pnpm bench:start`, or run paid/model-based evaluation.
- [hard] Do not ingest CC BY-SA, unlicensed, or license-unverified data. Do not infer a data license from a separately licensed research tool. Keep ConceptNet, Stack Overflow synonyms, SCANL, SEthesaurus data, SWordNet data, and Moby outside this implementation.
- [hard] Do not commit cloned repositories, upstream raw archives, or large generated datasets. Keep raw/intermediate/build data under ignored `.codemap/poc/related-terms/`.
- [deferred] LLM generation or judgment, embeddings, shared-neighbor synonym mining, Korean-English mapping, and repository-specific learned dictionaries.
- [deferred] GitHub Release publication, a latest-release downloader, global installation, automatic updates, and product lookup integration.

## Constraints
- JSONL is the canonical normalized representation. SQLite is rebuilt from it, never manually maintained as a second source of truth. Preserve source archives separately from normalized records.
- Distinguish expression identity from sense/concept identity. Namespace source IDs by source snapshot, preserve original spelling, and keep ambiguous meanings separate. Do not create equivalence by transitive closure through a polysemous expression.
- Define schema version, dataset content identity, normalization version, source snapshots, build-tool versions, and attribution references before generating the handoff. Dataset identity must not depend on absolute local paths or fetch timestamps.
- Make canonical JSONL logically deterministic through stable ordering and stable identifiers. Record SQLite file SHA-256 for artifact integrity separately; do not promise byte-identical databases across SQLite versions/platforms without demonstrating it.
- Preserve multi-word expressions and punctuation-bearing technical terms. Declare normalization rules explicitly and make the serving reader use the same version. Do not confuse identifier splitting, phrase tokenization, spelling variants, or morphology with synonym evidence.
- Use same-sense membership for WordNet synonym evidence, source-declared concept aliases for alias evidence, and explicit expansion mappings for abbreviation evidence. Preserve `related`, antonym, and other links separately; do not relabel them as synonyms.
- Preserve relation direction where the source requires it. Multiple sources supporting one relation must not inflate independent repository support. Source confidence and cross-repository counts are different quantities.
- Keep corpus support absent when it was not measured. Never fabricate support to make curated relations satisfy a corpus threshold. The old support 10, 6-language/5-domain minimum, top-20 cap, 70% target, and 10 MB commit rule are proposal values, not locked acceptance thresholds for this set.
- Define and record a bounded source-selection policy before child 03 evaluates the dataset. Do not discard every untagged WordNet sense merely because only some records have a domain tag, and do not treat presence in code as proof of the intended meaning.
- Keep an auditable source archive/hash route for every frozen upstream revision, including Wikidata entity revisions. A mutable URL plus fetch date is insufficient. Do not use live APIs to fill missing candidates during lookup.
- Keep normalized JSONL and SQLite artifacts under `.codemap/poc/related-terms/datasets/<dataset-content-id>/`. Track the builder, schema, lock manifests, attribution, compact statistics, and handoff, not the full raw/generated payloads.
- Authoring did not establish representative semantic-quality or latency thresholds. Completion here means a valid reproducible dataset and honest evidence of gaps, not a claim that a particular coverage target already passed.

## Related Files / Entry Points
- `apps/codemap-search/experiments/related-terms-poc/` (proposed) — own source adapters, canonical data contract, SQLite builder, and dataset evidence.
- `apps/codemap-search/experiments/event-navigation-poc/corpus.json` — inspect pinned-corpus metadata conventions, not its evaluation labels or acceptance claims.
- `apps/codemap-search/experiments/event-navigation-poc/README.ko.md` — follow explicit measured-versus-unverified reporting and Korean reproduction guidance.
- `apps/codemap-search/src/parser/tokenize.rs` — inspect `split_identifier` and `QueryTokens::parse` before defining normalization compatibility.
- `apps/codemap-search/Cargo.toml` — verify existing `serde_json`, bundled `rusqlite`, and experiment packaging exclusion without editing.
- `.gitignore` — verify `.codemap/` exclusion before storing acquired or generated data.
- `docs/briefs/2026-10-06-feat-related-terms-poc.md` — use as historical input only; the parent and current user decisions supersede its assumptions.

## Execution Plan
### Stage 1 — 원본 고정과 데이터 계약 수립
- Starts when: The current checkout and the approved JSONL/SQLite, local-only, phase-1 boundaries are available.
- Work: Create the experiment's source manifests and data contract. Fetch the verified WordNet asset, freeze revisions for the other selected sources, inspect actual raw structures and licenses, and record a bounded source/corpus selection policy.
- No-op when: A local dataset already satisfies this brief's complete canonical/SQLite round trip, attribution, normalization, and provenance requirements, with hashes rechecked against its lock manifests.
- No-op handoff: Child 02 receives `apps/codemap-search/experiments/related-terms-poc/results/dataset-handoff.json` with the same required schema and verified existing-artifact evidence. Failed proof stops the no-op route and resumes the owning stages before any successor starts.
- Deliverable: `apps/codemap-search/experiments/related-terms-poc/contracts/dataset.md`, `contracts/dataset.schema.json`, and `sources.lock.json`, including record shapes, identity rules, source scopes, and license references.
- Verify: `Inspect each selected source snapshot and the dataset contract against its original record shape`; Inputs: the verified WordNet ZIP, frozen topic/entity/abbreviation records, source licenses, and the three contract artifacts; Expected: non-empty selected records with stable IDs, complete snapshot/hash/license fields, and separate semantic relation kinds.
- Ends when:
  - [ ] Every selected source has reproducible retrieval and explicit attribution or an exclusion reason.
  - [ ] The bulk WordNet map-key IDs and entry/synset relationship are preserved rather than inferred from the live API shape.
  - [ ] Source selection, normalization, ambiguous-sense handling, and SQLite/JSONL identity are documented before evaluation.
- Handoff: Stage 2 consumes the frozen manifests and canonical record contract.
- Replan when: A required source cannot be frozen or its data license cannot be verified. Exclude it with evidence if the remaining sources preserve the goal, otherwise return to the parent rather than silently substituting a forbidden source.
- Worker decision: Importer organization and bounded corpus selection, with declared acquisition/storage budgets and unchanged semantic/source-license constraints.

### Stage 2 — 정규화 JSONL과 SQLite 생성
- Starts when: Stage 1's source snapshots and canonical contract are fixed.
- Work: Implement the builder, emit normalized JSONL, generate indexed SQLite lookup structures, and collect relation/source/ambiguity counts. Preserve semantic evidence and attach code-use evidence separately where measured.
- Deliverable: Local canonical JSONL, SQLite, attribution, and `apps/codemap-search/experiments/related-terms-poc/results/dataset-stats.json`, with precise artifact paths, content identity, populations, exclusions, and build environment.
- Verify: `Compare canonical records with generated SQLite rows and their referenced source records`; Inputs: all generated logical records, SQLite tables/indexes, sources.lock.json, and attribution; Expected: matching non-zero populations, zero dangling identifiers, recoverable evidence for every serving relation, and query-key indexes present.
- Ends when:
  - [ ] Regeneration from the same frozen inputs yields the same canonical record content and logical SQLite content.
  - [ ] All serving relations resolve to a source record and semantic scope; unverified co-occurrences are not counted as confirmed synonyms.
  - [ ] Multi-word expressions, ambiguous abbreviations, and records without alternate expressions are accounted for explicitly.
- Handoff: Stage 3 receives built artifacts and their integrity/coverage statistics.
- Replan when: Data cannot represent meaning and provenance without flattening distinct senses, or only unsupported associations remain. Repair the contract in this child and rebuild before releasing a handoff.

### Stage 3 — 로컬 배포본과 인계 자료 확정
- Starts when: Stage 2 has a reproducible non-empty dataset and valid attribution.
- Work: Document exact build/rebuild commands and emit a machine-readable handoff. Close SQLite writers and deliver a self-contained database without a required WAL/journal companion. Do not publish a release.
- Deliverable: `apps/codemap-search/experiments/related-terms-poc/results/dataset-handoff.json`, containing `dataset_id`, `schema_version`, `normalization_version`, `jsonl_paths`, `sqlite_path`, `artifact_hashes`, `source_lock_path`, `attribution_path`, `counts`, `build_commands`, `environment`, `integrity_status`, and `limitations`.
- Verify: `Inspect the dataset handoff and reopen its SQLite artifact read-only`; Inputs: dataset-handoff.json and every referenced local artifact; Expected: every path/hash resolves, the database opens without creating files, relation/source counts match, and no lookup-time network dependency is required.
- Ends when:
  - [ ] The handoff records actual commands and outcomes, not proposed invocations or unmeasured quality claims.
  - [ ] The Korean README reproduces the dataset and explains source gaps, including the observed retry sense and missing technical members.
- Handoff: Child 02 consumes `apps/codemap-search/experiments/related-terms-poc/results/dataset-handoff.json` after integrity verification; child 03 later uses the same identity through the lookup handoff.
- Replan when: Handoff verification fails. Stop child 02, return to the failed build/source stage, correct and re-verify, and update parent handoffs before resuming.

## Side Effect Checkpoints
- [ ] `git status --short` shows no implementation changes outside the experiment and preserves the pre-existing untracked brief. Product source, Cargo files, benchmark data, and user/global configuration are unchanged.
- [ ] `git ls-files .codemap/poc/related-terms` lists no tracked artifacts, and acquisition did not stage cloned repositories or raw data.
- [ ] All actual importer data sources are in the audited source lock and attribution; no documentation with a different license was silently imported with entity data.
- [ ] The packaged main crate remains independent of the experiment; no dataset release or global installation occurred.

## Acceptance Criteria
- [ ] The dataset handoff resolves to a non-empty canonical JSONL/SQLite pair whose logical contents match and regenerate from frozen sources with traceable attribution.
- [ ] Bounded inspection of all relation kinds and evidence references shows no co-occurrence, mere spelling collision, or transitive polysemy being represented as source-proven synonymy.
- [ ] Actual raw-source counts and resulting serving counts are reported separately, with exclusions and unverified code-domain coverage stated explicitly.
- [ ] Child 02 can recover the dataset format, local paths, versions, and rebuild route solely from this brief and the handoff without choosing a new storage or delivery policy.

## Open Questions
- None — storage and phase-1 delivery are user-decided; source-adapter and normalization details are bounded implementation work. Future release ownership and update policy are deferred to the integration phase.
