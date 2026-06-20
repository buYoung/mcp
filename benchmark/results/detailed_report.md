# 상세 보고서 — cms-official-benchmark-20260619-03

run-id: `cms-official-benchmark-20260619-03` · judge: `opus`(=`claude-opus-4-8`) · executed N=**180** · 본문 수치는 이번 런 전수(180) 기준. -02 런은 교정 원본으로, -03이 교정·확장한 결과를 이 문서가 기록한다.

---

## 1. 헤드라인 — 순위 방향 일치·강도 비대칭: codebase 의존 fingerprint

> **n=3/cell·1 task/repo → 모든 수치 descriptive only. 점수 frozen.** 동률 밴드(±0.25 CH, ±0.125 deno/angular) 이내는 tie로 읽는다.

-02 보고서에서 핵심 걸림돌이었던 **-02 HALT2에서 사용자가 수락한 "codex behavioral null"** 결론은 정정된다. codex 27개 MCP episode 의 tool-event가 `extractCodexOutput` 버그로 `toolEvents:[]`로 하드코딩 오기록됐음이 확인됐다. stdout 재파싱으로 MCP 도구호출이 실재함을 복구했다(예: codex-codemap deno r1 = 80 events). 점수는 raw_answer 기반이라 유효하다 — 망가진 것은 도구 텔레메트리뿐이다.

이 교정 결과, -03의 핵심 업그레이드는 다음과 같다: **두 모델 간 순위 방향은 대체로 일치하나, 밴드를 넘는 실제 win은 claude에서만이다. codex는 전 codebase에서 밴드 내 tie에 그친다.**

- **ClickHouse**: claude serena Δ+0.33 **win**, codex serena Δ+0.17<0.25 = **tie**(순위 최상이나 밴드 미달)
- **deno**: claude codemap Δ+0.48 **win**, codex codemap Δ+0.04<0.125 = **tie**(순위 최상이나 밴드 미달); codex deno codegraph/serena는 **loss**
- **angular**: claude/codex 전부 **tie** → 두 모델 모두 MCP 무차별

**codex에는 밴드를 넘는 win backend가 없다**(순위 방향만 claude와 일치; deno codegraph/serena는 loss). MCP는 약한 baseline(claude)에서만 실효이고, codex(강한 baseline)는 이를 보강(반박 아님)한다 — 단 sandbox confound 분리 불가.

단일 우승 backend는 여전히 없다. 최적 선택은 codebase(=task)에 종속된다. confound와 한계는 유지된다(§8).

---

## 2. 벤치마크 개요

- 설계: 5 model/runtime × 4 backend × 3 codebase × 1 task × 3 round = nominal 180.
- 실행: **executed N=180**. -02의 153(opencode-serena 27 skip)에 신규 opencode-serena 27 추가.
- valid(harness_valid && !timed_out): **166**. invalid 14, timed_out 10.
- backend_off: **24**(전부 opencode — deepseek 7개·mimo 12개·minimax 5개). codex 기여 0(교정 완료).
- selected task 경로(repo당 1):
  - ClickHouse-master(F=4): `.agents/orchestration/cms-dataset-hardening-v3-redesign-targetroot-20260618/phases/ClickHouse-master/round-3/public_question.md`
  - deno-main(F=8): `.agents/orchestration/cms-dataset-hardening-v3-redesign-targetroot-20260618/phases/deno-main-retry-1/public_question.md`
  - angular-main(F=8): `.agents/orchestration/cms-dataset-hardening-v3-sequential-20260618/phases/angular-main/round-2/public_question.md`
- scorer proof: frozen-judge(opus=`claude-opus-4-8`), per-fact verdict ∈ {present,partial,absent} → {1.0,0.5,0.0}, 가중평균 `Σ(weight×value)/Σ(weight)`. 재채점 없이 per_fact_score frozen schema 공식으로 검증 → formula_match=true, mismatch 0.
- warmup proof: mutation_guard 전 전 episode `clean`(소스 파일 변경 0). serena cold reindex 실측(ClickHouse 62s clangd, angular 41s tsserver), codegraph angular cold init 80s.
- tok_in 집계 기준: claude/opencode = input+cache_read+cache_creation; codex = input_tokens 그대로(캐시 포함, 이중계산 방지).
- 교정 산출물: `analysis/scored_episodes.180.json`(codex 교정 persist 완료), `analysis/mcp_comparison_tables.{json,md}`, `phases/result-metric-correction/correction_notes_v2.md`.

---

## 3. 품질 — codebase별·model별 backend ranking + no-mcp 대비 paired delta

동률 밴드 = task fact band(±0.25 ClickHouse F=4, ±0.125 deno/angular F=8). 상세 수치·round_scores는 `analysis/mcp_comparison_tables.md` §1.

### claude-sonnet (clean, backend_off=0 전 cell)

| codebase | backend ranking (mean) | no-mcp 대비 |
|---|---|---|
| ClickHouse | **serena 0.79** > codegraph 0.625 > codemap 0.54 > no-mcp 0.46 | serena=win(+0.33), codegraph/codemap=tie |
| deno | **codemap 0.67** > serena 0.44 > no-mcp 0.19 = codegraph 0.19 | codemap=win(+0.48), serena=win(+0.25), codegraph=tie(0) |
| angular | codegraph 0.77 ≈ codemap 0.75 ≈ no-mcp 0.73 ≈ serena 0.71 | 전부 tie(MCP 잉여) |

해석: claude에서 MCP 효용은 no-mcp 난이도에 종속된다. no-mcp가 약한 ClickHouse/deno에서만 특정 backend가 win 밴드를 넘고, no-mcp가 이미 강한 angular에서는 4 backend 모두 tie. backend 우열도 codebase(=task)마다 달라진다. repo당 task 1개라 codebase 축이 task 축과 교락되어 있으므로, 이는 'backend×codebase 상호작용'이 아니라 '이 selected task별 관찰된 방향성'으로만 읽는다.

### codex-gpt54 (usable, confound 동반 — 신규 활용 데이터)

-02의 "codex MCP 비호출(behavioral null)" 결론은 텔레메트리 버그 교정으로 정정된다. codex 27 MCP episode 전수 backend_exercised=true(stdout 재파싱 복구 확인). 단 다음 confound는 교정과 무관하게 유효한 한계다:

- **런타임 confound 유지**: codex=read-only OS sandbox(mutating bash 없음) vs claude=mutating bash. 따라서 codex 점수는 **usable한 2차 비교**이지 claude 동급 clean 비교가 아니다.
- **codex-serena=degraded**: serena 호출 에러 3/9 episode(에러 25건 / ok 183건). codex-codemap·codex-codegraph=usable.

| codebase | backend ranking (mean) | no-mcp 대비 |
|---|---|---|
| ClickHouse | **serena 0.83** > codegraph 0.79 > no-mcp 0.67 = codemap 0.67 | serena=tie(+0.17<밴드0.25), codegraph=tie |
| deno | **codemap 0.77** > no-mcp 0.73 > serena 0.48 > codegraph 0.29 | codemap=tie(+0.04<밴드0.125), codegraph=loss(−0.44), serena=loss(−0.25) |
| angular | no-mcp 0.69 ≈ serena 0.69 > codemap 0.625 ≈ codegraph 0.625 | 전부 tie |

핵심 발산: **deno에서 codex의 codegraph(−0.44)·serena(−0.25)가 no-mcp 대비 손해**다. 이 발산은 claude에서도 codegraph deno가 tie(0)로 무효였던 점과 방향은 일치하나 폭이 다르다 — sandbox confound와 codex-serena degraded 영향이 함께 작용한 것으로 추정된다(inferred, 분리 불가).

**codex에는 밴드를 넘는 win backend가 없다(전 codebase 전부 tie; deno codegraph/serena는 loss).** 순위 방향(CH=serena 최상, deno=codemap 최상, angular=tie)은 claude와 일치하나, 강도 비대칭이 명확하다: claude deno codemap Δ+0.48 win vs codex deno codemap Δ+0.04 tie. MCP는 약한 baseline(claude)에서만 실효이며, 강한 baseline의 codex는 이를 반박이 아닌 보강한다(inferred, sandbox confound 분리 불가). 이것이 -03의 핵심 업그레이드다.

### opencode 3종 (약체·노이즈)

- deepseek 평균 ~0.28, mimo ~0.36, minimax ~0.35 수준이나 분산 크고 MCP가 오히려 해로운 셀 다수.
  - 대표 사례: minimax ClickHouse no-mcp 0.79 → codegraph 0.08 / serena 0.00 (MCP가 크게 손해). baseline이 강하면 MCP 추가가 오히려 해가 될 수 있다는 신호(1 task·n=3 descriptive).
  - codemap 효과: deepseek ClickHouse Δ+0.625, deepseek deno Δ+0.1458, deepseek angular Δ+0.3125 — 특히 baseline이 0인 경우 codemap이 메우는 패턴(deepseek ClickHouse no-mcp=0).
- backend_off 24 전부 opencode(deepseek 7·mimo 12·minimax 5). timeout/invalid도 전부 opencode.
- **opencode-serena 신규 데이터(27 episode)**: **valid 22개 평균 0.151**(timeout 3 제외; 전수 27 기준 0.123). 모델별(valid) deepseek 0.188(n=7) / mimo 0.143(n=7) / minimax 0.125(n=8). 코드베이스별(valid) CH 0.047(n=8) / deno 0.167(n=6) / angular 0.242(n=8). backend_exercised 15/27, timeout 3.
  - **과소집계 비대칭 주의**: opencode가 serena 호출 후 task 서브에이전트로 위임하는 패턴 → 내부 도구 미집계. 도구수 비교 시 감안.
- opencode 3종은 이번 벤치마크에서 backend 효과를 신뢰 가능한 수준으로 측정하기 어렵다 — 약체·노이즈 레이어로 분류.

### 보조: claude-sonnet 기준 backend 순위(clean 기준선) (confound 명시)

runtime/model confound 때문에 풀링은 보조 지표로만. codex usable 복구 후에도 sandbox confound로 인해 풀링을 backend 성능 결론으로 쓰면 안 된다.

| codebase | no-mcp | codemap | codegraph | serena |
|---|---|---|---|---|
| ClickHouse | 0.46(claude 기준) | 0.54(claude 기준) | 0.625(claude 기준) | 0.79(claude 기준) |
| deno | 0.19(claude 기준) | 0.67(claude 기준) | 0.19(claude 기준) | 0.44(claude 기준) |
| angular | 0.73(claude 기준) | 0.75(claude 기준) | 0.77(claude 기준) | 0.71(claude 기준) |

전체 풀링 수치는 `analysis/mcp_comparison_tables.md` §1을 참조. clean 비교 기준선은 claude-sonnet cell이 유일하다.

---

## 4. 효율 (같은 runtime/model 내 backend 비교)

효율은 같은 runtime/model 안에서만 비교한다. 비용 단가표 없어 $ 산출 생략. wall_time은 병렬 co-tenancy로 backend별 비대칭 부풀림 → 1차 신호는 tool_calls·token, wall_time은 보조. invalid 행은 평균 제외. 전 episode 수치는 `analysis/mcp_comparison_tables.md` §2.

### claude-sonnet backend별 도구 프로파일 (valid 평균)

| backend | tool_calls | tok_out | search | nav | read | grep/shell |
|---|---|---|---|---|---|---|
| no-mcp | 14.9 | 6641 | 0 | 0 | 6.6 | 6.6 |
| codemap | 11.4 | 5937 | 4.4 | 0.6 | 4.6 | 0.3 |
| codegraph | 6.0 | 5951 | 0 | 2.7 | 2.2 | 0 |
| serena | 14.0 | 6447 | 5.0 | 0 | 5.9 | 0 |

도구 구조: **codegraph가 가장 적은 호출(6.0)**로 내비게이션 중심(nav 2.7), grep/shell 0. **codemap은 grep/shell(6.6→0.3)을 search(0→4.4)로 치환** — builtin 탐색 부담을 MCP search가 흡수. **serena는 search+read 혼합**(호출수는 no-mcp와 유사 14.0). tok_out은 backend 간 큰 차이 없음(5.9k–6.6k). codegraph="적은 호출/내비게이션형", codemap="검색 치환형", serena="탐색 보강형" 프로파일이다.

codex 효율: tool_calls 집계 기준이 claude와 다르고(stdout 재파싱 기반), sandbox confound 포함. 상세는 `analysis/mcp_comparison_tables.md` §2 per-episode 참조.

opencode 효율: backend_off·timeout 혼입으로 backend 내 평균 불안정. tool_calls 척도도 runtime마다 다름(서브에이전트 위임으로 과소집계). per-episode 표 직접 참조.

---

## 5. 행동 프로파일 — 실제 MCP 사용 패턴

- **claude-sonnet**: 전 MCP cell backend 전수 호출(off 0). serena도 전 round 사용. clean 기준선.
- **codex-gpt54**: 텔레메트리 버그 교정 후 27/27 MCP episode 전수 backend_exercised=true 확인. codex-codemap·codex-codegraph=usable, codex-serena=degraded(에러 3/9 episode).
- **opencode**: backend_off 24 전부 opencode(deepseek 7·mimo 12·minimax 5). 일부는 timeout truncation과 중복. opencode-serena의 서브에이전트 위임으로 도구 집계 비대칭.
- backend_tool_bytes: codegraph/serena가 큰 반환(예: claude codegraph angular cell 수만 bytes), codemap은 가벼운 read-only. codebase×runtime×backend별 평균은 `analysis/mcp_comparison_tables.md` §3.

도구 클래스 믹스(§4 표)가 곧 행동 fingerprint: codegraph=navigation, codemap=search 치환, serena=search+read, no-mcp=read+grep/shell.

---

## 6. warmup / readiness / 도입비용

index 비용은 backend×codebase 단위이며 그 cell을 쓰는 모든 runtime이 공유한다(arm별 중복 부담 아님).

| backend | config 필요 | manual setup | index 디스크 | cold build 실측 | 비고 |
|---|---|---|---|---|---|
| no-mcp | no | no | — | — | builtin only, 도입비용 0 |
| codemap | yes(.codemap config.toml) | no | 9.7M(deno)–31M(angular) | 미측정(사전존재) | read-only, 가장 가벼움 |
| codegraph | no | no | 197M(deno)–454M(ClickHouse) | angular 80s(cold init) | SQLite DB, 동시읽기 경합 없음 |
| serena | yes(project index) | no | 151M(CH)–277M(angular) | CH 62s(clangd), angular 41s(tsserver) | 언어서버 부하 최대, codex-serena degraded |

핵심: **serena가 도입비용 최대**(언어서버 cold index 41–62s, 디스크 277M까지, 메모리/CPU 최대). codemap이 가장 저렴(read-only, 디스크 최소, config.toml만). codegraph는 config 불요이지만 DB가 큼(최대 454M). mutation_guard는 전 cell clean.

CLI/model readiness: claude 2.1.183·codex-cli 0.140.0·opencode 1.17.7, serena 1.5.3·codegraph 1.0.1·codemap available. 전 runtime preflight exit 0.

---

## 7. 권장사항 (이번 3 selected task 범위 안에서만)

n=3/cell·1 task/repo·descriptive 한계 안에서, 관찰된 경향만 기록한다(일반화 금지):

- **claude-sonnet + 어려운 C++/Rust 탐색(no-mcp가 약한 codebase)**: ClickHouse 류에선 serena(LSP), deno(Rust/TS) 류에선 codemap이 no-mcp 대비 win 밴드를 넘었다. codex에서도 순위 방향은 일치(CH=serena, deno=codemap)하나 codex는 전부 tie(밴드 미달). **codebase=task 교락으로 언어/구조 일반화는 금지** — backend 선택은 task별로 결정해야 한다.
- **no-mcp가 이미 강한 task(angular)**: 어떤 MCP도 win 밴드를 못 넘김(전부 tie). claude/codex 모두 동일 결론. 이런 경우 도입비용 대비 효용이 낮으므로 **builtin으로 충분**.
- **효율 우선이면 codegraph**: claude에서 가장 적은 도구 호출(6.0)로 동등 품질대 도달(angular 0.77). 단 DB 디스크 비용이 큼(최대 454M).
- **가장 낮은 도입비용이면 codemap**: read-only·config 최소·디스크 최소, deno에서 claude Δ+0.48 win(codex Δ+0.04 tie — 밴드 미달). codex-codemap=usable.
- **codex-gpt54 MCP 활용**: -02와 달리 codex의 codemap·codegraph는 usable로 교정됐다. 단 sandbox confound와 codex-serena degraded는 유지되므로 serena 배정은 현 시점 효과를 보장하지 않는다.
- **opencode 계열**: 약체·노이즈 레이어. MCP가 오히려 손해인 셀 다수. opencode-serena 신규 데이터(평균 0.151)는 참고 수준이며 과소집계 비대칭을 감안해야 한다.

---

## 8. 한계·캐비엇·계보

- **1 task/repo, 3 round**: cell당 n=3, descriptive only. 추론통계 불가.
- **scorer 한계**: LLM judge(opus), temp 고정 불가→frozen-judge self-consistency로 재정의, ±1/F fact 밴드.
- **codebase=task 교락**: repo당 task 1개라 codebase 효과와 task 특이성 분리 불가.
- **fairness 비대칭**: claude=mutating bash / codex=read-only sandbox / opencode=no-bash → no-mcp baseline 자체가 runtime마다 다른 도구 표면 confound. codex 점수는 usable이지 claude 동급 clean이 아니다.
- **codex-serena=degraded**: 에러 3/9 episode. codex-codemap·codegraph=usable.
- **opencode-serena 과소집계**: 서브에이전트 위임으로 도구수 비대칭.
- **backend_off 24**: 전부 opencode. codex 기여 0(교정 완료).
- **병렬 wall_time 비대칭 부풀림**: tool_calls·token 1차, wall_time 보조.
- **-02 정정 사항**: -02 HALT2에서 사용자 수락한 "codex MCP 비호출(behavioral null)" 결론은 텔레메트리 버그 교정으로 정정됨. -02 수치(N=153)는 현재 데이터셋에 포함되어 있으며 별도 라벨 없이 통합됐다.
- **독립검증**: C-verify 5/5 PASS, persist-fix 4/4 PASS(상세: `phases/result-metric-verify/verify.md`, `phases/integrate/persist_fix_report.md`).

