# 정식 벤치마크 — 최종 보고 (cms-official-benchmark-20260619-03)

> trigger=**completed** (검토 대기)
> run_id: `cms-official-benchmark-20260619-03` · base=`-02` · resumed/superseded(`-02`)
> judge: `opus`(=`claude-opus-4-8`) · executed N=**180** · valid=**166**

---

## §0 가장 중요한 발견 — codex 텔레메트리 버그와 -02 결론 정정

**-02 HALT2에서 사용자가 수락한 "codex 비교 무의미(behavioral null)" 결론은 이번 런에서 정정된다.**

runner의 `extractCodexOutput` 함수가 `toolEvents:[]`를 하드코딩으로 반환하는 버그로 인해 codex 27개 MCP episode의 도구 호출이 전수 0으로 오기록됐다. 실제 codex stdout에는 `mcp_tool_call` 이벤트가 실재한다(codex-codemap deno r1 = 80 events 확인). 복구 후 codex 27개 MCP episode 전수 `backend_exercised=true`로 확정됐으며, 이 결과는 `scored_episodes.180.json`에 persist 완료됐다(confirmed).

점수는 `raw_answer` 기반이므로 **유효(불변)**. 망가진 것은 도구 텔레메트리뿐이며, 점수·per_fact·valid 분류는 frozen 상태다.

이에 따라 codex는 "측정 불능"이 아니라 **사용 가능한 2차 비교(usable)**로 재분류된다. 단, codex = usable이지 claude 동급 clean이 아님을 명시한다(가드레일):
- **런타임 confound 유지(confirmed)**: codex=read-only OS sandbox(mutating bash 없음) vs claude=mutating bash. 버그와 무관하게 유효한 공정성 한계.
- **codex-serena degraded(confirmed)**: serena 호출 에러 3/9 episode(에러 25건 / ok 183건). codex-codemap/codegraph는 usable.

---

## §1 헤드라인 — MCP는 약한 baseline에서만 실효 (codebase 의존)

**단일 우승 backend는 없다. 최적 backend는 codebase에 의존하며, MCP의 실효는 no-mcp baseline이 약할 때만 나타난다.** (n=3/cell, descriptive only)

claude(상대적으로 약한 baseline)와 codex(강한 baseline)는 backend **순위 방향**은 대체로 일치하나, **밴드(±0.25 CH / ±0.125 deno·angular)를 넘는 실제 win은 claude에서만** 나온다:

- ClickHouse: 순위상 serena 최상 (claude 0.79 / codex 0.83). 단 **claude만 win**(Δ+0.33>밴드), **codex는 밴드 내 tie**(Δ+0.17<0.25).
- deno: 순위상 codemap 최상 (claude 0.67 / codex 0.77). 단 **claude만 win**(Δ+0.48>밴드), **codex는 밴드 내 tie**(Δ+0.04<0.125).
- angular: 두 모델 모두 전 backend tie (MCP 무차별, baseline 강함).

즉 codex는 **밴드를 넘는 실제 win backend가 없다**(전 codebase tie; deno에서 codegraph/serena는 loss). 이는 "MCP는 baseline이 약할 때만 실효"를 **반박이 아니라 보강**한다 — codex의 강한 no-mcp baseline이 MCP 한계효용을 낮춘 것으로 추정(inferred; sandbox confound와 분리 불가).

-02의 "claude 1모델 의존" 우려는 codex가 usable 2차 비교로 복원되며 **부분 개선**(순위 방향 일치)됐으나, codex의 win 부재·confound·opencode 약체로 **완전 해소는 아니다**. ("2-모델이 동일 win을 복제"라는 강한 주장은 데이터(codex tie)가 뒷받침하지 않는다.)

한계도 유지된다: codex confound(read-only sandbox vs claude mutating bash) + deno에서 codex codegraph/serena loss(codegraph −0.44 / serena −0.25) + opencode 3종 약체·노이즈.

---

## §2 집계

- 설계(nominal): 5 model/runtime × 4 backend × 3 codebase × 1 task × 3 round = **nominal 180**
- **executed 180** = -02의 153 + 신규 opencode-serena 27 episode 추가 실행
- **valid 166** (harness_valid && !timed_out). invalid 14, timed_out 10
- **backend_off 24** = 전부 opencode. codex 기여 **0** (교정 완료)
  - 구성: 153 carryover 12(opencode non-serena) + 신규 opencode-serena 중 12 = 24
  - runtime별: deepseek 7(non-serena 3+serena 4) / mimo 12(non-serena 8+serena 4) / minimax 5(non-serena 1+serena 4)
  - -02 기준 원래 39(codex 27 버그 + opencode 12) → codex 교정 후 12 → 180 기준 24로 재산출
- n=3/cell, 1 task/repo → **descriptive only** (추론통계 아님)
- 동률 밴드: ±0.25 ClickHouse, ±0.125 deno/angular
- 점수·per_fact·valid 분류 **불변(frozen)**

---

## §3 backend×codebase 요지

### claude-sonnet (clean 비교 — 4-backend 전수 backend_off=0)

| codebase | backend ranking (mean) | no-mcp 대비 |
|---|---|---|
| ClickHouse | **serena 0.79** > codegraph 0.625 > codemap 0.54 > no-mcp 0.46 | serena=win(+0.33), 나머지 tie |
| deno | **codemap 0.67** > serena 0.44 > no-mcp 0.19 = codegraph 0.19 | codemap=win(+0.48), serena=win(+0.25), codegraph=tie |
| angular | codegraph 0.77 ≈ codemap 0.75 ≈ no-mcp 0.73 ≈ serena 0.71 | **전부 tie** (MCP 잉여) |

### codex-gpt54 (usable, confound 동반 — 신규 활용 데이터)

| codebase | backend ranking (mean) | no-mcp 대비 | 비고 |
|---|---|---|---|
| ClickHouse | serena 0.83 > codegraph 0.79 > codemap 0.67 = no-mcp 0.67 | **전부 tie**(serena Δ+0.17<0.25) | 순위는 serena 최상이나 win 아님 |
| deno | codemap 0.77 > no-mcp 0.73 > serena 0.48 > codegraph 0.29 | codemap=**tie**(Δ+0.04<0.125); codegraph/serena=loss | codegraph −0.44 / serena −0.25 |
| angular | no-mcp 0.69 ≈ serena 0.69 > codemap 0.625 ≈ codegraph 0.625 | **전부 tie** | baseline 강함 |

**codex는 밴드를 넘는 win backend가 없다**(순위 방향만 claude와 일치). codex-serena degraded(에러 3/9 episode). sandbox confound 유지.

### opencode 3종 (약체·노이즈 — 1~2줄 요약)

- deepseek ~0.28 / mimo ~0.36 / minimax ~0.35 수준이나 분산 큼. MCP가 오히려 손해인 셀 다수(minimax ClickHouse: no-mcp 0.79 → codegraph 0.08 / serena 0.00).
- opencode-serena 신규 27 episode 중 **valid 22개 평균 0.151**(timeout 3 제외; 전수 27 기준 0.123). 코드베이스별 valid 평균 CH 0.047(n8) / deno 0.167(n6) / angular 0.242(n8). backend_exercised 15/27. opencode가 serena 호출 후 task 서브에이전트로 위임 → 도구수 과소집계.

---

## §4 유지 한계

1. **런타임 confound**: claude=mutating bash / codex=read-only sandbox / opencode=no-bash → no-mcp baseline 자체가 runtime마다 도구 표면이 달라 backend 효과와 baseline 표면 분리 불가.
2. **codex-serena degraded**: 에러 3/9 episode → codex의 serena cell은 제한적 신뢰도.
3. **opencode 약체·과소집계**: 3종 모두 평균 점수 낮고 노이즈 큼. serena 호출 후 내부 서브에이전트 위임으로 도구수 비교 시 과소집계 감안 필요.
4. **n=3/cell descriptive only**: cell당 n=3, IQR/SE 넓고 paired delta 대부분 비유의. 추론통계 금지.
5. **scorer 한계**: LLM judge(opus), temperature 직접 고정 불가 → frozen-judge self-consistency로 재정의(사용자 수락), ±1/F fact 밴드.
6. **1 task/repo**: codebase 축이 task 축과 완전 교락 → "backend×codebase 상호작용" 일반화 불가.

---

## §5 산출물 경로

| 종류 | 경로 |
|---|---|
| 교정 표(JSON/MD) | `analysis/mcp_comparison_tables.{json,md}` |
| 교정 데이터셋 | `analysis/scored_episodes.180.json` (codex 교정 persist) |
| 상세 보고 | `analysis/detailed_report.md` |
| 한계·무결성 | `analysis/limitations_and_integrity.md` |
| 교정 노트 | `phases/result-metric-correction/{correction_notes.md, correction_notes_v2.md}` |
| 통합·persist | `phases/integrate/{integration_report.md, persist_fix_report.md}` |
| codex 진단 | `phases/codex-tool-exposure-diagnostic/findings.json` |
| opencode-serena | `phases/opencode-serena-rewire-retry/findings.md` |
| 검증 | `phases/result-metric-verify/verify.md` |

---

```yaml
termination:
  trigger: completed
  run_id: cms-official-benchmark-20260619-03
  resumed_from: null
  topology: nested
  phases_run: 13  # A(codex진단) B(opencode-serena retry) C(표교정v2) C2(backend_exercised재계산v3) C-verify(5/5 PASS) E(opencode-serena 27 run) F(통합180) F2(persist-fix 4/4 PASS) G(서술재작성×3) + HALT2검토(constructive+adversarial) + 검토반영(전부)
  executed_n: 180  # -02의 153 + 신규 opencode-serena 27
  valid_n: 166
  backend_off_n: 24  # 전부 opencode, codex 0 (교정 완료)
  user_decisions:
    - gate: codex_rerun
      question: codex 27 MCP episode 재실행 여부?
      answer: "재실행 안 함 — 점수는 raw_answer 기반 유효, 텔레메트리 stdout 재파싱으로 복구"
    - gate: opencode_serena_rerun
      question: opencode-serena 27 episode 재실행 여부?
      answer: "재실행 수락 — -02에서 skip된 27 episode 신규 실행, 180 집계 완성"
    - gate: narrative_rewrite
      question: 서술 재작성(-03 최종본) 여부?
      answer: "재작성 수락 — codex 교정·27 추가 반영, -02 HALT2 결론 정정"
    - gate: review_accept_reject
      question: HALT2 검토 findings(주요3·경미4·참고3, 치명1=codex win→tie 오분류) 반영 범위?
      answer: "전부 반영 — codex win→tie 교정(밴드 규칙 자기위반 시정), 헤드라인 '2-모델 교차복제'→'순위 방향 일치/MCP는 약한 baseline에서만 실효'로 약화. 4개 문서+표 일관성 재검증(codex win 0/tie 7/loss 2)."
  superseded_runs:
    - run_id: cms-official-benchmark-20260619-02
      reason: "codex 텔레메트리 버그 교정 + opencode-serena 27 episode 추가 + 서술 재작성"
    - run_id: cms-official-benchmark-20260619-01
      reason: "범위 불일치(2-codebase / Angular no-go / minimax 미포함); -02가 superset"
  residual_issues:
    - severity: major
      problem: "runner extractCodexOutput 미패치 — 향후 codex run 대비 하드코딩 버그 잔존"
    - severity: minor
      problem: "codex-serena degraded: 에러 3/9 episode (에러 25 / ok 183) — serena cell 제한적 신뢰도"
    - severity: minor
      problem: "opencode 과소집계: serena 호출 후 task 서브에이전트 위임 → 내부 도구수 비집계"
    - severity: info
      problem: "fairness 비대칭(claude bash / codex sandbox / opencode no-bash) — no-mcp baseline confound 유지"
  failure_log:
    - type: harness
      cause: "codex extractCodexOutput toolEvents:[] 하드코딩 버그 → codex 27 MCP episode 도구호출 0 오기록; stdout 재파싱으로 복구, 점수 유효"
    - type: harness
      cause: "서술 에이전트 컨텍스트 오버플로 사망 다수; 산출물(JSON/MD)은 durable 생존"
  artifacts:
    - analysis/detailed_report.md
    - analysis/mcp_comparison_tables.json
    - analysis/mcp_comparison_tables.md
    - analysis/limitations_and_integrity.md
    - analysis/scored_episodes.180.json
    - phases/result-metric-correction/correction_notes.md
    - phases/result-metric-correction/correction_notes_v2.md
    - phases/integrate/integration_report.md
    - phases/integrate/persist_fix_report.md
    - phases/codex-tool-exposure-diagnostic/findings.json
    - phases/opencode-serena-rewire-retry/findings.md
    - phases/result-metric-verify/verify.md
```
