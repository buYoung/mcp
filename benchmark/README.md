# codemap-search 1.0.0 벤치마크

한 측정 묶음에서 같은 에이전트·모델·문항·예산으로 **코드 검색 도구별 최종 답변 품질과 탐색 비용**을 비교한다. 풀이 모델은 GPT 6.1 Sol 또는 Claude Opus 5.5 중 하나를 선택하고, 기본값은 GPT 6.1 Sol이다. 에이전트는 도구를 사용하는 공통 사용자이며, 답변 작성 능력의 영향은 공통 사용 조건으로 허용한다. 도구 선택과 호출 순서는 LLM에 맡기고 근거 단계별 분리 집계를 필수로 요구하지 않는다.

현재 하네스는 **Grafana 3문항 × 7개 비교군 × 문항별 1회 = 21회 예정**인 `grafana-routes` 하나만 지원한다. 설치·색인·중립 조회·채점 및 측정 계약·풀이 입력의 준비는 `prepared_not_executed`에서 종료한다. 실행은 불변 계획의 정확한 ID·hash·모델에 대한 별도 승인으로만 시작하며, 21-slot ledger에 실패와 미시작까지 보존한다. **제품별 격리·semantic ready warm-host와 runner의 clock 연결이 통합되지 않았다면 실행은 `blocked`로 끝난다.** 준비 성공만으로 실제 실행이 허가되거나 검증된 것은 아니다.

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

측정은 사전 색인을 사용해 최대 4개 회차를 동시에 진행하는 조건이다. 같은 비교군의 문항은 색인·홈 경로의 복원과 조회가 충돌하지 않도록 순차 실행한다. 이 동시 실행 수는 풀이 회차 간 한도이며 제품 내부의 색인·검색 기본값을 바꾸지 않는다. campaign scheduler는 restore부터 product 종료·사후 검증·baseline 복원까지 최대 4개 lifecycle을 허용하며, 같은 비교군의 lifecycle은 겹치지 않는다. solver 정리가 끝난 뒤 Astra 채점도 최대 4개로 진행한다. 실제 제품 통합과 모델 실행은 별도 검증 대상이다.

| 비교군 | 고정 버전·역할 |
| --- | --- |
| codemap-search | 0.13.0 · `codemap-v0.13.0` (`4061c42a5`)의 `git_ref` 스냅샷 빌드 |
| CodeGraph (`colbymchenry/codegraph`) | 1.6.2 |
| zvec-grep | 0.2.2 · npm/tag stable, GitHub Release 없음 |
| Graphify | 0.9.95 · hash-pinned PyPI wheel + 고정 runtime requirements |
| codebase-memory-mcp | 0.12.1 · 공식 darwin-arm64 asset |
| Serena (`oraios/serena`) | 2.1.0 · 로컬 Go·TypeScript LSP, `--agent-interface tools` 예외 |
| 기본 코딩 에이전트 (`rg`) | 추가 검색 제품 없음; rg 15.2.0 사용 가능 |

[비교 대상 잠금](data/grafana-comparison-targets-v1/README.md) 개정 3이 공식 저장소·전체 커밋·아카이브 해시를 고정한다. codemap-search는 작업 트리가 아닌 release tag만 빌드하며 실제 버전·전체 commit·binary hash를 기록한다. 1.0.0 출시 목표를 이유로 제품 빌드를 1.0.0으로 표시하지 않는다. 구버전의 index/baseline/ready 기록은 최신 버전으로 재인증하지 않고 새 cache에서 다시 준비한다.

Graphify 0.9.95의 upstream `uv.lock`은 루트 버전과 필수 PyYAML이 맞지 않아 사용하지 않는다. bundle의 hash-pinned requirements로 공식 wheel을 설치하고 package 파일을 tag 원문과 대조하며, 준비 중 의존성을 다시 해석하지 않는다. Graphify·Serena 모두 `uv pip check`를 통과해야 한다. Graphify의 과거 `v1.0.0` release 대신 현재 upstream latest/PyPI가 일치하는 0.9.95를 사용한다.

## 준비 절차

현재 등록된 공식 바이너리 조합은 macOS arm64용이다. Python 3.12 이상, Node 22.13+ 또는 24.x, pnpm, npm, uv, Git, Rust/Cargo, Go, gopls, Codex와 정상 로그인 상태가 필요하다. Claude를 선택하면 `--restricted` 등 필요한 옵션을 제공하는 Claude Code CLI와 로그인도 확인한다. Astra 채점을 위해 Codex는 두 선택 모두 필요하다. 최초 준비는 고정 소스·패키지·로컬 임베딩 가중치를 내려받는다. 개인 MCP 설정이나 전역 도구 설치는 바꾸지 않는다.

모든 명령은 저장소 루트에서 실행한다. CUI 의존성이 없다면 먼저 `pnpm install`을 실행한다.

준비 데이터(소스·설치물·색인·복원본·기록)는 기본적으로 `benchmark/.cache/`에 저장한다. 다른 디스크에 두려면 `CODEMAP_BENCH_CACHE=<경로>`를 지정한다. 이때 Serena 작업 복사본도 그 아래에 둔다. 준비·검증·이후 실행은 같은 값을 사용해야 한다.

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

성공 결과는 `prepared_not_executed: <계획 ID>`다. GPT의 [최종 준비 기록](../docs/briefs/evidence/bench-ready/07-readiness.json)과 Claude의 `07-readiness-claude-opus-5-5.json`은 따로 저장된다. 선택한 기록의 `technical_ready`가 true이고 `solver_runs`, `grader_runs`, `calibration_runs`가 모두 0인지 확인한다. 같은 계획의 `benchmark/.cache/route-preparation/plans/<계획 ID>/`에 풀이 입력과 미실행 결과표가 저장된다. 각 계획에는 handoff·참조 근거·inventory·probe·corpus의 불변 snapshot이 포함된다. 다른 모델을 준비해 최신 포인터를 다시 게시해도 기존 계획의 바이트를 덮어쓰지 않는다. `plan_condition`에서 ID를 재계산하며, 코드·모델·제품 조건이 바뀌면 기존 계획을 재인증하지 않고 준비를 다시 해야 한다. 미완성 계획 디렉터리도 자동 덮어쓰지 않는다.

공유 설치·색인·계약 검증을 한 번 수행하고 두 backend를 각각 probe/finalize하려면 다음을 사용한다. 이는 두 모델의 **준비**이며 자동 실행이 아니다.

```sh
python3 -m benchmark.route_prepare prepare --solver-model all
python3 -m benchmark.route_prepare verify --solver-model all
```

준비·실행 승인 CUI는 다음 명령으로 연다.

```sh
pnpm bench:start
```

Grafana 프로필 다음에 풀이 모델을 선택한다. 모델 메뉴에서 Enter를 누르면 기본값 GPT 6.1 Sol이다. 선택한 모델로 7개 비교군 모두를 준비한 뒤 정확한 계획 ID·readiness hash·모델·버전·21회·한도를 표시한다. 별도 실행 확인의 기본값은 **아니요**다. 확인을 거절하면 풀이·채점 0회로 끝난다. Ctrl+C는 새 슬롯을 중단하고 자식 정리 후 ledger/report 위치를 안내한다. 준비와 실행의 lease는 cache/model 구분 없이 공유되며, host 공통 port lease를 먼저 얻는다. 다른 프로세스의 포트를 빼앗거나 종료하지 않는다.

## 운영자 실행·검토·보고

JSON-lines CLI는 stdin으로 JSON object를 받는다. 아래 명령의 입력 파일은 solver가 읽을 수 없는 운영자 영역에 둔다. `catalog`, `prepare`, `approve-review`, `report`, `inspect-operator`는 모델을 호출하지 않는다. `run`과 `resume`만 승인 범위 내 모델 호출이 가능하다.

```sh
# run-approval.json: plan_id, readiness(불변 plans/<id>/readiness.json),
# readiness_sha256, solver_model, actor, confirmed:true를 명시
python3 -m benchmark.cli run < /private/operator/run-approval.json

# resume.json: run_root, confirmed:true. 미시작 슬롯까지 시작하려면 retry_unstarted:true
python3 -m benchmark.cli resume < /private/operator/resume.json

# review.json: run_root, slot_id, approval(아래 형식)
python3 -m benchmark.cli approve-review < /private/operator/review.json

# report.json: run_root만 필요
python3 -m benchmark.cli report < /private/operator/report.json
```

`RunManifest`와 승인·준비 기록은 불변이다. 실행 결과는 cache의 `route-preparation/campaigns/<campaign_id>/`에 따로 저장한다. `ledger.json`은 fsync·atomic replace로 갱신하며 단일 coordinator만 작성한다. solver/grader launch intent를 먼저 내구 저장한다. intent 이후 crash는 실행 여부가 불확실해도 자동 재실행하지 않는다. 정리가 확인되지 않은 process group/workspace는 `blocked`로 두고 추측으로 종료하거나 복원하지 않는다. 미시작 재개만 명시적 `retry_unstarted`로 허용하고 이전 preflight의 원자료도 보존한다. 모델 호출 확인과 CLI spawn은 별개이며 미관측 provider 호출은 null이다.

ready 실패는 `warmup_failed`로 기록하고 solver를 시작하지 않는다. 원자료는 writer 종료·정리 뒤 파일별 hash/bytes로 봉인한다. 조건이 유효하고 cutoff 이전 완성 답변임이 입증된 슬롯만 채점한다. late answer, 미시작·취소·로그/모델 조건 무효는 실패·미채점 행으로 보존한다. 낮은 점수나 오류를 이유로 solver/grader를 자동 재시도하지 않는다.

도구·backend·모델 별칭, namespace, 경로, plan/run 식별, 사용량 자기보고는 private `reviews/<blind_id>/v1/draft.json`의 Unicode code-point 구간으로 검토한다. `approval`에는 `reviewer`, `revision`, `raw_sha256`, `draft_sha256`, 모든 flag의 `decisions`가 필요하다. 각 결정은 `flag_id`, `decision`, `evidence`를 포함한다.

- `legitimate_source_identifier`: 실제 source 근거를 기록하고 원문을 그대로 유지한다.
- `identity_only_anonymization`: identity 근거와 `replacement:"[anonymous-001]"` 같은 중립 표식만 허용한다. 승인된 정확한 구간 외 수정은 금지한다.
- `reject_packet`: 미채점으로 남긴다.

`approve-review`는 원답변/구간 hash·가역성을 확인하고 units/citations/schema/prompt를 다시 생성해 불변 packet을 봉인할 뿐 **모델을 호출하지 않는다**. 동일 승인 재적용은 기존 seal을 확인한다. `resume`에서만 sealed packet의 Astra 채점을 진행한다. 160 KiB 초과 packet은 절단하지 않고 실패한다. grader는 언제나 Codex/Astra/high이며 ID·fact 일대일 대응·차원/사유·unit/citation membership까지 검사한다. private registry/검토 근거는 grader 입력에 넣지 않는다.

보고서는 항상 21행이며 launch 예약·CLI 시작·provider 미관측·terminal·review/grade·failed/cancelled/not-started를 구분한다. warm/index 비용을 solver 비용과 합치지 않고, core/extended 분모와 macro/fact-weighted를 따로 표시한다. 미채점을 false/0점으로 바꾸지 않는다.

운영자 inspection 증거는 ad-hoc 스크립트 대신 다음 공식 명령으로 재생성한다. 실제 catalog와 검증한 불변 readiness projection으로 실제 CUI 함수를 실행하되 backend 호출을 차단하고 실행 확인은 거절한다. 결과는 `docs/briefs/evidence/bench-ready/07-operator-inspection*.json`이며 모델 0회 경계를 기록한다. 실제 터미널 입력은 기존 PTY checks가 별도로 검증한다.

```sh
printf '{}' | python3 -m benchmark.cli inspect-operator
printf '{}' | python3 -m benchmark.cli inspect-operator --solver-model claude-opus-5-5
```

### warm-host 통합 경계

`execution.run_campaign`은 테스트/통합용 `prepare_session`, `solver_executor`, `grader_executor`, `collector`, `restore`, `source_check`를 주입받는다. 기본 경로는 `benchmark.warm_host.prepare_session`과 `runner.execute_model(..., prepared_session=session)`을 요구하며 cold relay로 우회하지 않는다.

- `prepare_session(state, slot_dir, expected_client, cancel_event, process_registry)`는 `state.campaign_binding`을 받는다. binding에는 plan/slot/arm과 artifact/source/baseline/inventory/isolation/warmup-policy hash가 있다.
- `await_ready(deadline)`는 제품별 semantic assertion 뒤 `{ready:true, binding:<동일 값>, ready_at:<monotonic>, ...근거}`를 반환한다. `attachment()`는 같은 live 세션의 일회 연결 정보다.
- runner가 `Popen` 직전 t0에서 `session.open_solver_gate(t0)`를 호출한다. product restart나 warm probe 예산 혼입은 금지한다. 결과는 `answer_eligible`을 명시한다.
- `health().condition_errors`는 조건 열화를 보고하고, `close(reason)`는 bounded cleanup의 `complete`를 반환한다. 독립 그룹은 prompt/attach 허용 전에 `process_registry.register(pid, pgid, owner)`에 등록한다.
- warm/transport 원자료는 `slot_dir` 바로 아래에 둔다. product home이나 CLI credentials/workspace는 raw seal 대상이 아니다. CLI `process.json`이 없으면 실제 spawn을 인증하지 않는다.

## 도구와 측정 정책

- 제품 응답의 기본값을 유지한다. 하네스의 공통 바이트·행 제한, 자동 페이지 조회·요약·보충, 경쟁 도구의 예산 증가는 없다. codemap-search의 100000 설정도 각 설정의 단위를 따르며 공통 토큰 한도로 바꾸지 않는다.
- codemap-search의 `filesystem_permissions`(find·grep·read)는 native scaffold의 **`anywhere`**를 변경 없이 사용한다. `isolation.py`의 제품 child Seatbelt와 제품별 relay 정책이 경계를 집행한다. 별칭 `query`, file URI, 배열·glob·project selector도 거부·기록 대상이다. 동일 프로세스 pre-warm 통합과 전체 corpus 재준비 전에는 소형 probe 성공만으로 실행을 승인하지 않는다.
- 제품 자체 도구를 우선한다. Graphify에는 원문 본문 조회를 위해 `read`만 보조로 제공한다. zvec-grep의 후속 grep/rg 안내는 보조 도구 추가 사유로 쓰지 않는다. 낮은 품질이나 짧은 응답으로 보조 도구를 늘리지 않는다.
- `plain rg`는 선택한 CLI에 추가 검색 제품이 없는 기본 에이전트다. 전용 rg MCP 없이 기본 셸·파일 탐색을 제공하고, 명령 PATH에서 고정 rg가 우선한다. Codex는 [권한 프로필](https://learn.chatgpt.com/docs/permissions), Claude는 [restricted 모드](https://code.claude.com/docs/en/cli-reference)와 CLI 전체의 outer Seatbelt로 읽기 범위와 쓰기를 제한한다. 중첩 Seatbelt가 불가능하므로 Claude inner Bash sandbox는 끈다. Claude의 기본 파일 탐색은 `Bash`·`Read`·`Glob`·`Grep`을 사용하며 각 응답 기본값을 유지한다. Claude의 내장 Grep 구현을 외부 rg MCP로 대체하지 않는다.
- Serena 2.1.0의 native interface 기본값은 임의 Python 실행을 노출하는 REPL이다. 검색 전용 계약의 단일 interface 예외로 **`--agent-interface tools`**를 사용하고 native default와 예외 이유를 prepared state에 기록한다. `serena_repl`이 검색 도구 목록에 남으면 준비를 거부한다. 전용 `SERENA_HOME`에서 Grafana 전체 루트를 프로젝트로 등록하고 `go`, `typescript` 언어 서버로 `project index`를 실행한다. 언어별 색인 완료·캐시와 MCP 심볼 조회를 확인한 상태에서 측정을 준비한다. 프로젝트 전환·수정·셸·메모리 기능은 제외하고 원문 읽기와 심볼 검색 등 제품 도구를 제공한다. 응답 한도 150,000자·도구 시간 한도 240초·심볼 정보 예산 10초는 고정 버전의 기본값이다. 언어 서버·Go 캐시도 복원 대상에 포함한다. 상위 하네스의 Node 의존성이 섞이지 않도록 Serena는 ancestor에 `node_modules`가 있는 cache root를 거부한다. 모든 arm의 작업 복사본·홈은 `<CODEMAP_BENCH_CACHE>/<opaque hex>/{source,home}`에 놓고 정확한 경로를 준비 기록에 남긴다. 이 경로는 측정 전까지 보존해야 한다. [고정 버전 설정](https://github.com/oraios/serena/blob/5e4f5cd173c7ff3eed37ee3f1de6c266b937c5dc/src/serena/resources/serena_config.template.yml)을 따른다.
- 검색·색인은 로컬 조건이다. Jev는 비활성, Graphify는 빈 출력 디렉터리에서 코드 전용 추출, zvec는 고정 리비전의 `local/potion-code-16m-v2`를 사용한다. zvec index와 MCP 모두 `ZVEC_GREP_MODEL_CACHE=<home>/model-cache`를 공유한다. 제품의 제외 규칙과 선택적 파서 누락은 그대로 기록한다.
- CodeGraph는 `status --json`의 complete·버전·pending refs/changes를 확인한다. zvec는 check-ready 및 `freshness: fresh`, Graphify는 양수 `graph_stats`와 오류 텍스트 부재, Serena는 tools interface·LSP ready·심볼 응답과 timeout 후 진행 로그 부재를 확인한다. codemap의 warming/stale 응답도 성공으로 받지 않는다. exit 0 또는 `isError=false`만으로 준비 완료를 판정하지 않는다. 전체 corpus의 deeper semantic gate와 live warm hand-off는 별도 검증이 필요하다.
- CBM 0.12.1은 `index_status`의 ready·project·root를 대조한다. 모든 index/MCP/shutdown은 `CBM_RUNTIME_DIR=<CODEMAP_BENCH_CACHE>/cbmrt`를 공유하며 canonical socket 경로가 104 bytes 미만이어야 한다. 길면 더 짧은 cache root를 지정한다. 현재 전용 loopback 포트는 zvec 19700, CBM UI 19701이다. `detect_changes(scope="tests")`는 schema를 유지하며 요청마다 별도 graph를 생성하는 제품 동작이므로 사전 호출로 제거할 수 없는 실행 비용/위험이다.
- 22,898개 추적 파일과 생성된 TypeScript·JSON을 동일하게 제공한다. 문항마다 같은 절대 경로에 조회 전 상태를 복원하도록 준비한다. OS 페이지 캐시는 통제하지 않는다.
- 제품군은 MCP 탐색 시도(`tools/call` 및 `resources/*`·`prompts/*`의 콘텐츠 반환 메서드)를 공통 admission lock에서 예약한다. 경로·도구 거절도 시도 예산을 소비하며 `attempt`·`terminal`·`budget` 기록에 남기되 실제 dispatch와 구분한다. 80번째는 완료와 남은 예산 내 답변 작성을 허용하고, 81번째는 dispatch 전에 거부하며 cutoff를 고정한다. 제품 RPC 동안 lock을 잡지 않는다.
- Codex 기준군은 `command_execution`의 고유 item ID, Claude 기준군은 기본 탐색 도구의 고유 `tool_use` ID를 센다. 한 셸 명령 안의 여러 명령은 한 호출이며 시작·완료·부분 메시지를 중복 집계하지 않는다. 기본 도구는 후관측 방식이므로 81번째 사전 차단을 보장하지 않는다. 관측한 초과는 중단·조건 무효로 기록한다.
- usage drain은 0초다. cutoff 즉시 SIGINT를 보내고 이후 SIGTERM·SIGKILL 및 owned process-group 회수 중 출력·사용량은 비용으로 보존한다. CLI 리더가 먼저 종료해도 같은 그룹의 자식과 독립 제품 그룹을 정리한다. wall cutoff는 늦게 감지한 시각이 아니라 `solver_start + 300초`다. 두 backend 모두 cutoff 이전의 완전한 final을 입증해야 하며 Codex는 rollout timestamp와 완료 관측 시각을 함께 확인한다. 늦거나 적격성을 입증하지 못한 답변은 원문·SHA256을 별도 보존하고 채점 답변에서 제외한다.
- 최종 cached-inclusive 관측 토큰이 500,000을 초과하면 조건 무효다. 종료 직전·회수 중 증가분도 재검사하며 관측값을 한도로 clamp하지 않는다. 전체 사용량이 미인증이면 예산 준수도 인증하지 않는다. 즉시 결과와 재집계는 같은 사용량 reconciliation을 사용한다.
- host MCP 호출 timeout은 Codex `tool_timeout_sec=300`, Claude `MCP_TOOL_TIMEOUT=300000` ms, 내부 `MCPClient` 기본 300초다. 연결 timeout은 Codex `startup_timeout_sec=120`, Claude `MCP_TIMEOUT=120000` ms로 명시한다. 제품 내부 timeout은 유지하며 남은 전체 wall 예산이 우선한다. 두 backend와 중계에 부모가 resolve한 `CODEMAP_BENCH_CACHE`를 전달한다.
- 제품 원본 응답·중계 응답·실제 모델 전달 관측을 구분한다. CLI 출력만으로 모델이 모두 읽었다고 간주하지 않는다. 토큰·시간·호출·오류는 관측한 값과 원자료 경로를 남기며 미관측 값을 0으로 채우지 않는다. 손상된 JSONL은 첫 손상 전의 유효 prefix만 읽고 원본·손상 행·byte offset·불완전 사유를 보존한다. 호출 로그 완결성은 collection·relay 종료·미완료 host/제품 호출·중복 기록을 함께 확인한다. 완전히 기록한 오류 응답은 로그 손실과 구별하며 준비 probe의 transcript 완결성을 solver 로그 완결성으로 표시하지 않는다.
- Claude 토큰은 입력·캐시 읽기·캐시 쓰기를 합한 입력과 출력을 집계한다. 부분 메시지의 출력은 누적값으로 처리하고 최종 결과 사용량과 대조한다. 별도 추론 토큰 수와 완전한 후속 API 입력은 CLI가 제공하지 않으면 미관측으로 남긴다. 모델 대체가 관측되면 조건 무효로 처리한다.
- 최종 답변을 고정 rubric으로 채점한다. 인용 파서는 보조 정보이며 특정 형식이나 파싱 성공으로 `supported`를 강제하지 않는다. 확장·비채점 참고 사항을 핵심 필수 항목으로 바꾸지 않는다.
- 도구·모델 식별 노출은 채점 입력 확정 전에 차단하고 원답변과 정확한 구간을 검토 기록에 남긴다. 실행 조건 무효, 실패 사유, 호출 로그 불완전도 결과에서 보존한다. 1회 측정으로 반복 분산이나 통계적 유의성을 주장하지 않는다.

## 기록과 재검증

[준비 브리프셋](../docs/briefs/2026-10-03-briefset-bench-ready.md)과 `docs/briefs/evidence/bench-ready/`에 현재 준비 작업·검증 근거를 보존한다. 준비 데이터 위치(기본 `benchmark/.cache/`, 또는 `CODEMAP_BENCH_CACHE`)의 `route-preparation/`에는 설치물과 복원본, 대부분의 소스·색인·조회 로그가 있다. Serena의 실제 작업 경로는 앞서 설명한 별도 임시 루트이며 복원본은 공통 준비 캐시에 둔다. 준비 기록의 해시를 재검증할 때 이 캐시가 필요하며 바이너리·가중치·색인 캐시는 커밋에 넣지 않는다.

중립 조회는 당시 코드·명령·환경·권한·선택한 CLI 버전·노출 도구 구성이 현재와 일치할 때만 재사용한다. 실행기별 준비 기록을 분리하며 색인 생성 결과는 같은 제품·소스일 때 재사용한다. 변경 시 조회 전 상태로 복원한 뒤 다시 점검하며, 준비 중 코드가 바뀌어도 완료 처리를 막는다. 제품 소스 바이트가 같으면 고정된 빌드의 출처를 새 Git HEAD로 다시 표시하지 않는다.

해시 불일치나 설치 실패가 생기면 출력된 작업 전용 로그에서 원인을 확인한다. 다른 버전으로 자동 대체하거나 필수 비교군을 제외하지 않는다. 입력이나 색인이 변경된 경우 해당 준비 상태를 새로 만들어야 하며, 개인 데몬을 종료해 충돌을 해결하지 않는다.

현재 데이터셋 manifest와 비교 대상 잠금의 과거 연결 상태·초기 개정 경로는 동결 당시의 출처 기록이다. 현재 실행 의존성이 아니다. 현재 조건은 실행 계약과 최종 준비 기록을 따른다.

## 하네스 검증과 실제 실행 경계

```sh
python3 -m unittest benchmark.checks benchmark.cui_checks
node --check benchmark/start.mjs
```

기존 실행·사용량·취소·응답 처리 검증과 실제 Inquirer 터미널 검증을 사용하며 모델 실행은 로컬 대역으로 대체한다. [CUI 점검 기록](../docs/briefs/evidence/bench-ready/07-operator-inspection.json)에는 실제 준비 기록을 사용한 메뉴 흐름과 실행 호출 0회를 남긴다.

공통 실행 함수 `runner.execute_model`은 선택한 CLI로 풀이를 전달하고 채점 요청은 항상 Codex/Astra로 보낸다. campaign의 승인·ledger·scheduler·검토·보고는 무모델 대역으로 검증하며, 실제 실행은 최신 핀·제품별 OS 경계·semantic readiness·동일 live 세션/clock 통합과 새 준비 snapshot의 독립 검토 뒤에만 허용한다. 계정의 지정 모델 사용 가능 여부, 실제 모델 전달량, 채점 보정, 난도 실측은 아직 확인하지 않았다. Claude의 `--init-only` 점검은 설정을 읽고 시작할 수 있는지 확인하며, 실제 기본 도구의 권한 집행과 서버가 적용한 추론 강도까지 검증한 것으로 취급하지 않는다.

### OS 격리와 준비 시 경계 검증

- cache root는 사용자가 정한 위치를 유지한다. 그 아래 workspace 식별자는 제품/벤치 이름 없는 고정 opaque 값이다. **cache root 이름 자체는 제품 출력과 solver에게 보인다.** 색인과 실행은 같은 canonical 절대 경로를 사용하며 기존 이름 있는 workspace는 자동 재인증하지 않는다.
- 제품 stdio·zvec HTTP 서버·CBM shutdown과 그 자식 LSP는 모두 `sandbox-exec -f <profile>`로 실행한다. 기본 file data 읽기·쓰기·network는 거부하고 해당 설치/runtime의 read-only 접근, 해당 source/home의 읽기·쓰기, 예약 loopback 포트만 허용한다. macOS 기본 시스템 경로와 metadata/stat은 허용되므로 VM 수준의 process/Mach 격리라고 주장하지 않는다. source/home의 hard link와 외부 symlink는 시작 전에 거부한다.
- HOME/TMPDIR/XDG/npm/uv cache는 arm home 내부다. live slot은 전용 tmp를 지정한다. CBM의 짧은 runtime과 제품이 하드코딩한 `/private/tmp/cbm-*`만 예외이며 기존 항목은 지우지 않고 소유 실행 전후 새 residue를 정리·기록한다. host 공통 lease와 per-arm 직렬화가 필요하다. 준비 index 명령도 같은 경계를 쓰되 Serena 의존성 취득 단계만 network 예외를 기록한다. live 제품에는 이 예외를 허용하지 않는다.
- `product_profile(ident, state, artifact, *, slot_tmp)`는 profile 경로/hash·private env·허용 경로·포트를 반환한다. warm host/준비 probe가 이를 **client 복사본**의 `isolation_profile`에 넣어야 한다. `MCPClient`는 `product_arm`이 있는 제품의 profile 누락을 거부하며 stdio/HTTP/shutdown을 같은 profile로 감싼다. harness relay와 shim 자체는 감싸지 않는다.
- relay는 고정 release의 도구/인자 inventory를 검사한다. 신규 도구/인자는 검토 전 거부한다. 읽기 경로는 canonical tracked source와 그 ancestor로 한정하며 `.git`, `.codemap`, `.codegraph`, `.serena`, `.zvec-grep`, `.codebase-memory`, `graphify-out` 등을 읽지 못한다. metadata ignore bypass와 다른 project/root selector를 거부하며 입력이나 제품 결과를 몰래 고치지 않는다. 제품 capability는 보존하고 `resources/*`·`prompts/*`도 동일 admission/budget/log 경로를 쓴다. file URI는 source 내부만, product URI는 광고된 값만 허용하며 CBM prompt project도 준비 project에 결합한다.
- raw index의 파일 경로를 tracked/승인 synthetic/미승인으로 분류한다. Tantivy 저장 문서, SQLite 경로 열, zvec relative/absolute 파일 목록·root manifest, Graphify nodes/links, 실행 불가능한 Serena pickle reader를 사용한다. 외부 경로·알 수 없는 index 형식은 준비 실패다. CBM의 고정 semantic-input key와 excluded-directory coverage row는 source body와 구별한다.
- 경계 probe는 하네스 소유 canary와 metadata canary를 만들어 경로 alias·URI·array를 시도하고 삭제한다. 실제 정답/세션/다른 workspace 경로는 **존재 여부만** 확인한다. Codex baseline은 동일 `codex sandbox` profile에서 TMPDIR/session/cache/worktree/symlink/metadata canary 읽기 거부와 source 읽기·고정 rg를 검사한다.
- Claude baseline은 별도 소유 CONNECT proxy(19550)를 통해 `api.anthropic.com`, `claude.ai`, `platform.claude.com`의 443만 허용한다. 직접 외부 egress와 다른 proxy host는 거부한다. proxy는 TLS 본문이나 자격정보를 기록하지 않는다. private `CLAUDE_CODE_TMPDIR`/HOME/config 아래에서 `--init-only`와 `auth status`를 확인한 뒤 동일 경계의 cat/rg/curl canary를 검사한다. **현재 무모델 실측은 init-only 성공 후 private profile의 `loggedIn:false`로 중단되었다. Claude baseline은 blocked이며 자격 파일을 복사하거나 세션 디렉터리를 허용해서 우회하지 않는다.** 모델 호출을 통한 native tool 집행은 미검증이다.
