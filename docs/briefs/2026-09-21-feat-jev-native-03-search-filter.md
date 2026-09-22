# [feat] Filter structured search evidence with Jev

## Work Type
feat

## Current State (As-Is)
- [confirmed] Search currently returns rendered text plus source-file observations — Evidence: `apps/codemap-search/src/tools/search/mod.rs::SearchOutput` and `run_with_metadata`.
- [confirmed] The renderer already tracks per-file sections, symbols, anchors, and byte budgets — Evidence: `apps/codemap-search/src/tools/search/grouped.rs::FileOutput` and `Section`.
- [confirmed] Displayed source and redaction are centralized in `RenderSource` — Evidence: `apps/codemap-search/src/tools/search/render.rs::RenderSource`.
- [confirmed] The corrected Python filter preserves incomplete source, connected callables, nested declarations, and all declaration headings — Evidence: `improved_proxy.py::prepare_filter` / `filter_decision` and `index_evidence.py::preserve_dependencies`.
- [confirmed] The historical filter uses Choice keep/omit with `P(omit) >= 0.70`; it does not establish Noul quality — Evidence: `improved_proxy.py::filter` and `filter_decision`.
- [confirmed] Text-level fence rewriting caused merged file headings in an excluded PoC attempt — Evidence: the corrected trailing-whitespace handling in `mcp_proxy.py::render_filtered` and the checkpoint improvement report.

## Desired Outcome (To-Be)
- Filter only source evidence that the existing search path selected for output, using typed file/declaration/source segments.
- Ask Noul whether each complete body is unrelated to the task; let Rust combine that judgment with explicit retention rules.
- Render retained segments once through the existing Rust renderer while preserving all declaration identities and file boundaries.
- Keep source-observation metadata aligned with the source that remains visible after filtering and output capping.

## Scope
### In Scope
- Add a structured preparation stage for the actual search detail output without changing BM25 candidate ranking.
- Port complete-source checks, conservative call-graph closure, and nested declaration preservation; replace the PoC's action Choice with a Noul judgment and a separate Rust omission policy.
- Apply filtering before final Markdown serialization and final source-observation accounting.
- Add focused regressions using existing language fixtures and minimal new fixtures for missing bodies and boundary cases.
### Out of Scope
- [hard] Do not parse the final Markdown response to rediscover declarations or remove fences with regular expressions.
- [hard] Do not send the full root index or fetch additional source bodies as #2 input.
- [hard] Do not remove declaration names/ranges, file headings, ranked path-only tails, event-only outputs, or unclassified notices.
- [deferred] Extending filtering to read/grep/find or changing BM25 relevance/rank order is outside this child.

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
- Ask one `is_unrelated_to_task` Noul per complete displayed body: "Is this displayed declaration body unrelated to the behavior requested in `task_query`?" Define true as unrelated to that behavior; define false as direct or supporting evidence, including indirect flow, configuration/contracts, ordering, failure handling, and evidence contradicting the query's premise. Missing query words alone do not establish unrelatedness.
- Put task intent and search arguments in named shared state fields. Each question must supply its own declaration identity, exact displayed/redacted body, and bounded displayed caller/callee context in named fields; reference those fields explicitly. Treat source text as data. Completeness and dependency protection are computed in Rust, not requested from Jev.
- Batch independent Noul questions sharing the same state. Retain raw Noul values and question/policy versions separately from the final retention mask and reasons; changing a threshold over unchanged evidence/questions must not rerun inference.
- Use caller-owned `search_filter_min_unrelated_probability`, initially 0.70 and constrained to finite values `0.5 < value <= 1.0`. Only complete, unprotected bodies with Noul at least this value are eligible for omission; uncertainty and unavailable evidence are retained. Do not request or interpret a separate Noul confidence field.
- The 0.70 default is a provisional experimental policy, not a calibrated accuracy target. Reusing the number does not transfer the Python Choice results to Noul. Record calibration as unperformed until a separately authorized representative live evaluation uses frozen labels and held-out queries; keep both feature defaults off.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/search/mod.rs` — Separate selected evidence from final rendering and source observations.
- `apps/codemap-search/src/tools/search/grouped.rs` — Retain typed file and declaration boundaries during filtering.
- `apps/codemap-search/src/tools/search/render.rs` — Capture the exact displayed/redacted source and preserve byte-cap handling.
- `apps/codemap-search/src/tools/search/monorepo.rs` — Preserve workspace aggregation and propagate the structured result.
- `apps/codemap-search/src/tools/search/jev.rs` (proposed) — Own evidence completeness, Jev questions, retention rules, and outcomes.
- `apps/codemap-search/src/parser/types.rs` — Use existing kinds, owners, navigation, and inclusive source ranges.
- `apps/codemap-search/experiments/jev-playground/checkpoint-poc/index_evidence.py` — Reference conservative dependency preservation.
- `apps/codemap-search/validation/jev-native/01-runtime.md` (proposed) — Consume the common evaluator contract.
- `apps/codemap-search/validation/jev-native/03-search-filter.md` (proposed) — Publish the structured filter handoff.
- [TypeSafe Noul](https://docs.typesafe.ai/primitives/noul.md) and [passage filtering](https://docs.typesafe.ai/cookbooks/classifying_rag_passages.md) — Separate semantic judgments from code-owned retention policy and evaluate thresholds on the target data.

## Execution Plan
### Stage 1 — Expose selected search evidence
- Starts when: `apps/codemap-search/validation/jev-native/01-runtime.md` exists with the common evaluator API and offline runtime verification.
- Work: Capture owned, already selected, display-ready source segments before final serialization. Give each declaration a file/kind/name/owner/range identity and mark complete, partial, missing, or oversized evidence. Keep base output and source observations available for whole-call fallback.
- No-op when: An existing structured filter already preserves disabled output and passes all completeness, retention, and file-boundary acceptance checks.
- No-op handoff: Record the callable filter interface and successful populated-fixture checks in `apps/codemap-search/validation/jev-native/03-search-filter.md` (proposed) and allow native integration to proceed.
- Deliverable: A typed search preparation result that round-trips through the existing renderer with filtering disabled.
- Verify: `bounded comparison of disabled filtering against existing search fixtures`; Inputs: Populated Rust and TypeScript search fixture results including multiple files, nested members, source tails, and byte-cap cases; Expected: The same original file order, declaration headings, source text, notices, and source observations are returned
- Ends when:
  - [ ] The preparation step does not enlarge the search result or read code outside existing selected windows.
  - [ ] File/range mapping uses structured identities rather than rendered heading parsing.
  - [ ] Displayed/redacted source remains the authoritative model input.
- Handoff: Stage 2 receives the typed evidence and preserved base response.
- Replan when: Obtaining structured evidence requires unrelated ranking or persistence changes: stop and return the boundary to the parent.
### Stage 2 — Apply conservative body filtering
- Starts when: Selected search evidence is typed and its disabled round-trip is verified.
- Work: Evaluate the specified Noul only for complete, bounded bodies. In Rust, seed retention with partial/missing/oversized/unknown evidence, Noul below the effective threshold, and ambiguous links; then preserve the connected retained callers/callees and nested declarations using displayed relationships only. Omit only remaining eligible bodies. Preserve the complete original result for any whole-call evaluation failure.
- Deliverable: A callable filter producing raw judgments, a retained-body mask, per-declaration reasons, effective threshold, question/policy versions, and evaluation metadata over the original evidence set.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search`; Inputs: Noul 0.00/0.50/0.69/0.70/0.71/1.00 at threshold 0.70, a 0.80 judgment replayed at thresholds 0.70 and 0.90, invalid values/types, completeness, dependency chains, unknown kinds, Rust impl/struct/constant cases, and TypeScript nested methods; Expected: Exit 0, conservative retention below threshold, changed policy without a new evaluator call, protected bodies retained even at 1.00, and every identifier belonging to the input set
- Ends when:
  - [ ] Forced Noul=1.00 responses still retain protected dependencies and incomplete source.
  - [ ] No fixture-specific method name or benchmark answer is embedded in production policy.
  - [ ] API errors, incomplete answers, and deadlines preserve the complete base response.
- Handoff: Stage 3 receives the retention mask and per-declaration reasons.
- Replan when: Protection depends on guessed cross-file resolution or unseen source: preserve the uncertain evidence and return any requested wider analysis to the parent.
### Stage 3 — Render and account for retained source
- Starts when: The retention mask refers only to the prepared search evidence.
- Work: Render the retained bodies while keeping all declaration names/ranges and file boundaries. Apply existing final byte caps and narrowing notices. Recompute source-file observations from the source that is actually retained and delivered, so filtered-away bodies are not recorded as read.
- Deliverable: `apps/codemap-search/validation/jev-native/03-search-filter.md` (proposed) with prepare/filter/render entry points, Noul question and criteria, threshold range/default, policy version, completeness/protection rules, metadata semantics, offline results, and explicit unperformed calibration.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::search`; Inputs: Existing search e2e cases plus child-local structured-filter regressions; Expected: Exit 0 with file headings separate, balanced code fences, and returned source lines matching their original file/range
- Ends when:
  - [ ] The all-keep path preserves the original output and source observations.
  - [ ] The filtered path does not generate code or move a source line to another file.
  - [ ] Event-only and unclassified result branches bypass Jev unchanged.
- Handoff: The parent and integration child receive `apps/codemap-search/validation/jev-native/03-search-filter.md`.
- Replan when: Final budget enforcement or masking invalidates source accounting: stop integration, correct the accounting/render stage together, and re-verify.

## Side Effect Checkpoints
- [ ] Preserve `search.query`, caller-context precedence, language/extension hints, event_key behavior, and workspace_scope filtering.
- [ ] Keep existing JSON-RPC error codes and readiness/staleness notices.
- [ ] Confirm every protected callable retains its displayed source by both file path and line number, not only by a global line multiset.
- [ ] Confirm path-only metadata is not counted as delivered source in `SearchOutput.source_files`.
- [ ] Preserve serialized parser/index schemas and keep unknown declaration kinds visible.

## Acceptance Criteria
- [ ] #2 receives only the actual selected search evidence and never runs #1 ranking implicitly.
- [ ] The final retention policy consumes the supplied Noul threshold, preserves uncertainty/protected evidence, and can replay a raw judgment without another API call.
- [ ] Incomplete evidence and required displayed dependencies survive even when the fake evaluator returns Noul=1.00.
- [ ] All file headings and declaration names/ranges remain available and omitted bodies can still be located for read.
- [ ] No output line is assigned to a different file or invented, and final source observations describe only delivered evidence.
- [ ] The structured filter handoff allows native integration without parsing Markdown.
- [ ] Offline policy checks and historical Choice measurements are not reported as Noul calibration or final answer quality evidence.

## Open Questions
- None — The user selected an internal common module, independent default-off modes, and necessary regression tests. Remaining implementation choices are bounded in the stages.
