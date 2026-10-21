# [feat] Fetch the pinned dataset and add related_terms config keys

## Work Type
feat

## Current State (As-Is)
- [confirmed] As of `e473b5f48` on `feat/related-terms-lookup` (inspected 2026-10-17), codemap-search has no config key, download path, or global folder for related terms — Evidence: no `related_terms` match in `apps/codemap-search/src/`.
- [confirmed] The global folder is `$CODEMAP_HOME`, else `~/.codemap` — Evidence: `HOME_ENV` and `global_dir()` in `apps/codemap-search/src/config.rs`.
- [confirmed] The config schema is at version 29 and adding a key follows the schema-sync route: `CONFIG_VERSION`, migration, parser normalization, generated template comments (en/ko through `src/config_locale.rs`), `src/config/layout.rs` settings, `docs/configuration.md`, `docs/configuration.ko.md`, README — Evidence: `const CONFIG_VERSION: u32 = 29;` in `src/config.rs`; `apps/codemap-search/AGENTS.md` route "Config schema-sync route".
- [confirmed] Config loading never exits: unknown keys and type mismatches warn on stderr and fall back per key — Evidence: `apps/codemap-search/AGENTS.md` section "Never-exit config loading".
- [confirmed] An HTTPS client already exists: `reqwest` 0.13.5 with `rustls-no-provider`, and Jev installs the ring provider when building its client — Evidence: `reqwest = { version = "0.13.5", default-features = false, features = ["rustls-no-provider"] }` in `Cargo.toml`; `rustls::crypto::ring::default_provider().install_default()` in `build` of `apps/codemap-search/src/jev/transport.rs`.
- [confirmed] A cross-process file-lock pattern exists: open with `OpenOptions`, mode 0600 on Unix, then retry `File::try_lock()` — Evidence: `lock_registry()` in `apps/codemap-search/src/mcp/global_instructions/registry.rs`.
- [confirmed] `zstd` 0.13.3 is already compiled into the binary through `tantivy-sstable`, `sha2` 0.11.0 is a direct dependency, and no `tar` crate is present; `flate2` is only a build dependency — Evidence: `cargo tree -i zstd -e normal`; `Cargo.lock`; `[build-dependencies]` in `Cargo.toml`.
- [confirmed] README promises a single binary with no external runtime — Evidence: `apps/codemap-search/README.md` sentence "no system `rg`, language server, external runtime, account or API key is required".
- [confirmed] e2e tests run through `tests/e2e_tests.rs`, register modules in `tests/e2e/mod.rs`, and use helpers such as `create_mock_repo()` and `with_in_process_server()` in `tests/e2e/helpers.rs` — Evidence: those files.
- [inferred] Jev's client is built for its own API settings and may need a separate builder for a plain GET with timeouts — Confirm by: reading `build` and `HttpsSettings` in `src/jev/transport.rs` in Stage 1.

## Desired Outcome (To-Be)
- `[related_terms]` config has `enabled` (default `false`) and `dataset_path` (optional), documented in English and Korean, with an additive migration from version 29.
- A fetch API resolves the dataset source: a user-supplied `dataset_path`, or the pinned release asset under `<global_dir>/related-terms/rts5-75e02862daa29308/dataset.sqlite3`.
- The fetch downloads the `tar.zst` asset, extracts only `dataset.sqlite3`, checks size and SHA-256, and renames into place atomically; a file at the final path always means "verified".
- Because the folder name is the pinned dataset id, a codemap-search upgrade that keeps the same pinned spec finds the existing final file and makes no network request; only a binary that pins a new id downloads, into a new folder.
- Concurrent processes never corrupt the file: one downloads, others wait or report "in progress".
- Interrupted, corrupted, oversized, or malformed downloads leave no final file and report a reason; the next attempt can succeed.
- The fetch API itself never runs unless called; this child adds no start-up call.
- README states that the default setup uses no network and that enabling related terms downloads the dataset once.

## Scope
### In Scope
- Config keys `related_terms.enabled` and `related_terms.dataset_path` across every schema-sync location, `CONFIG_VERSION` 29 → 30, and the migration.
- Pinned dataset spec constant: id `rts5-75e02862daa29308`, URL, uncompressed size 31,715,328, SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`.
- Fetch module under `apps/codemap-search/src/related_terms/`: source resolution, lock, download to a temporary name, streamed zstd + tar extraction, entry checks, size bound, hash check, atomic rename, stale temporary cleanup, status values (ready with path and user-owned flag, in progress, failed with reason).
- Dependencies: add `tar`; make `zstd` a direct dependency at the version already in `Cargo.lock`.
- e2e tests for download integrity in `tests/e2e/`.
- README (en/ko) network wording, `docs/configuration.md` and `docs/configuration.ko.md` rows.
- Evidence log `docs/related-terms/integration-log/03-fetch-config.md`.
### Out of Scope
- [hard] Do not call the fetch API from `main.rs`, `src/mcp/`, or any start-up path; the MCP tool child wires it.
- [hard] Do not register an MCP tool or change `tools/list`.
- [hard] Do not build SQLite indexes or open the dataset with SQL here; preparation belongs to the lookup child.
- [hard] Do not add a public config key for the download URL, hash, or proxy.
- [hard] Do not add `xz2`, `liblzma`, or another compression library; do not move `flate2` out of `[build-dependencies]`.
- [hard] Do not rehash a file that already sits at the final path; preparation later adds indexes and changes its bytes.
- [deferred] Proxy and custom CA settings beyond what `reqwest` reads by default.
- [hard] Do not add a data update channel separate from codemap-search releases (manifest, version check, or remote lookup); the user chose pinned dataset updates.
- [deferred] Removing local folders of datasets that a newer binary no longer pins.

## Constraints
- Release identity (shared with the data-release child; do not change): tag `related-terms-data-rts5-75e02862daa29308`, asset `rts5-75e02862daa29308.tar.zst`, URL `https://github.com/buYoung/mcp/releases/download/related-terms-data-rts5-75e02862daa29308/rts5-75e02862daa29308.tar.zst`.
- Accept only a regular-file entry named `dataset.sqlite3`; reject absolute paths, `..`, links, and any other entry.
- Stop extraction as soon as the extracted byte count exceeds 31,715,328, and cap downloaded bytes with a fixed bound recorded in the log.
- The fetch API takes the dataset spec as a parameter so tests can serve a fixture archive with its own size and hash; the pinned spec is the only production value.
- Keep the temporary file in the same folder as the final file so the rename is atomic.
- Use the `lock_registry()` locking pattern for `<global_dir>/related-terms/<dataset_id>/download.lock`.
- When `dataset_path` is set, never write to that file or its folder; return it with the user-owned flag set.
- Config warnings follow the never-exit rule: a bad `dataset_path` type warns and falls back to unset.
- Explain each added dependency in the evidence log (user rule).
- New e2e tests cover only download integrity; no other new test files.
- Run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after Rust changes.
- The evidence log is Korean in "~다" style, like other `docs/related-terms` documents.

## Related Files / Entry Points
- `apps/codemap-search/src/config.rs` — start at `CONFIG_VERSION`, the config struct fields, merge, validation, and migration for the new section.
- `apps/codemap-search/src/config/layout.rs` — settings table for the generated template.
- `apps/codemap-search/src/config_locale.rs` — en/ko template comments.
- `apps/codemap-search/docs/configuration.md` — English config reference.
- `apps/codemap-search/docs/configuration.ko.md` — Korean config reference.
- `apps/codemap-search/README.md` — network wording next to "no ... external runtime".
- `apps/codemap-search/README.ko.md` — Korean README wording.
- `apps/codemap-search/src/jev/transport.rs` — client construction and rustls provider set-up to reuse.
- `apps/codemap-search/src/mcp/global_instructions/registry.rs` — `lock_registry()` pattern.
- `apps/codemap-search/Cargo.toml` — add `tar`, promote `zstd`.
- `apps/codemap-search/src/related_terms/fetch.rs` (proposed) — fetch module.
- `apps/codemap-search/tests/e2e/mod.rs` — register the new test module.
- `apps/codemap-search/tests/e2e/related_terms_fetch.rs` (proposed) — download integrity tests.
- `docs/related-terms/codemap-integration.md` — design section 2 "데이터 배포".
- `docs/related-terms/integration-log/03-fetch-config.md` (proposed) — evidence log handed to the MCP tool child.

## Execution Plan
### Stage 1 — Add the config keys
- Starts when: `CONFIG_VERSION` is 29 in `apps/codemap-search/src/config.rs` and no `related_terms` key exists.
- Work: Add `[related_terms]` with `enabled = false` and optional `dataset_path` through every schema-sync location, bump to version 30 with an additive migration, and update the README network wording.
- No-op when: `related_terms.enabled` and `related_terms.dataset_path` already exist with these defaults in `src/config.rs`, both config documents, and the generated template.
- No-op handoff: Stage 2 receives the existing key locations recorded in `docs/related-terms/integration-log/03-fetch-config.md` (proposed).
- Deliverable: Config keys readable from the loaded config, documented in both languages.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests config`; Inputs: existing config e2e tests; Expected: exit 0.
- Ends when:
  - [ ] A repository config at version 29 syncs to version 30 with the two new keys added and every existing key and comment kept.
  - [ ] `docs/configuration.md` and `docs/configuration.ko.md` both list `related_terms.enabled` with default `false` and `related_terms.dataset_path`.
- Handoff: Stage 2 receives the config accessors for both keys.
- Replan when: The config layout cannot hold a new top-level `[related_terms]` section without breaking the documented group order; return to the parent with the conflicting rule.

### Stage 2 — Implement the fetch
- Starts when: Stage 1's config accessors exist.
- Work: Implement source resolution, locking, download, streamed extraction with entry and size checks, hash check, atomic rename, stale temporary cleanup, and status values, using a dataset spec parameter and the pinned spec constant.
- Deliverable: Module `apps/codemap-search/src/related_terms/fetch.rs` (proposed) and the dependency changes.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the workspace with the fetch module; Expected: exit 0.
- Ends when:
  - [ ] With `dataset_path` set, the API returns that path with the user-owned flag and makes no network request.
  - [ ] With an existing final file, the API returns it without network access and without hashing it.
- Handoff: Stage 3 receives the fetch API and its status type.
- Replan when: Jev's client cannot be reused without changing Jev behavior; build a separate client in the fetch module and record why.
- Worker decision: Blocking versus async client inside a background thread, retry and timeout values, temporary file naming, wait-or-report behavior when another process holds the lock.

### Stage 3 — Prove download integrity
- Starts when: Stage 2's fetch API compiles.
- Work: Add e2e tests that serve fixture archives from a local HTTP listener and cover: a connection cut mid-body, two concurrent fetches, a body with the wrong hash, an archive with an extra or `..` entry, an archive that expands past the size bound, and a retry that succeeds after a failure.
- Deliverable: Test module `apps/codemap-search/tests/e2e/related_terms_fetch.rs` (proposed) registered in `tests/e2e/mod.rs`.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_fetch`; Inputs: the new test module; Expected: exit 0 and `test result: ok` with at least 6 passed tests whose names contain `related_terms_fetch` (a filter that matches nothing also exits 0).
- Ends when:
  - [ ] In every failure case, no file exists at the final path afterwards and the status names the reason.
  - [ ] In the concurrent case, the final file's hash matches the fixture spec and exactly one download wrote it.
- Handoff: Stage 4 receives the test names and results.
- Replan when: File locks behave differently on a supported platform (Linux, macOS, Windows) so the concurrent case cannot be made reliable; record the platform and stop for a parent replan.

### Stage 4 — Hand off the fetch contract
- Starts when: Stage 3's tests pass.
- Work: Write the evidence log with the config keys and defaults, the fetch API (function, spec type, status values), the pinned spec, the download byte bound, the added dependencies with reasons, and the e2e test names.
- Deliverable: Evidence log `docs/related-terms/integration-log/03-fetch-config.md` (proposed).
- Verify: `bounded inspection of docs/related-terms/integration-log/03-fetch-config.md`; Inputs: the log file; Expected: it names the fetch function, the three status values, both config keys with defaults, and the reasons for `tar` and `zstd`.
- Ends when:
  - [ ] `cargo check --manifest-path apps/codemap-search/Cargo.toml` exits 0 on the final state.
- Handoff: The MCP tool child (`docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`) receives `docs/related-terms/integration-log/03-fetch-config.md` and the fetch module.
- Replan when: None — the log records facts verified in Stages 1–3.

## Side Effect Checkpoints
- [ ] Existing config e2e tests pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests config` exits 0.
- [ ] Existing Jev e2e tests pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests jev` exits 0.
- [ ] `cargo tree --manifest-path apps/codemap-search/Cargo.toml -i zstd -e normal` shows one `zstd` version after the change.
- [ ] `flate2` still appears only under `[build-dependencies]` in `apps/codemap-search/Cargo.toml`.
- [ ] Starting `codemap-search mcp` with a fresh `CODEMAP_HOME` creates no `related-terms` folder (nothing calls the fetch yet).

## Acceptance Criteria
- [ ] `related_terms.enabled` defaults to `false` and `related_terms.dataset_path` is optional in code, template, and both config documents.
- [ ] Every download-integrity e2e test passes, and no failure case leaves a file at the final path.
- [ ] A file at the final path is only ever created by a rename after its size and SHA-256 matched the spec.
- [ ] README (en/ko) says the default setup makes no network request and enabling related terms downloads the dataset once.

## Open Questions
- None — the user decided tar.zst, default off, and core e2e tests; remaining choices are bounded in Constraints and Worker decision.
