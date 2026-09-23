# [test] 네이티브 Jev 기능과 재사용 계약의 전체 검증

## 작업 유형
test

## 목적과 기준 상태
- [상위 브리프셋](2026-09-21-briefset-jev-native.md)의 시작 점검과 SC-01–SC-06을 따른다. 01–04가 결합된 실제 Rust 경로를 검증한다.
- 기존 `tests/e2e_tests.rs`와 `tests/e2e/`의 MCP/config/search/codemap/redaction/기타 호환성 검사를 재사용한다.
- 과거 PoC에는 본문 보존·파일 heading 병합 관련 수정과 제외 시도가 있었다. 검증은 텍스트 전역 집합이 아니라 파일·원래 행·내용의 대응을 확인한다.
- offline 입력 구성과 fake-answer 정책 검증은 실제 모델 판단 품질·보정·속도의 증거가 아니다. 기존 구현 후보의 보고서나 체크 표시도 현재 소스의 검증을 대체하지 않는다.

## 범위와 소유권
- 포함: cross-feature 회귀, 최소한의 shared helper 보완, 실제 MCP 호출/최종 응답·관측 검증, 예제 실행·패키지 목록 확인, 향후 live 평가 방법 정의.
- 금지: 생산 로직을 이 단계에서 별도로 수정, 실제 provider 요청을 자동 테스트에 포함, 일회용 key 재사용, 사적 repository source fixture 커밋, release/publish, 새 lint/formatter 설정.
- 새 유료 benchmark, 성능 향상 백분율을 완료 기준으로 삼기, 광범위한 언어 품질 주장, default-on 결정은 범위 밖이다.
- 소유: `apps/codemap-search/tests/e2e/jev.rs`(필요 시), `tests/e2e/mod.rs`, 필요한 `tests/e2e/helpers.rs` 보완과 최소 비민감 fixture. 알고리즘 unit test는 01–03 소유자에게 둔다.
- 입력: `apps/codemap-search/validation/jev-native/01-runtime.md`부터 `04-integration.md`까지의 현재 인계.
- 출력: `apps/codemap-search/validation/jev-native/05-verification.md`.

## 검증 증거의 구분
| 층 | 무엇을 증명하는가 | 무엇을 증명하지 못하는가 |
| --- | --- | --- |
| 입력 구성 | 실제 serializer에 들어가는 state/questions/candidates, 근거 참조, masking, coverage, snapshot/source identity | 모델이 올바르게 이해하거나 판단한다는 사실 |
| 런타임·정책 | fake transport 응답 검증, 시간·취소·계측, threshold/보호/순위/출력 로직, MCP 연결 | 실제 의미 정확도·calibration·provider latency |
| 전송 라이브러리 | 최종 client 옵션·재시도/redirect/TLS 구성, 실행한 경우 로컬 socket 동작 | 실행하지 않은 실제 provider pooling·모든 플랫폼 호환성 |
| 실제 모델·전체 작업 | 별도 승인된 대표 입력에서 실제 판단과 최종 사용자 결과 | 다른 언어·repository·분포의 보편적 성능, 세 번 실행만으로의 안정성 보장 |

실패를 `missing_evidence/coverage`, `model_judgment`, `composition/rendering`, `service/transport`로 분리한다. 입력이 누락됐는데 모델 오류라고 하거나, 서비스 fallback을 모델의 정확한 no-match로 세지 않는다.

## 필수 통합 검증 행렬
행마다 실제 test 이름, 소유자, fixture 인원/파일/선언 수, 실행 명령·로그, 결과를 최종 인계에 붙인다. 아래 표 자체는 통과 증거가 아니다.

| 검증 영역 | 소유 계약 | 05가 actual pipeline에서 확인할 내용 |
| --- | --- | --- |
| 기본 호환성 | I01/I02/I08, F01 | 네 flag 조합, keyless/intentless, 잘못된 task_query, alias/scope, unrelated tool/CLI; 기대 HTTP 호출 수와 base 비교 |
| 실제 입력 조립 | R01, O02, F07 | fake evaluator만이 아니라 실제 evaluator+capturing transport에서 encoded state/questions를 확인; ID 대신 instructions에 의미와 근거가 존재 |
| root 전체 coverage | O01/O02/O04 | 24개 초과 파일, 많은 선언을 가진 파일의 마지막 fragment, 투영 누락·부재·masking; 자격이 limit보다 먼저 적용 |
| 추천 결과 | O03/O05 | no_match/동률/부족/혼합; 추천 0이면 역할 호출 0; unrelated 역할과 역할 서비스 실패 구분 |
| 재사용 | O08, F05 | 0.80의 threshold 0.70/0.90 replay, 순위 재계산으로 새 파일 선택; 새 파일 역할은 not_evaluated, 입력 변경은 reuse 거부 |
| body 완전성·정체성 | F02/F03/F09 | complete/partial/missing/oversized/unknown/masked, 인덱싱 이후 파일 변경과 평가 중 변경; 검증된 complete body는 실제 생략 가능 |
| 보호 closure | F04/F05 | Rust impl/struct/상수, TypeScript nested method, caller/callee chain, 같은 이름의 다른 파일, ambiguous link; Noul=1.00이어도 보호 |
| threshold 최종 전달 | I03/I04 | 실제 config 0.70/0.90과 invalid fallback, 지연 중 reload; adapter 입력뿐 아니라 최종 source 생략 여부가 달라짐 |
| deadline·실패 | R03–R07, O06, F06, I05 | multi-batch/두 단계가 같은 deadline 사용; Score 실패는 전체 fallback, 역할만 실패하면 파일 유지; 알려진/unknown usage |
| 출력 cap·fallback | O07, F01/F08, I06 | 한도에 찬 base에서 all-keep/timeout/invalid-answer; 진단 때문에 본문/heading/tail/안내 손실이나 새로운 cap 오류 없음 |
| 최종 source accounting | F10, I09 | 생략 body 없는 실제 응답, 파일+행 보존, metadata/marker-only 파일을 read로 세지 않음, analyze의 동일 단위 확인 |
| 마스킹·비민감 진단 | R07, I09 | 전송 전 모든 필드와 렌더링 후 masking, raw error/Debug/source/key 미노출, 로그에는 outcome·개수·사용량·시간 존재 |
| host lifecycle·설명 | I07 | tools/list 외부 접근 고지와 실제 활성화/reload 일치, lazy client/key 회전, unrelated 요청에서 생성/전송 없음 |
| 독립 재사용·패키징 | R09, I-C6 | 실제 mock 예제 assertions, 정상 binary의 주입 경로 차단, package 포함/제외 목록 |

### fixture와 비교 규칙
- 기존 비민감 Rust/TypeScript fixture를 먼저 사용하고 부족한 경계만 최소 입력으로 추가한다. 사적 PoC source·고유 benchmark 답변·key는 사용하지 않는다.
- 최소 하나의 ready root 추천과 하나의 complete 비보호 본문 생략을 actual adapter/transport 호출 횟수와 함께 assert한다. 모든 경로를 bypass/보호 처리하는 구현은 실패이다.
- 전부 무관/동률/근거 없음과 전부 유지도 별도 비어 있지 않은 입력에서 실행한다. "선택된 후보 없음"으로 원래 검증을 대체하지 않는다.
- baseline은 같은 binary/config/source snapshot에서 Jev off로 얻은 결과와 기존 호환성 fixture를 함께 사용한다. off/on을 모두 잘못 바꾼 새 구현끼리 비교해서 호환성을 입증하지 않는다.
- stats/readiness 같은 기존 변동 요소는 준비된 index/고정 설정으로 안정화한다. Jev 때문에 달라진 본문·heading·scope·cap·관측 정보를 정규화로 지워 버리지 않는다.
- 함수 이름뿐 아니라 `(file identity, original line number, redacted line content)`를 비교한다. 중복 brace나 같은 줄 텍스트가 다른 파일에 있다는 이유로 보존 판정을 하지 않는다.
- 생략 marker를 별도로 식별하고 변하지 않은 fixture에서 남은 이름·범위로 `read`했을 때 원래 body를 찾을 수 있는지 확인한다. 파일 변경 사례에서는 후속 read가 이전 캡처가 아니라 live source를 반환하는 기존 계약도 확인한다. source가 없는 파일/marker를 관측에 포함하지 않는다.
- 테스트가 고정하는 code/payload/policy 버전과 실제 소스 상태를 기록한다. HEAD만 같고 미커밋 구현이 다르면 같은 증거가 아니다.

## 대표 의미 사례와 향후 품질 평가
이 절은 평가 방법을 정의한다. 실제 provider 실행은 별도 명시적 지시 전까지 하지 않는다.

### 라벨 모집단
| 그룹 | 반드시 구분할 내용 |
| --- | --- |
| 직접 일치 | 직접 구현·직접 요청한 wrapper/설정/테스트 위치 |
| 지원 근거 | caller/consumer/contract, 간접 흐름, 순서·오류 처리 |
| 부정/반박 | 질문의 전제가 틀린 경우와 이를 반박하는 유용한 source |
| no-match | snapshot 안에 요청에 맞는 근거가 없는 질문 |
| 불명확/부족 | 질문 모호함, path-only/부분/masked/stale 근거; 강제로 no-match 라벨을 붙이지 않음 |
| 언어/길이 | 한국어·영어·혼합 의도, 긴 파일·많은 fragment·짧은 body |
| 비신뢰 내용 | docstring/comment의 유도 지시문, 근거 자신의 관련성을 주장하는 문구 |

- root 추천은 평가된 snapshot에서 relevant file 집합 `G`를 고정한다. source 전체가 아니라 indexed evidence만으로 가능한 판단이라는 한계를 라벨에 기록한다.
- body 필터는 query별 baseline의 실제 선택 body에 useful/unnecessary/ambiguous 라벨과 필요한 원래 행 근거를 붙인다. 구조적 필터 대상 여부, 보호 여부, unavailable 이유도 별도 기록한다.
- 라벨은 모델 출력 확인 전에 고정한다. 의견 불일치는 ambiguous로 남기거나 근거와 함께 판정하고, 모델 결과에 맞춰 사후 정답을 바꾸지 않는다.
- calibration query와 held-out evaluation query를 분리한다. 같은 코드 조각/유사 질문의 누출도 관리하고, 대표 언어·긍정/부정 그룹이 어느 분할에 있는지 기록한다.
- 한국어 문자열 전달 검증은 언어 정확도 검증이 아니다. 영어가 주 학습 언어라는 모델 문서의 한계를 사용자 문서/보고서에 반영한다. task_query를 자동 번역하는 기능은 추가하지 않는다.

### 지표 정의와 빈 분모
K는 1/5/10/24로 기록한다. `R_K`는 최종 실제 반환된 최대 K개 파일이다. 추천/필터 지표는 해당 mode에만 적용하고 Jev off의 미존재 추천을 0점 모델 판단으로 계산하지 않는다.

| 지표 | 정의 | 예외 처리 |
| --- | --- | --- |
| 반환 추천 정밀도@K | `count(R_K ∩ G) / count(R_K)` | 반환 0개는 N/A, 완벽한 정밀도가 아님. 고정 K 분모 지표와 혼동하지 않게 실제 반환 수 병기 |
| Recall@K | `count(R_K ∩ G) / count(G)` | G가 비어 있으면 N/A; matched 질문에 반환 0개면 0 |
| 잘못된 추천 비율 | labeled no-match 질문 중 추천이 하나라도 있는 수 / no-match 질문 수 | 모집단 없음은 N/A |
| 추천 누락 비율 | labeled matched 질문 중 추천이 없는 수 / matched 질문 수 | no_match/insufficient_evidence 결과를 분리해 병기 |
| 유용한 body 오생략률 | 최종 출력에서 필요한 body 근거를 잃은 useful body 수 / baseline의 labeled useful body 수 | 필터 적격 subset 분모도 별도 보고; 보호 전 mask와 보호·렌더링 후 실제 손실 구분 |
| 불필요한 body 제거율 | 최종 출력에서 완전히 생략된 unnecessary body 수 / baseline의 labeled unnecessary body 수 | protected/ineligible/ambiguous 수 병기; 일부 손실을 완전 제거로 세지 않음 |
| 구조적 보존 | 유지해야 할 파일/행 근거 중 실제 보존된 수와 손실 목록 | API 실패·cap 상황 포함; 이 보존이 최종 답변 정답률을 뜻하지 않음 |
| 효율 | 실제 최종 응답 bytes/token 감소, request/read 수, wall time | 품질 지표와 분리; 출력 감소 자체를 성공으로 판정하지 않음 |

- 위 의미 판단 지표의 분모에는 평가가 완료된 시도를 사용하고, service failure/coverage bypass는 별도 표에 원래 전체 시도 수와 함께 남긴다. 실패를 no-match 정답으로 세거나 성공 사례만으로 전체 서비스 성능을 주장하지 않는다.
- 전체 사용자 작업의 성공률·최종 답변 rubric·시간·비용은 fallback 포함 모든 시도에서 별도로 평가한다. 불명확한 라벨과 빠진 모집단은 N/A이며 0-error 증거가 아니다.
- Score 자격 기준과 Noul 0.70의 calibration은 별개이다. 0.70 시작 정책, 조정된 정책, held-out 결과를 분리한다. threshold를 조정한 데이터로 최종 품질을 보고하지 않는다.
- confidence는 보정된 정답률/사용자 허가로 해석하지 않는다. fragment 수·언어·근거 상태별 오류를 보고, 새 질문 분해/집계식이 필요한지는 그 이후 결정한다.

### 재현·실패 분석·비용 경계
각 시도에서 아래 정보를 남긴다. 완전한 입력/답변은 비민감 평가 산출물에만 보존하고 운영 source/key 로그는 만들지 않는다.
- source/index hash, binary revision과 미커밋 변경 식별, scope·출력 예산·cache 상태, model ID, evidence/question/criteria/policy 버전, threshold, 라벨 manifest.
- 실제 state/questions/candidate mapping과 fingerprint, raw 답변, 순위/retention/protection 결정, 최종 file+line 출력, applied/bypassed/fallback와 reason.
- 주 모델의 input total/cached/uncached/output/reasoning subset을 공급자 정의대로 구분한다. cached input을 total에 다시 더하거나 reasoning subset을 output에 다시 더하지 않는다.
- Jev input/output과 알려진 사용량/미확인 시도를 별도로 집계한다. 현재 모델 문서의 요금·확인 날짜·과금 단위를 기록해 비용을 계산하며 token throughput을 실제 비용으로 대신하지 않는다.
- whole-task time, 전체 Jev 단계 wall time, queue/HTTP 시간, 요청/도구/read 수, response bytes, 오류·fallback·제외 시도와 사유·알려진 사용량을 모두 포함한다.
- 이후 승인된 비교는 같은 Rust revision/source snapshot/budgets로 Jev off/#1/#2를 실행한다. mode별 세 번 실행 결과를 각각 남기고 일치율을 보고하되 세 번의 표본으로 안정성/일반 정확도를 보장하지 않는다.
- 과거 Python/rg 결과는 참고 열이다. Python Choice 결과를 Noul label/threshold calibration으로 쓰거나 서로 다른 index의 recall을 같은 모집단처럼 비교하지 않는다.

## 실행 단계

### Stage 1 — 현재 계약과 검증 행렬 연결
- 시작: 04 인계에 실제 통합 호출·설정·lifecycle·targeted 검사 결과가 있다.
- 작업: 01–04의 R/O/F/I 시나리오를 현재 test 이름/fixture와 연결하고 unit/integration/입력 조립/실제 socket/미실행 live를 구분한다.
- 산출물: `05-verification.md`의 초기 행렬과 fixture 수·소유자·정확한 명령. 없는 검증은 필요한 최소 case만 추가하도록 소유자에게 지정한다.
- 검증: 각 필수 행에 긍정/실패 경로와 non-empty assertion이 있는지 검사한다. mock answer만 보고 입력 구조 검증까지 통과라고 하지 않는다.
- 종료: 모든 필수 계약이 실제 검증 경로에 연결되고 자료의 민감성/네트워크 의존이 없다.
- 재계획: 계약 불일치/생산 결함은 담당 소유자에게 돌리고 관련 후속 검증을 멈춘다. 전체 작업을 무관한 결함 수정으로 확장하지 않는다.

### Stage 2 — 실제 Rust 통합·보존 검증
- 작업: 필요한 cross-feature case를 추가하고 baseline/최종 출력/계측을 비교한다. helper는 정상 production 요청에서 선택할 수 없는 injected boundary를 사용한다.
- 검증: 관련 변경을 확인한 뒤 다음을 실행한다. 01–03의 현재 소스와 동일한 성공 증거는 재사용할 수 있으나 소스/계약 변경 후 오래된 결과는 재사용하지 않는다.

```sh
cargo test --manifest-path apps/codemap-search/Cargo.toml --lib jev
cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview
cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::search
cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests
```

- 종료: 각 필터가 실제 필수 case를 선택하고 전부 통과한다. 필수 case의 ignored/fixture 없음/실패를 통과로 세지 않는다. 외부 provider traffic은 0회이다.
- 실패 처리: exact command·cwd·소스 상태·실패·부분 결과를 기록하고 해당 소유자에게 수정·재검증을 돌린다. assertion을 지워 통과시키지 않는다.

### Stage 3 — 예제·패키징·최종 증거
- 검증 명령:

```sh
cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock
cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets
cargo package --manifest-path apps/codemap-search/Cargo.toml --list --allow-dirty
```

- 예제는 컴파일만이 아니라 세 기본형과 usage assertions의 실제 실행을 확인한다.
- package list는 공통 모듈과 실행에 필요한 example fixture를 포함하고 기존 exclusions가 experiments/validation/사적 fixture를 제외하는지 확인한다. `--list`는 패키지 빌드·모든 release target·발행 성공의 증거가 아니다.
- `.github/workflows/codemap-search-release.yml`의 build/TLS/platform 조건을 조사하되 실행하지 않은 workflow/플랫폼은 미실행으로 적는다. release/publish는 하지 않는다.
- client의 30,000ms 설정 확인, fake transport의 scheduler 검사, 실제 socket idle 재사용 검증을 구분한다. 문서상 라이브러리 동작을 네트워크에서 관찰했다고 쓰지 않는다.
- 결과: 최종 행렬, 명령/exit code/검사·fixture 수, 로그, 실제 소스 상태, 사용자 문서 대조, 품질 평가 방법과 미실행 한계를 `05-verification.md`에 남긴다. 상위 담당자가 전역 완료를 판정한다.

## 최종 보고 형식
1. 검증 대상: cwd/branch/HEAD/변경 식별, binary/lockfile, config, fake evaluator/transport 모드.
2. 요구사항 행렬: R/O/F/I 연결, 실제 test 이름·fixture 수, 결과·증거 경로, 소유자 수정 이력.
3. 실행 결과: 명령·cwd·exit code·통과/실패/ignored 수, provider traffic을 막은 주입/구성 근거.
4. 실제 MCP 관찰: 대표 성공·no-match·all-keep·fallback·작은 cap·reload·source accounting.
5. 예제·패키징·문서: 실제 실행과 단순 검사·컴파일·미실행 항목의 구분.
6. 품질/벤치마크 방법: 라벨/분모/보정·평가 분리, 버전·raw 답변 재현, 모든 시도·비용 경계.
7. 남은 한계: 실제 품질/calibration/성능, 실제 transport pooling, 미실행 release target 등. offline 완료가 default-on 결정을 뜻하지 않음.

## 완료 조건
- [x] 모든 필수 offline 행렬과 영향받는 기존 e2e가 현재 소스에서 실제 통과하고 필수 빈 모집단/건너뜀/미해결 실패가 없다.
- [x] 실제 입력 조립과 정책/런타임, model quality의 증거가 분리되어 있다.
- [x] 기본 호환성, 독립 모드, 실제 긍정 추천/생략, 부정/불확실, 전체/역할 실패, cap/source 보존이 확인된다.
- [x] mock 예제를 실행해 assertions가 통과하고 all-target check와 package list 결과가 기록된다.
- [x] production 요청으로 test-only endpoint/evaluator를 선택할 수 없고 key·사적 source가 포함되지 않는다.
- [x] 향후 모델 평가의 지표·라벨·분모·버전·실패 분석을 정의했으며 실제로 하지 않은 품질/성능/calibration은 미실행으로 표시한다.

## 참고 자료
[모델·언어·요금](https://docs.typesafe.ai/models.md), [confidence](https://docs.typesafe.ai/confidence.md), [본문 필터 예제](https://docs.typesafe.ai/cookbooks/classifying_rag_passages.md), 상위 브리프의 스킬·과거 보고서. 예제 수치와 공개 시연은 이 제품의 acceptance threshold가 아니다.
