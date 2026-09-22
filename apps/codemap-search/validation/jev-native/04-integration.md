# 네이티브 MCP·설정 통합 검증

2026-09-23, Rust MCP의 네 가지 모드 조합, 설정 고정과 기본 결과 복구를 오프라인 주입 전송으로 확인했다. 기존 설정 e2e 9개, MCP e2e 26개가 통과했다. 큰 입력 두 사례를 생략하지 않았으며 MCP 그룹 전체는 216.17초에 완료했다.

## 활성화 계약

`analysis.jev`의 정식 키와 기본값:

| 키 | 기본값 |
| --- | --- |
| `overview_enabled` | `false` |
| `search_filter_enabled` | `false` |
| `model` | `jev-1.13.0` |
| `api_key_env` | `TYPESAFE_API_KEY` |
| `timeout_ms` | `45000` |
| `max_in_flight_requests` | `3` |
| `request_spacing_ms` | `300` |
| `max_batch_bytes` | `80000` |
| `pool_idle_timeout_ms` | `30000` |
| `search_filter_min_unrelated_probability` | `0.70` |

설정 버전 24, 영문·한글 템플릿, 키별 repo/global/default 병합과 인자 스키마를 함께 변경했다. 정수 범위는 01-runtime의 한도를 넘지 못하며 threshold는 유한한 `0.5 < 값 <= 1.0`이다. model은 고정 ID만, api_key_env는 환경변수 이름만 허용한다. 잘못된 값은 경고 후 하위 계층을 사용한다. 마이그레이션은 섹션별로 누락된 주석 키만 추가하며 다른 섹션의 model이나 사용자 주석·값을 덮어쓰지 않는다.

## 호출과 수명

운영자는 MCP 프로세스 환경에 자신의 키를 제공하고 둘 중 필요한 설정을 활성화한다. 아래는 `tools/call`의 전체 요청이다.

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"overview","arguments":{"path":".","task_query":"호출자의 취소가 외부 요청까지 전달되는 흐름 추적"}}}
```

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search","arguments":{"query":"cancellation request","task_query":"호출자의 취소가 외부 요청까지 전달되는 흐름 추적"}}}
```

task_query는 기존 query/path 별칭과 독립적이다. 생략·공백이면 평가를 우회하고 비문자열은 -32602이다. task_query는 다른 요청에서 재사용하지 않는다. 기본 비활성 호출의 content는 기존과 동일하며 live read/find/grep, 이벤트 전용 결과, initial_instructions는 추론하지 않는다.

`McpServer::new`는 유지한다. `with_evaluator`는 명시적인 Rust 호스트 주입이며 MCP 인자나 운영 설정으로 선택할 수 없다. `handle_request`와 `handle_request_with_options`는 같은 순차 처리 경로를 사용한다. 기존 notification 무응답/순서·JSON-RPC 결과/오류 형태를 유지한다. 네트워크 동안 config/index mutex를 보유하지 않는다.

요청 시작에 설정과 redaction을 고정하고 caller deadline/cancellation을 보존한다. 진행 중 threshold=0.70일 때 설정을 0.90으로 바꾼 회귀에서 Noul=0.80 본문은 진행 중 요청에서 생략되고 다음 요청에서 유지됐다. 준비한 Arc 스냅샷과 검색 출력은 await를 넘어 같은 요청이 소유한다. 자격 증명은 적격 호출에서만 환경변수로 해결하고 같은 정책·키를 쓰는 호출은 평가기/연결 pool을 재사용한다. 설정·키가 바뀌면 다음 요청에서 교체한다.

## 응답·관측

`content` 및 JSON-RPC error는 그대로이며 선택적 `_meta.jev`에 outcome, reason, model/policy, 실제 threshold, recommendation_status, 평가/선택 개수, API 입력·출력, 전체/완료 HTTP 시간(ms)을 추가한다. 비활성 모드는 이 메타데이터도 추가하지 않는다. 본문 상태 안내와 추천 내용은 기존 content byte cap 안에 들어갈 때만 추가한다. 꽉 찬 본문의 기본 근거를 유지하면서도 `_meta.jev`로 우회·복구를 검사할 수 있다.

overview의 `selected_count`는 표시 추천 파일 수, search에서는 생략 본문 수다. 실패·시간 초과·취소·키 없음·부적격 입력은 기본 결과를 보존한다. `no_match`는 정상 평가 상태이며 전송 실패와 구분한다. usage_scope는 보고된 응답만 의미하며 미보고 비용을 0으로 단정하지 않는다. source 관측은 검색 필터와 cap 이후 전달한 근거를 사용하고 전체 응답 바이트는 최종 마스킹 후 기록한다.

## 실제 검증과 문서

저장소 루트에서 종료 코드 0:

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib config::jev`: 2개 통과.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev`: 별도 프로세스에서 실제 Rust parser/index/MCP를 통과한 모드 행렬 1개 통과. Rust 선언 kind 기대값을 기존 fn에 맞춰 수정한 후 재실행했다.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config`: 9개 통과.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp`: 26개 통과.
- `cargo check --manifest-path apps/codemap-search/Cargo.toml`: 통과.
- 영문·한글 설정 예시의 10개 Jev 키와 템플릿 블록 일치를 직접 확인했다.

README와 configuration의 두 언어 문서에 명시적 task_query, 환경변수, 독립 활성화, 전송 범위, byte/token 한계, 무추천·불확실성·복구와 mock/live 예제를 기록했다. 실제 공급자 호출, 다른 플랫폼 빌드, Noul 보정, 속도·답변 품질 비교는 수행하지 않았다.
