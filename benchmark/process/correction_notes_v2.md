# metric 교정 노트 v2 (cms-official-benchmark-20260619-03-v3)

생성일: 2026-06-20
원본 run: cms-official-benchmark-20260619-02
v2 교정 범위: backend_exercised 재계산 + backend_off 재산출 + caveat 교정 + opencode task 비대칭 경고

---

## 1. backend_exercised 재계산 — 버그파생값 교정

### 버그 원인
runner의 `extractCodexOutput` 함수가 `toolEvents: []`를 하드코딩 반환.
codex stdout에 `item.completed/mcp_tool_call` 이벤트가 실재하지만,
scored_episodes의 모든 codex MCP arm 에피소드에서 `backend_exercised=false`로 오기록됨.

### 교정 방법
각 codex MCP arm 에피소드의 `stdout.txt`를 node/fs로 파싱.
배정 server의 `mcp_tool_call` 중 `item.completed` 이벤트이며 `error=null`인 성공 호출이 ≥1이면 `backend_exercised=true`.

배정 server 매핑:
- `codex-gpt54-codegraph` arm → server="codegraph"
- `codex-gpt54-codemap` arm → server="codemap-search"
- `codex-gpt54-serena` arm → server="serena"

### 뒤집힌 에피소드 목록 (27개 전수 false→true)

| arm | codebase | round | old | new | ok | error |
|---|---|---|---|---|---|---|
| codex-gpt54-codegraph | angular-main | 1 | false | true | 3 | 0 |
| codex-gpt54-codegraph | angular-main | 2 | false | true | 5 | 0 |
| codex-gpt54-codegraph | angular-main | 3 | false | true | 3 | 0 |
| codex-gpt54-codegraph | ClickHouse-master | 1 | false | true | 13 | 0 |
| codex-gpt54-codegraph | ClickHouse-master | 2 | false | true | 12 | 0 |
| codex-gpt54-codegraph | ClickHouse-master | 3 | false | true | 9 | 0 |
| codex-gpt54-codegraph | deno-main | 1 | false | true | 13 | 0 |
| codex-gpt54-codegraph | deno-main | 2 | false | true | 27 | 0 |
| codex-gpt54-codegraph | deno-main | 3 | false | true | 44 | 0 |
| codex-gpt54-codemap | angular-main | 1 | false | true | 5 | 0 |
| codex-gpt54-codemap | angular-main | 2 | false | true | 8 | 0 |
| codex-gpt54-codemap | angular-main | 3 | false | true | 8 | 0 |
| codex-gpt54-codemap | ClickHouse-master | 1 | false | true | 14 | 0 |
| codex-gpt54-codemap | ClickHouse-master | 2 | false | true | 11 | 0 |
| codex-gpt54-codemap | ClickHouse-master | 3 | false | true | 21 | 0 |
| codex-gpt54-codemap | deno-main | 1 | false | true | 34 | 6 |
| codex-gpt54-codemap | deno-main | 2 | false | true | 40 | 0 |
| codex-gpt54-codemap | deno-main | 3 | false | true | 30 | 0 |
| codex-gpt54-serena | angular-main | 1 | false | true | 11 | 0 |
| codex-gpt54-serena | angular-main | 2 | false | true | 20 | 0 |
| codex-gpt54-serena | angular-main | 3 | false | true | 15 | 0 |
| codex-gpt54-serena | ClickHouse-master | 1 | false | true | 26 | 0 |
| codex-gpt54-serena | ClickHouse-master | 2 | false | true | 28 | 0 |
| codex-gpt54-serena | ClickHouse-master | 3 | false | true | 15 | 10 |
| codex-gpt54-serena | deno-main | 1 | false | true | 2 | 10 |
| codex-gpt54-serena | deno-main | 2 | false | true | 19 | 0 |
| codex-gpt54-serena | deno-main | 3 | false | true | 47 | 5 |

- claude/opencode 에피소드: 교정 없음 (텔레메트리 정상)

---

## 2. backend_off 재산출

집계 기준: non-no-mcp 에피소드 전체(valid/invalid 포함)에서 `backend_exercised=false` 수.
(audit.totals.backend_exercised_false와 동일 기준)

| 항목 | old (텔레메트리 버그파생) | new (교정 후 실측) |
|---|---|---|
| backend_off 전체 | 39 | 12 |
| codex-gpt54 기여분 | 27 | 0 |
| 비-codex 기여분 | 12 | 12 |

- old=39: codex 27개 버그파생 + 비-codex 12개(opencode 일부)
- new=12: codex 전수 교정됨, 비-codex 12개 유지(측정 무결성 확인됨)

---

## 3. codex-serena degraded 판정 근거

codex-serena는 9 에피소드 전수 `backend_exercised=true`이지만 일부 에피소드에서 serena 호출 에러 발생.

| codebase | round | ok | error | unfinished | exercised | 판정 |
|---|---|---|---|---|---|---|
| ClickHouse-master | 1 | 26 | 0 | 0 | true | clean |
| ClickHouse-master | 2 | 28 | 0 | 0 | true | clean |
| ClickHouse-master | 3 | 15 | 10 | 0 | true | degraded |
| deno-main | 1 | 2 | 10 | 0 | true | degraded |
| deno-main | 2 | 19 | 0 | 0 | true | clean |
| deno-main | 3 | 47 | 5 | 0 | true | degraded |
| angular-main | 1 | 11 | 0 | 0 | true | clean |
| angular-main | 2 | 20 | 0 | 0 | true | clean |
| angular-main | 3 | 15 | 0 | 0 | true | clean |

- 에러 발생 에피소드: 3/9 (ClickHouse-master round-3, deno-main round-1, deno-main round-3)
- 총 에러 호출: 25건 (ok=183건 대비 ~12%)
- 판정: **degraded** (exercised=true이지만 clean이 아님)
- "serena 전수 타임아웃"이라는 이전 기술은 부정확. 정확히는 "일부 에피소드에서 에러 발생".

---

## 4. caveat 변경 전후

### 변경 전 (v2)

- codex MCP arm: `codex_backend_off_caveat=true` → 표에 "⚠full" 마킹, "MCP 전수 미사용 → 비교 무의미" 문구
- 런타임 confound: 위 caveat에 묶여 표현됨 (별도 구분 없음)
- serena: "전수 미사용" caveat에 포함 (degraded 개념 없음)
- opencode task: 각주 수준 언급

### 변경 후 (v3)

- codex MCP arm: `codex_backend_off_caveat=false` (교정됨) → "⚠full" 마킹 제거
- 런타임 confound: `codex_runtime_confound_caveat=true` 별도 플래그로 분리 보존
  - 표 confound 열: sandbox(codex 전반) / sandbox+degraded(codex-serena)
- serena: `codex_serena_degraded=true` → 표에 "sandbox+degraded" 마킹
- opencode task: **본문 레벨 경고로 격상** (각주 아님)
  - "⚠ opencode task 서브에이전트 내부 도구 과소집계: task로 위임하는 opencode 행은 실제보다 도구 수가 적게 보인다"
- 헤드라인 추가: "codex는 usable한 두 번째 비교 — codemap/codegraph usable, serena degraded. claude와 동급 clean 비교 아님."

---

## 5. 불변 항목 확인

- `scorer_score`: 변경 없음 (raw_answer 기반, 유효)
- `per_fact_score`: 변경 없음
- `valid/invalid` 분류: `harness_valid`, `timed_out` 원본값 그대로
- `tok_in`, `tool_call_breakdown`: v2에서 이미 교정됨 (v3 유지)
- 원본 -02 파일 일절 수정 없음

