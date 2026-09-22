# 루트 overview 추천 검증

2026-09-23, 동일한 게시 스냅샷 전체를 평가하는 어댑터의 7개 오프라인 테스트가 통과했다. MCP 활성화는 04 단계에서 연결한다.

## 통합 계약

- `overview::run(&ToolContext)`는 기존 문자열 API를 유지한다. `overview::prepare(&ToolContext)`는 같은 게시 세대에서 만든 `base_text`, `Arc<PublishedIndexSnapshot>`, 우회 사유를 반환한다.
- `overview::jev::evaluate(snapshot, task_query, &dyn Evaluator, EvaluationOptions)`가 평가한다. 폴더·파일, 빈 인덱스, 준비/정지/오류 상태는 preparation에서 스냅샷을 제공하지 않는다. 경로 별칭과 workspace 선택 규칙은 그대로다. 활성 workspace 업데이트는 기존 MCP 호출자가 담당한다.
- 모든 파일의 선언·문서·리터럴·가능한 호출 메타데이터를 최대 6,000 UTF-8 바이트 조각으로 평가한다. 리터럴은 source를 읽지 않고 마스킹이 켜졌을 때 숨긴다. 나머지 표현 사본도 기존 redaction을 거친다. 파일/조각 식별자가 원본 스냅샷으로 연결된다. BM25 사전 선택이나 추가 source 읽기는 없다.
- `P(2)+P(3) > P(0)+P(1)`인 조각이 있는 파일만 적합하다. 이후 최대 조각 Score 내림차순·경로 오름차순으로 최대 24개를 선택한다. 동률 확률은 불확실성이다. 적합 파일이 없으면 `no_match` 또는 `insufficient_evidence`이며 소스에 구현이 없다는 결론은 내리지 않는다.
- 선택된 파일의 선언에만 두 번째 Choice 단계를 수행한다. 선언당 한 가지 대표 역할, 파일당 최대 두 선언이다. 역할이 모두 unrelated여도 적합 파일은 유지한다. 선언 범위는 `end_line_inclusive()`이며 read 창은 최대 180줄이다.
- 원시 `scores`/`roles`, 후보 매핑, 스냅샷과 ID, 사용량, `overview-questions-v1`/`overview-experimental-v1`을 결과에 보존한다. `select`로 원시 판단을 재사용할 수 있다. 두 단계는 동일한 deadline/cancellation을 사용한다.
- `render(result, available_bytes)`는 예약된 추가 공간에 전부 들어갈 때만 문자열을 반환한다. 공간 부족·평가 실패 시 호출자는 기본 overview를 유지하고 bounded fallback 진단을 반환해야 한다. 시간·사용량은 두 단계 합산하며 role 실패에도 완료된 Score 사용량을 보존한다.

## 실제 검증

저장소 루트의 `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview`: 종료 코드 0, 7개 통과.

검증 모집단은 30/31개 파일, 다중 조각 문서, 세 선언 파일이다. unrelated/tangential/동률의 무추천과 role 호출 생략, 24개 제한 이전 적합성 판정, 경로 동점 순서, 새 스냅샷과 기존 요청의 세대 분리, 역할 없음, 두 역할 제한, 정확한 끝줄·180줄 창, 작은 출력 예산 실패, role 실패 사용량, 원시 Score 재판정을 확인했다.

실모델 추천 품질과 지연은 검증하지 않았다. 전역 호환성 및 readiness/별칭은 이후 MCP/e2e 검증에서 확인한다.
