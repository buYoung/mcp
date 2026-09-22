# [feat] Recommend indexed files through the Rust overview path

## Work Type
feat

## Current State (As-Is)
- [confirmed] `overview::run` reads one published codemap snapshot and returns root/folder/file text or readiness notices — Evidence: `apps/codemap-search/src/tools/overview.rs::run`.
- [confirmed] Path aliases and workspace-root resolution precede rendering — Evidence: `overview.rs::run` and its `monorepo` module.
- [confirmed] Indexed files carry symbols, docs, ranges, and optional navigation without source-body strings — Evidence: `apps/codemap-search/src/parser/types.rs::ExtractedFile`, `ExtractedSymbol`, and `NavigationFile`.
- [confirmed] Improved PoC #1 evaluates all root-index files, scores file fragments, then classifies declarations in the top 24 — Evidence: `mcp_proxy.py::Variant.recommend` and `improved_proxy.py::ImprovedVariant.recommend`.
- [inferred] The live Rust adapter can replace the pre-exported JSON snapshot while retaining one generation for overview and recommendation — Confirm by a snapshot-refresh regression with a held published snapshot.

## Desired Outcome (To-Be)
- Produce root overview recommendations directly from committed in-memory index metadata and the explicit original task query.
- Append at most 24 supported file references with indexed declaration roles, ranges, call candidates, and read arguments; permit an empty recommendation.
- Keep recommendation logic callable independently of MCP dispatch and leave the existing root/folder/file entry behavior available when inactive.

## Scope
### In Scope
- Project all eligible root-index metadata from the same captured snapshot used to construct the root overview.
- Port file-fragment scoring and deterministic local tie ordering; add explicit qualification before selecting up to 24 files and selecting declaration roles.
- Add bounded recommendation rendering and readiness/partial/failure handling.
- Add adapter-level regressions for snapshot identity, root scope, coverage, and output limits.
### Out of Scope
- [hard] Do not use BM25 or a hand-selected file list to prefilter the root candidates.
- [hard] Do not read source bodies, run a filesystem walk, or export a sidecar index merely to create #1 recommendations.
- [hard] Do not apply this mode to folder/file overview requests or implement #2 filtering.
- [deferred] Broad ranker tuning, new models, multi-label declaration roles, and replacing existing codemap views are separate work; implementing the specified no-match policy and recording its evaluation method remain in scope.

## Constraints
- Work on `feat/codemap-jev`, created from `main` / `origin/main` at `97e3ebc8e`. Preserve the existing untracked Jev PoC directory. Do not stage, commit, merge, publish, or run a release as part of this plan.
- Keep Jev inside the existing codemap-search Rust crate as an independently callable common module. Do not introduce a separate crate, Python runtime, stdio proxy, Unix-socket broker, or another MCP server.
- Keep overview recommendation and search filtering independently configurable and disabled by default. Enablement alone must not trigger background API requests.
- Use the original task intent supplied explicitly by the caller. Do not read an agent transcript, hardcode the benchmark question, or reuse another request's task intent.
- Preserve existing calls with their existing arguments: MCP content/error envelopes, path aliases, workspace selection, index readiness notices, output limits, and live read/find/grep behavior.
- Use new regression tests and minimal non-sensitive fixtures where needed; the user approved this scope. Keep CI offline through an injected evaluator or transport. Do not add lint/formatter setups.
- Keep the disposable PoC API key out of source, fixtures, logs, and documents. Resolve operator credentials at the integration boundary. Do not reuse the old key or run new paid API benchmarks without a fresh explicit execution instruction.
- Preserve existing redaction semantics for model-bound data and returned text. Keep original source files and the committed index unchanged. Do not send raw index metadata around the presentation-redaction boundary.
- Report API input/output usage, elapsed time, and applied/bypassed/fallback outcomes separately. Do not claim deterministic scores, general accuracy, or a Rust speedup from the historical Python measurements.
- Use one comparable Score per indexed fragment with standalone levels: no useful evidence; tangential background/generic wrapper; important supporting implementation/configuration/caller/consumer; direct implementation of the requested behavior. Keep the 0–3 ordering in code, never in place of level descriptions.
- Qualify a file when at least one fragment has `P(2)+P(3) > P(0)+P(1)`. Treat equal masses as uncertain. Apply qualification to the complete evaluated catalog before the existing maximum-fragment-score ordering and path tie break; never fill unused slots with unqualified files. Version this initial policy as experimental rather than claiming calibrated quality.
- After complete evaluation with no qualifying file, preserve the base overview and return zero recommendations. Set `recommendation_status=no_match` when every fragment favors the no-useful/tangential group, or `insufficient_evidence` when any fragment is tied or usable indexed evidence is absent. Say only that indexed evidence did not establish a recommendation; do not infer that the implementation does not exist. API failures remain fallback.
- Select at most two declarations per recommended file, each with exactly one representative role from Choice or `unrelated`. A declaration may serve multiple real roles; the selected label is a navigation hint, not an exhaustive classification. Keep a qualified file visible even if no declaration role is supported.
- Batch independent fragment questions with the same state, then batch representative-role questions only for the selected files. The second stage depends on first-stage selection; skip it for empty recommendations and do not add a separate presence call when the existing Score distributions suffice.
- Retain raw Score/Choice answers, candidate/snapshot identities, and question/policy versions in the adapter result. Changing only selection or display policy must be testable from those answers without another inference; do not add persistent caching.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/overview.rs` — Capture base overview and recommendation evidence from one published snapshot.
- `apps/codemap-search/src/tools/overview/monorepo.rs` — Preserve root aliases and canonical workspace resolution.
- `apps/codemap-search/src/tools/overview/jev.rs` (proposed) — Own code-index projection, ranking, and recommendation rendering.
- `apps/codemap-search/src/index/supervisor.rs` — Read the existing committed-snapshot access contract.
- `apps/codemap-search/src/parser/types.rs` — Reuse declaration kinds, ownership, and inclusive-line conversion without changing persisted formats.
- `apps/codemap-search/src/redact.rs` — Route presentation copies through existing masking before external evaluation.
- `apps/codemap-search/experiments/jev-playground/checkpoint-poc/improved_proxy.py` — Reference the corrected recommendation behavior.
- `apps/codemap-search/validation/jev-native/01-runtime.md` (proposed) — Consume the common evaluator contract.
- `apps/codemap-search/validation/jev-native/02-overview.md` (proposed) — Publish the overview adapter handoff.
- [TypeSafe Score](https://docs.typesafe.ai/primitives/score.md) and [candidate selection](https://docs.typesafe.ai/cookbooks/skill_suggestion.md) — Keep graded ranking distinct from candidate qualification and permit no match.

## Execution Plan
### Stage 1 — Capture an indexed recommendation input
- Starts when: `apps/codemap-search/validation/jev-native/01-runtime.md` exists with the common evaluator API, default policies, and successful offline runtime checks.
- Work: Introduce an owned preparation result containing the base overview, all eligible indexed file metadata, snapshot identity, and explicit task intent. Preserve root alias and workspace behavior. Use named shared state and self-contained candidate instruction fields with backticked references; each fragment must identify its file and supply its own evidence. Apply existing redaction before invoking the evaluator.
- No-op when: An existing Rust overview adapter already satisfies this child's candidate-coverage, snapshot, ranking, and rendering checks.
- No-op handoff: Record the adapter API, populated-input checks, and passing commands in `apps/codemap-search/validation/jev-native/02-overview.md` (proposed), then let the parent continue to native integration.
- Deliverable: A typed overview preparation path and a documented candidate identity/coverage mapping.
- Verify: `bounded inspection of overview preparation using a populated root snapshot and a refreshed successor snapshot`; Inputs: `overview.rs::run`, `ExtractedFile`, and adapter regression inputs with more candidates than the recommendation limit; Expected: The prepared base text and candidate set use one generation and include every eligible root file
- Ends when:
  - [ ] Folder/file and warming/dead/empty paths return their existing base behavior without API calls.
  - [ ] No raw metadata bypasses redaction and no private benchmark filenames are hardcoded.
- Handoff: Stage 2 receives the immutable candidate set and base overview.
- Replan when: A current snapshot cannot be retained without waiting on indexing or changing index persistence: stop and return the snapshot contract to the parent.
### Stage 2 — Evaluate and render recommendations
- Starts when: The prepared root input uses one generation and the shared evaluator is available.
- Work: Evaluate every indexed file fragment, apply the documented qualification policy, rank qualified files by maximum fragment score with path-based ties, and select up to 24. Return explicit no-match/insufficient-evidence results when empty. Classify representative roles only for declarations in selected files; attach at most two declarations per file with exact indexed lines, docs, possible calls/callers, and read windows capped at 180 lines. Reserve final output space before adding recommendations.
- Deliverable: A callable Rust overview adapter returning base text, qualified recommendations, raw judgments, recommendation status, and usage/outcome metadata.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview`; Inputs: Populated all-unrelated/tangential, tied, mixed-fit, more-than-24, fragmented-file, missing-role, scope-change, and small-budget cases; Expected: Exit 0, zero recommendations and no role-stage call for empty qualification, at most 24 qualified files otherwise, and every path/range resolving to the prepared input
- Ends when:
  - [ ] No API failure or deadline can turn an incomplete candidate evaluation into a claimed complete ranking.
  - [ ] If evidence or output room is insufficient, preserve the base overview and surface a bounded fallback/bypass reason.
  - [ ] Role labels and possible call matches are presented as navigation evidence, not verified source conclusions.
  - [ ] Qualification precedes the limit; a qualifying file below the unqualified top 24 is not silently lost.
  - [ ] No-match and uncertainty preserve the base overview and remain distinguishable from transport failure and index readiness bypass.
- Handoff: Stage 3 receives the adapter, result shape, and offline test results.
- Replan when: Work requires ranking beyond the specified qualification-plus-maximum-score policy, more than 24 recommendations, or source reads: return the scope change to the parent before expanding the adapter.
### Stage 3 — Publish the overview adapter boundary
- Starts when: The adapter passes its offline behavior checks.
- Work: Record the preparation/evaluation/render entry points, root activation conditions, output reservation behavior, and usage fields. Confirm the existing synchronous overview path remains usable until the integration child wires the new async entry.
- Deliverable: `apps/codemap-search/validation/jev-native/02-overview.md` (proposed) with adapter signatures, scope rules, full candidate coverage, qualification/status/role semantics, question/policy versions, fallback behavior, and regression results.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: The overview adapter and all existing callers of `overview::run`; Expected: Exit 0 with the pre-integration public callers still compiling
- Ends when:
  - [ ] The integration child can enable #1 without interpreting Markdown or loading Python snapshots.
  - [ ] The handoff specifies how to preserve active workspace updates.
- Handoff: The parent and integration child receive `apps/codemap-search/validation/jev-native/02-overview.md`.
- Replan when: Existing callers require a breaking signature change: preserve a compatibility entry point or return to the parent before integration.

## Side Effect Checkpoints
- [ ] Check root aliases, ambiguous workspace errors, and active workspace selection against the existing overview e2e cases.
- [ ] Keep `CodeRange` serialization and `end_line_inclusive()` semantics unchanged.
- [ ] Keep stats sections, readiness notices, and existing output caps in the rendered response.
- [ ] Confirm the evaluator receives presentation copies while the original index and source remain unchanged.

## Acceptance Criteria
- [ ] With #1 active and a populated root snapshot, every eligible root file participates in evaluation or the entire recommendation reports a fallback.
- [ ] The ranked output maps only to input files and includes actionable indexed declarations/read windows when available.
- [ ] Complete all-unrelated evaluation returns zero recommendations; tied or absent evidence is identified without claiming source-level absence.
- [ ] Files qualify before truncation, at most 24 are recommended, and each attached declaration has one representative role with at most two declarations per file.
- [ ] Changing the published snapshot during a fake API delay cannot mix generations in one result.
- [ ] Existing overview calls remain compatible when this adapter is inactive and the native integration handoff is complete.

## Open Questions
- None — The user selected an internal common module, independent default-off modes, and necessary regression tests. Remaining implementation choices are bounded in the stages.
