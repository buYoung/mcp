# [perf] Share request analysis and remove repeated scans

## Work Type
perf

## Current State (As-Is)
- [confirmed] The inspected implementation is commit `e5fc44172faaa87a8e0fd358d160bfe881d5e47b` on 2026-09-17 — Evidence: `git rev-parse HEAD` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`.
- [confirmed] Callable grep scans the same file in `redact_hits()` and again in `ExpansionPage::add_file()` — Evidence: `apps/codemap-search/src/tools/grep.rs` and `apps/codemap-search/src/tools/grep/expansion.rs`.
- [confirmed] `callable::bounds()` parses directly and then calls extraction, whose `extract_language_parts()` parses again — Evidence: `apps/codemap-search/src/tools/live_symbols/callable.rs` and `apps/codemap-search/src/parser/mod.rs`.
- [confirmed] `read_file_with_metadata()` masks the full buffer before selecting a line window — Evidence: `apps/codemap-search/src/tools/read.rs`, `redact::in_file` followed by `displayed_lines`.
- [confirmed] `RenderSource` already caches source, scan, and masked text within one file render — Evidence: `apps/codemap-search/src/tools/search/render.rs`; preserve this reuse while joining the common analysis path.
- [confirmed] Request config and MCP activation live in thread-local storage — Evidence: `apps/codemap-search/src/config.rs`, `REQUEST_CONFIG` and `pin_request()`, and `apps/codemap-search/src/redact.rs`, `IS_MCP_RESPONSE` and `is_enabled()`.
- [confirmed] Rich context can use test-filtered source and the resolver owns `Rc`/`RefCell` state — Evidence: `apps/codemap-search/src/tools/live_symbols/structure.rs`, `Outline::new()`, and `apps/codemap-search/src/callers/resolution.rs`, `SourceResolver` and `source()`.

## Baseline Measurement
- Reference the captured release measurement at `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`: 10,000 lines, 439,058 bytes, ten synthetic secrets, two warm-up calls and 21 measured calls per operation/condition with rotating condition order. The timer covers stdin write/flush through the complete stdout frame, excluding client JSON decoding, startup, and indexing.
- Median milliseconds for `off_with_secrets / on_with_secrets / on_without_secrets`: full read `2.989 / 36.897 / 37.295`; one-line read `0.936 / 34.664 / 34.397`; plain grep `3.501 / 36.935 / 37.457`; callable grep `73.716 / 142.082 / 140.566`; search `7.041 / 40.641 / 40.508`.
- Treat these as historical measurements, not portable latency limits or proof that parsing alone consumes the delta. The captured run has no hardware/compiler inventory and no redaction cache; reproduce the baseline with Child 01's harness and record the current environment.
- Target a reproducible reduction in repeated-request masking overhead while retaining correctness. The user chose no fixed millisecond SLO. Report median/p95 and first-request, post-edit, index-startup, and resource effects separately; unresolved repeatable regressions return to the parent.
- Consume the phase breakdown and reproduced baseline at `docs/briefs/evidence/redact-perf/01-baseline.md` before editing. Verify the duplicate-scan hypothesis with counts, not only the approximately doubled callable-grep delta.

## Desired Outcome (To-Be)
- One request reuses one completed detection result per identical source/parser/policy identity across its source rows, expansion, previews, and search labels.
- Consumers reuse a parsed tree when their original bytes and parser identity match; filtered/composite/preprocessed representations retain distinct identities.
- Pure detector work receives an immutable effective policy explicitly and is safe to call outside the MCP thread-local scope.
- Requested ranges are rendered from complete-source detection spans without allocating an unnecessary full masked copy for a one-line read.

## Scope
### In Scope
- Request-owned source/analysis identity and explicit detector policy.
- Atomic migration of read, grep, callable expansion, declaration/constant previews, and search rendering to the shared result.
- Internal parser entry points accepting an already parsed compatible tree while preserving the existing extraction entry points.
- Scan/parse count assertions and output-equivalence coverage for these consumers.
### Out of Scope
- [hard] Cross-request cache storage and runtime worker scheduling; those belong to Children 03 and 04.
- [hard] Replacing original-source analysis with a test-filtered, composite, macro-expanded, truncated, or differently decoded tree.
- [deferred] Broad caller-resolution algorithm changes beyond accepting compatible analysis inputs.

## Constraints
- Preserve MCP-only masking: ordinary `parse` CLI output, source files, persisted indexes, matching, ranking, counts, and public JSON-RPC schemas retain their current contracts.
- Preserve `[tool_output].is_redact_enabled`, the three `[redact]` lists, repo/global/default precedence, exact rule/value exceptions, and request-pinned configuration. Add no user-facing configuration keys or schema migration for this work.
- Add no redaction byte, candidate-count, or time scan limits. Retain `parser::parse_source`'s existing 5000ms deadline, existing tool input/output limits, and complete text fallback. Cache eviction or worker saturation must never mean skipped inspection.
- Detect against the complete original presentation source before clipping, escaping, or truncation. Preserve UTF-8 byte coordinates, BOM/CRLF handling, multiline values, PEM interiors, and conservative changed/unavailable-source behavior.
- Keep the final JSON-RPC response guard, contextual named-string handling, and existing treatment of parent objects, arrays, numeric values, and booleans. Never release an uninspected response while work is pending.
- Use only synthetic credentials in fixtures and measurement output. Do not log source text, secret values, exception values, or credential-bearing query strings in profiling records.
- Keep request analysis and its call-site migration atomic. Do not land an interface-only state in which a consumer can bypass masking.
- Resolve the effective rule catalog once per request instead of repeatedly cloning global/request config inside each AST-node check.
- Keep `CodeExtractor::extract`, existing indexed extraction behavior, `ExtractedLiteral`'s persisted shape, and the extraction format version compatible; add internal adapters rather than a public contract break.
- Share parser work only with an identity that includes the actual presentation bytes, decoding/BOM treatment, selected grammar/extension, and source representation. Respect existing callable/composite eligibility and input caps independently of redaction.
- A failed or incomplete parse must preserve complete text fallback and stale-literal suppression; zero detections must not be confused with skipped work.

## Related Files / Entry Points
- `apps/codemap-search/src/redact.rs` — start at `SourceScan`, policy lookup, rendering, and final-response helpers.
- `apps/codemap-search/src/redact/syntax.rs` — separate obtaining a tree from inspecting a compatible tree.
- `apps/codemap-search/src/redact/rules.rs` and `apps/codemap-search/src/redact/text.rs` — thread immutable policy through detection hot paths.
- `apps/codemap-search/src/redact/analysis.rs` (proposed) — request-owned source identity, parsed input, and completed detection sharing.
- `apps/codemap-search/src/config.rs` and `apps/codemap-search/src/mcp/mod.rs` — capture request policy once at the MCP boundary while retaining the current guard contract.
- `apps/codemap-search/src/parser/mod.rs` — extract the language walk behind an internal compatible-tree adapter.
- `apps/codemap-search/src/parser/bounded.rs` — preserve the existing parse deadline and reset/cancellation behavior.
- `apps/codemap-search/src/tools/read.rs` and `apps/codemap-search/src/tools/grep.rs` — retain original matching/selection while acquiring shared analysis.
- `apps/codemap-search/src/tools/grep/expansion.rs` and `apps/codemap-search/src/tools/live_symbols/callable.rs` — remove repeated source parsing/scanning.
- `apps/codemap-search/src/tools/live_symbols/structure.rs` and `apps/codemap-search/src/tools/live_symbols/references.rs` — keep filtered representations distinct and reuse original detection for previews.
- `apps/codemap-search/src/declarations.rs` and `apps/codemap-search/src/tools/search/render.rs` — route signatures and exact literal ranges through the shared analysis.
- `apps/codemap-search/src/tools/search/mod.rs` — preserve masking of anchor maps, qualified-literal hints, tails, and empty-query guidance.
- `apps/codemap-search/src/redact/tests.rs` and `apps/codemap-search/tests/e2e/redact.rs` — extend the existing unit and final-response assertions for shared-analysis counts and compatible-tree reuse.
- `docs/briefs/evidence/redact-perf/01-baseline.md` (proposed) — consume the verified workload, profiler route, and baseline evidence.

## Execution Plan
### Stage 1 — Stabilize the reusable analysis contract
- Starts when: `docs/briefs/evidence/redact-perf/01-baseline.md` contains the verified fixture/method/provenance and phase results required by Child 01.
- Work: Trace the listed consumers and define immutable source, parser, policy, and completion identities. Confirm compatible-tree ownership and the exact transformed-source boundaries before choosing the reusable interface.
- No-op when: Instrumented requests already perform no repeated full redaction scan or compatible parse and every consumer receives an explicit identical policy.
- No-op handoff: Record that proof in `docs/briefs/evidence/redact-perf/02-shared-analysis.md` (proposed) with the existing interface and completed checks; Child 03 consumes it without forcing a rewrite.
- Deliverable: `docs/briefs/evidence/redact-perf/02-shared-analysis.md` (proposed), contract section describing concrete API/types, source/parser identity, policy ownership, completion/fallback states, consumer mapping, and supported thread ownership.
- Verify: `Inspect the consumer map and run the Child 01 phase/count probes`; Inputs: full/one-line reads, both grep modes, source/rich views, search labels, and original versus filtered representations; Expected: a complete caller-to-consumer route with duplicate counts pinned and unsafe sharing cases excluded.
- Ends when:
  - [ ] Workers can distinguish completed clean analysis, completed detections, stable text-only support, and transient parse/failure paths.
  - [ ] Thread ownership is confirmed from the actual types/compiler; no unsafe `Send` or `Sync` assertion is proposed.
- Handoff: Stage 2 receives the contract section at `docs/briefs/evidence/redact-perf/02-shared-analysis.md`.
- Replan when: Sharing requires changing persisted extraction, language semantics, existing caps, or source identity; return to the parent to narrow the adapter and re-verify the contract before dependent work.

### Stage 2 — Integrate one analysis per request source
- Starts when: Stage 1 has named the compatible input and policy contract at `docs/briefs/evidence/redact-perf/02-shared-analysis.md`.
- Work: Implement the shared request analysis, policy-parameterized detector core, compatible-tree adapter, and all consumer migrations together. Preserve caller options and create requested presentation slices from the original byte spans.
- Deliverable: Integrated source-to-output reuse plus the consumer/API section of `docs/briefs/evidence/redact-perf/02-shared-analysis.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml && cargo test --manifest-path apps/codemap-search/Cargo.toml --lib redact::tests`; Inputs: changed analysis adapters and existing/new request-local reuse cases; Expected: zero build failures, unchanged masking outcomes, and exactly one completed full detection per compatible request source/policy identity.
- Ends when:
  - [ ] Callable grep reuses the scan between hits and expanded bodies, including fallback groups when expansion is unavailable.
  - [ ] Compatible parsing is reused; different source representations remain independent and are recorded as such.
  - [ ] Standalone errors/final responses retain their own required checks rather than being incorrectly treated as file-cache hits.
- Handoff: Stage 3 receives the integrated consumers and count instrumentation recorded in `docs/briefs/evidence/redact-perf/02-shared-analysis.md`.
- Replan when: A consumer requires a second scan because its bytes/policy differ; document that identity and return to Stage 1 instead of forcing unsafe reuse or suppressing output checks.

### Stage 3 — Verify preserved output and measured work reduction
- Starts when: Stage 2 passes build and request-analysis checks.
- Measurement procedure: In addition to the primary command below, execute the exact state/rich/phase invocations recorded in `docs/briefs/evidence/redact-perf/01-baseline.md`; give each revision, cache state, and serial/parallel variant its own result path and record the commands in this child's handoff. Never overwrite the original baseline or an earlier child's raw samples.
- Work: Run the complete redaction RPC regression, relevant extraction/callable checks selected from actual changed adapters, and the same three-condition release benchmark. Compare output content, counts, caps, parse/scan counts, and end-to-end samples.
- Deliverable: `docs/briefs/evidence/redact-perf/02-shared-analysis.md` containing final interfaces, compatible/isolated representations, counts before/after, exact verification commands/results, performance comparison paths, and residuals.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests redact:: && cargo build --manifest-path apps/codemap-search/Cargo.toml --release && python3 apps/codemap-search/scripts/benchmark_redact.py --binary apps/codemap-search/target/release/codemap-search --output docs/briefs/evidence/redact-perf/02-shared-analysis.json --rounds 21 --warmups 2`; Inputs: both 10,000-line variants and the new sharing probes; Expected: all executed redaction cases pass, original matching/counts remain, and the duplicated full-scan work is removed in the eligible paths.
- Ends when:
  - [ ] Side-effect checkpoints have evidence and the measured result is separated from inferred remaining bottlenecks.
  - [ ] Any repeatable unexplained slowdown or changed source/metadata output is resolved or routed back to the parent before handoff.
- Handoff: Child 03 receives `docs/briefs/evidence/redact-perf/02-shared-analysis.md` as the source/policy/result contract; Child 04 will use the same contract after cache integration.
- Replan when: Count reduction does not occur, redaction coverage changes, or baseline comparison fails; stop successors and correct this child's owned integration before re-verification.

## Side Effect Checkpoints
- [ ] Existing redaction unit and RPC populations still cover multiline/interior reads, malformed syntax fallback, original UTF-8/CRLF coordinates, safe same-line literals, custom exceptions, search hints/tails, errors, and disabled masking.
- [ ] Original grep matching, pagination, column widths, read byte ceilings, callable boundaries, and source-row numbering remain unchanged.
- [ ] Source/test exclusions, composite grammar selection, native preprocessing, and index/literal serialization retain their previous representations; no filtered tree proves an unfiltered file safe.
- [ ] CLI `parse` still returns original fixture values outside MCP while enabled MCP responses hide them.
- [ ] Request-pinned custom rules and exceptions are the same throughout detection, previews, formatting, and the final guard.

## Acceptance Criteria
- [ ] The complete consumer map in `docs/briefs/evidence/redact-perf/02-shared-analysis.md` uses the new contract without a detached or uninspected output path.
- [ ] Eligible requests have one completed full scan per exact source/parser/policy identity and no redundant compatible parser invocation introduced by the migration.
- [ ] Whole-work regression and performance evidence establishes preserved contracts and the work reduction; tests merely compiling is insufficient.
- [ ] The policy/result contract is explicit enough for cache and worker ownership without consulting thread-local request state from a future worker.

## Open Questions
- None — The user selected improvement/regression verification without a fixed millisecond SLO; remaining implementation choices are specified in this brief.
