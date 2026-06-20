# 통합 보고서: cms-official-benchmark-20260619-03 / 153→180 재집계

생성일: 2026-06-20
담당: integrate phase sub-agent

---

## 1. 실행 요약 (153→180)

| 항목 | 이전(v3, 153기준) | 이후(v4, 180기준) | 변화 |
|---|---|---|---|
| executed_n | 153 | 180 | +27 (opencode-serena 실데이터) |
| skipped_n | 27 (synthetic) | 0 | synthetic row 제거 |
| quality_valid_n | 144 | 166 | +22 |
| harness_invalid_n | 9 | 14 | +5 (opencode-serena 5개 invalid) |
| timeout_n | 7 | 10 | +3 (opencode-serena 3개 timeout) |

---

## 2. 스키마 parity 검증

153 scored_episodes.json 스키마 필드(21개):
`arm_id, runtime, model_label, backend, codebase, round, score, per_fact_score, wall_time_s, tokens, tool_call_distribution, backend_tool_calls, tool_class_counts, backend_tool_bytes, backend_exercised, extraction_status, harness_valid, mutation_guard_status, timed_out, co_tenancy, answer_sha256`

신규 27개 result_metrics → 153 스키마 매핑 결과: **누락 필드 0, 추가 필드 0 (parity OK)**

매핑 규칙:
- `scorer_score` → `score` (필드명 변환)
- `per_fact_score`: result_metrics에 없음 → `null`
- `backend_tool_calls`: `tool_call_distribution`에서 serena 도구 호출 수 합산
- `tool_class_counts`: `tool_call_distribution`에서 도구 분류별 집계 유도
- `backend_tool_bytes`: `assigned_backend_tool_bytes` 필드에서 복사
- `timed_out`: `extraction_status === "timeout"` 유도 (result_metrics에 timed_out 필드 없음)

---

## 3. backend_off old→new

| 기준 | backend_off | 비고 |
|---|---|---|
| 153 audit 원본 | 39 | 텔레메트리 버그 파생값 포함 |
| 153 codex 교정 후 | 12 | codex MCP arm 27개 false→true 교정 |
| 신규 27 (opencode-serena) | 12/27 | backend_exercised=false: 12개 |
| **180 기준 최종** | **24** | 12(교정후 153기준) + 12(신규) |

신규 27 backend_off 상세 (backend_exercised=false):
- opencode-deepseek: 4/9개 미호출
- opencode-mimo: 4/9개 미호출
- opencode-minimax: 4/9개 미호출
- (코드베이스별: ClickHouse 5개, angular 6개, deno 1개)

---

## 4. opencode-serena 모델별·코드베이스별 평균

### 모델별 (valid episode 한정)

| runtime | model | valid/total | avg_score | backend_exercised |
|---|---|---|---|---|
| opencode-deepseek | deepseek-v4-flash | 7/9 | 0.1875 | 5/9 |
| opencode-mimo | mimo-v2.5 | 7/9 | 0.1429 | 5/9 |
| opencode-minimax | minimax-m2.7 | 8/9 | 0.1250 | 5/9 |
| **전체** | — | **22/27** | **0.1506** | **15/27** |

### 코드베이스별 (valid episode 한정)

| codebase | valid/total | avg_score | backend_exercised |
|---|---|---|---|
| ClickHouse-master | 8/9 | 0.0469 | 4/9 |
| angular-main | 8/9 | 0.2422 | 3/9 |
| deno-main | 6/9 | 0.1667 | 8/9 |

---

## 5. synthetic→real 변화

**이전(v3):** opencode-serena 9 cell(27 episode)은 scored_episodes에 없음. 도입비용·무결성 표에만 synthetic "skipped" row로 주입, 품질 paired-delta에서 제외.

**이후(v4):** opencode-serena 27 에피소드 전부 real 데이터로 통합.
- `scored_episodes.180.json`: 153+27=180개 레코드
- 품질 표: opencode-serena 9 셀 real 값으로 진입 (no-mcp 셀 없어 paired-delta는 null)
- 효율 표: 27 행 추가
- 행동 프로파일 표: 9 셀 추가
- 도입비용 표: `skipped` → `executed_via_rewire`
- 무결성 표: synthetic skip row 제거 → real 행으로 대체

---

## 6. 재집계 결과 확인

aggregate.mjs (v4) 실행 결과:
- quality rows: 60 (기존 45 + 신규 15)
- efficiency rows: 180 (기존 153 + 신규 27)
- behavior_profile rows: 60 (기존 45 + 신규 15)
- adoption_cost rows: 21 (기존 12 + opencode-serena 9)
- integrity rows: 180 (기존 153 + 신규 27)

opencode-serena가 real cell로 진입 확인:
- quality: 9 신규 셀에 mean_score 값 존재 (0~0.375)
- paired_delta: null (no-mcp 셀 없음 — 정상)
- backend_off(180기준): 24 (실측)

---

## 7. Caveat 유지·추가

**유지 (v3에서 이어받음):**
- [경고1] codex backend_exercised 버그 교정 (27개 false→true)
- [경고2] opencode task 서브에이전트 과소집계 (미교정)
- [경고3] codex read-only sandbox 런타임 confound
- codex-serena degraded (에러 에피소드 3/9)

**추가 (v4 신규):**
- [경고4] opencode-serena 약체 데이터: avg_score=0.1506, backend_exercised=15/27
- opencode-serena에도 task 서브에이전트 과소집계 동일 적용 (serena 내부 task 위임 호출 미집계)
- per_fact_score=null (scorer 입력 불일치) → 사실 단위 분석 불가
- no-mcp 셀 없어 paired-delta 산출 불가 → serena vs no-mcp 비교 불가

---

## 8. 출력 파일

| 파일 | 설명 |
|---|---|
| `-03/analysis/scored_episodes.180.json` | 180개 통합 (스키마 parity OK) |
| `-03/analysis/mcp_comparison_tables.json` | 180 기준 집계 JSON |
| `-03/analysis/mcp_comparison_tables.md` | 180 기준 집계 MD (469 lines) |
| `-03/phases/integrate/integration_report.md` | 본 보고서 |
