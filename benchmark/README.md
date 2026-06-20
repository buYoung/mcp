# codemap-search 벤치마크 — 공개 기록

이 폴더는 `codemap-search`를 포함한 **4개 코드 내비게이션 백엔드**를, **3개 대규모 오픈소스 코드베이스**(ClickHouse·Deno·Angular)에서, **5개 모델/런타임**으로 비교한 벤치마크의 **전체 공개 기록**입니다.

## 한 줄 결론

> **단일 우승 백엔드는 없습니다.** 최적 백엔드는 코드베이스마다 다르고, MCP 도구는 "도구 없이도 잘하는 baseline이 약할 때"만 효과가 있습니다. `codemap-search`는 **Deno에서 가장 강하지만**, ClickHouse에서는 `serena`가 더 낫고, Angular에서는 차이가 없습니다.

## 왜 이 기록을 통째로 공개하나

`codemap-search`의 **저자가 직접 실행한 벤치마크**입니다. 자기 도구를 자기가 평가하면 편향 의심을 받는 것이 당연합니다. 그 의심에 답하는 유일한 방법은 **급진적 투명성**이라 판단했고, 그래서:

- **데이터를 codemap에 유리하게 스핀하지 않았습니다** — 위 결론대로 codemap은 단일 우승자가 아닙니다.
- **점수·도구호출·실행명령·전체 transcript·채점 기준(rubric)·실행 하네스·전 과정**을 함께 공개합니다.
- **AI 에이전트 오케스트레이션(Claude Code)으로 실행**했음을 명시합니다. 사람이 설계·결정·검토하고 에이전트가 실행·집계·교차검증했습니다.
- **벤치마크 도중 우리 하네스에서 버그를 찾아 직접 고쳐, 우리에게 불리했던 결론을 정정**했습니다. 또 우리 분석이 한쪽으로 과장한 부분을 **적대적 검토가 잡아 정정**했습니다. 그 자기수정 과정 전체를 `process/`에 남겼습니다.

즉, **이 기록의 목적은 "codemap이 최고"를 보이는 게 아니라, "조작 없이 정직하게 측정했다"를 증명**하는 것입니다.

## 빠르게 검증하려면

1. **결론과 표**: [`benchmark.md`](benchmark.md) — 비교 표(코드베이스별·모델별 win/tie/loss)와 한계.
2. **숫자 의심**: [`results/scored_episodes.180.json`](results/scored_episodes.180.json) — 180 episode 전부의 점수·도구호출·토큰. 직접 재집계 가능.
3. **답변 의심**: `raw-evidence/compact/<arm>/<codebase>/round-N/raw_answer.txt`(최종답변) + `scorer_output.json`(judge가 fact별로 어떻게 채점했는지).
4. **전 과정 의심**: `raw-evidence/transcripts.tar.gz` — 모델이 실제로 무엇을 검색하고 무엇을 읽었는지 전문([`raw-evidence/TRANSCRIPTS.md`](raw-evidence/TRANSCRIPTS.md)).
5. **재현**: [`REPRODUCE.md`](REPRODUCE.md) — 대상 레포 commit SHA, 모델 버전, 실행 커맨드.

## 폴더 구조

```
benchmark/
├── README.md                      # (이 문서) 진입점
├── benchmark.md                   # 공개용 요약 — 비교 표·결론·한계
├── REPRODUCE.md                   # 재현 가이드(SHA·모델 버전·커맨드)
├── results/                       # 최종 결과물
│   ├── report.md                  # 최종 보고서(+termination 기록)
│   ├── detailed_report.md         # 상세 분석
│   ├── limitations_and_integrity.md  # 한계·무결성 상세
│   ├── mcp_comparison_tables.md   # 전체 비교 표(MD)
│   ├── mcp_comparison_tables.json # 비교 표(JSON)
│   └── scored_episodes.180.json   # 채점 데이터셋(180 episode)
├── harness/                       # 실행/채점 하네스 (재현의 핵심)
│   ├── runner.mjs                 # 실행기(텔레메트리 버그 패치 포함)
│   ├── scorer.mjs                 # 채점기(고정 judge)
│   ├── aggregate.mjs / render_tables.mjs  # 집계·표 생성
│   ├── arm-config.json            # 20 arm 정의
│   ├── task-manifest.json         # 과제 정의(코드베이스당 1개)
│   └── scoring-schemas/           # frozen rubric(fact·가중치·밴드)
├── process/                       # 자기수정 감사 추적
│   ├── change_log.md              # 전 과정 변경 로그
│   ├── run_plan.md / state.json   # 오케스트레이션 계획·상태
│   ├── codex-tool-exposure-diagnostic.json  # codex 버그 진단
│   ├── correction_notes*.md       # 교정 노트
│   ├── integration_report.md / persist_fix_report.md / verify.md  # 통합·검증
│   └── reviews/                   # 건설적·적대적 검토 + HALT2 종합
└── raw-evidence/                  # episode 원자료
    ├── compact/                   # episode별 소형 증거(점수·도구·명령·답변)
    ├── transcripts.tar.gz         # 전체 transcript(압축)
    └── TRANSCRIPTS.md             # 압축 해제·내용 안내
```

## 설계 요약

| 축 | 값 |
|---|---|
| 백엔드 | `no-mcp` · `codemap-search` · `codegraph` · `serena`(LSP) |
| 코드베이스 | ClickHouse(C++) · Deno · Angular |
| 모델/런타임 | claude-sonnet · codex(gpt-5.4) · opencode×3(deepseek/mimo/minimax) |
| 규모 | 5 × 4 × 3 × 1과제 × 3라운드 = **180 episode** (executed 180, valid 166) |
| 채점 | 고정 LLM judge(Claude Opus), frozen rubric, fact별 {0,0.5,1.0}, ±1 fact 밴드 |
| 통계 성격 | n=3/cell → **기술통계(descriptive)일 뿐, 추론통계 아님** |

## 스크럽 / 프라이버시 고지

공개 전 절대 홈경로(`/Users/...`)만 `<REPO_ROOT>`/`<HOME>`으로 정규화했습니다. **점수·답변·도구결과 등 데이터 값(value)은 일절 변경하지 않았습니다.** 프로젝트/패키지명(`buyong-mcp`)처럼 경로가 아닌 공개 식별자는 그대로 둡니다.

> 정직성 부기: 커밋 시 저장소 표준 포매터(biome)가 `*.json` 증거 파일의 **들여쓰기/공백만 정규화**했습니다 — 모든 데이터 값은 동일합니다(검증: `harness/aggregate.mjs` 재실행 시 동일 표 재생성). 전체 transcript(`raw-evidence/transcripts.tar.gz`)는 경로 스크럽 외 **바이트 단위로 원본 그대로**입니다(포매터 미적용).
