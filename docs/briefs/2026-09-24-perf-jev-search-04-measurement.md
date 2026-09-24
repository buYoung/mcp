# [perf] Verify token savings and retained coverage

## Work Type
perf

## Current State (As-Is)
- [confirmed] Historical runs freeze `#13` at source `8b8222003584a3875ce3804557d3d5e70113ea4b` with Jev disabled — Evidence: `manifest.json` in `20260924-comparison-18x-32k`.
- [confirmed] Historical `#13` averaged 896,939.7 main-model total tokens over three runs. The original Jev variants averaged 1,114,512.0, 1,095,582.7, and 1,059,451.3 — Evidence: `data/comparison-default-rg.json` in `20260924-rg-default-and-jev-analysis`.
- [confirmed] The corrected rg baseline removes forced output-limit settings and budget instructions; individual tool calls may still choose their own native budgets — Evidence: `run_rg.py` and `data/final-validation.json` in that run.
- [confirmed] Existing orchestration runs three Codex sessions concurrently within a variant and serializes variant groups — Evidence: `run.py::execute` and its manifest fields.
- [confirmed] Existing quality grading has six core and five extended criteria and is coverage measurement, not a general accuracy proof — Evidence: `data/quality.json` and `evidence/quality-rubric.json` in the original comparison run.
- [confirmed] Enabling the search flag does not prove evaluation ran: unresolved evaluator configuration returns plain search, and an event-only branch can return without judgment — Evidence: `src/mcp/jev.rs::search`, its `host.resolve` fallback, and its event-only result branch.

## Baseline Measurement
- Target: `/Users/buyong/workspace/hicare/hicare-rpm-api` at `b1a3b06a0e1ed627260e917c0c80a0b301112b4d` with the original manifest's tracked-source diff hash. Verify both before reuse.
- Codex conditions: `gpt-6-astra`, reasoning `max`, historical CLI `0.156.1`, read-only task, no nested agents, no prior benchmark access, warmed isolated indexes, and 5MB read cap.
- Task: `codemap-search를 활용해서 이벤트버스 패턴 중 measure worker와 통신하는 구간 다 확인해줘.`
- rg task: `codemap-search를 사용하지않고, 이벤트버스 패턴 중 measure worker와 통신하는 구간 다 확인해줘.`
- Primary fresh control: archived `binaries/13` from the original comparison, SHA-256 `4800924c1b541080dfdfa48318956f196a2d29e564f1f9641471078ebb469770`, Jev off, existing 32,000 Codex/MCP output settings.
- Keep the same 32,000 settings for new codemap groups. For rg, use native defaults without forced global/per-server output limits or budget-specific developer instructions.
- User-approved target: fresh improved search must have a strictly lower three-run mean main-LLM total-token count than fresh frozen `#13`, with no additional omissions under the existing rubric.
- Preserve input/cached/uncached/output/total token definitions. Reasoning output is a subset of output and must not be added twice.

## Desired Outcome (To-Be)
- A reproducible fresh comparison establishes whether the completed search design reduces main-model tokens while preserving the existing evaluated evidence.
- All per-run results, failures, truncation, Jev usage, and source/configuration identities are available for inspection.
- The user receives the comparison and conclusions in conversation; no benchmark table is added to `CHECKPOINT.md`.

## Scope
### In Scope
- Freeze final artifacts and run four fresh groups: native-default rg, frozen `#13` off, improved binary Jev off, and improved binary search-only Jev on.
- Run exactly three sessions concurrently within each group; run groups sequentially and do not overlap paired replay/build/index warmup with timed sessions.
- Use the original scripts and metric extractors as the route, copying/adapting them into a new checkpoint run directory rather than overwriting old evidence.
- Grade final answers with the unchanged 11-item rubric and inspect omitted/recovered evidence.
- Report historical `#11.1` and `#13.1–#13.3` as clearly dated reference columns if included, never as fresh reruns.
- Produce a compact machine-readable handoff that points to raw evidence and records the acceptance decision.
### Out of Scope
- [hard] Changing source or target configuration between runs without a recorded variant transition and restoration.
- [hard] Tuning the implementation during a timed comparison or selectively dropping/retrying unfavorable runs.
- [hard] New benchmark tasks, new test cases, global CLI/model configuration changes, commits, publishing, or production deployment.
- [deferred] Statistical significance claims, broad workload generalization, and a larger benchmark corpus.

## Constraints
- Inspect and copy the existing harness before use: the old `run.py` forces 32,000 for every original group and must not be used unchanged for rg.
- The ordinary benchmark agent generates its task-specific questions from the user request. Do not hand it privileged answer paths, final rubric answers, or a hand-optimized query unavailable to the other groups.
- For diagnostic paired replay only, keep questions/candidates fixed and label it separately from whole-agent measurement.
- Preserve task, target source, model/version, MCP set, index readiness, output policy, and source permissions within each matched comparison.
- If the model or CLI version is unavailable, return to the parent to agree a newly paired baseline; never silently compare unlike environments.
- Record source HEAD plus working-tree diff/source hashes because implementation may remain uncommitted. Freeze and hash binaries, configuration, harness, prompts, session ids, and timestamps.
- Obtain credentials only through the already-authorized `TYPESAFE_API_KEY` environment path or equivalent non-persisted secure input. Do not embed the supplied key in files, arguments, transcripts, or handoffs.
- Restore the exact target configuration and preserve its pre-existing tracked and untracked changes. Record process cleanup after every group.
- Keep all three original outcomes. Separate invalid-run/environment failures from performance failure; report the reason and return to the parent before scheduling replacement evidence.
- Establish Jev exercise from observed data in each improved-on run: a registered question list, at least one successful provider evaluation, and its candidate judgments consumed by final search selection. An enabled flag or HTTP attempt alone is insufficient; zero omissions may still be a valid evaluated retention decision.
- If an improved-on run is entirely bypassed/fallback-only or lacks this trace, keep its tokens and final answer but mark Jev exercise as unproven. Do not mark the planned active-Jev comparison accepted or silently replace that run. Return to the parent with the actual cause; retain normal production fallback behavior.
- Coverage gate: all six core criteria must appear in every improved run. For each of the eleven unchanged criteria, its count of covered improved runs must be at least the fresh frozen `#13` count; gains in one criterion cannot compensate for loss in another.
- Report rubric counts as coverage, not proven overall accuracy. Record source-supported false claims separately and do not call a run quality-preserving when new contradictory claims are found.
- Report main-model tokens separately from Jev tokens and money. Also report cached/uncached tokens, wall time, tool wait, tool calls, output size, truncations, original-source recovery, and provider fallback/unknown usage.
- Primary acceptance uses main-model total tokens. Time and Jev cost are reported outcomes without an invented pass threshold.
- Report three separate deltas: frozen `#13` to improved-on for the approved overall target, frozen `#13` to improved-off for changes outside active Jev evaluation, and improved-off to improved-on for the observed incremental Jev effect. A passing overall target does not by itself establish an incremental Jev benefit; the third delta is diagnostic and is not a new acceptance threshold.
- Keep raw logs and full matrices under a new `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/` directory. Repository handoff JSON contains only identity, summary, checks, and evidence pointers.
- Do not claim improvement from three runs as statistical significance.

## Related Files / Entry Points
- `apps/codemap-search/Cargo.toml` — produce the final release binary with the existing locked build.
- `apps/codemap-search/validation/jev-search/03-selection.json` (proposed) — consume the final source and paired-selection evidence.
- `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-comparison-18x-32k/run.py` — reuse `prepare`, `warm`, and `execute` orchestration in a new run directory.
- `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-comparison-18x-32k/aggregate.py` — preserve metric definitions and three-run distributions.
- `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-comparison-18x-32k/validate.py` — reuse cleanup/hash/configuration checks.
- `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-rg-default-and-jev-analysis/run_rg.py` — preserve native-default rg behavior.
- `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-rg-default-and-jev-analysis/replay_plain.py` — inspect the paired replay method without confusing it with a fresh session.
- `/Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-comparison-18x-32k/evidence/quality-rubric.json` — reuse the exact coverage criteria.
- `apps/codemap-search/validation/jev-search/04-measurement.json` (proposed) — publish whole-work acceptance evidence.

## Execution Plan
### Stage 1 — Freeze the comparison and establish readiness
- Starts when: `apps/codemap-search/validation/jev-search/03-selection.json` is complete and identifies the final source, policy, and successful paired checks.
- Work: Freeze the release binary and fresh control inputs, verify historical hashes, prepare a new external run directory, and adapt the existing harness for four groups. Inspect commands and credential handling before launch.
- No-op when: A complete comparison for these exact final source/configuration/prompt hashes already satisfies all four groups, concurrency requirements, and coverage gates.
- No-op handoff: Publish the verified existing evidence pointers at `apps/codemap-search/validation/jev-search/04-measurement.json` and let the parent evaluate global acceptance without extra paid runs.
- Deliverable: A frozen manifest and readiness evidence referenced by `apps/codemap-search/validation/jev-search/04-measurement.json`.
- Verify: `cargo build --manifest-path apps/codemap-search/Cargo.toml --release --locked`; Inputs: final source plus bounded inspection of the archived control binary, target source/configuration, adapted commands, and four group definitions; Expected: exit 0, verified hashes, three concurrent runs per group, no rg output overrides, and no credentials persisted.
- Ends when:
  - [ ] The four groups have explicit binary/configuration/prompt identities and equal conditions where required.
  - [ ] Index warmup is complete and excluded from timed sessions.
- Handoff: Stage 2 receives the frozen manifest and reviewed executable harness commands.
- Replan when: A pinned input/version/hash is missing or readiness cannot be established; stop dependent measurement, return to the parent, activate bounded environment correction/re-verification, and refresh topology/handoffs before resuming.

### Stage 2 — Run the frozen comparison
- Starts when: Stage 1's manifest and readiness checks are complete.
- Work: Execute the four groups in the manifest order, three sessions concurrently per group. Collect complete raw transcripts, metrics, provider usage, and final answers without changing implementation. For each improved-on run, retain a masked registration-to-evaluation-to-selection trace and separately count successful evaluations, bypasses, and fallbacks.
- Deliverable: Twelve original session records and the external run manifest referenced by the measurement handoff.
- Verify: `Bounded inspection of all manifest sessions using the reused aggregation/validation scripts`; Inputs: four groups, three original ids per group, and the three improved-on activation traces; Expected: twelve accounted outcomes, concurrent timestamps within each group, no inter-group overlap, exact configuration restoration, and proven active Jev evaluation/selection in each improved-on run.
- Ends when:
  - [ ] Every original outcome is retained and errors/truncations are accounted for.
  - [ ] Every improved-on run has observed successful Jev evaluation consumed by selection, or its unproven exercise state stops acceptance and is routed to the parent.
  - [ ] No benchmark child process remains and the target/configuration state is restored.
- Handoff: Stage 3 receives the frozen twelve-session evidence population.
- Replan when: A run is invalid, canceled, or environmentally mismatched; stop successors, preserve all evidence, return to the parent for bounded correction and a documented replacement-run decision, then refresh handoffs.

### Stage 3 — Grade coverage and decide acceptance
- Starts when: Stage 2 provides the complete accounted comparison or Stage 1 proves the complete no-op route.
- Work: Grade each final answer with the unchanged rubric, inspect recovery and unsupported claims, validate Jev exercise, and compare fresh token means. Report the overall, improved-off, and incremental Jev deltas separately and deliver the result table in conversation.
- Deliverable: `apps/codemap-search/validation/jev-search/04-measurement.json` with `completed`, `source_identity`, `manifest_path`, `session_ids`, `metrics`, `jev_exercise_by_run`, `comparison_deltas`, `coverage_by_criterion`, `recovery`, `checks`, `accepted`, and `failure_owner` when needed.
- Verify: `Bounded independent arithmetic and rubric inspection against original session records`; Inputs: all twelve sessions and the unchanged 11-item rubric; Expected: no token double-counting, improved mean total tokens below fresh frozen #13, preserved per-criterion coverage, and no new source-contradicting claims.
- Ends when:
  - [ ] The comparison reports per-run values and mean/range with cached and uncached tokens separated.
  - [ ] Acceptance is explicitly true or false and the user has received the chat result.
- Handoff: The parent consumes `apps/codemap-search/validation/jev-search/04-measurement.json` for global acceptance; mark this child complete only when its acceptance criteria hold.
- Replan when: Token or coverage acceptance fails; stop completion, return to the parent, activate bounded correction in child 02 or 03 plus re-verification, recalculate topology/handoffs, and rerun only comparisons invalidated by that correction.

## Side Effect Checkpoints
- [ ] Benchmark prompts contain no leaked prior answers, hidden file recommendations, or hand-curated task questions.
- [ ] rg has no forced global output limit or 32,000-budget instruction; model-selected per-call budgets are recorded truthfully.
- [ ] Codemap groups use identical output limits and equivalent index readiness.
- [ ] Source/configuration identity includes uncommitted edits and no unrelated work is overwritten.
- [ ] Three-run concurrency is verified from actual timestamps, not just requested subprocess count.
- [ ] Jev activation is verified from consumed provider judgments, not just configuration or request attempts; bypass/fallback-only outcomes remain visible.
- [ ] Main-model, Jev, cache, reasoning, and money metrics retain distinct meanings.
- [ ] The checkpoint narrative/table and historical raw runs remain unchanged.

## Acceptance Criteria
- [ ] Twelve original fresh outcomes, or an exact-hash existing comparison satisfying the no-op route, are fully accounted for.
- [ ] All three improved-on runs prove successful Jev evaluation consumed by final selection; an unexercised mechanism is reported as inconclusive rather than an accepted active-Jev comparison.
- [ ] Improved search has lower three-run mean main-LLM total tokens than fresh frozen `#13` under matched conditions.
- [ ] Every core criterion is covered in all improved runs and no existing criterion's covered-run count regresses against fresh frozen `#13`.
- [ ] No new source-contradicting final claim is concealed by the coverage aggregate.
- [ ] Wall time, Jev usage/cost, truncation, recovery, and variability are reported alongside tokens.
- [ ] The report distinguishes the approved frozen-#13 improvement from the improved-off/improved-on diagnostic, including when the incremental Jev result is neutral or worse.
- [ ] Results are delivered in conversation and the handoff contains reproducible evidence pointers with restored environment state.

## Open Questions
- None — the user approved coverage preservation and a lower three-run mean total-token count without imposing an additional minimum percentage.
