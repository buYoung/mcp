# Jev native runtime handoff (child 01)

Status: implemented and verified offline on 2026-09-23. The work was done on the current
`opus-5.5` branch by operator instruction (no other branch or worktree was inspected), with
nothing staged or committed.

The runtime is the common module `codemap_search::jev` (`src/jev/`). It does not import MCP,
index, parser, workspace, or config code, reads no environment variable, and sends nothing
unless a caller constructs an evaluator and calls it.

## API

| Item | Purpose |
|------|---------|
| `Question::score(id, instructions, levels)` | Score question; 2–10 ordered level descriptions |
| `Question::choice(id, instructions, options: Vec<ChoiceOption>)` | Choice question; 2–255 unique option keys |
| `Question::noul(id, instructions, Option<NoulCriteria>)` | Yes-probability question; optional `true`/`false` descriptions |
| `ChoiceOption::new(key, description)` | `Value::Null` description means "no extra detail" |
| `NoulCriteria::new(when_true, when_false)` | Either side optional |
| `EvaluationRequest::new(state, questions)` | One shared JSON state plus independent questions |
| `.with_deadline(tokio::time::Instant)` | Caller deadline; the effective deadline is the earlier of this and `policy.timeout` |
| `.with_cancellation(CancellationToken)` | Caller cancellation; `CancellationToken::cancel()` stops in-flight work |
| `trait JevEvaluator { fn evaluate(&self, EvaluationRequest) -> BoxFuture<'_, Result<Evaluation, EvaluationFailure>> }` | Adapter-facing, object-safe boundary (`Arc<dyn JevEvaluator>`) |
| `Evaluator::new(transport: impl JevTransport, TransportPolicy)` | Validated evaluator over any transport |
| `Evaluator::https(&ApiKey, TransportPolicy)` | Production evaluator over `HttpsTransport` |
| `trait JevTransport { fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> }` | Injection point below validation |
| `ApiKey::new(value)` | Trimmed printable-ASCII key; `Debug` prints `ApiKey([redacted])` |
| `estimate_tokens(&[u8])` | The conservative local token estimate used for budgets |

`Evaluation` carries `model`, `answers: BTreeMap<String, Answer>`, `usage`, `timing`, and
`requests: Vec<RequestRecord>`; `score(id)`, `choice(id)`, `noul(id)` return typed views.
`EvaluationFailure` carries `error: JevError` plus the usage, timing, and request records
collected before the failure.

There is no task-intent parameter: callers put the explicitly supplied intent in a named
state field (both adapters use `task_query`). The runtime never reads transcripts, globals,
or another request's state.

## Primitive shapes

Request body (keys and question ids emitted in sorted order, so equal inputs hash equally):

```json
{"model":"jev-1.13.0","questions":{"<id>":{"criteria":…,"instructions":…,"type":"score|choice|noul"}},"state":…}
```

| Type | Criteria sent | Answer accepted | Retained as |
|------|---------------|-----------------|-------------|
| Score | ordered array of 2–10 level descriptions | `score` finite in [0, n−1]; `probabilities` keys exactly `"0".."n-1"`, each finite in [0, 1], Σ within 1 ± 0.02; `confidence` in [0, 1]; `legend` ignored | `ScoreAnswer { score, probabilities: Vec<f64> (index = level), confidence }` |
| Choice | map option → description or `null` | `choice` is a supplied key and within 0.02 of the highest probability; `probabilities` keys exactly the options, each in [0, 1], Σ within 1 ± 0.02; `confidence` in [0, 1] | `ChoiceAnswer { choice, probabilities: BTreeMap, confidence }` |
| Noul | optional `{"true": …, "false": …}` | `noul` finite in [0, 1]; no confidence required, any extra field ignored | `NoulAnswer { noul }` |

Instructions, levels, descriptions, and Noul criteria accept a non-blank string, a non-empty
object, or a non-empty array. State must be a non-blank string, a non-empty object, or a
non-empty array. Question ids are 1–128 bytes of `[A-Za-z0-9_.:-]`, unique per request; they
are routing keys only and appear in errors and logs, so adapters must use synthetic ids
(never file paths or source text).

`PROBABILITY_SUM_TOLERANCE = 0.02`: 91 historical Jev 1.13 responses (2,505 Score and 725
Choice answers) deviated from Σ = 1 by at most 0.0099 and their Score values from the
probability-weighted level by at most 0.02. The runtime does not require `score` to equal
the expected level, and it never clamps or rounds returned values.

Responses must report the pinned `model`, answer exactly the batch's question ids, and
include `usage.input_tokens` / `usage.output_tokens`; anything else fails the evaluation.

## Transport policy

| Field | Default | Accepted range | Meaning |
|-------|---------|----------------|---------|
| `model` | `jev-1.13.0` | `<name>-<major>.<minor>.<patch>`; aliases such as `jev-latest` rejected | Pinned model, compared with the response `model` |
| `timeout` | 45,000 ms | 1–600,000 ms | Whole-call deadline including validation, permit queueing, spacing, and every batch |
| `max_in_flight_requests` | 3 | 1–16 | Concurrent HTTP requests per evaluator instance (shared by every call on it) |
| `request_spacing` | 300 ms | 0–10,000 ms | Minimum gap between request starts per evaluator instance |
| `max_batch_bytes` | 80,000 bytes | 4,096–512,000 | Ceiling for one encoded request body |
| `pool_idle_timeout` | 30,000 ms | 1–600,000 ms | Pooled connections idle longer are closed, never reused |

No automatic retries: 429/529 and every other failure end the evaluation. `TransportPolicy::validate()`
enforces the ranges; `Evaluator::new` refuses an invalid policy.

## Batching and token budgets

- Questions keep caller order and are packed greedily; every body repeats the full state.
  No question is split, truncated, or dropped. A question that cannot fit in a body on its
  own fails the whole evaluation before dispatch (`RequestTooLarge` or `TokenBudgetExceeded`).
- Byte sizes are computed exactly from pre-encoded parts, so each body is at most
  `max_batch_bytes`.
- Jev 1.13 publishes two limits: 64k tokens for state plus all questions, and 32k tokens for
  state plus the longest question. No tokenizer is published, so the runtime estimates
  `ceil(ascii_bytes / 2) + 2 × non_ascii_chars + 512` tokens. Every question is checked with
  the state against 32k (failure: `TokenBudgetExceeded { budget: StateWithLongestQuestion }`)
  and every body against 64k (a full batch is split; a single question over it fails with
  `budget: TotalRequest`).
- Calibration: over 166 historical successful requests (200–79,943 bytes; 318–29,324
  provider input tokens) the estimate was 1.37–1.92× the reported tokens and never below
  them; the largest 80 KB body estimated 40,764 tokens, so the default byte ceiling does
  not trip the 64k estimate for ordinary ASCII evidence.
- Limitations: the estimate is a guard, not a tokenizer. Unusual text can still exceed a
  provider limit, and the provider may count differently in future models. A provider
  rejection with status 413, or 400/422 whose body mentions a token/context limit, becomes
  `JevError::ProviderContextLimit` (other 400/422 bodies become `HttpStatus`). Either way the
  evaluation fails explicitly; nothing is silently truncated or re-split.

## Deadlines, cancellation, and concurrency

- `run` polls every batch future inside the caller's task (no spawned tasks). The first
  failing batch ends the call; the remaining futures are dropped, which aborts their HTTP
  exchanges and releases their permits.
- The deadline races the whole join, so time spent waiting for permits or spacing counts.
  Expiry returns `DeadlineExceeded { elapsed }`; cancellation returns `Cancelled`. A token
  already cancelled, or a deadline already passed, fails before any request.
- Requests that were not dispatched or were dropped in flight are recorded as
  `RequestOutcome::NotCompleted`; the provider may still have processed a dropped request,
  so its usage is unknown and not counted.

## Failure contract

`JevError` is the single failure type for question validation, policy/credential checks,
budgets, transport, HTTP status, and response validation. `JevError::label()` gives stable
snake_case labels for status lines: `invalid_policy`, `invalid_credential`,
`invalid_question`, `invalid_request`, `request_too_large`, `token_budget_exceeded`,
`provider_context_limit`, `http_status`, `transport`, `deadline_exceeded`, `cancelled`,
`malformed_response`, `model_mismatch`, `answer_set_mismatch`, `invalid_answer`.

Messages contain counts, statuses, and question ids only. Provider error bodies are read
(at most 64 KiB) solely for context-limit classification and never stored; serde parse
errors report only the failure category and position.

## Usage, timing, and identity

- `Usage { input_tokens, output_tokens }` sums every provider response that reported usage,
  including a response whose answers were then rejected. `EvaluationFailure.usage` keeps
  that partial usage.
- `Timing.elapsed` is the whole call (queueing, spacing, validation). `Timing.http_elapsed`
  sums completed HTTP exchanges and can exceed `elapsed` when requests overlap.
- `RequestRecord` per body: `batch_index`, `question_count`, `body_bytes`,
  `estimated_tokens`, `body_sha256`, `outcome`, `http_status`, `http_elapsed`, `usage`.
  Bodies themselves are never retained.
- Adapters own question and policy versions; the runtime reports only the model id.
- Confidence describes how concentrated a distribution is; it is not correctness or
  permission to act.

## Dependencies and TLS

| Crate | Version | Features | Why |
|-------|---------|----------|-----|
| `reqwest` | 0.13.5 | `default-features = false`, `rustls-no-provider` | Async HTTP/1.1 client with connection pooling and idle expiry on the existing Tokio runtime |
| `rustls` | 0.23.45 | `default-features = false`, `ring`, `std`, `tls12` | TLS without OpenSSL; ring avoids the cmake/NASM toolchain aws-lc needs on the release targets |
| `rustls-platform-verifier` | 0.7.0 | default | OS trust store (Security.framework, SChannel, native certs on Linux) |

- Resolved transitively: hyper 1.11.1, hyper-util 0.1.20, hyper-rustls 0.27.10, ring
  0.17.14. `Cargo.lock` contains no `openssl-sys` or `aws-lc`.
- Highest dependency MSRV: 1.85 (`reqwest`, `rustls-platform-verifier`). The crate declares
  no `rust-version`; release builds use `dtolnay/rust-toolchain@stable`, and local
  verification used rustc 1.98.1.
- Release targets: ring builds with the C compiler the crate already needs for tree-sitter
  grammars (cross-rs image for aarch64 musl). `aarch64-pc-windows-msvc` stays best-effort:
  ring needs clang there, which the hosted Windows image provides.
- Client settings: preconfigured `rustls::ClientConfig` with an explicit ring provider (no
  process-wide default provider is installed), ALPN `http/1.1`, `http1_only`, HTTPS-only,
  redirects off, `retry::never()`, `pool_idle_timeout` and `pool_max_idle_per_host =
  max_in_flight_requests` from the policy, user agent `codemap-search/<version>`.
  Standard proxy environment variables are honoured; HTTPS still tunnels end to end.
- The authorization header is marked sensitive and lives only inside `HttpsTransport`;
  credentials never enter state or errors. Success bodies are capped at 4 MiB.
- Safe connection replacement: hyper-util may transparently re-send a request whose bytes
  were never written because a reused idle connection turned out to be closed. That
  replaces a dead connection; it is not a retry of a delivered POST. Connections idle longer
  than `pool_idle_timeout` are never reused.
- `tokio` gains the `test-util` feature in `[dev-dependencies]` only (paused virtual time
  for deterministic deadline, spacing, and cancellation tests); production builds are
  unchanged.

## Mock injection route

- Unit and adapter tests implement `JevTransport` (the runtime then validates their
  scripted responses) or `JevEvaluator` (to script answers and failures directly) and pass
  it by value or as `Arc<dyn JevEvaluator>`.
- `ENDPOINT` (`https://api.typesafe.ai/v1/systemone`) is a constant with no runtime
  override. The plain-HTTP loopback constructor used by the idle-reuse test is
  `#[cfg(test)]` and asserts a 127.0.0.1 host, so it does not exist in production builds.
- Credential resolution belongs to the integration boundary: construct `ApiKey` from the
  operator's source there and call `Evaluator::https`.

## Adapter guidance

- Put the explicit task intent and other shared facts in named state fields; give each
  question self-contained evidence in named instruction or state fields and reference them
  in backticks. Questions in one request cannot see sibling instructions or answers.
- A dependent second stage (for example roles for selected files) is a new
  `EvaluationRequest` built after the first completes.
- Keep candidate types, question wording, thresholds, versions, and ranking in the adapter.
  Raw answers in `Evaluation` are sufficient to replay selection policy without inference.
- Share one evaluator per policy/credential so permits and spacing apply across calls.

## Example

`examples/jev_decisions.rs` builds questions from caller-supplied state and calls the
evaluator directly. `--mock` (default) uses a fixed mixed-response fixture and exits
nonzero unless it reads Score=2.0, Choice=keep, Noul=0.9, and usage 412/36. `--live` reads
`TYPESAFE_API_KEY` and makes one billed request; it was not run.

## Verification (2026-09-23)

| Command | Result |
|---------|--------|
| `cargo check --manifest-path apps/codemap-search/Cargo.toml` | exit 0, no warnings |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev` | 19 passed, 0 failed; no provider traffic |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions` | exit 0 |
| `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock` | exit 0, `mock check passed` (run with `TYPESAFE_API_KEY` unset) |

The runtime tests cover mixed Score/Choice/Noul batches and wire shape, Noul without
confidence, criteria limits, malformed and incomplete answers, model mismatch, request
validation, byte packing, oversized questions, both local token budgets, provider
context-limit statuses, HTTP status classes without retry or body retention, caller and
policy deadlines including queue time, cancellation, concurrency and spacing bounds,
fail-fast stopping, permit release, `Debug` redaction, and idle connection reuse and
replacement against a loopback HTTP server.

Not run: any live provider request (no fresh execution instruction was given).
