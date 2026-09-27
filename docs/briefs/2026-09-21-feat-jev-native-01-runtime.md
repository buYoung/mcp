# [feat] 재사용 가능한 Rust Jev 판단 런타임

## 작업 유형
feat

## 목적과 기준 상태
- [상위 브리프셋](2026-09-21-briefset-jev-native.md)의 시작 점검과 SC-01–SC-06을 따른다. 원래 기준 `97e3ebc8e`는 Rust library/Tokio current-thread를 사용하며 Jev 모듈과 직접 HTTP client는 없다.
- `feat/codemap-jev`의 기존 구현은 재사용 후보이다. 이 문서의 현재 계약을 충족하는지 조사·재검증하고 필요한 부분만 수정한다. 모듈이 없다고 가정해 중복 구현하지 않는다.
- Score·Choice·Noul 질문을 명시적 입력만으로 평가하고, 정상·실패 양쪽에서 typed 결과·알려진 사용량·시간·요청 정체성을 반환한다.
- 독립 Rust 예제와 두 어댑터가 같은 인터페이스를 사용한다. MCP, index handle, cwd, parser, 전역 설정·작업 의도에 의존하지 않는다.

## 범위와 소유권
- 포함: 공통 타입·평가기 추상화, 실제 async HTTPS transport, 배치·검증·마감 시각·취소·동시성·간격·연결 정책, mock 주입, 독립 예제, 이 계층의 회귀 검증.
- 제외: 파일 추천 기준, 검색 보호/생략 기준, `task_query`의 MCP 검증, 설정 로더·환경 변수 조회, 별도 크레이트 발행·영속 cache·provider 확장.
- 소유 파일: `apps/codemap-search/Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/jev/`, `examples/jev_decisions.rs` 및 최소 비민감 example fixture.
- 인계 파일: `apps/codemap-search/validation/jev-native/01-runtime.md`.
- PoC의 `jev_transport.py`, `improved_proxy.py`는 상위 시작 점검에서 확인한 경로/리비전으로 읽는다. 예전 key나 사적 source를 가져오지 않는다.

## 구현 계약

### R-C1. 공통 API와 책임
구체 타입/메서드 이름은 Stage 1에서 기존 구현과 호환되게 고정한다. 다음 정보와 책임은 빠지면 안 된다.

| 경계 | 필수 계약 |
| --- | --- |
| 질문 입력 | 요청 내 유일한 ID, Score/Choice/Noul 구분, string/object/array instructions, 해당 기본형의 typed criteria |
| 평가 입력 | 명시적 JSON state(string/object/array), 질문 집합, caller의 monotonic deadline/cancellation; root/filesystem 정보 불필요 |
| 평가기 | 비동기 평가 인터페이스; HTTP 없는 fake evaluator와 실제 평가기+fake transport를 각각 주입 가능 |
| 정상 결과 | 모델 ID, ID별 typed/raw 답변, 사용량의 알려진 합계와 미확인 여부, 요청 목록, 단계별 시간 |
| 실패 결과 | typed reason, 성공한 batch/dispatch된 요청의 계측, 부분 답변의 불완전성; 완성된 결과로 사용할 수 없음 |
| transport | host가 전달한 자격 증명과 고정 endpoint로 HTTPS 수행; 환경 변수/전역 설정 직접 조회 금지 |
| adapter 경계 | 질문/근거/정책 버전과 후보 의미는 adapter 소유; 공통 모듈은 불투명한 ID와 입력 fingerprint를 전달 |

`task_query`라는 코드 탐색 전용 필드를 공통 평가기의 필수 최상위 인자로 만들지 않는다. 두 어댑터는 이를 state에 넣고, 다른 Rust caller는 자신이 정한 state를 사용할 수 있어야 한다.

입력 state와 질문은 요청 중 소유되거나 불변으로 유지한다. caller의 취소 신호를 다른 token으로 덮어쓰지 않는다. 추가 timeout은 caller 신호와 조합하고 더 이른 제한을 적용한다.

### R-C2. wire 형식과 검증
- 초기 모델은 `jev-1.13.0`, endpoint는 `POST https://api.typesafe.ai/v1/systemone`이다. `model/state/questions` envelope와 Bearer 인증을 사용한다. alias는 초기 pinned 계약에 포함하지 않는다.
- ID에 파일명이나 질문 의미를 넣었다는 이유로 instructions에서 그 의미를 생략하지 않는다. 빈/중복 ID와 지원하지 않는 최상위 state·instructions 타입은 dispatch 전에 거부한다.
- 독립 질문은 같은 state로 묶되 각각 자신의 근거를 가진다. 배치 분할은 질문 의미나 ID/근거 연결을 바꾸지 않는다.

| 기본형 | 입력 검사 | 응답 검사 |
| --- | --- | --- |
| Score | 2–10개의 독립적으로 이해 가능한 등급 설명; 문서가 허용한 structured criteria | `type=score`, 범위 내 유한 score, 모든 등급의 probability/legend key, 유한 [0,1] 확률과 confidence, 확률 합과 가중합의 일관성 |
| Choice | 문서가 지원하는 비어 있지 않은 선택지 집합, 최대 255개; option별 string/object/array/null 설명 | `type=choice`, 입력 option과 동일한 확률 key 집합, 유한 [0,1] 확률/confidence, 선택값이 최고 확률 option 중 하나 |
| Noul | yes/no 의미가 정렬된 instructions, 선택적 true/false criteria | `type=noul`, 유한 [0,1] noul; confidence를 요구하거나 합성하지 않음 |

- 응답 모델 ID와 질문/답변의 정확한 ID 집합을 확인한다. JSON duplicate key가 map 변환 과정에서 조용히 덮어써지지 않도록 검사한다. 누락·추가·중복 ID, 잘못된 타입은 전체 평가 실패이다.
- Score는 `sum(level_index * probability)`와 score를 비교한다. 확률 합·Score 가중합·Choice 동률에 사용할 허용 오차를 실제 문서/응답 정밀도에 맞춰 Stage 1에 기록한다. 반올림을 무시한 과도한 정밀도 요구나 잘못된 응답의 임의 재정규화는 하지 않는다.
- structured Score criteria의 응답 legend는 현재 API 표현을 따른다. 입력 object를 그대로 echo한다고 가정하지 않는다. 필수 필드와 허용 확장 필드를 인계에 구분한다.
- confidence는 분포 집중도이며 정답률·행동 허가가 아니다. 공통 모듈은 threshold로 추천/생략을 결정하지 않는다.

### R-C3. 제한과 스케줄링

| 정책 | 초기값 | 구현 규칙 |
| --- | --- | --- |
| encoded batch bytes | 80,000 bytes | envelope/state/questions를 실제 UTF-8 JSON으로 직렬화한 크기; 더 작은 caller/host 상한 우선 |
| in-flight HTTP | 최대 3개 | 평가기 인스턴스 내 호출·batch에 공유; permit은 모든 종료 경로에서 반환 |
| request spacing | 최소 300ms | 같은 평가기에서 HTTP 시작 간격을 공유; task 생성 간격으로 대신하지 않음 |
| 전체 Jev timeout | 기본 45,000ms | 상위 SC-03의 절대 마감 시각 사용; host가 정한 유효 제한보다 caller가 늘릴 수 없음 |
| idle timeout | 기본 30,000ms | 선택한 HTTP 라이브러리의 실제 pool 설정에 전달 |
| 자동 재시도 | 없음 | 429/529, timeout, stale connection, validation failure가 자동 새 POST를 만들지 않음 |

- 80,000 bytes/3개/300ms는 초기 안전 정책이다. 04는 byte·동시성 상한을 줄이거나 간격을 늘릴 수 있게 연결한다. timeout/idle 값은 양의 정수·안전한 시간 변환 범위로 검증한다. 지원 범위를 01/04에서 다르게 해석하지 않는다.
- 별도로 전체 질문 수, batch 수/누적 메모리, 단일 응답 body bytes의 유한 상한을 Stage 1에서 정하고 근거와 단위를 인계한다. 새 사용자 설정 키를 자동으로 추가하지 않는다. 한계를 넘으면 typed budget failure이지 후보 생략이 아니다.
- 질문 하나와 공유 state가 제한을 넘으면 명시적 오류를 반환한다. 질문 의미를 바꾸는 분할은 adapter 책임이다. 런타임은 source나 criteria를 임의 축약하지 않는다.
- `state + all questions` 64k tokens와 `state + longest question` 32k tokens를 별도 검사/추정한다. tokenizer가 없으면 추정식·언어/JSON overhead 한계·provider rejection 처리를 기록한다. byte 검사를 token 보장의 대체물로 쓰지 않는다.
- 전체 질문당 unbounded task를 spawn하지 않는다. 제한된 in-flight 집합과 bounded packing으로 처리한다. 큐/간격 대기에도 timeout·취소를 적용한다.
- 첫 필수 batch 실패 시 새 dispatch를 멈추고 남은 작업을 정리한다. 이미 전송한 요청의 식별/알려진 계측은 유지하되 부분 답변을 성공으로 반환하지 않는다.
- monotonic deadline은 모든 batch에서 동일하다. adapter의 두 번째 평가에도 같은 deadline을 받으며 Duration을 새로 더해 연장하지 않는다.

### R-C4. 오류와 계측

| 경우 | 처리/계측 |
| --- | --- |
| 로컬 입력·예산 오류, dispatch 전 취소 | HTTP 0회, 알려진 전송 사용량 0, 구체 reason |
| 정상 batch 후 다음 batch 실패 | 완료 batch의 알려진 사용량 유지; 전체 결과는 실패 |
| dispatch 후 timeout/취소/연결 단절 | 요청은 attempted로 남김; 미보고 token 사용량은 unknown |
| 잘못된 답변이지만 usage는 해석 가능 | 답변은 실패, 유효하게 읽은 usage는 알려진 사용량으로 보존 |
| 401/422/429/529 또는 기타 HTTP 실패 | 인증/검증/rate limit/과부하/기타 상태를 구분; 자동 재시도 없이 반환 |
| response body 초과/잘못된 JSON | bounded 읽기 후 실패; raw 오류 body를 진단에 출력하지 않음 |

- 성공·실패 모두 입력/출력 token의 알려진 합계, 보고/미보고 요청 수와 총량 완전성, dispatch 수를 제공한다. 미보고 하나 때문에 알려진 합계를 지우지도, 미보고를 0으로 더해 확정 총량이라 하지도 않는다.
- elapsed wall time, queue/spacing 대기, HTTP 작업 시간을 구분한다. 병렬 HTTP 시간의 합은 wall time과 다를 수 있다.
- 정확한 encoded 요청 fingerprint, 모델, 요청 ID, 상태, 시도 시점을 결과에 연결한다. 원시 요청/응답·key는 Debug/tracing/에러 문자열에 자동 출력하지 않는다.
- endpoint redirect와 transport 내부 POST retry 정책을 확인한다. 인증·source가 다른 endpoint로 전달되는 자동 redirect를 허용하지 않는다. client 구성만으로 외부 요청이 시작되어서는 안 된다.

### R-C5. 독립 예제
- `examples/jev_decisions.rs`는 기본 실행과 `--mock`에서 네트워크·key·MCP·Python 없이 공통 평가기의 실제 직렬화/검증 경로를 실행한다.
- 비민감 mixed-response fixture로 Score=2.0, Choice=keep, Noul=0.9, fixture usage와 요청 수를 assert한다. 결과를 출력만 하지 않고 불일치 시 nonzero exit한다.
- 선택적 `--live`만 host에서 환경 자격 증명을 읽어 실제 전송할 수 있다. mock 모드는 환경 key를 조회하지 않는다. 이 브리프 실행의 기본 검증은 live를 실행하지 않는다.

## 실행 단계

### Stage 1 — API·제한·실패 계약 고정
- 시작: 상위 시작 점검 완료; 기존 모듈과 두 adapter 브리프를 확인했다.
- 작업: 위 R-C1–R-C4에 대응하는 타입/함수, 허용 오차, 추가 유한 상한, tokenizer/추정 방법, HTTP/TLS 라이브러리·버전·MSRV·릴리스 target 호환성을 결정한다. provider 문서는 참고 자료에서 필요한 부분만 읽는다.
- 산출물: `01-runtime.md` 초안에 확정 계약, 라이브러리 지원 API, fake evaluator/transport 주입 경로, 값의 최종 전달 위치를 기록한다.
- 검증: 계약 표를 두 adapter의 Score/Choice/Noul 소비와 대조한다. generic evaluator가 task_query나 parser를 요구하면 수정한다.
- 종료: 타입/단위/실패 계측·절대 deadline·모든 상한이 구체적이고 downstream이 이를 해석할 필요가 없다.
- 재계획: 별도 크레이트나 기존 공개 API의 파괴적 변경이 필요할 때만 해당 범위를 상위로 돌린다.

### Stage 2 — bounded 런타임 구현·검증
- 시작: Stage 1 계약 고정.
- 작업: injectable async transport, wire 검증, bounded scheduling, deadline/cancellation, 실패 계측을 한 경로로 구현한다. 실제 transport를 거치지 않는 mock만으로 transport 동작을 입증했다고 하지 않는다.
- 검증: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev` — 아래 R01–R08을 실행하고 0개 통과를 허용하지 않는다.
- 종료: 오프라인 시나리오 통과, permit/작업 정리 확인, provider traffic 0회.
- 재계획: Tokio blocking, unbounded 저장/작업 생성, caller 신호 손실이면 먼저 공통 계약을 수정하고 adapter 시작을 보류한다.

### Stage 3 — 재사용 증명과 인계
- 시작: Stage 2 통과.
- 검증: `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock` 및 `cargo check --manifest-path apps/codemap-search/Cargo.toml`.
- 산출물: `01-runtime.md`에 최종 API·기본값/범위·검증 결과·예제 실행·의존성/TLS 판단·미검증 플랫폼/실제 pooling 한계를 기록한다.
- 종료: 02/03이 엔진·MCP·전역 설정 없이 평가기를 호출할 수 있다.
- 무변경 경로: 기존 구현이 모든 계약과 회귀/예제를 만족하면 코드를 재작성하지 않고 현재 검증 증거를 인계한다.

## 필수 검증 시나리오
| ID | 입력/상황 | 확인할 결과 |
| --- | --- | --- |
| R01 | mixed Score/Choice/Noul, structured instructions/criteria | 실제 encoded shape와 typed 답변; ID가 의미를 대신하지 않음; Noul confidence 불필요 |
| R02 | criteria 경계, 누락/추가/중복 ID, model/type 오류, NaN/범위/분포/가중합/Choice 최댓값 모순 | typed failure; 정해진 허용 오차의 정상 반올림·동률은 수용 |
| R03 | byte 경계, 큰 단일 질문/state, 두 token 제한, 누적/응답 상한 | silent truncation 없이 실패; dispatch 전 오류는 HTTP 0회 |
| R04 | 여러 batch·동시 호출·permit 대기·간격 대기 | 최대 3개, 최소 300ms, 같은 deadline; 종료 후 permit 누수 없음 |
| R05 | 사전/대기/HTTP 중 취소·짧은 deadline | caller 제한 우선, 새 요청 중단, typed failure와 알려진/unknown usage 보존 |
| R06 | 한 batch 성공 후 다른 batch 실패/누락 usage | 알려진 합계·attempt 목록 보존; 부분 성공을 전체 성공으로 반환하지 않음 |
| R07 | 401/422/429/529, malformed/큰 응답, transport 오류 | 재시도 0회, 안전한 reason, Debug/진단에 key·source·오류 body 없음 |
| R08 | idle/connect/request 설정과 실제 transport 구성 | 30,000ms idle 등 설정이 최종 client에 전달됨; mock scheduler 검사와 실제 socket pooling 검증을 구분 |
| R09 | 독립 예제 | 세 기본형·사용량 assertions가 실행되고 불일치 fixture는 실패 |

R08의 실제 socket 동작은 가능하면 test-only 로컬 transport 경계에서 확인한다. 외부 provider나 production endpoint override는 필요하지 않다. 라이브러리 설정/문서 확인만 했다면 그 한계를 명시하고 실제 idle 연결 교체를 테스트했다고 쓰지 않는다.

## 완료 조건과 부수 효과
- [x] R01–R09의 필수 오프라인 검사와 패키지 cargo check가 실제 통과한다.
- [x] 모든 초기 정책 값이 해당 scheduler/client/validator에서 소비되고 04에 전달할 범위가 고정된다.
- [x] 공통 모듈에 MCP·parser·index·workspace·전역 config import가 없다.
- [x] 기존 Rust library/binary 및 릴리스 target 계약을 유지하고 플랫폼별 OpenSSL 수동 설치를 요구하지 않는다.
- [x] 두 adapter가 실패 계측·raw 답변·마감 시각을 해석 없이 소비할 수 있다.
- [x] 실제 API 품질/속도·네트워크 pooling·플랫폼 검증 중 실행하지 않은 것은 인계에서 미실행으로 구분한다.

## 참고 자료
- [API](https://docs.typesafe.ai/api.md), [모델과 한도](https://docs.typesafe.ai/models.md), [state](https://docs.typesafe.ai/concepts/state.md).
- [Score](https://docs.typesafe.ai/primitives/score.md), [Choice](https://docs.typesafe.ai/primitives/choice.md), [Noul](https://docs.typesafe.ai/primitives/noul.md).
- 상위 브리프의 TypeSafe 스킬·설계 원칙과 실제 선택한 HTTP 라이브러리 문서를 기준으로 한다. 예제 SDK의 자동 retry 기본값을 이 제품의 정책으로 가져오지 않는다.
