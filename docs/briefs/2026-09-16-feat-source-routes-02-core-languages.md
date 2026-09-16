# [feat] 핵심 네 언어 분석기 이식

## Work Type
feat

## Current State (As-Is)
- [confirmed] The bullets below describe the `a3fe03468` input baseline. The parent checklist and completed handoff JSON own implementation status; retain this child as executable instructions.
- [confirmed] `apps/codemap-search/experiments/event-navigation-poc/engine.py` and its Rust/TypeScript helpers implement JavaScript, TypeScript, Go and Rust source analysis.
- [confirmed] The latest Rust PoC keeps source spans, macro/call contexts, wildcard import scope, generic/nominal type separation and actual return locations; see `rust_modules.py`, `rust_types.py`, `rust_values.py` and the 301-case manifest.
- [confirmed] The pinned public results in `language-corpus/remaining-routes/README.ko.md` leave three Bevy callback routes unresolved and separate Effect data consumers from callbacks.

## Desired Outcome (To-Be)
- JavaScript, TypeScript, Go and Rust use the shared native source-route contract with the implemented PoC capabilities and guards.
- Existing repository APIs are resolved from their source bodies and imports rather than a product framework catalog.

## Scope
### In Scope
- Port imports, namespaces, exports/re-exports, callable/closure captures, factories, actual/formal arguments, object fields, collection keys, loops and return provenance.
- For JavaScript/TypeScript preserve prototype/receiver context, live bindings, call/apply/arguments/rest, native generator creation versus explicit next/yield/yield* and completion, Array pop, source-backed wrappers and type-owner scopes. Preserve the unsupported iterator/control-flow guards.
- For Go preserve receiver ownership, interface/callback fields, slices/maps/channels and package/source contexts represented by the corpus without inferring execution or platform applicability.
- For Rust preserve module/re-export scope, Self/Deref, Option/Result extraction and take/clear/reinsert, reference-derived Box/NonNull/raw-pointer origins, tuple/TypeId key distinctions, generic substitution and user-standard-name shadowing.
- Port the bounded source macro subset with original spans and distinct expansion identities, block versus function return semantics, branch-tail evidence and safe retry constraints. Do not execute proc-macros or compiler-oracle files.
- Keep unresolved-call arguments (including the ten-argument regression), partial returns, generic metadata, unresolved glob imports, opaque initializers and external mutation boundaries.
- Preserve parser recovery as an explicit conditional source-only mechanism with unchanged source bytes/offsets. Reject recovery that changes executable bodies.

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
- `apps/codemap-search/src/events/source_routes/` — primary implementation route; reuse the shared model and inspect the allocated adapter modules before edits. Legacy flow files below describe the baseline boundary, not a request to restore its evaluator.
- `apps/codemap-search/experiments/event-navigation-poc/engine.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/syntax.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/rust_values.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/rust_types.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/rust_modules.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/rust_macros.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/ts_modules.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/ts_syntax.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/js_generators.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/src/flow/extract.rs` — inspect the named source contract or allocated evidence before edits.
- `docs/briefs/evidence/source-routes/coverage.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/src/events/source_routes/engine.rs` — native implementation or validation entry point; select a nearby equivalent module if existing code supports safe reuse, and record the actual path in the handoff.
- `docs/briefs/evidence/source-routes/core.json` — child evidence/handoff JSON, with status, baseline/current revision, implementation paths, case IDs and results, source hashes, limits, command/cwd/exit records and unresolved reasons.

## Execution Plan
### Stage 1 — Establish the native boundary
- Starts when: `docs/briefs/evidence/source-routes/model.json` exists with status=complete, implementation paths, command exits and source/case hashes; read its contract and `docs/briefs/evidence/source-routes/coverage.json`.
- Work: Read the core-owned ledger rows and their exact fixtures, then map PoC extraction and summary operations onto the native model. Execute representative failing native assertions before expanding adapters; retain all case IDs for the full gate.
- No-op when: The complete desired behavior already exists and all allocated acceptance checks pass on the current native product, with no missing or reclassified ledger rows.
- No-op handoff: Write proof and exact checks to `docs/briefs/evidence/source-routes/core.json` with status=complete and outcome=no-change. The next child continues from that file. If proof fails, record status=incomplete, perform Stage 2 bounded correction and re-verification, and have the main author recalculate affected parent handoffs before continuing.
- Deliverable: A source-to-native mapping, baseline checks and case allocation in `docs/briefs/evidence/source-routes/core.json`, initially status=incomplete unless the complete no-op proof succeeds.
- Verify: `inspect docs/briefs/evidence/source-routes/core.json`; Inputs: the exact entry-point files and every ledger row allocated to this child; Expected: nonempty input inventory, named implementation boundary, preserved expected outcomes and a recorded command or bounded-inspection result for each claimed baseline.
- Ends when:
  - [ ] The mapping covers this child's full scope and distinguishes existing behavior from missing native behavior.
  - [ ] No user-owned choice or unbounded prerequisite is hidden in the route.
- Handoff: Stage 2 receives the mapping and unmodified source/case inputs in `docs/briefs/evidence/source-routes/core.json`; the no-op route hands the same complete-format file directly to the successor.
- Replan when: Required source evidence is unavailable, a shared model cannot represent an allocated case, or a proposed solution needs a compiler/runtime or framework catalog. The main author pauses only the dependent route, records the failed proof, revises the model/owner handoff and resumes after bounded verification; ask the user only if an actual scope or compatibility decision is required.

### Stage 2 — Implement and prove the allocated behavior
- Starts when: Stage 1 has a complete mapping and recorded baseline or failure evidence in `docs/briefs/evidence/source-routes/core.json`.
- Work: Port source-backed core semantics in cohesive modules, preserving every allocated regression/control condition. Resolve failures in the main thread from actual facts and source spans. Run each core-owned fixture/regression row through the native relation API and record exact statuses, reasons and input hashes.
- Deliverable: Native source changes and `docs/briefs/evidence/source-routes/core.json` containing the minimum handoff fields above, actual command outcomes and the full allocated case result set.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib events::source_routes::tests::test_native_corpus -- --exact --nocapture`; Inputs: the runner options specified in Constraints plus the changed native modules and this child's corpus/side-effect checkpoints; Expected: exit 0 with native core-language tests and an exact case-ID report for all allocated rows.
- Ends when:
  - [ ] Every allocated acceptance criterion and side-effect checkpoint is backed by an actual native result or an explicit pre-authorized deferral.
  - [ ] The handoff has status=complete only after its observable acceptance checks pass.
- Handoff: The next parent wave consumes `docs/briefs/evidence/source-routes/core.json`; the final wave hands it to the parent for whole-work completion. Do not treat PoC results as native results.
- Replan when: Any native case or existing product contract fails. The main author records exact input and actual output, corrects the owning native layer, reruns affected checks and updates dependent handoffs. Do not weaken expected outcomes or mark completion with unexplained gaps.
- Worker decision: Choose the smallest reusable Rust module boundary and internal representation compatible with the shared proof contract. Reversible placement and bounded data-structure choices do not require another user approval.

## Side Effect Checkpoints
- [ ] A same name, same declared type or unbound generic cannot unify different instances or storage slots.
- [ ] Creating a generator, function, factory result or callback argument does not count as executing it.
- [ ] Source parser/macro identities preserve real file/line coordinates without merging different allocations at one mapped span.
- [ ] The Bevy payload return remains a data return and all three deferred callback paths stay unresolved.

## Acceptance Criteria
- [ ] In a separate frontend result group, preserve all five checks from `apps/codemap-search/experiments/event-navigation-poc/language-corpus/regressions/verify_rust_frontend.py` on `regressions/examples/rust/source-frontier/routes.rs`: all_ten_operands_preserved, nested_calls_same_start_distinct_results, impl_and_function_type_variables_have_distinct_scopes, source_call_contexts_remain_distinct, generic_formal_types_do_not_leak_between_calls. Record each check ID, source hash, inspected native value/call/type data and pass/fail.
- [ ] Replay the locked bevy-modules source selection and port the observational assertions in `remaining-routes/verify_bevy_frontier.py`: Some(system) retains a Box allocation inside the Option wrapper, self.spawn(RegisteredSystem::new(system)).id() retains the allocation-rooted entity field, and the observer impl variable remains nonconcrete. Record these three check IDs and values without adding production API-name rules or claiming the three deferred final callbacks.
- [ ] All core-owned fixture and regression rows preserve their original expected relation kinds, controls, required conditions and unresolved guards in native execution.
- [ ] Pinned public source variants use the exact locked bytes and classify capped/missing inputs separately.
- [ ] The handoff names implementation files, command results, per-case native outcomes and every remaining bounded unsupported construct.

## Open Questions
- None — the user has fixed language scope, compiler independence, deferred Bevy boundaries and execution ownership; remaining implementation choices belong to the main author.
