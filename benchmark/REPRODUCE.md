# 재현 가이드 (REPRODUCE)

이 벤치마크는 **방법론과 하네스를 전부 공개**하므로 제3자가 재현·검증할 수 있습니다. 단, LLM 모델은 시간에 따라 갱신되므로 **점수가 비트 단위로 동일하게 재현되지는 않습니다.** 재현의 목적은 "동일 숫자 복제"가 아니라 "절차가 조작 없이 열려 있음"의 검증입니다.

## 1. 대상 코드베이스 (정확한 commit SHA)

벤치마크는 아래 3개 오픈소스 저장소를 **고정 commit**에서 사용했습니다. 동일 SHA로 체크아웃해야 동일 과제 조건이 됩니다.

| 코드베이스 | 저장소 | commit SHA | 기준일 |
|---|---|---|---|
| ClickHouse (C++) | github.com/ClickHouse/ClickHouse | `215ff2543f8b19ca39ac6707fc5cad6195c57880` | 2026-06-12 |
| Deno (Rust/TS) | github.com/denoland/deno | `9f066b87ca674fbf43ce3109a2d061fcc428220e` | 2026-06-12 |
| Angular (TS) | github.com/angular/angular | `302dd0f7c606c3fefede7c2e4b815f8579cc5b28` | 2026-06-17 |

```bash
git clone <repo> && cd <repo> && git checkout <SHA>
```

## 2. 모델 / 런타임 버전

| arm_id | 런타임 | 모델 식별자 |
|---|---|---|
| claude-sonnet | Claude Code CLI (`claude -p`) | `sonnet` |
| codex-gpt54 | Codex CLI (`codex exec`) | `gpt-5.4` |
| opencode-deepseek | opencode CLI (`opencode run`) | `openrouter/deepseek/deepseek-v4-flash` |
| opencode-mimo | opencode CLI | `openrouter/xiaomi/mimo-v2.5` |
| opencode-minimax | opencode CLI | `openrouter/minimax/minimax-m2.7` |
| (채점 judge) | Anthropic | `opus` (`claude-opus-4-8`) |

> 모델은 갱신될 수 있으므로 동일 식별자라도 출력이 달라질 수 있습니다. 이는 LLM 벤치마크의 본질적 한계입니다.

## 3. 백엔드(backend)

| backend | 설치/구동 |
|---|---|
| `no-mcp` | 런타임 기본 도구만(Read/Glob/Grep/Bash 등). MCP 없음. |
| `codemap-search` | 본 저장소의 Rust 바이너리. `codemap-search mcp`로 MCP 서버 구동. |
| `codegraph` | `codegraph serve --mcp -p <root> --no-watch` |
| `serena` | `serena start-mcp-server --project <root> --context ide` (LSP 기반; 언어서버 필요) |

각 episode의 정확한 실행 커맨드는 `raw-evidence/compact/<arm>/<codebase>/round-N/exact_command.json`에 그대로 보존돼 있습니다.

## 4. 실행 (하네스)

전체 실행기는 `harness/runner.mjs`입니다. episode 목록(JSON)을 받아 각 episode를 실행→채점합니다.

```bash
node harness/runner.mjs \
  --episodes   <episodes.json> \
  --arm-config harness/arm-config.json \
  --manifest   harness/task-manifest.json \
  --scorer     harness/scorer.mjs \
  --schema-dir harness/scoring-schemas \
  --out-root   <output-dir> \
  --timeout-s  1800 \
  --judge-model opus
```

- `episodes.json` 형식: `[{ "arm_id": "...", "codebase": "...", "round": 1 }, ...]` (180개 = 5 arm-family × 4 backend × 3 codebase × 3 round; 단 arm은 arm-config가 정의).
- 동시성: 런타임 내부에서 자동 유도(serena 전역 3 / 코드베이스당 1, codegraph 4, 전역 상한 10) + serena 한정 메모리 가드. 인덱스는 사전 빌드 전제(실행 중 재인덱싱 금지).
- 안전장치: episode당 1800s timeout, target-root mutation 가드(git-ignore 규칙 기반), resume-skip(중단 후 동일 커맨드로 이어 실행).

## 5. 채점

- `harness/scorer.mjs`가 episode 답변을 받아 고정 judge(Opus)로 fact별 {0, 0.5, 1.0} 채점 후 가중평균.
- **채점 기준(rubric)**은 `harness/scoring-schemas/scoring_schema.<codebase>.json`에 frozen 상태로 공개. fact·가중치·허용 밴드가 그대로 들어 있습니다.
- task당 ±1 fact 허용 밴드. 점수는 raw 답변 기반이라 도구 텔레메트리와 독립적입니다.

## 6. 집계 / 표 재생성

```bash
node harness/aggregate.mjs       # scored_episodes → 비교표 JSON
node harness/render_tables.mjs   # JSON → mcp_comparison_tables.md
```

- 입력: `results/scored_episodes.180.json` (180 episode 채점 데이터셋; codex 텔레메트리 교정 persist 포함).
- 출력: `results/mcp_comparison_tables.{json,md}`.

## 7. 무엇을 신뢰할 수 있나

- **답변·점수·도구호출·명령**: episode별로 `raw-evidence/`에 원자료가 있어 누구나 재채점/재집계할 수 있습니다.
- **숫자 재현 불가**: 모델 드리프트 때문에 동일 점수는 안 나옵니다. 대신 **절차·기준·데이터가 전부 열려 있어 "조작 없음"을 검증**할 수 있습니다.
- 의문이 있으면 `raw-evidence/transcripts.tar.gz`를 풀어 특정 episode가 실제로 무엇을 검색하고 무엇을 답했는지 직접 확인하세요(`raw-evidence/TRANSCRIPTS.md` 참조).
