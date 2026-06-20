# persist_fix_report — codex MCP arm 텔레메트리 버그 데이터셋 교정

생성: 2026-06-20

## 수행 내용

### 문제
`scored_episodes.180.json`의 codex MCP arm 27 에피소드가 원본 -02 텔레메트리 버그값 그대로였음:
- `backend_exercised=false` (전부)
- `tool_call_distribution={}` (전부 비어있음)

`aggregate.mjs`는 메모리에서만 교정을 적용해 표는 맞지만 데이터셋은 버그값 — 자기모순 상태.

### 수정 방법
`aggregate.mjs`에 두 가지 변경을 추가:

1. **교정 루프 확장** (라인 210 이후): 기존 `backend_exercised` 덮어쓰기에 더하여:
   - `tool_call_distribution`이 비어있으면 `rebuildCodexToolDist()`로 재구성하여 덮어쓰기
   - `telemetry_corrected=true`, `backend_exercised_source="stdout_rebuilt"` provenance 필드 추가
   - 재구성 함수는 기존 `rebuildCodexToolDist()` 그대로 사용 (파싱 구현 단일 진실원 유지)

2. **persist 단계 추가** (mcp_comparison_tables.json write 직전):
   - 교정된 `eps[]`를 `scored180.episodes`에 반영
   - `scored180.count`, `scored180.schema_parity_ok`, `scored180.schema_parity_note`, `scored180.integration_stats` 갱신
   - `scored_episodes.180.json` write

### 파일 변경
- `<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-03/analysis/aggregate.mjs`: 교정 루프 확장 + persist 단계 추가
- `<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-03/analysis/scored_episodes.180.json`: codex 27개 교정값 persist
- `<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-03/analysis/mcp_comparison_tables.json`: 재생성
- `<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-03/analysis/mcp_comparison_tables.md`: 재생성

## 검증 결과

| # | 항목 | 기댓값 | 실측값 | 결과 |
|---|------|--------|--------|------|
| 1 | 총 에피소드 수 | 180 | 180 | **PASS** |
| 2 | codex MCP arm 27개 전부 backend_exercised=true | 27/27 | 27/27 | **PASS** |
| 3 | backend_off(non-no-mcp & !backend_exercised) | 24 | 24 | **PASS** |
| 4 | codex 27개 tool_call_distribution 비어있지 않음 | 0개 비어있음 | 0개 비어있음 | **PASS** |

**전체: 4/4 PASS**

### backend_off=24 구성
전부 opencode 런타임: `opencode-deepseek`(7) + `opencode-mimo`(12) + `opencode-minimax`(5) = 24
codex 기여 없음 (codex MCP arm 전수 교정됨).

### provenance 필드
codex MCP arm 27개 전부에 `telemetry_corrected=true`, `backend_exercised_source="stdout_rebuilt"` 추가 확인됨.

### idempotent 확인
aggregate.mjs 2회 연속 실행 시 2차 실행에서 `backend_exercised 뒤집힌 episode: 0개` — 데이터셋이 이미 교정값이므로 재실행 안전.

## schema_parity_ok 처리
codex 27 레코드에만 provenance 필드가 추가되어 스키마 비대칭 발생.
- `schema_parity_ok=false` (의도적)
- `schema_parity_note`: "codex MCP arm 27 레코드에 telemetry_corrected=true, backend_exercised_source='stdout_rebuilt' 추가됨. 나머지 153+27 레코드는 해당 필드 없음."
