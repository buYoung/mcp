# [fix] Restore the pre-Jev overview contract

## Work Type
fix

## Current State (As-Is)
- [confirmed] The inspected checkout is `8b8222003584a3875ce3804557d3d5e70113ea4b` on `docs/jev-integrate` — Evidence: `git log` and `git status` on 2026-09-24.
- [confirmed] `d14cc391ff154c28639e9110f183d295e20d62bb` introduced Jev integration. `97e3ebc8e3708df01062fdd3e90b9ac68f7b08f3` is the pre-Jev application baseline; its application tree equals the immediate pre-integration commit `530e476c3f088be18b9e33bfda86a394bdb051b4` — Evidence: `git diff --name-only 97e3ebc8e 530e476c3 -- apps/codemap-search` returned no paths.
- [confirmed] `tools::overview::prepare_root_recommendation` captures every indexed file and `mcp::jev::overview` appends recommendations — Evidence: `src/tools/overview.rs`, `src/tools/overview/jev.rs::recommend`, and `src/mcp/jev.rs::overview`.
- [confirmed] `tools::list_tools` advertises an overview-only `task_query` override and an enabled-dependent open-world annotation; `jev_guidance` requires root overview and inspection of its recommendations before other navigation — Evidence: `src/tools/mod.rs`.
- [confirmed] The pre-Jev overview implementation already has workspace selection, statistics, output caps, aliases, and warming/dead-index notices — Evidence: `git show 97e3ebc8e:apps/codemap-search/src/tools/overview.rs`.
- [confirmed] `codemap::significant_symbols` is currently also consumed by live-body filtering — Evidence: `src/tools/live_symbols/jev.rs::Capture::prepare`. Do not break that consumer while restoring shared rendering.

## Reproduction
- Inspect the diff from `97e3ebc8e` to `8b8222003` in overview dispatch, schema, guidance, configuration, and rendering before editing.
- On the existing repository/monorepo fixtures, compare root overview with Jev enabled against the pre-Jev baseline. Current behavior permits recommendation output and external evaluation; the required behavior is the original local overview contract.
- Use the existing historical recommendation runs under `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-comparison-18x-32k` as recorded evidence, not as a reason to send another paid request.
- Reproduce schema/guidance differences deterministically from `tools/list` and `initial_instructions`. Recommendation ranking itself is model-dependent and is not the restoration oracle.

## Desired Outcome (To-Be)
- Root, workspace, folder, file, and `llms-txt` overview retain their pre-Jev observable behavior, including errors, limits, statistics, aliases, and scope effects.
- Overview never requires a task registration or API key and never invokes Jev.
- No active overview recommendation flag, override, recommendation section, or mandatory recommendation-reading workflow remains.

## Scope
### In Scope
- Restore `src/tools/overview.rs` against the pinned pre-Jev version and remove the overview Jev adapter/module.
- Restore overview schema, description, annotations, dispatch, and original rendering limits against the same baseline.
- Remove only recommendation-related instructions from shared task/navigation guidance.
- Remove overview recommendation configuration from active parsing/defaults/templates/schema synchronization and current English/Korean documentation.
- Retire existing tests that specifically require the removed recommendation feature while preserving the original overview assertions.
- Preserve shared helper consumers until their replacement is integrated; document any temporary visibility-only deviation.
### Out of Scope
- [hard] Reverting entire integration commits or the whole MCP/configuration files, which would remove retained search functionality.
- [hard] Changing workspace discovery, indexing, language statistics, event navigation, or filesystem permissions.
- [deferred] New folder/file recommendation modes or alternative root rerankers.
- [hard] Editing historical validation reports or unrelated `codemap-prod` briefs.

## Constraints
- The restoration oracle is `97e3ebc8e3708df01062fdd3e90b9ac68f7b08f3`, not merely the current binary with `overview_enabled=false`.
- Restore observable overview behavior exactly. For files changed only to add recommendation plumbing, restore the baseline source rather than retaining dormant feature branches.
- Keep current search/task-registration behavior functional until child 02 replaces it. Do not remove shared Jev transport dependencies.
- A stale `analysis.jev.overview_enabled` entry must never reactivate evaluation. Follow existing never-exit configuration behavior for obsolete input and preserve unrelated user values.
- Update current docs and generated templates together; leave historical records intact.
- Use existing checks and fixtures. Do not add test files or cases, formatter/lint setup, or new dependencies.
- Store handoff evidence as machine-readable JSON. Do not publish benchmark tables into `CHECKPOINT.md`.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/overview.rs` — restore `run` and remove recommendation preparation.
- `apps/codemap-search/src/tools/overview/jev.rs` — remove the recommendation implementation and its module wiring.
- `apps/codemap-search/src/codemap/mod.rs` — restore original root-row rendering and inspect shared helper consumers.
- `apps/codemap-search/src/mcp/mod.rs` — restore only the overview dispatch branch and retain scope/lifecycle updates.
- `apps/codemap-search/src/mcp/jev.rs` — remove the overview adapter while preserving search transport.
- `apps/codemap-search/src/tools/mod.rs` — restore overview tool metadata and remove compulsory recommendation navigation.
- `apps/codemap-search/src/config/jev.rs` — retire the overview enable flag without breaking never-exit parsing.
- `apps/codemap-search/src/config.rs` — align template/schema-sync behavior.
- `apps/codemap-search/src/config/layout.rs` — remove retired overview keys from the active configuration layout.
- `apps/codemap-search/src/config_template.toml` — remove the active English overview recommendation setting.
- `apps/codemap-search/src/config_template.ko.toml` — apply the matching Korean template change.
- `apps/codemap-search/docs/configuration.md` — update the active Jev and overview contract with its Korean counterpart.
- `apps/codemap-search/README.md` — update overview-related usage with `README.ko.md`.
- `apps/codemap-search/tests/e2e/codemap.rs` — reuse the existing pre-Jev navigation checks.
- `apps/codemap-search/tests/e2e/jev_overview.rs` — identify obsolete feature expectations.
- `apps/codemap-search/validation/jev-search/01-overview.json` (proposed) — publish the restoration handoff.

## Execution Plan
### Stage 1 — Pin the restoration population
- Starts when: The inspected revision and both baseline commits are available and existing user changes have been recorded.
- Work: Enumerate every overview-specific hunk and active contract surface. Capture baseline tool metadata and deterministic outputs using existing fixtures, matched configuration, and ready statistics; record warming/dead cases separately.
- No-op when: Every enumerated overview surface already matches the pinned baseline and no overview Jev invocation/configuration path remains.
- No-op handoff: Publish the same complete evidence at `apps/codemap-search/validation/jev-search/01-overview.json` and let child 02 continue without a restoration edit.
- Deliverable: The restoration matrix at `apps/codemap-search/validation/jev-search/01-overview.json` with baseline/current revisions, enumerated paths, fixture/configuration identities, and outstanding differences.
- Verify: `Bounded inspection of git diff 97e3ebc8e HEAD -- apps/codemap-search/src/tools/overview.rs apps/codemap-search/src/codemap/mod.rs apps/codemap-search/src/tools/mod.rs apps/codemap-search/src/mcp/mod.rs`; Inputs: the four tracked files and pinned commits; Expected: every overview-specific difference is classified before mutation.
- Ends when:
  - [ ] The matrix covers output, schema, guidance, annotations, configuration, scope, limits, statistics, readiness, and external-call behavior.
- Handoff: Stage 2 receives the exact restoration population and baseline artifacts.
- Replan when: A post-baseline change unrelated to Jev changes overview behavior; isolate its ownership and resolve the conflict in the parent before overwriting it.

### Stage 2 — Restore the original navigation surface
- Starts when: Stage 1 provides the restoration matrix.
- Work: Restore the overview implementation and contract, remove its Jev adapters and active configuration, and update current documentation and obsolete existing expectations.
- Deliverable: Working source and documentation with the restoration matrix updated to identify each removed or restored surface.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the changed Rust package; Expected: exit 0 with no broken shared-helper consumers.
- Ends when:
  - [ ] All overview-specific Jev paths are removed and search remains buildable.
  - [ ] Existing root limits, statistics, path aliases, and scope transitions remain represented by the baseline implementation.
- Handoff: Stage 3 receives the buildable restoration and exact changed-file population.
- Replan when: Removal requires changing search semantics or another child's public contract; return that dependency to the parent instead of broadening this revert.

### Stage 3 — Prove restoration and publish the handoff
- Starts when: Stage 2 is buildable or Stage 1 established the complete no-op route.
- Work: Run the existing overview checks and compare matched baseline/current tool metadata and outputs. Confirm the overview evaluator call count is zero and stale flags cannot reactivate it.
- Deliverable: `apps/codemap-search/validation/jev-search/01-overview.json` with `completed`, `baseline_revision`, `source_revision`, `source_diff_hash`, `surfaces`, `checks`, `remaining_differences`, and external evidence paths.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::codemap`; Inputs: existing codemap tests plus the separately inspected Stage 1 fixture/configuration matrix; Expected: nonzero executed test count, exit 0, zero unexplained overview differences in the comparison, and zero Jev calls from overview.
- Ends when:
  - [ ] Every restoration surface has evidence and no unexplained difference remains.
  - [ ] The handoff sets `completed=true` only after the existing checks and baseline comparison succeed.
- Handoff: Child 02 consumes `apps/codemap-search/validation/jev-search/01-overview.json` as its first-stage prerequisite.
- Replan when: The comparison fails; stop child 02, correct this restoration, rerun its affected checks, and update the parent handoff before continuing.

## Side Effect Checkpoints
- [ ] Monorepo overview still changes subsequent search scope exactly as before Jev.
- [ ] Root statistics/readiness timing is controlled in comparisons rather than hidden by broad output normalization.
- [ ] Ordinary overview size-limit errors and file/folder output shapes remain unchanged.
- [ ] Shared search/read/grep helpers still compile throughout this child's changes.
- [ ] Unrelated user configuration values and historical validation artifacts remain untouched.

## Acceptance Criteria
- [ ] Every enumerated overview contract matches `97e3ebc8e` with no recommendation-specific exception.
- [ ] Overview works without registered task questions and without Jev credentials, with no evaluator invocation.
- [ ] Active schemas, instructions, templates, and English/Korean usage docs no longer expose overview recommendations.
- [ ] The completed handoff contains baseline comparison evidence and successful existing codemap verification.

## Open Questions
- None — the user explicitly required exact pre-Jev overview restoration and the baseline is established by Git history.
