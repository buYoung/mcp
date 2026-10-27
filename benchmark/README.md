# codemap-search 1.0.0 벤치마크

고정 Grafana에서 탐색 도구 7개의 답변 품질과 비용을 비교한다. **모델별 2문항 × 7비교군 × 3회 = 42슬롯**, 동시 lifecycle 최대 4개, 같은 비교군은 순차 실행한다. 풀이 시간·호출·누적 토큰 제한은 없다. 운영자 취소와 환경 무결성 오류만 풀이를 중단한다. 채점기 제한 300초는 별도다.

## 고정 조건

| 항목 | 조건 |
|---|---|
| 소스 | Grafana main `c0a598f5a49abf2014f67e806075d8ad267c9efb`, 추적 파일 24,115개 |
| 문항 | `grafana-routes-v4` revision 3. 사용자 지정 Git OAuth 서버 통신(TypeScript→Go), 앱 플러그인 manifest 사용자 정의 route의 HTTP 배포 백엔드 통신(Go) |
| 기본 풀이 | GPT 6.1 Sol / medium, **Codex CLI 0.164.0 `codex exec`** |
| GPT 모드 | **Codex 제품 기본 `code_mode_only`(exec/wait)**. `codemode` 슬롯 키를 사용하지만 과거 pi codemode와 다른 구현이다 |
| 선택 풀이 | Claude Opus 5.5 / medium, **Claude Code 2.1.311 `claude -p`**, direct |
| 채점 | pi 1.4.0의 `openai-codex/gpt-6-astra` / high, 도구·MCP 없음 |
| 반복 | 선택한 모델의 단일 모드로 42회. 반복 라운드 사이에는 이전 solver lifecycle 완료를 기다린다 |
| 채점 항목 | 핵심 12개·확장 2개를 분리하고 correct·supported를 각각 판정 |

학습 기준일 이후(2026-07-01 이후) 코드를 대상으로 선택한 질문·고정 커밋·rubric은 변경하지 않았다. 프롬프트의 두 번째 줄은 제품군의 경우 `navigation MCP`, rg는 `기본 코딩 에이전트 도구`를 안내한다. 모델·모드 결과를 합산하지 않는다. 슬롯 ID는 `<question>-<arm>-<mode>-<attempt>`, spec 키는 `<arm>/<mode>`이며 GPT는 `codemode`, Claude는 `direct`다. 같은 모델도 과거 pi 실행 결과와 동일 조건으로 합산하지 않는다.

Codex 0.164.0의 내장 `models-manager/models.json`은 gpt-6.1-sol에 `tool_mode: code_mode_only`를 지정한다. `core/src/tools/mod.rs::requested_tool_mode`는 이를 feature flag보다 우선한다. **model catalog를 수정하지 않는다.** 하위 에이전트는 `agents.enabled=false`로 차단한다. 제품군의 shell·web·이미지 접근 도구는 exec 내부에서도 비활성화하지만 apply_patch·계획·시계 같은 나머지 기본 도구는 유지하며 쓰기는 OS read-only 권한으로 차단한다. rg는 기본 shell을 exec 안에서 사용하며 검색 제품 MCP가 없다. Claude rg는 Bash·Read·Glob·Grep만 제공한다.

| 비교군 | 고정 버전 |
|---|---|
| codemap-search | 0.14.0, 로컬 태그 `codemap-v0.14.0` (`fc0a368d0`) git_ref snapshot |
| CodeGraph | 1.6.2 |
| zvec-grep | 0.2.2 |
| Graphify | 0.9.95 |
| codebase-memory-mcp | 0.12.1 |
| Serena | 2.1.0, `--agent-interface tools` 단일 interface 예외 |
| 기본 코딩 에이전트(rg) | 추가 검색 제품 없음, rg 15.2.0 |

[실행 계약](data/grafana-execution-v1.json), [데이터셋](data/grafana-routes-v4/README.md), [비교 lock](data/grafana-comparison-targets-v1/README.md)이 정확한 조건이다. lock **개정 8**은 두 solver CLI의 npm integrity·전체 설치 tree·native executable SHA-256을 고정한다. pi toolchain은 채점용으로 유지한다. 시스템 PATH의 Codex/Claude를 대신 사용하지 않는다.

## 준비와 인증

macOS arm64, Python 3.12+, Node 22.19+, npm/pnpm, uv, Git, Cargo, Go/gopls가 필요하다. `CODEMAP_BENCH_CACHE`는 제품·하네스 이름 없는 중립 경로여야 한다(`codemap`, `codegraph`, `zvec`, `graphify`, `codebase-memory`, `serena`, `bench`, `route`, `campaign` 금지). 두 CLI는 `<cache>/cli-toolchain`, 채점 pi는 `<cache>/pi-toolchain`에 `npm ci --ignore-scripts --no-audit --no-fund`로 설치하고 매 재사용 때 hash를 검증한다. 개인 설치물·설정·extensions를 변경하지 않는다.

- Codex 인증은 CLI가 읽는다. 격리 CODEX_HOME에 운영자의 auth.json을 symlink하고 종료 후 링크만 지운다. 하네스는 자격 파일 내용을 열거나 복사하지 않는다.
- Claude 제품군은 기존 사용자 로그인을 사용할 수 있다. 명시적으로 제공한 `CLAUDE_CODE_OAUTH_TOKEN` 또는 `CODEMAP_BENCH_CLAUDE_TOKEN_FILE`도 지원한다. 두 공급원을 동시에 지정하지 않는다. 파일은 운영자 소유의 0600 regular file이어야 하며 토큰은 메모리와 CLI 환경에만 둔다.
- **Claude rg는 명시적 OAuth token이 필요하다.** outer Seatbelt는 keychain/Mach 접근까지 차단한다. 없으면 prepare와 실행 전 검증이 명확한 오류로 중단된다. 하네스가 로그인하거나 토큰을 출력하지 않는다. 실제 계정·모델 권한은 별도 운영자 확인 사항이다.
- Claude의 사용자/프로젝트 설정은 `--restricted --setting-sources ''`, 명시 MCP는 `--strict-mcp-config`, hooks·memory·CLAUDE.md·slash commands는 비활성화한다. admin managed policy는 CLI flags보다 우선하므로 존재만 검사하여 실패-폐쇄한다. 값을 읽지 않는다. 준비·시작·종료 시 확인한다.
- CLI 부모 환경은 allowlist로 만들며 provider URL·모델 override·개인 plugins/hooks를 상속하지 않는다. loopback test endpoint는 함수의 명시적 인자로만 켤 수 있다. Claude fake provider에는 반드시 별도 HOME와 더미 키를 쓴다.

```sh
export CODEMAP_BENCH_CACHE=/neutral/path
python3 -m benchmark.route_prepare prepare --solver-model all
python3 -m benchmark.route_prepare verify --solver-model all
printf '{}' | python3 -m benchmark.cli inspect-operator
printf '{}' | python3 -m benchmark.cli inspect-operator --solver-model claude-opus-5-5
pnpm bench:start
```

prepare는 설치·색인·semantic readiness·경계 canary·문맥 감사·오프라인 검증과 불변 계획 생성을 수행하며 실제 풀이/채점은 0회다. 각 모델의 `prepared_not_executed` readiness와 42-row ledger를 따로 게시한다. 코드·계약·CLI·도구 schema·source·index가 바뀌면 재준비해야 한다. 제품 artifact·source·환경과 최종 절대 경로가 같은 기존 색인은 재사용하지만 조회·client/context 증거는 새 backend로 다시 만든다. 다른 경로로 옮긴 index를 자동 재인증하지 않는다. Graphify는 hash-pinned requirements/wheel, Serena는 locked dependencies와 local Go/TypeScript LSP를 유지한다.

CUI는 정확한 모델·CLI 버전·모드·42슬롯·풀이 제한 없음·별도 채점 제한을 표시한다. 실행 확인 기본값은 아니요다. 준비 성공 자체는 실제 실행 승인이나 hosted 인증·모델 품질 검증이 아니다.

## 운영자 실행·검토·보고

JSON-lines CLI는 stdin으로 JSON object를 받는다. 아래 명령의 입력 파일은 solver가 읽을 수 없는 운영자 영역에 둔다. `catalog`, `prepare`, `approve-review`, `report`, `inspect-operator`는 모델을 호출하지 않는다. `run`, `resume`, `grade-sealed`만 승인 범위 내 모델 호출이 가능하다.

```sh
# run-approval.json: plan_id, readiness(불변 plans/<id>/readiness.json),
# readiness_sha256, solver_model, actor, confirmed:true를 명시
python3 -m benchmark.cli run < /private/operator/run-approval.json

# resume.json: run_root, confirmed:true. 미시작 슬롯까지 시작하려면 retry_unstarted:true
python3 -m benchmark.cli resume < /private/operator/resume.json

# blocked-sealed.json: run_root, confirmed:true — blocked campaign의 clean raw-seal 슬롯만 검토/채점
python3 -m benchmark.cli grade-sealed < /private/operator/blocked-sealed.json

# review.json: run_root, slot_id, approval(아래 형식)
python3 -m benchmark.cli approve-review < /private/operator/review.json

# report.json: run_root만 필요
python3 -m benchmark.cli report < /private/operator/report.json
```

스모크는 같은 `run` 명령과 수명주기를 쓰되 승인 JSON에 `"scope": {"kind": "smoke", "slot_ids": [...]}`를 넣는다. 지정한 슬롯만 실행·채점하고 나머지는 `out_of_scope`로 시작하지 않는다. 결과는 `campaigns/smoke-<campaign_id>/`에 따로 남고 보고서 첫머리에 스모크임을 표시하며 비교 결과로 쓰지 않는다. 범위를 생략하면 42개 슬롯 전체(`{"kind": "full"}`)다.

`RunManifest`와 승인·준비 기록은 불변이다. 실행 결과는 cache의 `route-preparation/campaigns/<campaign_id>/`에 따로 저장한다. `ledger.json`은 fsync·atomic replace로 갱신하며 단일 coordinator만 작성한다. solver/grader launch intent를 먼저 내구 저장한다. intent 이후 crash는 실행 여부가 불확실해도 자동 재실행하지 않는다. 정리가 확인되지 않은 process group/workspace는 `blocked`로 두고 추측으로 종료하거나 복원하지 않는다. 미시작 재개만 명시적 `retry_unstarted`로 허용하고 이전 preflight의 원자료도 보존한다. 모델 호출 확인과 CLI spawn은 별개이며 미관측 provider 호출은 null이다.

`grade-sealed`는 운영자가 명시적으로 CLI를 실행할 때만 정리 실패를 해제하지 않는 부분 채점 예외를 제공한다. `blocked` campaign에서 원자료 seal의 cleanup 완료·유효 조건·적격 답변이 확인된 슬롯만 고정 baseline과 기존 `build_review_packet` 경로로 채점 입력을 만든다. identity flag가 있으면 `review_required`로 멈추고 운영자가 `approve-review`로 승인·재봉인한 뒤 `grade-sealed`를 다시 실행해야 한다. **봉인된 packet만** 기존 Astra 승인 범위로 채점하며 실패 grader를 재시도하지 않는다. solver/product launch, workspace restore, cleanup 승인, 남은 슬롯 시작은 모두 금지한다. 실패·미시작 슬롯과 원자료를 보존하고 campaign은 계속 `blocked`(CLI exit 1)다. ledger와 보고서의 `blocked_grading`에 명시적 부분 채점 요청·대상·차단 원인·검증 완료 슬롯을 남긴다.

ready 실패는 `warmup_failed`로 기록하고 solver를 시작하지 않는다. 원자료는 writer 종료·정리 뒤 파일별 hash/bytes로 봉인한다. 조건이 유효하고 완성 답변임이 입증된 슬롯만 채점한다. 운영자 취소·환경 오류의 중단 시점 이후 답변, 미시작·취소·로그/모델 조건 무효는 실패·미채점 행으로 보존한다. 낮은 점수나 오류를 이유로 solver/grader를 자동 재시도하지 않는다.

도구·backend·모델 별칭, namespace, 경로, plan/run 식별, 사용량 자기보고는 private `reviews/<blind_id>/v1/draft.json`의 Unicode code-point 구간으로 검토한다. `approval`에는 `reviewer`, `revision`, `raw_sha256`, `draft_sha256`, 모든 flag의 `decisions`가 필요하다. 각 결정은 `flag_id`, `decision`, `evidence`를 포함한다.

- `legitimate_source_identifier`: 실제 source 근거를 기록하고 원문을 그대로 유지한다.
- `identity_only_anonymization`: identity 근거와 `replacement:"[anonymous-001]"` 같은 중립 표식만 허용한다. 승인된 정확한 구간 외 수정은 금지한다.
- `reject_packet`: 미채점으로 남긴다.

`approve-review`는 원답변/구간 hash·가역성을 확인하고 units/citations/schema/prompt를 다시 생성해 불변 packet을 봉인할 뿐 **모델을 호출하지 않는다**. 동일 승인 재적용은 기존 seal을 확인한다. 정상 campaign은 `resume`, blocked 예외는 `grade-sealed`에서만 sealed packet의 pi/Astra 채점을 진행한다. schema를 포함한 342 KiB 초과 prompt는 절단하지 않고 실패한다. 채점 packet은 고정 근거·기준(v3 기준 문항당 약 67~98 KB)에 답변과 인용 구간 원문(앞뒤 6줄 포함)을 더한다. v2 문항 스모크의 일반 답변(인용 40~46개)은 155~163 KB였다. 처음 정한 160 KiB에서는 rg 답변이 채점 실패해 342 KiB(Astra 입력 약 115K 토큰, 문맥 272K)로 올렸다. 155 KB packet의 채점은 입력 45,252토큰·50초였다. grader는 언제나 `openai-codex/gpt-6-astra`/`high`이며 `--no-tools --no-mcp`로 실행한다. prompt에 judgment schema를 포함하고 JSON object 하나(선택적으로 단일 json fence)만 받는다. 중복 key·NaN·부가 설명·여러 object를 거부하고 기존 ID·fact 일대일 대응·차원/사유·unit/citation membership까지 검사한다. private registry/검토 근거는 grader 입력에 넣지 않는다.

채점은 슬롯별 private `PI_CODING_AGENT_DIR`/TMPDIR와 allowlist 환경을 사용한다. 부모의 `PI_*`·모델 provider 환경·개인 extensions/skills/context/MCP를 상속하지 않는다. 고정 pi가 운영자 로그인에서 fresh bearer token을 메모리로 가져오고 private models.json에는 환경 변수 참조만 저장한다. 토큰은 argv·prompt·설정·로그에 쓰지 않으며 stdout/stderr는 저장 전에 redact한다. grader는 `--no-session`으로 pi session 저장을 끄고 redacted `events.jsonl`을 원자료로 보존한다.

하네스는 packet당 한 번만 실행한다. 형식·schema 오류, 빈 답변, timeout, nonzero exit를 채점 실패로 보존하고 자동 재채점하지 않는다. grader도 공통 pi final/identity 판정을 사용하여 빈 final, 마지막 aborted settlement, 관측된 upstream 모델 불일치를 적격 답변으로 받지 않는다. pi의 기본 transport retry(최대 3회)는 유지하며 `auto_retry_start/end`와 시도 횟수를 기록한다. compaction은 기본 설정을 바꾸지 않지만 발생하면 packet 문맥 변경으로 조건 무효·미채점 처리한다. assistant `message_end.usage`와 compaction usage의 `input + output + cacheRead + cacheWrite`를 합산하며 오류/중단 시 미관측 비용을 0으로 단정하지 않는다. `grader-result.json`, ledger/report의 `grader_execution`에는 실패 때도 사용량·`token_accounting`·retry/실패 attempt 수·compaction·상태·근거를 solver 비용과 별도로 남긴다.

보고서는 항상 42행이며 launch 예약·CLI 시작·provider 미관측·terminal·review/grade·failed/cancelled/not-started를 구분한다. warm/index 비용을 solver 비용과 합치지 않고, core/extended 분모와 macro/fact-weighted를 따로 표시한다. 미채점을 false/0점으로 바꾸지 않는다.

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

## 격리·warm host·출력

제품 source/home은 cache 아래 opaque 16hex 경로에 둔다. 제품 solver cwd는 별도의 `<cache>/<16hex>/work`, rg cwd는 opaque source다. CLI 임시 파일·Bash HOME·rg 복사본도 중립 경로다. 고정 checkout은 `.git` 이외 미추적/ignored 파일을 거부하며 제품 작업 사본에는 해당 제품 metadata만 허용한다. 소스 청결·symlink·hardlink·index 외부 경로 감사를 유지한다.

제품 stdio/HTTP/daemon/LSP는 전용 Seatbelt 안에서 실행한다. 해당 install/runtime·source/home·slot tmp만 허용하고 외부 파일과 임의 network를 거부한다. warm host는 같은 live process에서 제품별 semantic gate를 통과시킨 뒤 일회용 Unix socket/stdio shim으로 CLI를 연결한다. warm-up 트래픽은 solver 호출과 분리한다. ready 실패는 solver 시작 전 `warmup_failed`이며 제품 재시작·cold relay 우회·warm 상태 복원은 금지한다. solver clock은 준비·CLI 설정 작성 뒤 Popen 직전에 시작한다.

relay는 원본 instructions·description·schema·메타·결과를 보존한다. product capabilities/resources/prompts도 경로·도구 정책과 같은 admission/logging을 거친다. root/scope/path 별칭·배열·glob·file URI·metadata 접근을 검증하고 거절을 기록하되 요청을 몰래 다시 쓰지 않는다. discovery는 탐색 호출에서 제외한다. request cancellation은 admission 전/후를 구분하고 이미 dispatch한 RPC는 terminal까지 기록한 뒤 응답만 버린다. 제품/CLI 취소·정리·로그 손상·usage 완결성은 실패-폐쇄하며 자동 재풀이·재채점하지 않는다.

초기 준비의 MCP clientInfo는 선택 backend를 사용한다. 실제 attach 시에는 **실제 CLI initialize를 그대로 제품에 전달**한다. codemap이 `codex-mcp-client`에서만 주는 Codex 전용 안내를 readiness-probe identity로 누락시키지 않는다. 준비 원문 instructions와 live 원문을 비교한다. Codex 0.164.0은 initialize instructions를 첫 wire 요청에 자동 삽입하지 않아 하네스가 그 **원문만** developer_instructions로 전달한다. Claude는 native MCP instructions 경로를 쓴다. 긴 CodeGraph 안내를 CLI가 `… [truncated]` 접두로 렌더링하는 기본 동작은 유지하고, 문맥 감사는 정확한 원문 접두 일치만 예외로 인정한다. 제품 안내 내용을 재구성하거나 제품의 응답 동작을 모방하지 않는다.

### Client 출력 기준과 고정 소스 근거

제품 설정은 기본값이며 하네스가 응답을 자르거나 자동 페이지 조회·요약·보충하지 않는다.

| 경계 | 설정/기본값 | 근거 |
|---|---|---|
| 제품 응답 | 제품 기본 상한 유지(codemap은 Codex client에서 기본 100000×3.5 bytes) | codemap 0.14.0 configuration Client delivery budgets |
| Codex codemap MCP 도구 | `output_token_limit=100000` | 제품 `codex-config --server-name navigation`의 모든 도구 출력과 동일 |
| Codex 나머지 MCP 도구 | `N=ceil(B/4)` | 원문 UTF-8 text 기준. Serena 312500, 나머지 262144 |
| Codex exec/wait | **제품 기본 10000토큰**, 모델 pragma/인자가 지정하면 그 값 | `core/src/unified_exec/mod.rs::DEFAULT_MAX_OUTPUT_TOKENS/resolve_max_tokens`, `tools/code_mode/mod.rs::truncate_code_mode_result` |
| Codex 전역 history | 제품군만 `tool_output_token_limit`: 같은 비교군의 도구별 N; rg 미설정 | 실제 다음 요청에서 별도 history 절단 확인. exec/wait 기본 10000은 바꾸지 않음 |
| Claude codemap MCP | 제품 `_meta["anthropic/maxResultSizeChars"]=100000` 그대로 | codemap 0.14.0 configuration Client delivery budgets |
| Claude 메타 없는 MCP | 해당 키만 `500000` 보충, 기존 다른 메타 보존 | Claude Code 2.1.311 MCP converter `maxResultSizeChars: present ? Math.min(value, Eie) : default`, `Eie=500000` |
| Claude 별도 MCP token guard | 제품군만 `N=2×ceil(B/4)` | 2.1.311 본체 `d()`는 env 양수값 우선, 없으면 remote flag/기본 `E=25000` |
| rg native 출력 | 각 CLI 기본값 | MCP 출력 guard를 적용하지 않음. Codex code-mode exec의 기본 출력도 바꾸지 않음 |

공유 계산은 `client_delivery.output_limits` 하나다. pi 때 maxBytes는 정규화 텍스트 기준이며 client wire JSON 크기와 동일하지 않다.

| 비교군 | 기준 B (bytes) | Codex 도구별·전역 history N | Claude MAX_MCP_OUTPUT_TOKENS N |
|---|---:|---:|---:|
| codemap | 100000 | 100000 (제품 codex-config 그대로) | 50000 |
| Serena | 1250000 | 312500 | 625000 |
| CodeGraph·zvec·Graphify·CBM | 1048576 | 262144 | 524288 |
| rg | 미설정 | 제품 기본 | 제품 기본 |

Codex 고정 소스는 `openai/codex` tag `rust-v0.164.0`이다. `protocol/src/models.rs:2467–2503`은 text의 본문을 그대로 content item으로 바꾸며 wire JSON envelope/escape를 재지 않는다(M=B). structuredContent가 있으면 **그 JSON 직렬화만 선택**하고 text를 동시에 더하지 않는다. Serena의 B는 과거 text+structuredContent 중복까지 고려한 값이므로 더 작은 단일 structured 결과에도 같은 여유를 유지한다. `tools/context.rs:184–213`은 header와 20% history allowance를 적용한다. `protocol.rs:3523–3555`, `utils/string/src/truncate.rs:87–99`에 따라 tokens N의 byte budget은 **4N UTF-8 bytes**, allowance는 **4×ceil(1.2N)** bytes다(10000→48000). `tools/handlers/mcp.rs:274–278`의 per-tool limit은 model/global 기본보다 우선한다.

**제품 응답 → MCP 도구별 budget → exec pragma → 전역 history budget**은 별개다. code-mode nested MCP는 raw 객체를 script에 준다. 모델이 객체/여러 결과를 JSON.stringify하여 출력하면 원문 B보다 커질 수 있으며 별도 exec/history 예산의 절단 대상이다. 임의 envelope 상한을 발명하지 않는다. exec/wait 기본 10000토큰은 바꾸지 않고, 제품군의 전역 history만 위 도구별 N과 같게 올린다. rg 전역 history는 미설정이다. fake wire에서 큰 exec pragma로 1 MiB를 출력해도 기본 history는 약 48000 bytes로 줄었으며, 새 guard와 원문 출력으로 B-1 bytes 전체가 다음 요청에 도달함을 검증한다. raw rollout은 history 절단 전 원문을 저장하므로 `codex_output_truncated=0`은 최종 전달 절단 0의 증명이 아니다. raw exec 표식과 `codex_history_delivery`를 분리하고, 실제 다음 요청이 없는 hosted 슬롯은 history를 미관측/null로 둔다.

Claude 2.1.311 본체의 `AWe`는 text마다 `Qd(text)=Math.round(JS UTF-16 length/4)`를 합산하며 image는 1600으로 계산한다. `Pon`은 estimate≤N/2이면 API 없이 통과, text estimate>4N이면 즉시 초과, 중간은 `jGe`의 count_tokens API 결과>N을 사용한다(실패/없음이면 estimate). `Ion`은 초과를 N×4 문자로 잘라 `[OUTPUT TRUNCATED - exceeded N token limit]`을 붙인다. 문자 기반이며 UTF-8 바이트 절단이 아니다. `d()`의 우선순위는 양수 env N → remote flag → 25000이다. 고정 native binary의 raw offsets: guard module 251583394, `Pon/Ion` 같은 모듈; `Qd/Zs` 222443966; `jGe` 227547166. `jGe`에는 gateway 501 시 1-token sampling fallback도 있다. N=2×ceil(B/4)는 UTF-16 길이≤UTF-8 bytes인 text의 B 범위에서 N/2 fast path를 사용하도록 한다. B 초과 결과의 count API/추가 sampling은 별도 `claude_auxiliary_requests`에 관측하며 solver assistant/tool turn 비용과 혼합하지 않는다. CLI가 전체 network 로그를 공개하지 않으므로 미관측을 0회로 쓰지 않는다. 추가로 실제 call path `Ft`(raw offset 258900435)의 `hasResultSizeAnnotation`은 `ps`(258892xxx)에 전달되며, 유효 메타가 있는 순수 text는 `if (hasResultSizeAnnotation && !containsMedia) return`으로 이 token/count 경로를 우회한다. 현재 relay의 text 도구는 모두 이 분기를 사용해 B 초과에서도 문자 persistence만 관측될 수 있다. 계산/env는 유지하지만 실험을 통과시키려고 운영 메타를 제거하지 않는다.

Claude 메타가 없을 때 persistence ceiling은 50000자, 메타가 있으면 최대 500000자다. relay는 codemap의 native 값 100000을 올리지 않는다. **500000자는 1 MiB/1.25MB ASCII 결과를 온전히 전달하지 못하는 Claude 고유 상한**이다. fake provider에서 60000자는 두 제품군 모두 원문, 150000자는 codemap만 저장, 600000자는 두 제품군 모두 저장을 확인했다. 저장 후에는 모델에 미리보기/경로가 전달된다. `<persisted-output>` 표식과 실제 파일 count/bytes를 계수하며 파일 본문을 열지 않는다. `[OUTPUT TRUNCATED - exceeded ...]`도 보이는 경우 별도 계수하되 persisted file에 숨은 표식은 미관측이라고 표시한다.

### Native rg 경계와 Claude 흔적

Codex는 named read-only filesystem permission과 network deny를 사용한다. source·중립 경로의 고정 rg·최소 runtime 읽기, slot tmp 쓰기만 허용한다. 모델 문맥에 렌더링되는 denied paths에는 존재하는 source metadata만 넣는다. rg source는 제품 metadata가 없는 청결 상태이며 읽기 전용이므로, 존재하지 않는 제품 이름을 나열해 문맥을 오염시키지 않는다.

Claude rg는 전체 CLI를 하나의 outer Seatbelt로 감싼다(중첩 Seatbelt 없음). native Bash는 `CLAUDE_CODE_SHELL` private wrapper를 통해 PATH/HOME/TMPDIR/LANG/LC_ALL만 받은 zsh를 실행한다. 도구 목록·명령·기본 출력은 유지하며 auth/provider 환경은 제거한다. 외부 파일·source 쓰기·임의 network 외에도 lsopen·AppleEvent·Mach/keychain·다른 프로세스 정보·kern.proc를 거부한다. model traffic만 허용된 CONNECT proxy를 통한다. Bash 명령에 OAuth token을 주지 않으며 stdout/stderr/전사본은 저장 전 redact한다.

Claude Bash의 detached group은 private Unix socket의 Darwin LOCAL_PEERPID로 실제 연결 프로세스를 확인하고, CLI descendant와 PGID를 등록한 뒤에만 모델 명령을 실행한다. PID를 모델 입력에서 받지 않는다. 취소·정상 종료 모두 등록된 shell group까지 회수한 뒤 원자료를 봉인한다. 소유권을 입증하지 못한 PID는 추측으로 종료하지 않는다. Seatbelt는 VM 수준의 격리가 아니다.

Claude launch 전 정확한 workspace slug가 이미 존재하면 거부한다. 종료 후 모든 자체 transcript의 cwd binding을 대조하고 그 slug만 지운다. 다른 project/credential 파일은 읽거나 정리하지 않는다. 저장 결과는 파일 내용이 아닌 개수·크기로 계수한다. 자체 전사본은 사후 감사용으로 raw slot에 보존한다. `~/.claude.json`은 내용을 읽지 않고 크기·mtime·inode 변화만 기록하며 하네스가 편집하지 않는다.

## 문맥 감사와 지표

prepare/verify는 실제 pinned CLI와 loopback fake provider로 첫 instructions/system·tools·input을 캡처한다. Codex code mode는 추가 synthetic `ALL_TOOLS` 이름 조회로 nested 도구도 확인한다(실제 모델/검색 호출 아님). 제품군의 shell/network/subagent 혼입, rg의 검색 MCP 혼입을 거부한다. 모든 문항 프롬프트, 절대 경로 구성요소와 하네스 문구에서 `IDENTIFYING_PATH_WORDS` + 문항 ID + smoke/grafana를 검사한다. 원문 제품 instructions·description·schema·도구 식별자만 provenance-bound 예외이며 절대 경로는 예외가 아니다.

슬롯 후 Codex rollout의 system/developer/user/turn context와 Claude 자체 transcript의 모든 prompt snapshot·API request shape·system attachment·user 메시지를 다시 검사한다. 제품 tool-result 본문은 질문의 실제 소스이므로 하네스 문구로 취급하지 않는다. 감사 실패/필수 전사본 누락은 조건 무효다. 원자료·hash·위반 목록은 `context-audit.json`에 남는다.

토큰은 Codex의 response-identified token_usage_record와 누적/terminal reconciliation, Claude의 message/stream usage와 native result 누적 usage로 집계한다. 캐시 읽기 제외 입력(쓰기 포함)+출력 및 캐시 포함 전체를 함께 기록하며 clamp하지 않는다. 오류/중단·미완료 호출·손상 JSONL은 미관측 비용을 0으로 추정하지 않고 incomplete로 남긴다. 정상 완결성이 인증되지 않은 답변은 채점하지 않는다.

turns·호출 분포·relay dispatch/refusal·native 호출·CLI 절단·Claude persisted marker와 파일 count/bytes를 백엔드별로 기록한다. Claude API/tool duration은 자체 cost-state, 제품 tool duration은 relay clock을 사용한다. Codex가 native 로그로 공개하지 않는 전체 model-request duration은 null이며 wall time에서 도구 시간을 빼서 꾸며내지 않는다. 병렬 도구 구간 합을 wall time에 더하지 않는다. raw JSON/relay JSON/CLI 관측/실제 wire는 서로 다른 전달 증거다.

## 검증과 실행 경계

```sh
python3 -m unittest benchmark.checks benchmark.cui_checks
node --check benchmark/start.mjs
python3 -m py_compile benchmark/*.py
git diff --check
```

새 테스트 파일이나 lint/formatter setup은 추가하지 않는다. 오프라인 테스트와 pinned CLI loopback 검증은 hosted 모델 권한·실제 답변 품질·채점 보정·전체 Grafana 재준비를 대신하지 않는다. 새 계약의 불변 readiness와 독립 검토 없이 과거 pi 계획/승인을 재사용하거나 실제 모델을 실행하지 않는다. 무제한 풀이의 비용·로그·대기시간은 크게 증가할 수 있어 운영자 취소가 필요하다.
