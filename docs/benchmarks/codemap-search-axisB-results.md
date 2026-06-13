# codemap-search 축 B — 에이전트 E2E 위치탐색 실측 (2026-06-10)

> 과제: "이 기능을 구현하려면 어디를 고쳐야 하나"를 `file:line`으로 답(edit-site localization).
> 3 arm × 3 rep × corpus 2 = **18 런**, **sonnet**, 병렬. 헤드라인 = **비용(토큰)+행동**, F2는 정확도 게이트.
> 데이터셋: `codemap-search-fd-dataset.json`(fd-feat-1), `codemap-search-ripgrep-dataset.json`(rg-feat-1, 본 작업서 큐레이션).
> 원자료: `artifacts/axisB-{fd,rg}-runs.json`. 채점: `harness/score_axisB.py` (본 문서 기준 상대 경로).

## 측정 환경
- contestant 모델 **sonnet**(Agent override). codemap 서버: corpus별 scoped stdio(`codemap-search-fd`/`-ripgrep`, cwd=corpus), release 바이너리, 인덱스 pre-warm.
- corpus: fd@`25461e5`(23파일/6.8k LOC, parity 티어), ripgrep@`82313cf`(100파일/39k LOC, 멀티크레이트, 변별 티어).
- arm: **baseline**(`bench-default`, Read/Glob/Grep/Bash) · **pure**(`bench-codemap-pure`, codemap 5도구만, Bash 없음) · **additive**(`bench-codemap-add`, 빌트인+codemap).
- 토큰·tool_calls는 Agent 결과 `<usage>`(subagent_tokens·tool_uses)에서 직접 캡처.
- **동시성 검증**: corpus당 codemap 에이전트 6개가 단일 stdio 서버를 동시 호출 — 응답 garbling 없이 깨끗이 multiplex(9/9 유효 JSON).

## 결과 (보정 앵커, n=3 평균)

### fd (parity 티어, 23파일)
| arm | recall | precision | F2 | over-return | **tokens** | tool_calls |
|---|--:|--:|--:|--:|--:|--:|
| baseline | 1.00 | 0.75 | 0.94 | 1.0 | **19,412** | 10.7 |
| pure | 1.00 | 0.75 | 0.92 | 1.0 | **20,871** (+7.5%) | 15.3 |
| additive | 1.00 | 0.67 | 0.89 | 1.3 | **25,221** (+30%) | 14.3 |

### ripgrep (변별 티어, 100파일/39k LOC)
| arm | recall | precision | F2 | over-return | **tokens** | tool_calls |
|---|--:|--:|--:|--:|--:|--:|
| baseline | 1.00 | 1.00 | 1.00 | 0.0 | **34,303** | 21.3 |
| pure | 1.00 | 0.88 | 0.97 | 1.3 | **53,130** (+55%) | 22.7 |
| additive | 0.83 | 0.77 | 0.82 | 1.7 | **52,405** (+53%) | 25.7 |

원자료(tokens): fd-base 16605/25880/15752 · fd-pure 19122/20852/22640 · fd-add 29679/21206/24777 ·
rg-base 36154/34755/32000 · rg-pure 53786/56919/48685 · rg-add 52124/65606/39486.

### raw 앵커(보정 전) — robustness 감사용
| corpus | arm | recall | F2 |
|---|---|--:|--:|
| fd | baseline / pure / additive | 0.50 / 0.50 / 0.33 | 0.50 / 0.48 / 0.32 |
| ripgrep | baseline / pure / additive | 0.75 / 0.67 / 0.58 | **0.76 / 0.66 / 0.55** |

→ **보정 전 raw에선 baseline 우위가 오히려 더 컸다**(rg F2 0.76 vs pure 0.66). 보정(diff 편집범위=함수/구조체 경계로 확장, arm 동일 적용)은 codemap에 **유리한** 방향이었고, 그럼에도 순위·결론 불변. "데이터 보고 앵커 조작" 혐의를 차단.

### ripgrep "stats 강제 활성화"(가장 비자명한 앵커) arm별 적중
| arm | 적중 | 메커니즘 |
|---|--:|---|
| baseline | **3/3** | grep/read로 Count→stats 비활성 흐름을 추적, 게이트 도달 |
| pure | **3/3** | codemap search/read로 동일 추적(hiargs:1250/1254) |
| additive | **1/3** | 2개 rep은 from_low_args 배선(hiargs:251/280)을 게이트로 오인 — 두 도구셋 병행이 가장 어려운 지점에서 산만 |

## 실측이 지지하는 결론 (스코프: keyword-greppable edit-site 과제)

> **스코프 경계(헤드라인과 동급):** 이 과제군의 edit-site는 `Count`/`stats`/`FLAGS`/`quiet` 등 **검색가능 심볼·플래그명**으로 특정된다. 결론은 이 조건에 한정되며, codemap의 가설적 강점(키워드를 모를 때의 **구조적 네비게이션**, BM25 심볼/docstring 검색)은 **본 벤치가 시험하지 않았다.**

1. **codemap은 빌트인에 추가 정확도를 주지 않는다(중복, redundant).** 보정 후 recall: fd 전 arm 1.00, rg baseline/pure 1.00·additive 0.83. 비자명 핵심 앵커도 baseline 3/3·pure 3/3 동률 적중. **pure는 codemap 단독으로 baseline 정확도를 매칭** — 도구가 부정확한 게 아니라, 이미 강한 grep 위에서 **변별을 못 만든다.**

2. **그 매칭을 토큰 ~1.5×로 산다.** ripgrep pure +55%, additive +53%(34.3k → 53.1k/52.4k). 분산 비중첩(baseline 32–36k vs codemap 48–66k) → n=3에서도 견고. codemap의 구조화 응답(overview·search 결과)이 컨텍스트를 더 먹는다. **즉 "더 비싼 동률".**

3. **additive(빌트인+codemap 동시)는 Pareto 열위 — "열등"이 아니라 "산만".** baseline이 가진 걸 전부 + codemap을 더 줬는데 recall↓(0.83)·over-return↑·tool_calls↑(25.7)·토큰↑. 위 stats-gate 1/3이 메커니즘: 두 도구셋 병행이 **가장 어려운 지점에서 오히려 탐색을 흩뜨린다.**

4. **이 과제군에서 baseline이 Pareto 최적.** 정확도 ≥ codemap이면서 토큰 최소. 결론은 "codemap이 나쁘다"가 아니라 **"greppable edit-site 과제에선 순정 빌트인으로 충분하고, codemap을 얹으면 비용·노이즈만 추가(중복)"**.

## 한계·해석 경계 (과대해석 금지)

- **과제가 키워드-grep 친화적(스코프 핵심).** 위 헤드라인 스코프 경계 참조. "codemap이 항상 열위/중복"이 아니라 "**greppable edit-site 과제에선** 빌트인으로 충분"이 정확한 진술.
- **비용 프리미엄에 고정 스키마 오버헤드 포함(미분해).** codemap arm 에이전트는 매 컨텍스트에 도구 스키마 10개 + MCP instructions 2벌(fd+ripgrep)을 싣는다(빌트인은 상시 존재라 "공짜"). +55% 중 *per-use 응답 verbosity* vs *고정 도구정의 비용*의 분해는 미측정 — 후자 비중이 크면 더 큰 과제에서 amortize되어 격차가 줄 수 있다. 또 `subagent_tokens`가 output-only인지 input 누적인지 미확정. 단, **어느 쪽이든 codemap이 더 비쌌다는 방향은 불변.**
- **parity 경고(fd):** fd는 23파일이라 도구 무관 동률 예상(데이터셋 명시) — fd 수치는 변별 신호 아님. ripgrep이 본 비교의 무게중심.
- **n=3 분산.** 정확도·precision은 rep 변동 있음(특히 over-return). **비용 격차만 분산을 명확히 초과**(비중첩) → 추가 런 불필요.
- **행동 분해 부재.** Agent 결과는 tool_uses **총수**만 줌(도구 타입별 분해 불가). "pure가 search/grep을 어떻게 섞나"의 정량은 미측정 — tool_calls 총수와 서사적 관찰만.
- **앵커 보정(arm 중립):** 원본 region 범위가 타이트해 정당한 위치를 1~3줄 차이로 놓침. diff 헌크 편집범위=함수/구조체 경계로 확장(전 arm 동일 적용, 데이터셋 `calib_note` 기록). 위 raw 표가 보정 robustness를 감사 가능하게 한다.

---

# 2라운드: context-gathering (착수 컨텍스트 수집) — scrapy (2026-06-10)

> 회사 벤치("X가 어떻게 동작하는지 조사") 재현 시도. edit-site와 **다른 task class**: 정답 = 작업 착수에 필요한 구조적 컨텍스트(호출 체인) 집합. 헤드라인 = **context recall @ 토큰**.
> corpus: scrapy@`4e956bd`(174 py files/63k LOC). task `scrapy-ctx-1`: "다운로더 미들웨어가 설정 문자열→로드→요청·응답 처리에 끼어들기까지 어떻게 동작하나" — 핵심 위치 수집.
> 데이터셋: `codemap-search-scrapy-context-dataset.json`(8 essential 컴포넌트 = 실제 호출 체인, 코드로 검증). 원자료: `artifacts/axisB-scrapy-runs.json`.
> **설계 강화(advisor 반-grep-bias):** 정답을 "tracer가 연 파일"이 아니라 **코드가 실제 거치는 호출 체인**으로 한정(코드가 arbiter). grep/read + codemap 두 뷰 교차 검증. greppability 실측(`DOWNLOADER_MIDDLEWARES`=3파일, 동적 로딩 체인은 그 키워드 미포함).

| arm | recall | precision | F2 | over | **tokens** | tool_calls | uniq_files |
|---|--:|--:|--:|--:|--:|--:|--:|
| baseline | **1.00** | 0.74 | 0.93 | 7.3 | **23,429** | 20.3 | 7.3 |
| pure | 1.00 | 0.68 | 0.91 | 8.7 | 36,511 (+56%) | 21.0 | 8.0 |
| additive | 0.92 | 0.71 | 0.86 | 7.0 | 36,277 (+55%) | 19.3 | 7.0 |

원자료(tokens): base 26664/21951/21671 · pure 38035/35463/36036 · add 34466/44475/29889.

## 가설 반증 — 이 규모(174파일)에선 baseline이 또 Pareto 최적

1. **가설("indirection은 grep가 못 따라가 codemap이 이긴다")이 틀렸다.** baseline이 동적 로딩 체인(`build_component_list`/`MiddlewareManager.from_crawler`/`load_object`/`build_from_crawler`)을 **recall 1.00으로 완전 포착**. 메커니즘: 에이전트는 순수 grep이 아니라 **grep→read→import·호출 추적**이라, `core/downloader/middleware.py`를 읽으면 나오는 import를 따라 `middleware.py`·`misc.py`로 자연히 이동. 174파일 규모에선 "읽어서 조립"이 충분히 싸다.
2. **codemap은 또 +56% 토큰에 동률.** pure가 overview·search를 **읽기에 더해** 수행 → 같은 recall에 토큰만 추가(같은 ~7–8파일·~27위치 회수). edit-site 결과와 동일 패턴.
3. **additive 또 Pareto 열위**(recall 0.92, 토큰↑) — 단발 sub-agent의 두 도구셋 juggling 재현.

## 종합 (3 task class, 모두 baseline Pareto 최적)
edit-site(fd·rg) + context-gathering(scrapy) 모두에서 baseline grep+read가 recall ≥ codemap이면서 토큰 ~55% 저렴. **회사 경험("압도적")이 재현되지 않음.** 남은 미검증 변수(다음 수):

- **규모(가장 유력):** 전 corpus ≤174파일/≤128k LOC. 이 규모에선 "읽어서 조립"이 싸서 codemap 구조가 중복. 회사 코드는 10–50× 클 가능성 → grep hit·read 비용이 폭증하는 **대형(200k–500k+ LOC, 수천 파일)에서만 교차점**이 날 수 있음. mypy(214k)/cargo(285k) 또는 실제 회사 레포로 검증.
- **측정 아티팩트 — transcript 실측으로 반증됨(중요):** subagent transcript(`subagents/agent-*.jsonl`) 분석 결과, (a) 보고된 "토큰"은 output이 아니라 **context-inclusive 측정**이다 — output_tokens는 arm 무관 거의 동일(baseline 2,608 vs pure 2,425). 차이는 전부 context. (b) "멀티-corpus 스키마 15개 고정비가 비용을 부풀린다"는 가설은 **반증**: codemap 도구는 ToolSearch로 지연 로딩돼 turn1 고정비가 pure 4,614 < baseline 7,528. (c) 비용은 **진짜**: cost-weighted(cache_read×0.1+creation×1.25) baseline 91k vs pure 172k(**1.9×**), context_processed 372k vs 796k(2.1×). 원인 = codemap overview/search **결과 verbosity**가 턴마다 context를 부풀림 + 턴 수↑(29 vs 23). **raw 토큰 지표는 비용을 과소평가했지 과대평가가 아님.**
- **핵심 한계 — 단발 sub-agent ≠ 인터랙티브 main-context:** 본 벤치는 sub-agent의 context 비용을 잰다. 이 워크플로에서 에이전트는 codemap을 **읽기에 더해** 써서 context를 부풀린다. 회사의 "압도적"은 거의 확실히 **인터랙티브 main-context** 경험 — 대형·messy 코드에서 파일 10개를 main에 덤프하는 대신 overview 1회로 **대체**하는 절약 — 이고, 이는 본 단발 sub-agent·file:line recall 지표가 측정하지 않는다. 충실한 재현은 OSS corpus가 아니라 **회사 레포 + 인터랙티브/스텝 지표**.

## 다음 수 — codemap의 진짜 자리
본 결과는 "codemap 무용"이 아니라 "**이 규모(≤174파일)·이 과제에선 중복**"이다. 결정적 미검증 레버는 **규모**:
- **대형 corpus**(mypy 214k Python / cargo 285k Rust / 또는 회사 레포 자체)에서 동일 context-gathering 과제 → grep 노이즈·read 비용이 폭증해 codemap이 recall·토큰 교차점을 만드는지.
- **멀티턴 세션** 측정(단발 location-recall이 아닌, 후속 질의 누적 비용).
- 단일-서버 agent 타입으로 고정 스키마 비용 제거 후 재측정(클린 비용 비교).

## 산출물
- 런 원자료: `artifacts/axisB-fd-runs.json`, `axisB-rg-runs.json`.
- 채점기: `harness/score_axisB.py`(point ±3 / region / region_multi, recall·precision·F2·over-return).
- ripgrep 데이터셋(신규): `codemap-search-ripgrep-dataset.json` — blind A(구현+빌드+스모크) / 독립 B(재도출) / diff-as-arbiter 파이프라인.
