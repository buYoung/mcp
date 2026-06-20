# codemap-search 코드 내비게이션 벤치마크 (공개 기록)

> 이 문서는 `codemap-search`를 포함한 4개 코드 내비게이션 백엔드를, 3개 대규모 오픈소스 코드베이스에서, 5개 모델/런타임으로 비교한 벤치마크의 **공개용 요약**입니다.
> **이 벤치마크는 `codemap-search`의 저자가 직접 실행했습니다.** 그래서 편향 의심을 피하기 위해, 유리하게 해석될 여지를 의도적으로 배제하고 **모든 원자료·하네스·과정·자기수정 기록을 함께 공개**합니다. 결론부터 말하면 **codemap-search는 단일 우승자가 아닙니다.**

## 0. 투명성 고지 (먼저 읽어주세요)

- **저자가 자기 도구를 벤치마킹했습니다.** 이해상충이 존재합니다. 이를 상쇄하기 위해 (a) 데이터를 codemap에 유리하게 스핀하지 않았고, (b) 점수·도구호출·명령·전체 transcript·채점 기준·실행 하네스·전 과정을 공개합니다(`raw-evidence/`, `harness/`, `process/`).
- **이 벤치마크는 AI 에이전트 오케스트레이션(Claude Code)으로 실행됐습니다.** 사람이 설계·결정·검토를 맡고, 에이전트가 실행·집계·교차검증을 수행하는 구조입니다. 채점은 고정된(frozen) LLM judge(Claude Opus)가 동일 기준으로 수행했습니다.
- **벤치마크 도중 우리 하네스에서 버그를 발견해 직접 수정했고, 그 결과 우리에게 불리했던 결론을 뒤집었습니다.** 또한 우리 분석이 한쪽으로 과장한 부분을 **적대적 검토(adversarial review)가 잡아내 정정**했습니다. 이 자기수정 과정 전부를 `process/`에 남겼습니다 — 숨기지 않은 것 자체가 비편향의 증거입니다.

## 1. 핵심 결론 (한 줄)

**단일 우승 백엔드는 없습니다. 최적 백엔드는 코드베이스마다 다르고, MCP 도구의 실효는 "도구 없이도 잘하는 baseline이 약할 때"만 나타납니다.**

- `codemap-search`는 **Deno(TypeScript)에서 가장 강하지만**, **ClickHouse(C++)에서는 `serena`(LSP 기반)가 더 낫고**, **Angular에서는 어떤 백엔드도 차이가 없었습니다**(baseline이 이미 충분히 강함).
- 따라서 "codemap이 항상 이긴다"는 주장은 **이 데이터로 성립하지 않습니다.**

## 2. 비교 대상 / 설계

| 축 | 값 |
|---|---|
| 백엔드(backend) | `no-mcp`(기본 도구만) · `codemap-search`(저자 도구) · `codegraph` · `serena`(LSP) |
| 코드베이스 | ClickHouse(C++) · Deno(Rust/TS) · Angular(TS) |
| 모델/런타임 | `claude-sonnet`, `codex(gpt-5.4)`, `opencode`×3(deepseek-v4-flash / mimo-v2.5 / minimax-m2.7) |
| 과제 | 코드베이스당 1개의 어려운 "코드 위치/원인 규명" 과제 |
| 반복 | 과제당 3 round (독립 시도) |
| 규모 | 5 × 4 × 3 × 1 × 3 = **180 episode** (executed 180, valid 166) |
| 채점 | 고정 LLM judge(Claude Opus)가 frozen rubric의 fact별 {0, 0.5, 1.0} 채점 → 가중평균. task당 ±1 fact 허용 밴드. |

- **동률 밴드(tie band)**: 점수 차가 ±0.25(ClickHouse) / ±0.125(Deno·Angular) 이내면 "차이 없음(tie)"으로 본다(과제당 fact 수 기준). 이보다 작은 차이는 n=3 노이즈로 간주.
- **통계 성격**: cell당 n=3, 코드베이스당 과제 1개 → **기술통계(descriptive)일 뿐, 추론통계가 아닙니다.** "유의미한 우열"을 주장할 표본이 아닙니다.

## 3. 비교 표 — claude-sonnet (가장 깨끗한 비교)

claude-sonnet은 4개 백엔드 전부에서 도구를 정상 사용했습니다(backend_off 0). 가장 신뢰할 수 있는 비교입니다. 점수는 valid episode 평균(0~1).

| 코드베이스 | no-mcp | codemap-search | codegraph | serena | 판정 (no-mcp 대비, 밴드 적용) |
|---|---|---|---|---|---|
| ClickHouse (C++) | 0.46 | 0.54 | 0.625 | **0.79** | **serena win** (+0.33); codemap·codegraph tie |
| Deno (TS) | 0.19 | **0.67** | 0.19 | 0.44 | **codemap win** (+0.48), **serena win** (+0.25); codegraph tie |
| Angular (TS) | 0.73 | 0.75 | 0.77 | 0.71 | **전부 tie** (baseline이 이미 강함) |

**읽는 법**: codemap-search는 **Deno에서만 분명한 우위**입니다. ClickHouse는 serena가 이깁니다. Angular는 도구를 붙여도 의미가 없습니다. → "코드베이스 의존".

## 4. 비교 표 — codex (gpt-5.4) (2차 비교, 한계 동반)

codex는 도구를 실제로 활발히 사용했으나(아래 §6 버그 참조), **밴드를 넘는 win이 하나도 없습니다.** 순위 방향은 claude와 비슷하지만 codex의 no-mcp baseline이 이미 높아 MCP의 한계효용이 작습니다.

| 코드베이스 | no-mcp | codemap-search | codegraph | serena | 판정 (밴드 적용) |
|---|---|---|---|---|---|
| ClickHouse | 0.67 | 0.67 | 0.79 | 0.83 | **전부 tie** (serena가 순위상 최상이나 Δ+0.16 < 0.25) |
| Deno | 0.73 | 0.77 | 0.29 | 0.48 | codemap **tie** (Δ+0.04); codegraph·serena **loss** (−0.44 / −0.25) |
| Angular | 0.69 | 0.625 | 0.625 | 0.69 | **전부 tie** |

**읽는 법**: codex에서는 **codemap을 포함해 어떤 백엔드도 baseline을 의미 있게 넘지 못합니다.** 오히려 Deno에서 codegraph·serena는 codex 성능을 떨어뜨렸습니다. 이는 "MCP는 약한 baseline에서만 실효"를 **반박이 아니라 보강**합니다(codex의 강한 baseline → 도구 이득 작음). codex는 read-only sandbox라 claude(쓰기 가능 셸)와 실행환경이 달라(confound), claude와 동급의 깨끗한 비교는 아닙니다.

## 5. 비교 표 — opencode 3종 (약체·노이즈, 참고용)

opencode 3개 모델은 전반적으로 점수가 낮고 분산이 큽니다. 도구를 줘도 오히려 손해인 셀이 많고, 일부는 과제 코드베이스 밖(이 저장소 자체)을 참조하는 등 노이즈가 큽니다. **결론에 거의 기여하지 못하는 약한 데이터원**으로만 봅니다.

| 모델 | 대략 평균 | 특징 |
|---|---|---|
| deepseek-v4-flash | 낮음·분산 큼 | MCP가 도움/손해 혼재 |
| mimo-v2.5 | 낮음·분산 큼 | serena에서 특히 약함 |
| minimax-m2.7 | 낮음·분산 큼 | ClickHouse는 no-mcp(0.79)가 codegraph(0.08)·serena(0.00)보다 훨씬 나음 — MCP가 크게 손해 |

- opencode-serena 27 episode는 이번 공개를 위해 추가 실행한 신규 데이터입니다(valid 22개 평균 0.151; timeout 3 제외). backend_exercised 15/27.
- opencode는 도구 호출 후 내부 서브에이전트(`task`)로 위임해 **도구 사용이 과소집계**됩니다(아래 §7 한계). 그래서 opencode의 효율 비교는 보수적으로 읽어야 합니다.

## 6. 우리가 발견해 고친 하네스 버그 (신뢰성 기록)

- **증상**: 초기 분석에서 codex가 MCP 도구를 "한 번도 안 쓴 것(27/27 backend_off)"으로 집계됐고, 그래서 "codex의 MCP 비교는 무의미"라고 결론냈습니다.
- **원인**: 우리 실행 하네스(`runner.mjs`)의 `extractCodexOutput` 함수가 codex의 도구 이벤트를 파싱하지 않고 빈 배열을 하드코딩 반환하고 있었습니다(텔레메트리 수집 버그). 실제 codex 출력에는 도구 호출이 가득했습니다(예: 한 episode에 80개 도구 이벤트).
- **수정**: stdout 재파싱으로 codex 도구 호출을 복구했고, codex 27 episode 전부 "도구 사용함"으로 정정했습니다. 점수는 모델의 실제 답변 기반이라 **불변**입니다. 하네스 코드도 패치했습니다(`harness/runner.mjs`).
- **의미**: 이 버그는 codex에 **불리**하게 작용하고 있었습니다. 우리는 우리 도구(codemap)에 유리한 방향이 아니라, **공정성을 회복하는 방향으로** 버그를 고쳤습니다.

추가로, 분석 과정에서 우리가 codex의 "동률(tie)"을 "승리(win)"로 한 번 **과장**했는데, **적대적 검토 단계가 독립 재계산으로 이를 잡아내** 헤드라인을 약화시켰습니다(`process/reviews/adversarial.md`).

## 7. 한계 (반드시 함께 읽어주세요)

1. **표본이 작습니다**: cell당 n=3, 코드베이스당 과제 1개. 통계적 유의성 없음(descriptive only).
2. **채점이 LLM judge입니다**: Claude Opus가 frozen rubric으로 채점. rubric·스키마는 `harness/scoring-schemas/`에 공개. 과거 점수를 그대로 재현하는 것은 당시 judge/프롬프트가 기록되지 않아 불가능했고, 대신 "고정 judge 자기일관성"으로 재정의했습니다(한계로 명시).
3. **런타임 confound**: claude=쓰기 가능 셸 / codex=read-only sandbox / opencode=셸 없음. no-mcp baseline 자체가 런타임마다 달라 백엔드 효과와 섞입니다.
4. **codex-serena degraded**: codex의 serena 호출 9 episode 중 3개에서 에러가 발생했습니다.
5. **opencode 과소집계**: opencode는 `task` 위임으로 내부 도구 호출이 집계되지 않습니다.
6. **과제 1개/코드베이스**: 코드베이스 축과 과제 축이 완전히 얽혀 "백엔드×코드베이스 상호작용"을 일반화할 수 없습니다.

자세한 한계·무결성 논의는 `results/limitations_and_integrity.md`.

## 8. 함께 공개하는 것 (검증하려면)

| 폴더 | 내용 |
|---|---|
| `results/` | 최종 보고서, 상세 분석, 한계·무결성, 비교표(MD·JSON), 채점 데이터셋(`scored_episodes.180.json`) |
| `harness/` | 실행기(`runner.mjs`, 패치 포함), 채점기(`scorer.mjs`), 집계 스크립트, arm-config, 과제 매니페스트, **채점 스키마(rubric)** |
| `process/` | 변경 로그, 진단, 교정 노트, 통합·검증 기록, **건설적·적대적 검토 보고서**(자기수정 추적) |
| `raw-evidence/` | episode별 압축 증거(점수·도구호출·명령·답변) + 전체 transcript(`transcripts.tar.gz`) |
| `REPRODUCE.md` | 재현 가이드(대상 레포 commit SHA, 모델 버전, 실행 커맨드) |

**스크럽 고지**: 공개 자료의 절대 경로(`/Users/...` 홈 경로)는 `<REPO_ROOT>`/`<HOME>`으로 정규화했습니다. 그 외 데이터·점수·답변 **값**은 일절 변경하지 않았습니다. (커밋 시 저장소 포매터 biome가 `*.json` 증거의 들여쓰기/공백만 정규화 — 값 동일, `harness/aggregate.mjs` 재실행으로 검증 가능. 전체 transcript는 경로 스크럽 외 바이트 그대로.)
