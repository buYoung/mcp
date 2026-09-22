# [feat] Add a reusable Rust Jev decision runtime

## Work Type
feat

## Current State (As-Is)
- [confirmed] The inspected baseline is `97e3ebc8e` on `feat/codemap-jev` — Evidence: branch creation from the fetched `main` revision.
- [confirmed] The crate already exposes a Rust library and uses Tokio on a current-thread runtime, but declares no Jev module or direct HTTP client dependency — Evidence: `src/lib.rs`, `src/main.rs::main`, and `Cargo.toml` under `apps/codemap-search/`.
- [confirmed] The Python PoC batches Score/Choice questions, validates response IDs and model, and records usage — Evidence: `experiments/jev-playground/checkpoint-poc/jev_transport.py::pack`, `evaluate`, and `Broker.call`.
- [confirmed] The corrected PoC replaces HTTP connections after 30 seconds idle and performs no automatic HTTP retries — Evidence: `jev_transport.py::Broker.call`.
- [confirmed] The HTTP API supports Score, Choice, and Noul; Noul returns a yes-probability without a confidence field — Evidence: [TypeSafe HTTP API](https://docs.typesafe.ai/api.md), inspected 2026-09-21.
- [inferred] A generic evaluator taking owned data and request-local policy can serve multiple Rust tool adapters without MCP coupling — Confirm by executing a mock-backed standalone example inside the existing crate.

## Desired Outcome (To-Be)
- Expose a reusable asynchronous evaluator for typed Jev Score, Choice, and Noul questions over caller-supplied JSON state.
- Return typed answers, usage, timing, and bounded failure information without depending on MCP, index handles, current directory, or global task intent.
- Allow a mock evaluator/transport to exercise adapters without credentials or network access.

## Scope
### In Scope
- Create the common module, request/response types, evaluator abstraction, and real asynchronous HTTPS implementation.
- Implement bounded batching, response validation, request-local deadlines, caller cancellation, concurrency control, and idle connection handling.
- Provide an in-crate Rust calling example and focused runtime regression tests.
- Record the API contract and initial policy defaults for both adapter children.
### Out of Scope
- [hard] Do not add overview/search policy or MCP/config wiring in this child.
- [deferred] Standalone crate publication, other providers, persistent inference caches, and generic plugin discovery are outside this initiative.

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
- Pin the initial provider model to `jev-1.13.0`; keep alias changes and deterministic-result assumptions out of the contract.
- Carry forward the PoC ceilings of 80,000 encoded request bytes, three in-flight HTTP requests, 300ms request spacing, and no automatic retries. Use a finite 45,000ms initial whole-call deadline including queue time and let callers shorten it.
- Treat byte packing as a conservative transport bound, not an exact provider tokenizer. Oversized questions or provider context-limit errors must yield explicit failure rather than silently losing input.
- Record both published Jev 1.13 limits: 64k tokens for state plus all questions, and 32k tokens for state plus the longest question. A batch meeting the byte ceiling does not prove either token limit. In Stage 1, document tokenizer availability or the bounded estimation method and its limitations; retain provider-limit fallback without silent truncation.
- Keep typed validation, transport limits, and their fallback errors together as one shared failure contract.
- Use `POST https://api.typesafe.ai/v1/systemone` with the documented model/state/questions JSON envelope. Keep authentication in the transport boundary and never put credentials in the model state.
- Validate Score criteria at 2–10 levels and its answer against that level count, rather than hardcoding #1's 0–3 scale. Validate Choice against its supplied option keys and the 255-option limit. Accept only finite Noul values in [0, 1]; do not require or synthesize a Noul confidence field.
- Accept string/object/array instructions and supported structured criteria. Keep shared facts in named state fields and candidate-specific evidence in named state or instruction fields. Put the complete judgment and backticked evidence references in instructions; question IDs are routing identifiers and never supply meaning.
- Batch independent questions sharing state together. Each question must contain or reference its own needed evidence; it cannot read sibling instructions or answers. A dependent second stage receives explicitly constructed new input after the first stage completes.
- Preserve raw answers and probabilities in the returned result so adapters can recompute policy without inference. Include model ID, request identity, usage, and timing; adapters own question/policy versions. Confidence describes distribution concentration, not correctness or permission to act.
- Set the initial idle connection lifetime to 30,000ms. Keep the request and connection lifetimes bounded and document any HTTP-library-specific safe connection replacement behavior.

## Related Files / Entry Points
- `apps/codemap-search/Cargo.toml` — Add only the async HTTPS/TLS dependency needed by the common evaluator and update its lockfile.
- `apps/codemap-search/src/lib.rs` — Export the common module.
- `apps/codemap-search/src/jev/` (proposed) — Own typed inputs, outputs, evaluator abstraction, transport, and policy.
- `apps/codemap-search/examples/jev_decisions.rs` (proposed) — Demonstrate calling the common evaluator independently of MCP.
- `apps/codemap-search/experiments/jev-playground/checkpoint-poc/jev_transport.py` — Port the validated transport behavior, including the idle-connection correction.
- `apps/codemap-search/experiments/jev-playground/checkpoint-poc/improved_proxy.py` — Read the two existing consumers to settle the minimal shared contract.
- `apps/codemap-search/validation/jev-native/01-runtime.md` (proposed) — Publish the runtime handoff.
- [TypeSafe API](https://docs.typesafe.ai/api.md), [models](https://docs.typesafe.ai/models.md), and [state](https://docs.typesafe.ai/concepts/state.md) — Recheck wire types, both context limits, and structured input guidance before implementing transport.

## Execution Plan
### Stage 1 — Lock the evaluator contract
- Starts when: `feat/codemap-jev` exists at the inspected main baseline and the preserved Python PoC is available.
- Work: Define the smallest common API needed by both adapters: explicit task intent, JSON state, Score/Choice/Noul questions, caller-supplied deadline/cancellation, raw typed answers, usage, and errors. Inspect current TypeSafe API documentation and the selected HTTP library before choosing supported APIs. Record both token-limit checks or estimates, independent batching, and validation tolerances. Keep code-specific candidate types and ranking/filter thresholds in their owning adapters.
- No-op when: An existing common module already satisfies every runtime acceptance criterion and its independent example and runtime tests pass.
- No-op handoff: Record the exact API and successful commands in `apps/codemap-search/validation/jev-native/01-runtime.md` (proposed) and let the parent start the adapter children without rewriting the module.
- Deliverable: `apps/codemap-search/validation/jev-native/01-runtime.md` (proposed) with API names, three primitive shapes, transport-policy units, token-budget limitations, dependency version/MSRV/TLS choice, and mock-injection route.
- Verify: `bounded inspection of the runtime contract against both adapters and current HTTP documentation`; Inputs: The preserved PoC consumers, children 02/03's revised judgments, and the official API; Expected: Score/Choice/Noul consumers map to the contract without MCP, filesystem, or parser dependencies
- Ends when:
  - [ ] Every question and answer has an explicit type and request-local identifier.
  - [ ] The selected HTTP/TLS APIs support the existing Rust toolchain and release targets.
- Handoff: Stage 2 receives the contract recorded in the runtime handoff.
- Replan when: A separately published Jev crate or a breaking existing library API change becomes necessary: return that scope choice to the parent. Selecting the required third-party HTTP/TLS dependency within this crate remains this child's implementation decision.
### Stage 2 — Implement the bounded runtime
- Starts when: The contract in `apps/codemap-search/validation/jev-native/01-runtime.md` is concrete and supports both consumers.
- Work: Implement asynchronous HTTPS with an injectable boundary. Validate the pinned model, exact response-ID set, all three answer types, finite values, probability keys/ranges, and distributions within the recorded tolerance. Enforce byte/request/deadline budgets before dispatch, include queue time in deadlines, stop on cancellation, and return a typed failure on malformed or incomplete batches. Reuse connections with bounded idle lifetime and avoid automatic POST retries.
- Deliverable: The implemented common module and updated runtime contract.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev`; Inputs: Mixed Score/Choice/Noul batches, Noul without confidence, invalid criteria/answers, timeout, cancellation, byte caps, both provider context-limit failures, and idle reuse; Expected: Exit 0 with no live provider requests
- Ends when:
  - [ ] All failure paths release concurrency permits and respect the caller's shorter deadline.
  - [ ] Credentials and request source data are absent from Debug output and routine diagnostics.
  - [ ] Usage and timing distinguish HTTP work from whole-call elapsed time.
- Handoff: Stage 3 receives the runtime and its offline regression evidence.
- Replan when: Budget enforcement requires unbounded task storage or blocks the Tokio thread: stop, revise the contract within the parent, and re-verify before consumers proceed.
### Stage 3 — Prove independent reuse
- Starts when: The runtime tests pass and the common module is exported from the existing crate.
- Work: Implement and execute an example that invokes the evaluator directly with caller-supplied state. Make `--mock` the default offline mode and verify Score=2.0, Choice=keep, Noul=0.9 plus usage from a fixed mixed-response fixture. Return a nonzero exit on a mismatch. Require explicit `--live` and host-resolved credentials for an optional operator-run real call; never use credentials in mock mode.
- Deliverable: `apps/codemap-search/validation/jev-native/01-runtime.md` (proposed) containing the final API, defaults, example path, dependency decisions, and test results.
- Verify: `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock`; Inputs: The direct example, mixed-response fixture, and common module; Expected: Exit 0 after checking Score=2.0, Choice=keep, Noul=0.9 and fixture usage, without credentials, provider traffic, MCP, or Python
- Ends when:
  - [ ] The example needs no engine, workspace snapshot, or global application config.
  - [ ] The handoff provides everything the overview and filter adapters need.
- Handoff: The parent and both adapter children receive `apps/codemap-search/validation/jev-native/01-runtime.md`.
- Replan when: The example needs tool-specific state inside the common module: stop consumer work and move that state back into its adapter.

## Side Effect Checkpoints
- [ ] Run `cargo check --manifest-path apps/codemap-search/Cargo.toml` and retain the current library and binary targets.
- [ ] Inspect the dependency tree and existing release workflow: avoid introducing platform-specific OpenSSL installation or a new published crate.
- [ ] Confirm the module does not import MCP, parser, index, workspace, or the global config loader.
- [ ] Confirm production construction cannot enable requests merely because the module is linked.

## Acceptance Criteria
- [ ] Score, Choice, and Noul can be evaluated together by a Rust caller using only explicit inputs and the common interface.
- [ ] Runtime regressions cover all three wire types, criteria limits, malformed/incomplete answers, timeouts, cancellation, both context-budget failures, and idle reuse without external requests.
- [ ] The direct calling example executes its mock assertions successfully and the runtime handoff identifies the complete reusable API and token-estimation limitations.
- [ ] No Python process or local proxy is required by the compiled Rust functionality.

## Open Questions
- None — The user selected an internal common module, independent default-off modes, and necessary regression tests. Remaining implementation choices are bounded in the stages.
