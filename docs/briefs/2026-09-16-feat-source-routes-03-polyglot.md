# [feat] 열세 언어의 공통 분석기 이식

## Work Type
feat

## Current State (As-Is)
- [confirmed] The bullets below describe the `a3fe03468` input baseline. The parent checklist and completed handoff JSON own implementation status; retain this child as executable instructions.
- [confirmed] `apps/codemap-search/experiments/event-navigation-poc/language-corpus/polyglot.py`, `scala_semantics.py` and `jvm_semantics.py` implement the shared additional-language source path.
- [confirmed] `apps/codemap-search/src/flow/mod.rs::supports` already recognizes these thirteen languages, but extraction support alone is not evidence that source routes work.
- [confirmed] `language-corpus/remaining-routes/inputs.json` pins all four Monix JVM/JavaScript and Scala 2/3 source variants.

## Desired Outcome (To-Be)
- Java, Kotlin, Groovy, Scala, C#, C, C++, Swift, Dart, PHP, Python, Ruby and Lua share the native relation model while retaining language-specific ownership and invocation rules.
- All four Monix source variants are checked without compiling or executing their target programs.

## Scope
### In Scope
- Port the existing polyglot adapters and grammar dispatch for all thirteen named languages, including same-owner fields, callback arrays/lists/maps, destructuring, dynamic key conditions and wrapper forwarding.
- Carry C/C++ pointer, struct/namespace/member and function-pointer distinctions; C# delegate/event syntax and receiver ownership; JVM local/member/static/import and closure ownership; Swift/Dart closure/container semantics; PHP/Python/Ruby/Lua indexed and callable values.
- Preserve Scala case-class update/copy and Set addition element provenance, concrete method/source dispatch, namespace/type-key separation, Atomic wrappers and the four source variants.
- Keep one shared matching engine and language adapters near their extraction logic; do not create thirteen unrelated API-name lists.
- Use each language-owned positive and negative pair from the ledger, including the originally missed example now handled by the PoC.

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
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/polyglot.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/scala_semantics.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/jvm_semantics.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/experiments/event-navigation-poc/language-corpus/grammars.py` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/src/flow/extract/syntax.rs` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/src/lang/mod.rs` — inspect the named source contract or allocated evidence before edits.
- `docs/briefs/evidence/source-routes/coverage.json` — inspect the named source contract or allocated evidence before edits.
- `apps/codemap-search/src/events/source_routes/polyglot.rs` — native implementation or validation entry point; select a nearby equivalent module if existing code supports safe reuse, and record the actual path in the handoff.
- `docs/briefs/evidence/source-routes/polyglot.json` — child evidence/handoff JSON, with status, baseline/current revision, implementation paths, case IDs and results, source hashes, limits, command/cwd/exit records and unresolved reasons.

## Execution Plan
### Stage 1 — Establish the native boundary
- Starts when: `docs/briefs/evidence/source-routes/core.json` exists with status=complete, implementation paths, command exits and source/case hashes; read its contract and `docs/briefs/evidence/source-routes/coverage.json`.
- Work: For each of the thirteen languages, trace the original detected and missed examples, their disconnected controls and the native extraction shape. Map shared versus adapter-owned semantics and record grammar limitations.
- No-op when: The complete desired behavior already exists and all allocated acceptance checks pass on the current native product, with no missing or reclassified ledger rows.
- No-op handoff: Write proof and exact checks to `docs/briefs/evidence/source-routes/polyglot.json` with status=complete and outcome=no-change. The next child continues from that file. If proof fails, record status=incomplete, perform Stage 2 bounded correction and re-verification, and have the main author recalculate affected parent handoffs before continuing.
- Deliverable: A source-to-native mapping, baseline checks and case allocation in `docs/briefs/evidence/source-routes/polyglot.json`, initially status=incomplete unless the complete no-op proof succeeds.
- Verify: `inspect docs/briefs/evidence/source-routes/polyglot.json`; Inputs: the exact entry-point files and every ledger row allocated to this child; Expected: nonempty input inventory, named implementation boundary, preserved expected outcomes and a recorded command or bounded-inspection result for each claimed baseline.
- Ends when:
  - [ ] The mapping covers this child's full scope and distinguishes existing behavior from missing native behavior.
  - [ ] No user-owned choice or unbounded prerequisite is hidden in the route.
- Handoff: Stage 2 receives the mapping and unmodified source/case inputs in `docs/briefs/evidence/source-routes/polyglot.json`; the no-op route hands the same complete-format file directly to the successor.
- Replan when: Required source evidence is unavailable, a shared model cannot represent an allocated case, or a proposed solution needs a compiler/runtime or framework catalog. The main author pauses only the dependent route, records the failed proof, revises the model/owner handoff and resumes after bounded verification; ask the user only if an actual scope or compatibility decision is required.

### Stage 2 — Implement and prove the allocated behavior
- Starts when: Stage 1 has a complete mapping and recorded baseline or failure evidence in `docs/briefs/evidence/source-routes/polyglot.json`.
- Work: Implement the shared polyglot semantics and the narrow per-language adapters. Execute every allocated native row with exact endpoint/condition checks and all four locked Monix source selections. Reuse the core engine and correct shared defects before duplicating logic.
- Deliverable: Native source changes and `docs/briefs/evidence/source-routes/polyglot.json` containing the minimum handoff fields above, actual command outcomes and the full allocated case result set.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib events::source_routes::tests::test_native_corpus -- --exact --nocapture`; Inputs: the runner options specified in Constraints plus the changed native modules and this child's corpus/side-effect checkpoints; Expected: exit 0 plus native per-case results for thirteen languages and separately labeled Monix JVM/JS Scala 2/3 variants.
- Ends when:
  - [ ] Every allocated acceptance criterion and side-effect checkpoint is backed by an actual native result or an explicit pre-authorized deferral.
  - [ ] The handoff has status=complete only after its observable acceptance checks pass.
- Handoff: The next parent wave consumes `docs/briefs/evidence/source-routes/polyglot.json`; the final wave hands it to the parent for whole-work completion. Do not treat PoC results as native results.
- Replan when: Any native case or existing product contract fails. The main author records exact input and actual output, corrects the owning native layer, reruns affected checks and updates dependent handoffs. Do not weaken expected outcomes or mark completion with unexplained gaps.
- Worker decision: Choose the smallest reusable Rust module boundary and internal representation compatible with the shared proof contract. Reversible placement and bounded data-structure choices do not require another user approval.

## Side Effect Checkpoints
- [ ] Static, instance and namespace owners do not collapse merely because identifiers match.
- [ ] Copies preserve existing elements and distinguish newly allocated containers and appended subscriber values.
- [ ] A passing negative requires its positive control and a nonempty fully analyzed input population.

## Acceptance Criteria
- [ ] For scala-state-subscriber in each locked Monix variant, the actual storage value ends in the new `:subscriber` parameter (not merely a copied prior Set element) and reaches the exact onNext endpoint. Every variant requires source_companion_implicit_selection_required; Scala 2 additionally requires source_macro_template_preservation_required and compiler_macro_typing_unproven; JVM additionally requires qualified_jdk_varhandle_access and compare_and_set_success_required. Record exact storage value, endpoint, variant and matched conditions.
- [ ] Each of the thirteen languages passes all allocated fixture/regression rows with source spans, relation kinds, guards and positive controls preserved.
- [ ] JVM and JavaScript, Scala 2 and Scala 3 remain four explicit source-validation outcomes.
- [ ] No target compiler, runtime, framework name or runtime-delivery assumption is necessary for a supported route.

## Open Questions
- None — the user has fixed language scope, compiler independence, deferred Bevy boundaries and execution ownership; remaining implementation choices belong to the main author.
