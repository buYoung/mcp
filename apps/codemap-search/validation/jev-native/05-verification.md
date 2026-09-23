# Jev native whole-feature verification (child 05)

Status: offline verification completed on 2026-09-23 on the current `opus-5.5` branch at
`82d1e8ab2` plus uncommitted changes (nothing staged or committed). The work used the current
branch instead of `feat/codemap-jev` because the user prohibited inspecting other branches and
worktrees; see `04-integration.md` for the other brief deviations.

No automated check contacted the provider. This covers functional correctness and preservation;
live latency, cost and answer quality were not measured.

Evaluated end-to-end cases used the following setup:

- **Evaluator.** The real `Evaluator` (validation, batching, deadlines, answer checks) runs over the debug-only scripted transport. The script is selected by `CODEMAP_TEST_JEV_SCRIPT` and records each request body.
- **Global configuration.** Each case writes a global `[analysis.jev]` with `api_key_env = "CODEMAP_TEST_JEV_KEY"`, `request_spacing_ms = 0` and the case's mode switches. Every other key keeps its default: model `jev-1.13.0`, 45 s timeout, 3 in-flight requests, 80,000-byte batches, threshold 0.70.
- **Other state.** The key is the fixed offline value `offline-test-key`. Every server gets an isolated `CODEMAP_HOME`.

## Regression matrix

Every contract below has a named case on a populated fixture. Child 05 added `tests/e2e/jev.rs`
(5 cases). The other rows name the owner child's tests, which were rerun here.

Child 05 fixture (`tests/e2e/jev.rs`), all synthetic:

- `src/upload/policy.rs`: Rust const, struct, impl with two methods, and three functions.
- `web/uploadSession.ts`: TypeScript const, and a class with three methods, one holding a nested arrow function.
- `src/banner.rs`: a single Rust function.

The masking case reuses the fake credentials of `tests/e2e/redact.rs`. Search `retry upload banner`
displays 6 complete bodies in 3 files, and the script judges 3 of them unrelated.

| Contract | Case (owner) | Population and assertion |
|----------|--------------|--------------------------|
| Default off; disabled calls unchanged | `e2e::jev::test_jev_disabled_and_keyless_servers_answer_like_the_baseline` (05) | Servers compared: no Jev settings, both modes `false`, both modes `true` without a key. Calls: `tools/list` plus 8 calls (search with and without `task_query`, root overview with and without it, folder overview, `read`, `grep`, `find`). The disabled server's results are byte-identical to the baseline. |
| Keyless operation without network | same (05) | The keyless server runs the production transport path, and the harness strips `TYPESAFE_API_KEY`. Search and overview return the exact baseline text plus one bypass note (`no_task_query`, `missing_credential` or `not_repository_root`); `read`/`grep`/`find` are identical. Only `openWorldHint`, the descriptions and the `task_query` text change in `tools/list`. |
| Intentless and invalid intent | `e2e::mcp::jev::test_jev_missing_key_intent_and_failures_preserve_the_base_output` (04), `tools::task_query` (04) | A blank intent bypasses, `7` and `["retry"]` return `-32602`, and no request is sent. |
| Independent modes | `e2e::mcp::jev::test_jev_modes_are_independent_and_off_by_default` (04) | All four combinations. The disabled tool's output is identical to its regular output. |
| Complete/partial/missing evidence | `tools::search::jev` (03): `incomplete_oversized_and_unknown_evidence_survives_certain_answers`, `bodies_cut_by_the_output_cap_stay_partial`, `certain_unrelated_answers_keep_protected_and_incomplete_bodies` | Rust, TypeScript and JSON fixtures. Partial, missing, oversized and unknown bodies are kept even when the answer is certain. |
| Dependency preservation | `tools::search::jev::retention_closes_over_links_nesting_and_ambiguity` (03) | Links, nesting and ambiguous links are closed over. |
| Source identity and boundaries | `e2e::jev::scripted::test_jev_filter_keeps_source_identity_boundaries_and_observations` (05) | Filtered output: `omitted=3/6 bodies · judged=6`. File headings, declaration headings and `- Symbol:` rows keep their order. Every displayed row in both outputs equals the original line of its file, fences are balanced, and the removed rows are exactly the listed ranges (`describeBanner` L20-23, `format_upload_banner` L33-35, `render_banner` L1-3). All 6 model-bound bodies equal the original lines over their declared range. |
| Post-filter observations | same (05); `tools::search::jev::filtered_away_files_and_the_ranked_tail_are_not_observed` (03) | `analyze` reads after the filtered search list only the two files with shown bodies. `src/banner.rs` appears only after a regular search. |
| Whole-call fallback on malformed answers | `e2e::jev::scripted::test_jev_invalid_answers_fall_back_to_the_whole_regular_call` (05); `jev::tests::test_jev_answer_validation_accepts_raw_values_and_rejects_malformed_answers` (01) | Noul `1.5` gives search `fallback (invalid_answer)`. Score mass 0.5 gives overview `fallback (invalid_answer)`. Missing role answers give overview `fallback (answer_set_mismatch) · requests=2` with no ranking. Each result equals the regular output plus its note. |
| Provider status, deadline, missing answers | `e2e::mcp::jev` (04); `jev::tests` HTTP, deadline and first-failure cases (01); overview role-stage and deadline cases (02) | 529 gives `http_status`, a 3 s delay against a 1 s timeout gives `deadline_exceeded`, and an unanswered question gives `answer_set_mismatch`. |
| Cancellation | `jev::tests::test_jev_cancellation_stops_in_flight_requests` (01) | The runtime stops in-flight work. MCP has no cancellation input; the loop stays sequential. |
| Idle connections | `jev::tests::test_jev_transport_reuses_idle_connections_only_within_the_idle_timeout` (01) | Loopback HTTP server: one connection is reused back to back and replaced after the idle timeout. |
| Output limits | `e2e::jev::scripted::test_jev_additions_stay_within_the_output_caps` (05); overview `small_output_room`/`rendered_section` (02); search `omission_summary_stays_within_its_room` (03) | Overview cap = base + 300: no request, and the output equals the base without a note. Search cap = regular + 40: the evaluation ran but its note did not fit, so the output equals the plain call with no note. Roomy caps: both additions fit, the section is present, and `omitted=0/6`. |
| Threshold fixed per call | `e2e::mcp::jev::test_jev_threshold_is_captured_per_call_across_a_config_change` (04) | Noul 0.8 is omitted at 0.70 and kept at 0.90. An in-flight call keeps 0.70 across a reload. |
| #1 coverage of one snapshot | `tools::overview::jev` mixed-fit, fragmented-file, qualification-before-limit and snapshot-refresh cases (02) | Every indexed file is scored, the best fragment drives ranking, and generations are never mixed. |
| No forced recommendation | `e2e::mcp::jev::test_jev_zero_match_overview_keeps_the_base_overview` (04); `no_match`/`insufficient_evidence` cases (02) | `status=no_match` with the base text and an "absence not shown" wording. |
| Masking before evaluation and after rendering | `e2e::jev::scripted::test_jev_model_bound_evidence_follows_the_redaction_setting` (05) | With masking on, 3 requests and both responses contain none of the three fake secrets. `task_query` arrives as `… the key [REDACTED]?`, and the search body shows `PASSWORD: &str = "[REDACTED]"`. With masking off, the evaluator receives the same raw values the client sees. Sources are unchanged. |
| Live tools and other methods never evaluate | `e2e::mcp::jev::test_jev_live_tools_and_other_methods_never_evaluate` (04) | `read`, `grep`, `find`, `initial_instructions` and `ping` send 0 requests. |
| Config contract | `e2e::config::jev` (04, 3 cases); `config` library group (04) | Defaults off, 0.70/0.90/repo overrides, 6 invalid thresholds, additive v23 → v24 sync. |
| Mock injection not selectable by requests | structural check plus the release binary check below | The scripted transport is compiled only with `debug_assertions` and is chosen from the process environment at start. No argument or config key reaches transport selection. |
| Reuse without MCP | `examples/jev_decisions.rs` runs below; `cargo check --all-targets` | Score, Choice and Noul over explicit state, without a server, index or config. |

## Commands and results (2026-09-23)

| Command | Result |
|---------|--------|
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib "jev::tests::test_jev_"` (runtime, 01) | 19 passed, 0 failed |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview` (02) | 15 passed, 0 failed |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search` (03) | 18 passed, 0 failed |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib config` (04) | 41 passed, 0 failed |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::task_query` (04) | 1 passed, 0 failed |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev` (05) | 5 passed, 0 failed; rerun 3 more times, 5 passed each |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml` (first full run) | exit 101: `e2e_tests` 218 passed, 3 failed, 1 ignored; see "Defect found and routed" |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --no-fail-fast` (after the fix; includes `--test e2e_tests`) | exit 0 in 209 s. Library: 332 passed, 2 ignored (existing Clang/NASM cases). `e2e_tests`: 221 passed, 1 ignored (existing Clang case), in 199.36 s. `extract_snapshots`: 21 passed. `file_format_fixtures`: 12 passed. `navigation_fixtures`: 1 passed. Doc tests: 0. |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --release --test e2e_tests e2e::jev` | exit 0: the keyless and disabled baseline case passed against the release binary. The 9 scripted Jev cases compile out, leaving 213 e2e cases in that profile. |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets` | exit 0, 0 warnings |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml --release --tests` | exit 0, no warnings: the debug-only helpers compile out cleanly |
| `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock` | exit 0: `mock check passed: Score=2.0, Choice=keep, Noul=0.9, fixture usage` (412 input / 36 output tokens from the fixture, 1 request) |
| `cargo run ... --example jev_decisions -- --live` with `TYPESAFE_API_KEY` unset | exit 2: `--live needs TYPESAFE_API_KEY in the environment`; nothing was sent |
| `cargo package --manifest-path apps/codemap-search/Cargo.toml --list --allow-dirty` | exit 0, 450 files; see the next section |
| `cargo build --manifest-path apps/codemap-search/Cargo.toml --release` | exit 0; see the release binary check below |

## Package, release and credential checks

- **Package list.** It includes:
  - the common module `src/jev/*` (10 files) and `examples/jev_decisions.rs`;
  - the adapters, integration and instruction files;
  - `src/mcp/jev/scripted.rs` as source, compiled only in debug builds;
  - the new tests.

  `experiments/`, `validation/`, `docs/`, `tests/fixtures/` and `.codemap` contribute 0 entries under the existing `exclude` list. Nothing was published.
- **Release workflow.** It builds each target with `cargo build --release --target …` and publishes with `cargo publish`. `Cargo.toml` has no profile override, so release artifacts are built without `debug_assertions`. No workflow or CI setting was changed.
- **Release binary.**
  - `target/release/codemap-search` contains 0 occurrences of `CODEMAP_TEST_JEV_SCRIPT` and 0 of `loopback_for_test`, and contains the provider endpoint `api.typesafe.ai/v1/systemone`.
  - The debug binary contains the script variable name, as intended for tests.
- **Endpoint and test hooks.** The endpoint is a constant with no environment override, and `HttpsTransport::loopback_for_test` is `#[cfg(test)]`. The only test that builds `HttpsTransport` checks its `Debug` output and sends nothing.
- **Disposable PoC key.**
  - The `runs/20260921-*` reference runs (506 files) contained one key-like literal.
  - That literal was matched without being printed against 3,560 worktree files, excluding `.git`, `target` and `node_modules`: 0 files contain it.
  - No test fixture holds a real credential. The masking case reuses the existing fake values.
- **Provider traffic.**
  - The e2e harness removes `TYPESAFE_API_KEY`, the script variable and the test key variable from every spawned server.
  - Evaluated e2e cases use the debug-only script. The keyless cases never build an HTTPS client.
  - No test runs the `--live` example.

## Defect found and routed

The first full run failed 3 of 222 e2e cases:

- `e2e::exclusions::test_exclusions_generated_common_and_project_globs`
- `e2e::exclusions::test_exclusions_v6_is_not_regenerated_on_restart`
- `e2e::exclusions::test_exclusions_pre_v6_transition_and_opt_out`

That run reported 218 passed and 1 ignored, which is existing.

- **Cause.** Each case compares the generated header with `CURRENT_CONFIG_HEADER`, still `# codemap-config-version: 23` after child 04 moved the schema to 24 (`left: Some("# codemap-config-version: 24")`).
- **Fix.** Child 04 owns the schema bump. The fix updated that one test constant and changed no production code; child 04's handoff records it.
- **Re-verification.** After the fix, `e2e::exclusions` passed 4/4, and the full suite was rerun (table above).

## Findings and limits

- **Free-text masking.** Masking of `task_query` follows the tool-output rules. Credential formats such as `sk-proj-…` are masked, but free text the rules cannot recognize is sent as written, for example `fixture::auth::credential` without its source assignment. This matches the existing redaction semantics. The configuration guide and README now tell operators to keep secrets out of `task_query`.
- **Discarded search evaluations.** When `output.search.max_bytes` leaves no room for the note, a completed search evaluation is discarded and the regular output is returned. That request's usage appears only in the stderr log.
- **Debug builds.** A debug build, such as `cargo run` from a checkout, can use the scripted transport when `CODEMAP_TEST_JEV_SCRIPT` is set. It logs a warning at startup. `cargo install` and the release workflow build without it.
- **Nondeterminism.** Model answers are not deterministic. The historical run below observed repeated identical requests with differing Score values. No test depends on live answers.

## Not run

- Live provider calls and any paid benchmark: no fresh execution instruction was given, and the old key was not reused. Real latency, throughput, cost, calibration of the 0.70 threshold, and recommendation or filter quality are therefore unmeasured.
- Windows and Linux runs, and cross-target release builds: only macOS arm64 (Darwin 24.6) was used.
- Release-profile execution of the scripted e2e cases, which exist only in debug builds. In release, the keyless and disabled baseline case did run and pass.
- Very large index projection cost for overview recommendations.

## Historical reference (Python PoC, not Rust measurements)

Source: `/Users/buyong/.codex/checkpoints/codemap-search-comparison/JEV-COMPARISON.md`, 2026-09-21. The
private repository, task and quality rubric are recorded there and not copied here. It covers one
task with three runs per variant; the rg baseline had one run. The Python proxy ran against a fixed
older binary, so these figures cannot serve as native baselines and no Rust speedup, quality
equivalence or determinism is claimed from them.

| Variant (3-run mean) | Wall time | Main-model total tokens | Jev input / output tokens | 11-item fulfilment |
|---|---:|---:|---:|---:|
| codemap-search, Jev off | 338.053 s | 1,054,421.7 | 0 / 0 | 72.7% (24/33) |
| #1 overview recommendations | 335.575 s | 944,918.3 | 702,208.7 / 32,894.7 | 63.6% (21/33) |
| #2 search filter | 309.498 s | 857,413.7 | 41,269 / 2,129 | 69.7% (23/33) |

Other recorded facts:

- **Filter effect (#2).** Search response bytes went from 276,965 to 248,195 and code rows from 3,264 to 2,648, with 15/15 reference lines kept.
- **Excluded attempts.** Twelve sessions were started and six used; a connection-reset batch and an output-format batch were excluded with their usage recorded.
- **Repeat consistency.** Identical #1 requests gave 0/48 fully identical answer pairs, equal Score values for 269/1670 questions and equal Choice values for 81/84.
- **Index size.** The earlier baseline indexed 822 files, against 815 for the Jev variants.

## Method for a later authorized live comparison

The method below needs an explicit execution instruction and a fresh operator key.

1. **Pin the inputs.** Freeze one repository commit and index hash, one Rust binary revision, `jev-1.13.0`, and the output budgets and client limits. Use the same cache state for every mode.
2. **Run the variants.** Run Jev off, #1 only and #2 only on that same revision, three runs each, with the same original task passed as `task_query`. Label any Python or `rg` figures as reference columns only.
3. **Record per run:**
   - the applied, bypassed or fallback outcome and reason of every overview and search call, and the Jev requests;
   - Jev input and output tokens, `elapsed_ms` and `http_ms`, taken from the notes and logs;
   - the main model's input, cached input, uncached input and output tokens, plus reasoning as a subset of output, without double counting;
   - whole-task time, tool and `read` counts, response bytes, and the filter reduction in bodies and bytes;
   - API failures, and every excluded attempt with its reason and known usage.
4. **Measure quality.** Freeze the rubric before the runs. Report:
   - for #1, root-file recall of the known reference files within the 24 recommendations;
   - for #2, retention of protected reference lines;
   - repeat-request consistency.

   State the three-run sample limits, and report token throughput separately from price-based cost.
