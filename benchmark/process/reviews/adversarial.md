# 적대적 검토 보고서 — cms-official-benchmark-20260619-03

> 생성일: 2026-06-20  
> 검토 대상: `report.md`, `analysis/detailed_report.md`, `analysis/limitations_and_integrity.md`, `analysis/key_facts_digest.md`  
> 독립 재계산: `analysis/scored_episodes.180.json` (node 직접 집계)  
> **치명 오류**: 있음 (finding #3)  
> **수치 불일치 개수**: 5개 (반올림 오류 3개 + opencode 평균 표현 오류 1개 + win 오분류 계열 1개)

---

## Finding #1 — 반올림 오류 3셀

- **심각도**: 경미  
- **주장**: 보고서가 3개 cell 평균을 0.63으로 표기  
- **반증 시도**: node 재계산 결과 실제값은 0.6250 (차이 0.0050)  
- **대상 셀**:
  - `claude ClickHouse codegraph` → report: 0.63, actual: 0.6250
  - `codex angular codemap` → report: 0.63, actual: 0.6250
  - `codex angular codegraph` → report: 0.63, actual: 0.6250
- **판정**: holds (불일치 확인됨)  
- **근거**: 0.6250을 0.63으로 반올림한 것은 통상적 허용오차 ±0.005를 경계에서 초과함. 나머지 24개 cell 수치는 ±0.005 이내에서 일치. 서술 결론에는 직접 영향 없으나 수치 정밀도 신호.

---

## Finding #2 — opencode-serena "27 episode 평균 0.151" 표현 오류

- **심각도**: 경미  
- **주장**: 보고서(report.md §3, key_facts §E)가 "opencode-serena 신규 27 episode 평균 **0.151**"이라고 표기  
- **반증 시도**: node 재계산  
  - 27 전체 평균 (null score=0 포함): **0.1227**  
  - valid only 22개 평균 (harness_valid && !timed_out): **0.1506** ≈ 0.151 ← 보고서 수치와 일치  
  - integration_stats.opencode_serena_avg_score = 0.15056... (valid only 방식으로 산출됨 확인)
- **판정**: holds (표현 오류 확인됨)  
- **근거**: n=27이라고 명시하면서 실제 평균은 valid 22개 기준으로 산출됐다. 이 표현은 독자에게 27개 전체 평균이라고 오해하게 한다. 유사하게 per-model 분류값(deepseek 0.188, mimo 0.143, minimax 0.125)도 valid only 7/7/8개 기준이다.
- **codebase별 비교** (report vs valid only 재계산):
  - CH: 0.047 vs 0.0469 (OK — valid 8개)
  - deno: 0.167 vs 0.1667 (OK — valid 6개)
  - angular: 0.242 vs 0.2422 (OK — valid 8개)
  - 단, "27 episode 평균" 문구 자체가 n 기술과 불일치.

---

## Finding #3 — codex win 오분류 2건 (치명 오류)

- **심각도**: 주요  
- **주장**: 보고서(report.md §1·§3, detailed_report §3, key_facts §E)가 다음을 주장함:
  - "ClickHouse: 두 모델 모두 serena 최강 — codex 0.83 (win)"
  - "deno: 두 모델 모두 codemap 최강 — codex 0.77 (win)"
  - "claude·codex 둘 다 usable 비교이며, 독립적으로 같은 패턴을 복제" (핵심 업그레이드)
- **반증 시도**: node 재계산, 동률 밴드 직접 적용
  - **codex CH serena**: no-mcp 대비 delta = **+0.1667**, band = **±0.25** → **TIE** (win 아님)
  - **codex deno codemap**: no-mcp 대비 delta = **+0.0417**, band = **±0.125** → **TIE** (win 아님)
  - 보고서 §3이 "codemap=win(+0.04, 밴드 경계)"라고 표기했으나 0.04 < 0.125 이므로 밴드 안 = TIE
  - 보고서 §3이 "serena=win(+0.16)"이라고 표기했으나 0.17 < 0.25 이므로 밴드 안 = TIE
- **판정**: holds (오분류 확인됨)
- **근거**: 재계산 결과 **codex에서 실제 WIN인 backend는 없다** (전 codebase·backend TIE, deno에서 codegraph/serena는 LOSS). 보고서 자신이 정의한 동률 밴드(±0.25 CH, ±0.125 deno/angular)를 codex 셀에 적용하지 않고 win으로 오분류했다.
- **2-모델 교차복제 주장에 대한 영향**:
  - claude는 CH serena WIN(+0.33), deno codemap WIN(+0.48)이 맞다.
  - codex는 해당 셀이 TIE이다. 방향(serena 가장 높음, codemap 가장 높음)은 동일하나, win/tie 분류 기준으로는 복제가 성립하지 않는다.
  - 보고서가 "독립적으로 같은 패턴을 복제"라는 헤드라인 주장은 **순위 방향 일치**에 기반할 수 있으나, 보고서 자신이 정의한 win 기준을 위반하며 달성된 것이다. n=3·confound 조건에서 방향 일치만으로 "복제"라고 부르는 것은 주장 강도가 과장이다.
- **자기모순**: 보고서가 "동률 밴드를 넘어야 win"이라고 명시하면서 codex 셀에는 이를 적용하지 않았다. 보고서 내부 기준 충돌.

---

## Finding #4 — "2-모델 교차복제" 주장 강도 과장

- **심각도**: 주요  
- **주장**: 보고서 헤드라인(§0, §1, detailed_report §1)이 "claude·codex 둘 다 usable 비교이며, 독립적으로 같은 패턴을 복제"를 -03의 핵심 업그레이드로 강조  
- **반증 시도**:
  - codex에서 실제 win인 backend = **없음** (finding #3)
  - codex CH: 모든 backend TIE. codex deno: codemap TIE, codegraph/serena LOSS.
  - 방향 일치(serena 점수가 가장 높다, codemap 점수가 가장 높다)는 관찰 가능하지만, win 강도는 클로드와 codex에서 현저히 다름 (claude CH serena +0.33 vs codex +0.17)
  - n=3/cell, confound(sandbox vs mutating bash) 존재
  - codex 36 episodes 중 MCP 27은 모두 1 task/repo에서 나온 것
- **판정**: holds (주장 과장 확인됨)
- **근거**: "독립 복제"라 부르려면 두 모델이 동일 win 분류를 달성해야 설득력이 있다. 현 데이터는 순위 방향은 같으나 effect size가 달라 한 모델에서는 win, 다른 모델에서는 TIE이다. 이를 "복제"라고 부르는 것은 지나치다. 보고서 detailed_report §3이 "codex CH serena win", "codex deno codemap win(밴드 경계)"이라고 서술했으나, 둘 다 TIE 판정을 받아야 한다. "방향 일치(ordinal agreement)"로 약화해야 적절하다.
- **추가**: codex confound(sandbox)로 codex no-mcp baseline 자체가 claude보다 탐색력이 낮을 수 있어, codex에서 MCP 효용이 상대적으로 작게 나타날 수 있다. 이 confound가 effect size 차이의 원인일 가능성을 보고서가 언급하지 않는다.

---

## Finding #5 — deno에서 codex codegraph/serena LOSS가 "복제" 서사에서 약화됨

- **심각도**: 참고  
- **주장**: 보고서(report.md §1·§3)가 deno codex 손해를 "codex confound와 codex-serena degraded 영향이 함께 작용한 것으로 추정(inferred, 분리 불가)"으로 설명하고 한계 섹션에 명시  
- **반증 시도**: 보고서 §1 헤드라인이 "두 모델 모두 codemap 최강"이라고 선언한 직후 "deno에서 codegraph/serena가 codex에 손해(codegraph −0.44/−0.25)"를 덧붙임. LOSS 값이 헤드라인 바로 다음 줄에 명시는 됨.
- **판정**: refuted (LOSS 수치 자체는 정확히 표기됨 — 단, 헤드라인 배치가 이를 약화시키는 구성)
- **근거**: delta 수치(−0.4375, −0.2500)는 node 재계산과 일치. 단, 헤드라인에서 "두 모델 모두 codemap 최강"이라는 표현은 codex deno에서 codegraph·serena LOSS라는 사실을 독자가 덜 주목하게 하는 구성이다. 경미한 framing 문제이나 치명은 아님.

---

## Finding #6 — backend_off 153→180 전환 서술 불명확

- **심각도**: 참고  
- **주장**: 보고서 §2가 "153 기준 원래 39 → codex 교정 후 12 → 180 기준 24"로 설명  
- **반증 시도**: node 재계산
  - 153 기준 backend_off (신규 27 제외): **12** (codex 교정 후) ✓
  - 180 기준 backend_off: **24** = 12 + 신규 opencode-serena 12 ✓
  - integration_stats.backend_off_153 = 39 (교정 전 값) ✓
- **판정**: refuted (수치는 일치, 서술도 일관됨)
- **근거**: 153→180 전환 로직은 데이터와 일치한다. "39 → 12 → 24" 흐름이 올바르다.

---

## Finding #7 — 기본 집계 수치 전수 확인

- **심각도**: 참고 (정상 확인)  
- **검증 항목**: executed 180 / valid 166 / invalid 14 / timed_out 10 / backend_off 24 / codex MCP exercised 27
- **node 재계산 결과**: 모두 보고서 수치와 일치
  - total: 180 ✓
  - valid (harness_valid && !timed_out): 166 ✓
  - timed_out: 10 ✓
  - backend_off: 24 (전부 opencode — deepseek 7 / mimo 12 / minimax 5) ✓
  - codex MCP episodes exercised: 27/27 ✓
- **판정**: refuted (불일치 없음)

---

## 치명 오류 판정 요약

| finding | 심각도 | 주장 | 판정 | 비고 |
|---|---|---|---|---|
| #1 반올림 3셀 | 경미 | 0.63 vs 0.625 | holds | 결론 영향 없음 |
| #2 opencode n=27 표현 | 경미 | valid 22개 기준인데 27로 표기 | holds | 수치는 맞으나 분모 표기 오류 |
| **#3 codex win 오분류** | **주요** | codex CH serena / deno codemap을 win으로 오분류 | **holds** | **치명 — 교차복제 근거 붕괴** |
| **#4 교차복제 주장 과장** | **주요** | "독립 복제" 헤드라인이 codex TIE 데이터와 불일치 | **holds** | **치명 — 핵심 서사 과장** |
| #5 deno LOSS 약화 | 참고 | 헤드라인 배치 framing | refuted | 수치는 명시됨 |
| #6 backend_off 전환 | 참고 | 153→180 서술 | refuted | 정확함 |
| #7 기본 집계 | 참고 | 전수 수치 | refuted | 전부 일치 |

**치명 오류**: 있음 — Finding #3과 #4가 보고서의 핵심 서사("2-모델 교차복제")를 지탱하는 codex win 분류를 직접 무력화한다.

---

## 수치 불일치 상세 (재계산값 vs 보고서)

| 항목 | 보고서 | 재계산 | 차이 | 분류 |
|---|---|---|---|---|
| claude CH codegraph mean | 0.63 | 0.6250 | 0.005 | 반올림 오류 |
| codex angular codemap mean | 0.63 | 0.6250 | 0.005 | 반올림 오류 |
| codex angular codegraph mean | 0.63 | 0.6250 | 0.005 | 반올림 오류 |
| opencode-serena 전체 평균 (n 기술) | "27 episode avg 0.151" | valid 22개 avg 0.1506 | n 기술 오류 | 표현 오류 |
| codex CH serena status | win(+0.16) | TIE (delta 0.1667 < band 0.25) | 분류 오류 | **주요** |
| codex deno codemap status | win(+0.04) | TIE (delta 0.0417 < band 0.125) | 분류 오류 | **주요** |

---

## 재계산 명령어 기록 (재현 가능)

```js
// 전체 집계
node -e "
const data = JSON.parse(require('fs').readFileSync('analysis/scored_episodes.180.json','utf8'));
const eps = data.episodes;
const valid = eps.filter(e => e.harness_valid && !e.timed_out);
console.log(valid.length, eps.filter(e=>!e.backend_exercised).length);
"
// → valid: 166, backend_off: 24

// codex CH serena delta
// no-mcp mean: 0.6667, serena mean: 0.8333, delta: 0.1667, band: 0.25 → TIE

// codex deno codemap delta
// no-mcp mean: 0.7292, codemap mean: 0.7708, delta: 0.0417, band: 0.125 → TIE
```

