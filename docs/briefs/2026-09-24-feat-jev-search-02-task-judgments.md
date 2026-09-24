# [feat] Evaluate search functions with task questions

## Work Type
feat

## Current State (As-Is)
- [confirmed] At `8b8222003`, `tools::TASK_QUERY_DESCRIPTION` requires the user's full request and `mcp::McpServer` stores one `registered_task_query` string — Evidence: `src/tools/mod.rs::task_query` and the `initial_instructions` branch in `src/mcp/mod.rs`.
- [confirmed] `tools::search::jev::body_question` creates one fixed Noul question per eligible function and embeds that function's body in the question — Evidence: `src/tools/search/jev.rs::body_question` and `shared_state`.
- [confirmed] `JevEvaluator::evaluate_inner` batches questions sharing one state and starts batches concurrently with shared permits and spacing — Evidence: `src/jev/evaluator.rs` and `src/jev/batch.rs::pack`.
- [confirmed] The current limits are 3 concurrent requests, 300ms minimum start spacing, and 80,000 encoded bytes per batch — Evidence: `src/jev/mod.rs`.
- [confirmed] Enabled read/grep filters reuse task registration and may suppress explicitly requested source — Evidence: `src/mcp/jev.rs::live` and `src/tools/live_symbols/jev.rs::Plan::apply`.
- [inferred] Merely inserting two sentences into the existing string will still produce one fixed judgment per function — Confirm by: following `body_question` into `EvaluationRequest::new` and inspecting the emitted question count.

## Desired Outcome (To-Be)
- The main LLM prepares explicit task-specific yes/no questions once per task; enabled search requires their registration and uses every registered question in actual function evaluation.
- Each candidate is evaluated against shared function evidence and independently addressable questions, with deterministic composition and an explicit uncertain state.
- Read and grep deliver original source through their existing local tool behavior; Jev applies only to search.
- The existing shared HTTP runtime remains bounded, cancellable, and credential-safe.

## Scope
### In Scope
- Extend task registration, validation, connection-local storage, task replacement, tool schemas, and agent guidance as one end-to-end contract.
- Require a nonempty question list when Jev search is enabled; return an actionable invalid-argument error for legacy task-text-only registration.
- Generate per-candidate questions from the registered list, share candidate bodies in bounded group-specific states, and consume the returned answers in search.
- Carry file identity, source ranges, body completeness, verified call/event evidence, and missing-context indicators through evaluation.
- Preserve existing response fallback and cancellation contracts across all groups.
- Remove read/grep Jev invocation and active filter settings/guidance while preserving their source views, expansion, pagination, redaction, observations, and limits.
- Align configuration, examples, existing test inputs, and current English/Korean documentation with the new search-only contract.
### Out of Scope
- [hard] Reintroducing overview recommendation or implicit task registration through overview.
- [deferred] Candidate/output-budget optimization owned by child 03.
- [deferred] Separate LLM API calls to generate questions, persistent judgment caches, adaptive concurrency, and nested task-expression languages.
- [hard] Repository-wide retrieval changes or hardcoded production logic for the measure-worker example.

## Constraints
- The user explicitly approved mandatory question lists and rejection of legacy text-only registration on 2026-09-24. Jev-disabled `initial_instructions({})` remains valid.
- Use a readable `task_query` goal plus a `questions` array with unique ids, explicit yes/no wording, and aligned true/false criteria. Expose `match` as `all` or `any`, defaulting to `all`; describe the mode in the schema. Do not infer conjunction from punctuation.
- Permit a single question for simple tasks. Bound list size, text bytes, nesting, and encoded payloads using documented validation; record the selected technical bounds in the handoff.
- Preserve the request's target, direction, and full coverage. For the reference task, Q1 covers event publication/subscription/receipt/relay/handling and Q2 asks whether that same flow connects to measure-worker input/output.
- Do not expand the target from `measure worker` to arbitrary workers or reduce bidirectional communication to direct calls.
- Treat absent cross-file evidence as uncertainty, not proof of irrelevance. Ask about supplied facts; do not require Jev to reconstruct an unseen call chain.
- Use Noul for these yes/no questions. Compose leaf decisions in code; never label a product, mean, or minimum of answers as a calibrated joint probability.
- Keep task registration atomic and connection-local; invalid replacement clears stale task state as today. Re-register when the user's task changes, not for each search query.
- Share candidate source once per group state and explicitly name the candidate in each question. Split evidence before packing questions; never copy the full candidate catalogue into every state.
- Preserve the caller's cancellation token and one absolute tool deadline across all groups. Reuse the existing shared evaluator/connection pool and its 3-request/300ms bounds.
- Any required group/answer/provider failure preserves the unfiltered base output for that search and records the reason. A missing mandatory registration remains an error, not an unfiltered fallback.
- Read/grep must not evaluate even when stale legacy filter flags are present. Retire those flags using existing never-exit configuration behavior without rewriting unrelated user settings.
- Update existing cases only where the approved contract changes their inputs/expectations. Add no test files/cases or external dependencies.
- Keep this child atomic under BDR K1/K2: registration, its consumers, grouped evaluation, and failure propagation must ship together. Leave output selection to child 03.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/mod.rs` — replace task registration schemas, parsing, and Jev navigation guidance.
- `apps/codemap-search/src/mcp/mod.rs` — carry structured registration from initialization to search and restore local read/grep dispatch.
- `apps/codemap-search/src/mcp/jev.rs` — preserve credential resolution, deadline, cancellation, fallback, and stage diagnostics.
- `apps/codemap-search/src/tools/search/jev.rs` — generate registered questions and consume per-candidate answers.
- `apps/codemap-search/src/tools/search/mod.rs` — carry the registered plan into the search adapter and its current output consumer.
- `apps/codemap-search/src/jev/evaluator.rs` — reuse bounded dispatch and whole-request failure accounting.
- `apps/codemap-search/src/jev/batch.rs` — inspect state duplication before introducing grouped evidence.
- `apps/codemap-search/src/jev/question.rs` — reuse typed Noul questions and id validation.
- `apps/codemap-search/src/config/jev.rs` — retire live-tool activation and document remaining search settings.
- `apps/codemap-search/src/config.rs` — align schema synchronization and generated comments with the new contract.
- `apps/codemap-search/src/config/layout.rs` — remove retired live-filter keys from active layout metadata.
- `apps/codemap-search/src/config_template.toml` — update the English generated configuration contract.
- `apps/codemap-search/src/config_template.ko.toml` — update the matching Korean configuration contract.
- `apps/codemap-search/src/tools/read.rs` — retain original source-window and observation behavior while removing Jev capture.
- `apps/codemap-search/src/tools/grep.rs` — retain expansion, pagination, and source accounting while removing Jev capture.
- `apps/codemap-search/src/tools/live_symbols.rs` — remove obsolete module/capture wiring without changing shared symbol extraction.
- `apps/codemap-search/src/tools/live_symbols/jev.rs` — remove the obsolete live-filter route without changing ordinary source extraction.
- `apps/codemap-search/tests/e2e/mcp.rs` — adapt existing registration expectations to the approved contract.
- `apps/codemap-search/tests/e2e/jev.rs` — reuse captured evaluator requests and failure-path assertions.
- `apps/codemap-search/tests/e2e/jev_live.rs` — retire removed live-filter expectations and retain applicable original-source checks.
- `apps/codemap-search/README.md` — synchronize task registration examples with `README.ko.md`.
- `apps/codemap-search/docs/configuration.md` — synchronize examples with the Korean counterpart and config templates.
- `apps/codemap-search/examples/jev_decisions.rs` — keep retained runtime examples buildable.
- `apps/codemap-search/validation/jev-search/01-overview.json` (proposed) — consume child 01's restoration evidence.
- `apps/codemap-search/validation/jev-search/02-task-judgments.json` (proposed) — publish the task/evaluation contract.

## Execution Plan
### Stage 1 — Fix the registration-to-answer contract
- Starts when: `apps/codemap-search/validation/jev-search/01-overview.json` reports completed restoration with no unexplained differences.
- Work: Trace registration through dispatch, evidence capture, runtime requests, answers, and the existing body-output consumer. Specify the typed task/question shape, all/any composition, uncertain state, validation bounds, and migration of retired live filters.
- No-op when: The complete required registration, grouped evaluation, original read/grep behavior, and final search consumer already satisfy this contract with recorded evidence.
- No-op handoff: Publish complete evidence at `apps/codemap-search/validation/jev-search/02-task-judgments.json` and allow child 03 to proceed without feature edits.
- Deliverable: The contract draft in `apps/codemap-search/validation/jev-search/02-task-judgments.json` with schema, question semantics, composition, limits, and traced producers/consumers.
- Verify: `Bounded inspection of tools::task_query, MCP registration/search dispatch, body_question, EvaluationRequest, and the search output consumer`; Inputs: the listed Rust entry points and child 01 handoff; Expected: every field has a named producer and final consumer, with no legacy-only silent path.
- Ends when:
  - [ ] Missing/invalid/stale registrations and disabled-Jev behavior have explicit outcomes.
  - [ ] Function evidence and question ids have a stable mapping through grouped requests and answers.
- Handoff: Stage 2 receives the implementation contract and bounded technical choices.
- Replan when: A proposed schema change exceeds the approved mandatory-question break or requires another user-facing compatibility break.

### Stage 2 — Implement the complete task evaluation path
- Starts when: Stage 1 defines the registration and result contract.
- Work: Implement registration, task guidance, bounded evidence grouping, per-candidate Noul questions, deterministic composition, fallback, and consumption by the current search output path. Remove Jev from read/grep and align active config/docs.
- Deliverable: Buildable end-to-end task evaluation with masked diagnostic ids and separate per-question judgments.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets`; Inputs: the changed package, retained examples, and module consumers; Expected: exit 0 and no orphaned live-filter imports.
- Ends when:
  - [ ] Registered questions reach the provider request and their answers reach the final search retention decision.
  - [ ] Candidate source is shared within each group, groups share the bounded transport, and failure/cancellation preserves the original contracts.
  - [ ] Read/grep return original source without requiring Jev credentials or registration.
- Handoff: Stage 3 receives the implementation and exact contract changes.
- Replan when: Evidence required for the reference questions cannot be represented without inventing cross-file relationships; mark uncertainty and route bounded evidence work to child 03.

### Stage 3 — Verify contract propagation and publish evidence
- Starts when: Stage 2 is buildable or Stage 1 proves the full no-op route.
- Work: Run existing MCP checks with the approved input adaptations and inspect their captured requests. Also run `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev`, `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev`, `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools`, and `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config` as separate commands. Use the existing Jev integration cases to inspect requests and delivered output through the real MCP pipeline. Inspect historical measure-worker evidence against the registered Q1/Q2 wording.
- Deliverable: `apps/codemap-search/validation/jev-search/02-task-judgments.json` with `completed`, `source_revision`, `source_diff_hash`, `schema`, `question_examples`, `composition`, `limits`, `consumer_trace`, `failure_behavior`, `checks`, and evidence paths.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp`; Inputs: existing MCP cases, adapted registration payloads, and the separately executed library/Jev-integration/tools/config checks; Expected: nonzero executed counts, exit 0 for each command, captured request-to-output propagation through the real MCP pipeline, and no Jev calls from read/grep/overview.
- Ends when:
  - [ ] Mandatory questions, invalid replacement, task replacement, disabled mode, and provider failure have inspected evidence.
  - [ ] The handoff records all/any and uncertainty semantics without claiming new end-to-end token savings.
- Handoff: Child 03 consumes `apps/codemap-search/validation/jev-search/02-task-judgments.json` to implement output selection.
- Replan when: Verification reveals a broken registration/runtime/output contract; stop child 03, correct this child, and rerun affected verification before updating the parent.

## Side Effect Checkpoints
- [ ] `search.query` remains retrieval input and does not replace registered task intent.
- [ ] `workspace_scope`, `event_key`, read/grep views, range aliases, and filesystem boundaries retain their existing meanings.
- [ ] Raw task text, source, and credentials do not leak through new diagnostics; API keys remain environment-only.
- [ ] Concurrent groups cannot extend deadlines, lose cancellation, or apply late answers to another registration.
- [ ] Existing source observations count delivered source accurately and never treat omission notes as source.
- [ ] English/Korean templates and examples agree with the active search-only schema and removed settings.

## Acceptance Criteria
- [ ] With Jev search enabled, text-only registration fails explicitly and a valid question list drives the actual search evaluation path.
- [ ] The reference task produces separate event-flow and measure-worker-link judgments for each eligible candidate.
- [ ] Grouping avoids duplicating a candidate body per question and stays within transport/context bounds.
- [ ] Unknown or failed judgments cannot silently remove required evidence.
- [ ] Read/grep/overview invoke no Jev evaluation and retain their local navigation contracts.
- [ ] The completed handoff documents actual checks and the contract consumed by child 03.

## Open Questions
- None — the user approved mandatory question lists; remaining schema bounds and internal layout are bounded implementation choices.
