# [test] 제품 분석과 조회 동작 검증

## Work Type
test

## Current State (As-Is)
- [confirmed] The bullets below describe the `a3fe03468` input baseline. The parent checklist and completed handoff JSON own implementation status; retain this child as executable instructions.
- [confirmed] `apps/codemap-search/experiments/event-navigation-poc/language-corpus/remaining-routes/verification.json` records PoC-only 301/301 regressions and source-result hashes, not native product verification.
- [confirmed] The combined public PoC baseline is 31 callbacks, 1 data return, 2 argument-only and 3 unresolved out of 37 positives, with 14 disconnected public pairs.
- [confirmed] Ninety common fixture cases and six additional Effect consumer pairs are distinct from the public 37; 26 public syntax scenarios have not been runtime-executed.

## Desired Outcome (To-Be)
- A reproducible native validation report distinguishes product-proven behavior, static PoC comparison, excluded/capped input and deferred limitations.
- Product readiness is based on actual native execution and tool output rather than the Python result counts.

## Scope
### In Scope
- Execute the native analyzer against all 301 regression rows and 90 common fixture rows, preserving original IDs, positive controls, source hashes, exact endpoints, relation types and required conditions.
- Replay the 37 public positives and 14 public negatives on the pinned sources. Keep the 26 syntax scenarios labeled unexecuted source examples rather than treating them as runtime tests.
- Replay all six expanded variants, including Monix JVM/JS Scala 2/3, and the separate six Effect consumer pairs. Permit 1 MiB only in the explicit validation input path and verify production still diagnoses files above 512 KiB.
- Check missing source, parse error, unknown/mutated key, separate instance/type/namespace, alias, standard-name shadowing, generator creation, unsupported control flow, budget exhaustion and false-positive controls.
- Verify production output/options, index refresh, stale dependencies, delete/restart behavior, test/file filtering, deterministic caps and disabled-event compatibility.
- Record observed cold readiness and warm tool-call elapsed time, corpus/input sizes and actual resource bounds with the baseline revision and native implementation hash attached. Distinguish IPC/client delays from analyzer-only timing. Do not claim a before/after latency improvement without a matching baseline measurement or invent unrequested latency thresholds.
- Preserve immutable PoC evidence and distinguish native pass counts from PoC counts; record any unsupported row as a remaining gap and correct in the owning child rather than marking the set complete.

### Out of Scope
- [hard] Production dependencies on compiler auxiliary data, target compilation, Python, network fetches or external runtime services.
- [hard] Sub-agent execution except the single parent-level cold-pickup exception above.
- [hard] Shell, documentation, markup and infrastructure languages as additions to the 18-language source-route feature.
- [hard] Implicit dependency-directory unignore, out-of-root access, new event-classification claims, or reintroduction of `Value relationships`.
- [deferred] Bevy Observer, SystemId and glTF registration-to-callback routes through generic ECS physical storage and trait dispatch. Preserve their unresolved status and the existing front-end/payload improvements; completing these three routes is not a release gate.
- [deferred] Arbitrary generators, fiber continuations, proc-macro execution, full Rust type/trait solving, Queue end-to-end payload delivery and runtime delivery/order/concurrency proofs.

## Constraints
- **Execute all analysis, planning, implementation, validation and corrections in the main thread. Never use sub-agents for this work. The sole exception is one fresh `gpt-6-astra` agent at `max` reasoning for one cold-pickup review of this entire briefset. No other sub-agents, child agents or delegated work are allowed. That reviewer is read-only and must not delegate.**
- Do not use codemap-search for this investigation. Use bounded source reads and `rg`. Work from the repository root for the commands below.
- Keep the production implementation native Rust and self-contained. Do not launch Python, a language server, a target compiler, build scripts, macros or target applications at index or query time. Existing Python PoC scripts may be used only as development comparison tools.
- Use source-derived ownership, bindings, keys, storage and call evidence. Do not hardcode repository names or framework/API catalogs to manufacture source relations. Existing configured EventRule behavior remains supported and separate.
- Every emitted source relation must retain `certainty=conditional_source_relation`, `concrete_instance_proven=false`, `event_classification=not_inferred`, conditions and exact source evidence. Keep callback invocation, argument passing, data return, object write/read and lookup-key consumption distinct.
- Use the original corpus manifests and the coverage ledger as immutable comparison inputs. Do not change an expected positive to unknown, remove a negative, or count a data relation as a callback to make validation pass.
- Preserve the existing 512 KiB/file, 64 MiB/4096-file snapshot and bounded query/input/output limits. The prior 1 MiB exception applies only to the expanded-input validation invocation, not production defaults. Record excluded or budget-limited inputs explicitly and never count them as a successful native route.
- Do not restore the removed `Value relationships` output or its request-time evaluator. Preserve `include_events=false`, disabled event configuration, source-first output and current MCP/CLI schemas.
- Run package Rust checks after relevant edits. Announce each verification command and its working directory. Existing case/test work is authorized by the user; do not add unrelated test, lint or formatting setups.
- Keep this child cohesive around its adapter or integration boundary. Main-thread execution is sequential; do not split into agent tasks or nested briefsets.

- Native corpus runner: after 01 Stage 1 creates `apps/codemap-search/src/events/source_routes/tests.rs`, execute `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib events::source_routes::tests::test_native_corpus -- --exact --nocapture` from the repository root. Require one executed test and a nonempty complete selected-case report, not merely cargo compilation or zero matching tests.
- Runner inputs: set `CODEMAP_SOURCE_ROUTES_LANGUAGES=all` and `CODEMAP_SOURCE_ROUTES_GROUPS=regressions,fixtures` for the consolidated adapter gate; select this child's allocated language rows in its handoff. Run public, expanded, consumers and frontend as separate single-group invocations as allocated; only regressions and fixtures may share an invocation. Set `CODEMAP_SOURCE_ROUTES_SOURCE_ROOT` to the checkout prepared from the immutable input lock and `CODEMAP_SOURCE_ROUTES_REPORT` to `docs/briefs/evidence/source-routes/native-corpus.json`. The report contains case/check IDs, expected and actual kinds, exact values/spans/conditions, notices, source hashes, native implementation hash and pass/fail. Missing input, empty selection, unmatched positive controls or unsupported runner options fail.
- Runner size: omit `CODEMAP_SOURCE_ROUTES_MAX_FILE_BYTES` for the 524288-byte production boundary. Set it to 1048576 only in the expanded validation invocation, write a distinct expanded report and verify the default-size exclusion separately. The production runtime never reads these test-only variables. Consumers verify the six locked pairs against the matching expanded report via `CODEMAP_SOURCE_ROUTES_EXPANDED_REPORT`, without another oversized-input analysis. Require its native implementation fingerprint and input hashes to match. Reports keep raw analyses in a SHA-256-bound `.analyses.json.gz` sibling. The test-only runner may use git, shasum and gzip; production must not use them.

## Related Files / Entry Points
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/manage.py` — development-only source preparation from the existing lock; production never invokes it.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/remaining-routes/prepare.py` — development-only preparation of the six locked expanded variants.
- `docs/briefs/evidence/source-routes/commands.json` — exact final native/compatibility commands, input-root options and successful exits.
- `apps/codemap-search/src/events/source_routes/` — primary implementation route; reuse the shared model and inspect the allocated adapter modules before edits. Legacy flow files below describe the baseline boundary, not a request to restore its evaluator.
- `docs/briefs/evidence/source-routes/coverage.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/regressions/cases.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/fixture-cases.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/cases.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/remaining-routes/inputs.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/remaining-routes/consumers.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/tests/e2e/tools.rs` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/tests/e2e_tests.rs` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/src/events/source_routes/tests.rs` — native implementation or validation entry point; select a nearby equivalent module if existing code supports safe reuse, and record the actual path in the handoff.
- `docs/briefs/evidence/source-routes/verification.json` — child evidence/handoff JSON, with status, baseline/current revision, implementation paths, case IDs and results, source hashes, limits, command/cwd/exit records and unresolved reasons.

## Execution Plan
### Stage 1 — Establish the native boundary
- Starts when: `docs/briefs/evidence/source-routes/integration.json` exists with status=complete, implementation paths, command exits and source/case hashes; read its contract and `docs/briefs/evidence/source-routes/coverage.json`.
- Work: Check that every ledger case has a native owner and machine-readable result. Confirm locked source availability and input bytes before running the complete gate. Pin the same target workload for baseline/current timing.
- No-op when: The complete desired behavior already exists and all allocated acceptance checks pass on the current native product, with no missing or reclassified ledger rows.
- No-op handoff: Write proof and exact checks to `docs/briefs/evidence/source-routes/verification.json` with status=complete and outcome=no-change. The next child continues from that file. If proof fails, record status=incomplete, perform Stage 2 bounded correction and re-verification, and have the main author recalculate affected parent handoffs before continuing.
- Deliverable: A source-to-native mapping, baseline checks and case allocation in `docs/briefs/evidence/source-routes/verification.json`, initially status=incomplete unless the complete no-op proof succeeds.
- Verify: `inspect docs/briefs/evidence/source-routes/verification.json`; Inputs: the exact entry-point files and every ledger row allocated to this child; Expected: nonempty input inventory, named implementation boundary, preserved expected outcomes and a recorded command or bounded-inspection result for each claimed baseline.
- Ends when:
  - [ ] The mapping covers this child's full scope and distinguishes existing behavior from missing native behavior.
  - [ ] No user-owned choice or unbounded prerequisite is hidden in the route.
- Handoff: Stage 2 receives the mapping and unmodified source/case inputs in `docs/briefs/evidence/source-routes/verification.json`; the no-op route hands the same complete-format file directly to the successor.
- Replan when: Required source evidence is unavailable, a shared model cannot represent an allocated case, or a proposed solution needs a compiler/runtime or framework catalog. The main author pauses only the dependent route, records the failed proof, revises the model/owner handoff and resumes after bounded verification; ask the user only if an actual scope or compatibility decision is required.

### Stage 2 — Implement and prove the allocated behavior
- Starts when: Stage 1 has a complete mapping and recorded baseline or failure evidence in `docs/briefs/evidence/source-routes/verification.json`.
- Work: Run the complete native corpus gate and live product compatibility tests. For a mismatch, preserve the failure record, return to the owning implementation child, correct the source semantics in the main thread, and rerun affected checks before the full acceptance gate. Write per-case results, command exits, hashes, timings, skipped inputs and remaining limitations.
- Deliverable: Native source changes and `docs/briefs/evidence/source-routes/verification.json` containing the minimum handoff fields above, actual command outcomes and the full allocated case result set.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib events::source_routes::tests::test_native_corpus -- --exact --nocapture`; Inputs: all languages and all six selected corpus groups through separate default/expanded invocations as specified in Constraints plus the changed native modules and this child's corpus/side-effect checkpoints; Expected: one executed test per invocation, exit 0, native corpus result rows for the complete inventory, and product integration check exits with no unexplained gaps.
- Ends when:
  - [ ] Every allocated acceptance criterion and side-effect checkpoint is backed by an actual native result or an explicit pre-authorized deferral.
  - [ ] The handoff has status=complete only after its observable acceptance checks pass.
- Handoff: The next parent wave consumes `docs/briefs/evidence/source-routes/verification.json`; the final wave hands it to the parent for whole-work completion. Do not treat PoC results as native results.
- Replan when: Any native case or existing product contract fails. The main author records exact input and actual output, corrects the owning native layer, reruns affected checks and updates dependent handoffs. Do not weaken expected outcomes or mark completion with unexplained gaps.
- Worker decision: Choose the smallest reusable Rust module boundary and internal representation compatible with the shared proof contract. Reversible placement and bounded data-structure choices do not require another user approval.

## Side Effect Checkpoints
- [ ] From the repository root, run `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_value_relationships_stay_removed_across_live_views_and_restart` and `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_implementation_rust_target_reload_with_events_disabled`; each must execute its named test and exit 0. Run the event group separately; its filter does not cover these two contracts.
- [ ] Zero false-positive results is meaningful only with the paired positive controls, nonempty input population and required relation kinds verified.
- [ ] A missing dependency, parser failure or exhausted budget cannot be labeled a disconnected negative success.
- [ ] The 1 MiB validation exception never changes production defaults.
- [ ] Compiler-oracle 13/13 and source parse success are not counted as callback/runtime delivery success.

## Acceptance Criteria
- [ ] For scala-state-subscriber in each locked Monix variant, the actual storage value ends in the new `:subscriber` parameter (not merely a copied prior Set element) and reaches the exact onNext endpoint. Every variant requires source_companion_implicit_selection_required; Scala 2 additionally requires source_macro_template_preservation_required and compiler_macro_typing_unproven; JVM additionally requires qualified_jdk_varhandle_access and compare_and_set_success_required. Record exact storage value, endpoint, variant and matched conditions.
- [ ] In a separate frontend result group, preserve all five checks from `apps/codemap-search/experiments/event-navigation-poc/language-corpus/regressions/verify_rust_frontend.py` on `regressions/examples/rust/source-frontier/routes.rs`: all_ten_operands_preserved, nested_calls_same_start_distinct_results, impl_and_function_type_variables_have_distinct_scopes, source_call_contexts_remain_distinct, generic_formal_types_do_not_leak_between_calls. Record each check ID, source hash, inspected native value/call/type data and pass/fail.
- [ ] Replay the locked bevy-modules source selection and port the observational assertions in `remaining-routes/verify_bevy_frontier.py`: Some(system) retains a Box allocation inside the Option wrapper, self.spawn(RegisteredSystem::new(system)).id() retains the allocation-rooted entity field, and the observer impl variable remains nonconcrete. Record these three check IDs and values without adding production API-name rules or claiming the three deferred final callbacks.
- [ ] All 301 regressions and 90 common fixtures pass native expectations without modifying PoC truth data.
- [ ] The pinned public and expanded reports preserve the established relation classifications wherever full allowed source input is present, and explicitly distinguish the three deferred Bevy callbacks and production-cap exclusions.
- [ ] All actual product integration/compatibility checks pass and the report lists exact commands, native counts, source hashes, timings and remaining unverified runtime behavior.
- [ ] No child is marked complete while an allocated nondeferred parity or integration gap remains.

## Open Questions
- None — the user has fixed language scope, compiler independence, deferred Bevy boundaries and execution ownership; remaining implementation choices belong to the main author.
