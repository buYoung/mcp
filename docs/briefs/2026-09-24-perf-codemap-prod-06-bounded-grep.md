# [perf] Retain only the grep data each mode needs

## Work Type
perf

## Current State (As-Is)
- [confirmed] At revision `8b8222003584a3875ce3804557d3d5e70113ea4b`, every grep mode uses `CollectSink`, which copies every matched/context line into owned strings — Evidence: `CollectSink::matched()`, `context()`, and `grep_impl()` in `apps/codemap-search/src/tools/grep.rs`.
- [confirmed] Count and file-list modes derive metadata only after collecting those strings; non-expanded content builds all `ContentRow` values before pagination — Evidence: the `output_mode` branches and `paginate()` in `src/tools/grep.rs`.
- [confirmed] Default callable expansion already uses a bounded `ExpansionPage`, but its per-file sink still collects hits before grouping — Evidence: `LiveOptions::parse_grep()` and `src/tools/grep/expansion.rs`.
- [confirmed] Non-expanded content, count, and file-list responses have no mandatory byte ceiling when `grep_response_byte_cap` is `None`; configured response caps reject oversized final masked text with `-32602` — Evidence: `ResolvedConfig::default()`/`merge()` in `src/config.rs` and `enforce_response_cap()` after `redact::response()` in `src/mcp/mod.rs`.
- [confirmed] Callable expansion separately receives `grep_output_byte_cap`, defaulting to 5 MiB with the existing config/read-limit fallback; an explicitly configured response cap can still reject its final response — Evidence: the cap selection in `mcp::jev::live()` and `ExpansionPage::new()`.
- [confirmed] `paginate()` adds `start + limit` unchecked and numeric arguments accept `u64` values/strings — Evidence: `paginate()` and `lenient_usize()` in `src/tools/mod.rs`.
- [inferred] Dense matches cause unnecessary retention in metadata modes and overflow/oversized paging can undermine request safety — Confirm with counted sink/page workloads below rather than treating output length as a memory measurement.

## Baseline Measurement
- Capture immutable baseline `8b8222003584a3875ce3804557d3d5e70113ea4b` and candidate release binaries with identical toolchains, fixtures, effective config, and isolated homes.
- Use dense-match regular files of 1/4/16/32 MiB and repositories of 100/1,000/4,000 fixed-size files. Exercise `count`, `files_with_matches`, `content` with `expand=none`, and default callable expansion using identical raw regex inputs.
- Measure first-page, later-page, empty-page, `head_limit=0`, large numeric arguments, multiline/context matches, long lines, unsupported/over-callable-cap files, and early versus late first matches.
- Cross each mode with response-cap unset/configured and finite/zero `head_limit`. For configured caps, test final masked text at cap-minus-one, exactly cap, and cap-plus-one bytes using ASCII, multibyte UTF-8, and masking that changes length. Separately verify callable expansion's mandatory body budget and optional final response rejection.
- Record collector/capture working bytes, necessary ordering metadata, selected result bytes, and final serialized response storage separately, alongside files scanned, match regions counted, peak RSS/live allocation, and p95. In uncapped `head_limit=0` requests, preserve the full requested result and measure output-dependent memory rather than claiming constant total RSS or introducing an implicit cap.
- Use two warm-ups and 100 measured calls per operation in each of three paired same-host release runs with nearest-rank p95 and raw samples. End-to-end p95 for ordinary requests must be at most 1.10 times baseline.
- Pair only equivalent positive mode results for the ordinary latency comparison. Record explicit large-source/byte-limit fallbacks separately; fewer returned matches, a smaller page, an error, or omitted masking cannot count as a performance improvement.
- Target: metadata modes retain zero matched/context body strings; file-list matching stops after the first positive region per file; non-expanded content avoids off-page/duplicate body staging while preserving the requested result and existing cap/error behavior. Scaling is linear in scanned input plus necessary metadata/order/output work, with bounded auxiliary capture/collector buffers rather than a new fixed ceiling on uncapped requested output.

## Desired Outcome (To-Be)
- Each grep mode retains only data required by its observable contract, not every matched body.
- Exact counts, mtime file ordering, content ordering, context/multiline semantics, page units, and continuation notices remain correct.
- Unconfigured non-expanded responses remain uncapped, explicitly capped responses retain `-32602` overflow errors, and callable expansion alone uses its existing body-budget pagination; partial success must not replace an existing response-cap error.
- Body capture, redaction, callable grouping, and Jev use bounded exact-source evidence without bypassing source authorization.

## Scope
### In Scope
- Mode-specific sinks, streaming page selection, safe arithmetic, per-file hit retention, and bounded body capture.
- Mode-specific optional response caps, mandatory callable body budgets, and preservation of their distinct success/error/continuation behavior while removing unnecessary retention.
- Regression and release measurement coverage for all modes, page boundaries, large input, callable fallback, redaction, and Jev.
### Out of Scope
- [hard] Changing regex syntax, ignore precedence, file-type filters, count definitions, or the mtime ordering contract.
- [hard] A new public grep mode/argument, external `rg` process dependency, or weakened secret masking.
- [hard] Adding a mandatory default response cap to non-expanded content/count/file-list modes, inheriting the callable 5 MiB cap into them, or replacing their configured-cap errors with truncated success.
- [hard] Applying the read-only admission cap to all grep scanning and silently excluding otherwise searchable large files.
- [hard] Redaction cache/parallel worker changes from the independent older performance briefset.

## Constraints
- Consume Child 05's authorized bounded-source ownership/capture contract, but distinguish bounded source capture from streaming regex scanning. Large files must retain explicit safe fallback rather than disappear.
- Count mode counts the same match regions as before; context is not a match. File-list mode may stop matching a file after one hit but must still inspect candidate file metadata needed for exact total/order.
- Preserve content walk order and callable path/group order. Preserve newest-first file-list ordering with filename ties and valid `offset`/`next_offset` units.
- Use checked/saturating conversions and range arithmetic; oversized input numbers must return a defined result/error, never panic or wrap into an invalid slice.
- Non-expanded content/count/file-list with `grep_response_byte_cap=None`: impose no new response-byte ceiling. Preserve finite count pagination and, for `head_limit=0`, the complete remaining requested result without an invented byte-limit footer or error.
- Non-expanded content/count/file-list with `grep_response_byte_cap=Some(cap)`: preserve `-32602` when the existing final response check exceeds the cap. Do not turn that error into partial content, fewer counted rows, or a successful continuation page.
- Callable content: preserve `grep_output_byte_cap` selection, the default 5 MiB body budget, existing unavailable-body/truncation notices, and callable-group pagination. A configured `grep_response_byte_cap` still applies afterward and may return `-32602`; rendering within the body budget is not permission to bypass final rejection.
- `head_limit=0` removes only the count limit. It neither creates an optional response cap nor disables a configured response cap or the mandatory callable body budget. Preserve repo/global/tool/read-fallback precedence without merging the two cap fields.
- Response-cap comparisons use the same final masked MCP text bytes as `enforce_response_cap()`, not raw match bytes, characters, or serialized JSON envelope bytes. Reject early only when it is proven equivalent to that final decision, including existing Jev eligibility/fallback behavior; otherwise preserve the final check while reducing staging.
- Freeze separate auxiliary-buffer bounds and output/ordering-memory scaling measurements. A caller-requested uncapped full result can grow with returned bytes; do not hide duplicate/off-page body retention in that category or claim an absolute total-memory bound that the unchanged output contract cannot provide.
- If safe whole-source masking/callable capture is unavailable under its bound, use the existing conservative unavailable/masked-output behavior. Do not release uninspected original matching lines.
- Preserve caller configuration, permission policies, source stamps, and the final response redaction guard. The user authorized focused tests and measurement tooling only.
- Run all commands from the repository root; use synthetic fixtures, isolate `CODEMAP_HOME`, and serialize performance measurements against other builds/index workloads.

## Related Files / Entry Points
- `apps/codemap-search/src/tools/grep.rs` — start at `CollectSink`, `grep_impl()`, result retention, and `paginate()`.
- `apps/codemap-search/src/tools/grep/expansion.rs` — preserve callable group/page/budget semantics while eliminating unbounded per-file staging.
- `apps/codemap-search/src/tools/live_options.rs` — keep mode-specific defaults and invalid option combinations.
- `apps/codemap-search/src/tools/mod.rs` — preserve numeric coercion and make tool guidance distinguish count limits from mode-specific byte limits.
- `apps/codemap-search/src/config.rs` — trace `grep_response_byte_cap` versus `grep_output_byte_cap`, defaults, precedence, and callable read-limit fallback.
- `apps/codemap-search/src/mcp/jev.rs` — preserve mode-specific cap selection and oversized-base filtering bypass.
- `apps/codemap-search/src/mcp/mod.rs` — preserve the final post-redaction `enforce_response_cap()` decision and JSON-RPC error shape.
- `apps/codemap-search/src/tools/live_symbols/jev.rs` — keep selected-body capture coordinates and filtering identity correct.
- `apps/codemap-search/tests/e2e/tools.rs` — extend exact mode/pagination and boundary assertions.
- `apps/codemap-search/tests/e2e/redact.rs` — verify conservative large-source masking and unchanged raw matching semantics.
- `apps/codemap-search/tests/e2e/jev_live.rs` — verify filtered and fallback final grep output.
- `apps/codemap-search/scripts/` — colocate the focused opt-in grep measurement driver.
- `apps/codemap-search/docs/configuration.md` — align byte-cap and continuation behavior with final output.
- `apps/codemap-search/docs/configuration.ko.md` — mirror the same limits and mode contracts.

## Execution Plan
### Stage 1 — Freeze mode contracts and retention baselines
- Starts when: `docs/briefs/evidence/codemap-prod/05-bounded-read.md` supplies the verified source-buffer/capture contract and refusal semantics.
- Work: Pin exact mode counts/order/pages and the three-case cap contract in Constraints. Instrument working/capture bytes independently of requested output and ordering metadata, then baseline the complete mode-by-cap-by-head-limit matrix. Preserve large-source capture fallback without changing streaming search eligibility.
- No-op when: Current mode sinks already satisfy every retention, first-hit, arithmetic, output, and p95 criterion with complete measurements.
- No-op handoff: Record proof in `docs/briefs/evidence/codemap-prod/06-bounded-grep.md` (proposed); the parent validates it before forwarding the unchanged implementation to Child 08.
- Deliverable: `docs/briefs/evidence/codemap-prod/06-bounded-grep.md` (proposed), with fixture/request hashes, the mode/cap/head-limit success-error-continuation matrix, working/output memory accounting, exact commands, raw baseline samples/counters, and capture fallback rules.
- Verify: `Inspect the mode/cap/head-limit matrix and memory counters against the frozen fixture oracle`; Inputs: all four mode/expansion populations with caps unset/configured, finite/zero head limits, final masked byte boundaries, multiline/context matches, and over-capture-cap files; Expected: exact counts/order for positive cases, explicit -32602 for configured response overflow, callable-only body-budget continuation, and separate working-versus-output byte evidence.
- Ends when:
  - [ ] Mode-specific cap absence, response errors, callable pagination, and numeric limits are frozen before sink changes.
  - [ ] Positive cases have known nonempty output oracles and negative cases assert the exact error rather than passing because a response is empty.
- Handoff: Stage 2 consumes the mode/capture contract at `docs/briefs/evidence/codemap-prod/06-bounded-grep.md`.
- Replan when: An existing documented cap/continuation contract conflicts with the implementation or a fixture changes the mode semantics; stop, return to the parent for bounded contract correction and re-verification, and recalculate handoffs before proceeding.

### Stage 2 — Integrate mode-specific bounded collection
- Starts when: Stage 1 has frozen exact outputs, byte budgets, and retention counters.
- Work: Separate metadata-only collection from selected content, short-circuit positive file-list matches, and enforce safe page arithmetic and bounded auxiliary staging. Preserve the exact mode-specific uncapped/configured-error/callable-pagination contract through redaction, Jev, and the final response-cap consumer.
- Deliverable: Integrated sinks/pagination and the final buffer/mode contract in `docs/briefs/evidence/codemap-prod/06-bounded-grep.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: grep sinks, expansion paging, numeric helpers, and capture consumers; Expected: exit 0 and an inspected path proving metadata-only modes cannot allocate matched body strings.
- Ends when:
  - [ ] Metadata sinks store no matched bodies, off-page/duplicate content staging is removed, and requested output growth is accounted for separately from bounded working buffers.
  - [ ] Cap absence remains absence, configured response overflow remains an error, and callable body-budget pagination retains its separate final response check.
  - [ ] Maximum numeric arguments cannot panic, wrap, or silently change page units.
- Handoff: Stage 3 receives the integrated collectors and frozen output oracles.
- Replan when: A proposed shortcut loses exact counts/order or requires returning uninspected text; reject it, correct the owning sink/capture path, and preserve the previous safe semantics until verified.
- Worker decision: Use mode-specific sink types or a private enum and bounded page/top-k metadata selection; do not introduce a new regex engine or general scheduler.

### Stage 3 — Verify final modes and resource scaling
- Starts when: Stage 2 is integrated with the source/capture contract from Child 05.
- Work: Run existing mode/config/redaction/Jev cases and the frozen dense/page/cap matrix, then repeat release measurements with the same hashes and requests. Verify final masked-byte error boundaries, uncapped full-result equivalence, callable continuation, and large-source fallback as distinct populations.
- Deliverable: `docs/briefs/evidence/codemap-prod/06-bounded-grep.md`, with exact success/error outputs, nonzero matrix case counts, separate working/output memory data, before/after raw counters/samples, mode-specific scaling, commands, and target replay cases.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests -- e2e::tools:: e2e::config:: e2e::redact:: e2e::jev_live:: --test-threads=1`; Inputs: existing and added mode/cap/head-limit, masked-byte boundary, pagination, and large-source fixtures; Expected: selected populations execute and pass with exact counts/order, preserved -32602 overflow errors, no invented cap for uncapped responses, and no unmasked synthetic secret in known nonempty content output.
- Ends when:
  - [ ] Metadata matched-body retention is zero and file-list first-hit termination is observed.
  - [ ] All success/error/continuation matrix cells, separate working/output memory limits, safe numeric boundaries, exact totals, and the 1.10 p95 gate pass.
- Handoff: Child 08 receives `docs/briefs/evidence/codemap-prod/06-bounded-grep.md` and the integrated four-population replay contract.
- Replan when: Counts, order, masking, measured scaling, or p95 fail; stop acceptance, correct Stage 2, and re-run the affected frozen populations before reconciling the parent.

## Side Effect Checkpoints
- [ ] `-A`/`-B`/`-C`, multiline, `-n`, type/glob aliases, ignored-file controls, and explicit-file matching semantics remain correct.
- [ ] Content rows versus callable groups retain distinct pagination units and notices; `head_limit=0` changes only the count limit and preserves each mode's cap presence/absence.
- [ ] Config precedence and the callable read-limit fallback remain intact; only the final masked text byte count controls an explicitly configured response-cap error.
- [ ] Non-expanded configured overflow returns the existing `-32602` envelope rather than partial success, while uncapped full-result output is neither shortened nor implicitly refused.
- [ ] Binary/invalid-UTF-8 input, source-change stamps, long-column omission, and unavailable callable bounds remain explicit.
- [ ] Jev only captures eligible selected bodies and its disabled/provider-fallback paths preserve the plain accepted output.
- [ ] CLI/MCP permission policies and final JSON-RPC envelope shapes are unchanged.

## Acceptance Criteria
- [ ] Count/file-list modes retain zero matched/context body strings, with first-positive-region termination in file-list mode.
- [ ] Non-expanded responses preserve no mandatory cap when unconfigured and preserve `-32602` on configured final-text overflow; callable content preserves body-budget pagination plus the optional final response check.
- [ ] Finite-page collection avoids off-page/duplicate bodies, while uncapped `head_limit=0` retains the complete requested result. Measurements distinguish bounded working/capture buffers from necessary ordering metadata and returned/serialized bytes without claiming constant total memory for arbitrary full results.
- [ ] The full mode/cap/head-limit and masked-byte-boundary matrix passes without panic, invented truncation/errors, or leaked uninspected source.
- [ ] Before/after release measurements demonstrate the intended retention/work reduction and p95 at most 1.10 times baseline per ordinary operation.
- [ ] The complete mode/capture contract and target replay cases are delivered for Child 08's integrated seven-target validation.

## Open Questions
- None — Preserve the explicit per-mode cap/error contract above; no new grep response cap or partial-success behavior is approved or needed for this scoped retention improvement.
