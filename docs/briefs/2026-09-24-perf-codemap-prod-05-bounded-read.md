# [perf] Bound read input without weakening masking

## Work Type
perf

## Current State (As-Is)
- [confirmed] At revision `8b8222003584a3875ce3804557d3d5e70113ea4b`, `read_file_impl()` bypasses the 256 KiB no-window admission check when `limit` is supplied, then reads the complete file — Evidence: `READ_FILE_BYTE_CAP` and `std::fs::read(&resolved)` in `apps/codemap-search/src/tools/read.rs`.
- [confirmed] The read path builds both original and masked complete line vectors before selecting the requested window — Evidence: `all_lines`, `displayed_lines`, and `window` in `read_file_impl()`.
- [confirmed] Complete-source masking occurs before clipping to preserve context and multiline values — Evidence: `redact::in_file()` and `SourceScan::new()` in `src/redact/source.rs` and `src/redact/detection.rs`.
- [confirmed] Callable input and output limits are separate existing contracts — Evidence: `tools::live_symbols::callable::input_byte_cap()` and `read_output_byte_cap`.
- [inferred] A one-line read of a large regular file can retain input/line data proportional to the entire file and delay all sequential MCP requests — Confirm with bounded release fixtures and allocation/input-byte counters below.

## Baseline Measurement
- No release memory baseline was recorded by the audit. Build the immutable pre-initiative revision `8b8222003584a3875ce3804557d3d5e70113ea4b` separately from the candidate; never time debug binaries or treat the earlier test durations as a baseline.
- Measure `read` with `view=source`, `expand=none`, `offset=1`, and `limit=1`, plus a 200-line window near EOF, on synthetic 1/4/8/16/32 MiB text files. Use a separate safely bounded stress process for larger inputs; record baseline timeouts/resource failures without exhausting the host.
- Exercise the existing large-file secret fixture, multiline credentials/PEM interiors, BOM/CRLF, invalid UTF-8, a long single line, empty input, offset beyond EOF, callable expansion, and all argument aliases.
- Record actual input bytes read, largest input reservation, retained line entries, peak live allocation where measurable, peak RSS delta, response bytes, masking assertions, and exact errors. RSS alone is not proof of an allocation leak or bound.
- Freeze fixture hashes and requests before candidate timing. Use identical release toolchains/hosts/configs, isolated `CODEMAP_HOME`, two warm-ups and 100 measured calls per operation in each of three paired runs; retain raw samples and nearest-rank p95.
- Compare latency only for ordinary positive requests admitted by both revisions with equivalent delivered content. Keep newly refused over-cap requests in the separate admission/resource population; an error, empty result, shorter window, or weaker masking is not a speedup.
- Target: admitted raw input is bounded by a frozen finite cap plus one detection byte, rejected larger files do not trigger whole-file parsing/masking, no complete-file line-vector duplication remains, and ordinary-request p95 is at most 1.10 times baseline.

## Desired Outcome (To-Be)
- A small requested output cannot cause unbounded file input allocation, even if the file grows after metadata inspection.
- Oversized input is explicitly refused with actionable guidance; the user approved this compatibility change and did not approve weaker masking.
- Accepted input preserves exact source-window, encoding, permission, masking, callable, and Jev contracts.

## Scope
### In Scope
- Bounded read admission and actual opened-handle reads, selected-window storage, refusal diagnostics, and final-consumer verification.
- Exact numeric input-cap selection through measurements, documentation, and boundary tests; use a private limit unless user configuration is demonstrably necessary.
- A focused reproducible measurement driver and redaction/window regressions using the existing test harness.
### Out of Scope
- [hard] Accepting arbitrary-size input by skipping whole-source masking or silently returning an uninspected fragment.
- [hard] Reinterpreting `output.read.max_bytes` or `index.max_file_bytes` as the new admission limit.
- [hard] Executing the independent redaction cache/parallelism initiative in `docs/briefs/2026-09-17-briefset-redact-perf.md`.
- [deferred] Streaming arbitrary-size context-sensitive redaction; this task deliberately uses the user-approved explicit refusal boundary.

## Constraints
- Use Child 02's authorized opened-source contract and the same immutable buffer for parsing, detection, slicing, and Jev capture.
- Freeze an exact finite byte limit before implementation comparisons; evaluate practical candidates such as 8/16/32 MiB against existing valid fixtures and the memory profile, then document the selected value and rationale. Do not leave the cap as a future decision or permit an unlimited sentinel.
- Enforce the cap both before expensive work and while reading from the opened handle. A metadata check alone is insufficient for growing files; avoid unbounded reservation from metadata length.
- Refuse non-regular unbounded streams/devices unless a genuinely bounded nonblocking read contract is established; a FIFO must not hang the sequential MCP server.
- Preserve the existing 256 KiB no-window behavior, 1-based aliases, numeric-string coercion, trailing empty-line convention, lossy UTF-8, BOM/CRLF handling, and explicit over-output-cap errors.
- For accepted input, detect against the complete original presentation source before clipping. Cap rejection is not successful partial analysis and must not claim a file is clean.
- User authorization covers necessary tests/measurement tools and explicit oversize refusal, not new public tool arguments, disabled masking, or lint/formatter setup.
- Run commands from the repository root; use synthetic credentials and isolated homes. Earlier redaction plans are historical context, not evidence that their proposed harness/cache exists.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/read.rs` — start at `read_file_impl()` admission, full-file reading, line vectors, and output cap.
- `apps/codemap-search/src/workspace.rs` — consume the authorized opened-handle contract from Child 02.
- `apps/codemap-search/src/tools/live_symbols/callable.rs` — preserve the distinct callable parsing input bound.
- `apps/codemap-search/src/tools/live_symbols/jev.rs` — keep exact-buffer capture and row coordinates consistent.
- `apps/codemap-search/src/redact/source.rs` — preserve complete-source masking before window selection.
- `apps/codemap-search/tests/e2e/tools.rs` — extend range/admission/growth tests without changing existing expected output.
- `apps/codemap-search/tests/e2e/redact/large_file.rs` — reuse the existing synthetic large-source fixture.
- `apps/codemap-search/scripts/` — add only the focused opt-in measurement entry point needed to reproduce this population.
- `apps/codemap-search/docs/configuration.md` — document exact input versus output limits and refusal recovery.
- `apps/codemap-search/docs/configuration.ko.md` — align the same numeric limits and guidance.

## Execution Plan
### Stage 1 — Measure input retention and freeze admission
- Starts when: `docs/briefs/evidence/codemap-prod/02-path-boundary.md` supplies the verified opened-source API, and the parent's earlier shared documentation/test write windows are closed.
- Work: Capture baseline allocations/input bytes and final outputs, choose and freeze the exact finite admission cap, and pin the accepted/rejected window and masking matrix. Add a reproducible opt-in measurement invocation if none exists.
- No-op when: The current implementation already enforces the complete admission/growth contract and meets every resource, output, masking, and p95 criterion with executed evidence.
- No-op handoff: Record proof in `docs/briefs/evidence/codemap-prod/05-bounded-read.md` (proposed); the parent validates it before allowing Child 06 and Child 08 to consume the unchanged implementation.
- Deliverable: `docs/briefs/evidence/codemap-prod/05-bounded-read.md` (proposed), containing cap bytes, fixture hashes, exact commands/requests, baseline samples/counters, error contract, and source-buffer ownership.
- Verify: `Inspect measured read bytes, reservations, masking assertions, and the admission decision`; Inputs: the fixed size ladder, existing secret fixture, small/EOF windows, growth, and non-regular inputs; Expected: nonempty baseline populations and one exact cap selected before candidate measurements.
- Ends when:
  - [ ] The baseline and selected cap are reproducible and do not depend on unbounded baseline stress.
  - [ ] Accepted-source and refused-source behavior is unambiguous for each boundary case.
- Handoff: Stage 2 consumes the frozen admission/source-buffer contract in `docs/briefs/evidence/codemap-prod/05-bounded-read.md`.
- Replan when: The selected cap cannot admit existing under-cap fixtures safely or measurements do not isolate source retention; stop successors, return to the parent for bounded measurement/admission correction, and re-verify before changing the contract.

### Stage 2 — Integrate bounded source-window handling
- Starts when: Stage 1 has frozen the cap, source-open API, and output assertions.
- Work: Bound input/reservations on the opened handle, remove unnecessary complete-line retention, preserve complete-source detection for admitted files, and return explicit safe refusal for over-cap or unbounded input. Carry the exact buffer and coordinates through callable/Jev consumers.
- Deliverable: Integrated bounded read behavior and the source-buffer/error contract in `docs/briefs/evidence/codemap-prod/05-bounded-read.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: bounded read, authorized source acquisition, and callable/Jev adapters; Expected: exit 0 with an inspected path from the cap through the actual read and final response.
- Ends when:
  - [ ] Metadata races cannot bypass the actual read bound and no input-sized duplicate line vector remains.
  - [ ] Masking completes before accepted output is clipped and refused input never becomes partial success.
- Handoff: Stage 3 receives the integrated cap and unchanged fixture assertions.
- Replan when: Bounded processing would require ignoring a caller signal/permission, skipping context detection, or broadening a public contract; stop and return the affected design to the parent rather than weakening safeguards.
- Worker decision: Use a bounded owned buffer and iterator/range-based window extraction; keep buffer sharing local and preserve exact-source lifetime rather than adding a new general cache.

### Stage 3 — Prove bounds at the MCP output
- Starts when: Stage 2 is integrated and the frozen baseline population is unchanged.
- Work: Run read/redaction/Jev regressions, cap-minus-one/exact/plus-one and growth cases, then repeat paired release measurements. Document exact limits, error guidance, and the fact that smaller output windows cannot bypass input refusal.
- Deliverable: `docs/briefs/evidence/codemap-prod/05-bounded-read.md`, with cap and source-buffer contracts, executed case counts, raw release measurements, final-output comparisons, exact replay commands, and documentation references.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests -- e2e::tools:: e2e::redact:: e2e::jev_live:: --test-threads=1`; Inputs: existing and added admission/window/masking cases; Expected: nonzero selected cases pass, accepted output retains known safe content, and synthetic secrets are absent without relying on an empty/error response.
- Ends when:
  - [ ] Input/read-reservation counters enforce the frozen cap and no oversized input enters parsing or whole-source masking.
  - [ ] Ordinary per-operation p95 is at most 1.10 times baseline and all local side-effect checks pass.
- Handoff: Child 06 and Child 08 receive `docs/briefs/evidence/codemap-prod/05-bounded-read.md`, including buffer ownership, refusal rules, and target replay cases.
- Replan when: Resource counters, source output, masking, or latency fail; correct Stage 2, stop downstream buffer consumers, and re-verify the same population before accepting.

## Side Effect Checkpoints
- [ ] Source/full/definitions/relations views, callable fallback notices, and argument aliases keep their accepted-input semantics.
- [ ] UTF-8 replacement, BOM/CRLF, trailing newline, zero/EOF windows, and single-line output caps behave as documented.
- [ ] Read permission choices and raced-path denial from Child 02 reach the actual bounded file handle.
- [ ] Jev-disabled/missing-provider fallback preserves byte-identical accepted output; filtering does not bypass admission or masking.
- [ ] English/Korean guidance never suggests reducing `limit` as a way around a whole-file input refusal.

## Acceptance Criteria
- [ ] One exact finite input cap is documented, boundary/growth-tested, and applied at the final read consumer, not only to metadata or output.
- [ ] No accepted read holds redundant whole-file line vectors, and over-cap input is rejected without input-proportional parsing/masking allocation.
- [ ] Accepted-output and synthetic-secret regressions pass with known nonempty positive output and intact source files.
- [ ] Paired release measurements meet the 1.10 p95 gate and provide byte/allocation evidence beyond RSS alone.
- [ ] The finalized read/source-buffer contract and all target replay cases are ready for Child 06 and Child 08.

## Open Questions
- None — The user explicitly approved finite-input refusal with unchanged masking; cap selection is a measured bounded implementation decision.
