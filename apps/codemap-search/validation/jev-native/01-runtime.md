# Jev 공통 평가기 검증

2026-09-23, 현재 소스에서 공통 Rust 평가기와 독립 예제 검증을 통과했다. 사용자 지시에 따라 worktree와 브랜치는 조회하지 않았다. 문서에 인용된 Python PoC 경로는 현재 디렉터리에 없어 실행하거나 변경하지 않았다.

## 재사용 계약

| 항목 | 구현 계약 |
| --- | --- |
| 진입점 | `jev::Evaluator::evaluate(EvaluationRequest, EvaluationOptions)` → 비동기 `Result<Evaluation, Failure>` |
| 구현·주입 | `Runtime::new(Arc<dyn Transport>, Policy)`; 호스트가 `HttpsTransport::new(api_key, &policy)`로 인증을 해결한다. 어댑터는 `&dyn Evaluator`를 받는다. |
| 입력 | 요청별 `request_id`, 명시적 JSON `state`, ID별 `Question` 사전. 작업 의도는 어댑터가 `state.task_query`로 전달한다. 전역 작업 상태 없음. |
| Score | 구조화된 지시문과 2–10개 기준. 값·확률·confidence·legend를 보존하고 단계 수, 확률 합, 가중값을 검증한다. |
| Choice | 1–255개 키와 문자열/객체/배열/null 기준. 반환 키·확률 집합·선택값을 검증한다. |
| Noul | 유한한 `[0,1]` 값. 별도 confidence가 없다. true/false 기준은 선택 사항이다. |
| 반환 | 고정 모델 ID, 요청 ID, typed answers, 입력/출력 토큰, 전체/HTTP 누적 시간(ms), 시작/완료 요청 수 |
| 실패 | `FailureKind`와 이미 확인된 사용량. 원문·키·HTTP 오류 본문은 오류에 담지 않는다. 부분 답변으로 성공하지 않는다. |
| 취소 | 복제 가능한 `Cancellation`; 절대 시각 `deadline`을 기본 제한보다 짧게 줄일 수 있다. 두 단계는 같은 옵션을 이어받는다. |

## 전송 정책과 한계

`jev-1.13.0`, HTTPS POST `/v1/systemone`, 최대 80,000바이트, 동시 요청 최대 3개, 시작 간격 최소 300ms, 대기 포함 최대 45,000ms, 유휴 연결 30,000ms. 자동 재시도와 redirect는 비활성이다. 최대 16,384개 질문·512개 배치·응답 2,000,000바이트로 제한한다. 증거를 잘라 넣지 않으며 초과하면 실패한다. 확률 합·가중 Score 오차 허용치는 0.001이다.

공식 토크나이저를 사용하지 않는다. JSON UTF-8 바이트를 토큰 대용으로 계산하고 1,024의 여유를 둔다. `state + longest question <= 32,000`, 전체 배치 `<= 64,000` 추정치를 각각 적용한다. 이는 정확한 토큰 수나 공급자 수용을 보장하지 않는다. 실제 context 거절은 HTTP 실패로 보존하며 재시도하지 않는다. 사용량을 알 수 없는 미완료/실패 요청의 비용은 0으로 확정하지 않는다.

`reqwest = 0.12.28`, 기본 기능 끔, `rustls-tls`/`json` 사용. reqwest 명시 MSRV는 1.64.0이며 전체 의존성은 로컬 `rustc 1.98.1`에서 검증했다. 릴리스 workflow의 stable Rust 및 Linux GNU/musl, macOS, Windows 대상에 OpenSSL 설치를 추가하지 않는다. 다른 플랫폼 빌드는 미실행이다. 유휴 연결은 reqwest pool 만료 정책으로 교체하며 이미 수행 중인 연결을 강제 종료하지 않는다.

## 실제 검증

작업 디렉터리: 저장소 루트. 모두 종료 코드 0.

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev`: 10개 통과. 세 원시형, 기준 수/키/분포/모델/응답 누락, 포장, 두 context 추정 경계, 공급자 거절, 시간 제한, 취소, permit 반환, loopback 유휴 연결 재사용/만료.
- `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock`: Score=2.0, Choice=keep, Noul=0.9, 입력 123/출력 11 토큰 fixture 확인. 자격 증명·Python·MCP·공급자 통신 없음.
- `cargo check --manifest-path apps/codemap-search/Cargo.toml`: library/binary 컴파일 통과.
- `cargo tree --manifest-path apps/codemap-search/Cargo.toml -p reqwest --edges normal --depth 1`: rustls 의존성 확인.

실서비스 속도·정확도·Noul 보정·플랫폼별 배포는 검증하지 않았다. 예제의 `--live`는 운영자가 명시적으로 선택할 때만 환경변수를 읽고 API를 호출한다.

최종 검증에서 포장을 질문별 바이트 합산으로 보완하고 실제 동시 최대 3개·시작 간격 300ms 회귀를 추가했다. 런타임 소유 검증은 11개이며, 어댑터·설정을 포함한 `--lib jev` 27개가 모두 통과했다. 현재 결과는 [전체 검증 기록](05-verification.md)과 [jev-unit.log](jev-unit.log)에 있다.

## 명세 근거

2026-09-23 확인: [HTTP API](https://docs.typesafe.ai/api.md), [모델과 context 제한](https://docs.typesafe.ai/models.md), [구조화된 state](https://docs.typesafe.ai/concepts/state.md), [ClientBuilder](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html), [reqwest metadata](https://docs.rs/crate/reqwest/0.12.28/source/Cargo.toml).
