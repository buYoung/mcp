# Jev native — 04 MCP·설정 통합 인계

- 하위 브리프: `docs/briefs/2026-09-21-feat-jev-native-04-integration.md` (개정판)
- 입력 인계: `01-runtime.md`, `02-overview.md`, `03-search-filter.md`
- 작성일: 2026-09-23 / 소스 상태: `01-runtime.md` §0(digest `867b202d2fc89450`)
- 상태: 오프라인 완료. `e2e::config` 11개, `e2e::mcp` 31개(jev_stages 5 포함), `e2e::jev` 11개 통과. provider 요청 없음.

기존 구현과 개정 계약의 차이 및 처리:

| 차이 | 처리 |
| --- | --- |
| 설정 범위: in-flight 상한 없음, spacing 0 허용, batch ≥1024, timeout 상한 없음 | 1–3, ≥300, 1–80000, 양수·≤7일(`is_safe_duration`) — 01과 동일 해석 |
| `openWorldHint: false` 고정 | overview/search는 각 mode flag를 따름(요청 시점 effective 설정) |
| bypass/fallback inline 안내 문구 | 제거. base 바이트 동일. `jev stage` stderr 진단이 canonical |
| 진단 필드: tool/outcome/status/tokens/시간/요청 수 | model, evidence/question/policy 버전, reason/detail(coverage·판단·보호·생략·표시 수, threshold, note_inline), reported/unreported 응답 수, attempts, queue 시간 추가 |
| deadline을 adapter의 Duration으로 전달 | host가 준비 시작 시 `deadline_at` 1회 계산해 두 adapter에 전달 |
| search bypass가 `FilterRequest::Bypass`로 필터 경로 진입 | bypass는 `run_with_metadata`(base)로 직행 |

## 1. 변경 파일 (04 소유)

| 경로 | 변경 |
| --- | --- |
| `src/config/jev.rs` | 범위 검증기(`as_safe_timeout_ms`, `as_in_flight_requests`, `as_request_spacing_ms`, `as_batch_bytes` 상한), 단위 테스트 4개 갱신 |
| `src/config.rs`, `src/config/layout.rs` | `CONFIG_VERSION = 24`, `[analysis.jev]` owned subtable, `AfterSubtable` 마이그레이션(v24 블록 문구 갱신) — 구조는 `feat/codemap-jev` 유지 |
| `src/config_template.toml`, `src/config_template.ko.toml` | 범위·마감·stderr 문구 갱신(버전 24 유지 — 문구 변경은 bump 대상 아님) |
| `src/mcp/jev.rs` | `JevHost`, `deadline_at`, `StageLog` 진단, base-only bypass/fallback |
| `src/mcp/mod.rs` | `bootstrap`, `with_evaluator`, `run_with_io`, async `handle_request`(요청 단위 config/redaction 고정) — `feat/codemap-jev` 유지 |
| `src/tools/mod.rs` | `task_query` 검증·설명, mode별 `openWorldHint`, 도구 설명·`initial_instructions` 안내 문구 |
| `src/tools/search/arguments.rs` | `task_query` 허용(유지) |
| `docs/configuration.md`, `docs/configuration.ko.md`, `README.md`, `README.ko.md` | 표·섹션 갱신 |
| `tests/e2e/helpers.rs`(in-process 서버, 유지), `tests/e2e/mcp.rs::jev_stages`(5), `tests/e2e/config.rs::jev_config`(2), `tests/e2e/exclusions.rs`(버전 24 헤더) | 회귀 |

## 2. canonical 설정 `[analysis.jev]`

| 키 | 기본값 | 검증(유효값) | 최종 소비자 |
| --- | --- | --- | --- |
| `overview_enabled` | `false` | bool | `handle_request_inner` overview arm → `mcp::jev::overview` |
| `search_filter_enabled` | `false` | bool | search arm → `mcp::jev::search` |
| `model` | `"jev-1.13.0"` | 비어 있지 않은 문자열(alias 확장 없음; 응답 model과 정확 비교) | `EvaluatorConfig.model` |
| `api_key_env` | `"TYPESAFE_API_KEY"` | `[A-Za-z0-9_]+` | `JevHost::resolve`의 `std::env::var`(요청 시점) |
| `timeout_ms` | `45000` | 양의 정수, ≤ 7일(`is_safe_duration`) | `deadline_at = now + timeout`(host, 준비 시작 시), `EvaluatorConfig.deadline` |
| `max_in_flight_requests` | `3` | 1–3 | `EvaluatorConfig.max_in_flight_requests`(semaphore), `HttpsSettings.max_idle_connections` |
| `request_spacing_ms` | `300` | ≥300, ≤7일 | `EvaluatorConfig.request_spacing` |
| `max_batch_bytes` | `80000` | 정수 또는 크기 문자열, 1–80000 | `EvaluatorConfig.max_batch_bytes` |
| `pool_idle_timeout_ms` | `30000` | 양의 정수, ≤7일 | `HttpsSettings.pool_idle_timeout`(실제 client pool; 01 로컬 소켓 검증) |
| `search_filter_min_unrelated_probability` | `0.70` | 유한 `0.5 < v <= 1.0` | `FilterPolicy.min_unrelated_probability` → `apply_policy` |

잘못된 타입·NaN/Infinity·범위·시간 overflow는 per-key 경고 후 하위 계층 → built-in default 순으로 대체한다(clamp 없음; `config::jev::tests::invalid_values_warn_and_fall_back_to_the_lower_layer`, e2e `test_jev_threshold_overrides_and_invalid_values_reach_the_retention_policy`). 01 내부 상한(질문 8,192·batch 128·응답 4 MiB)은 공개 키로 노출하지 않는다. `CONFIG_VERSION` 24, additive sync는 `[analysis]` 뒤에 주석 블록으로 추가하며 기존 키·주석을 보존한다(`test_jev_template_and_migration_add_a_commented_section_after_analysis`). 양언어 template/마이그레이션 블록/docs 예시 config는 같은 문구다.

## 3. 요청 인자와 활성화 행렬 (실제 관측)

`task_query`: 선택적 문자열. 누락/`null`/공백 → `None`(우회), non-string → `-32602 "Invalid task_query: …"`(index lifecycle·client 생성 전, 활성화 여부 무관). 유효 의도는 trim만 하고 보존하며 masking은 adapter 캡처 시 적용한다. `search.query` 필수, `caller_context`/hints/`event_key`/`include_events`/`workspace_scope`, overview path 별칭 우선순위는 그대로다.

| 상태/요청 | #1 | #2 | 관측 (테스트) |
| --- | --- | --- | --- |
| 두 flag off | ✗ | ✗ | 바이트 동일, evaluator 미호출, 안내·annotation 없음 (`e2e::jev::test_jev_disabled_stages…`, `mcp::test_jev_task_query_is_validated_and_ignored_while_disabled`) |
| overview만 on + 의도·key·ready root | ✓ | ✗ | base + 섹션 (`mcp::test_jev_overview_recommendation_reports_matched_and_no_match_separately`) |
| search만 on + 의도·key·검증된 body | ✗ | ✓ | 같은 선택 집합의 보수적 생략 (`mcp::test_jev_search_filter_uses_the_captured_threshold…`) |
| 두 flag on | overview에서만 | search에서만 | (`mcp::test_jev_stages_are_independent…`, `jev::test_jev_korean_intent…`) |
| key 없음 / 의도 없음 | ✗ | ✗ | base 바이트 동일, stderr `bypassed missing_credentials|missing_task_query` (`mcp::test_jev_missing_credentials_bypass_without_any_request`, `jev::test_jev_stage_logs…`) |
| folder/file overview, warming/dead/empty | ✗ | — | 기존 출력, HTTP 0회(폴더는 로그도 없음) |
| event-only/unclassified/전부 보호 search | — | ✗ | 기존 출력(`Prepared::Done`, `bypassed:no_complete_bodies`) |
| initial_instructions/read/find/grep/CLI/notification | ✗ | ✗ | Jev 호출·로그 없음 (`jev::test_jev_stage_logs…`의 read 호출, 기존 e2e suite) |
| malformed task_query | ✗ | ✗ | `-32602` (`mcp::test_jev_task_query_is_validated…`, 서브프로세스) |

## 4. 도구 설명·외부 접근 고지

- `readOnlyHint: true` 유지. `overview`는 `openWorldHint = overview_enabled`, `search`는 `openWorldHint = search_filter_enabled`(요청 시점 effective 설정; key 유무와 무관). 다른 도구는 `false` 유지. 검증: `mcp::test_jev_search_filter_uses…`(search true/overview false), `test_jev_task_query_is_validated…`·`jev::test_jev_disabled_stages…`(둘 다 false).
- 도구 설명은 활성화된 단계에 한해 "may send a masked copy of the intent and of the … to the external TypeSafe API … Without task_query, or when the stage bypasses or fails, the base output is returned unchanged; the reason is logged on stderr." 문장을 덧붙인다. `initial_instructions`도 같은 조건으로 `jev_guidance`를 붙인다.
- reload: 활성화 flag·threshold는 기존 config reload 경계를 따르며 현재 요청은 `pin_request()`로 고정된다(`mcp::test_jev_search_filter…`: Gate로 in-flight 중 0.70→0.90 reload, 현재 요청 0.70 적용·다음 요청 0.90). 별도 세션 flag·재시작 gate 없음.
- 클라이언트 metadata 캐시: 문서(README·configuration 양언어)에 "단계를 켜거나 끈 뒤 목록 재조회 또는 재연결이 필요할 수 있으며 모든 클라이언트가 자동 갱신하지는 않는다"고 명시했다. 실제 검증 범위는 같은 서버에서의 `tools/list` 재조회와 같은 설정의 호출 동작이다. 클라이언트별 자동 갱신은 검증하지 않았다. list-changed/cancellation protocol은 추가하지 않았다.

## 5. host lifecycle와 요청 경계

1. `handle_request`가 `config::pin_request()`·`redact::begin_request()`를 요청 전체(await 포함)에 고정한다. current-thread runtime + sequential loop이므로 thread-local guard는 요청 사이에 교차하지 않는다(`run_with_io`는 한 번에 한 요청).
2. 인자 검증(`task_query` 포함) → 기존 scope/readiness/base 경로 → 이번 도구의 활성화·의도·근거 가능성 판정. 비활성 호출은 evaluator·환경 key를 조회하지 않는다.
3. key는 `JevHost::resolve`에서만 `api_key_env`로 해결하고 `SecretString`으로 evaluator 안에만 둔다. transport 설정(`model`, `api_key_env`, `timeout_ms`, `max_in_flight_requests`, `request_spacing_ms`, `max_batch_bytes`, `pool_idle_timeout_ms`)이나 key 값이 바뀌면 다음 적격 요청에서 재구성한다. threshold/flag 변경은 재구성하지 않는다. fingerprint·key는 진단에 없다.
4. `deadline_at = Instant::now() + timeout_ms`를 adapter 준비 시작 전에 1회 계산해 두 adapter(`RecommendationPolicy`/`FilterPolicy`)에 전달한다. 평가기는 `min(deadline_at, now + config.deadline)`을 적용하므로 늘어나지 않는다. 준비에서 예산이 소진되면 runtime이 HTTP 0회로 `deadline_exceeded` fallback을 돌려주고 base가 반환된다(e2e 실행 중 관측: 디버그 빌드에서 `timeout_ms = 100`이면 준비만으로 소진되어 attempts 0; 테스트는 1,500ms로 dispatch를 확인). 기존 기본 결과 생성은 timeout 오류로 바뀌지 않는다.
5. owned snapshot(`RootInput`)/렌더된 FileOutput/정책을 유지한 채 async 평가를 await한다. 네트워크 중 index/config lock을 잡지 않고 blocking HTTP가 없다(reqwest async).
6. 렌더링 → `redact::response` → `enforce_response_cap`(overview cap) → `source_files` → `call_recorder.record`(응답 bytes는 최종 envelope text에서 계측). search는 자체 cap을 적용하므로 `enforce_response_cap` 대상이 아니다.
7. active workspace 갱신·JSON-RPC envelope·notification 무응답·요청 순서 유지. 취소 알림 protocol은 없으며 host는 `cancel: None`을 전달한다(재사용 API의 취소 지원을 MCP 취소로 광고하지 않음).
8. `McpServer::new` 유지, `bootstrap(cwd)`·`with_evaluator(Arc<dyn Evaluator>)`는 Rust 코드에서만 선택 가능하다(설정 키·인자·환경 endpoint override 없음; 01의 test endpoint는 `#[cfg(test)]`).

## 6. 응답·진단·source accounting

| 영역 | 구현 |
| --- | --- |
| MCP stdout | 기존 content/error 구조만. raw judgment·metrics 필드 없음 |
| overview text | base 우선. 평가된 결과(`matched`/`no_match`/`insufficient_evidence`)만 `"\n\n" + 섹션`을 덧붙이며 남은 예산(`overview_output_byte_cap − base − 2`)에 맞을 때만. bypass/fallback은 base 바이트 동일 |
| search text | 기존 선택 집합. 생략 표시는 body 자리, 상태 줄은 확보 공간 내에서만. bypass/fallback/all-keep은 base 바이트 동일 |
| canonical 진단 | `tracing::info!(target: "codemap_search::mcp::jev", "jev stage")` 1줄/단계 실행: `tool`, `outcome`(applied/bypassed/fallback), `status`(overview: `matched|no_match|insufficient_evidence|bypassed:<r>|fallback:<k>`; search: `applied|bypassed:<r>|fallback:<k>`), `detail`(overview: role_stage·snapshot_files·eligible_files·declarations·fragments·path_only·questions·judged·qualified·ranked·rendered·role_candidates_omitted; search: bodies·judged·omitted·rendered_omissions·protected·linked·unverified·masked_unavailable·threshold·note_inline), `model`, `versions`, `input_tokens`/`output_tokens`(Option), `reported_responses`/`unreported_responses`, `attempts`, `elapsed_ms`/`http_ms`/`queue_ms`. 원문 task/source/key/provider 오류 본문 없음. 비활성 호출·폴더 overview·read/find/grep은 로그 없음 |
| source observations | `SearchOutput.source_files`(03 §6 단위). 오류 응답이면 search arm이 output을 만들지 않으므로 pending 관측이 비어 있다(`pending_source_files`는 요청 시작에 초기화). `CallRecorder::record`는 `is_error`와 함께 기록한다 |
| 전체 응답 bytes | `handle_request`가 최종 마스킹·cap 후 envelope text 길이 합으로 계측 |

기존 cap에 들어가는 base가 Jev 문구 때문에 `-32602`가 되는 경로는 없다(문구를 추가하지 않거나 예산 검사 후에만 추가; `jev::test_jev_output_caps_bound_stage_output`).

## 7. 문서·예제

- README(en/ko) "Optional Jev decision stages"/"선택적 Jev 판단 단계", `docs/configuration.md`/`.ko.md`의 섹션 표·키 참조·예시 config·전용 절, 양언어 template, `config.rs` v24 마이그레이션 블록, tool description/`initial_instructions`가 동일한 기본값·범위·전송 데이터·진단 위치·미검증 품질을 설명한다.
- 문서 예시 호출 2건(`overview {task_query}`, `search {query, task_query}`)은 `tools/list` schema(`task_query: string`)와 `arguments::validate`에 대조했다(e2e `test_jev_task_query_is_validated…`가 schema를 확인). `--mock`은 오프라인 실행 예제, `--live`는 운영자 지시가 필요한 실제 요청으로 구분했다.

## 8. 실행한 명령과 결과 (cwd `apps/codemap-search`)

| 명령 | 결과 | 로그 |
| --- | --- | --- |
| `cargo test --test e2e_tests e2e::config` | exit 0 — 11 passed | `logs/e2e__config.log` |
| `cargo test --test e2e_tests e2e::mcp` | exit 0 — 31 passed | `logs/e2e__mcp.log` |
| `cargo test --test e2e_tests jev_` | exit 0 — 18 passed(e2e::jev 11 + mcp::jev_stages 5 + config::jev_config 2) | `logs/e2e-all.log`에 포함 |
| `cargo check --all-targets` | exit 0 | `logs/check-all-targets.log` |

I01–I09 대응:

| ID | 테스트 |
| --- | --- |
| I01 | `jev::test_jev_disabled_stages…`, `mcp::test_jev_stages_are_independent_and_failures_preserve_the_base_output`, `mcp::test_jev_missing_credentials_bypass_without_any_request`, `jev::test_jev_stage_logs…`(missing_task_query) |
| I02 | `mcp::test_jev_task_query_is_validated_and_ignored_while_disabled`(non-string → -32602, 서브프로세스), 기존 `e2e::mcp`·`e2e::search` suite(별칭·scope·event 옵션) |
| I03 | `config::jev::tests::*`, `config::jev_config::test_jev_template_and_migration…`, `test_jev_threshold_overrides_and_invalid_values_reach_the_retention_policy`(0.90 유지·invalid→0.70 생략) |
| I04 | `mcp::test_jev_search_filter_uses_the_captured_threshold_for_an_in_flight_call`(Gate로 지연 중 reload; 0.70 적용, 다음 요청 0.90 all-keep) |
| I05 | `jev::test_jev_configured_deadline_wins_over_the_runtime_default_through_mcp`(1.5초 deadline, attempts 1, usage unknown), `jev::test_jev_context_and_batch_limits_fail_explicitly_without_sending`(attempts 0), 02 단위 `role_stage_failure…` |
| I06 | `jev::test_jev_output_caps_bound_stage_output`(search cap 1,600 축소만; overview base+40 사전 bypass HTTP 0회·항목 단위 절단·정확한 경계), stderr `bypassed:insufficient_output_room` 관측 |
| I07 | `mcp::test_jev_search_filter…`(tools/list annotation/설명이 활성 flag와 일치), `jev::test_jev_disabled_stages…`(비활성 annotation), in-flight reload; key/transport 회전은 `JevHost::resolve` fingerprint 코드 검사(e2e 미실행) |
| I08 | `jev::test_jev_stage_logs…`(read 호출에 로그 없음), `jev::test_jev_disabled_stages…`(initial_instructions), 기존 e2e suite 226개 무변경 통과 |
| I09 | `jev::test_jev_stale_files…`, `jev::test_jev_omitted_bodies_restore_through_read…`, `jev::test_jev_stage_logs_report_usage_and_outcome_without_evidence`(로그에 source·intent 없음, usage·개수·버전·모델 존재) |

## 9. 미실행·한계

- 실제 key/transport 회전(lazy 재구성)은 코드 검사만 했다(e2e는 injected evaluator 사용).
- 클라이언트별 tool metadata 자동 갱신은 검증 범위 밖이다.
- 취소 알림은 프로토콜에 없어 host에서 지원하지 않는다.
- live provider 호출·품질·속도는 미실행이다.
