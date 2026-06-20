# 한계·무결성 — cms-official-benchmark-20260619-03

이 문서는 executed N=180 결과의 신뢰 경계를 명시한다. 본문 수치는 이번 -03 런만 사용하며, -02(153 episode)는 비교 맥락 및 무결성 교정 근거로만 분리한다. 모든 통계는 descriptive(서술적)이며 inferential(추론적)이 아니다.

---

## 1. 표본 한계 — 1 task/repo, n=3 round

- **repo당 selected task 1개**. codebase 결론은 각 1개 hard task의 결과이지 codebase 전반의 일반화가 아니다.
- **cell당 n=3 round.** codebase×runtime×backend cell의 통계(mean/median/IQR/SE)는 표본 3에서 나온 값이라 IQR/SE가 넓고, paired delta 대부분이 통계적으로 비유의하다. 예: claude deno serena의 IQR=0.3125. 본 분석은 이 한계를 받아들여 **win/tie 동률 밴드를 task fact band(±0.25 ClickHouse, ±0.125 deno/angular)로 넓게** 잡았고, 그보다 좁은 차이는 noise(tie)로 처리했다.
- 따라서 어떤 paired delta도 "유의한 개선"으로 읽지 말 것. 관찰된 방향성(경향)만 기록한다.
- **stdev/SE 산식 각주**: 표·본문의 stdev·SE는 **모집단 표준편차(÷n)** 규약이라 통상의 표본 SE(÷(n−1))보다 작게 보인다. 'IQR/SE 넓다' 캐비엇은 이 작은 SE가 아니라 n=3 분포 폭과 round 중첩에 근거하며, 어떤 결론도 SE 절대값에 의존하지 않는다.

---

## 2. scorer 한계 — LLM judge + frozen-judge self-consistency 재정의

- 채점자는 LLM judge(`opus`=`claude-opus-4-8`)이며 closed-book(코드베이스/도구 접근 차단, candidate raw answer 텍스트만 판정).
- **temp=0 직접 고정 불가**: claude -p CLI가 temperature 파라미터를 노출하지 않는다. 따라서 "가장 결정적인 설정"(고정 모델 + 고정 system/user 프롬프트 + 모든 빌트인 도구 비활성 + `--strict-mcp-config` + structured JSON schema)으로 대체했다. 잔여 비결정성은 task당 **±1/F fact 허용오차 밴드**(ClickHouse ±0.25, deno/angular ±0.125)가 흡수한다.
- 이로 인해 "self-consistency"는 **reproducibility(동일 입력→동일 출력) 보장이 아니라 frozen-judge self-consistency 게이트로 재정의**된다: 재채점 없이 기존 per_fact_score를 frozen schema 공식으로 재계산해 reported score와 대조한 것이며(formula_match=true, mismatch 0), judge의 verdict 자체가 매번 동일하다는 의미는 아니다.
- 가중평균 공식 `Σ(weight×value)/Σ(weight)`, per-fact verdict ∈ {present,partial,absent} → {1.0,0.5,0.0}. codebase별 frozen schema(ClickHouse F=4 weight 0.25, angular/deno F=8 weight 0.125).
- **점수·per_fact·valid 분류는 불변(frozen)**이다. 교정 대상은 도구 텔레메트리뿐이며 raw_answer 기반 채점 결과에는 영향이 없다.

---

## 3. Angular 앵커 variance — 0.5625 vs 0.1875 (약 3배 swing)

- **출처 구분(중요)**: 0.5625/0.1875는 **이번 180 executed 집계 밖의 앵커-검증 시도값**이다(angular-main selected task의 round-2 canonical 앵커 0.5625, sonnet 2x 재검증에서 run-1=0.5625 / run-2=0.1875로 갈림; run-2는 저장소 오매핑으로 codebase 접근 없이 지식 기반 답변으로 후퇴한 경우). 이번 런 angular claude no-mcp 실측 round 점수(scored_episodes.180.json)는 별개이며, 이 변동성 논의를 이번 런 angular 실측 변동으로 오인하지 말 것.
- 이 약 3배 swing은 scorer flakiness가 아니라 서로 다른 solve 시도의 model variance다(같은 task, 다른 시도). angular 결과를 해석할 때 이 변동성을 전제로 둘 것 — angular의 작은 backend 간 차이(전부 tie)는 이 variance 폭 안에 충분히 들어간다.

---

## 4. [핵심 무결성 사건] codex 텔레메트리 버그 — -02 결론 정정

### 4-1. 버그 요약

-02 런 이후, codex 전용 출력 파싱 함수 `extractCodexOutput`가 `toolEvents:[]`를 **하드코딩 반환**하는 구현 버그가 발견됐다. 이로 인해 codex 27 MCP episode의 도구 호출 이벤트가 모두 0으로 오기록됐다.

실제 codex stdout에는 `mcp_tool_call` 이벤트가 실재했다(예: codex-codemap deno r1 = 80 events). stdout 재파싱을 통해 codex 27 MCP episode **전수**의 `backend_exercised=true`가 복구됐고 `scored_episodes.180.json`에 persist 완료됐다.

점수(raw_answer 기반)는 버그의 영향을 받지 않아 **유효**하다. 망가진 것은 도구 텔레메트리뿐이다.

### 4-2. 발견 및 검증 경위

| 단계 | 내용 |
|---|---|
| 초기 발견 | 오케스트레이터 직접 진단 — codex stdout과 toolEvents 불일치 확인 |
| 독립 수렴 | 2개 서브에이전트(diagnostic agent, correction agent)가 독립적으로 동일 버그 특정 |
| 교차 검증 | C-verify 에이전트 5/5 PASS(검증 통과) |
| 데이터 fix | persist-fix 4/4 PASS(교정 데이터셋 저장 완료) |

### 4-3. -02 HALT2 수락 결론 정정

-02에서 사용자가 HALT2 체크포인트에서 수락한 "codex 비교 무의미(behavioral null)" 결론은 이 텔레메트리 버그에 의한 오판이었다. **이 결론을 정정한다.**

교정 후 codex-gpt54는 **사용 가능한 2차 비교 대상(usable secondary comparator)**이다. backend **순위 방향**은 claude와 대체로 일치하나, **codex는 밴드를 넘는 win은 없다(전부 tie)**:
- ClickHouse: serena 순위 최상(codex 0.83 / claude 0.79) — 방향 일치이나 codex Δ+0.17 < 밴드 0.25 = **tie**
- deno: codemap 순위 최상(codex 0.77 / claude 0.67) — 방향 일치이나 codex Δ+0.04 < 밴드 0.125 = **tie**; codegraph/serena는 **loss**
- angular: 전 backend tie(codex / claude 모두 일치)

순위 방향 일치는 -02 대비 핵심 업그레이드이나, "2-모델이 동일 win을 교차복제"라는 강한 주장은 데이터(codex tie)가 뒷받침하지 않는다. "순위 방향 일치 / 강도 비대칭"으로 기술한다.

### 4-4. 텔레메트리 버그의 분류(class) 명시

이번 발견은 단발 사건이 아니라 **텔레메트리 배선 버그 클래스**를 노출했다:

1. **codex toolEvents 하드코딩** — 이번에 교정 완료.
2. **opencode task 위임 미집계** — opencode가 serena 호출 후 task 서브에이전트로 위임 시 내부 도구 호출이 상위 집계에 포함되지 않는 구조적 과소집계. 현재 미교정.

두 케이스 모두 "측정된 0"이 아니라 "미계측(not instrumented)" 구간이다. 도구 사용 수치 비교 시 이 구분을 유지할 것.

---

## 5. backend_off 재조정 (180 기준)

-02 기준 backend_off는 39(codex 27 버그 + opencode 12)였으나, 교정 후 전면 재조정된다.

| 기준 | backend_off | 구성 |
|---|---|---|
| -02 (153 episode) | 39 → 교정 후 **12** | codex 27 버그 교정; opencode 12 잔존 |
| -03 (180 episode, 이번 런) | **24** | carryover 12(opencode non-serena) + 신규 opencode-serena 중 12 = 24; runtime별 deepseek 7(non-serena 3+serena 4) / mimo 12(non-serena 8+serena 4) / minimax 5(non-serena 1+serena 4) |

**현재 backend_off 24는 전부 opencode**(deepseek 7 / mimo 12 / minimax 5). codex의 기여는 0이다.

- 153 기준 opencode backend_off 12(carryover): codegraph 7 / codemap 5 분포(deepseek 3, mimo 8, minimax 1 산발). 이 중 일부는 timeout episode와 중복이라 truncation 가능성이 있어 "의도적 미사용"과 구분이 어렵다.
- 신규 opencode-serena 27 중 12개: backend_exercised=false(backend 미 exercise).

---

## 6. codex — usable(사용 가능), but NOT clean(완전 청정 아님)

교정으로 codex가 사용 가능해졌으나, claude-sonnet과 동급 clean 비교로 취급하지 말 것.

### 6-1. 런타임 confound (버그와 무관하게 유효한 한계)

- **claude-sonnet**: mutating bash 가능(쓰기 가능 셸).
- **codex-gpt54**: read-only OS sandbox(셸 있으나 읽기 전용; mutating bash 없음).

같은 "no-mcp" 라벨이라도 builtin 탐색 역량이 다르다. codex no-mcp baseline은 claude no-mcp와 동일 조건이 아니다. 이 confound는 텔레메트리 버그 교정과 무관하게 유효하며, codex 점수를 claude와 절대값 비교할 때 반드시 명시해야 한다.

### 6-2. codex-serena degraded

- codex-serena 9 episode 중 serena 호출 에러 3건(에러 25 / ok 183). codex-codemap·codegraph는 usable.
- serena에서 codex deno의 paired delta가 손해(-0.25 수준)인 점은 이 degraded 상태와 연관 가능성이 있다.

---

## 7. opencode 약체 및 비대칭

- **opencode 3종**(deepseek/mimo/minimax) 평균 점수: deepseek ~0.28 / mimo ~0.36 / minimax ~0.35. 분산 큼. MCP가 오히려 해로운 셀 다수(예: minimax ClickHouse no-mcp 0.79 → codegraph 0.08 / serena 0.00).
- **opencode-serena 신규 27**: **valid 22개 평균 0.151**(timeout 3 제외; 전수 27 기준 0.123). 코드베이스별 n = CH8 / deno6 / angular8. 모델별(valid) deepseek 0.188(n=7) / mimo 0.143(n=7) / minimax 0.125(n=8). codebase별 CH 0.047 / deno 0.167 / angular 0.242. backend_exercised 15/27. -02의 미측정(transport 배선 미완)을 해소했으나 약체 패턴은 신규 데이터에서도 동일하게 나타났다.
- **opencode task 위임 비대칭**: opencode가 serena 호출 후 task 서브에이전트로 위임 시 내부 도구가 상위 집계에 포함되지 않는다(과소집계). 도구 수 비교 시 이 비대칭을 명시할 것. §4-4 참조.
- 결과적으로 opencode cell의 MCP vs no-mcp paired delta는 신호 대 잡음 비가 낮아 방향성 판단에 주의를 요한다.

---

## 8. fairness 비대칭 — no-mcp baseline confound

각 runtime의 **기본 도구 표면(tool surface)이 달라 no-mcp baseline 자체가 동일 조건이 아니다**:
- **claude-sonnet**: mutating bash 가능(쓰기 가능 셸).
- **codex-gpt54**: read-only sandbox(셸 있으나 읽기 전용).
- **opencode 계열**: no-bash(`bash:false` 적용).

따라서 같은 "no-mcp" 라벨이라도 runtime마다 builtin 탐색 역량이 다르다. **기본 비교를 같은 runtime/model 내 backend vs no-mcp paired delta로 한정**했고, 5 model 풀링 순위는 confound를 명시한 보조 지표로만 제시한다.

---

## 9. tok_in 집계 규약 및 tool_call 재구성 공정성

- **tok_in 런타임별 규약**: claude/opencode = input+cache_read+cache_creation 합산; codex = input_tokens 그대로(캐시 포함, 이중계산 방지). 런타임 간 직접 절대값 비교 시 이 규약 차이를 감안할 것(key_facts_digest.md §G 참조).
- **tool_call 재구성**: 모든 도구 흔적 집계 + codegraph/serena 도구별 괄호; codex는 stdout 재구성(텔레메트리 버그 교정 후). ToolSearch는 별도 열. codex stdout 재구성의 공정성은 C-verify 5/5 PASS로 검증됐다.

---

## 10. 병렬 실행이 wall_time을 backend별 비대칭 부풀림

- 병렬(co-tenancy) 실행은 한 건당 wall_time을 부풀리며, 그 폭이 **backend마다 비대칭**이다. backend별 부하 수치(codemap≈0초대 read-only / codegraph≈8–12s SQLite WAL 동시읽기 경합 없음 / **serena≈42–45s** rust-analyzer·clangd가 프로젝트를 매번 로드해 자원 부하 최대)는 별도 co-tenancy 프로파일링/warmup 관찰에서 나온 값이며 이번 180 scored_episodes의 wall_time 평균에서 직접 재현되지 않는다.
- 각 episode의 `co_tenancy{count,backends}`(동시 실행 backend 구성)는 전 episode에 기록돼 있다. **효율 비교의 1차 신호는 tool_calls·token 수이고, wall_time은 보조 지표**로만(반드시 co_tenancy 기록과 함께) 읽는다. wall_time 절대값으로 backend 효율을 직접 비교하지 말 것.

---

## 11. 무결성 invalid 14 / timeout 10 (전부 opencode)

executed 180(= -02의 153 + 신규 opencode-serena 27). valid 166 / invalid 14.

**측정된 0**: 전 episode `mutation_guard=clean`(전수 clean). **미계측 0(not instrumented)**: wrong_root / out_of_repo / transport_retry / retry는 데이터·audit 어디에도 필드가 없어 집계가 false로 하드코딩한 값이므로 측정된 무결성으로 읽으면 안 된다. 이 둘을 같은 줄에 섞지 않는다.

timeout 10 + empty(invalid) 4 = 14 invalid, **전부 opencode 계열**.

**앵커-검증 variance 구분**: 앵커-검증 variance(0.5625/0.1875)는 이번 런 180 집계 **밖**의 선행 시도값 → 이번 런 angular 실측과 별개(§3 참조).

**텔레메트리 버그 class 교차참조**: 도구수 0이 미계측(not instrumented) 2종: ① codex toolEvents 하드코딩(교정 완료) ② opencode task 위임 과소집계(미교정) — §4-4 참조.

---

## 12. 계보 — -02와의 관계 및 분리

- 본 분석의 **본문 수치는 전부 이번 cms-official-benchmark-20260619-03의 executed N=180**(scored_episodes.180.json, 오케스트레이터 직접 재계산·검증)에서 나온다.
- **-02(153 episode)** 수치는 §4의 텔레메트리 버그 교정 맥락 및 비교 참조로만 사용한다. -02의 개별 셀 점수를 이번 표에 복사하지 않는다.
- 20260616(cms-4way-toolbench)의 REPORT.md / P6-analysis / P8-report / metric6-adoption은 보고서 구조·표 열·해석 규칙의 형식 참고로만 사용했다.
- 세 런은 task set·model/runtime 구성·backend 구성이 다르므로 점수의 직접 비교는 무효다.

---

## 무결성 한 줄 요약

executed N=180(-02의 153 + 신규 opencode-serena 27) / valid 166 / invalid 14(timeout 10 + empty 4, 전부 opencode) / **측정된** mutation 위반 0(mutation_guard 전수 clean) · wrong-root·out-of-repo·retry는 **미계측(not instrumented)**이라 0을 측정값으로 보지 않음 / scorer formula_match=true, C-verify 5/5 PASS / **핵심 교정**: codex `extractCodexOutput` toolEvents:[] 하드코딩 버그 → codex 27 MCP episode 전수 backend_exercised=true 복구 → -02 HALT2 "codex behavioral null" 결론 정정(codex는 usable 2차 비교; backend 순위 방향은 claude와 일치하나 **밴드 넘는 win은 없다(전부 tie)**) / 단 codex confound(read-only sandbox)·codex-serena degraded(에러 3/9)·opencode 약체·opencode task 위임 과소집계 비대칭은 유효한 한계로 유지됨 / **앵커-검증 variance(0.5625/0.1875)는 이번 런 180 집계 밖의 선행 시도값 → 이번 런 angular 실측과 별개(§3 참조)** / **도구수 0이 미계측 2종: ① codex toolEvents 하드코딩(교정 완료) ② opencode task 위임 과소집계(미교정) — §4-4 참조**.
