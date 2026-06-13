# `@buyong-mcp/scout` 코드검색 벤치마크 — Claude Code × Codex 교차검증 (2026-06-04)

> **TL;DR:** scout(zoekt+ctags)를 순정 빌트인 도구(`default`)와 두 독립 에이전트 하베스에서 비교했다.
> **scout-pure(MCP 단독)는 두 하베스 모두에서 default와 품질 동률(parity)**, **scout-add(빌트인+scout)는 Claude에서 동률·Codex에서 유의하게 우세**(Δ F2 +0.075, 95% CI 0 미포함) — 즉 scout는 어느 하베스에서도 품질을 해치지 않고, Codex에서는 도움이 됐다. 대가는 turns·tokens·latency·cost 증가다. 참고 비교군 serena(LSP)는 두 하베스 모두 default 이하이며 Claude에서 유의하게 나쁘다. scout 도구 지연 자체는 매우 빠르다(웜질의 1–50ms, 30k 파일 색인 ~4초).
> ⚠️ **잠정 표기:** Codex의 'scout-add 유의 우세'는 **reps=2 · scout만 별도 배치 rerun · rg 부재 default baseline**라는 3중 미통제 위에 얹혀 있다(§8.4). 차기 측정(§11: N≥3 · same-batch · rg arm 통제)에서 재확인 전까지 **확정 결론이 아닌 잠정 신호**로 읽어야 한다.

---

## 1. 개요 (Overview)

- **목적:** "scout의 zoekt+ctags 코드검색이 코딩 에이전트의 *코드 위치 찾기* 품질을 끌어올리는가"를, **모델·하베스가 서로 다른 두 환경에서 교차검증**한다. 한 하베스의 결과가 우연/특정 모델 효과인지, 아니면 도구의 일반적 성질인지 가르기 위함.
- **비교 대상(baseline / targets):**
  - baseline = **`default`** — 에이전트의 순정 빌트인 파일/검색 도구(read·grep·glob 또는 codex shell)만.
  - target = **`scout-add`**(빌트인+scout MCP) · **`scout-pure`**(scout MCP 단독).
  - 참고 비교군 = **`serena-add`/`serena-pure`**(LSP 기반 심볼 MCP) — scout가 아닌 대안 MCP의 거동 대조용.
- **두 하베스:**
  - **① Claude Code** — in-session Workflow 오케스트레이션, 모델 Claude Sonnet 4.6, **N=3**, 5-arm, 144런.
  - **② Codex** — `codex exec`(헤드리스), 모델 gpt-5.4-mini, **reps=2**(serena-add=4), 4-arm(serena-pure 미실행), 96잡.
- **요약 결과:** §7 결과표 · §8 분석 참조. 핵심은 **두 하베스에서 scout가 default를 유의하게 밑돌지 않는 방향이 일관**되게 나왔다는 점(scout-pure=parity 양쪽 일치, scout-add는 Claude neutral→Codex positive).

> ⚠️ **교차 에이전트 해석 규칙(이 문서 전체에 적용):** Claude와 Codex는 모델뿐 아니라 도구 설계·시스템프롬프트·오케스트레이션이 통째로 다르다. 따라서 **두 하베스의 절대수치를 직접 비교/랭킹하지 않는다**(예: "Codex 0.48 < Claude 0.66" 식 금지). 정당한 비교는 **각 하베스 내부의 `arm − default` 방향과 유의성**이며, 그 방향이 두 독립 하베스에서 일치하는지를 본다(§6.5, §8.1).

---

## 2. 측정 환경 (Environment)

| 항목 | 값 |
|---|---|
| CPU | Apple **M4 Pro** (14코어) |
| RAM / 디스크 | (M4 Pro, 통합메모리) · NVMe SSD |
| OS | macOS **15.7.1** (Darwin 24.6.0) |
| 측정 시 부하 | Track A 측정 시 loadavg **7.28**(고부하 — 절대 ms ~20–30% 부풀 가능) |
| scout 바이너리 | managed **v0.0.3** — zoekt-index/webserver + **Universal Ctags 6.1.0**. PATH 고정(`~/.scout/bin/v0.0.3`)으로 로컬 빌드 혼입 차단 |
| 인덱스 엔진 | zoekt(트라이그램 역색인) + ctags(심볼) |
| 모델 — Claude | `claude-sonnet-4-6` |
| 모델 — Codex | `gpt-5.4-mini` (`codex exec`) |
| serena(참고군) | LSP 기반 심볼 MCP (vscode: tsserver, k8s: gopls) |
| 동시성 | Claude: Workflow 병렬 / Codex: 잡 스케줄러 concurrency 12 |

> ⚠️ **재현성 갭(정직 고지):** 실행 산출물에 **Node 런타임 버전·codex-cli 버전**이 기록되지 않았다. scout/ctags/모델 버전은 고정·확인됐으나, 위 두 항목은 다음 측정에서 manifest에 박아야 한다.

---

## 3. 데이터셋 (Dataset)

인덱싱·탐색 대상은 실제 대형 OSS 두 개를 **고정 SHA**로 핀했다.

| 항목 | vscode | kubernetes |
|---|--:|--:|
| 커밋(SHA) | `64d8ca8` | `4ea9058` |
| 주 언어 | TypeScript | Go |
| 총 파일 수 | **15,610** | **30,689** |
| 코드 LOC (tokei 14.0, 주석·공백 제외) | **3,530,767** | **5,355,924** |
| 총 LOC (tokei, 주석·공백 포함) | 4,454,753 | 6,756,985 |
| 주 언어 LOC | TypeScript 2,429,978 | Go 3,889,004 |
| 콜드 전체 색인 시간(median) | 2,848 ms | 4,071 ms |
| 인덱스(shard) 크기 | 428.4 MB | 616.7 MB |
| 제외 규칙 | `.gitignore` 준수 + 바이너리 제외(zoekt 기본) |

> LOC는 `tokei 14.0`으로 추가 집계했다(2026-06-04, 체크아웃 작업트리 기준, `.gitignore` 준수). **표의 '총 파일 수'(15,610/30,689)와 tokei 인식 파일 수(14,023/25,999)가 다른 이유:** 전자는 zoekt가 색인한 전체 파일(비코드 포함), 후자는 tokei가 인식한 소스 파일만. k8s는 체크인된 `vendor/`(65MB)를 포함하며 이는 rg·scout 양쪽 검색 범위에 **동일하게** 들어가 공정성에 영향 없다. 청크 수는 zoekt가 트라이그램 색인이라 개념이 약해 생략.

---

## 4. 쿼리셋 & 정답 (Queries & Ground Truth)

이 벤치는 "특정 증상/요구가 주어졌을 때 **어디를 고쳐야/이해해야 하는가**"를 묻는 실무형 코드 네비게이션이다. 즉 쿼리=과제, 정답=편집 핵심 지점의 `file:line`.

- **쿼리(과제) 개수·출처:** **12과제**(레포별 6 = 난이도 상/중/하 × 2), 수작업 큐레이션(실무 시나리오 기반). 총 **46 essential anchor**.
- **쿼리 유형:** 자연어 증상 서술(심볼명·파일명을 일부러 노출하지 않음) → 에이전트가 코드를 직접 탐색해 위치를 찾아야 함. 카테고리: **fix(버그수정)·feat(기능추가)·flow(교차흐름)**.
- **정답(ground truth) 정의 방식(순환 편향 차단):** 현 pin에서 코드 정독 → **R0·R2 독립 opus 검증** → **합의 앵커 큐레이션**. ripgrep 단독으로 정답을 만들지 않음(텍스트검색 arm에 유리해지는 순환 방지). 정답은 **복수 앵커**(과제당 2~8개)이며 이해용 read/기계적 지점은 `optionalContext`로 분리(비채점).

| id | repo·cat·난이도 | 앵커 | 시나리오(요약) |
|---|---|--:|---|
| vsc-fix-1 | vscode·fix·하 | 2 | 검색·바꾸기 "대소문자 형태 보존" 옵션 버그 |
| vsc-fix-2 | vscode·fix·하 | 2 | 여러 줄 합치기 시 조각 사이 공백 처리 버그 |
| vsc-feat-1 | vscode·feat·중 | 8 | 줄 주석 처리 동작을 바꾸는 설정값 추가 |
| vsc-feat-2 | vscode·feat·중 | 5 | 줄 알파벳 정렬 비교 기준 변경 |
| vsc-flow-1 | vscode·flow·상 | 4 | 저장 시 자동 정리 동작들의 흐름 |
| vsc-flow-2 | vscode·flow·상 | 5 | 같은 단어 자동 강조 동작 흐름 |
| k8s-fix-1 | kubernetes·fix·하 | 2 | 메모리 압박 축출 시 임계치 분기 버그 |
| k8s-fix-2 | kubernetes·fix·하 | 2 | NotReady/Unreachable taint 흐름 버그 |
| k8s-feat-1 | kubernetes·feat·중 | 3 | kubelet 워크로드 소스 등록 추가 |
| k8s-feat-2 | kubernetes·feat·중 | 3 | scheduler 자원 점수화 방식 추가 |
| k8s-flow-1 | kubernetes·flow·상 | 4 | API admission 검사·거부 흐름 |
| k8s-flow-2 | kubernetes·flow·상 | 6 | scheduler 노드 필터링 단계 흐름 |

---

## 5. 측정 항목 (Metrics)

모든 답은 `{"locations":["repo상대경로:LINE", ...]}` 형식으로 받아 `{file,line}`을 추출, 정답 앵커와 매칭한다.

**검색 품질 (Retrieval Quality)** — 헤드라인은 **F2**.

| 지표 | 정의 | 비고 |
|---|---|---|
| **F2** | recall 가중 조화평균 `5PR/(4P+R)` (β=2) | 헤드라인. "핵심 앵커를 빠짐없이 찾았나" 중시 + 과도 over-return만 약한 precision 페널티 |
| recall | 정답 앵커 중 적중 비율(커버리지) | file:line 매칭 **tol ±3줄** |
| precision | 반환 위치 중 적중 비율 | |
| over-return | 반환 위치 수 ÷ 앵커 수 | 1=정확, >1=과반환(노이즈) |

**성능 (Performance)**

| 지표 | 정의 | 비고 |
|---|---|---|
| Indexing Time | 콜드 전체 색인 소요(median) | Track A |
| Index Size | 디스크상 shard 크기 | Track A |
| Query Latency p50/p90 | scout **도구** 질의 지연 분포(웜) | Track A — 에이전트 무관 |
| E2E Latency p50/p90/p95 | 과제 1런의 **에이전트 e2e** 소요(MCP 부팅·왕복 포함) | Track B — 도구 지연 아님 |

**MCP 특화 (LLM 연동 관점)**

| 지표 | 정의 | 비고 |
|---|---|---|
| Tool Call Success Rate | (성공 MCP 호출 / 전체 MCP 호출) | scout/serena 대상 서버 한정 |
| Token Usage | 1런당 입력/출력 토큰(누적) | 컨텍스트 비용 직결 |
| Context Efficiency (proxy) | precision·over-return으로 대용 | 반환 위치 중 관련 비율(노이즈 역지표) |
| Tool-calls / Turns | 1런당 도구 호출 수 / 모델 발화 수 | within-agent만(하베스 간 의미 다름) |

**측정 항목 정의 원칙:** 숫자는 반드시 지표명·k와 함께. 지연은 평균 대신 분포(p50/p90/p95).

---

## 6. 측정 방법 (Methodology)

### 6.1 반복·통계
- **반복 횟수(N):** Claude **N=3**(전 arm). Codex **reps=2**(serena-add만 4).
- **워밍업:** Track A(도구 지연)는 색인 후 웜질의를 별도 측정(콜드/웜 분리 보고). Track B(에이전트)는 과제별 독립 세션이라 별도 워밍업 없음 — 콜드 MCP 부팅이 e2e latency에 포함됨(명시).
- **통계 처리:** 과제별 run 평균 → 과제평균을 종합값으로. 분산은 **run간 SD**·**과제간 F2 SD** 보고. 유의성은 **paired bootstrap**(과제평균 F2, B=10,000, seed=12345, treat−base 교집합 과제쌍; CI가 0을 포함하지 않으면 유의).
- **이상치 처리:** 없음(제거하지 않음). 빈 답/비파싱은 **DNF로 점수 0 정직 반영**(은폐·제외 안 함).

### 6.2 채점기 (model-agnostic 불변식)
두 하베스가 **데이터셋·정답셋·채점기를 100% 공유**한다. 채점 로직(`harness/locations.mjs`: `extractLocations`+`score`, tol ±3)은 **한 글자도 바꾸지 않았다.** Codex 답은 동일 채점기에 `env="native-codex"` 라벨만 분리해 별 파일로 적재(Claude 결과와 안 섞음).

### 6.3 격리 (arm = 도구 집합)
- Claude: custom agentType **화이트리스트 하드격리**(pure arm은 빌트인 도구 원천 차단). 144런 **격리 위반 0건**.
- Codex: `~/.codex/config.toml`의 MCP 등록/생략 + `shellTool` 토글. scout-pure는 `shellTool:false`로 shell이 **하드 비활성** → shell 호출 0건(소프트격리지만 결과적으로 깨끗).

### 6.4 ⚠️ scout MCP 통합 오류와 재실행 (측정 무결성 이력 — 반드시 고지)
**Codex 초기 실행에서 scout MCP 호출이 사실상 100% 실패**했다(`"user cancelled MCP tool call"` 다수). 즉 초기 Codex의 scout-add/scout-pure는 scout 효용이 아니라 "도구 부재/shell 폴백"을 측정한 무효 데이터였다. **scout 두 arm만 통합 수정 후 재실행**(`logs/…-scout-rerun`)했고, 재실행에서 scout 호출 성공률은 **scout-pure 675/678(99.6%)·scout-add 724/736(98.4%)**로 회복됐다. 본 문서는 **유효 데이터만** 사용한다:
- `default`·`serena-add` = **초기 실행**(MCP 정상이었음).
- `scout-add`·`scout-pure` = **재실행**.
- 두 배치의 **과제 프롬프트는 바이트 단위 동일**함을 확인(공정성). 단, 실행 시점이 달라 **배치-타이밍 교란**이 latency/cost에 있을 수 있음(품질 F2/recall엔 영향 없음) — §8.4 한계.

### 6.5 cross-agent 비교 규칙
§1 박스대로: 두 하베스의 절대수치는 직접 비교 금지. §7 결과는 하베스별 분리 표, §8.1에서 **within-agent Δ의 방향·유의성만** 병치.

### 6.6 벤치 하니스
- Claude: in-session Workflow self-contained 스크립트(`harness/generated/bench-run-native-sonnet-12t-*.js`), per-agent transcript(`agent-*.jsonl`) 파싱.
- Codex: `codex exec` 잡 러너 → 세션 jsonl(`turn.completed.usage`, `item.completed`) + `status.json` 파싱 → 수집기 `harness/collect-codex.mjs`.

---

## 7. 결과 (Results)

> 표는 하베스별로 분리. 각 표의 Δ는 **그 하베스 내부의 `arm − default`**(within-agent, 허용). 두 표를 가로로 빼지 말 것(§6.5).

### 7.1 검색 품질 — Claude Code / Sonnet 4.6 (5-arm, N=3)

| arm | **F2** | ΔF2 vs default° | recall | precision | run간 SD | over-return |
|---|--:|--:|--:|--:|--:|--:|
| **default** (baseline) | **0.663** | — | 0.787 | 0.468 | 0.009 | 2.89 |
| scout-add | 0.655 | −0.009 (−1.4%) | **0.791** | 0.460 | 0.012 | 3.70 |
| scout-pure | 0.646 | −0.017 (−2.6%) | 0.766 | 0.464 | 0.027 | 3.48 |
| serena-add¹ | 0.589 | −0.098† | 0.707 | 0.400 | 0.017 | 2.75 |
| serena-pure¹ | 0.574 | −0.113† | 0.671 | 0.390 | 0.058 | 2.06 |

° Δ = **§7.3 paired bootstrap obs Δ**(같은 과제집합 쌍대비). ¹ serena 두 arm은 **vscode 전용(n=6)** — kubernetes는 gopls workspace-symbol 타임아웃 DNF. † serena F2(6과제)는 default의 12과제 F2(0.663)와 직접 비교 불가 → Δ는 default를 같은 6과제로 제한한 쌍대비값(§7.3).

### 7.2 검색 품질 — Codex / gpt-5.4-mini (4-arm, reps=2; scout=재실행)

| arm | **F2** | ΔF2 vs default° | recall | precision | run간 SD | over-return |
|---|--:|--:|--:|--:|--:|--:|
| **scout-add** | **0.556** | **+0.075 (+15.6%)** | **0.600** | **0.525** | 0.021 | 1.46 |
| default (baseline) | 0.481 | — | 0.526 | 0.461 | 0.043 | 1.56 |
| serena-add¹ | 0.464 | −0.008† | 0.511 | 0.427 | 0.098 | 1.47 |
| scout-pure | 0.453 | −0.028 (−5.8%) | 0.491 | 0.441 | 0.029 | 1.50 |

° Δ = **§7.3 paired bootstrap obs Δ**. ¹ serena-add=vscode 전용(n=6), reps=4. † serena F2(6과제)는 default 12과제 F2와 직접 비교 불가 → Δ는 같은 6과제 쌍대비값(§7.3).

### 7.3 통계 유의성 — paired bootstrap (과제평균 F2, B=10,000, seed=12345)

| 대비 | Claude: Δ · 95% CI · 판정 | Codex: Δ · 95% CI · 판정 |
|---|---|---|
| scout-pure − default | −0.017 · [−0.077, +0.028] · **유의X (parity)** | −0.028 · [−0.099, +0.036] · **유의X (parity)** |
| scout-add − default | −0.009 · [−0.038, +0.022] · **유의X (parity)** | +0.075 · [**+0.004**, +0.136] · **유의 (default보다 나음)** |
| serena-add − default | −0.098 · [−0.212, **−0.005**] · **유의 (나쁨)** | −0.008 · [−0.121, +0.105] · 유의X |
| serena-pure − default | −0.113 · [−0.224, **−0.009**] · **유의 (나쁨)** | (Codex 미실행) |

### 7.4 성능 — E2E Latency & 효율 (per-run)

> E2E 백분위는 per-run `latencyMs`에 **nearest-rank**(`ceil(p/100·n)`)로 산출. n=Claude 36(serena 18)·Codex 24라 p99는 표본 부족으로 생략.

**Claude / Sonnet 4.6**

| arm | tool-calls | turns | tok in | tok out | cost(USD) | E2E p50 / p90 / p95 (s) |
|---|--:|--:|--:|--:|--:|--:|
| default | 17.64 | 21.28 | 417,638 | 2,080 | **0.277** | **60 / 123 / 154** |
| scout-add | 18.28 | 23.58 | 526,135 | 2,135 | 0.347 | 73 / 129 / 135 |
| scout-pure | 21.08 | 27.92 | 500,992 | 2,278 | 0.356 | 89 / 172 / 181 |
| serena-add¹ | 16.39 | 20.44 | 318,348 | 2,018 | 0.233 | 66 / 130 / 132 |
| serena-pure¹ | 17.39 | 23.78 | 346,532 | 1,943 | 0.255 | 101 / 165 / 212 |

**Codex / gpt-5.4-mini** (scout 행=재실행 배치)

| arm | tool-calls | turns² | tok in | tok out | cost(USD)³ | E2E p50 / p90 / p95 (s) |
|---|--:|--:|--:|--:|--:|--:|
| default | 25.25 | 7.58 | 829,005 | 25,843 | 0.089 | 288 / 388 / 474 |
| scout-add | 33.42 | 7.50 | 979,161 | 26,103 | 0.096 | 375 / 484 / 528 |
| scout-pure | 28.25 | 5.33 | 1,028,876 | 25,273 | 0.092 | 342 / 549 / 614 |
| serena-add¹ | 37.33 | 6.29 | 1,039,993 | 23,122 | 0.082 | 303 / 484 / 491 |

² Codex "turns"=모델 발화(agent_message) 수로 Claude turns와 의미가 달라 **두 하베스 turns 직접비교 불가**(within-agent만). ³ **gpt-5.4-mini 단가 미확정 가정값**(mini-tier 근사: in $0.25 / cached-in $0.025 / out $2.00 per MTok). 토큰은 정확 측정, cost는 파생·예시값.

### 7.5 성능 — scout 도구 지연 (Track A, 에이전트 무관 · 직접 측정)

| 지표 | vscode (15,610 파일) | kubernetes (30,689 파일) |
|---|--:|--:|
| 콜드 전체 색인 (median) | 2,848 ms | 4,071 ms |
| 콜드 첫 질의 | 3,510 ms | 4,097 ms |
| 웜질의 p50 — rare / common / regex / langFilter | 1.2 / 18.7 / 35.1 / 2.0 ms | 0.6 / 50.2 / 2.2 / 0.4 ms |
| 웜질의 overall **p50 / p90** | **10.2 / 35.2 ms** | **1.4 / 50.3 ms** |
| 무변경 recheck (p50) | 334.9 ms | 467.8 ms |
| 1파일 touch 재색인 (p50) | 3,417 ms | 4,960 ms |

→ 30k 파일 색인 ~4초, 웜질의 전부 sub-50ms. **단, 1파일 변경에도 전체 재색인(~3–5초)** — 증분 색인 부재.

### 7.6 MCP 특화 — Tool Call Success / Context Efficiency

| arm | Tool Call Success(Codex)⁴ | over-return(Cl / Cx) | precision(Cl / Cx) |
|---|--:|--:|--:|
| scout-add | 724/736 = **98.4%** | 3.70 / 1.46 | 0.460 / 0.525 |
| scout-pure | 675/678 = **99.6%** | 3.48 / 1.50 | 0.464 / 0.441 |
| serena-add | 876/876 = **100%** | 2.75 / 1.47 | 0.400 / 0.427 |
| default | (MCP 없음) | 2.89 / 1.56 | 0.468 / 0.461 |

⁴ Codex 재실행 기준. **초기 실행에서는 scout 호출 성공률 0%**(통합 오류, §6.4) → 재실행으로 ~99% 회복. Claude에서는 scout/serena 도구 호출 정상.
**Context Efficiency 해석:** over-return은 Claude(2~3.7배)가 Codex(~1.5배)보다 높으나 이는 **하베스의 반환 성향 차이**(within-agent 비교용). scout가 noise를 특별히 늘리지는 않음(arm 간 over-return 일관).

### 7.7 분해 (repo · category · task)

**repo별 F2**

| arm | Cl k8s | Cl vsc | Cx k8s | Cx vsc |
|---|--:|--:|--:|--:|
| default | 0.640 | 0.687 | 0.489 | 0.473 |
| scout-add | 0.614 | 0.695 | **0.574** | **0.538** |
| scout-pure | 0.590 | 0.703 | 0.460 | 0.446 |
| serena-add¹ | — | 0.589 | — | 0.464 |

**category별 F2** (난이도 서열 fix<feat<flow가 두 하베스 공통으로 재현)

| arm | Cl feat / fix / flow | Cx feat / fix / flow |
|---|--:|--:|
| default | 0.609 / 0.954 / 0.427 | 0.432 / 0.653 / 0.357 |
| scout-add | 0.591 / 0.968 / 0.405 | 0.497 / 0.704 / **0.466** |
| scout-pure | 0.557 / 0.962 / 0.420 | 0.416 / 0.704 / 0.239 |
| serena-add¹ | 0.460 / 0.934 / 0.373 | 0.333 / 0.630 / 0.429 |

**과제별 F2 (task × arm) — 두 하베스 병치**

| task | 앵커 | Cl default | Cl scout-add | Cl scout-pure | Cx default | Cx scout-add | Cx scout-pure |
|---|--:|--:|--:|--:|--:|--:|--:|
| k8s-feat-1 | 3 | 0.874 | 0.833 | 0.588 | 0.705 | 0.788 | 0.598 |
| k8s-feat-2 | 3 | 0.397 | 0.430 | 0.502 | 0.466 | 0.456 | 0.435 |
| k8s-fix-1 | 2 | 1.000 | 1.000 | 1.000 | 0.556 | 0.556 | 0.556 |
| k8s-fix-2 | 2 | 1.000 | 1.000 | 1.000 | 0.778 | 1.000 | 0.778 |
| k8s-flow-1 | 4 | 0.490 | 0.421 | 0.448 | 0.289 | 0.426 | 0.322 |
| k8s-flow-2 | 6 | 0.076 | 0.000 | 0.000 | 0.139 | 0.216 | 0.074 |
| vsc-feat-1 | 8 | 0.381 | 0.296 | 0.362 | 0.124 | 0.130 | 0.125 |
| vsc-feat-2 | 5 | 0.785 | 0.804 | 0.776 | 0.435 | 0.615 | 0.508 |
| vsc-fix-1 | 2 | 0.939 | 0.943 | 0.968 | 0.778 | 0.556 | 0.778 |
| vsc-fix-2 | 2 | 0.875 | 0.928 | 0.879 | 0.500 | 0.705 | 0.705 |
| vsc-flow-1 | 4 | 0.423 | 0.519 | 0.490 | 0.239 | 0.357 | 0.119 |
| vsc-flow-2 | 5 | 0.719 | 0.681 | 0.743 | 0.762 | 0.863 | 0.441 |

(Codex serena-add, vscode 6과제만: vsc-feat-1 0.148, vsc-feat-2 0.519, vsc-fix-1 0.556, vsc-fix-2 0.705, vsc-flow-1 0.265, vsc-flow-2 0.594.)

---

## 8. 분석 (Analysis)

### 8.1 주요 발견 — cross-agent 방향 일치 (절대값 아닌 부호·유의성)

| 대비 | Claude | Codex | 두 하베스 방향 |
|---|---|---|---|
| **scout-pure vs default** | parity (Δ−0.017, 유의X) | parity (Δ−0.028, 유의X) | ✅ **둘 다 동률** — 강한 일치 |
| **scout-add vs default** | parity (Δ−0.009, 유의X) | **우세 (Δ+0.075, 유의)** | ✅ **둘 다 ≥0** — neutral→positive, 해롭지 않음 |
| **serena-add vs default** | **나쁨 (Δ−0.098, 유의)** | 이하 (Δ−0.008, 유의X) | ✅ **둘 다 ≤0** — serena 우위 없음 |

- **scout-pure(MCP 단독)는 두 독립 하베스 모두에서 default와 통계적 동률.** zoekt+ctags 검색만으로도 빌트인 grep+read와 품질이 같다 — 빌트인 파일 도구 없이 MCP만 쓰는 환경에서도 품질 손실이 없다는 뜻.
- **scout-add(빌트인+scout)는 어느 하베스에서도 품질을 해치지 않으며, Codex에선 유의하게 도움**이 됐다(Δ+0.075, CI 0 미포함; recall 0.526→0.600, precision 0.461→0.525 동반 상승). 특히 Codex의 **flow(교차흐름) 0.357→0.466, feat 0.432→0.497**에서 scout가 분산된 지점 발견을 도왔다.
- **serena(LSP)는 두 하베스 모두 default 이하**, Claude에선 유의하게 나쁨. "분산된 편집지점 찾기"에 심볼 그래프 탐색이 우위를 주지 못한다는 신호가 양쪽에서 같은 방향.

### 8.2 왜 하베스마다 scout-add 효과가 다른가 (가설)
- **Claude(parity):** Sonnet의 빌트인 grep+read 전략이 이미 강해, 이 규모(15k~31k 파일)에서 scout가 더 줄 게 적다. scout는 turns·latency만 더했다(§8.3).
- **Codex(우세):** gpt-5.4-mini는 default(shell) 단독에서 *그 하베스 내부* recall·precision이 더 낮았고, scout의 구조적 검색(zoekt 정규식·ctags 심볼)이 부족한 탐색을 **보강**해 recall(0.526→0.600)·precision(0.461→0.525)을 함께 끌어올렸다. 즉 scout의 한계효용은 **빌트인 탐색이 약한 에이전트에서 더 크다**는 가설.

### 8.3 트레이드오프 (솔직하게)
- **품질↑/동률의 대가는 비용↑.** scout는 두 하베스 모두 **tool-calls·tokens·E2E latency·cost를 높인다.**
  - Claude: scout-pure가 default 대비 turns +31%·E2E p50 +48%(89s vs 60s)·cost +29%, **품질 이득은 0**(동률).
  - Codex: scout-add가 tool-calls 25.3→33.4·tok in 829k→979k·E2E p50 288→375s·cost $0.089→$0.096, **대신 품질 +15.6%**(유의) — 여기선 값을 한다.
- **이 latency는 도구 속도가 아니다.** Track A에서 scout 웜질의는 1–50ms로 매우 빠르다(§7.5). E2E 격차는 MCP 왕복·에이전트가 scout로 **더 많은 탐색 라운드**를 도는 데서 온다.
- **증분 색인 부재(이 벤치엔 미해당):** zoekt-index에 `-delta`/`-incremental`이 없어 1파일 변경에도 전체 재색인(~3–5초)이다(scout 자체 한계, `DESIGN §6.1` 실측). **단 이 벤치는 읽기전용 위치찾기라 편집이 없고, Track B는 `.scout/zoekt`를 wipe하지 않아 인덱스가 영속(세션 간 재사용)** → 최초 레포 접촉 1회만 콜드 빌드, 이후 전부 웜. 즉 이 비용은 *편집 루프* 사용처의 caveat이지 본 벤치의 latency/F2엔 영향 없으며, scout E2E 증가분은 재색인 artifact가 아니라 **실제 탐색 라운드 증가**가 본질이다.

### 8.4 한계 / 편향 가능성
- **scout 재실행 교란:** scout 두 arm은 초기 MCP 오류로 무효 → **수정 후 재실행** 데이터(§6.4). 프롬프트는 바이트 동일하나 **실행 배치·시점이 default/serena와 다르다.** 품질(F2/recall/precision)은 타이밍 무관이지만, **E2E latency·cost의 within-Codex 비교는 배치-타이밍 caveat**를 안고 본다.
- **단일 모델 ×2(Sonnet 4.6 / gpt-5.4-mini).** 다른 모델에서 scout-add 효과의 부호가 바뀔 수 있음(가설 8.2).
- **표본 작음:** scout 비교 n=12 과제, serena 비교 n=6(vscode). bootstrap CI가 넓음 — "parity"는 "효과 없음 입증"이 아니라 "이 표본에서 유의차 미검출".
- **Codex reps=2 < 권고선(N≥3).** Claude는 reps=1이 거짓 신호를 줬고(scout-pure가 reps=1에서 명목 1위→N=3에서 3위) N=3에서 교정됨을 직접 보였다 → Codex 단독 결론(특히 scout-add 유의)은 **N을 늘려 재확인** 권장.
- **k8s-serena DNF**(gopls 타임아웃) → serena 결론은 vscode 한정(두 하베스 공통). serena-pure는 Codex 미실행.
- **단일 유의 결과의 confound 수렴:** 0을 배제한 유일한 양성(Codex scout-add +0.075)이 reps=2·배치분리 rerun·rg 부재 default 위에 **동시에** 얹혀 있다. 개별 고지는 했으나 하필 헤드라인에 수렴 → §11 통제로 재확인 전 "잠정".
- **Codex 환경 미동결:** 모델 reasoning effort, `~/.codex/config.toml`의 arm 토글 스냅샷, codex-cli/Node 버전이 배치별로 기록되지 않았다(현 config는 이미 `gpt-5.5`/`xhigh`로 변동). 위 배치 confound와 결합 시 배치 간 환경 drift를 **배제할 수 없다** — §6.4의 "F2는 타이밍 무관" 주장은 환경 동일성 가정에 의존.
- **Codex 과제 격리 미명시:** Claude는 과제당 독립 sub-agent지만 Codex가 과제당 새 세션인지 한 세션에 묶었는지 문서에 없다(CODEX-HANDOFF은 묶음 시 앞 과제 맥락 누수를 경고). 묶였다면 per-task 토큰·latency·F2가 교란된다.
- **DNF 채점 비대칭:** 빈 답은 0점인데 serena k8s 색인실패는 채점에서 제외 → serena가 최난도 repo를 면제받아 유리해진다. 차기엔 동일 규칙(0점 또는 본문 명시).
- **헤드라인 지표 편향:** F2(β=2)는 recall 4배 가중이라 over-return이 큰 도구(scout 3.5~3.7 vs default 2.9)를 약하게만 페널티한다 → F1·Fβ-sweep 병기 필요. 과제 동일가중(앵커 2~8개 무관)도 anchor-pooled micro-average로 교차검증 필요.
- **인덱스 상태 manifest 미기록:** 측정 시작 시 인덱스가 콜드/사전빌드였는지, `.scout/zoekt` 영속 여부가 산출물에 없다. ②(영속)를 고려하면 인덱스 상태는 1급 실험변수 → 차기 manifest 필수.
- **중간 규모 레포:** 15k~31k 파일에서 ripgrep이 이미 충분히 빠르다. scout의 색인 우위는 이 벤치가 다루지 않은 **초대형 레포·느린 FS·grep 부재 환경**에서 더 드러날 수 있다.
- **gpt-5.4-mini 단가 미확정 / Codex "turns"≠Claude "turns" / Track A 절대값 ~20–30% 부풀 가능(loadavg 7.28)** — §7 각주.
- **cherry-picking 방지:** 불리한 지표(scout의 비용↑, Claude parity, serena DNF, 초기 MCP 실패)를 모두 본문에 명시했다.

---

## 9. 재현 방법 (Reproduction)

```bash
# 0) 전제: repos/ 고정 SHA 체크아웃(vscode 64d8ca8, k8s 4ea9058),
#    scout managed v0.0.3 PATH 고정(~/.scout/bin/v0.0.3), groundtruth.json(v2) 준비.
cd /Users/buyong/scout-bench

# 1) Track A — scout 도구 지연(콜드 색인/웜질의/재색인)
bash harness/perf-run.sh            # → results/perf-native.json

# 2) Track B (Claude) — in-session Workflow 5-arm × 12과제 × N=3
#    self-contained 워크플로 스크립트 실행 → results/trackB-native-v2-n3.jsonl
#    (harness/generated/bench-run-native-sonnet-12t-{1r,2r-from2}.js)

# 3) Track B (Codex) — codex exec 4-arm × 12과제 × reps
#    arm별 ~/.codex/config.toml의 scout/serena MCP 활성/비활성 후 잡 러너 실행.
#    ⚠️ scout 두 arm은 MCP 통합 확인 후 실행(초기 오류 시 재실행).
#    로그: logs/codex-exec-gpt-5.4-mini-2026-06-04/(default,serena-add)
#          logs/codex-exec-gpt-5.4-mini-scout-rerun/(scout-add,scout-pure)

# 4) Codex 원답 수집 → 채점기 호환 jsonl (arm별 소스 디렉터리는 collect-codex.mjs에 매핑)
node harness/collect-codex.mjs      # → results/trackB-codex-n3.jsonl (96레코드)

# 5) 채점 (Claude·Codex 분리, 채점 로직 동일)
node harness/score-trackB-multi.mjs results/trackB-native-v2-n3.jsonl   # Claude
node harness/score-trackB-multi.mjs results/trackB-codex-n3.jsonl --out=scores-trackB-codex-n3   # Codex
```

검증 sanity: `wc -l results/trackB-codex-n3.jsonl` = 96 · Codex no-result = 0 · scout MCP 성공률 ≥98% · Claude 수치는 `REPORT.md`/`scores-trackB-v2-n3.md`와 표 대조 일치.

---

## 10. 부록 (Appendix)

**원시 데이터·산출물 경로**
- 정답셋: `groundtruth.json`(v2, 12과제/46앵커) · 구축 근거 `results/gt-anchors.json`, `gt-validation{,-v2}.json`.
- 채점기(불변): `harness/locations.mjs`(`extractLocations`+`score`, tol±3) · 집계 `harness/score-trackB-multi.mjs`.
- Claude: 원시 `results/trackB-native-v2-n3.jsonl`(144런) · 점수 `results/scores-trackB-v2-n3.{json,md}` · 격리감사 `results/audit-v2-n3.json` · 정본 `REPORT.md`.
- Codex: 수집기 `harness/collect-codex.mjs`(신규) · 적재 `results/trackB-codex-n3.jsonl`(96, scout=재실행) · 점수 `results/scores-trackB-codex-n3.{json,md}`.
- Codex 세션 로그: `logs/codex-exec-gpt-5.4-mini-2026-06-04/`(default·serena-add) · `logs/codex-exec-gpt-5.4-mini-scout-rerun/`(scout-add·scout-pure).
- Track A: `results/perf-native.json`(+ vscode/kubernetes 분리본).

**실행 규모**
- Claude: 144런, $44.06, 64.3M 토큰, 2,660 tool-calls(Sonnet 단가 근사). Codex: 96잡(scout 48 재실행 포함), cost는 §7 각주의 가정 단가에서 파생.

**Codex 4-arm within-agent 순위(참고 — 절대 교차랭킹 아님):** ① scout-add 0.556 · ② default 0.481 · ③ serena-add 0.464¹ · ④ scout-pure 0.453.
**Claude 5-arm within-agent 순위:** ① default 0.663 · ② scout-add 0.655 · ③ scout-pure 0.646 · ④ serena-add 0.589¹ · ⑤ serena-pure 0.574¹. (¹ vscode n=6. **두 순위표를 합치거나 가로로 비교하지 말 것** — §6.5.)

---

## 11. 차기 측정 — 공정성 헌장 + 계획 (Next Measurement)

> 이 절은 **차기 측정**의 규칙·계획이다(과거 결과 수정 아님). 최상위 원칙: **측정은 무조건 공정하다. scout에 추가점수를 주는 어떤 행동도 하지 않는다.**

### 11.1 공정성 헌장 7조 (전 측정에 무조건 적용)

1. **품질·비용 분리.** F2/recall/precision은 인덱스·워밍업과 무관해야 한다(사전워밍이 F2를 바꾸면 그것이 버그 — 검증 필수).
2. **1회성 셋업 비용은 모두 동일 규칙으로 별도·명시 보고.** scout 색인 빌드·serena LSP onboarding·rg=0. **silent exclude도 silent include도 금지.** latency는 cold-start와 warm/steady-state **두 레짐 모두 라벨 달아** 보고.
3. **baseline 약화 금지.** rg arm은 진짜 rg(PATH 보장 + "사용 가능" 고지). 어떤 arm에도 한쪽만 유리한 프롬프트/도구설명을 주지 않는다 — 시스템프롬프트 골격 동일(현 `bench-run.js buildPrompt`가 arm 무관인 점 유지).
4. **동일 환경·동일 배치.** 전 arm을 **같은 배치**에서, 같은 codex-cli/Node/모델 스냅샷·reasoning effort로 실행하고 버전을 manifest에 기록. (scout만 별도 배치 rerun = 위반 소지 → 통합.)
5. **pre-registration.** 새 레포·SHA·ground truth를 **결과 보기 전에 동결.** "scout가 잘 나오는 레포" 선택 금지. GT는 scout/rg/serena **어느 도구 출력에서도** 만들지 않는다(tool-agnostic 수작업 + 독립 검증).
6. **DNF·실패 동일 채점·미은폐.** serena 색인실패, scout 초기 MCP 실패 등 불리한 사건도 본문에. 빈 답 0점 규칙을 도구 실패에도 동일 적용.
7. **피측정 도구의 선호를 채점규칙으로 삼지 않는다.** `DESIGN §6` "벤치 비용은 색인시간 제외" 같은 scout 자신의 선호가 있어도, 공정 벤치는 색인 비용을 숨기지 않고 별도 보고한다.

### 11.2 인덱스/워밍업 공정 처리 (②의 정정된 결론)

- **사실(정정):** Track B는 `.scout/zoekt`를 wipe하지 않아 인덱스가 디스크에 영속된다 → scout는 레포당 **최대 1회** 콜드 빌드, 이후 웜. 따라서 "매 세션 재색인"은 일어나지 않으며, scout E2E 증가분은 재색인 artifact가 아니라 **실제 탐색 라운드 증가**가 본질이다(scout에 불리한 방향이지만 사실).
- **공정 조치:** ⓐ 타임드 런 **시작 전** 전 레포 인덱스를 사전빌드(`perf-run.sh`의 cold-index 경로 재사용)해 **단일 과제가 1회성 빌드를 우발적으로 떠안지 않게** 한다. ⓑ 인덱스 상태("commit X 사전빌드, N세션 영속, 타임드 런 중 `rebuilt:true` 0회")를 로그로 검증·manifest 기록. ⓒ 1회성 빌드 비용(Track A 2.8s/4.0s)은 **항상 별도 라인 노출.** serena도 동일하게 LSP onboarding 시간을 사전·별도 처리.

### 11.3 데이터셋 추가 — tikv (pre-registered)

| 항목 | 값 |
|---|---|
| 레포 | **tikv/tikv** (Rust 멀티크레이트 모노레포) |
| 핀 SHA | 측정 전 동결(미정 → 확정 시 기록) |
| 목적 | 언어 다양성(TS·Go→Rust) + 멀티크레이트 모노레포 구조. Rust 매크로·trait dispatch는 텍스트검색·ctags 양쪽을 까다롭게 시험 |
| 규모 | 클론 후 tokei로 확정 기록(현 vscode 3.53M / k8s 5.36M 코드 LOC와 동급 비교) |
| GT | fix/feat/flow 난이도 상/중/하 큐레이션, R0·R2 독립검증, tool-agnostic |

> 현 vscode/k8s는 이미 3.5M/5.4M 코드 LOC라 "줄 수 증대"가 목적이 아니다 — tikv는 **언어·구조 다양성**이 목적. rg가 실제로 느려지는 레짐(10M+ LOC, 느린/원격 FS)은 별도 후속.

### 11.4 우선순위 (공정성 가드 부착)

| 우선 | 항목 | 공정성 가드 |
|---|---|---|
| **P0** | rg arm 추가(Codex 필수) | rg PATH 보장 + arm 무관 프롬프트(3조) |
| **P0** | 전 arm same-batch 재실행 + 환경/버전 동결 기록 | 배치 confound 제거(4조) |
| **P0** | latency cold/warm 분리 + 인덱스 사전워밍·상태기록 | 색인비용 미은폐(2·7조) |
| **P1** | Codex N≥3 + 과제당 1세션(per-task 격리) | Claude와 격리 대칭 |
| **P1** | Claude tool-call success/serena 분해 = Codex granularity | 보고 대칭 |
| **P1** | DNF 일관 채점 / Codex serena-pure 실행 | 동일 규칙(6조) |
| **P2** | tikv 추가(pre-register + tokei) | cherry-pick 금지(5조) |
| **P2** | F1·Fβ-sweep / anchor-pooled micro-avg 병기 | 지표가 결론을 만들지 않게 |

### 11.5 하베스 구조(불변)

- **Claude:** Workflow(`harness/bench-run.js`)에서 **다중 sub-agent를 `agent()`로 직접 실행**(arm별 `bench-*` agentType 하드격리). 외부 스크립트 구동 아님. **jsonl 적재만** `finalize.mjs`/수집 스크립트 담당.
- **Codex:** arm = `~/.codex/config.toml` MCP 등록/생략 + shell 토글. 과제당 1세션 실행 → 세션 jsonl → `collect-codex.mjs`로 동일 스키마 적재.
- **공유 불변:** `groundtruth.json` · `harness/locations.mjs`(채점기, tol±3) 한 글자도 안 바꾼다.
