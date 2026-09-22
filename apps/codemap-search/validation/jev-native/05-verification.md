# 05 — offline native Jev verification

## Execution identity and scope

The checks ran against the uncommitted Rust codemap-search implementation. Per the user's restriction, no worktree inventory, other branch or HEAD revision was inspected. The local checked toolchain was `rustc 1.98.1` on macOS arm64. All Jev evaluation checks below use injected answers or a loopback HTTP server; no production provider request, paid benchmark, or Python proxy was run.

## Regression matrix

| Contract | Populated offline case and observation |
|---|---|
| Shared Score/Choice/Noul runtime | Mixed response fixture asserts Score 2.0, Choice keep, Noul 0.9 without a Noul confidence, exact IDs/model, and usage 123/9. Invalid criteria/answers, 255-option ceiling, cancellation, deadline, byte/context packing and both loopback 422 context-limit forms are separate runtime cases. Loopback HTTP confirms reuse, expiry and no retry without contacting the provider. |
| Root recommendation coverage | 31 indexed files (30 qualified and one unqualified) are evaluated before the 24-file cap. Additional cases cover all-negative, tied and no-usable-evidence results, output-limit fallback, and an immutable published snapshot held across a delayed fake evaluation. |
| Structured search retention | Fixed Noul 0.80 replays as omit at 0.70 and retain at 0.90. A forced 1.00 cannot remove incomplete, noncallable, nested or displayed-dependency bodies. Invalid thresholds fail. The multi-file MCP fixture contains two files and three named functions, with no empty-population success path. |
| Final search rendering and source accounting | Injected MCP calls preserve two distinct file headings, declaration names, balanced fences and each original body under its own file when retained. At 0.70 the selected bodies are omitted and `analyze reads` reports zero source files; at 0.90 the bodies are retained and it reports two. No source or index file is rewritten. |
| Activation and fallback | The injected server exercises both enabled, overview-only and filter-only settings, live config reload, keyless/intentless bypass, and fake invalid-response fallback with base content. A normal spawned binary confirms missing-key bypass and non-string `task_query` error `-32602`. Existing disabled calls have no Jev metadata. |
| Unrelated tool and protocol compatibility | Existing config, MCP, search, codemap, redaction, live-tool, watcher and cross-feature cases execute within the e2e target. JSON-RPC content/error envelopes and sequential notification behavior remain in the existing MCP coverage. |

## Executed checks

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev`: 20 passed, 0 failed on the final source. This selection includes runtime, config and both adapter regressions.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview`: 5 passed, 0 failed after the delayed-snapshot case.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search`: 3 passed, 0 failed after typed-file final rendering.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib config::tests::test_v24_migration_adds_inactive_jev_keys_without_changing_existing_values`: 1 passed, 0 failed. Earlier `cargo test --lib config` had 37 passed before this added case.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev`: 3 passed, 0 failed after multi-file rendering, fallback and threshold cases. The parent case launches one isolated child process for injected evaluation; its fixture asserts nonempty evaluated/omitted bodies.
- `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets`: exit 0 after source changes.
- `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock`: exit 0; Score=2.0, Choice=keep, Noul=0.9, input_tokens=123, output_tokens=9.
- `cargo package --manifest-path apps/codemap-search/Cargo.toml --list --allow-dirty`: exit 0. The list contains `src/jev/mod.rs` and `examples/jev_decisions.rs`; experiments, validation notes and private test fixtures are excluded. This only inspected the package list.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests`: exit 0 on the final source; 211 passed, 0 failed, 1 ignored. The ignored pre-existing `test_macro_expansion_reaches_search_read_and_refreshes_header_changes` requires an installed Clang preprocessor and is unrelated to Jev. Full log: `/tmp/jev-native-e2e-postjoin.log`.

## Limits and a later live comparison

Live provider response validity, representative Noul threshold calibration, relative answer quality and Rust speed were not measured. The local macOS build does not establish the release workflow's Linux, Windows, macOS x86_64 or musl behavior. No deterministic model score or quality equivalence is claimed. Historical Python measurements and the previously noted 815-versus-822 index difference remain reference material only; they are not a native baseline.

The brief's referenced `experiments/jev-playground/checkpoint-poc/` files were absent in this working directory. The runtime wire contract was checked against the public TypeSafe API and the written brief, while line-by-line parity with that Python PoC remains unverified. The PoC directory was not changed.

A separately authorized live comparison should freeze one Rust binary, task wording, index/source snapshot and settings for Jev off, overview recommendation and search filtering. Run each mode three times, retain every attempt including fallback/exclusion, report actual applied status, API usage separately from the main agent's tokens, total elapsed time, source-preservation checks and a fixed file-plus-line quality rubric. Do not compare metrics from different index populations as though they were the same run.
