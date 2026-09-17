# [perf] Reuse completed redaction across requests safely

## Work Type
perf

## Current State (As-Is)
- [confirmed] The inspected implementation is commit `e5fc44172faaa87a8e0fd358d160bfe881d5e47b` on 2026-09-17 — Evidence: `git rev-parse HEAD` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`.
- [confirmed] `SourceScan` retains detection ranges and literal ranges without raw detected values, but no cross-request cache exists — Evidence: `apps/codemap-search/src/redact.rs`, `Detection` and `SourceScan`.
- [confirmed] Search reuse ends with a `RenderSource` instance — Evidence: `apps/codemap-search/src/tools/search/render.rs`, its request-owned `OnceCell` fields.
- [confirmed] Config reload replaces the resolved `Arc`, while a request pins its previous snapshot — Evidence: `apps/codemap-search/src/config.rs`, `reload_from_paths()`, `pin_request()`, and `get()`.
- [confirmed] Separately reread grep hits are withheld when their source stamp or text does not agree — Evidence: `apps/codemap-search/src/tools/grep.rs`, `redact_hits()`.
- [inferred] Reusing completed positive and negative results can remove most repeated-request analysis cost — Confirm with Child 01's harness, full source/policy identity checks, and the cache evidence defined below.

## Baseline Measurement
- Reference the captured release measurement at `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`: 10,000 lines, 439,058 bytes, ten synthetic secrets, two warm-up calls and 21 measured calls per operation/condition with rotating condition order. The timer covers stdin write/flush through the complete stdout frame, excluding client JSON decoding, startup, and indexing.
- Median milliseconds for `off_with_secrets / on_with_secrets / on_without_secrets`: full read `2.989 / 36.897 / 37.295`; one-line read `0.936 / 34.664 / 34.397`; plain grep `3.501 / 36.935 / 37.457`; callable grep `73.716 / 142.082 / 140.566`; search `7.041 / 40.641 / 40.508`.
- Treat these as historical measurements, not portable latency limits or proof that parsing alone consumes the delta. The captured run has no hardware/compiler inventory and no redaction cache; reproduce the baseline with Child 01's harness and record the current environment.
- Target a reproducible reduction in repeated-request masking overhead while retaining correctness. The user chose no fixed millisecond SLO. Report median/p95 and first-request, post-edit, index-startup, and resource effects separately; unresolved repeatable regressions return to the parent.
- Use `docs/briefs/evidence/redact-perf/02-shared-analysis.md` as the immediate pre-cache baseline as well as the original baseline. Show incremental cache benefit separately from Child 02's duplicate removal.

## Desired Outcome (To-Be)
- Identical current presentation bytes and effective policy reuse completed detection metadata across requests, including completed zero-detection results.
- File edits, path/parser differences, rule/exception changes, disabled/enabled transitions, and transient failures cannot reuse an incompatible safe result.
- Cache memory is bounded and eviction only causes a fresh complete scan.

## Scope
### In Scope
- Process-local cache ownership, identity, completed-result publication, same-key in-flight coordination, and eviction.
- Wiring cache lookup into the request analysis contract delivered by Child 02.
- Deterministic freshness, negative-result, policy-change, representation, failure, and eviction tests.
### Out of Scope
- [hard] Persisting redaction metadata or raw source in Tantivy, changing extraction/schema versions, or writing an on-disk secret cache.
- [hard] Treating mtime/length, watcher notification, a path-only key, or a failed scan as proof that a file is clean.
- [hard] Worker scheduling and index-time prewarming; Child 04 consumes this cache API.
- [deferred] Cross-process/distributed cache sharing and a public cache-control/configuration API.

## Constraints
- Preserve MCP-only masking: ordinary `parse` CLI output, source files, persisted indexes, matching, ranking, counts, and public JSON-RPC schemas retain their current contracts.
- Preserve `[tool_output].is_redact_enabled`, the three `[redact]` lists, repo/global/default precedence, exact rule/value exceptions, and request-pinned configuration. Add no user-facing configuration keys or schema migration for this work.
- Add no redaction byte, candidate-count, or time scan limits. Retain `parser::parse_source`'s existing 5000ms deadline, existing tool input/output limits, and complete text fallback. Cache eviction or worker saturation must never mean skipped inspection.
- Detect against the complete original presentation source before clipping, escaping, or truncation. Preserve UTF-8 byte coordinates, BOM/CRLF handling, multiline values, PEM interiors, and conservative changed/unavailable-source behavior.
- Keep the final JSON-RPC response guard, contextual named-string handling, and existing treatment of parent objects, arrays, numeric values, and booleans. Never release an uninspected response while work is pending.
- Use only synthetic credentials in fixtures and measurement output. Do not log source text, secret values, exception values, or credential-bearing query strings in profiling records.
- Cache completed ranges/rule IDs/kinds, necessary literal/line metadata, and identity only; retain raw source and parsed-tree lifetimes within the active request/job.
- Derive cache identity from canonical source identity, the exact bytes used for presentation, selected grammar/file-kind and representation, and the effective built-in/custom rules and exceptions. An epoch may be conservative, but must never ignore a redaction-relevant change.
- Re-read/validate permitted source before use and compare its content identity; watchers may accelerate invalidation but are not its correctness basis. Preserve filesystem permission checks on every request.
- Do not cache disabled mode as a clean analysis. Do not publish transient parser failure, timeout, panic, partial work, or unavailable input as a successful negative result. Stable unsupported-language full-text analysis needs an explicit mode identity.
- Do not hold a cache lock while reading, parsing, scanning, rendering, or waiting for a worker. Same-key waiters must be released on both success and failure.
- Use an internal measured memory/entry budget and deterministic eviction; it is a storage limit, not an inspection limit. Oversized/evicted entries are inspected normally without being retained.

## Related Files / Entry Points
- `apps/codemap-search/src/redact/cache.rs` (proposed) — cache identity, publication, in-flight ownership, and bounded retention.
- `apps/codemap-search/src/redact/analysis.rs` (proposed) — consume Child 02's finalized request-analysis interface.
- `apps/codemap-search/src/redact.rs` — keep completed result semantics and the disabled fast path explicit.
- `apps/codemap-search/src/config.rs` and `apps/codemap-search/src/config/redact.rs` — identify effective rule/exception/config generations without changing public settings.
- `apps/codemap-search/src/tools/grep.rs` — retain matched-buffer and reread race checks when lookup hits.
- `apps/codemap-search/src/workspace.rs` — preserve canonicalization and per-request permission checks ahead of cached data use.
- `apps/codemap-search/src/tools/search/render.rs` — retain exact current-source literal provenance on cache hits.
- `apps/codemap-search/src/redact/tests.rs` and `apps/codemap-search/tests/e2e/redact.rs` — add deterministic freshness/negative/policy/cache-consumer cases.
- `docs/briefs/evidence/redact-perf/02-shared-analysis.md` (proposed) — source/policy/completion contract and immediate baseline consumed by this child.

## Execution Plan
### Stage 1 — Define cache identity and publication invariants
- Starts when: `docs/briefs/evidence/redact-perf/02-shared-analysis.md` provides the integrated source/policy/completion contract, preserved-output evidence, and parse/scan counts.
- Work: Map every detector input into cache identity, classify cacheable completion states, and define ownership, failure cleanup, memory accounting, eviction, and exact-source revalidation. Confirm which config changes invalidate entries.
- No-op when: Existing cache behavior already satisfies every positive/negative, identity, failure, policy, and bounded-retention invariant with current evidence.
- No-op handoff: Put the checked cache API and evidence in `docs/briefs/evidence/redact-perf/03-cache.md` (proposed) for Child 04 and skip only the redundant implementation stages.
- Deliverable: `docs/briefs/evidence/redact-perf/03-cache.md` (proposed), contract section with key fields, completion states, policy identity, in-flight cleanup, cacheable data, memory bound, and lookup/publication API.
- Verify: `Compare cache identity fields with every detector input and request-analysis identity`; Inputs: Child 02's contract plus rules, exceptions, grammar/filename handling, decoding, and permissions; Expected: no omitted redaction-relevant input and no partial/failed negative publication route.
- Ends when:
  - [ ] Same bytes under Python versus ENV/plain-text handling and transformed versus original source have distinct compatible identities where needed.
  - [ ] A late result from an older source/policy cannot become the current result for a newer request.
- Handoff: Stage 2 receives the invariant/API section at `docs/briefs/evidence/redact-perf/03-cache.md`.
- Replan when: Correct identity requires new public settings, persistent secret storage, or sharing a transformed tree as original source; return to the parent to narrow the cache design.
- Worker decision: Choose bounded retention/accounting and a conservative generation strategy from measured metadata sizes; record the tradeoff without creating new inspection limits.

### Stage 2 — Implement completed-result reuse
- Starts when: The invariant/API section at `docs/briefs/evidence/redact-perf/03-cache.md` is complete and Child 02's source/policy interfaces are unchanged.
- Work: Implement the cache and request-analysis lookup/publication flow, including completed empty results, same-key coordination, error cleanup, and eviction. Preserve the existing disabled fast path; when masking is enabled, use the full-scan path for misses, unusable entries, and unsupported cache admission.
- Deliverable: Integrated cache API and deterministic cache probes recorded in `docs/briefs/evidence/redact-perf/03-cache.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml && cargo test --manifest-path apps/codemap-search/Cargo.toml --lib redact::tests`; Inputs: new positive/negative reuse, same-size/same-mtime edit, same-bytes/different-language, config replacement, eviction, and failed-publication cases; Expected: hits avoid another scan, incompatible inputs rescan, every waiter terminates, and no failure becomes a clean hit.
- Ends when:
  - [ ] A second unchanged request reuses metadata without retaining raw secrets in the cache.
  - [ ] Content and custom rule/exception changes cannot reuse the old result even when size/mtime or watcher timing would otherwise match.
  - [ ] Tests force eviction with a known non-empty population and prove the evicted entry is rescanned rather than omitted.
- Handoff: Stage 3 receives the integrated cache and recorded invariant tests at `docs/briefs/evidence/redact-perf/03-cache.md`.
- Replan when: Any path bypasses source permissions, source identity validation, failure cleanup, or required inspection; stop and fix this child's publication/lookup logic before continuing.

### Stage 3 — Measure hits and prove miss safety at consumers
- Starts when: Stage 2's cache tests pass.
- Measurement procedure: In addition to the primary command below, execute the exact state/rich/phase invocations recorded in `docs/briefs/evidence/redact-perf/01-baseline.md`; give each revision, cache state, and serial/parallel variant its own result path and record the commands in this child's handoff. Never overwrite the original baseline or an earlier child's raw samples.
- Work: Run read, grep, search, metadata/error, and disabled-mode RPC coverage. Measure repeated hits, actual first misses, same-size edits, policy changes, and eviction misses with separately reported cache state.
- Deliverable: `docs/briefs/evidence/redact-perf/03-cache.md` containing the final API, memory/eviction choice, fixture and generation tests, cache hit/miss/scan evidence, comparison result paths, and residuals.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests redact:: && cargo build --manifest-path apps/codemap-search/Cargo.toml --release && python3 apps/codemap-search/scripts/benchmark_redact.py --binary apps/codemap-search/target/release/codemap-search --output docs/briefs/evidence/redact-perf/03-cache.json --rounds 21 --warmups 2`; Inputs: the two equal-size fixtures in all three primary conditions and deterministic mutation/policy populations; Expected: raw-secret absence remains, repeated unchanged enabled requests avoid full rescans, and changed inputs demonstrably miss and are remasked.
- Ends when:
  - [ ] Repeated-request improvement is measured separately for secret-bearing and clean inputs, not inferred from hit counters alone.
  - [ ] First/post-edit costs, memory retention, and off-mode behavior are reported; unexplained repeatable regressions are routed back before publication.
- Handoff: Child 04 receives `docs/briefs/evidence/redact-perf/03-cache.md` and the completed cache API; Child 05 later consumes its correctness and measurement evidence.
- Replan when: A stale value escapes, a clean result is reused after failed inspection, or a reported speedup depends on skipped work; stop successors, correct this child's owned behavior, then rerun the affected proof and recalculate parent handoffs.

## Side Effect Checkpoints
- [ ] Test file replacement, deletion/unavailability, same-length edits with preserved timestamps, and changed decoding/BOM treatment against actual consumer output.
- [ ] Test built-in policy, added custom regex/field, exception addition/removal, and off-to-on transitions using deterministic request snapshot changes; distinguish watcher delivery from cache correctness.
- [ ] Keep ongoing requests on their pinned policy while the next request uses the new policy; an old background completion cannot overwrite that distinction.
- [ ] Verify original grep match counts, column caps, search literal provenance, and response guards on hits as well as misses.
- [ ] Confirm memory remains bounded under a non-empty multi-file/key workload, locks are not held during analysis, and eviction preserves full inspection.

## Acceptance Criteria
- [ ] `docs/briefs/evidence/redact-perf/03-cache.md` demonstrates safe completed-result reuse for both positive and negative cases with no raw-source cache persistence.
- [ ] Deterministic edit/policy/language/failure/eviction cases prove freshness at the returned JSON-RPC response, not only at an internal invalidation flag.
- [ ] Measured repeated-request masking overhead improves relative to the same-host pre-cache state for both enabled conditions; any repeatable regression has a resolved or user-accepted disposition recorded by the parent.
- [ ] The cache contract supports future worker completion without requiring a worker to read mutable global or thread-local request settings.

## Open Questions
- None — The user selected improvement/regression verification without a fixed millisecond SLO; remaining implementation choices are specified in this brief.
