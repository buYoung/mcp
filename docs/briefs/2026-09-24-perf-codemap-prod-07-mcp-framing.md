# [perf] Read MCP frames with linear bounded work

## Work Type
perf

## Current State (As-Is)
- [confirmed] At revision `8b8222003584a3875ce3804557d3d5e70113ea4b`, `LimitedLineReader::next_line()` reads 1,024 bytes and searches the entire accumulated buffer from its beginning after each read — Evidence: `byte_buf` and `self.buffer.iter().position()` in `apps/codemap-search/src/mcp/protocol.rs`.
- [confirmed] The MCP loop uses a `10 * 1024 * 1024 + 100 * 1024` limit and preserves the pending line future while retention ticks run — Evidence: `McpServer::run_with_io()` in `src/mcp/mod.rs`.
- [confirmed] Existing large-payload and over-limit e2e tests passed during the audit but each emitted a greater-than-60-second test-harness notice; the complete selected e2e run took 183.94 seconds in a debug/test build — Evidence: the recorded audit execution of `test_mcp_huge_payload()` and `test_mcp_oom_mitigation()`.
- [inferred] Repeated prefix scanning is quadratic for a long fragmented frame; the observed debug duration is not a release-only attribution — Confirm by scan-byte counters and isolated release framing measurements.

## Baseline Measurement
- Build baseline revision `8b8222003584a3875ce3804557d3d5e70113ea4b` and candidate release binaries with the same compiler/target. Separate framing-only measurements from JSON parsing and full stdio round trips.
- Use valid UTF-8 JSON-RPC `ping` frames with inert padding at 1/2/4/8/10 MiB, fragmented into 1,024-byte chunks, plus small ordinary frames. Exercise chunk sizes 1, 1,024, and 8,192 for correctness without forcing every tiny-chunk stress case into baseline timing.
- Count bytes examined for delimiters, input bytes, buffer length/capacity, copies, and frame completion. Measure first-frame and sustained streams of multiple frames; preserve unconsumed suffixes.
- For ordinary small-frame latency, use two warm-ups and 100 measured calls in each of three paired runs, no parallel build/index workload, identical isolated homes, raw samples, and nearest-rank p95. Required p95 is at most 1.10 times baseline.
- Target: total delimiter bytes inspected are linear in received bytes independent of fragmentation; input retention is bounded by the existing per-frame limit plus one fixed read chunk, not by the number of frames. Freeze the exact counter inequality before candidate implementation.

## Desired Outcome (To-Be)
- Every input byte is delimiter-scanned only a constant number of times while preserving multiple-frame and partial-read state.
- The existing frame-size limit is enforced during reading, including no-newline and boundary-crossing input.
- Protocol behavior, cancellation-safe partial progress during retention ticks, EOF/CRLF handling, and small-request responsiveness remain intact.

## Scope
### In Scope
- Limited line-reader buffer/search state, bounded frame admission, and tests for fragmentation, coalesced frames, timers, encoding, EOF, and size boundaries.
- Focused scan/allocation counters and release comparison measurements through the actual stdio reader.
### Out of Scope
- [hard] Concurrent MCP request handling, protocol cancellation support, tool business logic, or notification-response changes.
- [hard] Increasing the frame-size limit or replacing JSON-RPC/newline framing with a different transport.
- [hard] Index lifecycle, filesystem authorization, and recovery test files owned by Child 01.

## Constraints
- Keep the existing maximum frame length and public error/envelope behavior unless a boundary test proves an implementation contradicts the existing per-frame contract; document such a correction explicitly.
- Retain progress when the idle-retention timer fires. Do not replace the preserved future with a cancellation-unsafe partially consumed read loop.
- Do not let a `read_until` convenience call allocate beyond the frame budget before checking it.
- Retain bytes after a delimiter for the next frame, including multiple frames delivered in one read. Evaluate the limit per frame, not against unrelated suffix bytes.
- Keep current-thread sequential request execution and thread-local config/redaction guard ownership. Async wrapping is not a reason to add worker threads.
- Use test-only counters or payload-free opt-in measurement. No real task text/credentials may be written to timing reports.
- The user authorized necessary tests/measurement tooling. Run commands from the repository root; serialize timed runs with all other measurement jobs.

## Related Files / Entry Points
- `apps/codemap-search/src/mcp/protocol.rs` — start at `LimitedLineReader::next_line()` and add colocated framing regressions/counters.
- `apps/codemap-search/src/mcp/mod.rs` — verify timer/pending-future integration and the existing maximum length without changing dispatch.
- `apps/codemap-search/tests/e2e/mcp.rs` — preserve and extend the actual stdio large-input cases.
- `apps/codemap-search/tests/e2e/helpers.rs` — reuse framing/client behavior; do not change shared helper APIs during concurrent Child 01 work.
- `apps/codemap-search/scripts/` — colocate a focused opt-in framing measurement driver if needed.

## Execution Plan
### Stage 1 — Pin framing state and measured complexity
- Starts when: The inspected baseline revision is available and Child 01 is restricted to its disjoint index/recovery files.
- Work: Capture valid/over-limit framing behavior, define delimiter-work counters, freeze the frame/chunk populations and linear bound, and measure release baseline latency separately from full tool work.
- No-op when: Current framing already satisfies the complete state/size/counter/latency contract with executed evidence.
- No-op handoff: Record proof in `docs/briefs/evidence/codemap-prod/07-mcp-framing.md` (proposed); the parent validates it and forwards it to Child 08 without implementation edits.
- Deliverable: `docs/briefs/evidence/codemap-prod/07-mcp-framing.md` (proposed), with exact frame/chunk identities, limit interpretation, raw baseline samples, scan/copy counters, linear bound, and commands.
- Verify: `Inspect counted frame traces against LimitedLineReader and run_with_io`; Inputs: the fixed size/chunk ladder, coalesced frames, partial reads with retention ticks, EOF/CRLF, invalid UTF-8, and cap-minus-one/exact/plus-one cases; Expected: every byte/frame population is nonempty and expected frames/errors are recorded before optimization.
- Ends when:
  - [ ] The counter bound and per-frame size interpretation are frozen.
  - [ ] A measured baseline or complete no-change proof distinguishes reader work from JSON/tool processing.
- Handoff: Stage 2 consumes the framing contract in `docs/briefs/evidence/codemap-prod/07-mcp-framing.md`.
- Replan when: Existing EOF/limit semantics cannot be preserved or the suspected cost is not reproduced; stop, return to the parent for bounded framing investigation and re-verification, and recalculate handoffs before continuing.

### Stage 2 — Integrate incremental delimiter scanning
- Starts when: Stage 1 has frozen state, size, and work-count contracts.
- Work: Implement a bounded reader that advances its scan state rather than repeatedly scanning old prefixes. Preserve suffix bytes and pending progress across timers and cap input before excess allocation.
- Deliverable: Integrated bounded linear framing and its state invariants in `docs/briefs/evidence/codemap-prod/07-mcp-framing.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: the line reader and its existing MCP integration; Expected: exit 0 with inspected ownership of incomplete-frame and suffix state across awaits.
- Ends when:
  - [ ] Delimiter scanning advances monotonically until a frame is emitted.
  - [ ] Capacity/length remain within the fixed frame-plus-chunk bound and multiple frames do not lose bytes.
- Handoff: Stage 3 receives the integrated reader and frozen counters/fixtures.
- Replan when: A simpler implementation loses partial progress or allows over-budget allocation; reject it and return to the owning reader stage before accepting results.
- Worker decision: Use an explicit scan cursor or bounded `BufReader` fill/consume state with existing Tokio/std facilities; avoid adding a framing dependency for convenience.

### Stage 3 — Verify linear work through stdio
- Starts when: Stage 2 is integrated and the baseline frame/chunk populations are unchanged.
- Work: Run unit/e2e framing regressions, confirm exact frame/error sequences, and repeat paired release latency/counter measurements. Record peak retained capacity after large frames and continued small-request operation.
- Deliverable: `docs/briefs/evidence/codemap-prod/07-mcp-framing.md`, with exact commands, nonzero case counts, byte/frame assertions, baseline/candidate samples, scan/capacity bounds, and target replay instructions.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --release --test e2e_tests e2e::mcp:: -- --test-threads=1`; Inputs: existing MCP tests and the added frame/chunk population; Expected: selected tests execute and pass, oversized input yields the established explicit error/termination behavior, and admitted streams return every frame exactly once.
- Ends when:
  - [ ] Counted delimiter work meets the frozen linear inequality at every frame/chunk size.
  - [ ] Frame memory bounds and the ordinary small-request 1.10 p95 gate pass.
- Handoff: Child 08 receives `docs/briefs/evidence/codemap-prod/07-mcp-framing.md`; the parent closes this child's MCP-test write window before Child 02.
- Replan when: Any size, byte-preservation, timer, memory, or latency criterion fails; return to Stage 2 for bounded correction and re-verification without increasing the size limit.

## Side Effect Checkpoints
- [ ] Initialize, tools/list, tools/call, ping, parse errors, notifications without responses, and response IDs retain their protocol shapes.
- [ ] EOF without a final newline, CRLF, invalid UTF-8, blank frames, and delimiters crossing reads follow the documented existing handling.
- [ ] Retention ticks do not consume or reset partial frame data, and EOF/disconnect still terminates cleanly.
- [ ] No stdout diagnostics or unbounded retained buffer capacity is introduced.

## Acceptance Criteria
- [ ] Scan-byte counters prove linear delimiter work for the complete nonempty frame/chunk population, not merely faster wall time for one frame.
- [ ] Actual input retention respects the existing frame limit plus one fixed chunk while multiple coalesced frames remain intact.
- [ ] Release e2e framing and ordinary-request p95 at most 1.10 times baseline pass with recorded commands and raw evidence.
- [ ] The exact framing contract and target replay instructions are handed to Child 08 without changing request concurrency or public transport semantics.

## Open Questions
- None — The existing framing limit is preserved and the user approved linear-scaling and 10% p95 validation.
