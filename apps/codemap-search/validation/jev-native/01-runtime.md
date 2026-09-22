# 01 — reusable Jev runtime handoff

## Contract

- Entry point: `codemap_search::jev::Evaluator::evaluate(Request, Policy, Cancellation)` returns an asynchronous `Evaluation`. `BatchEvaluator` accepts an `Arc<dyn Transport>`; `HttpTransport::new(api_key, pool_idle_timeout)` is the production HTTPS implementation. No MCP, parser, index, workspace, config loader, Python process, or global task intent is imported by the common module.
- `Request` owns JSON `state` and a map of request-local question IDs to `Question::Score { instructions, criteria }`, `Choice { instructions, criteria }`, or `Noul { instructions, criteria }`. Instructions accept string/object/array. Score has 2–10 levels; Choice has 2–255 named options. IDs are routing keys and carry no judgment meaning.
- `Evaluation` owns typed `Answer` values, pinned model ID, summed `Usage { input_tokens, output_tokens }`, whole-call elapsed time, and per-batch question IDs, usage, and HTTP elapsed time. Score retains its weighted value and raw level distribution; Choice retains its selected key and raw option distribution; Noul retains its yes probability without a fabricated confidence field. Adapters own question and policy versions.
- `JevError` reports a bounded non-sensitive reason. On a failed or timed-out multi-batch call, final provider usage can be unknown; integrations report `null` rather than inventing zero.

## Wire and policy

- Endpoint: `POST https://api.typesafe.ai/v1/systemone` with bearer authentication at the transport boundary and `{"model":"jev-1.13.0","state":...,"questions":...}`. Response model, exact answer-ID set, answer type, finite numbers, option/level keys, probability range and sum (tolerance 0.02), and weighted Score (tolerance 0.05) are checked before returning any result.
- Defaults: 45,000ms whole-call deadline including packing/queue/spacing/HTTP, 80,000 encoded bytes per batch, at most three in-flight HTTP requests, at least 300ms between starts, and a 30,000ms idle connection pool lifetime. POST redirects and automatic retries are disabled. Caller cancellation and shorter policy deadlines are honored. A body larger than the configured or estimated budget fails explicitly; no question is truncated.
- Context handling: Jev 1.13 publishes 64k tokens for state plus all questions and 32k tokens for state plus the longest question. This implementation bounds the encoded JSON batch to 64,000 bytes and the encoded state plus each question to 32,000 bytes as an estimate. Bytes are not an exact Jev tokenizer. The provider may still reject a request, which becomes `provider_context_limit` or another explicit fallback reason.
- Dependency: `reqwest 0.12.28` resolved in `Cargo.lock`, using rustls and no system OpenSSL requirement. The checked local toolchain is `rustc 1.98.1`. The existing release workflow builds Linux GNU/musl, macOS and Windows targets; this handoff did not execute those cross-platform builds.

## Offline evidence and reuse

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev`: passed 20 selected tests on final source, including mixed Score/Choice/Noul, absent Noul confidence, invalid criteria and answers, deadlines, cancellation, batching, both local 422 context-limit forms, no retry, and local HTTP connection reuse/idle expiry. This selection also contains config/adapter regressions matching `jev`.
- `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock`: passed with Score=2.0, Choice=keep, Noul=0.9, input_tokens=123 and output_tokens=9. It uses fixed offline responses and no credentials, MCP process, index, or Python. `--live` exists only as an explicit operator action and was not run.
- `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets`: passed before the final documentation and test-only snapshot case. The final gate is recorded in `05-verification.md`.

Source contracts: [TypeSafe HTTP API](https://docs.typesafe.ai/api.md), [Jev 1.13 model limits](https://docs.typesafe.ai/models.md). No live provider behavior or non-macOS release build is claimed.
