# 07 — 전체 overview 기반 추천과 live 필터 회귀

## 작업 순서와 소스 상태

- 사용자 요청대로 기존 구현을 먼저 `d14cc391f` (`feat(Jev 판단 단계와 도구별 본문 필터 추가): docs/jev-integrate`)로 커밋했다. 실행 로그는 제외했다.
- 기존 pre-commit의 fmt/Clippy 실패를 우회하지 않고 서식과 최소 동등 리팩터링·테스트 비교식을 정리했다. 최종 커밋 훅은 모두 통과했다.
- 이후 #1 입력 변경과 #2 회귀 추가는 같은 `docs/jev-integrate` 작업 트리에서 수행했으며 아직 커밋하지 않았다.
- 검증 소스: 위 HEAD + 후속 변경. `apps/codemap-search`의 수정·신규 파일에서 `validation/`을 제외하고 경로순 파일 SHA-256 목록을 다시 해시한 값: `6933da338e11aee7166cd89c49d024aba7ea7940088db0f29ae7eea9a764a09a`.
- 파일별 목록: `logs/final-source-sha256.txt`. 이전 `01`–`06` 보고서는 각 당시 소스와 계약의 역사적 기록이며 현재 완료 근거는 이 문서다.

## #1의 현재 계약

- 기본 root overview와 `RootInput::capture`가 `CodemapGenerator::generate_root_view`의 파일 요약 및 `render_file_summary`를 공유한다. 사용자 출력은 기존 파일/종류별 심볼 제한을 유지하고, Jev 입력은 모든 파일과 모든 유의미한 이름을 포함한다.
- 모노레포의 사용자 응답은 기존 workspace 목록이다. 내부 입력은 이 목록 문자열에 의존하지 않고 같은 snapshot의 모든 파일별 overview 행을 사용한다.
- 입력 내용: 파일 경로, 줄/심볼 수, 종류별 이름. 원문 본문·추가 색인 문서·소유자·행 범위·호출 그래프는 넣지 않는다. root의 통계 및 도구 사용 안내도 모델에 반복하지 않는다. MCP 재호출은 없다.
- 전체 파일 행을 마스킹하고, UTF-8 문자와 JSON escaping 비용을 고려해 최대 10,000 encoded bytes의 연속 조각으로 분할한다. 조각을 다시 합치면 전체 행과 바이트 단위로 일치한다. 공유 state에는 의도와 카탈로그/버전 정보만 넣고 전문을 매 배치 복제하지 않는다.
- Score는 구현 사실이 아니라 다음 탐색 위치로서의 적합성을 판단한다. 유용한 등급 질량이 나머지보다 큰 조각이 있으면 파일이 자격을 얻는다. 전체 평가가 끝난 뒤 최고 조각 점수·경로 순으로 최대 24개를 추천한다.
- 이름 근거가 없는 파일은 path-only의 긍정 답변만으로 자격을 얻지 않는다. 누락·중복·다른 후보 매핑·바뀐 의도/snapshot/overview에 대한 replay는 입력 fingerprint로 거부한다.
- Choice 역할 단계와 역할별 선언/호출 출력은 제거했다. 추천은 파일 overview로 연결된다. 재사용 Jev 런타임의 Choice 지원은 그대로다.
- 근거/질문/정책 버전: `overview-recommendation-projection/3`, `overview-recommendation-questions/3`, `overview-recommendation-policy/3-experimental`.
- 출력 여유 사전 검사, 전체 실패의 base 복구, no-match/insufficient 구분, 후보 누락 없는 coverage, 동일 deadline, 알려진/미확인 usage는 유지한다. MCP content/error 구조는 변경하지 않았으며 adapter의 역할 관련 Rust 결과 필드는 새 계약에서 제거했다.

## 회귀 증거

| 범위 | 실제 사례와 판정 |
| --- | --- |
| 공통 overview와 전체 입력 | `common_overview_rows_have_no_file_or_symbol_presentation_caps`: 70파일 × 12선언. 일반 출력에 없는 마지막 파일/다섯 번째 이후 심볼이 입력에 포함되고 공통 렌더러 결과와 일치 |
| 추가 인덱스 근거 배제 | `evidence_is_only_the_overview_not_private_docs_calls_or_bodies`: 이름은 포함, private 문서·호출·소스 literal은 불포함 |
| 분할 완전성 | `fragments_rejoin_exactly_and_bound_json_escaped_unicode_bytes`: 900개의 긴 Unicode/escape 이름, 조각별 크기와 전문 재결합 확인. `useful_evidence_in_the_last_fragment_can_qualify_a_large_file`: 800선언 이후 마지막 조각으로도 자격 부여 |
| 단일 Score 단계와 순위 | 전체 후보→24개, 자격 우선, 동률 경로, path-only·no-match·부족, 출력 예산, input 변경/중복/누락 replay 거부 |
| 실패/캡처 | 응답 없는 실패의 unknown usage, 실제 evaluator+mock transport의 invalid answer에서 reported usage 보존, 대기 중 새 snapshot이 기존 입력을 교체하지 않음 |
| 실제 MCP·batch 입력 | `e2e::jev_overview::monorepo_overview_sends_every_full_file_row_in_bounded_score_batches`: 두 workspace의 70파일 × 9선언. 일반 root는 workspace 목록 유지, 실제 serializer+batcher가 모든 파일을 16,000바이트 이하 여러 요청으로 전송. Score만 존재하고 60파일/4심볼 표시 제한이 입력에 적용되지 않음 |
| 긍정 생략·정확한 계측 | `read_and_grep_omit_only_unrelated_bodies_and_record_delivered_bytes`: 두 도구 × source/full 보기. 유지 행의 원래 번호·내용 보존, 무관한 본문 실제 부재, 응답 bytes=최종 text 길이, file result_bytes=기본 소스 bytes−생략한 소스 bytes |
| 원문 복원 | 위 사례에서 활성화 상태 그대로 `task_query` 없는 read로 marker의 정확한 범위를 복원. 추가 evaluator 호출 없음 |
| marker-only 파일 | `marker_only_files_are_not_recorded_as_read_source`: 두 도구에서 marker와 호출 자체는 존재하지만 file_observations는 비어 있음. `complete_bodies_in_unexpanded_grep_context_can_be_omitted`도 확인 |
| 부분·보호·페이지 | `partial_windows_rows_and_metadata_modes_never_send_body_questions`: 6모드에서 byte identity와 요청 0. `retained_callers_and_nested_declarations_protect_their_bodies`: 호출 연결·중첩 보존. `an_omitted_grep_page_keeps_its_footer_and_does_not_refill_from_the_next_page`: footer/next_offset 보존, 뒤 페이지로 채우지 않음 |
| 전체 유지/실패 | `all_keep_and_provider_failure_preserve_text_and_source_observations`: 두 도구 × 두 보기 × 두 결과에서 실제 text와 SQLite observations가 base와 동일 |
| 마스킹·실제 버퍼 | `captures_masked_payload_and_keeps_the_snapshot_during_inference`: 두 도구에서 전송 입력의 합성 secret 마스킹, 대기 중 파일 교체 후에도 기존 유지 소스 사용, 후속 무필터 read는 새 live source 반환 |
| 설정·스키마 | `tool_flags_schema_reload_and_threshold_reach_live_consumers`: 도구별 openWorldHint/task_query, 독립 활성화, 1.0 유지→reload 후 0.70 생략의 실제 결과, 잘못된 task_query 거부 |
| 비ASCII 식별자 | `identity_matching_advances_on_unicode_boundaries_after_a_substring_match`: 긴 식별자 안의 부분 일치 후 UTF-8 경계를 지키며 실제 선언을 확인 |

live 계측 검사는 in-process MCP의 실제 `CallRecorder`를 켜고 `.codemap/analysis.sqlite3`를 읽는다. 테스트에서 보존 정책을 재구현하지 않는다. 파일/행 범위로 반환 소스를 대조하며, 최종 응답 bytes와 파일 단위 소스 bytes를 구분한다.

## 최종 실행 결과

cwd는 모두 `/Users/buyong/workspace/private/buyong-mcp`. 아래 Cargo 명령은 모두 `--manifest-path apps/codemap-search/Cargo.toml` 사용.

| 명령 | 결과 | 로그 |
| --- | --- | --- |
| `cargo fmt --check` | exit 0 | `logs/final-fmt.log` |
| `cargo clippy --all-targets -- -D warnings` | exit 0 | `logs/final-clippy.log` |
| `cargo check --all-targets` | exit 0, 경고 0 | `logs/final-check.log` |
| `cargo test --lib` | 360 passed, 0 failed, 기존 ignored 2 | `logs/final-lib-all.log` |
| `cargo test --test e2e_tests` | 236 passed, 0 failed, 기존 ignored 1 | `logs/final-e2e-all.log` |
| `cargo run --example jev_decisions -- --mock` | Score/Choice/Noul/usage/request assertions 통과 | `logs/final-example.log` |
| `cargo package --list --allow-dirty` | exit 0, 444 entries | `logs/final-package.log` |
| `git diff --check` | exit 0 | 도구 출력에서 확인 |

최종 suite에는 overview unit 16개, search 필터 unit 26개, 새 live MCP 9개와 overview 실제 batch MCP 1개가 포함된다. 역할 단계 제거로 더 이상 해당하지 않는 unit 기대값을 새 입력 계약의 사례로 교체했다. 기존 MCP 성공/no-match/부족 assertions도 새 출력 문구에 맞췄으며 오류를 숨기거나 필수 사례를 ignored로 바꾸지 않았다.

## 한계

- 실제 TypeSafe provider 요청, 실제 의미 판단 품질·보정·성능 비교, TLS/pooling 및 다른 플랫폼 빌드는 실행하지 않았다. 전송은 mock transport, 판단은 injected evaluator만 사용했다.
- 기본값은 계속 꺼짐이다. 새 overview 입력이 더 단순하고 의도에 맞는다는 판단과 실제 모델 품질 향상은 별개다.
- 패키지 목록은 포함 파일의 확인이지 release 빌드나 발행의 증거가 아니다. 이번 작업은 publish/push를 수행하지 않았다.
