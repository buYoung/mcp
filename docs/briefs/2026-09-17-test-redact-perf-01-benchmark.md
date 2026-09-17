# [test] Make redaction performance measurements reproducible

## Work Type
test

## Current State (As-Is)
- [confirmed] The inspected implementation is commit `e5fc44172faaa87a8e0fd358d160bfe881d5e47b` on 2026-09-17 — Evidence: `git rev-parse HEAD` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`.
- [confirmed] The durable large-file regression creates a 10,000-line, 439,058-byte TypeScript file with ten synthetic secrets — Evidence: `apps/codemap-search/tests/e2e/redact/large_file.rs`, `secret_cases()`, `large_source()`, and `test_redact_ten_secrets_in_ten_thousand_line_file()`.
- [confirmed] The three-condition benchmark is captured in `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`, but its executable harness and no-secret file were temporary artifacts — Evidence: the captured `requests`, `conditions`, and `results` fields and the current large-file test's secret-only generator.
- [confirmed] Current response tests cover read/grep/search, literal hints, errors, disabled masking, original matching, and byte ceilings — Evidence: `apps/codemap-search/tests/e2e/redact.rs` and `apps/codemap-search/src/redact/tests.rs`.
- [inferred] The roughly 34ms common delta combines parsing, syntax walking, rules, rendering, and request overhead — Confirm by isolated phase measurements with the same original buffer and policy; do not assign the whole delta to Tree-sitter.

## Desired Outcome (To-Be)
- A clean checkout can regenerate the two equal-size fixtures and repeat the same three-condition JSON-RPC comparison without any historical temporary directory.
- Successor workers receive raw samples, exact requests, hashes, environment metadata, output-equivalence evidence, and a phase-cost breakdown.
- The harness separates startup/first-request, repeated-request, and changed-source measurements and can record whether a future cache actually hit.

## Scope
### In Scope
- Promote the previous temporary stdio measurement procedure into a repository-owned, standard-library-only harness.
- Share a canonical synthetic fixture specification between the existing Rust regression and the measurement harness; preserve the original secret fixture's bytes.
- Add ignored/test-only phase probes where existing code cannot expose parse, syntax inspection, regex/context detection, and rendering times independently.
- Add correctness assertions and provenance checks to every timing population.
### Out of Scope
- [hard] Runtime caching, parser reuse, worker threads, or modifications to production detector semantics.
- [hard] Real credentials, new public benchmark MCP tools, public telemetry, or a new benchmark framework dependency.
- [deferred] Broad language-corpus benchmarking beyond the primary fixture and the correctness population owned by Child 05.

## Constraints
- Keep the primary cases exactly `off_with_secrets`, `on_with_secrets`, and `on_without_secrets`; the off baseline uses the secret-bearing file.
- Generate both inputs at exactly 10,000 lines and 439,058 bytes. Preserve quote/newline/node structure while replacing sensitive identifiers and credential syntax with same-length public data; verify zero masking in the clean variant.
- Preserve the original SHA-256 `d91387443d2bf1ad4a488033f9539ed48cc6b4658ee7cd8c0c83b419c14dda47` and clean SHA-256 `c50aca76ad69f4780d3cb5469ab7697da82530ab82c5330a28967db5a70578a6`, or stop and reconcile an explicit fixture-version change before timing.
- Reproduce the historical clean bytes with the recipe below. `secret_source` comes from `large_source()` and `secret_values` are the ten `SecretCase.value` strings in declaration order. The author verified the resulting hash against the historical clean file; the recipe has no dependency on that temporary file.

```python
clean = secret_source
for number, value in enumerate(secret_values, start=1):
    prefix = f"public-example-{number:02}-"
    clean = clean.replace(value, prefix + "x" * (len(value) - len(prefix)))
for old, new in [
    ("const password =", "const passcode ="),
    ("accessToken:", "accessLabel:"),
    ("const clientSecret =", "const clientPublic ="),
    ("postgres://reader:", "postgres://reader/"),
    ("@localhost/example", "/localhost/example"),
    ("PRIVATE KEY-----", "PUBLIC DATA-----"),
]:
    clean = clean.replace(old, new)
```

- Reproduce the five exact requests from the captured JSON, including `view=source`, `offset=9124`, the eleven matches for the common const pattern, and `loadLargeFixture` search with caller/event context disabled.
- Keep the existing two warm-ups, 21 samples, and rotating three-condition order. Run requests serially and finish builds/index preparation before sampling.
- Historical process setup: one temporary working directory per condition, source at `src/large_fixture.ts`, `CODEMAP_HOME` set to that directory, and `.codemap/config.toml` containing `[update] config_auto_update=false` plus `[tool_output] is_redact_enabled=<condition>`. Write valid TOML with separate table/value lines; keep the other defaults identical. Launch the supplied binary with the `mcp` subcommand and line-delimited UTF-8 JSON-RPC over stdin/stdout.
- Historical readiness: call `initialize` with protocol version `2024-11-05`, empty capabilities, and a benchmark client identity, then poll `overview` for that file until `loadLargeFixture` is returned without `warming up` (historically every 50ms, at most 60s). Read the full file and verify the eleven-match const count before the two warm-up passes. Record these preparation calls separately because they can warm a later redaction cache; do not include them in first-miss measurements.
- Historical sample order: for each of 21 rounds, visit the five operations in captured order; for each operation visit all three conditions starting at `(round_index + shift) % 3`. Keep each server alive for its entire population. Compute p95 as sorted sample at `ceil(n * 0.95) - 1`; also retain raw acquisition order in the new harness.
- Add a separate rich-view population for the runtime parallelism work; do not merge its timings into the historical source-only population.
- Record first response after index readiness separately from an actual redaction-cache miss. Background prewarming can make the first request a hit; use verified trace/test state to label it.
- Tests and performance verification are authorized by the user's earlier test request and current optimization request. Keep timing assertions out of ordinary correctness tests.

## Related Files / Entry Points
- `apps/codemap-search/tests/e2e/redact/large_file.rs` — start at the canonical secret cases and generator before extracting shared fixture data.
- `apps/codemap-search/tests/e2e/helpers.rs` — preserve CLI stdio framing, isolated `CODEMAP_HOME`, and readiness handling.
- `apps/codemap-search/src/redact/tests.rs` — locate the existing request/config guards before adding ignored phase probes.
- `apps/codemap-search/src/redact/syntax.rs` — read `parse()` and `inspect()` to distinguish parser cost from walking cost.
- `apps/codemap-search/src/redact/rules.rs` — read `detect()` for the context-free rule phase.
- `apps/codemap-search/src/redact/text.rs` — read `detect()` for fallback context cost.
- `apps/codemap-search/scripts/benchmark_redact.py` (proposed) — portable benchmark entry point with explicit binary and output arguments.
- `apps/codemap-search/tests/fixtures/redact/large_cases.json` (proposed) — canonical synthetic case/generation manifest shared with the Rust regression.
- `docs/briefs/evidence/redact-perf/baseline-e5fc441.json` — immutable historical requests, fixture hashes, and raw sample reference.

## Execution Plan
### Stage 1 — Pin the workload and measurement boundaries
- Starts when: Commit `e5fc441` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json` are available; read the parent decision that no fixed ms SLO applies.
- Work: Reconstruct the generator, safe substitutions, timer boundary, output assertions, and first/repeated/post-edit populations from the repository and captured evidence. Specify separate rich-view requests and a deterministic cache-state observation method for later children.
- No-op when: An existing repository-owned harness already reproduces all fixture hashes, request populations, state labels, and sample fields in this brief.
- No-op handoff: Record that proof in `docs/briefs/evidence/redact-perf/01-baseline.md` (proposed) and pass it to Child 02; the parent may bypass the harness edits only after the proof is checked.
- Deliverable: `docs/briefs/evidence/redact-perf/01-baseline.md` (proposed), workload section containing generator inputs, safe substitutions, hashes, exact JSON-RPC requests, measurement boundaries, and state-label rules.
- Verify: `Compare the workload section with the existing Rust generator and captured JSON`; Inputs: `apps/codemap-search/tests/e2e/redact/large_file.rs` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`; Expected: ten unique secret cases, matching two hashes, five historical requests, and explicit separate rich/cold/edit populations.
- Ends when:
  - [ ] Every timed population has a non-empty target and an assertion proving the expected file, row count, or match count was actually returned.
  - [ ] No phase is mislabeled as a cache hit merely because the OS file cache or index is warm.
- Handoff: Stage 2 consumes the workload section at `docs/briefs/evidence/redact-perf/01-baseline.md`.
- Replan when: The historical fixture cannot be reconstructed or a no-change proof fails; stop successors, return to the parent to assign bounded harness correction and re-verification, then recalculate handoffs.

### Stage 2 — Provide the portable harness and phase probes
- Starts when: The workload section at `docs/briefs/evidence/redact-perf/01-baseline.md` is complete.
- Work: Implement the harness and shared fixture manifest, preserve the existing large-file regression, and expose phase measurements through ignored/test-only probes or existing opt-in tracing without secret payloads. Require explicit paths instead of developer-machine paths.
- Deliverable: The executable harness at `apps/codemap-search/scripts/benchmark_redact.py` (proposed), canonical fixture manifest, and the harness/probe invocation section at `docs/briefs/evidence/redact-perf/01-baseline.md`.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests redact::large_file:: -- --nocapture`; Inputs: the regenerated secret-bearing 10,000-line fixture and existing JSON-RPC assertions; Expected: one executed test passes with all ten secret values absent from enabled output and original source unchanged.
- Ends when:
  - [ ] The harness accepts `--binary`, `--output`, `--rounds`, and `--warmups`; defaults and commands are documented in the handoff.
  - [ ] Reports include raw samples, median/p95, response bytes, effective conditions, fixture/binary hashes, revision, OS/CPU/compiler identity, startup/index times, and state labels or an explicit unavailable reason.
  - [ ] Each ignored phase probe has a named invocation and nonzero executed-case count; public detector behavior and CLI schemas are unchanged.
- Handoff: Stage 3 receives the runnable harness and probes documented at `docs/briefs/evidence/redact-perf/01-baseline.md`.
- Replan when: Cross-language fixture duplication, a new dependency, or a public debug endpoint is required; return to the parent for a narrower test-only route.
- Worker decision: Choose the smallest shared-data representation and standard-library report format that preserves exact fixture bytes and the existing regression assertions.

### Stage 3 — Capture the reproducible baseline
- Starts when: Stage 2's harness and probes execute on the fixture without changing its hashes.
- Work: Build the release binary, reproduce the three-condition baseline, capture phase samples, and add first-request/post-edit and rich-view reference populations. Keep historical and new measurements separate.
- Deliverable: `docs/briefs/evidence/redact-perf/01-baseline.md` containing revision/environment, exact commands, fixture hashes, raw-result paths, phase definitions, output checks, rich-view/cold/edit populations, variability, and unresolved limits.
- Verify: `cargo build --manifest-path apps/codemap-search/Cargo.toml --release && python3 apps/codemap-search/scripts/benchmark_redact.py --binary apps/codemap-search/target/release/codemap-search --output docs/briefs/evidence/redact-perf/baseline-current.json --rounds 21 --warmups 2`; Inputs: both generated fixtures in isolated repositories with the three effective configs; Expected: build exits zero, fifteen historical operation/condition cells each contain 21 samples, off full-read output contains the ten values, all enabled secret-bearing outputs contain none of them, and enabled clean full-read output reconstructs the original clean file.
- Ends when:
  - [ ] Phase results separate parsing, syntax inspection, rules/fallback, rendering, and end-to-end request time without presenting summed overlapping durations as wall time.
  - [ ] The handoff records how to rerun the original binary in an isolated checkout and how to compare later revisions on the same host.
- Handoff: Child 02 receives `docs/briefs/evidence/redact-perf/01-baseline.md`; the parent confirms this evidence before opening the optimization waves.
- Replan when: Output checks fail, samples overlap build/index work, or the baseline method cannot be reproduced; stop successors and return to the parent for bounded correction, re-verification, and dependency recalculation.

## Side Effect Checkpoints
- [ ] `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests redact::large_file:: -- --nocapture` still verifies original bytes, ten secrets, safe neighbors, line numbers, and count semantics after fixture extraction.
- [ ] Harness processes and temporary repositories are isolated, terminated/reaped, and do not read the developer's global config.
- [ ] The clean fixture produces no markers and no byte changes beyond line-number framing; an empty/error response cannot satisfy that assertion.
- [ ] New probes are ignored/opt-in and do not add ordinary test timing thresholds or production output fields.

## Acceptance Criteria
- [ ] A worker with only this checkout can run the documented commands and reproduce every primary condition without the original temporary artifacts.
- [ ] `docs/briefs/evidence/redact-perf/01-baseline.md` contains the complete input/provenance/result contract and non-empty evidence for all three conditions and five historical operations.
- [ ] First/repeated/post-edit, rich-view, and startup/index/resource observations are separately identifiable; missing observations are not recorded as passes.
- [ ] No fixed ms target has been introduced, and no runtime optimization has been implemented in this child.

## Open Questions
- None — The user selected improvement/regression verification without a fixed millisecond SLO; remaining implementation choices are specified in this brief.
