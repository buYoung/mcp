# Jev native — 01 재사용 가능한 판단 런타임 인계

- 하위 브리프: `docs/briefs/2026-09-21-feat-jev-native-01-runtime.md` (개정판, 작업 트리의 미커밋 상태)
- 작성일: 2026-09-23
- 상태: 오프라인 계약 완료. 런타임 회귀, 로컬 소켓 transport 검증, 독립 mock 예제, 패키지 type-check가 모두 실제 실행되어 통과했다. 실제 provider 요청은 보내지 않았고 자격 증명은 읽거나 저장하지 않았다.

## 0. 시작 점검 (상위 브리프셋 실행 기준)

| 항목 | 확인한 값 |
| --- | --- |
| 저장소 루트 / cwd | `/Users/buyong/workspace/private/buyong-mcp` (Cargo 명령은 모두 이 루트에서 `--manifest-path apps/codemap-search/Cargo.toml`로 실행하거나 `apps/codemap-search`에서 실행) |
| 브랜치 / HEAD | `docs/jev-integrate`, `530e476c3` (= `97e3ebc8e` + 브리프 6개 추가 커밋) |
| 시작 시 작업 트리 | 브리프 Markdown 6개만 수정(미커밋 개정판). 이 개정판이 실행 계약이다. |
| 기존 구현 후보 | `feat/codemap-jev` = `2feadb26a`(단일 커밋, `97e3ebc8e` 기반). 코드·PoC·01–05 인계·브리프 원본이 있다. |
| 실행 위치 결정 | 현재 승인된 위치인 `docs/jev-integrate` 작업 트리에서 진행. checkout/reset/cherry-pick/merge는 하지 않았다. 기존 구현 파일(`git diff --name-only 97e3ebc8e feat/codemap-jev`)만 `git restore --source=feat/codemap-jev --worktree`로 작업 트리에 복원(스테이징 없음)한 뒤 개정 계약과 대조해 수정했다. 제외한 것: `experiments/jev-playground/**`(PoC·사적 벤치마크 입력), 과거 `validation/jev-native/*.md`(완료 체크 복사 금지 — 새로 작성), `docs/briefs/*`(현재 개정판 유지), `docs/event-navigation-handoff.ko.md`(무관 변경). |
| PoC 경로 | 현재 체크아웃에 `apps/codemap-search/experiments/jev-playground/checkpoint-poc/`는 없다. 필요한 부분(`jev_transport.py`의 정책 수치, 기록된 응답 정밀도)은 `git show feat/codemap-jev:<path>`로 읽기 전용 참고했고 어떤 파일도 복사하지 않았다. 기록된 원시 확률 분포는 남아 있지 않았고(집계값만) score/confidence는 소수 둘째 자리였다. |
| 공식 문서 | 이번 실행에서는 다시 조회하지 않았다. 계약은 브리프에 고정된 값(모델 `jev-1.13.0`, endpoint, 64k/32k 한도)을 그대로 사용한다. 문서 변경으로 pinned 값이 깨지는지는 미확인이며 live 실행 전 재확인 대상이다. |
| 도구 체인 | rustc 1.98.1, cargo 1.98.1, macOS(darwin 24.6.0). 크레이트 `codemap-search` 0.10.0 |
| 최종 소스 상태 | HEAD `530e476c3` + 미커밋 변경(수정 28개, 신규 17개 파일; 목록은 `05-verification.md` §1). 소스 digest(수정·신규 소스 파일 sha256 목록의 sha256 앞 16자): `867b202d2fc89450` |

기존 구현과 개정 계약의 차이(01 범위)와 처리:

| 차이 | 처리 |
| --- | --- |
| `EvaluationRequest::new(task_query, Map, questions)`가 `task_query`를 필수 최상위 인자로 요구하고 state에 기록함 (R-C1 위반) | state를 caller 소유 JSON(`Value`; string/object/array)으로 일반화. 런타임은 필드를 추가·읽지 않는다. 두 adapter가 `TASK_QUERY_FIELD`를 state에 직접 넣는다. |
| `RequestPolicy.deadline: Option<Duration>` (상대값) | `deadline_at: Option<tokio::time::Instant>` 절대 시각. 유효 마감 = min(caller 시각, evaluate 진입 + 평가기 deadline). |
| 응답 duplicate key 미검사, Choice 최댓값·Score 가중합 미검사 | `NoDuplicateKeys` walker로 응답 전체 검사, Choice 선택값이 최고 확률(허용 오차 내)인지, Score가 `Σ level·p`와 일치하는지 검사. |
| 응답 body 무제한 읽기, 질문 수·batch 수 무제한 | `MAX_RESPONSE_BODY_BYTES`(4 MiB, bounded chunk 읽기), `MAX_QUESTIONS_PER_REQUEST`(8,192), `MAX_BATCHES_PER_REQUEST`(128) |
| reqwest 기본 redirect(최대 10회 추종)와 기본 retry(protocol NACK 재전송) | `redirect::Policy::none()`, `retry::never()`로 고정 |
| 설정 범위: in-flight 상한 없음, spacing 0 허용, batch 1024 이상 무제한 | in-flight 1..=3, spacing ≥ 300ms, batch 1..=80,000, timeout/idle 양수·≤7일 (01/04 동일 해석) |
| transport 실제 동작 검증 없음 | `#[cfg(test)]` 전용 로컬 listener endpoint로 pooling·idle 교체·무재시도·redirect 미추종·body 한도를 실제 소켓에서 검증 |

## 1. 변경 파일 (01 소유)

| 경로 | 역할 |
| --- | --- |
| `src/jev/mod.rs` | 모듈 루트, `Evaluator` trait, `CancelToken`, `JevError`, 상한 상수 |
| `src/jev/question.rs` | `QuestionId`, `Question`(Score/Choice/Noul), `EvaluationRequest`(state `Value`), `RequestPolicy`(`deadline_at`) |
| `src/jev/answer.rs` | typed 답변, 허용 오차, `Usage`/`Timing`/`RequestIdentity`, `EvaluationOutcome`/`EvaluationFailure`, 답변 검증 |
| `src/jev/batch.rs` | byte-bounded 배치, batch 수 상한, 두 token 추정 |
| `src/jev/transport.rs` | `Transport` trait, `SecretString`, `HttpsSettings`(validate), `HttpsTransport`(redirect/retry 차단, bounded 읽기, test 전용 endpoint) |
| `src/jev/evaluator.rs` | `EvaluatorConfig`(범위 검증, `is_safe_duration`, `MAX_SAFE_DURATION`), `JevEvaluator`(permit·spacing·절대 deadline·취소·검증·계측) |
| `src/jev/mock.rs` | `MockTransport`, `MockEvaluator`, `Gate`, `RecordedRequest`(state·questions·deadline_at, `task_query()` 도우미) |
| `src/jev/tests.rs` | 오프라인 회귀 38개 + 로컬 소켓 4개 (`cargo test --lib jev`) |
| `examples/jev_decisions.rs`, `examples/jev_decisions.mock.json` | 독립 예제(`--mock` 기본, `--live` 선택)와 비민감 fixture |
| `Cargo.toml`, `Cargo.lock` | `reqwest 0.13.5`(`rustls-no-provider`), `rustls 0.23`(`ring`), dev `tokio/test-util` — `feat/codemap-jev`와 동일한 의존성 집합(`git diff feat/codemap-jev -- Cargo.lock` 공백) |

`src/jev/`는 크레이트의 다른 모듈을 import하지 않는다(`grep -rn 'crate::' src/jev` 0건). MCP·parser·index·workspace·전역 config·환경 변수 조회가 없고, 모듈 링크만으로는 어떤 요청도 시작되지 않는다.

## 2. 공개 API (`codemap_search::jev`)

```rust
pub trait Evaluator: Send + Sync {
    fn evaluate(&self, request: EvaluationRequest) -> BoxFuture<'_, Result<EvaluationOutcome, EvaluationFailure>>;
}
pub struct JevEvaluator;                       // new(Arc<dyn Transport>, EvaluatorConfig) / https(SecretString, EvaluatorConfig, HttpsSettings)
pub struct EvaluationRequest;                  // new(state: Value, questions: Vec<Question>) -> Result<_, JevError>
                                               // with_policy / with_deadline_at(Instant) / with_deadline(Duration: 호출 시점 기준 변환) / with_cancel
                                               // state() -> &Value, state_field(name), questions(), policy()
pub struct RequestPolicy { pub deadline_at: Option<tokio::time::Instant>, pub cancel: Option<CancelToken> }
pub const TASK_QUERY_FIELD: &str = "task_query";   // adapter 관례일 뿐 런타임 요구사항이 아님
```

- state: 비어 있지 않은 string, object(빈 것 허용), array. null/bool/number는 `InvalidState`로 dispatch 전 거부. 런타임은 state를 그대로 전송하며 어떤 필드도 추가·해석하지 않는다.
- 질문: 요청 내 유일한 `QuestionId`(1–64자 `[A-Za-z0-9_.:-]`), Score 2–10 등급(문자열/객체), Choice 2–255 선택지(문자열/객체/배열/null), Noul 선택적 true/false criteria. instructions는 비어 있지 않은 string/object/array.
- 결과: `EvaluationOutcome { model, answers, raw_answers, usage, timing, requests }`, 실패: `EvaluationFailure { error, usage, timing, requests }`. `raw_answers`는 provider 답변 객체 원본(legend 포함)이다.
- 내부(`pub(crate)`): `batch::pack`, `answer::parse_answer/parse_usage`, `HttpsTransport::with_endpoint_for_tests`(`#[cfg(test)]`만 컴파일).

## 3. wire 형식과 검증

배치 본문: `{"model":"jev-1.13.0","questions":{"<id>":{"type","instructions","criteria"?}},"state":<caller state>}`. endpoint `POST https://api.typesafe.ai/v1/systemone`, `Authorization: Bearer`, `Content-Type: application/json`, `User-Agent: codemap-search/<version>`, HTTP/1.1.

| 기본형 | 입력 검사 | 응답 검사 |
| --- | --- | --- |
| Score | 2–10 등급 | `type=score`; 모든 등급 key(`"0"`…`"n-1"`) 존재, 추가 key 없음; 각 p 유한 [0,1]; 합 ≤ 1 ± `distribution_sum_tolerance(n)`; score 유한 [0, n-1] ± 0.02(그 뒤 clamp); `|score − Σ i·p_i| ≤ score_weighted_sum_tolerance(n)`; confidence 선택·유한 [0,1]; `legend` 등 추가 필드는 raw에만 보존(입력 object echo를 가정하지 않음) |
| Choice | 2–255 선택지 | `type=choice`; 확률 key 집합 = 입력 선택지 집합; 각 p 유한 [0,1], 합 허용 오차 내; `choice`가 선택지 중 하나이고 `p(choice) ≥ max − 0.01` |
| Noul | criteria 선택 | `type=noul`; `noul` 유한 [0,1]; confidence는 요구·합성하지 않음 |

- 허용 오차 근거: 기록된 응답(score/confidence)과 05 인계의 실제 응답 확률은 소수 둘째 자리 → `PROBABILITY_PRECISION = 0.005`. 합 허용 오차 `max(0.02, n·0.005)`, Score 가중합 허용 오차 `0.005·n(n−1)/2 + 0.005`(4등급 0.035, 10등급 0.23), Choice 동률 허용 0.01. 재정규화는 하지 않는다.
- 응답 전체: `model`이 설정 모델과 정확히 일치(`ModelMismatch`), `answers` key 집합이 batch 질문 ID 집합과 정확히 일치(`IncompleteAnswers { missing, unexpected }`), JSON 어디에서든 duplicate key가 있으면 `InvalidResponse`(serde_json `Value`의 last-wins 덮어쓰기를 막는 `NoDuplicateKeys` walker가 먼저 실행). usage는 `usage.input_tokens`/`output_tokens` 둘 다 있을 때만 알려진 값, 아니면 unreported(0이 아님).

## 4. 제한과 스케줄링 (01/04 공통 범위)

| 정책 | 초기값 | 허용 범위 | 소비 위치 |
| --- | --- | --- | --- |
| encoded batch bytes | 80,000 | 1..=`MAX_BATCH_BYTES`(80,000) | `batch::pack` (실제 UTF-8 JSON 크기; 들어가지 않는 질문은 `QuestionTooLarge`) |
| in-flight HTTP | 3 | 1..=`MAX_IN_FLIGHT_REQUESTS`(3) | `JevEvaluator.permits`(semaphore, owned permit — 모든 종료 경로에서 반환) |
| request spacing | 300ms | ≥ `MIN_REQUEST_SPACING`(300ms) | `next_start` 슬롯(평가기 인스턴스 내 모든 호출·batch 공유) |
| 전체 deadline | 45,000ms | 양수·≤ `MAX_SAFE_DURATION`(7일)·`Instant` 변환 가능 | evaluate 진입 시 `min(caller.deadline_at, now + deadline)`; 큐·간격 대기·HTTP·검증 전부 포함(`timeout_at`) |
| idle timeout | 30,000ms | 양수·≤ 7일 | `reqwest::ClientBuilder::pool_idle_timeout` (실제 소켓 검증 §6) |
| 자동 재시도 / redirect | 없음 / 없음 | 고정 | `retry::never()`, `redirect::Policy::none()` |
| 질문 수 / batch 수 / 응답 body | 8,192 / 128 / 4 MiB | 고정 상수(사용자 키 없음) | `EvaluationRequest::new` / `pack` / transport chunk 읽기 + `interpret` |

근거: 128 batch × 300ms 간격은 최소 38.1초의 dispatch 시간이라 기본 45초 예산에 근접하고, 128 × 80,000 = 10.24 MB가 메모리 상한이 된다. 8,192개 질문은 최소 ~120바이트 질문 기준 약 1 MB로 위 한도 안이며 응답도 4 MiB 아래다. 한도 초과는 `TooManyQuestions`/`TooManyBatches`/`InvalidResponse`/`Transport(ResponseTooLarge)`의 typed 실패이며 후보 생략이 아니다.

token 추정: tokenizer 미공개 → `ceil(bytes/3)`(`ESTIMATED_BYTES_PER_TOKEN = 3`). 영어 산문에는 보수적(≈4바이트/토큰), 코드·JSON 구두점에는 근사, CJK는 3바이트≈1토큰이라 거의 정확. `state + all questions` ≤ 64,000, `state + longest question` ≤ 32,000을 batch마다 별도 검사(`EstimatedTokenLimit { kind }`). byte 검사는 token 보장이 아니며 provider 422는 `Rejected { excerpt ≤256자 }`로 반환하고 자르지 않는다.

취소: `CancelToken`은 다음 await 지점에서 호출을 끝내고 남은 batch task를 `JoinSet::shutdown`으로 정리한다. 이미 dispatch된 요청 identity(`http_status: None`)와 알려진 usage는 유지된다. 취소는 이미 전송한 요청의 과금을 취소하지 않는다.

## 5. 오류와 계측

`JevError::kind()` 라벨: `invalid_state`, `empty_questions`, `too_many_questions`, `too_many_batches`, `duplicate_question_id`, `invalid_question_id`, `invalid_instructions`, `invalid_criteria`, `invalid_config`, `question_too_large`, `estimated_token_limit`, `cancelled`, `deadline_exceeded`, `transport`(Client/Connect/Timeout/ResponseTooLarge/Io), `unauthorized`(401/403), `rejected`(422), `rate_limited`(429), `overloaded`(529), `http_status`, `model_mismatch`, `invalid_response`, `incomplete_answers`, `invalid_answer`, `internal`.

| 경우 | 실제 동작 (테스트) |
| --- | --- |
| 입력·예산 오류, dispatch 전 취소·소진된 deadline | HTTP 0회, usage 비어 있음 (`request_validates_state_shape_unique_ids_and_question_count`, `pack_*`, `one_absolute_deadline_is_shared_by_consecutive_requests`) |
| 정상 batch 후 다음 batch 실패 | 완료 batch usage 유지, 전체 실패, 새 dispatch 없음 (`a_failed_batch_stops_new_dispatch_but_keeps_completed_accounting`, `deadline_counts_queue_time_and_reports_completed_work`) |
| dispatch 후 timeout/취소 | attempted identity(status None), usage unknown(`is_empty`) (`caller_deadline_shorter_than_the_configured_one_wins`, `cancellation_stops_the_call_and_the_evaluator_stays_usable`) |
| 잘못된 답변 + 유효 usage | 실패 + usage 보존 (`invalid_answer_fails_the_batch_but_keeps_usage_and_identity`) |
| 401/403/422/429/529/기타 | 구분된 variant, 재시도 없음 (`http_statuses_and_transport_errors_map_to_bounded_errors`, 로컬 소켓 429/302) |
| 초과 body / 잘못된 JSON / duplicate key | bounded 읽기 후 실패, 원문 미출력 (`response_bodies_above_the_limit_fail_without_parsing`, `duplicate_answer_keys_are_rejected_before_any_answer_is_read`, `https_client_abandons_bodies_above_the_limit`) |

`Timing { elapsed, queue_wait, http, request_count }`는 wall time·큐/간격 대기·HTTP 합계를 구분한다(병렬 HTTP 합은 wall time과 다를 수 있음). `RequestIdentity { batch_index, question_ids, request_bytes, request_sha256, http_status, http_elapsed, queue_wait }`. `Debug`는 `Question`/`EvaluationRequest`/`SecretString`/`HttpsTransport`/`JevEvaluator`에서 근거·key를 출력하지 않는다(`debug_output_hides_evidence_and_credentials`).

## 6. 의존성·TLS·transport 판단

- `reqwest 0.13.5`(`default-features = false`, `rustls-no-provider`) + `rustls 0.23.45`(`ring`, `std`, `tls12`, `logging`), `hyper 1.11.1`, `hyper-rustls 0.27.10`, `rustls-platform-verifier 0.7.0`(OS trust store). lockfile에 `openssl`/`openssl-sys`/`aws-lc-*` 없음(`openssl-probe 0.2.1`는 pure-Rust 인증서 디렉터리 탐색 도우미). 릴리스 워크플로(`.github/workflows/codemap-search-release.yml`)의 7개 target(gnu, musl, cross aarch64-musl, mac 2종, windows 2종)에 OpenSSL 설치 요구가 추가되지 않는다 — 워크플로는 조사만 했고 실행하지 않았다.
- redirect: `Policy::none()` — 302+Location 응답은 `Http { status: 302 }`로 반환되고 두 번째 요청이 없다(로컬 소켓 검증). retry: `retry::never()` — reqwest 0.13의 기본 "protocol NACK 재전송"과 hyper의 stale connection 재전송을 끈다. 서버가 응답 후 연결을 닫아도 다음 요청은 새 연결에서 정확히 한 번만 전송된다(로컬 소켓 검증).
- 실제 소켓 검증(`#[cfg(test)]` 로컬 HTTP/1.1 listener, 127.0.0.1, 외부 endpoint 없음): (1) 두 요청이 한 idle 연결을 재사용(연결 1개, 요청 2개), 헤더·본문·sha256이 identity와 일치; (2) `pool_idle_timeout = 100ms`로 400ms 유휴 후 새 연결(연결 2개) — 설정이 최종 client에 전달됨을 실제로 확인; (3) 429 재시도 없음, 302 미추종; (4) 4 MiB + 1 응답은 `ResponseTooLarge`. TLS handshake·실제 provider·플랫폼별 pooling은 검증하지 않았다(§8).
- 예제 `--live`는 환경 변수 `TYPESAFE_API_KEY`를 host 코드(예제 main)에서만 읽는다. `--mock`은 환경을 조회하지 않는다.

## 7. 실행한 명령과 결과 (cwd: `apps/codemap-search`, 2026-09-23, 네트워크·key 없음)

| 명령 | 결과 | 로그 |
| --- | --- | --- |
| `cargo test --lib jev` | exit 0 — 93 passed; 0 failed (jev::tests 42 = 오프라인 38 + 로컬 소켓 4, config::jev 4, tools::overview::jev 22, tools::search::jev 25; 274 filtered out) | `validation/jev-native/logs/lib-jev.log` |
| `cargo run --example jev_decisions -- --mock` | exit 0 — `relevance (score): 2.00`, `body (choice): keep P(keep)=0.90`, `helps_task (noul): 0.90`, `usage: input_tokens=296 output_tokens=20`, `request 0: 3 question(s), 1169 bytes, http_status=200`, `checks: passed (score, choice, noul, usage, request count)` | `logs/example-mock.log` |
| `cargo run --example jev_decisions -- --bogus` | exit 2 (사용법 오류, 전송 없음) | `logs/example-bogus.log` |
| `env -u TYPESAFE_API_KEY cargo run --example jev_decisions -- --live` | exit 1 — `live mode needs the TYPESAFE_API_KEY environment variable` (전송 없음) | `logs/example-live-nokey.log` |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets` | exit 0, 경고 0 | `logs/check-all-targets.log` |

R01–R09 대응:

| ID | 테스트 |
| --- | --- |
| R01 | `mixed_round_trip_returns_typed_answers_usage_and_request_identity`, `noul_answer_ignores_confidence_and_never_requires_it`, `noul_wire_shape_omits_absent_criteria`, `https_client_reuses_one_idle_connection_and_sends_the_documented_headers`(실제 encoded shape) |
| R02 | `score_question_enforces_the_published_level_limits`, `choice_question_enforces_option_limits_and_keys`, `instructions_must_carry_content`, `parse_answer_rejects_malformed_answers_with_a_reason`(NaN/범위/분포/가중합/Choice 최댓값), `parse_answer_accepts_values_within_the_recorded_tolerances`, `duplicate_answer_keys_are_rejected_before_any_answer_is_read`, `model_mismatch_incomplete_sets_and_non_json_bodies_are_explicit` |
| R03 | `pack_splits_greedily_on_the_exact_byte_ceiling`, `pack_rejects_a_single_question_over_the_ceiling`, `pack_reports_the_total_token_limit`, `pack_reports_the_state_plus_longest_question_limit`, `pack_bounds_the_number_of_batches`, `request_validates_state_shape_unique_ids_and_question_count`(8,193개 질문), `response_bodies_above_the_limit_fail_without_parsing` |
| R04 | `request_spacing_and_in_flight_limits_bound_dispatch`(시작 0/300/600/1000/1300ms, 동시 최대 3), `request_spacing_is_shared_across_sequential_calls`, 모든 실패 테스트의 `available_permits() == 3` |
| R05 | `caller_deadline_shorter_than_the_configured_one_wins`, `caller_deadline_longer_than_the_configured_one_is_clamped`, `one_absolute_deadline_is_shared_by_consecutive_requests`, `cancellation_stops_the_call_and_the_evaluator_stays_usable`, `cancel_token_resolves_for_early_and_late_cancels` |
| R06 | `deadline_counts_queue_time_and_reports_completed_work`, `a_failed_batch_stops_new_dispatch_but_keeps_completed_accounting`, `invalid_answer_fails_the_batch_but_keeps_usage_and_identity`, `usage_sums_keep_unknown_totals_unknown` |
| R07 | `http_statuses_and_transport_errors_map_to_bounded_errors`, `https_client_never_retries_or_follows_redirects`, `debug_output_hides_evidence_and_credentials`, `error_kinds_are_stable_secret_free_labels` |
| R08 | `evaluator_config_defaults_carry_the_initial_safety_policy`, `evaluator_config_rejects_values_outside_the_initial_policy`(설정 범위·HttpsSettings 검증), `https_client_replaces_a_connection_idle_beyond_the_configured_timeout`(실제 idle 교체), `https_client_abandons_bodies_above_the_limit` |
| R09 | 예제 `--mock` 실행(위 표); fixture 불일치 시 nonzero exit는 `check()` 구현으로 보장(불일치 fixture 실행은 별도 fixture를 만들지 않아 미실행) |

## 8. 미실행·한계

- 실제 provider 요청, TLS handshake, 실제 endpoint의 idle 연결 동작, 실제 답변 품질·속도는 검증하지 않았다. 로컬 소켓 검증은 평문 HTTP/1.1이며 TLS 계층은 라이브러리 구성만 확인했다.
- 플랫폼: macOS(darwin 24.6.0)에서만 실행했다. Linux/musl/Windows 빌드는 워크플로 조사에 그친다.
- token 한도는 추정이다(§4). provider tokenizer가 없으므로 64k/32k 준수를 보장하지 않는다.
- 공식 문서의 현재 상태(모델 alias, legend 표현, 한도 변경)는 이번 실행에서 재조회하지 않았다.
- `--live`는 실행하지 않았다(운영자의 별도 명시적 지시 필요).
