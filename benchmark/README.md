# codemap-search 1.0.0 벤치마크

한 측정 묶음에서 같은 에이전트·모델·문항·예산으로 **코드 검색 도구별 최종 답변 품질과 탐색 비용**을 비교한다. 풀이 모델은 GPT 6.1 Sol 또는 Claude Opus 5.5 중 하나를 선택하고, 기본값은 GPT 6.1 Sol이다. 에이전트는 도구를 사용하는 공통 사용자이며, 답변 작성 능력의 영향은 공통 사용 조건으로 허용한다. 도구 선택과 호출 순서는 LLM에 맡기고 근거 단계별 분리 집계를 필수로 요구하지 않는다.

현재 하네스는 **Grafana 3문항 × 7개 비교군 × 문항별 1회 = 21회 예정**인 `grafana-routes` 하나만 지원한다. 설치·색인·중립 조회·채점 및 측정 계약·풀이 입력을 준비한 뒤 `prepared_not_executed`에서 종료한다. 실제 21회 풀이·채점 실행 흐름 연결은 별도 작업이다.

## 고정 조건

| 항목 | 조건 |
| --- | --- |
| 코드베이스 | Grafana `c6fad8695a96577eb466d425e6ac4a759ca30f47` |
| 문항 | 즐겨찾기, 기존 대시보드 저장, Grafana Live 수신 — 모두 Go·TypeScript 연결 |
| 기본 풀이 | `gpt-6.1-sol / medium` — Codex CLI |
| 선택 가능한 풀이 | `claude-opus-5-5 / medium` — Claude Code CLI |
| 채점 | 풀이 선택과 관계없이 `gpt-6-astra / high` — Codex CLI |
| 횟수 | 문항·비교군마다 1회, 총 21개 슬롯, 기본 최대 4개 병렬 실행 |
| 한도 | 300초, 탐색 80회, 누적 입력+출력 500,000토큰 |
| 채점 항목 | 핵심 21개와 확장 8개를 분리, `correct`·`supported`를 각각 판정 |

정확한 값은 [실행 계약](data/grafana-execution-v1.json)을 따른다. [고정 데이터셋](data/grafana-routes-v2/README.md)의 질문·정답·범위·채점 의미는 유지한다. 정답 자료를 풀이 대상 소스에 넣지 않는다.

한 번의 선택으로 7개 비교군에 같은 풀이 모델을 적용한다. 두 모델을 모두 자동 실행하거나 결과를 합산하지 않는다. 모델·실행기·추론 강도는 계획 ID와 결과에 포함하고 다른 모델의 결과가 들어오면 거부한다. 채점 항목과 Astra의 추론 강도는 그대로 유지한다. Claude의 `medium`은 [공식 기본값](https://code.claude.com/docs/en/model-config#choose-an-effort-level)을 명시한 값이며, 모델 간 동일한 계산량을 뜻하지 않는다.

측정은 사전 색인을 사용해 최대 4개 회차를 동시에 진행하는 조건이다. 같은 비교군의 문항은 색인·홈 경로의 복원과 조회가 충돌하지 않도록 순차 실행한다. 이 동시 실행 수는 풀이 회차 간 한도이며 제품 내부의 색인·검색 기본값을 바꾸지 않는다. 실제 병렬 실행과 비교군별 직렬화는 향후 실행 흐름 연결 시 적용해야 하며, 현재 준비 단계에서는 설정 전달까지만 검증한다.

| 비교군 | 고정 버전·역할 |
| --- | --- |
| codemap-search | 현재 평가 소스·빌드를 별도 고정; 실제 버전을 기록 |
| CodeGraph (`colbymchenry/codegraph`) | 1.6.1 |
| zvec-grep | 0.2.0 |
| Graphify | 0.9.74 |
| codebase-memory-mcp | 0.11.0 |
| Serena (`oraios/serena`) | 1.7.0 · 로컬 Go·TypeScript LSP |
| 기본 코딩 에이전트 (`rg`) | 추가 검색 제품 없음; rg 15.2.0 사용 가능 |

[비교 대상 잠금](data/grafana-comparison-targets-v1/README.md)이 공식 저장소·전체 커밋·아카이브 해시를 고정한다. 1.0.0 출시 목표를 이유로 현재 제품 빌드를 1.0.0으로 표시하지 않는다.

## 준비 절차

현재 등록된 공식 바이너리 조합은 macOS arm64용이다. Python 3.12 이상, Node 22.13+ 또는 24.x, pnpm, npm, uv, Git, Rust/Cargo, Go, gopls, Codex와 정상 로그인 상태가 필요하다. Claude를 선택하면 `--restricted` 등 필요한 옵션을 제공하는 Claude Code CLI와 로그인도 확인한다. Astra 채점을 위해 Codex는 두 선택 모두 필요하다. 최초 준비는 고정 소스·패키지·로컬 임베딩 가중치를 내려받는다. 개인 MCP 설정이나 전역 도구 설치는 바꾸지 않는다.

모든 명령은 저장소 루트에서 실행한다. CUI 의존성이 없다면 먼저 `pnpm install`을 실행한다.

```sh
pnpm bench:ready
```

기본 명령은 GPT 6.1 Sol을 선택한다. Claude로 준비하려면 다음 명령을 사용한다.

```sh
python3 -m benchmark.route_prepare prepare --solver-model claude-opus-5-5
```

입력 해시 확인 → 도구 설치·빌드 → 전체 Grafana 소스 대조 → 제품별 색인 → 중립 조회·종료·상태 복원 → 채점·측정 계약 확인 → 기존 오프라인 검증 → 21개 풀이 입력과 미실행 계획 생성을 수행한다. 제품 도구의 로컬 임베딩 계산은 포함하며 풀이·채점 LLM은 호출하지 않는다.

완료 후 현재 파일과 준비 기록을 대조한다.

```sh
python3 -m benchmark.route_prepare verify
```

Claude 준비 기록은 `python3 -m benchmark.route_prepare verify --solver-model claude-opus-5-5`로 확인한다.

성공 결과는 `prepared_not_executed: <계획 ID>`다. GPT의 [최종 준비 기록](../docs/briefs/evidence/bench-ready/07-readiness.json)과 Claude의 `07-readiness-claude-opus-5-5.json`은 따로 저장된다. 선택한 기록의 `technical_ready`가 true이고 `solver_runs`, `grader_runs`, `calibration_runs`가 모두 0인지 확인한다. 같은 계획의 `benchmark/.cache/route-preparation/plans/<계획 ID>/`에 풀이 입력과 미실행 결과표가 저장된다. 공유 준비 자료를 다시 생성했다면 각 모델의 기록도 현재 해시로 재검증해야 한다.

준비 CUI는 다음 명령으로 연다.

```sh
pnpm bench:start
```

Grafana 프로필 다음에 풀이 모델을 선택한다. 모델 메뉴에서 Enter를 누르면 기본값 GPT 6.1 Sol이다. 선택한 모델로 7개 비교군 모두를 준비하고 종료한다. 실제 풀이를 시작하거나 과거 답변을 재사용하는 메뉴는 없다. Ctrl+C로 준비를 취소할 수 있으며, 다시 준비할 때 검증된 설치물과 색인을 재사용한다.

## 도구와 측정 정책

- 제품 응답의 기본값을 유지한다. 하네스의 공통 바이트·행 제한, 자동 페이지 조회·요약·보충, 경쟁 도구의 예산 증가는 없다. codemap-search의 100000 설정도 각 설정의 단위를 따르며 공통 토큰 한도로 바꾸지 않는다.
- 제품 자체 도구를 우선한다. Graphify에는 원문 본문 조회를 위해 `read`만 보조로 제공한다. zvec-grep의 후속 grep/rg 안내는 보조 도구 추가 사유로 쓰지 않는다. 낮은 품질이나 짧은 응답으로 보조 도구를 늘리지 않는다.
- `plain rg`는 선택한 CLI에 추가 검색 제품이 없는 기본 에이전트다. 전용 rg MCP 없이 기본 셸·파일 탐색을 제공하고, 명령 PATH에서 고정 rg가 우선한다. Codex는 [권한 프로필](https://learn.chatgpt.com/docs/permissions), Claude는 [restricted 모드](https://code.claude.com/docs/en/cli-reference)와 [Bash 샌드박스](https://code.claude.com/docs/en/sandboxing)로 읽기 범위와 쓰기를 제한한다. Claude의 기본 파일 탐색은 `Bash`·`Read`·`Glob`·`Grep`을 사용하며 각 응답 기본값을 유지한다. Claude의 내장 Grep 구현을 외부 rg MCP로 대체하지 않는다.
- Serena는 전용 `SERENA_HOME`에서 Grafana 전체 루트를 프로젝트로 등록하고 `go`, `typescript` 언어 서버로 `project index`를 실행한다. 언어별 색인 완료·캐시와 MCP 심볼 조회를 확인한 상태에서 측정을 준비한다. 프로젝트 전환·수정·셸·메모리 기능은 제외하고 원문 읽기와 심볼 검색 등 제품 도구를 제공한다. 응답 한도 150,000자·도구 시간 한도 240초·심볼 정보 예산 10초는 고정 버전의 기본값이다. 언어 서버·Go 캐시도 복원 대상에 포함한다. 상위 하네스의 Node 의존성이 섞이지 않도록 Serena의 작업 복사본·홈은 `$TMPDIR/codemap-benchmark-<저장소 식별값>/serena/` 아래에 격리하고 정확한 경로를 준비 기록에 남긴다. 이 경로는 측정 전까지 보존해야 한다. [고정 버전 설정](https://github.com/oraios/serena/blob/949a27ef1e5fda1a6e7b561e777bcece345c6ffd/src/serena/resources/serena_config.template.yml)을 따른다.
- 검색·색인은 로컬 조건이다. Jev는 비활성, Graphify는 빈 출력 디렉터리에서 코드 전용 추출, zvec는 고정 리비전의 `local/potion-code-16m-v2`를 사용한다. 제품의 제외 규칙과 선택적 파서 누락은 그대로 기록한다.
- 22,898개 추적 파일과 생성된 TypeScript·JSON을 동일하게 제공한다. 문항마다 같은 절대 경로에 조회 전 상태를 복원하도록 준비한다. OS 페이지 캐시는 통제하지 않는다.
- 제품군은 MCP 호출, Codex 기준군은 `command_execution`의 고유 item ID, Claude 기준군은 기본 탐색 도구의 고유 `tool_use` ID를 센다. 한 셸 명령 안의 여러 명령은 한 호출이며 시작·완료·부분 메시지를 중복 집계하지 않는다. 관측 시점에 한도를 집행하고 동시 호출의 초과량과 조건 위반을 기록한다.
- 제품 원본 응답·중계 응답·실제 모델 전달 관측을 구분한다. CLI 출력만으로 모델이 모두 읽었다고 간주하지 않는다. 토큰·시간·호출·오류는 관측한 값과 원자료 경로를 남기며 미관측 값을 0으로 채우지 않는다.
- Claude 토큰은 입력·캐시 읽기·캐시 쓰기를 합한 입력과 출력을 집계한다. 부분 메시지의 출력은 누적값으로 처리하고 최종 결과 사용량과 대조한다. 별도 추론 토큰 수와 완전한 후속 API 입력은 CLI가 제공하지 않으면 미관측으로 남긴다. 모델 대체가 관측되면 조건 무효로 처리한다.
- 최종 답변을 고정 rubric으로 채점한다. 인용 파서는 보조 정보이며 특정 형식이나 파싱 성공으로 `supported`를 강제하지 않는다. 확장·비채점 참고 사항을 핵심 필수 항목으로 바꾸지 않는다.
- 도구·모델 식별 노출은 채점 입력 확정 전에 차단하고 원답변과 정확한 구간을 검토 기록에 남긴다. 실행 조건 무효, 실패 사유, 호출 로그 불완전도 결과에서 보존한다. 1회 측정으로 반복 분산이나 통계적 유의성을 주장하지 않는다.

## 기록과 재검증

[준비 브리프셋](../docs/briefs/2026-10-03-briefset-bench-ready.md)과 `docs/briefs/evidence/bench-ready/`에 현재 준비 작업·검증 근거를 보존한다. `benchmark/.cache/route-preparation/`에는 설치물과 복원본, 대부분의 소스·색인·조회 로그가 있다. Serena의 실제 작업 경로는 앞서 설명한 별도 임시 루트이며 복원본은 공통 준비 캐시에 둔다. 준비 기록의 해시를 재검증할 때 이 캐시가 필요하며 바이너리·가중치·색인 캐시는 커밋에 넣지 않는다.

중립 조회는 당시 코드·명령·환경·권한·선택한 CLI 버전·노출 도구 구성이 현재와 일치할 때만 재사용한다. 실행기별 준비 기록을 분리하며 색인 생성 결과는 같은 제품·소스일 때 재사용한다. 변경 시 조회 전 상태로 복원한 뒤 다시 점검하며, 준비 중 코드가 바뀌어도 완료 처리를 막는다. 제품 소스 바이트가 같으면 고정된 빌드의 출처를 새 Git HEAD로 다시 표시하지 않는다.

해시 불일치나 설치 실패가 생기면 출력된 작업 전용 로그에서 원인을 확인한다. 다른 버전으로 자동 대체하거나 필수 비교군을 제외하지 않는다. 입력이나 색인이 변경된 경우 해당 준비 상태를 새로 만들어야 하며, 개인 데몬을 종료해 충돌을 해결하지 않는다.

현재 데이터셋 manifest와 비교 대상 잠금의 과거 연결 상태·초기 개정 경로는 동결 당시의 출처 기록이다. 현재 실행 의존성이 아니다. 현재 조건은 실행 계약과 최종 준비 기록을 따른다.

## 하네스 검증과 실제 실행 경계

```sh
python3 -m unittest benchmark.checks benchmark.cui_checks
node --check benchmark/start.mjs
```

기존 실행·사용량·취소·응답 처리 검증과 실제 Inquirer 터미널 검증을 사용하며 모델 실행은 로컬 대역으로 대체한다. [CUI 점검 기록](../docs/briefs/evidence/bench-ready/07-operator-inspection.json)에는 실제 준비 기록을 사용한 메뉴 흐름과 실행 호출 0회를 남긴다.

공통 실행 함수 `runner.execute_model`은 선택한 CLI로 풀이를 전달하고 채점 요청은 항상 Codex/Astra로 보낸다. 실제 21회 실행에는 준비 검증 이후 슬롯별 상태 복원·풀이·수집·익명 채점·결과 집계를 연결하는 작업이 남아 있다. 현재 CLI는 `catalog`와 `prepare`만 제공하며 실행 설정은 준비 전용이다. 계정의 지정 모델 사용 가능 여부, 실제 모델 전달량, 채점 보정, 난도 실측은 아직 확인하지 않았다. Claude의 `--init-only` 점검은 설정을 읽고 시작할 수 있는지 확인하며, 실제 기본 도구의 권한 집행과 서버가 적용한 추론 강도까지 검증한 것으로 취급하지 않는다.
