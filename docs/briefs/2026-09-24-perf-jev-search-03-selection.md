# [perf] Select search evidence before rendering

## Work Type
perf

## Current State (As-Is)
- [confirmed] At `8b8222003`, `prepare_detail` builds ranked file output before `run_inner_with_filter` captures displayed bodies and `finish_detail` renders the retained output — Evidence: `src/tools/search/mod.rs::DetailState` and `run_inner_with_filter`.
- [confirmed] Existing retention protects incomplete bodies, small bodies, retained containers, and connected declarations after obtaining judgments — Evidence: `src/tools/search/jev.rs::apply_policy`.
- [confirmed] Search preserves declaration/relationship metadata and compact tail rows while filtering eligible bodies — Evidence: `src/tools/search/grouped.rs`, `render.rs`, and `jev.rs`.
- [confirmed] Paired replay of the original six filtered sessions found 18 modified responses among 312 source responses, with 294 unchanged controls — Evidence: `data/replay-summary.json` in the 20260924-rg-default-and-jev-analysis checkpoint run.
- [inferred] Better question wording alone may leave most output unchanged because retention and rendering still decide what reaches the main LLM — Confirm by: paired replay after child 02 using identical candidate evidence and output budgets.

## Baseline Measurement
- Historical `#13.2` reduced whole raw source-response characters by 2.016%, and `#13.3` by 1.910%; these are character measurements, not main-model token savings.
- Historical `#13.2` evaluated an average of 26 body-filter calls and changed output in only 3. Preserve this distinction between paid judgments and useful output changes.
- The historical six filtered sessions reread omitted worker bootstrap/TypeORM source. Treat these existing recovery records as evidence of lost utility, not as permission to hardcode file-name exceptions.
- Reuse `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-rg-default-and-jev-analysis/data/replay-summary.json` and `data/replays/`. Establish a new matched baseline on child 02 before changing selection.
- Hold query text, candidate identities, source revisions, registered questions, model version, and output caps constant for paired comparisons.
- Local target: strictly reduce aggregate delivered source text on the populated recorded comparison set without losing its required evidence. Global target, verified by child 04: lower mean main-LLM total tokens than fresh frozen `#13` while preserving rubric coverage.

## Desired Outcome (To-Be)
- Jev chooses which candidate functions and bounded supporting evidence are rendered, before large detail output is assembled.
- Related candidates retain useful bodies and supported relationships; uncertain candidates remain discoverable with explicit missing-evidence/partial notices.
- Known non-beneficial evaluations are bypassed before the API, and omitted source remains directly recoverable through ordinary read.
- Output reduction is measured independently from changes in retrieval wording or agent navigation.

## Scope
### In Scope
- Move the Jev-active candidate/evidence selection boundary between retrieval and final detail rendering.
- Use child 02's per-question decisions to select direct matches, verified supporting evidence, uncertain locators, and compact exclusions.
- Supply bounded call-site/event evidence needed to distinguish generic transport helpers from target communication paths.
- Exclude deterministically retained or non-shrinkable candidates from paid evaluation where doing so cannot change the final decision.
- Rework over-broad container/relationship retention where necessary to preserve concrete support without retaining unrelated connected code automatically.
- Keep result/source observations, output caps, stale evidence, partial output, and read suggestions consistent with delivered content.
- Update search-specific tool guidance and current docs to describe selection and recovery without mandatory extra reads.
### Out of Scope
- [hard] Changing BM25 retrieval/ranking, workspace routing, source parsers, or index schemas to manufacture a better comparison.
- [hard] Filtering exact `event_key` map results, ordinary read, grep, or overview.
- [deferred] Persistent decision caching, learned ranking weights, adaptive HTTP limits, or a separate broad retrieval expansion engine.
- [hard] New test files/cases or hardcoded exceptions for the benchmark repository.

## Constraints
- Preserve the Jev-disabled search route byte-for-byte for matched inputs, apart from changes already owned by predecessors.
- Preserve exact-name ranking and supported argument names, including `workspace_scope`, `caller_context`, `include_events`, and `event_key`.
- Build evidence only from bounded available indexed/live source and supported static relationships. Mark missing or stale cross-file links explicitly instead of inventing them.
- Source completeness is not the same as complete communication context. A complete function with unresolved channel/caller evidence may still be uncertain.
- Use child 02's all/any semantics. Keep thresholds explicit and calibrate them against the existing recorded evidence; do not reinterpret a legacy unrelated-probability config key as a positive relevance score silently.
- Preserve the target-flow relationship in Q2. Two independently true properties do not prove that the worker and the event belong to the same flow.
- Preserve concrete support for known relevant functions; do not protect every class member or entire transitive component solely because one member is relevant.
- Keep declarations, exact source locations, and bounded recovery hints for omitted/uncertain candidates. Report exhausted budget and remaining candidates explicitly; never claim an exhaustive answer from truncated output.
- Use configured output budgets, including the client ceiling. Score tables and omission notes must not consume the bytes saved from source.
- Skip API work only when final retention/omission cannot benefit from a judgment. Unknown relevance alone is not a deterministic bypass reason.
- Preserve masking, identity/hash verification, caller cancellation, absolute deadlines, and whole-search fallback.
- Record numeric judgments, decision reasons, eligibility, protected/omitted counts, bytes, timing, and provider usage without persisting raw secrets or unmasked source.
- Keep this child cohesive under BDR K2: selection, relationship retention, and rendered-source accounting share the same missing-evidence diagnosis and recovery path.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/search/mod.rs` — move selection relative to `prepare_detail` and `finish_detail`.
- `apps/codemap-search/src/tools/search/jev.rs` — consume task judgments and replace unnecessary post-evaluation protection work.
- `apps/codemap-search/src/tools/search/grouped.rs` — select function detail blocks and supporting context before serialization.
- `apps/codemap-search/src/tools/search/render.rs` — account for compact locators, partial notices, and output caps.
- `apps/codemap-search/src/tools/search/monorepo.rs` — preserve workspace routing and grouped behavior.
- `apps/codemap-search/src/mcp/jev.rs` — preserve fallback and expose bounded stage measurements.
- `apps/codemap-search/src/tools/mod.rs` — update search evidence guidance without changing restored overview metadata.
- `apps/codemap-search/src/config/jev.rs` — preserve/document threshold semantics if calibration requires changes.
- `apps/codemap-search/docs/configuration.md` — align selection and recovery descriptions with the Korean counterpart.
- `apps/codemap-search/docs/analysis.md` — align delivered-source accounting with the Korean counterpart.
- `apps/codemap-search/tests/e2e/search.rs` — run the existing search contract checks.
- `apps/codemap-search/tests/e2e/jev.rs` — retain existing failure and delivered-text checks with necessary expectation updates.
- `apps/codemap-search/validation/jev-search/02-task-judgments.json` (proposed) — consume the implemented task/result contract.
- `apps/codemap-search/validation/jev-search/03-selection.json` (proposed) — publish paired evidence and the final selection contract.

## Execution Plan
### Stage 1 — Capture a matched selection baseline
- Starts when: `apps/codemap-search/validation/jev-search/02-task-judgments.json` reports completed end-to-end task evaluation and exposes its schema/decision contract.
- Work: Reuse the historical replay inputs, pin candidate/body hashes and task questions, and measure child 02 output plus protected/no-change evaluations before selection edits. Record unsupported or incomplete branches separately.
- No-op when: The complete requested pre-render selection already exists and matched replay proves useful reduction with all required evidence preserved.
- No-op handoff: Publish complete evidence at `apps/codemap-search/validation/jev-search/03-selection.json` and allow child 04 to measure the unchanged implementation.
- Deliverable: Baseline inputs and measurements referenced by `apps/codemap-search/validation/jev-search/03-selection.json`, including query/candidate/source hashes and nonempty response counts.
- Verify: `Bounded comparison of the historical data/replays/ population and the child 02 adapter output on the same recorded requests`; Inputs: the six original filtered-session request sequences and frozen target source; Expected: matched candidate evidence, recorded exclusions, and an explicit nonzero evaluated population.
- Ends when:
  - [ ] Retrieval differences cannot be misreported as Jev compression.
  - [ ] Existing bootstrap/TypeORM recovery, generic transport, and source-completeness limitations are mapped to their original records.
- Handoff: Stage 2 receives fixed inputs, eligibility/protection costs, and evidence-retention obligations.
- Replan when: Historical inputs or source hashes cannot be reproduced; return to the parent to rebase the measurement before optimization.

### Stage 2 — Select bounded useful evidence before rendering
- Starts when: Stage 1 supplies the matched baseline and child 02 supplies per-candidate task judgments.
- Work: Integrate pre-render selection, bounded supporting context, early deterministic bypasses, compact uncertain/excluded locators, and correct delivered-source accounting. Preserve original search fallback and exact-event branches.
- Deliverable: A buildable selection path plus recorded policy/threshold decisions and instrumentation.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the changed package plus bounded inspection from candidate selection to SearchOutput.source_files; Expected: exit 0 and every inspected source observation corresponds to delivered source.
- Ends when:
  - [ ] Selection reduces renderable source before large detail assembly.
  - [ ] Uncertain candidates remain discoverable and ordinary read restores exact source.
  - [ ] Deterministic no-benefit candidates are not sent merely to be protected afterward.
- Handoff: Stage 3 receives the integrated path and selected policy.
- Replan when: A change requires widening retrieval, weakening source identity, or silently dropping uncertain candidates to meet the target.

### Stage 3 — Verify selection separately from agent behavior
- Starts when: Stage 2 is buildable or Stage 1 proves the no-op route.
- Work: Repeat the matched replay once for the final change, inspect required evidence and recovery, and run the affected existing search checks. Also run `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev` to exercise the Jev-enabled MCP path and fallback behavior after selection changes. Keep raw provider judgments so later policy interpretation is reproducible.
- Deliverable: `apps/codemap-search/validation/jev-search/03-selection.json` with `completed`, `source_revision`, `source_diff_hash`, `policy`, `baseline`, `paired_results`, `coverage`, `recovery`, `checks`, and external evidence paths.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::search`; Inputs: existing search tests, separately executed Jev integration cases, and paired output for Stage 1's fixed requests; Expected: nonzero executed counts, exit 0 for both commands, lower aggregate paired output text, and no loss of the recorded required evidence.
- Ends when:
  - [ ] Paired results distinguish characters/bytes from model tokens and identify retained, omitted, uncertain, and bypassed candidates.
  - [ ] The final source/binary identity is ready for whole-agent measurement.
- Handoff: Child 04 consumes `apps/codemap-search/validation/jev-search/03-selection.json` without treating local payload reduction as final token success.
- Replan when: Coverage fails or output does not improve; stop child 04, return to the parent, perform bounded selection/evidence correction and re-verification, then refresh topology and handoffs.

## Side Effect Checkpoints
- [ ] Caller/callee, static event maps, bus identity, qualifiers, source routes, and workspace scope do not gain unsupported relationships.
- [ ] Partial/oversized/stale source remains explicitly incomplete and is never confidently excluded on missing evidence alone.
- [ ] Search fallback, exact-event output, and Jev-disabled output remain intact.
- [ ] Direct reads of previously omitted ranges return full source without changing configuration.
- [ ] Output caps include compact notes and headers and never rely on outer Codex truncation for correctness.
- [ ] Usage accounting excludes notes and removed bodies while retaining truthful source observations.

## Acceptance Criteria
- [ ] Matched replay demonstrates lower aggregate delivered text with preserved required evidence and recorded uncertainties.
- [ ] Per-question judgments affect candidate selection before detail rendering rather than only adding metadata.
- [ ] Proven no-benefit evaluations are bypassed before API dispatch, with reasons recorded.
- [ ] Ordinary read remains a working original-source recovery path.
- [ ] Existing affected search checks pass and the completed handoff is ready for child 04's independent end-to-end comparison.

## Open Questions
- None — the user approved coverage preservation and lower mean main-model total tokens; local algorithm and threshold calibration are bounded by those outcomes.
