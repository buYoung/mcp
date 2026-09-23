# 05 — 네이티브 Jev 기능 전체 검증

> 이 문서는 #2가 search에만 적용되던 최초 완료 시점의 기록입니다. 후속 read/grep 확장은 [06-live-tools.md](06-live-tools.md), overview 입력 변경과 활성 경로의 최종 회귀 결과는 [07-overview-live-regressions.md](07-overview-live-regressions.md)를 참고하세요.

## 1. 검증 대상

| 항목 | 값 |
| --- | --- |
| cwd / 저장소 루트 | `/Users/buyong/workspace/private/buyong-mcp` (Cargo 명령은 `apps/codemap-search`에서 실행) |
| 브랜치 / HEAD | `docs/jev-integrate`, `530e476c3` |
| 미커밋 변경 | 브리프 6개(개정판) + `apps/codemap-search` 수정 28개·신규 17개 파일(스테이징 없음). 소스 digest(수정·신규 소스 파일 sha256 목록의 sha256 앞 16자, `validation/` 제외): `867b202d2fc89450` |
| 신규 파일 | `examples/jev_decisions.{rs,mock.json}`, `src/config/jev.rs`, `src/jev/{mod,question,answer,batch,evaluator,transport,mock,tests}.rs`, `src/mcp/jev.rs`, `src/tools/overview/jev.rs` + `jev/tests.rs`, `src/tools/search/jev.rs` + `jev/tests.rs`, `tests/e2e/jev.rs` |
| 수정 파일 | `Cargo.toml`, `Cargo.lock`, `README.md`, `README.ko.md`, `docs/configuration.md`, `docs/configuration.ko.md`, `src/codemap/mod.rs`, `src/config.rs`, `src/config/layout.rs`, `src/config_template.toml`, `src/config_template.ko.toml`, `src/lib.rs`, `src/main.rs`, `src/mcp/mod.rs`, `src/redact.rs`, `src/redact/transform.rs`, `src/tools/mod.rs`, `src/tools/overview.rs`, `src/tools/search/{arguments,grouped,mod,monorepo,render}.rs`, `tests/e2e/{config,exclusions,helpers,mcp,mod}.rs` |
| 도구 체인 / 크레이트 | rustc 1.98.1, cargo 1.98.1, macOS darwin 24.6.0 / `codemap-search` 0.10.0, lockfile 의존성은 `feat/codemap-jev`와 동일(reqwest 0.13.5, rustls 0.23.45, ring 0.17.14, hyper 1.11.1; openssl·aws-lc 없음) |
| 설정 | 기본값(두 flag off, `jev-1.13.0`, 45,000ms, 3/300ms/80,000, 30,000ms, 0.70); 시나리오별 `.codemap/config.toml` 오버라이드 |
| evaluator 모드 | 단위·e2e: injected `jev::mock::MockEvaluator` 또는 real `JevEvaluator` + `MockTransport`; 01 transport 검증: real `HttpsTransport`(`#[cfg(test)]` 로컬 HTTP listener). 외부 provider traffic 0회, 어떤 환경 변수 key도 설정하지 않음(`missing_credentials` 사례는 미설정 변수 이름 `CODEMAP_TEST_JEV_KEY_UNSET`) |
| 버전 | projection `overview-recommendation-projection/2`, questions `overview-recommendation-questions/2`, policy `overview-recommendation-policy/2-experimental`; evidence `search-filter-evidence/2`, questions `search-filter-questions/2`, policy `search-filter-policy/2-experimental`; threshold 0.70(잠정) |

기존 구현 후보(`feat/codemap-jev`)의 과거 보고서·체크 표시·live 연결 기록은 재사용하지 않았다. 아래 결과는 모두 이 트리에서 새로 실행한 것이다.

## 2. 요구사항 행렬

단위 suite: `src/jev/tests.rs` 42(오프라인 38 + 로컬 소켓 4), `src/tools/overview/jev/tests.rs` 22, `src/tools/search/jev/tests.rs` 25, `src/config/jev.rs` 4. MCP 경계: `tests/e2e/jev.rs` 11, `tests/e2e/mcp.rs::jev_stages` 5, `tests/e2e/config.rs::jev_config` 2. 모든 e2e 시나리오는 실제 파이프라인(`McpServer::bootstrap` → `run_with_io` → tools → adapters → evaluator)을 in-process로 구동하고 전달된 텍스트·evaluator가 받은 요청·stderr `jev stage` 줄로 판정한다. 정책을 테스트에서 재구현하지 않는다. 빈 모집단 실행은 없다.

| 검증 영역 | 소유 계약 | 실제 확인 (테스트 · fixture) | 결과 |
| --- | --- | --- | --- |
| 기본 호환성 | I01/I02/I08, F01 | `e2e::jev::test_jev_disabled_stages_ignore_intent_and_an_injected_evaluator`(off/off 바이트 동일, evaluator 미호출, annotation false), `mcp::jev_stages::test_jev_task_query_is_validated_and_ignored_while_disabled`(서브프로세스, `-32602`), `test_jev_stages_are_independent…`(10/01/11), 기존 e2e 208개 무변경 통과 · `src/budget.ts`(keepMe L1–8, dropMe L10–15) | 통과 |
| 실제 입력 조립 | R01, O02, F07 | 01 `https_client_reuses_one_idle_connection…`(real evaluator + 로컬 소켓의 encoded body: model/state/questions/헤더/sha256), 02 `all_unrelated_evaluation…`(질문 문구·backtick 참조·4등급), 03 `evaluate_sends_one_noul_question…`(state/candidate 필드), `capture_masks_secret_like…`(양 adapter), e2e `test_jev_korean_intent_reaches_both_stages_verbatim` | 통과 |
| root 전체 coverage | O01/O02/O04 | `e2e::jev::test_jev_overview_qualifies_before_the_cap_and_skips_roles_without_a_match`(30 파일: q00–q25 자격, a0–a3 동률 → "26 qualified, showing 24", Score 질문 30·Choice 24, 두 요청 `deadline_at` 동일), 02 `fragments_split_large_files…`(200 선언 전부), `oversized_documentation_continues…`, `files_without_usable_evidence…`(path-only) | 통과 |
| 추천 결과 | O03/O05 | 위 e2e의 no-match·tied 변형(요청 1회, Choice 0회), `mcp::jev_stages::test_jev_overview_recommendation_reports_matched_and_no_match_separately`(`src/a.ts` applyBudgetCap, `src/b.ts` greet; 폴더 overview 무변경·HTTP 0회), 02 `roles_attach_at_most_two…`, `a_negative_option_tying…`, `role_stage_failure_keeps_the_complete_file_ranking` | 통과 |
| 재사용 | O08, F05 | 02 `stored_judgments_replay_a_changed_policy_without_inference`(재순위, 역할 재계산, NotEvaluated, 부분 judgments 거부), 03 `replaying_raw_judgments_with_another_threshold…`(0.80 → 0.70 생략/0.90 유지, evaluator 추가 호출 0) | 통과 |
| body 완전성·정체성 | F02/F03/F09 | 03 `capture_builds_entities…`, `identity_is_verified_on_the_displayed_buffer_not_on_line_counts`, `masking_beyond_the_signature…`, `oversized_complete_bodies…`; `e2e::jev::test_jev_search_protects_partial_windows_and_omits_only_complete_bodies`(`src/window.ts`, snippet 3줄: keepMe 창 보호, wideDrop 생략, 질문 1개), `test_jev_stale_files_are_protected_and_metadata_only_files_are_not_read_observations`(색인 후 재작성·같은 행 수 → unverified 1, judged 1) | 통과 |
| 보호 closure | F04/F05 | 03 `policy_keeps_bodies_linked_to_nested_in_or_containing_retained_blocks`, `rust_containers_constants_and_unknown_kinds…`, `forced_unrelated_answers_still_keep_protected_bodies`(1.00); `e2e::jev::test_jev_search_keeps_linked_rust_methods_and_never_judges_data_declarations`(`src/checkout.rs`: const/struct 2/impl 메서드 2/free fn 1 → bodies 4, judged 3, omitted 1(banner), protected 1, linked 1(submit↔total)) | 통과 |
| threshold 최종 전달 | I03/I04 | `config::jev_config::test_jev_threshold_overrides_and_invalid_values_reach_the_retention_policy`(0.90 → plain 바이트 동일, 0.5/"seventy" → 0.70 생략 2건), `mcp::jev_stages::test_jev_search_filter_uses_the_captured_threshold_for_an_in_flight_call`(in-flight reload) | 통과 |
| deadline·실패 | R03–R07, O06, F06, I05 | 01 deadline/취소/batch 실패 suite, 02 `one_absolute_deadline_bounds_both_stages_together`, 03 `whole_call_failure…`/`incomplete_or_invalid_answers…`; `e2e::jev::test_jev_configured_deadline_wins_over_the_runtime_default_through_mcp`(1,500ms, 두 도구 attempts 1, usage unknown, base 동일), `test_jev_context_and_batch_limits_fail_explicitly_without_sending`(estimated_token_limit ×2, question_too_large; attempts 0) | 통과 |
| 출력 cap·fallback | O07, F01/F08, I06 | `e2e::jev::test_jev_output_caps_bound_stage_output`(search cap 1,600: ≤ plain 길이; overview base+40 사전 bypass HTTP 0회, full−1 항목 절단, full 정확 경계), 02 `output_budget_bypasses_before_sending…`, 03 `the_status_line_is_inline_only_when_omissions_freed_the_room` | 통과 |
| 최종 source accounting | F10, I09 | `e2e::jev::test_jev_omitted_bodies_restore_through_read_and_retained_lines_are_verbatim`(유지 8줄 원문 동일, 생략 4줄 부재, 행 유지, `read offset 10 limit 6` 복원), 03 `retention_rewrites…`(`delivered_source_bytes`가 note 제외) | 통과 |
| 마스킹·비민감 진단 | R07, I09 | 01 `debug_output_hides_evidence_and_credentials`, `e2e::jev::test_jev_stage_logs_report_usage_and_outcome_without_evidence`(applied/bypassed 줄의 tool·status·detail·model·versions·tokens·attempts·시간; source·intent 없음; read 호출 로그 없음), `e2e::redact` suite 무변경 통과 | 통과 |
| host lifecycle·설명 | I07 | `mcp::jev_stages::test_jev_search_filter…`(tools/list `openWorldHint` search true/overview false, 설명 문구), `test_jev_disabled_stages…`(전부 false), in-flight reload; key/transport 회전은 코드 검사 | 통과(회전은 미실행) |
| 독립 재사용·패키징 | R09, I-C6 | `cargo run --example jev_decisions -- --mock` 실제 실행·assertions, `--bogus` exit 2, key 없는 `--live` exit 1(전송 없음), `cargo package --list --allow-dirty` | 통과 |

실패 분류: 이번 실행에서 최종 실패는 없다. 개발 중 발견·수정한 결함(생산 코드): `is_safe_duration`이 `i64::MAX`ms를 통과시켜 시간 상한(7일)을 추가. 나머지는 테스트 기대값 조정(부동소수 등가 비교, mock failing 요청 수, 생략 표시보다 작은 fixture body, 확보 공간 계산, 문자열 분할, 100ms deadline이 디버그 빌드 준비 시간보다 짧음)이었다.

### fixture와 비교 규칙 적용

- fixture는 모두 inline 합성 소스(TypeScript/Rust)와 생성 config다. 사적 PoC source·벤치마크 답변·key는 없다.
- 긍정 사례: ready root 추천(`src/budget.ts` matched, q00–q23 24건), complete 비보호 본문 생략(dropMe, wideDrop, banner), evaluator 호출 횟수 assert. 전부 bypass/보호하는 구현은 통과할 수 없다.
- 부정/불확실: no-match·동률·근거 부재(path-only)·all-keep(0.90) 사례를 비어 있지 않은 입력에서 실행.
- baseline: 같은 서버·config·snapshot에서 Jev off(또는 의도 없음)로 얻은 출력과 바이트 비교. 유지 검증은 `(파일, 원래 행 번호, 내용)`으로 확인(`test_jev_omitted_bodies_restore…`가 `BUDGET_TS.lines()` 원문을 대조).
- 생략 marker는 `- _omitted body:` 로 식별하고 변하지 않은 fixture에서 `read`로 원본 body를 복원, 변경 사례에서는 live source를 표시함을 확인.

## 3. 실행 결과 (최종 트리, cwd `apps/codemap-search`, 2026-09-23, 네트워크 미사용)

| 명령 | exit | 결과 | 로그 |
| --- | --- | --- | --- |
| `cargo test --lib jev` | 0 | 93 passed; 0 failed; 274 filtered out | `validation/jev-native/logs/lib-jev.log` |
| `cargo test --lib tools::overview` | 0 | 22 passed | `logs/lib-tools__overview.log` |
| `cargo test --lib tools::search` | 0 | 25 passed | `logs/lib-tools__search.log` |
| `cargo test --lib` | 0 | 365 passed; 0 failed; 2 ignored(기존) | `logs/lib-all.log` |
| `cargo test --test e2e_tests` | 0 | 226 passed; 0 failed; 1 ignored(기존), 267.18s | `logs/e2e-all.log` |
| `cargo test --test e2e_tests e2e::jev` / `e2e::mcp` / `e2e::config` / `e2e::search` / `e2e::codemap` | 0 | 11 / 31 / 11 / 24 / 15 passed | `logs/e2e__*.log` |
| `cargo check --all-targets` | 0 | 경고 0 | `logs/check-all-targets.log` |
| `cargo run --example jev_decisions -- --mock` | 0 | Score 2.00, Choice keep(0.90), Noul 0.90, usage 296/20, 1 request(1169 bytes, sha256 기록), `checks: passed` | `logs/example-mock.log` |
| `cargo package --list --allow-dirty` | 0 | 441 entries; `src/jev/*` 8, `src/mcp/jev.rs`, `src/config/jev.rs`, overview/search `jev.rs`+tests, `examples/jev_decisions.*`, `tests/e2e/jev.rs` 포함; `experiments/`, `validation/`, `tests/fixtures/`, `docs/` 항목 0 | `logs/package-list.log` |

provider traffic 차단 근거: 모든 평가는 injected `MockEvaluator`/`MockTransport` 또는 `#[cfg(test)]` 로컬 listener를 사용한다. 생산 `HttpsTransport::new`는 `ENDPOINT` 상수만 사용하고 override가 없다. `missing_credentials` 사례는 환경 변수 미설정을 사용한다.

## 4. 실제 MCP 관찰(대표)

- 성공 추천: 30파일 catalog에서 "Evaluated all 30 indexed files in this snapshot; 26 qualified, showing 24." + `### 1. src/q00.ts · relevance 1.10/3` … `- q00 (fn) L1–1 · role: implementation`; Score 30·Choice 24 질문, 두 요청 동일 `deadline_at`.
- no-match / 동률: "none qualified. Indexed evidence did not establish a recommendation …" / "none qualified and some indexed evidence was tied or unavailable."; Choice 요청 없음.
- all-keep: threshold 0.90에서 Noul 0.80 → 출력이 plain search와 바이트 동일.
- fallback: 1,500ms deadline·6초 응답 → search/overview 모두 base 바이트 동일, stderr `outcome="fallback" status="fallback:deadline_exceeded" attempts=1 input_tokens=None unreported_responses=0`.
- 작은 cap: overview base+40 → base 바이트 동일·HTTP 0회·stderr `bypassed:insufficient_output_room attempts=0`; full−1 → 항목 절단 각주; full → 전체 섹션.
- reload: in-flight 0.70 유지, 다음 요청 0.90.
- source accounting: 생략 body 없이 유지 행이 원문과 동일, `read offset 10 limit 6`으로 복원; 색인 후 변경된 파일은 live source 표시·unverified 보호.

## 5. 예제·패키징·문서

- 예제는 컴파일이 아니라 실제 실행으로 세 기본형·usage·요청 수 assertions를 통과했다(§3).
- 패키지 목록은 공통 모듈·예제 fixture를 포함하고 기존 exclusions가 experiments/validation/fixtures/docs를 제외함을 확인했다. `--list`는 빌드·release target·발행 성공의 증거가 아니다.
- `.github/workflows/codemap-search-release.yml`: 7개 target(x86_64 gnu, x86_64 musl, aarch64 musl(cross), aarch64/x86_64 apple, x86_64/aarch64 windows msvc(best effort)). rustls+ring 선택이라 OpenSSL 설치 단계가 필요 없다. 워크플로는 조사만 했고 실행하지 않았다.
- transport: 30,000ms idle 설정의 client 전달은 (a) 설정 검증 단위 테스트, (b) 로컬 소켓에서 `pool_idle_timeout = 100ms` 유휴 후 새 연결(연결 2개) 관찰, (c) idle 재사용(연결 1개) 관찰로 확인했다. 실제 provider·TLS 경로의 pooling은 미관찰이다.
- 문서 대조: README(en/ko)·configuration(en/ko)·template(en/ko)·v24 마이그레이션 블록·tool description·`initial_instructions`가 기본값·범위(1–3, ≥300, 1–80000, ≤7일)·전송 데이터·stderr 진단·`openWorldHint`·재조회 안내·0.70 잠정·no retry/redirect·`--mock`/`--live` 구분을 동일하게 설명한다.

## 6. 품질/벤치마크 방법 (정의만; 미실행)

- 라벨 모집단: 직접 일치, 지원 근거, 부정/반박, no-match, 불명확/부족(path-only·부분·masked·stale), 언어/길이(한국어·영어·혼합, 긴 파일·많은 fragment·짧은 body), 비신뢰 내용(유도 지시문). root 추천은 평가된 snapshot의 relevant 파일 집합 G를 index 근거 한계와 함께 고정; body 필터는 query별 baseline 선택 body에 useful/unnecessary/ambiguous와 원래 행 근거, 구조적 대상·보호·unavailable 사유를 기록. 라벨은 모델 출력 전에 고정하고 사후 변경하지 않는다. calibration query와 held-out query를 분리하고 유사 코드 누출을 관리한다. 한국어 전달 검증은 언어 정확도 검증이 아니다(자동 번역 없음).
- 지표(K ∈ {1,5,10,24}, R_K = 실제 반환 상위 K): 반환 추천 정밀도@K = |R_K∩G|/|R_K|(반환 0 → N/A, 실제 반환 수 병기); Recall@K = |R_K∩G|/|G|(G 비면 N/A, matched 질문에 반환 0이면 0); 잘못된 추천 비율 = no-match 질문 중 추천 ≥1 / no-match 질문 수; 추천 누락 비율 = matched 질문 중 추천 0 / matched 질문 수(no_match/insufficient_evidence 분리 병기); 유용 body 오생략률 = 최종 출력에서 필요 근거를 잃은 useful body / labeled useful body(필터 적격 subset 분모 병기, 정책 mask vs 최종 손실 구분); 불필요 body 제거율 = 완전 생략된 unnecessary body / labeled unnecessary body(protected/ineligible/ambiguous 병기); 구조적 보존 = 유지해야 할 (파일, 행) 중 보존 수와 손실 목록; 효율 = 최종 응답 bytes/token 감소, 요청/read 수, wall time(품질과 분리).
- 분모: 의미 지표는 평가 완료 시도만; service failure/coverage bypass는 원래 전체 시도 수와 함께 별표. 실패를 no-match 정답으로 세지 않는다. 전체 작업 성공률·최종 답변 rubric·시간·비용은 fallback 포함 모든 시도에서 별도 평가. Score 자격 기준과 Noul 0.70 보정은 별개이며 조정 데이터로 최종 품질을 보고하지 않는다. confidence는 정답률이 아니다.
- 재현 기록: source/index hash, binary revision·미커밋 식별(digest), scope·출력 예산·cache 상태, model ID, evidence/question/criteria/policy 버전, threshold, 라벨 manifest; 실제 state/questions/candidate mapping·fingerprint(`request_sha256`), raw 답변(`raw_answers`), 순위/retention/protection 결정, 최종 파일+행 출력, applied/bypassed/fallback와 reason. 주 모델 input total/cached/uncached/output/reasoning은 공급자 정의대로 분리(재합산 금지); Jev usage는 알려진/미확인 시도로 별도 집계하며 요금·확인 날짜·과금 단위를 기록해 비용을 계산한다. 승인된 비교는 같은 Rust revision/snapshot/budgets로 off/#1/#2를 각 3회 실행하고 일치율을 보고하되 안정성을 보장하지 않는다. 과거 Python/rg 결과는 참고 열이며 Noul 라벨·threshold 보정에 쓰지 않는다.

## 7. 남은 한계

- 실제 모델 품질·calibration·성능·live transport(TLS·실제 pooling)·플랫폼(macOS 외) 검증은 미실행이다. 오프라인 완료는 default-on 결정을 뜻하지 않는다.
- 공식 문서의 현재 계약(모델·legend·한도) 재확인은 live 실행 전 필요하다.
- key/transport 회전의 lazy 재구성, 클라이언트별 tool metadata 갱신, MCP 취소는 검증하지 않았다(각각 코드 검사·문서 안내·프로토콜 부재).
- fixture는 최소 합성 입력이다. 대규모 저장소의 batch/deadline 거동(부분 ranking 없이 fallback)은 단위 상한 테스트로만 확인했다.

## 8. 전역 완료 조건 대응

| 조건 | 근거 |
| --- | --- |
| 시작 점검·01–05 인계가 현재 소스 상태 식별, 필수 실패 없음 | `01-runtime.md` §0, 이 문서 §1·§3 |
| 공통 평가기를 두 adapter와 예제가 사용, Python/proxy/exported index 불필요 | `src/jev` 자립(`crate::` import 0), `mcp/jev.rs`가 `Arc<dyn Evaluator>` 공유, 패키지 목록 |
| mock 예제 실행·assertions 통과 | §3 |
| 네 활성화 조합·keyless/intentless·기존 CLI/도구 호환, provider 호출 없음 | §2 기본 호환성 행, e2e 226 |
| #1 coverage·24개·no-match/부족·두 선언·역할만 실패 구분 | §2 root coverage·추천 결과 행 |
| #2 선택 근거만 판단·최종 threshold·정체성/부분/보호 유지·파일/행 대응 | §2 body 완전성·보호·threshold·accounting 행 |
| 하나의 마감 시각 공유, 알려진/미확인 사용량 구분 | 02 `one_absolute_deadline…`, e2e overview `deadline_at` 동일, deadline fallback 로그 `input_tokens=None` |
| cap에 찬 all-keep/fallback에서 base·관측 보존 | §2 출력 cap 행 |
| 버전·raw 답변 재사용 범위 고정, outbound masking/참조 검증 | 02/03 §5–§8, 01 로컬 소켓 body 검증 |
| `cargo check --all-targets` exit 0 | §3 |
| e2e + 01–03 lib 그룹 실제 통과 | §3 |
| 문서·템플릿·schema·최종 소비자 일치 | §5, `04-integration.md` §7 |
| 이 문서에 package list·실행 결과·검증 구분·지표 분모·보정/평가 분리·미실행 항목 기록 | §3–§7 |
