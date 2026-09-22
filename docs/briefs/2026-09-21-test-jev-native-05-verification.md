# [test] Verify native Jev behavior and reusable integration

## Work Type
test

## Current State (As-Is)
- [confirmed] The existing integration target is e2e_tests and organizes MCP, config, search, codemap, and redaction cases by module — Evidence: `apps/codemap-search/tests/e2e_tests.rs` and `tests/e2e/mod.rs`.
- [confirmed] The accepted Python PoC needed source-preservation and Markdown-boundary corrections — Evidence: `checkpoint-poc/analyze_improvements.py`, `index_evidence.py`, and `mcp_proxy.py::render_filtered`.
- [confirmed] The historical benchmark uses fixed original intent, records API usage separately, and excludes unsuccessful attempts explicitly — Evidence: `checkpoint-poc/aggregate.py` and `report_improved.py`.
- [confirmed] The current product has a crates.io publishing route — Evidence: `.github/workflows/codemap-search-release.yml` and `apps/codemap-search/README.md` installation instructions.
- [inferred] Functional parity and safe native integration can be verified offline; real-provider latency and answer quality require a separately authorized live run — Confirm through the fake-evaluator regression matrix and record live measurement as not run when unavailable.

## Desired Outcome (To-Be)
- Provide reproducible proof for independent modes, default-off compatibility, preserved source, and reuse of the common Rust module.
- Produce an evidence report separating offline correctness, historical PoC reference metrics, and any separately authorized live measurement.
- Keep release-facing code free of test hooks, credentials, Python dependencies, and private benchmark source.

## Scope
### In Scope
- Add cross-feature regression coverage and the smallest necessary shared test-helper changes.
- Exercise realistic structured evidence in Rust and TypeScript, including impl/struct/constant and nested-method cases.
- Run existing relevant suites and check operator examples against actual native invocation behavior.
- Record the benchmark method and output metrics so a later authorized live comparison is reproducible.
- Specify main-model input, cached input, uncached input, output, reasoning subset, and Jev input/output separately without double counting. Distinguish token throughput from actual cost.
- Specify whole-task and Jev-stage time, tool/read counts, response bytes, filtering reduction, API/fallback failures, and all excluded attempts with their reason and known usage.
- Specify frozen quality-item scoring, root-file recall, protected-source retention, repeat-request consistency, and the limits of a three-run sample.
- Pin source/index hashes, scope, model, Rust binary revision, output budgets, and cache state. Compare native Jev off/#1/#2 on that same revision and keep historical Python measurements labeled as references.
### Out of Scope
- [hard] Do not repair production logic silently in this verification child; return defects to their owning child and rerun the affected dependency chain.
- [hard] Do not publish crates/releases, commit private hicare source fixtures, reuse the disposable key, or send requests to the real provider during automated tests.
- [deferred] New paid live benchmark runs, percentage-based speed acceptance targets, broad multilingual quality claims, and default-on rollout decisions are outside this implementation verification.

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

## Related Files / Entry Points
- `apps/codemap-search/tests/e2e_tests.rs` — Use the existing integration-test target.
- `apps/codemap-search/tests/e2e/mod.rs` — Register the new focused regression module.
- `apps/codemap-search/tests/e2e/helpers.rs` — Reuse existing server/fixture setup and extend only the necessary injection boundary.
- `apps/codemap-search/tests/e2e/jev.rs` (proposed) — Own cross-mode and compatibility regression scenarios.
- `apps/codemap-search/tests/fixtures/` — Reuse populated language/source fixtures before creating new minimal inputs.
- `apps/codemap-search/experiments/jev-playground/checkpoint-poc/analyze_improvements.py` — Preserve the stronger file-plus-line verification logic.
- `apps/codemap-search/experiments/jev-playground/checkpoint-poc/aggregate.py` — Reference metric boundaries and excluded-attempt accounting.
- `.github/workflows/codemap-search-release.yml` — Inspect existing build/package targets without publishing.
- `apps/codemap-search/validation/jev-native/04-integration.md` (proposed) — Consume the integrated activation and compatibility contract.
- `apps/codemap-search/validation/jev-native/05-verification.md` (proposed) — Record final whole-feature verification and remaining limits.

## Execution Plan
### Stage 1 — Establish the regression matrix
- Starts when: `apps/codemap-search/validation/jev-native/04-integration.md` exists with integrated sample calls, settings, lifecycle behavior, and successful targeted checks.
- Work: Bind each accepted requirement to a concrete offline scenario using the existing test harness, injected evaluator, and non-sensitive fixtures. Cover disabled/keyless/intentless calls, independent modes, complete/partial/missing evidence, dependency preservation, source identity, malformed answers, timeout/cancellation, idle connections, and output limits.
- No-op when: The full new regression matrix already exists and all required suites plus the reuse example have current passing evidence.
- No-op handoff: Record the exact commands, populated cases, and results in `apps/codemap-search/validation/jev-native/05-verification.md` (proposed), then let the parent evaluate global acceptance without redundant test edits.
- Deliverable: An enumerated regression matrix with exact fixture populations and test ownership.
- Verify: `bounded inspection of the matrix against the four implementation handoffs`; Inputs: The populated Rust/TypeScript fixtures and the 01-runtime through 04-integration handoff files; Expected: Every behavioral contract has a named positive or failure case and no check can pass on an empty fixture population
- Ends when:
  - [ ] Existing test facilities are reused and new inputs are minimal and non-sensitive.
  - [ ] No scenario requires real credentials or a production endpoint override.
  - [ ] The matrix distinguishes source preservation from final answer correctness.
- Handoff: Stage 2 receives the complete matrix and fixture mappings.
- Replan when: An implementation contract fails inspection: stop dependent verification, return to the parent, activate bounded correction and re-verification in the owner, and recalculate topology/handoffs before resuming.
### Stage 2 — Run native integration and preservation checks
- Starts when: The regression matrix is complete and the integrated adapters can be invoked through the test boundary.
- Work: Add the approved regression coverage, run focused new cases and existing MCP/config/search/codemap/redaction suites, and compare disabled outputs against matching baseline fixtures. Check retained code by file identity and original line content, all declaration names/ranges, balanced file sections, source observations, and whole-call fallback.
- Deliverable: Test logs with explicit passed/failed scenarios and artifact paths.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests`; Inputs: The existing e2e target including the new e2e::jev module and populated fixtures; Expected: Exit 0 with no outbound provider traffic and no silent skipped mandatory cases
- Ends when:
  - [ ] The same failure-prone cases pass through the actual Rust pipeline rather than only through a duplicate test implementation.
  - [ ] Applied/bypassed/fallback and API usage are distinguishable in observable results.
  - [ ] Current CLI and unrelated tool behavior remain covered by existing suites.
- Handoff: Stage 3 receives regression results and any owner-directed corrections.
- Replan when: A suite or invariant fails: stop the completion gate, return the defect to its owning child for bounded correction, rerun affected verification, and update parent handoffs before continuing.
### Stage 3 — Publish the execution evidence
- Starts when: The regression matrix and existing affected suites pass after all owner corrections.
- Work: Check the direct Rust example and existing package/build route. Record commands, fixture counts, implementation revision, configuration, evaluator mode, and limitations. Run `cargo package --manifest-path apps/codemap-search/Cargo.toml --list --allow-dirty` to inspect package contents without publishing. Preserve the historical four-way metrics only as reference. Document the future live-run method: same task/source snapshot/binary settings, three runs per mode, actual applied status, all attempts, API/main-model token separation, total time, source preservation, and frozen quality rubric.
- Deliverable: `apps/codemap-search/validation/jev-native/05-verification.md` (proposed) with the regression matrix, actual logs, example/package checks, historical references, and explicit live-run status.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets`; Inputs: The library, binary, examples, tests, and current dependency lockfile; Expected: Exit 0 and the documented reuse example compiles without Python or MCP startup
- Ends when:
  - [ ] All stage and side-effect checks are recorded with actual outcomes.
  - [ ] No live metric is fabricated or presented as a Rust improvement without a real authorized run.
  - [ ] Any unavailable platform/live-provider checks are named instead of reported as passed.
- Handoff: The parent receives `apps/codemap-search/validation/jev-native/05-verification.md` for whole-set acceptance.
- Replan when: Packaging, platform support, or real-provider behavior is needed to resolve a failing requirement: stop completion, route bounded correction/re-verification through the parent, and update the dependency map before proceeding.

## Side Effect Checkpoints
- [ ] Keep new fixtures free of credentials and private repository source.
- [ ] Keep production endpoint selection, logging, and runtime features free of test-only overrides.
- [ ] Check output masking before external evaluation and after rendering using the existing redaction fixture population.
- [ ] Ensure test-side mock injection cannot be selected by a normal production request.
- [ ] Check package contents against the existing release workflow without publishing or changing unrelated CI/lint settings. The package list contains the Rust common module and calling example while existing exclusions keep experiments, validation artifacts, and private fixtures out.

## Acceptance Criteria
- [ ] The required new regression matrix and affected existing integration suites pass with offline evaluation.
- [ ] Disabled native calls preserve baseline responses and no-key operation without network access.
- [ ] Both enabled adapters exercise the shared Rust evaluator, retain required evidence, and fall back safely.
- [ ] The independent reuse example and all Cargo targets compile.
- [ ] The final evidence explicitly separates offline functional completion from unperformed live performance/quality validation.

## Open Questions
- None — The user selected an internal common module, independent default-off modes, and necessary regression tests. Remaining implementation choices are bounded in the stages.
