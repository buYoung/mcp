# 06 — #2를 search·read·grep으로 확장

## 범위와 소스 상태

- 후속 사용자 요청: #2의 적용 도구를 `search`, `read`, `grep`으로 확장. #1 overview는 제안 내용을 재검토해 보고하며 이번에는 구현을 변경하지 않음.
- 브랜치 `docs/jev-integrate`, HEAD `530e476c3`. 기존 작업 트리를 보존했으며 stage/commit/branch 전환 없음.
- 변경 소스 식별: 저장소 루트 기준 `apps/codemap-search`의 수정·신규 파일 51개에서 `validation/`을 제외한 경로를 정렬하고, 파일별 `shasum -a 256` 출력 목록을 다시 SHA-256 처리.
- digest: `41d2bc16eae62b868e49096d9c74dbba60fe7269262c89df672f8789f36320e2`.
- `01`–`05` 인계와 그 로그는 search만 대상으로 하던 이전 구현의 기록이다. 이번 확장에 대한 실행 범위와 한계는 이 문서를 기준으로 한다.

## 구현

- `[analysis.jev].read_filter_enabled`, `grep_filter_enabled` 추가. 각각 기본값 false이며 기존 search 활성화만으로 live 도구를 켜지 않는다. 스키마 25 마이그레이션은 새 키를 주석으로 추가하고 기존 활성 값을 보존한다.
- `task_query`를 read/grep schema와 MCP 인자 검사에 추가. 유효 의도·해당 도구 활성화·인증정보가 있을 때만 실행한다. 도구별 설명과 `openWorldHint`도 같은 활성화 설정을 따른다.
- `search_filter_min_unrelated_probability`는 공개 키 이름을 유지한 채 세 도구의 공통 임계값으로 사용한다. 기본 0.70, 유효 범위 `(0.5, 1.0]` 유지.
- 공통 Noul 평가·생략 임계값·호출/중첩 보호 정책은 `src/tools/search/jev.rs`를 재사용한다. search의 입력 구조/버전은 유지하고 live 입력은 `tool`, `tool_arguments`, `task_query`와 본문별 `candidate`를 사용한다.
- live 근거/질문 버전: `live-body-filter-evidence/1`, `live-body-filter-questions/1`. 공통 정책은 `search-filter-policy/2-experimental`.
- 새 `src/tools/live_symbols/jev.rs`는 출력 생산자가 제공한 파일·행·바이트 구간을 이용한다. 문자열에서 파일 경로나 행 번호를 역으로 추측하지 않는다.
- read는 선택된 창, grep은 선택된 content 페이지에서 완전한 함수 본문만 판단한다. `expand=none`도 선택 결과 안에 본문 전체가 있으면 후보가 될 수 있다. `view=definitions|relations`와 grep `files_with_matches|count`는 우회한다.
- 선언/안전한 함수 범위는 출력에 사용한 동일한 UTF-8 live 버퍼에서 기존 bounded parser로 추출한다. 오래된 인덱스로 생략 범위를 정하거나 Jev 대기 후 소스를 다시 읽지 않는다. 크기·언어·문법·경계·정체성 검사를 통과하지 못한 본문은 유지한다.
- 소스 마스킹은 전송 전에 적용한다. 인수의 문자열과 민감한 키 이름에도 기존 redaction 정책을 적용한다. 원래 소스/인덱스/파일 권한 정책은 바꾸지 않는다.
- 기본 응답과 문맥을 먼저 완성한 뒤, producer 구간을 최종 렌더링 위치에 연결해 생략한다. 생략으로 생긴 공간을 새로운 소스/관계로 채우지 않는다. live 도구에는 별도의 inline 상태 요약을 넣지 않는다.
- 파일 제목·문맥·페이지 안내는 보존하고, 생략한 본문과 생략 안내를 source observations에서 제외한다. 전부 유지/실패는 기본 text와 observations를 보존한다. 생략 없는 복원은 `read`에서 `task_query`를 생략하면 된다.
- grep 기본 응답이 명시적 최종 응답 한도를 이미 넘으면 `base_output_over_cap`으로 Jev만 우회한다. 최종 마스킹 이후의 기존 cap 판정은 MCP가 수행한다.
- 공통 식별자 검사의 실패한 일치 후 이동을 UTF-8 문자 경계로 보완했다. 비ASCII 이름을 포함하는 live source에서 문자열 중간 바이트를 잘라 panic할 수 있는 경계를 제거한다.

## 실행한 검증

모든 Cargo 명령 cwd: `/Users/buyong/workspace/private/buyong-mcp`. 모든 명령에 `--manifest-path apps/codemap-search/Cargo.toml` 사용. 실제 TypeSafe 요청은 실행하지 않았으며 기존 mock/로컬 소켓 검증만 사용했다.

| 명령(공통 manifest 인자 생략) | 결과 | 로그 |
| --- | --- | --- |
| `cargo check --all-targets` | 최종 소스 exit 0, 경고 0 | `logs/live-check.log` |
| `cargo test --lib` | 확장 구현 후 365 통과, 실패 0, 기존 ignored 2 | `logs/live-lib.log` |
| `cargo test --test e2e_tests` | 확장 구현 후 226 통과, 실패 0, 기존 ignored 1 | `logs/live-e2e.log` |
| `cargo test --lib jev` | grep cap 우회·인수 마스킹 보완 후 93 통과 | `logs/live-lib-jev.log` |
| `cargo test --test e2e_tests e2e::tools` | 위 보완 후 35 통과, 기존 ignored 1 | `logs/live-tools.log` |
| `cargo test --lib tools::search` | 마지막 UTF-8 식별자 보완 후 25 통과 | `logs/live-search.log` |
| `cargo package --list --allow-dirty` | exit 0, 442 entries, `src/tools/live_symbols/jev.rs` 포함 | `logs/live-package.log` |
| `git diff --check` | 최종 exit 0 | 도구 출력에서 확인 |

전체 suite 실행 뒤에는 grep의 한도 초과 우회, 인수 마스킹, UTF-8 식별자 검사만 보완했다. 이후 영향받는 기존 검증을 위 표처럼 실행했으며 전체 suite를 최종 소스에서 다시 실행했다고 주장하지 않는다. 패키지 목록 확인 이후 파일 집합은 바뀌지 않았다.

기존 설정/마이그레이션 사례의 버전 기대값과 기본값 assertions를 현재 계약에 맞췄다. 새 테스트 파일/사례는 추가하지 않았다.

## 검증 한계와 필요한 후속 검증

- 기존 read/grep 회귀와 설정 회귀는 통과했으나, **새 `read_filter_enabled=true`/`grep_filter_enabled=true` 경로의 실제 생략·보호·fallback·관측 계측을 직접 확인하는 회귀 사례는 아직 없다.** 컴파일/기존 suite 통과를 이 검증의 대체로 보지 않는다.
- 새 활성 경로에는 최소한 실제 mock evaluator를 주입한 MCP 요청으로 긍정 생략, 부분/중첩/호출 연결 보존, 의도 없는 복원, 실패/all-keep byte identity, 전송 전 마스킹, marker-only 파일의 관측 제외를 확인하는 사례가 필요하다.
- 실제 provider, TLS/pooling, 모델 품질·임계값 보정, 다른 OS 빌드는 미실행이다.
- UTF-8 식별자 보완에 대한 별도 비ASCII 선언 회귀도 아직 추가하지 않았다.

## #1 재검토 결과 — 구현 보류

공통 overview 콘텐츠를 Jev 입력으로 재사용하는 방향을 권장하되, 현재 root 응답의 byte cap만 없애는 변경으로는 부족하다.

1. 일반 root는 모든 파일의 summary를 구성하지만 출력에서 파일/디렉터리 60개와 종류별 심볼 4개로 줄인다. 이런 표시용 절단을 입력용 경로에서 제거해야 한다.
2. 모노레포 root는 workspace 목록으로 분기한다. 파일 추천을 위해서는 모노레포에서도 각 파일의 개요를 포함하는 내부 전체 모드가 필요하다.
3. 기존 요약 데이터/렌더링 로직을 공통화해 사용자 출력에는 기존 제한, Jev에는 전체 파일별 개요를 적용한다. MCP overview 재호출이나 전체 문자열을 공유 state에 반복하는 방식은 권장하지 않는다.
4. 모델 입력은 파일 경계로 분할하고 전송 byte/token 한도·마감·마스킹은 유지한다. snapshot의 파일 수와 전달된 파일 수를 대조해 coverage를 확인해야 한다.
5. overview 수준의 이름/종류 요약으로 바꾸면 현재 전용 입력의 문서·호출 근거가 줄어든다. 파일 탐색 추천에는 목적이 맞지만 품질 향상을 단정할 수 없다. 상세 caller/consumer 역할 판정은 유지하기보다 별도로 재정의하거나 제외하는 편이 타당하다.

이번에는 위 설계 검토만 했고 overview 입력·추천 동작을 변경하지 않았다.
