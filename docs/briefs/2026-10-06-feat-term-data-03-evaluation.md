# [feat] 유의어 반환 검증과 후속 단계 진입 근거 작성

## Work Type
feat

## Current State (As-Is)
- [confirmed] The user defined the sequence as dataset construction and actual synonym-return verification, then LLM judgment, then a later integration experiment. This briefset details only the first phase — Evidence: the 2026-10-06 clarification and authoring scope answer.
- [confirmed] The user selected canonical JSONL, generated SQLite, local PoC access, and future GitHub Releases/global-directory delivery — Evidence: the authoring decision answers. No release publication or downloader is authorized by this child.
- [confirmed] The existing Grafana dataset contains 36 questions and reference code paths, not a synonym-relation gold set — Evidence: `benchmark/v2/data/dataset.json`, `questions[].prompt`, `entrypoint`, and `evidence[].path`. Its file-retrieval scores do not directly prove synonym quality.
- [confirmed] Authoring inspected real source records and one full WordNet core archive, but did not build the planned merged dataset, execute its lookup, or measure performance — Evidence: the preceding source-research observations and absence of an implemented experiment at authoring.
- [confirmed] Observed sources have both useful relations and important gaps: WordNet connects delete/erase in a digital-information sense but retry/rehear in a legal sense; Wikidata's event-loop entity has alternate expressions whereas the inspected memoization entity has none — Evidence: child 01's pinned source references and authoring counts. These already-known examples are diagnostic controls, not blind evaluation evidence.
- [inferred] A dictionary may produce non-empty results while returning only aliases, abbreviations, unrelated senses, or statistical associations. Repository filtering can also remove every otherwise useful relation — Confirm by: separate before/after-filter measurements and source-grounded review in this child.
- [inferred] LLM judgment can only be evaluated meaningfully after candidate evidence and index-time statistics are dependable; it cannot retroactively establish correct counts or repair missing provenance — Confirm by: technical handoff verification and explicit remaining-error classification in the phase-1 report.

## Desired Outcome (To-Be)
- Frozen, reproducible lookup observations show what the dataset actually returns, with genuine synonym evidence separated from aliases, abbreviations, associations, wrong senses, and uncertain judgments.
- A Korean report distinguishes data construction success, serving correctness, observed semantic usefulness, costs, and unverified generality without asserting product search improvement.
- The report provides evidence-based entry conditions for an LLM-judgment phase and a later integration phase, while implementing neither.

## Scope
### In Scope
- Add experimental query inputs, review records, execution/evaluation scripts, and result aggregation under `apps/codemap-search/experiments/related-terms-poc/`.
- Freeze a non-empty set of development-oriented inputs with intended meanings before measured evaluation. Cover explicit synonyms, aliases, ambiguous abbreviations, phrases, spelling/forms, unregistered terms, known source gaps, and wrong-sense/association negatives.
- Exercise the actual local Rust lookup and index-time statistics artifacts from child 02. Save full candidate provenance and pre/post-filter outcomes, not just attractive examples.
- Measure build/load/lookup work separately, including artifact sizes, memory observations when available, cold/warm conditions, result bytes, and index-finalization cost.
- Write `README.ko.md`, compact machine-readable results, and a phase-boundary recommendation based on measured evidence.
### Out of Scope
- [hard] Do not change source-selection/scoring/filter rules to fit the final evaluation inputs or results. Return defects to their owning child with a new version and explicit invalidation of affected evidence.
- [hard] Do not implement or call an automated LLM judge, generate synonym candidates with model calls, invoke embeddings, or perform paid evaluation.
- [hard] Do not modify product source, Cargo manifests/lock, public MCP/CLI/config contracts, benchmark code/data, or the benchmark-managed Grafana checkout.
- [hard] Do not publish a GitHub Release, download datasets during lookup, install into the real global directory, or run `pnpm bench:start`.
- [deferred] Grafana question-solving, top-5 file recovery, equal-search-budget controls, suggested-query adoption, actual agent token savings, and production ranking/transport behavior belong to the later integration experiment.
- [deferred] Training or tuning a repository-specific dictionary, Korean-English mapping, production output byte/query limits, and automatic identifier expansion policies.

## Constraints
- Consume `apps/codemap-search/experiments/related-terms-poc/results/lookup-handoff.json`, its referenced dataset handoff, and `workspace_manifest_path`. Every observation carries dataset identity, normalization/filter policy identity, frozen workspace/input identity, repository generation, and executed command/environment. Do not silently replace the target corpus after observing results.
- Distinguish pipeline completion from semantic success. A valid negative or inconclusive PoC result is reportable completion, not permission to claim useful coverage or proceed automatically to phase 2.
- Do not inherit the older brief's 70%, 200-pair sample, recovery fraction, 20% frequency cutoff, 2048-byte block, or corpus-support numbers as user-approved gates. Record experimental settings as hypotheses and report measured numerators/denominators without inventing acceptance thresholds.
- Freeze the input list, intended meaning, review rubric, sampling seed/population/count, result limits, source-selection version, and filtering policy before the final run. Keep any development calibration sample separate from the final sample, avoiding reversed-pair/same-sense leakage.
- Use original prompt text only if deriving optional diagnostic terms from existing Grafana prompts; do not use its evidence/entrypoint to construct supposedly blind inputs. Grafana is not required for this phase and its managed checkout is never a writable target.
- Source-derived positive controls verify extraction and lookup, not independent domain-wide quality. Label already-researched examples as controls; report independently selected probes separately.
- Preserve distinctions between exact synonym in the stated sense, near-synonym, same-concept alias, abbreviation, related-only, wrong-sense, and uncertain evidence. Do not count every useful or related candidate as a synonym hit.
- Record the labeler's identity/method and original evidence. Agent-authored manual review is not independent human annotation or an implemented LLM-judgment stage. If no defensible semantic label is available, keep it uncertain and do not manufacture a precision claim.
- Report input coverage, inputs with at least one supported synonym, candidate precision by relation/source, alias-only/abbreviation-only returns, ambiguity/wrong-sense rates, and abstention. Keep missing expected expressions separate from incorrect returned ones.
- Report candidates at dictionary lookup, normalization, repository-presence filtering, commonness filtering, deduplication, and final limit stages, with explicit rejection reasons. Empty responses and unavailable statistics are different outcomes.
- Do not claim repository-confirmed phrase occurrence from marginal word counts. Inspect the reader's declared phrase evidence semantics and report their limitations.
- Measure lookup without including hidden source acquisition, parsing, indexing, vocabulary rebuilding, or model calls. Attribute startup/SQLite loading separately, record release/debug profile and sample counts, and label missing memory measurements as unmeasured rather than zero.
- Keep per-repository source paths and vocabulary private in ignored artifacts. Track compact summaries and public-source examples only. Experiment input lists, review tables, and run/evaluate scripts are PoC deliverables, not authorization to add a new production unit-test suite, lint setup, or formatter task.

## Related Files / Entry Points
- `apps/codemap-search/experiments/related-terms-poc/results/lookup-handoff.json` (proposed) — verify child 02's actual commands, artifact identities, readiness proof, and limitations before evaluation.
- `apps/codemap-search/experiments/related-terms-poc/cases.json` (proposed) — freeze lookup inputs, intended meanings, categories, and control/probe distinctions.
- `apps/codemap-search/experiments/related-terms-poc/evaluate.py` (proposed) — aggregate recorded observations and review labels without changing the dictionary or serving policy.
- `apps/codemap-search/experiments/related-terms-poc/results/` (proposed) — store compact protocol, review, run manifest, and aggregate evidence.
- `apps/codemap-search/experiments/related-terms-poc/README.ko.md` (proposed) — append findings and reproduction instructions without rewriting the previous children's contract/build commands.
- `apps/codemap-search/experiments/event-navigation-poc/evaluate.py` — inspect the separation between recorded execution results and evaluation, without importing its callback-specific metrics.
- `apps/codemap-search/experiments/event-navigation-poc/README.ko.md` — preserve explicit measurement scope, denominators, remaining gaps, and non-generalization language.
- `benchmark/v2/data/dataset.json` — optional read-only prompt context, not the synonym gold set or primary acceptance route.

## Execution Plan
### Stage 1 — 평가 입력과 판정 기준 고정
- Starts when: `apps/codemap-search/experiments/related-terms-poc/results/lookup-handoff.json` exists, its technical status passes, and every referenced local dataset/index/statistics artifact matches its identity.
- Work: Freeze the evaluation population, inputs/meanings, diagnostic controls, review rubric, sample protocol, and measurement environment. Enumerate the serving entry points whose offline/index-time boundary will be checked.
- No-op when: A complete frozen evaluation and report already refer to the exact current dataset, reader, policies, and repository generation and satisfy every criterion of this brief on reinspection.
- No-op handoff: The parent receives `apps/codemap-search/experiments/related-terms-poc/results/poc-report.json` with verified report/evidence references. Failed proof stops global completion, returns to the responsible child for bounded correction/re-verification, and recalculates affected handoffs before resuming.
- Deliverable: `apps/codemap-search/experiments/related-terms-poc/cases.json` and `results/evaluation-protocol.json`, recording input identity, categories, control/probe flags, rubric, sample seed/population/count, frozen limits/policies, and measurement conditions.
- Verify: `Inspect the frozen evaluation population and protocol against the lookup contract and original source evidence`; Inputs: cases.json, evaluation-protocol.json, lookup-handoff.json, and the cited semantic records; Expected: non-zero input coverage across declared categories, no reference-path leakage, separate controls/probes, and no policy left to tune after results are seen.
- Ends when:
  - [ ] Every case has a stated intent/category and all final settings are frozen with hashes.
  - [ ] The review distinguishes source-supported senses from aliases/abbreviations/associations and names its uncertainty handling.
- Handoff: Stage 2 receives the frozen protocol and existing local executable/artifacts.
- Replan when: Required semantic labels cannot be defended or the technical handoff fails. Narrow the claim and mark uncertainty, or return the technical failure to child 01/02 before executing dependent measurements.
- Worker decision: A feasible sample size and input mix, fixed before execution with coverage rationale and uncertainty reported; this does not set a product go/no-go percentage.

### Stage 2 — 실제 반환과 비용 측정
- Starts when: Stage 1 inputs and policies are frozen and the isolated target index/statistics are ready.
- Work: Run actual local lookups, collect every pre/post-filter candidate with provenance, perform source-grounded review, and measure costs in documented cold/warm conditions. Demonstrate a source-supported synonym return such as the digital delete/erase sense if admitted by the frozen build, without fabricating a fallback mapping.
- Deliverable: Recorded per-input observations, `results/sample-review.tsv`, and `results/measurements.json`, with larger/raw traces under ignored `.codemap/poc/related-terms/` and exact references in the run manifest.
- Verify: `Reconcile all frozen input IDs and reviewed candidate IDs with the recorded lookup outputs`; Inputs: cases.json, run manifest, complete observations, sample-review.tsv, and measurements.json; Expected: one accounted outcome per input, traceable semantic evidence, reproducible denominators, and no missing/error/empty result silently omitted.
- Ends when:
  - [ ] Actual output demonstrates supported relations or records their absence as a failed semantic finding, with aliases and related-only output excluded from synonym counts.
  - [ ] The final sample is not used to alter source selection, normalization, scoring, or filters; tuning evidence is kept separate.
  - [ ] Repeated offline lookup evidence confirms no download, source traversal, postings scan, or vocabulary recomputation in the serving path.
  - [ ] Costs are attributed to source acquisition, dataset build, repository indexing/finalization, startup, and lookup rather than mixed into a single unexplained number.
- Handoff: Stage 3 receives complete measured outcomes and review records.
- Replan when: An implementation defect invalidates evidence. Stop this run, return the defect to child 01/02, preserve the invalidated run, and resume under new artifact IDs and a documented protocol revision rather than quietly overwriting results.

### Stage 3 — 결과와 후속 진입 조건 보고
- Starts when: All frozen inputs and required measurements have an accounted outcome.
- Work: Write a Korean report and machine summary that separate construction, serving correctness, semantic findings, and unknowns. Recommend whether candidate evidence is sufficient to investigate LLM judgment, and identify the conditions that a later integration experiment must establish.
- Deliverable: `apps/codemap-search/experiments/related-terms-poc/results/poc-report.json`, containing dataset/lookup handoff references, protocol hash, input and reviewed-pair denominators, relation/source metrics, filter attrition, costs, failure examples, `technical_status`, `semantic_findings`, `next_phase_recommendation`, `unverified_limits`, and `readme_path`; final `README.ko.md` accompanies it.
- Verify: `Recalculate the report's numerators and denominators from the saved observations and review labels`; Inputs: poc-report.json, protocol, cases, complete observations, review table, and README.ko.md; Expected: matching totals, explicit negative/inconclusive outcomes, accurate commands, and no claim of LLM or product-integration validation.
- Ends when:
  - [ ] Every metric names its population and uncertainty, and every failure class has an observed example or is marked unobserved.
  - [ ] The report distinguishes an implemented pipeline from evidence of useful synonym returns; a no-go finding is not relabeled as a quality pass.
  - [ ] Phase 2 is defined as later contextual judgment over preserved evidence, not a substitute for missing provenance or broken counts.
  - [ ] Phase 3 remains a later experiment covering real tool exposure, retrieval/adoption effects, and GitHub latest-dataset-release/global-download delivery; no provisional tool or asset names are asserted as existing contracts.
- Handoff: The parent receives `apps/codemap-search/experiments/related-terms-poc/results/poc-report.json` and evaluates phase-1 completion and the evidence-based follow-up recommendation.
- Replan when: Aggregation or report claims disagree with saved evidence. Correct this child's reporting and re-verify; if underlying serving/data evidence is invalid, stop global completion and return to the owning child before updating the parent.

## Side Effect Checkpoints
- [ ] `git status --short` and `git diff --stat` show only experiment changes during execution, with product files, benchmark data/managed checkout, user/global configuration, and the historical brief preserved.
- [ ] The measured dataset and lookup-policy hashes match their handoffs and have not been silently retuned by evaluation.
- [ ] No API keys, private source content, or private repository paths are included in tracked results or attribution.
- [ ] No model-based judging service, GitHub publication, downloader, or production MCP/CLI/schema change was introduced.

## Acceptance Criteria
- [ ] The final report is reproducible from frozen cases, local artifact identities, saved observations, and disclosed review labels, with no omitted input outcomes or hidden policy changes.
- [ ] The report answers separately whether data can be built, what actual synonyms/aliases/abbreviations are returned, why candidates are lost, and whether repository statistics are genuinely index-time work.
- [ ] Non-empty output is never treated alone as synonym success; supported semantic returns and wrong-sense/uncertain/related-only outputs are reported separately with denominators and provenance.
- [ ] Actual timing/size/memory evidence and unmeasured limits are distinguished, and no Grafana solving, real LLM judgment, token savings, or product integration is claimed as verified.
- [ ] The parent can use poc-report.json and README.ko.md to decide the next phase from evidence, including a negative or inconclusive result, without reconstructing the experiment.

## Open Questions
- None — this child reports measured phase-1 findings rather than inventing numerical promotion thresholds. The user will decide whether and how to proceed with LLM judgment from the completed report.
