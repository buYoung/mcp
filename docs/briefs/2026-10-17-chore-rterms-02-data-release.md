# [chore] Release rts5 as tar.zst and drop license notices

## Work Type
chore

## Current State (As-Is)
- [confirmed] As of `e473b5f48` on `feat/related-terms-lookup` (inspected 2026-10-17), the default sense dataset is tracked in git at `apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3`, 31,715,328 bytes, SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced` — Evidence: `docs/related-terms/sense-mapping-log/16-compact-storage.md` section 3, row "크기".
- [confirmed] On 2026-10-17 no GitHub release carries the dataset: the repository has 16 releases, all tagged `codemap-v*`, and `codemap-v0.11.0` is marked Latest — Evidence: `gh release list -R buYoung/mcp --limit 50`. The binary workflow only runs for tags matching `codemap-v[0-9]+.[0-9]+.[0-9]+` (`.github/workflows/codemap-search-release.yml`), so a data tag does not start it.
- [confirmed] `install.sh` without a pinned version downloads from the repository-wide `releases/latest/download/<asset>` redirect, so a data release marked "latest" would break default binary installs — Evidence: `apps/codemap-search/install.sh` function `resolve_urls()` and its comment "a repo-global redirect".
- [confirmed] Binary release assets ship with a `<asset>.sha256` file produced by `shasum -a 256` — Evidence: `.github/workflows/codemap-search-release.yml` step that runs `tar -czf "${name}.tar.gz"` then `shasum -a 256`.
- [confirmed] Measured sizes of the 31.7MB file: tar.gz 13.24MB (same as plain gzip), zstd `-19` 10.57MB, zstd `--ultra -22` 10.41MB, xz `-9` 9.55MB; the user chose `tar.zst` — Evidence: `docs/related-terms/codemap-integration.md` section 2, table under "올리는 것".
- [confirmed] The sense dataset derives from the stage-1 LLM generation, not from the external dictionaries that only `rtr1` uses — Evidence: `docs/related-terms/pipeline.md` diagram under "세 데이터셋은 모두 아래 1단계의 LLM 생성 자료에서 출발한다" and section 4 steps 1–19.
- [confirmed] The rts5 tables are `sense_meta`, `terms`, `fields`, `topics`, `topic_fields`, `concepts`, `personas`, `input_senses`, `input_relations`, `relation_kinds`, `relations`, with no gloss or definition column — Evidence: `sqlite_master` of the rts5 file.
- [confirmed] The PoC folder has `ATTRIBUTION.md`, a CC BY 4.0 / WordNet License / CC0 / MIT attribution file for the local-only `rtr1` dataset — Evidence: `apps/codemap-search/experiments/related-terms-poc/ATTRIBUTION.md` headings "Open English WordNet 2025" and "GitHub Explore".
- [confirmed] `docs/related-terms/sources.md` carries license notices: the bullet "**라이선스와 저작자 표시:**" pointing to `ATTRIBUTION.md`, a "라이선스" column in the external-source table, the paragraph "Open English WordNet은 복사본에 `LICENSE.md`와 `WNDB_License.txt`를 함께 두도록 요구한다", and the line about `NOASSERTION` — Evidence: those lines in `docs/related-terms/sources.md`.
- [confirmed] The `license_spdx` column in `build_related_dataset.py` and `apps/codemap-search/experiments/related-terms-poc/lookup-v3/src/` is part of the local `rtr1` schema, not a notice; changing it would stop `rtpoc3` from reading the existing `rtr1` — Evidence: `CREATE TABLE sources (source TEXT PRIMARY KEY, license_spdx TEXT NOT NULL, ...)` in `build_related_dataset.py`; `license_spdx` reads in `lookup-v3/src/dataset.rs`.
- [confirmed] The user decided to remove all CC BY-style license notices without debate, and chose deletion for `ATTRIBUTION.md` and the license notes in `sources.md` — Evidence: `docs/related-terms/codemap-integration.md` table "결정한 것", row "라이선스".
- [confirmed] The repository slug is `buYoung/mcp` — Evidence: `repository = "https://github.com/buYoung/mcp"` in `apps/codemap-search/Cargo.toml`.
- [inferred] `sense_meta` values such as `graph` hold local paths (`.codemap/poc/...`) and script names but no third-party text — Confirm by: dumping every `sense_meta` row in Stage 1.

## Desired Outcome (To-Be)
- A GitHub release tagged `related-terms-data-rts5-75e02862daa29308` carries `rts5-75e02862daa29308.tar.zst` and `rts5-75e02862daa29308.tar.zst.sha256`.
- The archive holds exactly one regular file, `dataset.sqlite3`, whose size and SHA-256 equal the tracked rts5 file.
- The release is not marked latest; `releases/latest` still resolves to a `codemap-v*` binary release.
- The release carries no third-party data, attribution, or CC BY notice.
- The repository carries no related-terms license notice: `ATTRIBUTION.md` is deleted, and `sources.md` keeps each external source's address and pinned version but no license column or license paragraphs.
- A packaging route that reproduces the same archive bytes exists in the repository, and an evidence log records the URL, sizes, hashes, provenance check, and removed notices.

## Scope
### In Scope
- Provenance check of every rts5 table and `sense_meta` row against the pipeline lineage.
- Deleting `apps/codemap-search/experiments/related-terms-poc/ATTRIBUTION.md`.
- Removing from `docs/related-terms/sources.md` the "라이선스와 저작자 표시" bullet, the "라이선스" table column, the Open English WordNet license-file paragraph, and the `NOASSERTION` line; fixing any link to `ATTRIBUTION.md` elsewhere in `docs/related-terms/`.
- Packaging route (script or documented commands) that builds the archive with fixed metadata (single entry, fixed mtime, uid/gid 0, mode 0644) and writes the `.sha256` file.
- Release notes text: dataset id, schema `rt-sense/5`, uncompressed size and SHA-256, archive size, generation summary.
- Publishing the release with `--latest=false`, after explicit user approval.
- Post-publish download check of the public URL.
- Evidence log `docs/related-terms/integration-log/02-data-release.md`.
### Out of Scope
- [hard] Do not publish, push a tag, or edit an existing release without the user's explicit approval at that moment.
- [hard] Do not mark the data release as latest or as a binary release; do not touch `codemap-v*` releases.
- [hard] Do not change `.github/workflows/codemap-search-release.yml` or `apps/codemap-search/install.sh`.
- [hard] Do not modify the tracked rts5 file or add new tables or indexes to the shipped copy.
- [hard] Do not add attribution, CC BY notices, or a new license statement to the release, its notes, or the repository.
- [hard] Do not ship `rtr1`, vectors, `build-report.json`, or any file other than `dataset.sqlite3` inside the archive.
- [hard] Do not change the `license_spdx` schema column or its readers in `build_related_dataset.py` and `lookup-v3/src/`; it is the local `rtr1` format.
- [hard] Do not remove the external sources' addresses, pinned versions, or imported-field descriptions from `sources.md`; only license notices go.
- [hard] Do not touch the codemap-search crate's own `LICENSE` file or `license` field in `Cargo.toml`.
- [deferred] Automating data releases in CI.

## Constraints
- Release identity (shared with the fetch child; do not change): tag `related-terms-data-rts5-75e02862daa29308`, asset `rts5-75e02862daa29308.tar.zst`, URL `https://github.com/buYoung/mcp/releases/download/related-terms-data-rts5-75e02862daa29308/rts5-75e02862daa29308.tar.zst`.
- The archive root contains only `dataset.sqlite3`; the fetch child rejects every other entry name.
- Record the zstd level used; either `-19` or `--ultra -22` is acceptable.
- If the provenance check finds any column or row taken from Open English WordNet, GitHub Explore, Wikidata, abbrcode, or software-engineering abbreviation sources, stop before packaging.
- The tag target is the commit that contains `datasets/rts5-75e02862daa29308/`; confirm it with the user together with the publish approval.
- "License activation" in sense-mapping logs is a sense name, not a notice; leave it.
- `docs/related-terms` documents stay Korean in "~다" style; the evidence log follows the same style.

## Related Files / Entry Points
- `apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3` — the file to package; read only.
- `docs/related-terms/pipeline.md` — lineage for the provenance check (diagram and section 4).
- `apps/codemap-search/experiments/related-terms-poc/ATTRIBUTION.md` — file to delete; also the list of sources that must not appear in the release.
- `docs/related-terms/sources.md` — remove the license bullet, column, and paragraphs.
- `apps/codemap-search/install.sh` — `resolve_urls()` explains why the release must not become latest.
- `.github/workflows/codemap-search-release.yml` — asset and `.sha256` naming convention to mirror.
- `apps/codemap-search/experiments/related-terms-poc/package_release_dataset.py` (proposed) — packaging route, if a script is chosen.
- `docs/related-terms/codemap-integration.md` — design section 2 "데이터 배포".
- `docs/related-terms/integration-log/02-data-release.md` (proposed) — evidence log.

## Execution Plan
### Stage 1 — Check provenance
- Starts when: The tracked rts5 file matches SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`.
- Work: Record the starting commit (`git rev-parse HEAD`) and check `gh release view related-terms-data-rts5-75e02862daa29308 -R buYoung/mcp`; then list every table, column, and `sense_meta` row, and map each to its pipeline step; confirm none carries third-party dictionary text or rows.
- No-op when: The tag `related-terms-data-rts5-75e02862daa29308` already exists, its `rts5-75e02862daa29308.tar.zst` asset extracts to a single `dataset.sqlite3` with the pinned SHA-256, and `releases/latest` still points to a `codemap-v*` tag.
- No-op handoff: Skip Stages 3–4 (archive and publish); Stage 2 still removes license notices, and Stage 5 records the existing release and its checks at `docs/related-terms/integration-log/02-data-release.md` (proposed) for global acceptance.
- Deliverable: Provenance table (table → producing step → source type) for the evidence log.
- Verify: `sqlite3 apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3 "select key from sense_meta"`; Inputs: the tracked rts5 file; Expected: exit 0 and keys including `schema_version` and `dataset_id`.
- Ends when:
  - [ ] Every table and `sense_meta` key is mapped to an LLM-generated or derived source.
- Handoff: Stage 2 receives the provenance table and the go-ahead.
- Replan when: A table needed by the lookup carries external-source content, or the tag exists with a different asset; stop and report it to the user, because removing a table changes lookup behavior and replacing a published asset breaks pinned binaries.

### Stage 2 — Remove license notices
- Starts when: Stage 1 has mapped every table to a non-external source.
- Work: Delete `ATTRIBUTION.md`, remove the license bullet, the "라이선스" column, the Open English WordNet license-file paragraph, and the `NOASSERTION` line from `sources.md`, and fix any remaining link to `ATTRIBUTION.md` under `docs/related-terms/`.
- Deliverable: `ATTRIBUTION.md` removed and `docs/related-terms/sources.md` without license notices.
- Verify: `rg -n "CC BY|CC0|WordNet License|라이선스|ATTRIBUTION|NOASSERTION" docs/related-terms/sources.md`; Inputs: the edited `sources.md`; Expected: no matches, while `rg -c "github.com|wikidata.org" docs/related-terms/sources.md` still counts the external-source rows.
- Ends when:
  - [ ] `test ! -e apps/codemap-search/experiments/related-terms-poc/ATTRIBUTION.md` exits 0.
  - [ ] `rg -n "ATTRIBUTION" docs/related-terms apps/codemap-search/experiments/related-terms-poc --glob '*.md'` returns no matches.
- Handoff: Stage 3 receives the cleaned documents; Stage 5 records the removed notices.
- Replan when: Another document or script under the PoC folder reads `ATTRIBUTION.md`; record it and remove only the reference, not the code path that uses `license_spdx`.

### Stage 3 — Build the archive
- Starts when: Stage 2 has removed the notices.
- Work: Build `rts5-75e02862daa29308.tar.zst` with a single `dataset.sqlite3` entry and fixed metadata, write the `.sha256` file, and keep the packaging route in the repository.
- Deliverable: Archive and checksum outside the repository, plus the packaging route.
- Verify: `zstd -dc rts5-75e02862daa29308.tar.zst | tar -tvf -`; Inputs: the built archive; Expected: exactly one line, a regular file named `dataset.sqlite3` with size 31715328.
- Ends when:
  - [ ] Extracting the archive into an empty scratch folder yields `dataset.sqlite3` with SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`.
  - [ ] Running the packaging route twice yields identical archive SHA-256 values.
- Handoff: Stage 4 receives the archive, its SHA-256, its size, and the draft release notes.
- Replan when: The archive cannot be made byte-reproducible on this machine; record the cause and continue only with the user's approval, since the `.sha256` file then describes one specific build.

### Stage 4 — Publish with approval
- Starts when: Stage 3's archive passes its checks and the user has approved the exact command, tag target, title, and notes.
- Work: Create the release with `gh release create related-terms-data-rts5-75e02862daa29308 --latest=false` and the two assets.
- Deliverable: Public release and its URL.
- Verify: `gh api repos/buYoung/mcp/releases/latest --jq .tag_name`; Inputs: the repository after publishing; Expected: a tag that starts with `codemap-v`.
- Ends when:
  - [ ] `curl -fsSL` on the asset URL into an empty scratch folder, followed by extraction, yields the pinned SHA-256.
  - [ ] The downloaded `.sha256` file matches the archive's SHA-256.
- Handoff: Stage 5 receives the release URL, archive SHA-256, and download check result.
- Replan when: The user declines publishing or changes the tag or asset name; stop and return to the parent, because the fetch child pins those names.

### Stage 5 — Record the release
- Starts when: Stage 4's download check passes, or Stage 1's no-op condition holds and Stage 2 is done.
- Work: Write the evidence log with the provenance table, removed license notices, packaging route, zstd level, archive size and SHA-256, uncompressed size and SHA-256, release URL, and the latest-release check.
- Deliverable: Evidence log `docs/related-terms/integration-log/02-data-release.md` (proposed).
- Verify: `bounded inspection of docs/related-terms/integration-log/02-data-release.md`; Inputs: the log file; Expected: it contains the release URL, both SHA-256 values, the `releases/latest` tag observed after publishing, and the list of removed notices.
- Ends when:
  - [ ] The log's uncompressed SHA-256 equals the value pinned in the parent Shared Constraints.
- Handoff: Global acceptance in `docs/briefs/2026-10-17-briefset-rterms.md` receives `docs/related-terms/integration-log/02-data-release.md` and the public URL.
- Replan when: None — the log only records facts already verified in Stages 1–4.

## Side Effect Checkpoints
- [ ] `gh api repos/buYoung/mcp/releases/latest --jq .tag_name` still starts with `codemap-v` after publishing.
- [ ] `shasum -a 256 apps/codemap-search/experiments/related-terms-poc/datasets/rts5-75e02862daa29308/dataset.sqlite3` still prints the pinned SHA-256.
- [ ] `git diff --stat <starting commit> -- apps/codemap-search/experiments/related-terms-poc/datasets/ .github/workflows/codemap-search-release.yml apps/codemap-search/install.sh apps/codemap-search/experiments/related-terms-poc/build_related_dataset.py apps/codemap-search/experiments/related-terms-poc/lookup-v3/` is empty, using the commit recorded in Stage 1, so staged or committed changes are caught too.
- [ ] The release notes contain no "CC BY", "WordNet", "GitHub Explore", or "Wikidata" text (search the notes file before publishing).
- [ ] `python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py token` still exits 0 after the document changes.

## Acceptance Criteria
- [ ] The public asset URL downloads an archive whose single entry `dataset.sqlite3` has 31,715,328 bytes and SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`.
- [ ] The release is not latest, and default `install.sh` still resolves a binary release.
- [ ] `ATTRIBUTION.md` no longer exists, and `docs/related-terms/sources.md` has no license notice while still listing every external source's address and pinned version.
- [ ] The packaging route in the repository rebuilds the same archive SHA-256.
- [ ] The evidence log records provenance, removed notices, sizes, hashes, zstd level, and URL.

## Open Questions
- None — the user chose to delete the license notices and to keep dataset updates pinned to codemap-search releases; publishing still waits for explicit approval in Stage 4.
