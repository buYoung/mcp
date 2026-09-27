# [feat] Jev 단계를 네이티브 MCP·설정에 통합

## 작업 유형
feat

## 목적과 기준 상태
- [상위 브리프셋](2026-09-21-briefset-jev-native.md)의 시작 점검과 SC-01–SC-06을 따른다. 01–03의 API를 실제 최종 소비자까지 연결하는 단계이다.
- 기존 MCP는 current-thread Tokio의 sequential loop에서 synchronous search/overview를 호출한다. `handle_request`가 config/redaction을 고정하고 응답 masking·cap·호출 기록을 처리한다.
- overview는 active workspace를 갱신하고 search는 pending source observations를 반환한다. search는 미지원 인자를 거부하며 overview의 `query`는 이미 path 별칭이다.
- 설정은 repository/global/default 우선순위, lenient parsing, additive sync, 다국어 template/comment 경계를 가진다. CLI path-only search는 별도 경로이며 Jev 활성화 대상으로 바꾸지 않는다.

## 범위와 소유권
- 포함: canonical 설정·검증·마이그레이션·템플릿·문서, task_query schema, host client lifecycle, async dispatch, 진단/최종 계측, 실제 MCP 통합 회귀.
- 제외: 새 MCP 도구·서버·프로세스, CLI Jev 옵션, read/grep/find 필터, 일반 concurrent dispatch, 새 cancellation-notification 처리, 공개 provider endpoint override.
- 소유: `apps/codemap-search/src/mcp/`, `src/tools/mod.rs`, `src/tools/instructions/`, `src/tools/search/arguments.rs`, 설정 관련 파일과 사용자 문서, 통합 test helper.
- 02/03의 adapter/renderer 변경이 필요하면 해당 소유자에게 수정과 회귀를 돌린다. 이 단계에 대체 필터나 두 번째 순위 구현을 만들지 않는다.
- 입력 인계: `apps/codemap-search/validation/jev-native/{01-runtime,02-overview,03-search-filter}.md`.
- 출력 인계: `apps/codemap-search/validation/jev-native/04-integration.md`.

## 구현 계약

### I-C1. canonical 설정
초기 설정 그룹은 `[analysis.jev]`이다. 이름·기본값·범위·최종 소비자를 하나의 표로 유지하고 parser/default/template/docs/test를 함께 변경한다.

| 키 | 기본값 | 유효값/소비자 |
| --- | --- | --- |
| `overview_enabled` | `false` | boolean; root overview host 활성화 |
| `search_filter_enabled` | `false` | boolean; search filter host 활성화 |
| `model` | `"jev-1.13.0"` | 초기에는 pinned ID만 지원; alias/provider 확장 금지 |
| `api_key_env` | `"TYPESAFE_API_KEY"` | 비어 있지 않은 유효 환경 변수 이름; 비밀 값 자체를 저장하지 않음 |
| `timeout_ms` | `45000` | 양의 정수와 안전한 Duration/Instant 변환 범위; 어댑터 하나의 deadline과 평가기의 제한 |
| `max_in_flight_requests` | `3` | 1–3; 공유 평가기의 semaphore |
| `request_spacing_ms` | `300` | 300 이상이며 안전한 시간 범위; 공유 dispatch 시작 간격 |
| `max_batch_bytes` | `80000` | 1–80000; 실제 encoded request 상한 |
| `pool_idle_timeout_ms` | `30000` | 양의 정수와 안전한 시간 범위; 실제 HTTPS client pool |
| `search_filter_min_unrelated_probability` | `0.70` | 유한 `0.5 < value <= 1.0`; 최종 Rust retention policy |

- 01이 고정한 추가 내부 질문/메모리/응답 상한을 임의 public config 키로 늘리지 않는다. 전송 범위와 초기 안전 정책을 확장해야 한다면 상위로 돌린다.
- 잘못된 타입·NaN/Infinity·범위·시간 overflow는 기존 per-key 경고/fallback 방식으로 처리하고 서버를 종료하지 않는다. 잘못된 threshold를 임의로 0.5001로 clamp하지 않는다.
- 유효한 하위 설정과 built-in default의 선택은 기존 precedence를 따른다. invalid 설정의 실제 fallback 출처/effective 값을 검증하고 문서화한다.
- `CONFIG_VERSION`, parser normalization, additive sync/layout, `config_locale`, 양언어 template/comment와 README/configuration 문서를 함께 확인한다. 기존 key·사용자 주석을 지우거나 전체 config를 덮어쓰지 않는다.

### I-C2. 요청 인자와 활성화 행렬
- overview/search에 선택적 `task_query: string`을 추가한다. non-string은 정상 `-32602` 검증 오류로 처리하며 index lifecycle·client 생성보다 먼저 검증한다.
- 누락 또는 trim 검사상 공백뿐인 문자열은 우회한다. 실제 유효 의도는 caller가 보낸 문자열을 보존하고 한국어를 번역·요약하거나 search.query로 대체하지 않는다. 외부 전송 전 masking은 기존 정책대로 적용한다.
- search.query 필수 조건, caller_context 우선순위, language/extension hints, event_key/include_events, workspace_scope는 유지한다. overview의 path/file_path/file/query 충돌 우선순위도 유지한다.

| 상태/요청 | #1 | #2 | 결과 |
| --- | --- | --- | --- |
| 두 flag off | 호출 안 함 | 호출 안 함 | 기존 결과, key 불필요 |
| overview만 on, 유효 의도·key·ready root | 실행 | 호출 안 함 | base + 들어가는 추천 |
| search만 on, 유효 의도·key·검증된 detail | 호출 안 함 | 실행 | 같은 선택 집합의 보수적 필터 |
| 두 flag on | overview 요청에서만 실행 | search 요청에서만 실행 | 한 도구가 다른 모드를 암묵적으로 실행하지 않음 |
| key/의도 없음 | 호출 안 함 | 호출 안 함 | base; 미활성 조건은 비민감 진단 |
| folder/file overview, index 미준비/불완전 | 호출 안 함 | 해당 없음 | 기존 overview/안내 |
| event-only/unclassified/no-body/전부 보호 search | 해당 없음 | 호출 안 함 | 기존 검색 결과 |
| initial_instructions, read/find/grep, CLI, 다른 MCP 메서드 | 호출 안 함 | 호출 안 함 | 기존 경로; 내부 overview 호출도 Jev 실행 금지 |
| malformed task_query | 호출 안 함 | 호출 안 함 | 기존 형식의 인자 오류 |

진단 때문에 기존 keyless/intentless 결과에 추가 텍스트가 강제되지 않는다. unknown/unsupported 기존 인자의 검증 의미를 느슨하게 바꾸지 않는다.

### I-C3. 도구 설명과 외부 접근 고지
- `readOnlyHint=true`를 유지한다. overview/search가 외부 provider를 호출할 수 있는 경우 `openWorldHint=false`라고 광고해서는 안 된다. 다른 local tool은 기존 annotation을 유지한다.
- `tools/list`와 initial instructions는 해당 요청에 고정된 effective 설정을 사용한다. 외부 접근 가능성은 mode flag로 판단하며, 그 순간 key가 없다는 이유로 enabled 도구를 local-only라고 광고하지 않는다.
- 활성화 flag도 기존 config reload 경계를 따른다. 현재 요청의 설정은 고정하고 다음 요청은 새 값을 사용한다. 이 기능 때문에 별도의 세션 허용 flag나 강제 재시작 gate를 추가하지 않는다.
- 도구 metadata를 캐시하는 클라이언트는 활성화 변경 후 목록 재조회 또는 서버 재연결이 필요할 수 있음을 운영 문서에 설명한다. 실제 `tools/list` 재조회 결과와 같은 설정의 호출 동작을 검증하고, 모든 클라이언트가 자동 갱신한다고 주장하지 않는다. 새 list-changed/cancellation protocol은 추가하지 않는다.
- tool description/initial instructions에는 effective 활성화, 명시적 task_query, 전송 데이터 범위, source가 아니라 indexed hint일 수 있다는 점과 기존 read/grep 대안을 설명한다. 시작 시 off인 정상 설치에 key나 네트워크가 필수인 것처럼 안내하지 않는다.

### I-C4. host lifecycle와 요청 경계
1. 현재 request config/redaction을 고정하고 인자를 검증한다.
2. 기존 scope/readiness/base 경로를 보존하며 이번 도구의 활성화·의도·근거 가능성을 판정한다. 불필요한 평가기/환경 key 조회를 피한다.
3. host가 필요한 경우에만 지정 환경 변수의 key를 해결하고 공유 평가기를 lazy 생성한다. 일반 Rust 평가기는 환경/전역 config를 읽지 않는다.
4. 선택한 어댑터의 준비 시작 시 한 번 `deadline_at`을 정한다. host 제한과 caller 제한 중 더 이른 시각을 사용하며 두 단계/모든 batch에 그대로 전달한다. 준비에서 예산을 소모했으면 HTTP를 시작하지 않고 base로 돌아간다. 기존 기본 결과 생성 자체를 새 timeout 오류로 바꾸지 않는다.
5. owned snapshot/source/policy를 유지한 채 async 평가를 await한다. 네트워크 중 index/config lock을 잡거나 current-thread에서 blocking HTTP를 실행하지 않는다.
6. 02/03의 결과를 정책대로 렌더링하고 최종 masking/cap을 적용한다. source observations를 실제 전달 결과와 맞춘 뒤 호출 기록을 남긴다.
7. 기존 active workspace 갱신·JSON-RPC envelope·notification no-response·request 순서를 유지한다. 후속 요청은 현재 요청 뒤에서 처리한다.

- transport 관련 설정이나 key 값이 바뀌면 다음 적격 요청에서 client를 재구성한다. threshold/flag만 바뀌었다고 매번 pool을 버리지 않는다. key 비교·fingerprint를 진단에 노출하지 않는다.
- 평가 중 config/key/index/file 변경이 발생해도 현재 요청은 캡처한 설정·근거를 사용한다. 다음 요청만 새 유효값을 받는다. key 회전 확인은 host 책임이다.
- 현재 MCP 계약에 cancellation input이 없으면 host는 그 사실을 기록한다. 재사용 API의 caller cancellation 지원을 MCP 취소 알림 지원으로 광고하지 않는다.
- thread-local config/redaction guard를 await 너머로 유지할 경우 현재-thread·sequential 조건을 실제 코드에서 확인한다. runtime worker가 전역 설정/masking을 다시 읽게 하지 말고 필요한 값은 owned input으로 전달한다.
- 기존 `McpServer::new` 호출자를 유지하거나 호환 생성자를 제공한다. test-only evaluator 주입은 정상 MCP 인자/환경 endpoint override로 선택할 수 없어야 한다.

### I-C5. 응답·진단·source accounting
| 영역 | 계약 |
| --- | --- |
| MCP stdout | 기존 JSON-RPC content/error 구조만. raw judgment·metrics를 새 필수 response field로 추가하지 않음 |
| overview text | base 우선, 남은 공간에만 추천/안내; 역할 unavailable·no-match를 구분 |
| search text | 기존 선택 집합 유지; body replacement는 확보한 공간 안에만; all-keep/bypass/fallback은 base |
| canonical 진단 | 비민감 bounded stderr/tracing summary. inline 진단을 넣을 공간이 없어도 관측 가능 |
| source observations | 최종 전달 소스가 있는 파일만; 생략 marker/metadata-only 파일은 read로 세지 않음; 기존 필드 단위와 계산 위치 보존 |
| 전체 응답 bytes | 최종 masking/cap/envelope text에서 계산; Jev 진단이 cap 검사를 우회하지 않음 |

진단은 최소한 tool, mode, applied/bypassed/fallback, reason, 모델, evidence/question/policy version, effective threshold, coverage/판단/보호/실제 생략·표시 개수, 알려진 token 합계/미확인 요청 수, elapsed/HTTP 시간/attempt 수를 조사할 수 있어야 한다. raw task/source/key/provider 오류 body는 넣지 않는다. 비활성 호출마다 불필요한 Jev 로그를 생성하지 않는다.

- 추천에는 recommendation_status와 role-stage 상태를 별도로 유지한다. 파일 점수 실패는 fallback, 역할만 실패하면 파일 추천 적용 + 역할 unavailable이다.
- 최종 응답이 오류가 되어 source가 전달되지 않았다면 이 기능의 pending observations를 성공한 읽기로 기록하지 않는다. mask/cap 처리 이후 관측 갱신 책임을 03과 하나의 계약으로 고정한다.
- 기존 cap에 들어가는 base가 Jev 상태 텍스트 때문에 `-32602`로 바뀌면 실패이다. 문구 공간 예약으로 baseline을 줄이는 우회도 허용하지 않는다.

### I-C6. 사용자 문서와 예제
- `README.md`/`README.ko.md`, `docs/configuration.md`/`.ko.md`, 설정 template 양언어판, tool schema/설명을 함께 갱신한다.
- 두 flag 조합, 환경 변수 이름 설정, task_query 예제, config reload와 클라이언트 metadata 재조회/재연결 안내, 전송되는 데이터, masking off일 때의 기존 정책, 실패·진단 위치를 설명한다.
- 80,000 bytes와 두 token 제한의 차이, pinned 모델, 자동 retry 없음, 0.70의 실험적 성격을 명시한다.
- `jev_decisions -- --mock`은 실제 실행되는 offline 예제, `--live`는 operator의 별도 실행 지시가 필요한 실제 요청으로 구분한다. 정상 설치에 Python·proxy가 필요하다고 쓰지 않는다.

초기화 후의 schema 대조용 JSON-RPC 요청 예시. 각각 별도 frame이며 batch/concurrent dispatch를 뜻하지 않는다.

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"overview","arguments":{"task_query":"검색 응답의 바이트 제한이 적용되는 위치를 찾아줘"}}}
```

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search","arguments":{"query":"response byte cap","task_query":"검색 응답의 바이트 제한이 적용되는 위치를 찾아줘"}}}
```

위 호출은 예시이지 지금 실행하라는 지시가 아니다. 실제 테스트에서는 비민감 fixture와 injected evaluator를 사용한다.

## 실행 단계

### Stage 1 — 활성화·설정·진단 계약 고정
- 시작: 01–03 인계가 현재 소스와 맞고 계약/회귀가 통과했다.
- 작업: 설정 표, schema/validation, 외부 접근 annotation과 reload, lazy host lifecycle, 진단 소비자, source observation 필드 단위를 고정한다. defaults부터 최종 HTTP/retention/renderer까지 각 값의 전달 경로를 기록한다.
- 검증: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config` — defaults/precedence/invalid/additive sync/최종 threshold 소비를 확인한다.
- 종료: parser/template/schema/docs의 key·단위·범위 일치, normal offline 호출에 key/task 의도 불필요.
- 재계획: mandatory task argument, credential 저장, default-on, 새 protocol이 필요하면 해당 변경을 상위로 돌린다.

### Stage 2 — sequential async 통합
- 작업: 기존 request 경계를 async로 연결하고 immutable 설정·snapshot·deadline을 전달한다. adapter 결함은 02/03에 돌린다.
- 검증: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp` — 아래 I01–I09를 actual request path로 확인한다.
- 종료: 모든 활성화 조합, fallback·한도·최종 계측, 요청 중 reload와 다음 요청 차이가 실제 관측된다.
- 재계획: 동시 dispatch 또는 전역 guard 손실이 필요하면 중단하고 현재 protocol 안에서 해결한다.

### Stage 3 — 문서·호환성·인계
- 검증: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; sample calls를 실제 tools/list와 parser에 대조한다.
- 산출물: `04-integration.md`에 canonical 설정 표, 요청/활성화 표, 최종 소비자 경로, annotation/reload/client 결정과 확인한 클라이언트 범위, 진단 표, 실제 명령/결과와 한계를 기록한다.
- 무변경 경로: 기존 통합이 현재 계약을 만족하면 현재 증거로 인계한다. 과거 통과/완료 체크만 복사하지 않는다.

## 필수 검증 시나리오
| ID | 시나리오 | 확인할 결과 |
| --- | --- | --- |
| I01 | 00/10/01/11, disabled/keyless/빈 의도 | 올바른 adapter만 실행, base 보존, 의도·key 없을 때 HTTP 0회 |
| I02 | non-string task_query, query/path 별칭, workspace/event 옵션 | 기존 validation/error/scope 의미 보존, 오류 전 side effect 없음 |
| I03 | pinned 설정·threshold 0.70/0.90, invalid/NaN/overflow, config sync | effective 값이 final consumer에 적용, 주석/기존 key 보존 |
| I04 | Noul=0.80 지연 중 threshold 변경·다음 요청 | 현재 요청 0.70이면 생략, 0.90이면 유지; 중간에 정책이 섞이지 않음 |
| I05 | Score 완료 후 역할 timeout, multi-batch 일부 실패, 짧은 deadline | 한 deadline, 정확한 역할/전체 fallback, 알려진/unknown usage |
| I06 | cap에 찬 all-keep/fallback, 작은 overview 공간, masking | baseline 근거 손실·신규 cap 오류 없음, stderr 진단으로 이유 관측 |
| I07 | tool metadata 재조회, off→on/on→off reload, key/transport 회전 | 같은 설정의 광고와 실제 외부 접근 일치, lazy 재구성과 요청 경계; 클라이언트 cache 자동 갱신은 별도 확인 범위 |
| I08 | initial_instructions/read/find/grep/CLI/notification | Jev 호출 없음, stdout framing/순서/기존 호출자 호환 |
| I09 | body 생략 후 source_files/analyze 계측, 최종 오류, 비민감 로그 | 실제 전달 source만 관측, 진단 bytes/marker를 읽기로 세지 않음, key/source 누출 없음 |

통합의 fake evaluator/transport는 고정 답변만 반환하는 동시에 실제 전달된 값·payload·요청 수를 캡처해야 한다. 항상 bypass하는 구현이 테스트를 통과하지 않게 성공 경로에서 adapter/transport 호출과 실제 생략/추천을 assert한다.

## 완료 조건
- [x] I01–I09의 실제 populated 사례와 관련 기존 config/MCP 검사가 통과한다.
- [x] config·schema·문서부터 두 adapter·HTTP client·retention·최종 관측까지 값이 전달된다.
- [x] 외부 접근 annotation·per-request reload·클라이언트 metadata 갱신 한계가 문서와 코드에 일치한다.
- [x] 기본 설치는 key/Python/network 없이 동작하며 기존 CLI/도구 계약을 유지한다.
- [x] 05가 실제 integrated behavior를 검증할 수 있는 test boundary와 인계 증거가 있다.
