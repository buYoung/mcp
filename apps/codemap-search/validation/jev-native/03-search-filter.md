# 03 — structured search-filter handoff

## Callable boundary

- `tools::search::run_with_metadata(&ToolContext)` keeps the existing synchronous, unfiltered path. `prepare_for_jev(&ToolContext)` executes the same BM25, workspace, detail selection, redaction and byte-budget path, but retains typed `FileOutput` objects, selected source segments, declaration identity, completeness, insertion positions and the original base response before final relation insertion.
- `search::filter_prepared(output, task_query, arguments, evaluator, policy, cancellation, threshold)` evaluates only the selected complete bodies. It returns `FilterResult` with the final response, raw Noul answers, effective threshold, per-body retention reasons, evaluated/omitted counts, policy versions, usage and elapsed time. `FilterFailure` owns the untouched base result and reason; `finish_unfiltered` restores ordinary relation output. Event-key, empty, partial, warming, dead and stale results do not call the evaluator.
- `FileOutput::source_segments` records exact renderer-selected body/literal spans. Complete means every original numbered source line in the declaration range was displayed without clipping; partial, missing and oversized bodies cannot be omitted. Model input uses the exact displayed and redacted source body plus bounded displayed context, not a fresh source read or parsed Markdown.

## Retention and rendering

- The Noul question asks whether the displayed declaration body is unrelated to the caller's `task_query`. True means unrelated; false includes direct/supporting flow, configuration, ordering, failure and contradicting evidence. The host omits only complete, unprotected bodies with `Noul >= search_filter_min_unrelated_probability` (default 0.70, finite and `0.5 < value <= 1.0`). Raw probabilities are separate from the retention mask, so the pure Rust policy can be replayed at 0.70 or 0.90 without another evaluation.
- Incomplete or over-16 KiB bodies, noncallable/unknown declarations, ambiguous names and displayed nested/call connections seed conservative retention. Closure retains connected bodies even when their Noul is 1.00. More than 64 selected body segments yields an explicit `too_many_selected_bodies` fallback, bounding the policy's pairwise work. No guessed unseen call target is used to remove code.
- Omissions are applied to the typed `FileOutput.results` segments. The existing file/declaration renderer then serializes the retained sections for the final response. File headings, all declaration names/ranges, literal rows, path-only tails and balanced fences survive. The original response is available for whole-call fallback. No final Markdown heading/fence parser or regex remover is used.
- If no body is removed, the original source-file observations are kept. If bodies are removed, observations are rebuilt from retained source/literal segments after output selection; removed body-only files contribute no read. Relation insertion positions and anchors are adjusted to the final file layout.

## Verification

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search`: 3 passed, 0 failed. The cases replay a raw 0.80 Noul answer under 0.70/0.90, retain incomplete/noncallable/nested/dependent evidence under forced 1.00, and reject invalid thresholds.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev`: previously passed an injected multi-file pipeline. It checks separate headings, balanced fences, original file/body association after 0.90 all-keep, omitted source absence at 0.70, `analyze reads` file count 0 after omission and 2 after retention. The final post-join result is recorded in `05-verification.md`.
- Existing `e2e::search` (24 passed) and `e2e::codemap` (15 passed) covered the disabled path before final joining.

Question version: `search-unrelated-noul-v1`; policy version: `search-retention-v1-experimental`. The threshold has not been calibrated with representative live Noul judgments; historical Python Choice measurements do not transfer to it.
