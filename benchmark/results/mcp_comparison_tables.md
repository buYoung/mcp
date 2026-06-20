# MCP 비교표 — cms-official-benchmark-20260619-03 (metric correction v4 / 180 통합)

생성: 2026-06-20T06:17:33.755Z · 원본 run: cms-official-benchmark-20260619-02 · judge=opus=claude-opus-4-8 · scorer formula match=true
metric correction: cms-official-benchmark-20260619-03-v4 — tok_in 런타임별 규약 교정 + tool_call 괄호 분해 + codex stdout 재구성 + backend_exercised 재계산 + opencode-serena 27 실데이터 통합(180 기준)

## ⚠ 텔레메트리 비대칭 경고 (데이터 해석 전 필독)

**[경고 1] codex-gpt54 backend_exercised 버그 — v3에서 교정됨**

이전(-03 v2)의 backend_off=39(codex-gpt54 단독 27)은 텔레메트리 버그 파생값이다. runner의 `extractCodexOutput`이 `toolEvents:[]`를 하드코딩 반환해 codex MCP arm 전 에피소드가 `backend_exercised=false`로 오기록됐다. stdout `item.completed/mcp_tool_call` 파싱으로 재계산 결과 **codex MCP arm 27 에피소드 전수 교정(false→true)**. 교정 후 backend_off=24(codex 기여 0).

**[경고 2] opencode task 서브에이전트 내부 도구 과소집계 — 미교정 (측정 불가)**

opencode가 `task` 서브에이전트로 위임한 내부 도구 호출은 **부모 세션에 집계되지 않는다**(`task(N)[inner_untracked]`). 이는 codex 버그와 같은 계열의 실사용 도구 과소집계다. task로 위임하는 opencode 행은 실제보다 도구 수가 적게 보인다. 도구 수 기반 효율 비교 시 opencode의 과소집계를 반드시 감안해야 한다. (codex는 v3에서 교정됐으나, opencode task 내부는 구조적으로 추적 불가 — 미교정 상태 유지.)

**[경고 3] codex 런타임 confound — 교정 불가, 해석 시 감안 필수**

codex는 read-only OS sandbox이므로 mutating bash가 없다. claude(mutating bash)와 실행환경이 본질적으로 다르다. 이는 텔레메트리 버그와 무관하게 유효한 한계다. codex의 MCP 효과를 claude와 동급의 clean 비교로 읽으면 안 된다. codemap/codegraph는 usable(사용가능한 2차 비교), serena는 degraded(일부 에피소드 에러 발생).

**[경고 4] opencode-serena 약체 데이터 — v4 신규 추가, 과소집계 동일 적용**

opencode-serena(deepseek/mimo/minimax × serena × 3 codebase × 3 round = 27 에피소드)는 v4에서 real 데이터로 통합됐다. 단: (1) 평균 점수 0.1506(전체 arm 중 최저권). (2) backend_exercised=15/27 — 15개만 serena를 실제 사용. (3) opencode task 서브에이전트 과소집계(경고2)가 동일 적용 — serena 내부 task 위임 호출 미집계. (4) per_fact_score 없음(scorer 입력 불일치로 null) → 사실 단위 분석 불가. no-mcp 셀 없어 paired-delta 산출 불가. 탐색 데이터로 취급; 통계적으로 유의한 결론 도출에 충분하지 않다.

## 요약 (denominator·skip·무결성)

| 항목 | 값 |
|---|---|
| nominal N | 180 (5 model/runtime × 4 backend × 3 codebase × 1 task × 3 round) |
| executed N [v4: 180] | 180 (구: 153 + 신규 opencode-serena 27) |
| skipped N [v4: 0] | 0 — 없음(opencode-serena 실행됨) |
| quality valid (harness_valid && !timed_out) [v4: 180 기준] | 166 |
| harness invalid | 14 (timeout 10) |
| backend_exercised=false [v4: 180 기준 실측] | 24 (구 153기준: 39, codex 27 교정됨 + opencode-serena신규 12/27 추가) |
| codex backend_exercised 교정 | 0개 false→true (codex MCP arm 전수) |
| codex-serena degraded | true (에러발생 에피소드 3/9) |
| opencode-serena [v4 신규] | 27 에피소드, valid=22, backend_exercised=15/27, avg_score=0.1506 (약체 — caveat 참고) |
| mutation violations | 0 (mutation_guard 전 episode clean) |

> 해석 규칙: 166 valid episodes (harness_valid && !timed_out, 180 기준). 기존 153 셀 quality_stats: integrity_audit.quality_stats 동결값 사용(점수·valid/invalid 불변). 신규 opencode-serena 셀: scored_episodes.180.json의 score 필드에서 직접 재계산. · 180 executed episodes (180 기준, invalid 14개 포함, answer_extraction_status로 flag). timeout/empty 행의 wall_time·tool_calls 는 절단(truncated)되었을 수 있으므로 평균 해석 시 제외 권장. · paired_delta = mean(backend cell) − mean(no-mcp cell), 같은 codebase×runtime 내. round 끼리 매칭하지 않음(round 는 독립 시도). [v4] opencode-serena는 no-mcp 셀이 없어 paired-delta 산출 불가 → pairedDelta=null, winOrTie=null. · 동률 밴드 = task fact band (±0.25 ClickHouse, ±0.125 deno/angular). 그보다 좁은 차이는 n=3 noise. · [v3 교정] codex-gpt54의 backend_exercised=false는 텔레메트리 버그(extractCodexOutput이 toolEvents:[]를 하드코딩)로 인한 오기록이었음. stdout item.completed/mcp_tool_call 파싱 결과 codex MCP arm 전수(codegraph 9, codemap 9, serena 9 = 27 에피소드) backend_exercised=true로 교정. 따라서 codex도 사용가능한(usable) 두 번째 비교다 — backend별 입자도: codemap/codegraph usable, serena degraded(일부 에피소드 serena 호출 에러 발생). 단 codex는 read-only OS sandbox이므로 mutating bash가 없고 실행환경이 claude(mutating bash)와 본질적으로 다르다(런타임 confound). 이는 텔레메트리 버그와 무관하게 유효한 한계이며 claude와 동급의 clean 비교라고 과장해서는 안 된다. · ⚠ opencode가 task 서브에이전트로 위임한 내부 도구 호출은 부모 세션에 집계되지 않는다(task(N)[inner_untracked]). 이는 codex 텔레메트리 버그와 같은 계열의 실사용 도구 과소집계다. task로 위임하는 opencode 행은 실제보다 도구 수가 적게 보인다. 도구 수 기반 효율 비교 시 opencode의 과소집계를 반드시 감안해야 한다. opencode-serena 신규 27 에피소드에도 동일하게 적용 — task 위임 내부 serena 호출은 미집계. · [v4 신규] opencode-serena(deepseek/mimo/minimax)는 약체 데이터원이다. 평균 점수 ~0.1506(유효 에피소드 기준)로 전체 arm 중 최저권. backend_exercised=15/27 — 미호출 12개는 serena 도구를 사용하지 않고 builtin만 사용했음을 의미. per_fact_score는 scorer 입력 불일치로 null(점수만 있음). opencode 특유의 task 서브에이전트 위임 과소집계 적용. 이 데이터는 serena+opencode 조합의 탐색 데이터로 취급할 것; statistically significant 결론 도출에 충분하지 않다(n=3/cell, 점수 낮음). · n=3/cell, 1 task/repo 이므로 IQR/SE 넓고 대부분 paired delta 는 비유의. inferential 아닌 descriptive. opencode-serena(신규 27)는 per_fact_score 없어 사실 단위 분석 불가. · tok_in 보정 공식 (런타임별): claude-sonnet·opencode-* → input_tokens + cache_read_input_tokens + cache_creation_input_tokens (stdout usage 실측: claude input_tokens=신규분만, cache 별도 필드); codex-gpt54 → input_tokens 그대로 (turn.completed usage 실측: input_tokens=캐시포함 합산, cached_input_tokens=그 중 캐시분 → 재합산 시 이중계산). 보정 후 tok_in은 전 런타임 공통 '캐시 포함 총 입력 토큰'. · tool_call_breakdown: 모든 도구 실행 흔적. ToolSearch(스키마 로딩 메타도구)는 tool_calls_total에서 제외, tool_search_count 열로 별도 표기. invalid/skill/todowrite 등 내부 도구도 제외. codex-gpt54는 scored_episodes 추출기 갭으로 distribution이 비어있어 stdout item.completed 이벤트에서 재구성(tool_call_source=stdout_rebuilt). codegraph·serena·codemap backend는 도구별 괄호 분해(예: explore(N), node(M)); codegraph_explore의 arguments에 mode/kind 없음 → operation 재분해 불가, 도구명 단위 분해.

### tok_in 런타임별 규약 (결함1 교정)

| 런타임 | tok_in 산출 공식 | 근거 |
|---|---|---|
| claude-sonnet | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` | stdout result 이벤트 usage: input_tokens=신규분만(예: 10), cache 필드 분리. 보정 전 tok_in=10 → 보정 후 ~186k |
| opencode-* | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` | step_finish tokens: input=신규분, cache.read·write 분리 확인. result_metrics도 동일 분리 구조 |
| codex-gpt54 | `input_tokens` (그대로) | turn.completed usage: input_tokens=캐시포함 합산(예: 190248), cached_input_tokens=그 중 캐시분(106752). 재합산 시 이중계산 → 불적용 |

> 보정 후 tok_in = 전 런타임 공통 '캐시 포함 총 입력 토큰'. codex 190k와 claude ~186k는 동일 기준으로 직접 비교 가능.

## 1. 품질 (codebase × runtime × backend)

- denominator: 166 valid episodes (harness_valid && !timed_out, 180 기준). 기존 153 셀 quality_stats: integrity_audit.quality_stats 동결값 사용(점수·valid/invalid 불변). 신규 opencode-serena 셀: scored_episodes.180.json의 score 필드에서 직접 재계산.
- paired_delta: paired_delta = mean(backend cell) − mean(no-mcp cell), 같은 codebase×runtime 내. round 끼리 매칭하지 않음(round 는 독립 시도). [v4] opencode-serena는 no-mcp 셀이 없어 paired-delta 산출 불가 → pairedDelta=null, winOrTie=null.
- win/tie 밴드: 동률 밴드 = task fact band (±0.25 ClickHouse, ±0.125 deno/angular). 그보다 좁은 차이는 n=3 noise.
- [v3] codex caveat: [v3 교정] codex-gpt54의 backend_exercised=false는 텔레메트리 버그(extractCodexOutput이 toolEvents:[]를 하드코딩)로 인한 오기록이었음. stdout item.completed/mcp_tool_call 파싱 결과 codex MCP arm 전수(codegraph 9, codemap 9, serena 9 = 27 에피소드) backend_exercised=true로 교정. 따라서 codex도 사용가능한(usable) 두 번째 비교다 — backend별 입자도: codemap/codegraph usable, serena degraded(일부 에피소드 serena 호출 에러 발생). 단 codex는 read-only OS sandbox이므로 mutating bash가 없고 실행환경이 claude(mutating bash)와 본질적으로 다르다(런타임 confound). 이는 텔레메트리 버그와 무관하게 유효한 한계이며 claude와 동급의 clean 비교라고 과장해서는 안 된다.

> **[v3 헤드라인]** codex는 이제 MCP를 실제로 호출했으므로 **무의미(null)가 아닌 사용가능(usable)한 두 번째 비교**다. 단 backend별 입자도: codemap/codegraph = usable, serena = degraded(일부 에러). claude와 동급의 clean 비교는 아님 — 런타임 confound 유지.

### ClickHouse-master  (task: .agents/orchestration/cms-dataset-hardening-v3-redesign-targetroot-20260618/phases/ClickHouse-master/round-3/public_question.md)

| runtime | model | backend | round_scores | mean | median | IQR | SE | stdev | valid_n | off | inval | fail | Δ vs no-mcp | win/tie | confound | notable misses |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| claude-sonnet | sonnet | no-mcp | 0.75, 0.375, 0.25 | 0.4583 | 0.375 | 0.25 | 0.1227 | 0.2125 | 3 | 0 | 0 | 0 | — | — | — | F3(abs×3) F2(abs×2) F4(abs×1,par×1) |
| claude-sonnet | sonnet | codemap | 0.5, 0.75, 0.375 | 0.5417 | 0.5 | 0.1875 | 0.09 | 0.1559 | 3 | 0 | 0 | 0 | +0.0834 | tie | — | F3(abs×3) F2(abs×2) F4(par×1) |
| claude-sonnet | sonnet | codegraph | 0.75, 0.375, 0.75 | 0.625 | 0.75 | 0.1875 | 0.1021 | 0.1768 | 3 | 0 | 0 | 0 | +0.1667 | tie | — | F3(abs×3) F2(abs×1) F4(par×1) |
| claude-sonnet | sonnet | serena | 0.625, 0.75, 1 | 0.7917 | 0.75 | 0.1875 | 0.09 | 0.1559 | 3 | 0 | 0 | 0 | +0.3334 | win | — | F3(abs×2) F2(par×1) |
| codex-gpt54 | gpt-5.4 | no-mcp | 0.625, 0.75, 0.625 | 0.6667 | 0.625 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | — | — | sandbox | F3(abs×3) F2(par×2) |
| codex-gpt54 | gpt-5.4 | codemap | 0.75, 0.625, 0.625 | 0.6667 | 0.625 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | 0 | tie | sandbox | F3(abs×3) F2(par×2) |
| codex-gpt54 | gpt-5.4 | codegraph | 0.875, 0.75, 0.75 | 0.7917 | 0.75 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | +0.125 | tie | sandbox | F3(abs×2) F2(par×1) |
| codex-gpt54 | gpt-5.4 | serena | 1, 0.75, 0.75 | 0.8333 | 0.75 | 0.125 | 0.068 | 0.1179 | 3 | 0 | 0 | 0 | +0.1666 | tie | sandbox+degraded | F3(abs×2) |
| opencode-deepseek | deepseek-v4-flash | no-mcp | 0, 0, 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | — | — | — | F1(abs×3) F2(abs×3) F3(abs×3) |
| opencode-deepseek | deepseek-v4-flash | codemap | 1, 0.375, 0.5 | 0.625 | 0.5 | 0.3125 | 0.1559 | 0.27 | 3 | 0 | 0 | 0 | +0.625 | win | — | F3(abs×2) F4(abs×1,par×1) F2(abs×1) |
| opencode-deepseek | deepseek-v4-flash | codegraph | 0, 0 | 0 | 0 | 0 | 0 | 0 | 2 | 1 | 1 | 1 | 0 ⚠partial(1/2) | tie | — | F1(abs×2) F2(abs×2) F3(abs×2) |
| opencode-deepseek | deepseek-v4-flash | serena | 0.375, 0, 0 | 0.125 | 0 | 0.375 | 0.1021 | 0.1768 | 3 | 2 | 0 | 0 | +0.125 ⚠partial(2/3) | tie | — | — |
| opencode-mimo | mimo-v2.5 | no-mcp | 0.75, 0 | 0.375 | 0.375 | 0.375 | 0.2652 | 0.375 | 2 | 0 | 1 | 1 | — | — | — | F3(abs×2) F1(abs×1) F2(abs×1) |
| opencode-mimo | mimo-v2.5 | codemap | 1, 0.125 | 0.5625 | 0.5625 | 0.4375 | 0.3094 | 0.4375 | 2 | 1 | 1 | 1 | +0.1875 ⚠partial(1/2) | tie | — | F2(abs×1) F3(abs×1) F4(abs×1) |
| opencode-mimo | mimo-v2.5 | codegraph | 1, 0.125, 0.125 | 0.4167 | 0.125 | 0.4375 | 0.2381 | 0.4125 | 3 | 2 | 0 | 0 | +0.0417 ⚠partial(2/3) | tie | — | F2(abs×2) F3(abs×2) F1(abs×1,par×1) |
| opencode-mimo | mimo-v2.5 | serena | 0, 0, 0 | 0 | 0 | 0 | 0 | 0 | 3 | 2 | 0 | 0 | -0.375 ⚠partial(2/3) | loss | — | — |
| opencode-minimax | minimax-m2.7 | no-mcp | 1, 0.75, 0.625 | 0.7917 | 0.75 | 0.1875 | 0.09 | 0.1559 | 3 | 0 | 0 | 0 | — | — | — | F3(abs×2) F2(par×1) |
| opencode-minimax | minimax-m2.7 | codemap | 0.875, 0.625, 0.125 | 0.5417 | 0.625 | 0.375 | 0.18 | 0.3118 | 3 | 0 | 0 | 0 | -0.25 | tie | — | F3(abs×2) F2(abs×1,par×1) F4(par×2) |
| opencode-minimax | minimax-m2.7 | codegraph | 0.125, 0.125, 0 | 0.0833 | 0.125 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | -0.7084 | loss | — | F2(abs×3) F3(abs×3) F4(abs×3) |
| opencode-minimax | minimax-m2.7 | serena | 0, 0 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 1 | 1 | -0.7917 | loss | — | — |

### deno-main  (task: .agents/orchestration/cms-dataset-hardening-v3-redesign-targetroot-20260618/phases/deno-main-retry-1/public_question.md)

| runtime | model | backend | round_scores | mean | median | IQR | SE | stdev | valid_n | off | inval | fail | Δ vs no-mcp | win/tie | confound | notable misses |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| claude-sonnet | sonnet | no-mcp | 0.1875, 0.1875, 0.1875 | 0.1875 | 0.1875 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | — | — | — | F1(abs×3) F5(abs×3) F6(abs×3) |
| claude-sonnet | sonnet | codemap | 0.9375, 0.8125, 0.25 | 0.6667 | 0.8125 | 0.3438 | 0.1726 | 0.299 | 3 | 0 | 0 | 0 | +0.4792 | win | — | F8(abs×1,par×2) F6(abs×1,par×1) F1(abs×1) |
| claude-sonnet | sonnet | codegraph | 0.125, 0.25, 0.1875 | 0.1875 | 0.1875 | 0.0625 | 0.0295 | 0.051 | 3 | 0 | 0 | 0 | 0 | tie | — | F1(abs×3) F5(abs×3) F8(abs×3) |
| claude-sonnet | sonnet | serena | 0.3125, 0.8125, 0.1875 | 0.4375 | 0.3125 | 0.3125 | 0.1559 | 0.27 | 3 | 0 | 0 | 0 | +0.25 | win | — | F8(abs×3) F6(abs×2,par×1) F1(abs×2) |
| codex-gpt54 | gpt-5.4 | no-mcp | 0.6875, 0.8125, 0.6875 | 0.7292 | 0.6875 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | — | — | sandbox | F8(abs×3) F6(abs×2) F4(par×2) |
| codex-gpt54 | gpt-5.4 | codemap | 0.8125, 0.6875, 0.8125 | 0.7708 | 0.8125 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | +0.0416 | tie | sandbox | F8(abs×3) F6(abs×1,par×2) F5(par×1) |
| codex-gpt54 | gpt-5.4 | codegraph | 0.1875, 0.3125, 0.375 | 0.2917 | 0.3125 | 0.0938 | 0.045 | 0.078 | 3 | 0 | 0 | 0 | -0.4375 | loss | sandbox | F5(abs×3) F6(abs×3) F8(abs×3) |
| codex-gpt54 | gpt-5.4 | serena | 0.375, 0.8125, 0.25 | 0.4792 | 0.375 | 0.2813 | 0.1392 | 0.2412 | 3 | 0 | 0 | 0 | -0.25 | loss | sandbox+degraded | F8(abs×3) F6(abs×2,par×1) F5(abs×2) |
| opencode-deepseek | deepseek-v4-flash | no-mcp | 0.125, 0.1875, 0.1875 | 0.1667 | 0.1875 | 0.0313 | 0.017 | 0.0295 | 3 | 0 | 0 | 0 | — | — | — | F1(abs×3) F2(abs×3) F5(abs×3) |
| opencode-deepseek | deepseek-v4-flash | codemap | 0.375, 0.25 | 0.3125 | 0.3125 | 0.0625 | 0.0442 | 0.0625 | 2 | 0 | 1 | 1 | +0.1458 | win | — | F5(abs×2) F6(abs×2) F8(abs×2) |
| opencode-deepseek | deepseek-v4-flash | codegraph | 0.125, 0.0625 | 0.0938 | 0.0938 | 0.0313 | 0.0221 | 0.0313 | 2 | 1 | 1 | 1 | -0.0729 ⚠partial(1/2) | tie | — | F1(abs×2) F2(abs×2) F3(abs×2) |
| opencode-deepseek | deepseek-v4-flash | serena | 0.1875 | 0.1875 | 0.1875 | 0 | 0 | 0 | 1 | 0 | 2 | 1 | +0.0208 | tie | — | — |
| opencode-mimo | mimo-v2.5 | no-mcp | 0.1875, 0.1875 | 0.1875 | 0.1875 | 0 | 0 | 0 | 2 | 0 | 1 | 1 | — | — | — | F1(abs×2) F2(abs×2) F5(abs×2) |
| opencode-mimo | mimo-v2.5 | codemap | 0.1875, 0.1875, 0.1875 | 0.1875 | 0.1875 | 0 | 0 | 0 | 3 | 1 | 0 | 0 | 0 ⚠partial(1/3) | tie | — | F1(abs×3) F2(abs×3) F5(abs×3) |
| opencode-mimo | mimo-v2.5 | codegraph | 0.1875, 0.125, 0.125 | 0.1458 | 0.125 | 0.0313 | 0.017 | 0.0295 | 3 | 0 | 0 | 0 | -0.0417 | tie | — | F1(abs×3) F2(abs×3) F5(abs×3) |
| opencode-mimo | mimo-v2.5 | serena | 0.1875, 0.0625 | 0.125 | 0.125 | 0.125 | 0.0442 | 0.0625 | 2 | 0 | 1 | 0 | -0.0625 | tie | — | — |
| opencode-minimax | minimax-m2.7 | no-mcp | 0.6875, 0.1875, 0.1875 | 0.3542 | 0.1875 | 0.25 | 0.1361 | 0.2357 | 3 | 0 | 0 | 0 | — | — | — | F8(abs×3) F6(abs×2,par×1) F3(abs×1,par×2) |
| opencode-minimax | minimax-m2.7 | codemap | 0.1875, 0.1875, 0.5 | 0.2917 | 0.1875 | 0.1563 | 0.0851 | 0.1473 | 3 | 0 | 0 | 0 | -0.0625 | tie | — | F6(abs×3) F8(abs×3) F1(abs×2,par×1) |
| opencode-minimax | minimax-m2.7 | codegraph | 0.1875, 0.1875, 0.125 | 0.1667 | 0.1875 | 0.0313 | 0.017 | 0.0295 | 3 | 0 | 0 | 0 | -0.1875 | loss | — | F1(abs×3) F5(abs×3) F6(abs×3) |
| opencode-minimax | minimax-m2.7 | serena | 0.1875, 0.1875, 0.1875 | 0.1875 | 0.1875 | 0 | 0 | 0 | 3 | 1 | 0 | 0 | -0.1667 ⚠partial(1/3) | loss | — | — |

### angular-main  (task: .agents/orchestration/cms-dataset-hardening-v3-sequential-20260618/phases/angular-main/round-2/public_question.md)

| runtime | model | backend | round_scores | mean | median | IQR | SE | stdev | valid_n | off | inval | fail | Δ vs no-mcp | win/tie | confound | notable misses |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| claude-sonnet | sonnet | no-mcp | 0.6875, 0.75, 0.75 | 0.7292 | 0.75 | 0.0313 | 0.017 | 0.0295 | 3 | 0 | 0 | 0 | — | — | — | F7(abs×3) F5(par×3) F6(par×3) |
| claude-sonnet | sonnet | codemap | 0.75, 0.75, 0.75 | 0.75 | 0.75 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | +0.0208 | tie | — | F7(abs×3) F5(par×3) F6(par×3) |
| claude-sonnet | sonnet | codegraph | 0.75, 0.875, 0.6875 | 0.7708 | 0.75 | 0.0938 | 0.045 | 0.078 | 3 | 0 | 0 | 0 | +0.0416 | tie | — | F7(abs×2) F5(par×3) F6(par×3) |
| claude-sonnet | sonnet | serena | 0.625, 0.75, 0.75 | 0.7083 | 0.75 | 0.0625 | 0.034 | 0.0589 | 3 | 0 | 0 | 0 | -0.0209 | tie | — | F7(abs×3) F5(abs×1,par×2) F6(par×3) |
| codex-gpt54 | gpt-5.4 | no-mcp | 0.5625, 0.75, 0.75 | 0.6875 | 0.75 | 0.0938 | 0.051 | 0.0884 | 3 | 0 | 0 | 0 | — | — | sandbox | F7(abs×3) F5(abs×1,par×2) F6(par×3) |
| codex-gpt54 | gpt-5.4 | codemap | 0.6875, 0.5625, 0.625 | 0.625 | 0.625 | 0.0625 | 0.0295 | 0.051 | 3 | 0 | 0 | 0 | -0.0625 | tie | sandbox | F7(abs×3) F5(abs×1,par×2) F1(par×3) |
| codex-gpt54 | gpt-5.4 | codegraph | 0.6875, 0.5625, 0.625 | 0.625 | 0.625 | 0.0625 | 0.0295 | 0.051 | 3 | 0 | 0 | 0 | -0.0625 | tie | sandbox | F7(abs×3) F5(abs×2,par×1) F6(par×3) |
| codex-gpt54 | gpt-5.4 | serena | 0.6875, 0.6875, 0.6875 | 0.6875 | 0.6875 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | tie | sandbox+degraded | F7(abs×3) F1(par×3) F5(par×3) |
| opencode-deepseek | deepseek-v4-flash | no-mcp | 0.5, 0.125, 0.1875 | 0.2708 | 0.1875 | 0.1875 | 0.0947 | 0.164 | 3 | 0 | 0 | 0 | — | — | — | F5(abs×3) F7(abs×3) F3(abs×2,par×1) |
| opencode-deepseek | deepseek-v4-flash | codemap | 0.5, 0.5625, 0.6875 | 0.5833 | 0.5625 | 0.0938 | 0.045 | 0.078 | 3 | 0 | 0 | 0 | +0.3125 | win | — | F7(abs×3) F5(abs×2) F1(par×3) |
| opencode-deepseek | deepseek-v4-flash | codegraph | 0.5, 0.5 | 0.5 | 0.5 | 0 | 0 | 0 | 2 | 0 | 1 | 1 | +0.2292 | win | — | F5(abs×2) F7(abs×2) F1(par×2) |
| opencode-deepseek | deepseek-v4-flash | serena | 0.1875, 0.375, 0.1875 | 0.25 | 0.1875 | 0.1875 | 0.051 | 0.0884 | 3 | 2 | 0 | 0 | -0.0208 ⚠partial(2/3) | tie | — | — |
| opencode-mimo | mimo-v2.5 | no-mcp | 0.625, 0.6875, 0.5625 | 0.625 | 0.625 | 0.0625 | 0.0295 | 0.051 | 3 | 0 | 0 | 0 | — | — | — | F5(abs×3) F7(abs×2) F3(par×3) |
| opencode-mimo | mimo-v2.5 | codemap | 0.5, 0.5 | 0.5 | 0.5 | 0 | 0 | 0 | 2 | 1 | 1 | 1 | -0.125 ⚠partial(1/2) | tie | — | F5(abs×2) F7(abs×2) F1(par×2) |
| opencode-mimo | mimo-v2.5 | codegraph | 0.625, 0.875 | 0.75 | 0.75 | 0.125 | 0.0884 | 0.125 | 2 | 1 | 1 | 1 | +0.125 ⚠partial(1/2) | tie | — | F5(abs×1,par×1) F6(par×2) F7(abs×1) |
| opencode-mimo | mimo-v2.5 | serena | 0.125, 0.625 | 0.375 | 0.375 | 0.5 | 0.1768 | 0.25 | 2 | 1 | 1 | 1 | -0.25 ⚠partial(1/2) | loss | — | — |
| opencode-minimax | minimax-m2.7 | no-mcp | 0.5, 0.5, 0.5 | 0.5 | 0.5 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | — | — | — | F5(abs×3) F7(abs×3) F1(par×3) |
| opencode-minimax | minimax-m2.7 | codemap | 0.5625, 0.0625, 0.5625 | 0.3958 | 0.5625 | 0.25 | 0.1361 | 0.2357 | 3 | 1 | 0 | 0 | -0.1042 ⚠partial(1/3) | tie | — | F5(abs×3) F7(abs×3) F1(abs×1,par×2) |
| opencode-minimax | minimax-m2.7 | codegraph | 0.6875, 0.6875, 0.5 | 0.625 | 0.6875 | 0.0938 | 0.051 | 0.0884 | 3 | 0 | 0 | 0 | +0.125 | tie | — | F7(abs×3) F5(abs×1,par×2) F1(par×3) |
| opencode-minimax | minimax-m2.7 | serena | 0, 0.25, 0.1875 | 0.1458 | 0.1875 | 0.25 | 0.0613 | 0.1062 | 3 | 2 | 0 | 0 | -0.3542 ⚠partial(2/3) | loss | — | — |

> ⚠partial(k/n) = 비-codex cell 인데 backend_off>0: valid n 중 k episode 가 backend MCP 미호출(builtin-only)이라 이 paired_delta 도 부분적으로 builtin-only episode 를 반영한다. delta 를 순수 MCP 효과로 읽지 말 것.
> confound=sandbox: codex는 read-only OS sandbox(mutating bash 없음) — claude와 실행환경이 다름. sandbox+degraded: serena 호출 에러도 추가 발생.
> [v3] ⚠full (codex backend_off 전수)는 텔레메트리 버그로 인한 오기록이었음. v3에서 제거 — 관련 표 주석 참고.

## 2. 효율 (per-episode, 153 rows)

- denominator: 180 executed episodes (180 기준, invalid 14개 포함, answer_extraction_status로 flag). timeout/empty 행의 wall_time·tool_calls 는 절단(truncated)되었을 수 있으므로 평균 해석 시 제외 권장.
- 효율은 같은 runtime/model 안에서만 backend 4종을 비교한다. token 은 tool_calls 와 묶어 해석.
- **tok_in 보정**: claude/opencode = input+cache_read+cache_creation(캐시포함 총입력); codex = input_tokens 그대로(이미 캐시포함). 보정 후 전 런타임 동일 기준 비교 가능.
- cache_tokens: claude/opencode = cache_read + cache_creation, codex = cached_input.
- **tool_call_breakdown**: 모든 도구 실행 흔적. ToolSearch(메타도구)는 tool_search_cnt 열로 분리. codex 는 scored_episodes 추출기 갭으로 distribution {} → stdout 재구성(tool_src=rebuilt). codegraph·serena·codemap 는 도구별 괄호 분해.

> **[본문 경고] opencode task 내부 도구 과소집계**: ⚠ opencode가 task 서브에이전트로 위임한 내부 도구 호출은 부모 세션에 집계되지 않는다(task(N)[inner_untracked]). 이는 codex 텔레메트리 버그와 같은 계열의 실사용 도구 과소집계다. task로 위임하는 opencode 행은 실제보다 도구 수가 적게 보인다. 도구 수 기반 효율 비교 시 opencode의 과소집계를 반드시 감안해야 한다. opencode-serena 신규 27 에피소드에도 동일하게 적용 — task 위임 내부 serena 호출은 미집계.

| codebase | runtime | model | backend | rnd | wall_s | tools | ts_cnt | bk_calls | tok_in | tok_out | cache_tok | tool_breakdown | tool_src | extract |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ClickHouse-master | claude-sonnet | sonnet | no-mcp | 1 | 114.757 | 9 | 0 | 0 | 138101 | 3738 | 138093 | Grep(5), Read(3), Glob(1) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | no-mcp | 2 | 166.024 | 15 | 0 | 0 | 300269 | 6195 | 300258 | Read(9), Grep(6) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | no-mcp | 3 | 297.657 | 19 | 0 | 0 | 412879 | 12850 | 412865 | Read(8), Grep(6), Glob(5) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | codemap | 1 | 123.066 | 8 | 1 | 4 | 136869 | 5862 | 136858 | codemap:search(3), codemap:grep(1), Read(4) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | codemap | 2 | 144.275 | 9 | 1 | 5 | 184529 | 5444 | 184517 | codemap:search(3), codemap:grep(2), Read(4) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | codemap | 3 | 220.776 | 16 | 1 | 6 | 411697 | 10784 | 411683 | codemap:search(5), codemap:overview(1), Read(10) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | codegraph | 1 | 173.231 | 6 | 1 | 3 | 186243 | 7195 | 186233 | explore(3), Read(3) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | codegraph | 2 | 264.215 | 7 | 1 | 5 | 317805 | 12843 | 317791 | explore(5), Read(2) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | codegraph | 3 | 92.174 | 2 | 1 | 2 | 71436 | 3080 | 71429 | explore(2) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | serena | 1 | 161.659 | 18 | 3 | 7 | 375512 | 6885 | 375490 | serena:search_for_pattern(7), Read(11) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | serena | 2 | 162.725 | 7 | 3 | 4 | 161590 | 6842 | 161575 | serena:search_for_pattern(4), Read(3) | ep | success |
| ClickHouse-master | claude-sonnet | sonnet | serena | 3 | 158.761 | 8 | 3 | 4 | 204955 | 7280 | 204938 | serena:search_for_pattern(4), Read(4) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | no-mcp | 1 | 81.526 | 17 | 0 | 0 | 364971 | 3168 | 310784 | shell(17) | rebuilt | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | no-mcp | 2 | 149.061 | 13 | 0 | 0 | 369107 | 2895 | 302720 | shell(13) | rebuilt | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | no-mcp | 3 | 87.378 | 14 | 0 | 0 | 359072 | 2960 | 290944 | shell(14) | rebuilt | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codemap | 1 | 80.249 | 14 | 0 | 0 | 230993 | 2871 | 167808 | codemap:read(9), codemap:grep(3), codemap:search(1), codemap:overview(1) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codemap | 2 | 67.572 | 11 | 0 | 0 | 173489 | 1941 | 116608 | codemap:read(6), codemap:grep(3), codemap:search(2) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codemap | 3 | 115.665 | 21 | 0 | 0 | 419615 | 2735 | 354688 | codemap:grep(12), codemap:read(9) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codegraph | 1 | 86.154 | 13 | 0 | 0 | 190248 | 2353 | 106752 | search(7), explore(4), node(2) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codegraph | 2 | 58.394 | 12 | 0 | 0 | 232758 | 1719 | 146048 | search(7), explore(5) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codegraph | 3 | 44.116 | 9 | 0 | 0 | 127668 | 1642 | 96128 | search(4), node(3), explore(2) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | serena | 1 | 148.185 | 26 | 0 | 0 | 319607 | 3755 | 260736 | serena:search_for_pattern(21), serena:get_symbols_overview(3), serena:initial_instructions(1), serena:find_symbol(1) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | serena | 2 | 105.399 | 28 | 0 | 0 | 386997 | 4137 | 350976 | serena:search_for_pattern(21), serena:get_symbols_overview(3), serena:find_symbol(3), serena:initial_instructions(1) | ep | success |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | serena | 3 | 733.924 | 26 | 0 | 0 | 346208 | 4426 | 299008 | serena:search_for_pattern(11), serena:find_symbol(9), serena:get_symbols_overview(4), serena:initial_instructions(1), mcp__codex__list_mcp_resources(1) | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | no-mcp | 1 | 134.107 | 19 | 0 | 0 | 56858 | 521 | 3072 | Read(13), Glob(4), Grep(2) | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | no-mcp | 2 | 331.003 | 0 | 0 | 0 | 11503 | 467 | 5248 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | no-mcp | 3 | 171.62 | 1 | 0 | 0 | 12359 | 292 | 1664 | Read(1), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codemap | 1 | 209.851 | 40 | 0 | 22 | 49297 | 330 | 44800 | codemap:grep(8), codemap:read(6), codemap:find(5), codemap:search(2), codemap:overview(1), Read(18) | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codemap | 2 | 405.979 | 11 | 0 | 3 | 19523 | 226 | 3072 | codemap:find(2), codemap:overview(1), Read(8), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codemap | 3 | 136.76 | 23 | 0 | 23 | 56895 | 285 | 36864 | codemap:read(11), codemap:grep(7), codemap:search(4), codemap:overview(1) | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codegraph | 1 | 212.787 | 0 | 0 | 0 | 13112 | 216 | 9216 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codegraph | 2 | 178.468 | 20 | 0 | 17 | 62891 | 667 | 46592 | node(7), explore(6), search(4), Read(3) | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codegraph | 3 | 1800.769* | 7 | 0 | 7 | 29760 | 82 | 5504 | explore(4), search(2), node(1) | ep | timeout* |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | serena | 1 | 221.545 | 0 | 0 | 0 | 19972 | 671 | 1024 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | serena | 2 | 203.334 | 1 | 0 | 0 | 20923 | 420 | 1024 | Read(1), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | serena | 3 | 825.576 | 34 | 0 | 10 | 77226 | 591 | 73216 | serena:search_for_pattern(8), serena:find_symbol(2), Read(24), task(3)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | no-mcp | 1 | 393.512 | 14 | 0 | 0 | 30678 | 871 | 28992 | Read(10), Grep(3), Glob(1), task(2)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | no-mcp | 2 | 334.806 | 0 | 0 | 0 | 11839 | 698 | 8256 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | no-mcp | 3 | 374.723* | 18 | 0 | 0 | 37733 | 1 | 30720 | Grep(12), Read(6) | ep | empty* |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codemap | 1 | 164.335 | 0 | 0 | 0 | 13118 | 793 | 9280 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codemap | 2 | 1799.973* | 25 | 0 | 20 | 44461 | 82 | 27776 | codemap:grep(8), codemap:find(6), codemap:read(5), codemap:search(1), Read(5), task(1)[inner_untracked] | ep | timeout* |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codemap | 3 | 131.298 | 18 | 0 | 18 | 69376 | 921 | 62720 | codemap:search(15), codemap:read(2), codemap:find(1) | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codegraph | 1 | 203.323 | 5 | 0 | 0 | 23169 | 1099 | 16192 | Read(5), task(3)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codegraph | 2 | 195.088 | 7 | 0 | 2 | 33373 | 945 | 31872 | node(2), Read(5), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codegraph | 3 | 515.667 | 2 | 0 | 0 | 15467 | 658 | 9728 | Read(2), task(2)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | serena | 1 | 286.709 | 7 | 0 | 0 | 28949 | 709 | 28032 | Read(7), task(2)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | serena | 2 | 202.975 | 7 | 0 | 5 | 44318 | 817 | 44032 | serena:search_for_pattern(4), serena:initial_instructions(1), Read(2), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | serena | 3 | 168.043 | 7 | 0 | 0 | 33425 | 993 | 28224 | Read(7), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | no-mcp | 1 | 108.789 | 0 | 0 | 0 | 9457 | 309 | 7611 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | no-mcp | 2 | 107.686 | 0 | 0 | 0 | 9770 | 320 | 7611 | task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | no-mcp | 3 | 191.505 | 15 | 0 | 0 | 24220 | 310 | 21947 | Grep(10), Read(3), Glob(2), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codemap | 1 | 124.719 | 19 | 0 | 13 | 30730 | 418 | 28667 | codemap:grep(5), codemap:find(4), codemap:search(2), codemap:overview(1), codemap:read(1), Read(6) | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codemap | 2 | 113.999 | 13 | 0 | 11 | 49823 | 265 | 45691 | codemap:grep(8), codemap:search(1), codemap:find(1), codemap:read(1), Read(2) | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codemap | 3 | 639.424 | 33 | 0 | 14 | 49354 | 369 | 48827 | codemap:grep(12), codemap:read(2), Read(19), task(1)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codegraph | 1 | 158.636 | 13 | 0 | 13 | 70042 | 223 | 69883 | explore(11), search(2) | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codegraph | 2 | 567.458 | 34 | 0 | 24 | 92966 | 394 | 92283 | explore(9), node(9), search(6), Read(10), task(2)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codegraph | 3 | 147.936 | 14 | 0 | 14 | 42963 | 618 | 42555 | explore(6), node(5), search(3) | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | serena | 1 | 1800.026* | 3 | 0 | 0 | 31112 | 109 | 0 | Read(3), task(7)[inner_untracked] | ep | timeout* |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | serena | 2 | 537.206 | 15 | 0 | 5 | 30269 | 444 | 29115 | serena:search_for_pattern(5), Read(10), task(2)[inner_untracked] | ep | success |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | serena | 3 | 437.787 | 31 | 0 | 13 | 55640 | 122 | 43451 | serena:search_for_pattern(13), Read(18), task(1)[inner_untracked] | ep | success |
| deno-main | claude-sonnet | sonnet | no-mcp | 1 | 102.89 | 8 | 0 | 0 | 96164 | 5329 | 96158 | Read(4), Glob(3), Grep(1) | ep | success |
| deno-main | claude-sonnet | sonnet | no-mcp | 2 | 432.452 | 58 | 0 | 0 | 1873194 | 20449 | 1873151 | Bash(31), Read(23), Glob(4) | ep | success |
| deno-main | claude-sonnet | sonnet | no-mcp | 3 | 124.055 | 16 | 0 | 0 | 281594 | 5690 | 278599 | Read(9), Bash(4), Grep(2), Glob(1) | ep | success |
| deno-main | claude-sonnet | sonnet | codemap | 1 | 176.553 | 21 | 1 | 21 | 544750 | 8550 | 544728 | codemap:search(11), codemap:read(8), codemap:overview(2) | ep | success |
| deno-main | claude-sonnet | sonnet | codemap | 2 | 124.821 | 12 | 1 | 12 | 340510 | 5221 | 340496 | codemap:search(8), codemap:read(3), codemap:overview(1) | ep | success |
| deno-main | claude-sonnet | sonnet | codemap | 3 | 194.54 | 17 | 2 | 9 | 306913 | 9668 | 306894 | codemap:search(4), codemap:find(4), codemap:overview(1), Read(8) | ep | success |
| deno-main | claude-sonnet | sonnet | codegraph | 1 | 185.84 | 15 | 2 | 3 | 447673 | 7751 | 447655 | explore(3), Read(12) | ep | success |
| deno-main | claude-sonnet | sonnet | codegraph | 2 | 193.976 | 5 | 1 | 5 | 211437 | 9546 | 211427 | explore(5) | ep | success |
| deno-main | claude-sonnet | sonnet | codegraph | 3 | 132.805 | 5 | 1 | 3 | 135933 | 5860 | 135924 | explore(3), Read(2) | ep | success |
| deno-main | claude-sonnet | sonnet | serena | 1 | 311.608 | 26 | 3 | 14 | 731313 | 14896 | 731286 | serena:search_for_pattern(14), Read(12) | ep | success |
| deno-main | claude-sonnet | sonnet | serena | 2 | 123.29 | 9 | 3 | 4 | 171134 | 4707 | 171117 | serena:search_for_pattern(4), Read(5) | ep | success |
| deno-main | claude-sonnet | sonnet | serena | 3 | 221.865 | 23 | 5 | 8 | 479163 | 10173 | 479141 | serena:search_for_pattern(8), Read(15) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | no-mcp | 1 | 103.682 | 37 | 0 | 0 | 536216 | 4731 | 474752 | shell(37) | rebuilt | success |
| deno-main | codex-gpt54 | gpt-5.4 | no-mcp | 2 | 61.323 | 13 | 0 | 0 | 206169 | 2120 | 164608 | shell(13) | rebuilt | success |
| deno-main | codex-gpt54 | gpt-5.4 | no-mcp | 3 | 80.48 | 25 | 0 | 0 | 575079 | 3393 | 497536 | shell(25) | rebuilt | success |
| deno-main | codex-gpt54 | gpt-5.4 | codemap | 1 | 99.932 | 40 | 0 | 0 | 512765 | 3368 | 452480 | codemap:grep(19), codemap:read(16), codemap:search(2), codemap:find(2), codemap:overview(1) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | codemap | 2 | 102.103 | 40 | 0 | 0 | 662565 | 3832 | 583296 | codemap:grep(19), codemap:read(19), codemap:find(1), codemap:overview(1) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | codemap | 3 | 91.999 | 30 | 0 | 0 | 527474 | 3156 | 470272 | codemap:read(17), codemap:grep(10), codemap:search(1), codemap:overview(1), codemap:find(1) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | codegraph | 1 | 73.406 | 13 | 0 | 0 | 139210 | 3246 | 100224 | search(8), explore(3), node(2) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | codegraph | 2 | 123.601 | 27 | 0 | 0 | 628596 | 4368 | 521856 | search(16), node(9), explore(2) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | codegraph | 3 | 147.193 | 44 | 0 | 0 | 899619 | 5686 | 770048 | search(29), node(10), explore(5) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | serena | 1 | 1105.174 | 14 | 0 | 0 | 284903 | 12251 | 192384 | serena:search_for_pattern(8), serena:get_symbols_overview(3), serena:initial_instructions(1), mcp__codex__list_mcp_resources(1) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | serena | 2 | 101.345 | 19 | 0 | 0 | 417824 | 3066 | 383872 | serena:search_for_pattern(13), serena:find_symbol(3), serena:get_symbols_overview(2), serena:initial_instructions(1) | ep | success |
| deno-main | codex-gpt54 | gpt-5.4 | serena | 3 | 459.857 | 52 | 0 | 0 | 811428 | 7078 | 759552 | serena:search_for_pattern(36), serena:find_symbol(10), serena:get_symbols_overview(5), serena:initial_instructions(1) | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 1 | 412.855 | 30 | 0 | 0 | 57529 | 376 | 44928 | Read(20), Grep(7), Glob(3), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 2 | 1009.203 | 94 | 0 | 0 | 101803 | 518 | 81920 | Read(73), Glob(11), Grep(10), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 3 | 469.166 | 38 | 0 | 0 | 61698 | 401 | 60160 | Read(23), Grep(14), Glob(1), task(1)[inner_untracked] | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | codemap | 1 | 132.439 | 22 | 0 | 22 | 56147 | 594 | 49152 | codemap:read(11), codemap:grep(7), codemap:search(2), codemap:overview(1), codemap:find(1) | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | codemap | 2 | 1806.563* | 37 | 0 | 10 | 29111 | 112 | 9984 | codemap:find(4), codemap:grep(4), codemap:overview(1), codemap:read(1), Read(27) | ep | timeout* |
| deno-main | opencode-deepseek | deepseek-v4-flash | codemap | 3 | 279.403 | 45 | 0 | 29 | 73126 | 655 | 71680 | codemap:read(17), codemap:find(6), codemap:grep(3), codemap:search(2), codemap:overview(1), Read(16) | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | codegraph | 1 | 418.896 | 0 | 0 | 0 | 17157 | 487 | 4096 | task(2)[inner_untracked] | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | codegraph | 2 | 456.68 | 38 | 0 | 3 | 70113 | 373 | 5120 | explore(3), Read(35), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | codegraph | 3 | 1800.008* | 31 | 0 | 4 | 57580 | 150 | 57088 | explore(3), search(1), Read(27), task(2)[inner_untracked] | ep | timeout* |
| deno-main | opencode-deepseek | deepseek-v4-flash | serena | 1 | 565.782 | 38 | 0 | 7 | 68668 | 550 | 65024 | serena:search_for_pattern(7), Read(31), task(3)[inner_untracked] | ep | success |
| deno-main | opencode-deepseek | deepseek-v4-flash | serena | 2 | 418.612* | 6 | 0 | 1 | 37292 | 247 | 4096 | serena:search_for_pattern(1), Read(5), task(3)[inner_untracked] | ep | process_error* |
| deno-main | opencode-deepseek | deepseek-v4-flash | serena | 3 | 1799.985* | 27 | 0 | 2 | 52895 | 81 | 1920 | serena:search_for_pattern(2), Read(25), task(3)[inner_untracked] | ep | timeout* |
| deno-main | opencode-mimo | mimo-v2.5 | no-mcp | 1 | 195.926* | 17 | 0 | 0 | 28130 | 85 | 27968 | Read(14), Glob(3), task(1)[inner_untracked] | ep | empty* |
| deno-main | opencode-mimo | mimo-v2.5 | no-mcp | 2 | 221.628 | 24 | 0 | 0 | 37079 | 1004 | 35968 | Read(14), Grep(8), Glob(2), task(1)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | no-mcp | 3 | 221.524 | 0 | 0 | 0 | 14800 | 797 | 8256 | task(2)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | codemap | 1 | 642.443 | 40 | 0 | 12 | 72623 | 843 | 64704 | codemap:read(5), codemap:search(3), codemap:find(2), codemap:grep(2), Read(28), task(1)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | codemap | 2 | 213.33 | 0 | 0 | 0 | 16982 | 896 | 13056 | task(1)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | codemap | 3 | 330.508 | 7 | 0 | 3 | 20081 | 893 | 17024 | codemap:read(3), Read(4), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | codegraph | 1 | 607.05 | 44 | 0 | 4 | 59313 | 1165 | 58048 | node(2), explore(1), search(1), Read(40), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | codegraph | 2 | 374.402 | 66 | 0 | 2 | 98802 | 1316 | 91392 | explore(2), Read(64), task(1)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | codegraph | 3 | 646.561 | 65 | 0 | 7 | 101324 | 916 | 80192 | node(5), explore(2), Read(58), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | serena | 1 | 439.717 | 26 | 0 | 5 | 49415 | 665 | 48064 | serena:search_for_pattern(5), Read(21), task(2)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | serena | 2 | 509.136 | 28 | 0 | 5 | 53701 | 532 | 51072 | serena:search_for_pattern(4), serena:initial_instructions(1), Read(23), task(5)[inner_untracked] | ep | success |
| deno-main | opencode-mimo | mimo-v2.5 | serena | 3 | 100.913* | 53 | 0 | 4 | 57739 | 74 | 57280 | serena:search_for_pattern(4), Read(49) | ep | process_error* |
| deno-main | opencode-minimax | minimax-m2.7 | no-mcp | 1 | 329.767 | 29 | 0 | 0 | 28972 | 240 | 27771 | Read(21), Glob(6), Grep(2), task(1)[inner_untracked] | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | no-mcp | 2 | 345.385 | 0 | 0 | 0 | 9536 | 245 | 0 | task(1)[inner_untracked] | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | no-mcp | 3 | 110.007 | 21 | 0 | 0 | 22374 | 405 | 18811 | Read(16), Glob(4), Grep(1) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | codemap | 1 | 314.618 | 38 | 0 | 5 | 55267 | 276 | 54651 | codemap:find(3), codemap:overview(1), codemap:grep(1), Read(33), task(1)[inner_untracked] | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | codemap | 2 | 264.109 | 66 | 0 | 25 | 53376 | 414 | 46587 | codemap:grep(12), codemap:find(11), codemap:overview(2), Read(41) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | codemap | 3 | 139.317 | 27 | 0 | 26 | 77971 | 399 | 75707 | codemap:search(10), codemap:read(8), codemap:grep(6), codemap:overview(1), codemap:find(1), Read(1) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | codegraph | 1 | 162.107 | 14 | 0 | 14 | 74963 | 379 | 68224 | explore(10), node(4) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | codegraph | 2 | 235.009 | 40 | 0 | 5 | 74370 | 262 | 73467 | search(3), explore(2), Read(35) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | codegraph | 3 | 204.793 | 24 | 0 | 24 | 68985 | 406 | 68539 | node(13), explore(6), search(5) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | serena | 1 | 1334.346 | 30 | 0 | 3 | 59999 | 492 | 59579 | serena:search_for_pattern(3), Read(27), task(4)[inner_untracked] | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | serena | 2 | 240.414 | 53 | 0 | 13 | 64512 | 461 | 45691 | serena:search_for_pattern(13), Read(40) | ep | success |
| deno-main | opencode-minimax | minimax-m2.7 | serena | 3 | 194.06 | 35 | 0 | 0 | 59631 | 491 | 40763 | Read(35) | ep | success |
| angular-main | claude-sonnet | sonnet | no-mcp | 1 | 37.46 | 3 | 0 | 0 | 50508 | 1580 | 50504 | Glob(1), Grep(1), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | no-mcp | 2 | 69.729 | 3 | 0 | 0 | 50646 | 2036 | 50642 | Glob(1), Grep(1), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | no-mcp | 3 | 45.268 | 3 | 0 | 0 | 50710 | 1901 | 50706 | Grep(2), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | codemap | 1 | 96.511 | 4 | 1 | 4 | 90983 | 2485 | 90975 | codemap:search(2), codemap:read(2) | ep | success |
| angular-main | claude-sonnet | sonnet | codemap | 2 | 94.989 | 3 | 1 | 2 | 64169 | 2594 | 64162 | codemap:search(2), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | codemap | 3 | 90.243 | 3 | 1 | 2 | 67826 | 2822 | 67819 | codemap:search(2), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | codegraph | 1 | 63.969 | 2 | 1 | 1 | 61627 | 3023 | 61620 | explore(1), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | codegraph | 2 | 60.558 | 1 | 1 | 1 | 43924 | 2184 | 43918 | explore(1) | ep | success |
| angular-main | claude-sonnet | sonnet | codegraph | 3 | 45.42 | 1 | 1 | 1 | 43095 | 2073 | 43089 | explore(1) | ep | success |
| angular-main | claude-sonnet | sonnet | serena | 1 | 55.943 | 2 | 2 | 1 | 71521 | 2236 | 71511 | serena:search_for_pattern(1), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | serena | 2 | 62.626 | 3 | 3 | 2 | 92989 | 2641 | 92978 | serena:find_symbol(1), serena:search_for_pattern(1), Read(1) | ep | success |
| angular-main | claude-sonnet | sonnet | serena | 3 | 57.211 | 2 | 3 | 1 | 84577 | 2359 | 84564 | serena:search_for_pattern(1), Read(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | no-mcp | 1 | 41.158 | 7 | 0 | 0 | 72858 | 1837 | 58880 | shell(7) | rebuilt | success |
| angular-main | codex-gpt54 | gpt-5.4 | no-mcp | 2 | 57.998 | 8 | 0 | 0 | 153851 | 2037 | 116096 | shell(8) | rebuilt | success |
| angular-main | codex-gpt54 | gpt-5.4 | no-mcp | 3 | 48.963 | 10 | 0 | 0 | 165618 | 2100 | 137472 | shell(10) | rebuilt | success |
| angular-main | codex-gpt54 | gpt-5.4 | codemap | 1 | 53.535 | 5 | 0 | 0 | 55502 | 1355 | 43136 | codemap:grep(2), codemap:read(2), codemap:find(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | codemap | 2 | 58.458 | 8 | 0 | 0 | 86763 | 1437 | 61440 | codemap:read(4), codemap:grep(2), codemap:find(1), codemap:search(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | codemap | 3 | 51.839 | 8 | 0 | 0 | 95545 | 1746 | 75264 | codemap:read(5), codemap:search(1), codemap:grep(1), codemap:find(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | codegraph | 1 | 23.348 | 3 | 0 | 0 | 34399 | 925 | 17152 | search(2), explore(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | codegraph | 2 | 42.678 | 5 | 0 | 0 | 126807 | 1519 | 100224 | explore(2), node(2), callers(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | codegraph | 3 | 57.866 | 3 | 0 | 0 | 61651 | 1325 | 35968 | explore(2), search(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | serena | 1 | 60.737 | 11 | 0 | 0 | 125773 | 1861 | 98560 | serena:search_for_pattern(5), serena:find_symbol(3), serena:get_symbols_overview(2), serena:initial_instructions(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | serena | 2 | 71.238 | 20 | 0 | 0 | 178761 | 2693 | 135680 | serena:find_symbol(11), serena:search_for_pattern(6), serena:get_symbols_overview(2), serena:initial_instructions(1) | ep | success |
| angular-main | codex-gpt54 | gpt-5.4 | serena | 3 | 62.783 | 15 | 0 | 0 | 198556 | 2202 | 171776 | serena:find_symbol(7), serena:search_for_pattern(6), serena:initial_instructions(1), serena:get_symbols_overview(1) | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 1 | 194.429 | 0 | 0 | 0 | 13213 | 250 | 3328 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 2 | 51.132 | 0 | 0 | 0 | 9319 | 413 | 5120 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 3 | 96.207 | 0 | 0 | 0 | 9647 | 407 | 3328 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | codemap | 1 | 67.372 | 4 | 0 | 4 | 16185 | 369 | 5248 | codemap:grep(2), codemap:read(2) | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | codemap | 2 | 71.366 | 5 | 0 | 3 | 19918 | 283 | 1920 | codemap:search(3), Read(2) | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | codemap | 3 | 132.103 | 27 | 0 | 4 | 50404 | 374 | 33792 | codemap:search(2), codemap:grep(2), Read(23) | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | codegraph | 1 | 1799.952* | 0 | 0 | 0 | 9247 | 127 | 5120 | — | ep | timeout* |
| angular-main | opencode-deepseek | deepseek-v4-flash | codegraph | 2 | 28.109 | 5 | 0 | 5 | 18046 | 399 | 9216 | search(3), node(2) | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | codegraph | 3 | 40.875 | 6 | 0 | 6 | 20860 | 476 | 16000 | search(3), node(2), explore(1) | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | serena | 1 | 145.776 | 6 | 0 | 0 | 23538 | 298 | 17024 | Read(6), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | serena | 2 | 182.5 | 0 | 0 | 0 | 13891 | 405 | 1024 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-deepseek | deepseek-v4-flash | serena | 3 | 239.153 | 1 | 0 | 1 | 15904 | 491 | 1024 | serena:search_for_pattern(1), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | no-mcp | 1 | 138.561 | 3 | 0 | 0 | 14308 | 630 | 13696 | Read(3), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | no-mcp | 2 | 147.787 | 0 | 0 | 0 | 13076 | 523 | 8256 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | no-mcp | 3 | 246.882 | 5 | 0 | 0 | 15298 | 757 | 9856 | Glob(3), Read(2), task(2)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | codemap | 1 | 1799.971* | 0 | 0 | 0 | — | — | — | — | ep | timeout* |
| angular-main | opencode-mimo | mimo-v2.5 | codemap | 2 | 178.544 | 1 | 0 | 0 | 16682 | 623 | 12992 | Read(1), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | codemap | 3 | 42.222 | 7 | 0 | 7 | 29631 | 855 | 23232 | codemap:search(5), codemap:find(1), codemap:read(1) | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | codegraph | 1 | 48.962 | 9 | 0 | 9 | 32548 | 913 | 31104 | search(4), node(3), explore(2) | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | codegraph | 2 | 1799.945* | 0 | 0 | 0 | — | — | — | — | ep | timeout* |
| angular-main | opencode-mimo | mimo-v2.5 | codegraph | 3 | 274.713 | 0 | 0 | 0 | 13959 | 988 | 9344 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | serena | 1 | 92.848 | 0 | 0 | 0 | 15372 | 724 | 14400 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | serena | 2 | 194.243 | 18 | 0 | 14 | 36237 | 859 | 35136 | serena:search_for_pattern(9), serena_read_memory(2), serena_find_declaration(2), serena:get_symbols_overview(1), Read(4), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-mimo | mimo-v2.5 | serena | 3 | 1802.202* | 0 | 0 | 0 | — | — | — | — | ep | timeout* |
| angular-main | opencode-minimax | minimax-m2.7 | no-mcp | 1 | 112.428 | 11 | 0 | 0 | 22704 | 380 | 21499 | Grep(5), Read(4), Glob(2) | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | no-mcp | 2 | 186.492 | 3 | 0 | 0 | 12461 | 268 | 11643 | Read(3), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | no-mcp | 3 | 173.399 | 1 | 0 | 0 | 13556 | 342 | 9851 | Read(1), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | codemap | 1 | 66.486 | 7 | 0 | 7 | 16663 | 610 | 15663 | codemap:read(4), codemap:grep(2), codemap:search(1) | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | codemap | 2 | 120.874 | 0 | 0 | 0 | 10252 | 546 | 1339 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | codemap | 3 | 39.648 | 5 | 0 | 5 | 16866 | 276 | 16123 | codemap:grep(2), codemap:read(2), codemap:search(1) | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | codegraph | 1 | 190.29 | 4 | 0 | 3 | 17089 | 504 | 1339 | search(2), explore(1), Read(1), task(1)[inner_untracked] | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | codegraph | 2 | 151.701 | 23 | 0 | 6 | 32679 | 396 | 25344 | search(4), node(1), explore(1), Read(17) | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | codegraph | 3 | 33.216 | 2 | 0 | 2 | 21342 | 280 | 14976 | explore(2) | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | serena | 1 | 92.906 | 14 | 0 | 4 | 30077 | 117 | 30011 | serena:search_for_pattern(4), Read(10) | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | serena | 2 | 184.188 | 0 | 0 | 0 | 13779 | 983 | 12479 | task(1)[inner_untracked] | ep | success |
| angular-main | opencode-minimax | minimax-m2.7 | serena | 3 | 336.19 | 0 | 0 | 0 | 13924 | 449 | 0 | task(1)[inner_untracked] | ep | success |

> `*` = harness invalid (timeout/extraction_empty). 해당 행의 wall_time/tool_calls 는 미완(절단) 가능성이 있어 효율 평균 산출에서 제외 권장.
> tool_src: ep=scored_episodes, rebuilt=codex stdout 재구성(item.completed/mcp_tool_call·command_execution 이벤트 집계).
> ts_cnt=ToolSearch(스키마 로딩 메타도구, tools 합계 제외). invalid/skill/todowrite 도 제외.
> task(N)[inner_untracked]: opencode 서브에이전트 스폰 도구. N=스폰 횟수. 서브에이전트 내부 도구 호출은 parent에 기록되지 않아 추적 불가. tools 합계에도 미포함(측정 불가 명시).
> codegraph explore 인자는 {query:...} 뿐 mode/kind 없음 → operation 재분해 불가. 도구명 단위 분해(explore/node/search/callers). 단일 codegraph 콜이 serena 여러 콜의 일을 하므로 콜수 직접 비교 시 주의.

## 3. 행동 프로파일 (codebase × runtime × backend, valid 평균)

| codebase | runtime | model | backend | valid_n | search | nav | read | grep | shell/other | bk_bytes | bk_on | bk_off | notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ClickHouse-master | claude-sonnet | sonnet | no-mcp | 3 | 0 | 0 | 6.6667 | 5.6667 | 2 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| ClickHouse-master | claude-sonnet | sonnet | codemap | 3 | 3.6667 | 0.3333 | 6 | 1 | 1 | 56954.6667 | 3 | 0 | backend 전수 호출 (on 3/3) |
| ClickHouse-master | claude-sonnet | sonnet | codegraph | 3 | 0 | 3.3333 | 1.6667 | 0 | 1 | 81240.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| ClickHouse-master | claude-sonnet | sonnet | serena | 3 | 5 | 0 | 6 | 0 | 3 | 10953.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | no-mcp | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) ※read-only sandbox confound |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codemap | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) ※read-only sandbox confound |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | codegraph | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) ※read-only sandbox confound |
| ClickHouse-master | codex-gpt54 | gpt-5.4 | serena | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) [degraded: serena 에러 다수] |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | no-mcp | 3 | 0 | 0 | 4.6667 | 0.6667 | 2 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codemap | 3 | 2 | 1 | 14.3333 | 5 | 4.3333 | 93705 | 3 | 0 | backend 전수 호출 (on 3/3) |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | codegraph | 2 | 2 | 6.5 | 1.5 | 0 | 0.5 | 85539 | 1 | 1 | backend 일부 미호출 (off 1/2) |
| ClickHouse-master | opencode-deepseek | deepseek-v4-flash | serena | 3 | 2.6667 | 0.6667 | 8.3333 | 0 | 2.6667 | 166 | 1 | 2 | backend 일부 미호출 (off 2/3) |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | no-mcp | 2 | 0 | 0 | 5 | 1.5 | 2 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codemap | 2 | 7.5 | 0 | 1 | 0 | 1 | 84821.5 | 1 | 1 | backend 일부 미호출 (off 1/2) |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | codegraph | 3 | 0 | 0.6667 | 4 | 0 | 2 | 111 | 1 | 2 | backend 일부 미호출 (off 2/3) |
| ClickHouse-master | opencode-mimo | mimo-v2.5 | serena | 3 | 1.3333 | 0.3333 | 5.3333 | 0 | 1.3333 | 15021.3333 | 1 | 2 | backend 일부 미호출 (off 2/3) |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | no-mcp | 3 | 0 | 0 | 1 | 3.3333 | 1.6667 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codemap | 3 | 1 | 0.3333 | 10.3333 | 8.3333 | 2 | 95449 | 3 | 0 | backend 전수 호출 (on 3/3) |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | codegraph | 3 | 3.6667 | 13.3333 | 3.3333 | 0 | 0.6667 | 220329.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| ClickHouse-master | opencode-minimax | minimax-m2.7 | serena | 2 | 9 | 0 | 14 | 0 | 2 | 25040 | 2 | 0 | backend 전수 호출 (on 2/2) |
| deno-main | claude-sonnet | sonnet | no-mcp | 3 | 0 | 0 | 12 | 1 | 14.3333 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| deno-main | claude-sonnet | sonnet | codemap | 3 | 7.6667 | 1.3333 | 6.3333 | 0 | 2.6667 | 93189.6667 | 3 | 0 | backend 전수 호출 (on 3/3) |
| deno-main | claude-sonnet | sonnet | codegraph | 3 | 0 | 3.6667 | 4.6667 | 0 | 1.3333 | 85904.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| deno-main | claude-sonnet | sonnet | serena | 3 | 8.6667 | 0 | 10.6667 | 0 | 3.6667 | 25492 | 3 | 0 | backend 전수 호출 (on 3/3) |
| deno-main | codex-gpt54 | gpt-5.4 | no-mcp | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) ※read-only sandbox confound |
| deno-main | codex-gpt54 | gpt-5.4 | codemap | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) ※read-only sandbox confound |
| deno-main | codex-gpt54 | gpt-5.4 | codegraph | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) ※read-only sandbox confound |
| deno-main | codex-gpt54 | gpt-5.4 | serena | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) [degraded: serena 에러 다수] |
| deno-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 3 | 0 | 0 | 38.6667 | 10.3333 | 7.3333 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| deno-main | opencode-deepseek | deepseek-v4-flash | codemap | 2 | 2 | 1 | 22 | 5 | 3.5 | 406596.5 | 2 | 0 | backend 전수 호출 (on 2/2) |
| deno-main | opencode-deepseek | deepseek-v4-flash | codegraph | 2 | 0 | 1.5 | 17.5 | 0 | 2.5 | 36664.5 | 1 | 1 | backend 일부 미호출 (off 1/2) |
| deno-main | opencode-deepseek | deepseek-v4-flash | serena | 1 | 7 | 0 | 31 | 0 | 4 | 387 | 1 | 0 | backend 전수 호출 (on 1/1) |
| deno-main | opencode-mimo | mimo-v2.5 | no-mcp | 2 | 0 | 0 | 7 | 4 | 2.5 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| deno-main | opencode-mimo | mimo-v2.5 | codemap | 3 | 1 | 0 | 13.3333 | 0.6667 | 2.3333 | 3786.3333 | 2 | 1 | backend 일부 미호출 (off 1/3) |
| deno-main | opencode-mimo | mimo-v2.5 | codegraph | 3 | 0.3333 | 4 | 54 | 0 | 2.3333 | 38314.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| deno-main | opencode-mimo | mimo-v2.5 | serena | 2 | 4.5 | 0.5 | 22 | 0 | 4 | 3944.5 | 2 | 0 | backend 전수 호출 (on 2/2) |
| deno-main | opencode-minimax | minimax-m2.7 | no-mcp | 3 | 0 | 0 | 12.3333 | 1 | 4 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| deno-main | opencode-minimax | minimax-m2.7 | codemap | 3 | 3.3333 | 1.3333 | 27.6667 | 6.3333 | 5.3333 | 99395.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| deno-main | opencode-minimax | minimax-m2.7 | codegraph | 3 | 2.6667 | 11.6667 | 11.6667 | 0 | 0 | 167284.6667 | 3 | 0 | backend 전수 호출 (on 3/3) |
| deno-main | opencode-minimax | minimax-m2.7 | serena | 3 | 5.3333 | 0 | 34 | 0 | 1.6667 | 23439 | 2 | 1 | backend 일부 미호출 (off 1/3) |
| angular-main | claude-sonnet | sonnet | no-mcp | 3 | 0 | 0 | 1 | 1.3333 | 0.6667 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| angular-main | claude-sonnet | sonnet | codemap | 3 | 2 | 0 | 1.3333 | 0 | 1 | 26366 | 3 | 0 | backend 전수 호출 (on 3/3) |
| angular-main | claude-sonnet | sonnet | codegraph | 3 | 0 | 1 | 0.3333 | 0 | 1 | 23049 | 3 | 0 | backend 전수 호출 (on 3/3) |
| angular-main | claude-sonnet | sonnet | serena | 3 | 1.3333 | 0 | 1 | 0 | 2.6667 | 6084.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| angular-main | codex-gpt54 | gpt-5.4 | no-mcp | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) ※read-only sandbox confound |
| angular-main | codex-gpt54 | gpt-5.4 | codemap | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) ※read-only sandbox confound |
| angular-main | codex-gpt54 | gpt-5.4 | codegraph | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) ※read-only sandbox confound |
| angular-main | codex-gpt54 | gpt-5.4 | serena | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | backend 전수 호출 (on 3/3) [degraded: serena 에러 다수] |
| angular-main | opencode-deepseek | deepseek-v4-flash | no-mcp | 3 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| angular-main | opencode-deepseek | deepseek-v4-flash | codemap | 3 | 1.6667 | 0 | 9 | 1.3333 | 0 | 20249 | 3 | 0 | backend 전수 호출 (on 3/3) |
| angular-main | opencode-deepseek | deepseek-v4-flash | codegraph | 2 | 3 | 2.5 | 0 | 0 | 0 | 35367 | 2 | 0 | backend 전수 호출 (on 2/2) |
| angular-main | opencode-deepseek | deepseek-v4-flash | serena | 3 | 0.3333 | 0 | 2 | 0 | 1.3333 | 5 | 1 | 2 | backend 일부 미호출 (off 2/3) |
| angular-main | opencode-mimo | mimo-v2.5 | no-mcp | 3 | 0 | 0 | 1.6667 | 0 | 2.3333 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| angular-main | opencode-mimo | mimo-v2.5 | codemap | 2 | 2.5 | 0 | 1 | 0 | 1 | 32234 | 1 | 1 | backend 일부 미호출 (off 1/2) |
| angular-main | opencode-mimo | mimo-v2.5 | codegraph | 2 | 2 | 2.5 | 0 | 0 | 0.5 | 39700.5 | 1 | 1 | backend 일부 미호출 (off 1/2) |
| angular-main | opencode-mimo | mimo-v2.5 | serena | 2 | 4.5 | 1.5 | 2 | 0 | 2 | 29825 | 1 | 1 | backend 일부 미호출 (off 1/2) |
| angular-main | opencode-minimax | minimax-m2.7 | no-mcp | 3 | 0 | 0 | 2.6667 | 1.6667 | 1.3333 | 0 | 0 | 0 | no-mcp: backend 없음(builtin only) |
| angular-main | opencode-minimax | minimax-m2.7 | codemap | 3 | 0.6667 | 0 | 2 | 1.3333 | 0.3333 | 18500.3333 | 2 | 1 | backend 일부 미호출 (off 1/3) |
| angular-main | opencode-minimax | minimax-m2.7 | codegraph | 3 | 2 | 1.6667 | 6 | 0 | 0.3333 | 34595.3333 | 3 | 0 | backend 전수 호출 (on 3/3) |
| angular-main | opencode-minimax | minimax-m2.7 | serena | 3 | 1.3333 | 0 | 3.3333 | 0 | 0.6667 | 15483.3333 | 1 | 2 | backend 일부 미호출 (off 2/3) |

> backend_tool_bytes 는 backend MCP 가 반환한 바이트의 cell 평균. no-mcp 는 0(backend 없음). off=valid 중 backend 미호출 episode 수.
> read_bytes(§12 열): scored_episodes 에 미수록 → 산출 불가. backend_tool_bytes 만 제공한다(누락을 숨기지 않고 명시).

## 4. 도입비용 / readiness (backend × codebase — index 비용은 runtime 간 공유)

- index_build_time_s / index_disk_size 는 backend×codebase 단위이며 같은 cell 을 쓰는 모든 runtime 이 공유한다(arm 별 중복 부담 아님).
- opencode×serena 9 cell 은 scored_episodes 에 없는 skip row(backend_unsupported_transport)로 여기에만 기록한다.

| backend | codebase | runtime | readiness | index/cache path | build_s | build_type | disk | config_req | manual | writes(warmup/after) | mutation_guard | skipped_reason |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| no-mcp | ClickHouse-master | (shared) | ready | — | — | — | — | false | false | 0/0 | clean | — |
| no-mcp | deno-main | (shared) | ready | — | — | — | — | false | false | 0/0 | clean | — |
| no-mcp | angular-main | (shared) | ready | — | — | — | — | false | false | 0/0 | clean | — |
| codemap | ClickHouse-master | (shared) | ready | …/ClickHouse-master/.codemap | — | — | 26M | true | false | 2/0 | clean - config.toml 사전존재, baseline 이후 추가 쓰기 없음 | — |
| codemap | deno-main | (shared) | ready | …/deno-main/.codemap | — | — | 9.7M | true | false | 2/0 | clean - config.toml 사전존재(이전 cheap-proof 해소). baseline 이후 추가 쓰기 없음 | — |
| codemap | angular-main | (shared) | ready | …/angular-main/.codemap | — | — | 31M | true | false | 2/0 | clean - config.toml 사전존재. baseline 이후 추가 쓰기 없음 | — |
| codegraph | ClickHouse-master | (shared) | ready | …/ClickHouse-master/.codegraph/codegraph.db | — | — | 454M | false | false | 1/0 | clean - .codegraph/ 사전존재. baseline 이후 추가 쓰기 없음 | — |
| codegraph | deno-main | (shared) | ready | …/deno-main/.codegraph/codegraph.db | — | — | 197M | false | false | 2/0 | clean | — |
| codegraph | angular-main | (shared) | ready | …/angular-main/.codegraph/codegraph.db | 80 | — | 291M | false | false | 1/0 | clean - warmup에서 신규 생성(허용). baseline snapshot은 이 이후. episode 중 추가 쓰기만 violation | — |
| serena | ClickHouse-master | (shared) | ready | …/ClickHouse-master/.serena/cache/cpp | 62 | cold_reindex_measured (~62s 실측) | 151M | true | false | 2/0 | clean - 재인덱싱은 warmup write(허용). baseline 이후 episode 중 .serena 쓰기만 violation | — |
| serena | deno-main | (shared) | ready | …/deno-main/.serena/cache/ | — | warm_cache_reuse (pkl 재검증 <1s; cold build 미측정) | 184M | true | false | 2/0 | clean | — |
| serena | angular-main | (shared) | ready | …/angular-main/.serena/cache/typescript | 41 | cold_index_measured (~41s 실측, 신규 생성) | 277M | true | false | 2/0 | clean - 신규 인덱싱은 warmup write(허용). baseline 이후 episode 중 쓰기만 violation | — |
| serena | ClickHouse-master | opencode-deepseek | executed_via_rewire | …/ClickHouse-master/.serena/cache/cpp | — | — | 151M | true | false | 0/0 | clean | — |
| serena | deno-main | opencode-deepseek | executed_via_rewire | …/deno-main/.serena/cache/ | — | — | 184M | true | false | 0/0 | clean | — |
| serena | angular-main | opencode-deepseek | executed_via_rewire | …/angular-main/.serena/cache/typescript | — | — | 277M | true | false | 0/0 | clean | — |
| serena | ClickHouse-master | opencode-mimo | executed_via_rewire | …/ClickHouse-master/.serena/cache/cpp | — | — | 151M | true | false | 0/0 | clean | — |
| serena | deno-main | opencode-mimo | executed_via_rewire | …/deno-main/.serena/cache/ | — | — | 184M | true | false | 0/0 | clean | — |
| serena | angular-main | opencode-mimo | executed_via_rewire | …/angular-main/.serena/cache/typescript | — | — | 277M | true | false | 0/0 | clean | — |
| serena | ClickHouse-master | opencode-minimax | executed_via_rewire | …/ClickHouse-master/.serena/cache/cpp | — | — | 151M | true | false | 0/0 | clean | — |
| serena | deno-main | opencode-minimax | executed_via_rewire | …/deno-main/.serena/cache/ | — | — | 184M | true | false | 0/0 | clean | — |
| serena | angular-main | opencode-minimax | executed_via_rewire | …/angular-main/.serena/cache/typescript | — | — | 277M | true | false | 0/0 | clean | — |

> serena build_s 실측: ClickHouse 62s(clangd cold reindex), angular 41s(tsserver cold), deno warm-cache(<1s, cold 미측정). codegraph angular 80s(cold init). codemap/codegraph 의 다른 cell 은 index 사전존재로 build_s 미측정. serena 디스크가 가장 큼(151–277M).

## 5. 무결성 요약

| 지표 | 값 |
|---|---|
| valid | 166 |
| invalid | 14 (timeout 10 + empty 2) |
| skipped (backend_unsupported_transport) | 0 |
| wrong_root_detected | 0 |
| out_of_repo_answer_detected | 0 |
| target_mutation_detected | 0 |
| scorer_version | 1.0 (모든 episode 동일) · formula match=true |

### invalid episode (9, 전부 opencode)

| runtime | backend | codebase | round | extraction | cause |
|---|---|---|---|---|---|
| opencode-deepseek | codegraph | angular-main | 1 | timeout | timeout |
| opencode-deepseek | codegraph | ClickHouse-master | 3 | timeout | timeout |
| opencode-deepseek | codegraph | deno-main | 3 | timeout | timeout |
| opencode-deepseek | codemap | deno-main | 2 | timeout | timeout |
| opencode-mimo | codegraph | angular-main | 2 | timeout | timeout |
| opencode-mimo | codemap | angular-main | 1 | timeout | timeout |
| opencode-mimo | codemap | ClickHouse-master | 2 | timeout | timeout |
| opencode-mimo | no-mcp | ClickHouse-master | 3 | empty | extraction_empty |
| opencode-mimo | no-mcp | deno-main | 1 | empty | extraction_empty |
| opencode-deepseek | serena | deno-main | 2 | process_error | extraction_empty |
| opencode-deepseek | serena | deno-main | 3 | timeout | timeout |
| opencode-mimo | serena | angular-main | 3 | timeout | timeout |
| opencode-mimo | serena | deno-main | 3 | process_error | extraction_empty |
| opencode-minimax | serena | ClickHouse-master | 1 | timeout | timeout |

> wrong_root / out_of_repo / target_mutation = 0 (전 episode). skip 27 = opencode×serena transport 미부팅(readiness 가 codex/claude transport 로만 검증 → 실행 시점에 드러난 known-untested transport).
