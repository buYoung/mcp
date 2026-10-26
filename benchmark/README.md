# codemap-search 1.0.0 벤치마크

한 측정 묶음에서 같은 에이전트·모델·문항·예산으로 **코드 검색 도구별 최종 답변 품질과 탐색 비용**을 비교한다. 풀이 모델은 GPT 6.1 Sol 또는 Claude Opus 5.5 중 하나를 선택하고, 기본값은 GPT 6.1 Sol이다. 에이전트는 도구를 사용하는 공통 사용자이며, 답변 작성 능력의 영향은 공통 사용 조건으로 허용한다. 도구 선택과 호출 순서는 LLM에 맡기고 근거 단계별 분리 집계를 필수로 요구하지 않는다.

현재 하네스는 **Grafana 2문항 × 7개 비교군 × 3회 × 2모드 = 84회 예정**인 `grafana-routes` 하나만 지원한다. 설치·색인·중립 조회·채점 및 측정 계약·풀이 입력의 준비는 `prepared_not_executed`에서 종료한다. 실행은 불변 계획의 정확한 ID·hash·모델에 대한 별도 승인으로만 시작하며, 84-slot ledger에 실패와 미시작까지 보존한다. **제품별 격리·semantic ready warm-host와 runner의 clock 연결이 통합되지 않았다면 실행은 `blocked`로 끝난다.** 준비 성공만으로 실제 실행이 허가되거나 검증된 것은 아니다.

## 고정 조건

| 항목 | 조건 |
| --- | --- |
| 코드베이스 | Grafana `c6fad8695a96577eb466d425e6ac4a759ca30f47` |
| 문항 | 알림 규칙 평가 → Alertmanager 인계(Go), 브라우저 ↔ Grafana Live plugin stream 구독·수신·발행(TypeScript→Go). v3 원문은 보존하며, 식별어 없는 새 문항으로 교체하기 전 준비 감사가 실패하는 것이 정상이다 |
| 기본 풀이 | `openai-codex/gpt-6.1-sol / medium` — pi 1.4.0 |
| 선택 가능한 풀이 | `claude-bridge/claude-opus-5-5 / medium` — pi 1.4.0 |
| 채점 | 풀이 선택과 관계없이 `openai-codex/gpt-6-astra / high` — 고정 pi 1.4.0 |
| 횟수 | 문항·비교군·모드마다 3회, 총 84개 슬롯, 최대 4개 병렬. 반복 라운드 안에서 direct/codemode를 섞고 라운드 사이에는 이전 solver lifecycle 완료를 기다린다(채점에는 라운드 장벽 없음) |
| 한도 | 300초, 탐색 80회, 캐시 읽기를 제외한 누적 입력+출력 500,000토큰 |
| 채점 항목 | 핵심 16개와 확장 1개를 분리, `correct`·`supported`를 각각 판정. 사실별 `required_links`가 필수 연결이다 |

정확한 값은 [실행 계약](data/grafana-execution-v1.json)을 따른다. [고정 데이터셋](data/grafana-routes-v3/README.md)의 질문·정답·범위·채점 의미는 유지한다. 이전 3문항 데이터셋 `grafana-routes-v2`의 결과와 같은 점수로 섞지 않는다. 정답 자료를 풀이 대상 소스에 넣지 않는다.

한 번의 선택으로 7개 비교군에 같은 풀이 모델을 적용한다. 두 모델을 모두 자동 실행하거나 결과를 합산하지 않는다. `direct`와 `codemode`도 독립 조건이며 품질·비용을 합산하지 않는다. 슬롯 ID는 `<question>-<arm>-<mode>-<attempt>`, 실행 spec 키는 `<arm>/<mode>`다. 모델·실행기·추론 강도는 계획 ID와 결과에 포함하고 다른 모델의 결과가 들어오면 거부한다. 채점 항목과 Astra의 추론 강도는 그대로 유지한다. Claude의 `medium`은 [공식 기본값](https://code.claude.com/docs/en/model-config#choose-an-effort-level)을 명시한 값이며, 모델 간 동일한 계산량을 뜻하지 않는다.

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

[비교 대상 잠금](data/grafana-comparison-targets-v1/README.md) 개정 6이 pi toolchain과 공식 저장소·전체 커밋·아카이브 해시를 고정한다. codemap-search는 작업 트리가 아닌 release tag만 빌드하며 실제 버전·전체 commit·binary hash를 기록한다. 1.0.0 출시 목표를 이유로 제품 빌드를 1.0.0으로 표시하지 않는다. 구버전의 index/baseline/ready 기록은 최신 버전으로 재인증하지 않고 새 cache에서 다시 준비한다.

Graphify 0.9.95의 upstream `uv.lock`은 루트 버전과 필수 PyYAML이 맞지 않아 사용하지 않는다. bundle의 hash-pinned requirements로 공식 wheel을 설치하고 package 파일을 tag 원문과 대조하며, 준비 중 의존성을 다시 해석하지 않는다. Graphify·Serena 모두 `uv pip check`를 통과해야 한다. Graphify의 과거 `v1.0.0` release 대신 현재 upstream latest/PyPI가 일치하는 0.9.95를 사용한다.

## 준비 절차

현재 등록된 공식 바이너리 조합은 macOS arm64용이다. Python 3.12 이상, Node 22.19+ (실측 24.14.0), pnpm, npm, uv, Git, Rust/Cargo, Go, gopls가 필요하다. pi 1.4.0, `@buyong/pi-claude-bridge` 0.13.1, SDK 0.3.310와 bundled Claude Code 2.1.310은 cache 아래에 `npm ci --ignore-scripts`로 설치하며 lock integrity와 전체 설치 tree hash를 재사용 때마다 검증한다. host Node의 canonical 경로·버전·hash도 기록한다. `~/.pi/agent`의 설치물·설정·extensions·sessions는 사용하거나 수정하지 않는다. 최초 준비는 고정 소스·패키지·로컬 임베딩 가중치를 내려받으며 Astra 채점도 같은 고정 pi 설치물과 운영자의 pi `openai-codex` 로그인을 사용한다. 실제 모델 인증/사용 가능성은 별도 smoke 대상이다.

모든 명령은 저장소 루트에서 실행한다. CUI 의존성이 없다면 먼저 `pnpm install`을 실행한다.

준비 데이터(소스·설치물·색인·복원본·기록)는 `CODEMAP_BENCH_CACHE=<경로>`에 저장한다. 이때 Serena 작업 복사본도 그 아래에 둔다. 준비·검증·이후 실행은 같은 값을 사용해야 한다. source 경로가 cache root 아래에 있어 풀이 모델에게 보이므로, 경로에 제품·하네스 이름이 있으면 `prepare`가 거부한다. 지정하지 않을 때의 기본값 `benchmark/.cache/`도 `bench`가 들어 있어 쓸 수 없다. 2026-10-25 준비는 `/Volumes/Samsung_990_PRO/wscache`를 사용했다.

```sh
pnpm bench:ready
```

기본 명령은 GPT 6.1 Sol을 선택한다. Claude로 준비하려면 다음 명령을 사용한다.

```sh
python3 -m benchmark.route_prepare prepare --solver-model claude-opus-5-5
```

입력 해시 확인 → 도구 설치·빌드 → 전체 Grafana 소스 대조 → 제품별 색인 → 중립 조회·종료·상태 복원 → 채점·측정 계약 확인 → 기존 오프라인 검증 → 84개 풀이 입력과 미실행 계획 생성을 수행한다. 제품 도구의 로컬 임베딩 계산은 포함하며 풀이·채점 LLM은 호출하지 않는다.

완료 후 현재 파일과 준비 기록을 대조한다.

```sh
python3 -m benchmark.route_prepare verify
```

Claude 준비 기록은 `python3 -m benchmark.route_prepare verify --solver-model claude-opus-5-5`로 확인한다.

성공 결과는 `prepared_not_executed: <계획 ID>`다. GPT의 [최종 준비 기록](../docs/briefs/evidence/bench-ready/07-readiness.json)과 Claude의 `07-readiness-claude-opus-5-5.json`은 따로 저장된다. 선택한 기록의 `technical_ready`가 true이고 `solver_runs`, `grader_runs`, `calibration_runs`가 모두 0인지 확인한다. 같은 계획의 `benchmark/.cache/route-preparation/plans/<계획 ID>/`에 풀이 입력과 미실행 결과표가 저장된다. 각 계획에는 handoff·참조 근거·inventory·probe·corpus의 불변 snapshot이 포함된다. 다른 모델을 준비해 최신 포인터를 다시 게시해도 기존 계획의 바이트를 덮어쓰지 않는다. `plan_condition`에서 ID를 재계산하며, 코드·모델·제품 조건이 바뀌면 기존 계획을 재인증하지 않고 준비를 다시 해야 한다. 미완성 계획 디렉터리도 자동 덮어쓰지 않는다.

공유 설치·색인·계약 검증과 공통 pi backend의 중립 probe를 한 번 수행하고 두 provider의 계획을 각각 finalize하려면 다음을 사용한다. 이는 두 모델의 **준비**이며 자동 실행이 아니다. 기본 준비/verify/inspect 경로는 과거 Codex·Claude CLI의 help/login probe를 호출하지 않는다. pi toolchain은 `03-artifacts.json`의 별도 artifact이며 host 기록에 private dummy-key `auth check`, offline 모델 목록(두 solver와 Astra), 필요한 CLI flag의 무모델 probe를 보존한다. 이 검사는 실제 운영자 인증이나 모델 권한을 인증하지 않는다.

```sh
python3 -m benchmark.route_prepare prepare --solver-model all
python3 -m benchmark.route_prepare verify --solver-model all
```

준비·실행 승인 CUI는 다음 명령으로 연다.

```sh
pnpm bench:start
```

Grafana 프로필 다음에 풀이 모델을 선택한다. 모델 메뉴에서 Enter를 누르면 기본값 GPT 6.1 Sol이다. 선택한 모델로 7개 비교군 모두를 준비한 뒤 정확한 계획 ID·readiness hash·모델·버전·84회·한도를 표시한다. 별도 실행 확인의 기본값은 **아니요**다. 확인을 거절하면 풀이·채점 0회로 끝난다. Ctrl+C는 새 슬롯을 중단하고 자식 정리 후 ledger/report 위치를 안내한다. 준비와 실행의 lease는 cache/model 구분 없이 공유되며, host 공통 port lease를 먼저 얻는다. 다른 프로세스의 포트를 빼앗거나 종료하지 않는다.

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

스모크는 같은 `run` 명령과 수명주기를 쓰되 승인 JSON에 `"scope": {"kind": "smoke", "slot_ids": [...]}`를 넣는다. 지정한 슬롯만 실행·채점하고 나머지는 `out_of_scope`로 시작하지 않는다. 결과는 `campaigns/smoke-<campaign_id>/`에 따로 남고 보고서 첫머리에 스모크임을 표시하며 비교 결과로 쓰지 않는다. 범위를 생략하면 84개 슬롯 전체(`{"kind": "full"}`)다.

`RunManifest`와 승인·준비 기록은 불변이다. 실행 결과는 cache의 `route-preparation/campaigns/<campaign_id>/`에 따로 저장한다. `ledger.json`은 fsync·atomic replace로 갱신하며 단일 coordinator만 작성한다. solver/grader launch intent를 먼저 내구 저장한다. intent 이후 crash는 실행 여부가 불확실해도 자동 재실행하지 않는다. 정리가 확인되지 않은 process group/workspace는 `blocked`로 두고 추측으로 종료하거나 복원하지 않는다. 미시작 재개만 명시적 `retry_unstarted`로 허용하고 이전 preflight의 원자료도 보존한다. 모델 호출 확인과 CLI spawn은 별개이며 미관측 provider 호출은 null이다.

`grade-sealed`는 운영자가 명시적으로 CLI를 실행할 때만 정리 실패를 해제하지 않는 부분 채점 예외를 제공한다. `blocked` campaign에서 원자료 seal의 cleanup 완료·유효 조건·적격 답변이 확인된 슬롯만 고정 baseline과 기존 `build_review_packet` 경로로 채점 입력을 만든다. identity flag가 있으면 `review_required`로 멈추고 운영자가 `approve-review`로 승인·재봉인한 뒤 `grade-sealed`를 다시 실행해야 한다. **봉인된 packet만** 기존 Astra 승인 범위로 채점하며 실패 grader를 재시도하지 않는다. solver/product launch, workspace restore, cleanup 승인, 남은 슬롯 시작은 모두 금지한다. 실패·미시작 슬롯과 원자료를 보존하고 campaign은 계속 `blocked`(CLI exit 1)다. ledger와 보고서의 `blocked_grading`에 명시적 부분 채점 요청·대상·차단 원인·검증 완료 슬롯을 남긴다.

ready 실패는 `warmup_failed`로 기록하고 solver를 시작하지 않는다. 원자료는 writer 종료·정리 뒤 파일별 hash/bytes로 봉인한다. 조건이 유효하고 cutoff 이전 완성 답변임이 입증된 슬롯만 채점한다. late answer, 미시작·취소·로그/모델 조건 무효는 실패·미채점 행으로 보존한다. 낮은 점수나 오류를 이유로 solver/grader를 자동 재시도하지 않는다.

도구·backend·모델 별칭, namespace, 경로, plan/run 식별, 사용량 자기보고는 private `reviews/<blind_id>/v1/draft.json`의 Unicode code-point 구간으로 검토한다. `approval`에는 `reviewer`, `revision`, `raw_sha256`, `draft_sha256`, 모든 flag의 `decisions`가 필요하다. 각 결정은 `flag_id`, `decision`, `evidence`를 포함한다.

- `legitimate_source_identifier`: 실제 source 근거를 기록하고 원문을 그대로 유지한다.
- `identity_only_anonymization`: identity 근거와 `replacement:"[anonymous-001]"` 같은 중립 표식만 허용한다. 승인된 정확한 구간 외 수정은 금지한다.
- `reject_packet`: 미채점으로 남긴다.

`approve-review`는 원답변/구간 hash·가역성을 확인하고 units/citations/schema/prompt를 다시 생성해 불변 packet을 봉인할 뿐 **모델을 호출하지 않는다**. 동일 승인 재적용은 기존 seal을 확인한다. 정상 campaign은 `resume`, blocked 예외는 `grade-sealed`에서만 sealed packet의 pi/Astra 채점을 진행한다. schema를 포함한 384 KiB 초과 prompt는 절단하지 않고 실패한다. 채점 packet은 고정 근거·기준(v3 기준 문항당 약 67~98 KB)에 답변과 인용 구간 원문(앞뒤 6줄 포함)을 더한다. v2 문항 스모크의 일반 답변(인용 40~46개)은 155~163 KB였다. 처음 정한 160 KiB에서는 rg 답변이 채점 실패해 384 KiB(Astra 입력 약 115K 토큰, 문맥 272K)로 올렸다. 155 KB packet의 채점은 입력 45,252토큰·50초였다. grader는 언제나 `openai-codex/gpt-6-astra`/`high`이며 `--no-tools --no-mcp`로 실행한다. prompt에 judgment schema를 포함하고 JSON object 하나(선택적으로 단일 json fence)만 받는다. 중복 key·NaN·부가 설명·여러 object를 거부하고 기존 ID·fact 일대일 대응·차원/사유·unit/citation membership까지 검사한다. private registry/검토 근거는 grader 입력에 넣지 않는다.

채점은 슬롯별 private `PI_CODING_AGENT_DIR`/TMPDIR와 allowlist 환경을 사용한다. 부모의 `PI_*`·모델 provider 환경·개인 extensions/skills/context/MCP를 상속하지 않는다. 고정 pi가 운영자 로그인에서 fresh bearer token을 메모리로 가져오고 private models.json에는 환경 변수 참조만 저장한다. 토큰은 argv·prompt·설정·로그에 쓰지 않으며 stdout/stderr는 저장 전에 redact한다. grader는 `--no-session`으로 pi session 저장을 끄고 redacted `events.jsonl`을 원자료로 보존한다.

하네스는 packet당 한 번만 실행한다. 형식·schema 오류, 빈 답변, timeout, nonzero exit를 채점 실패로 보존하고 자동 재채점하지 않는다. grader도 공통 pi final/identity 판정을 사용하여 빈 final, 마지막 aborted settlement, 관측된 upstream 모델 불일치를 적격 답변으로 받지 않는다. pi의 기본 transport retry(최대 3회)는 유지하며 `auto_retry_start/end`와 시도 횟수를 기록한다. compaction은 기본 설정을 바꾸지 않지만 발생하면 packet 문맥 변경으로 조건 무효·미채점 처리한다. assistant `message_end.usage`와 compaction usage의 `input + output + cacheRead + cacheWrite`를 합산하며 오류/중단 시 미관측 비용을 0으로 단정하지 않는다. `grader-result.json`, ledger/report의 `grader_execution`에는 실패 때도 사용량·`token_accounting`·retry/실패 attempt 수·compaction·상태·근거를 solver 비용과 별도로 남긴다.

보고서는 항상 84행이며 launch 예약·CLI 시작·provider 미관측·terminal·review/grade·failed/cancelled/not-started를 구분한다. warm/index 비용을 solver 비용과 합치지 않고, core/extended 분모와 macro/fact-weighted를 따로 표시한다. 미채점을 false/0점으로 바꾸지 않는다.

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

### 준비 색인 시간 (실측)

고정 Grafana(추적 파일 22,898개), macOS 15.7.1 arm64 14코어, 외장 NVMe(Thunderbolt) cache에서 각 제품의 색인 명령이 걸린 wall time이다. 준비 기록 `plans/<plan_id>/readiness.json`의 `arms.<arm>.commands[].elapsed_seconds`가 원자료다. 비교 조건은 색인을 미리 끝낸 상태이므로 이 시간은 풀이 시간과 비용에 포함하지 않는다.

| 도구 | 색인 명령 | 2026-10-25 (`wscache`) | 2026-10-24 (다른 작업과 동시 실행) |
|---|---|---:|---:|
| Serena 2.1.0 | `serena project index` (Go·TypeScript LSP) | 2,011.2초 (약 34분) | 4,185.3초 (약 70분) |
| graphify 0.9.95 | `graphify extract --code-only` | 264.9초 | 255.5초 |
| zvec-grep 0.2.2 | `index --embedding local/potion-code-16m…` | 131.9초 | 123.8초 |
| codemap-search 0.13.0 | `codemap-search index .` | 90.3초 | 68.8초 |
| codegraph 1.6.2 | `codegraph init --yes` | 77.5초 | 64.1초 |
| codebase-memory-mcp 0.12.1 | `cli index_repository` | 29.0초 | 26.3초 |
| rg 15.2.0 | 색인 없음 | — | — |

새 프로세스의 semantic gate(준비 확인)는 2026-10-25 기준 codemap-search 24.6초, Serena 23.8초, zvec-grep 9.3초, codebase-memory-mcp 2.9초, codegraph 2.5초, graphify 1.3초였다. 전체 준비(설치·빌드·소스 취득·색인·준비 확인·계획 생성)는 1시간 10분이 걸렸다.

### 준비 색인과 live query-ready

`prepared_index_ready`는 최종 절대 source/home 경로에서 색인 명령 완료·디스크 상태를 검증하고 **새 프로세스**의 semantic gate를 통과한 기록이다. 준비는 같은 `warm_host.prepare_session`을 사용하여 gate → disposable 본문/세션 검증 → `isolation.boundary_canaries`·index path audit → 제품/daemon 정리 → baseline 복원 순서로 수행한다. 실패해도 정리가 확인된 뒤에만 복원하며 실패한 arm은 준비를 중단시킨다. 기존 `isError=false` 또는 initialize/port-open만으로는 ready가 아니다.

| arm | solver 시작 전 같은 live process의 gate | disposable 준비 검증 |
| --- | --- | --- |
| codemap-search | committed index/root 확인, root `overview`의 양수 파일·심볼과 warming/stale/unavailable 부재 | neutral indexed search/read, 실제 initialize 뒤 첫 read의 dedup 복원 |
| CodeGraph | complete/version/pending/freshness 준비 근거, source를 포함하는 nonempty explore | 반복·재-initialize 응답 동일성; native dedup off 유지 |
| zvec-grep | 최종 manifest/root/model hash, serving query의 `freshness: fresh`와 refresh 부재 | warm HTTP session DELETE 후 새 session에서도 fresh; server PID 유지 |
| Graphify | 최종 graph hash, 양수 node/edge `graph_stats` | `query_graph` 본문 검증; live warm-up에서는 querylog를 만들지 않음 |
| codebase-memory-mcp | root/runtime/cache·coverage/count 일치, ready status + graph + live-source search/snippet | 같은 gate와 경계 canary; graph-only 성공은 불충분 |
| Serena | Go/TS index 완료·실패 0·cache/root, ready config + 두 언어 symbol 및 실제 LSP reference | timeout 후 진행 로그 거부; warm-up에 `initial_instructions`를 사용하지 않음 |
| rg | 고정 executable·준비된 native 권한 계약 | index/MCP warm-up 없음; baseline 경계 probe는 별도 수행 |

연속 2회 semantic gate를 deadline 안에 통과해야 한다. 실제 warming/stale/`Error executing` 응답은 거부하되 일반 안내문 속 `warming`, CodeGraph의 “not a stale cache”, codemap의 `0 unavailable`은 실패로 오인하지 않는다. native truncation·통계의 partial collection은 삭제하거나 보충하지 않는다. 표본 조회 성공은 모든 가능한 질의의 정확성 보장이 아니다.

각 슬롯은 restore 이후 **별도의 `live_query_ready` receipt**를 받는다. 제품만 `isolation.product_profile(..., slot_tmp=<arm home의 private tmp>)`로 격리하고, `MCPClient`가 stdio/HTTP/shutdown launch를 wrap한다. 신뢰하는 harness host·shim에는 nested Seatbelt를 적용하지 않는다. warm host는 campaign 내부에서 제품 그룹을 소유하고, 0700 slot-private 디렉터리의 0600 Unix socket 하나를 공개한다. socket 경로가 Darwin 한도에 맞지 않으면 더 짧은 cache root가 필요하다. 실제 client initialize를 제품에 전달하며 zvec만 warm HTTP session을 DELETE하고 solver가 새 session을 시작한다. 제품은 재시작하지 않는다.

`benchmark.stdio_shim`은 byte-preserving stdio↔socket 연결만 수행한다. 두 번째 연결/재접속은 거부하며 EOF·운영자 취소 시 소유 제품/LSP/HTTP/daemon을 정리한다. warm 트래픽은 `warmup.jsonl`·`warm-native.jsonl`에 별도로 기록하고 solver의 `relay.jsonl`·`native.jsonl` 및 80-call 예산에 넣지 않는다. runner는 준비 receipt·attachment·CLI 설정 작성 뒤, **Popen 직전 t0**에서 solver gate를 연다. initialize·attach는 solver 시간에 포함되지만 색인·warm 시간은 포함되지 않는다. ready 뒤에는 제품 정리 전 source/home을 복원하지 않는다.

준비/실행 기록에는 index build·restore·initialize·첫 usable query·stable ready·attach·cleanup 시간, load average·CPU/memory process snapshot, assertion/transcript hash, generation·profile hash·소유 PID/PGID를 남긴다. attach는 실제 연결 시 `attachment.json`, cleanup은 `warm-cleanup.json`에 기록한다. 준비 중 solver attach는 없으므로 해당 시간은 null이다. warm 실패는 `warmup_failed`이며 solver/model 호출은 0회다. 실행 중 readiness 상실은 제품별 색인·세션 상태 표시(zvec의 선두 `freshness:`/`background_refresh:`, codemap의 warming·index unavailable·symbol index stale 문구, CodeGraph의 sync·미색인 경고, Serena의 `No active project.`·언어 서버 오류·LS 상태, Graphify의 graph 미로드, CBM `index_status`)에서만 판정한다. MCP `isError`, JSON 오류 객체, Serena의 일반 `Error executing tool:`처럼 요청 때문에 생기는 도구 오류는 제품의 정상 응답으로 전달하고 `tool_error_observed`로만 기록한다(준비 판정에서는 여전히 실패다). `tools/call`의 중계 거부(경로·인자·예산)와 제품 JSON-RPC 오류는 같은 메시지의 `isError: true` 도구 결과로 client에 전달하고 원래 오류는 `error_as_tool_result`로 남긴다. pi-mcp-adapter는 도구 호출의 프로토콜 오류마다 서버 연결을 닫고 다음 호출 때 다시 연결하는데, 일회용 attachment는 재연결을 받을 수 없기 때문이다(운영 경로 재현: 경로 거부 직후 나머지 호출이 `Connection closed`). 반환된 저장소 본문의 `timeout waiting`, `not indexed`, `stale` 등은 검사하지 않는다. 감지 시 원응답을 보존하고 **그 슬롯만** condition/environment error로 종료하며 자동 재풀이하지 않는다. client `notifications/cancelled`는 조건 오류가 아니다. requestId별로 기록하고 아직 admission 전인 요청은 건너뛰며 예산을 소비하지 않는다. 이미 dispatch한 RPC는 원래 예산·terminal 기록을 유지하고 끝까지 처리한 뒤 응답만 버린다. 연결을 유지하여 다음 호출을 받는다. 운영자 취소·cleanup 실패는 여전히 campaign 전체 중단 사유다. `isolation.py` 통합 및 새 준비 snapshot 검증 없이는 cold relay로 우회하지 않는다.

## 도구와 측정 정책

- 제품 응답 설정은 기본값을 유지하되 **pi-mcp-adapter 7.0.0**의 client `settings.outputGuard`를 명시한다. adapter 기본 50 KiB/2,000줄은 codemap 기본 100,000바이트보다 작아 원문 전달을 잘랐으므로 측정 조건으로 유지하지 않는다. `maxBytes`는 아래 표, `maxLines=maxBytes+1`로 바이트 제한이 먼저 걸린다. 전역 설정만 제공하는 adapter를 슬롯마다 구성하므로 제품별 값을 적용한다. 무상한 제품의 1 MiB는 제품 상한이라고 주장하지 않으며 초과 시 절단을 관측·보고한다. 자동 페이지 조회·요약·보충은 하지 않는다. rg의 pi bash 기본 50 KiB/2,000줄과 codemode 기본 script 출력 예산은 변경하지 않는다. adapter 원문 전달, script 출력, Claude Code의 후속 전달 제한은 서로 다른 경계다.

- codemap-search의 `filesystem_permissions`(find·grep·read)는 native scaffold의 **`anywhere`**를 변경 없이 사용한다. `isolation.py`의 제품 child Seatbelt와 제품별 relay 정책이 경계를 집행한다. 별칭 `query`, file URI, 배열·glob·project selector도 거부·기록 대상이다. 동일 프로세스 pre-warm 통합과 전체 corpus 재준비 전에는 소형 probe 성공만으로 실행을 승인하지 않는다.
- 제품 자체 도구를 우선한다. Graphify에는 원문 본문 조회를 위해 `read`만 보조로 제공한다. zvec-grep의 후속 grep/rg 안내는 보조 도구 추가 사유로 쓰지 않는다. 낮은 품질이나 짧은 응답으로 보조 도구를 늘리지 않는다.
- `plain rg`는 pi의 **bash만** 사용하는 기준군이다 (direct: `--tools bash --no-mcp`, codemode: `--tools bash,codemode --no-mcp`). in-process read/grep/find/ls는 노출하지 않는다. 슬롯별 `shellPath` wrapper가 admission lock 안에서 호출/stop을 원자적으로 검사한다. 80번째는 허용하고 81번째는 실행 전에 거부·기록한다. 명령 하나 안의 복합 shell 명령은 한 호출이며 `tool_execution_start`와 교차 대조한다. wrapper 밖 pi는 신뢰하는 model client이고, shell만 Seatbelt + `env -i`로 실행한다. source·고정 rg·시스템·pi 전용 TMPDIR 읽기, tmp 쓰기만 허용하며 source metadata/정답·다른 작업 트리·세션·캐시·공용 TMPDIR와 network는 거부한다. PATH는 고정 rg의 바이트 동일 복사본 디렉터리와 `/usr/bin:/bin`, HOME/TMPDIR는 슬롯 전용이며 token/`PI_*`는 shell에 전달하지 않는다. pi의 기본 출력 절단과 full-output 임시 파일 정책을 유지한다. CONNECT proxy는 쓰지 않는다. baseline은 `lsopen`, `appleevent-send`, `mach-lookup`과 다른 프로세스 정보(`process-info*` target others, `kern.proc` sysctl)도 거부하며 기본 bash/rg/coreutils에 필요한 Mach lookup 예외는 없다. 준비 canary는 `open -a`, 실제 AppleEvent 전송, `launchctl submit`, `ps -ax`와 perl `sysctl(KERN_PROC_ALL)` 거절, TMPDIR/HOME/PATH에 하네스·제품 이름이 없음과 bash·rg·cat·ls·find·grep·sed·awk·head·wc 동작을 확인한다. 실행 뒤 `bash-calls.jsonl`의 명령도 운영자가 감사해야 하며 VM 수준 격리로 간주하지 않는다.
- Serena 2.1.0의 native interface 기본값은 임의 Python 실행을 노출하는 REPL이다. 검색 전용 계약의 단일 interface 예외로 **`--agent-interface tools`**를 사용하고 native default와 예외 이유를 prepared state에 기록한다. `serena_repl`이 검색 도구 목록에 남으면 준비를 거부한다. 전용 `SERENA_HOME`에서 Grafana 전체 루트를 프로젝트로 등록하고 `go`, `typescript` 언어 서버로 `project index`를 실행한다. 언어별 색인 완료·캐시와 MCP 심볼 조회를 확인한 상태에서 측정을 준비한다. 프로젝트 전환·수정·셸·메모리 기능은 제외하고 원문 읽기와 심볼 검색 등 제품 도구를 제공한다. 응답 한도 150,000자·도구 시간 한도 240초·심볼 정보 예산 10초는 고정 버전의 기본값이다. 언어 서버·Go 캐시도 복원 대상에 포함한다. 상위 하네스의 Node 의존성이 섞이지 않도록 Serena는 ancestor에 `node_modules`가 있는 cache root를 거부한다. 모든 arm의 작업 복사본·홈은 `<CODEMAP_BENCH_CACHE>/<opaque hex>/{source,home}`에 놓고 정확한 경로를 준비 기록에 남긴다. 이 경로는 측정 전까지 보존해야 한다. [고정 버전 설정](https://github.com/oraios/serena/blob/5e4f5cd173c7ff3eed37ee3f1de6c266b937c5dc/src/serena/resources/serena_config.template.yml)을 따른다.
- 검색·색인은 로컬 조건이다. Jev는 비활성, Graphify는 빈 출력 디렉터리에서 코드 전용 추출, zvec는 고정 리비전의 `local/potion-code-16m-v2`를 사용한다. zvec index와 MCP 모두 `ZVEC_GREP_MODEL_CACHE=<home>/model-cache`를 공유한다. 제품의 제외 규칙과 선택적 파서 누락은 그대로 기록한다.
- CodeGraph는 `status --json`의 complete·버전·pending refs/changes를 확인한다. zvec는 check-ready 및 `freshness: fresh`, Graphify는 양수 `graph_stats`와 오류 텍스트 부재, Serena는 tools interface·LSP ready·심볼 응답과 timeout 후 진행 로그 부재를 확인한다. codemap의 warming/stale 응답도 성공으로 받지 않는다. exit 0 또는 `isError=false`만으로 준비 완료를 판정하지 않는다. 전체 corpus의 deeper semantic gate와 live warm hand-off는 별도 검증이 필요하다.
- CBM 0.12.1은 `index_status`의 ready·project·root를 대조한다. 모든 index/MCP/shutdown은 `CBM_RUNTIME_DIR=<CODEMAP_BENCH_CACHE>/cbmrt`를 공유하며 canonical socket 경로가 104 bytes 미만이어야 한다. 길면 더 짧은 cache root를 지정한다. 현재 전용 loopback 포트는 zvec 19700, CBM UI 19701이다. `detect_changes(scope="tests")`는 schema를 유지하며 요청마다 별도 graph를 생성하는 제품 동작이므로 사전 호출로 제거할 수 없는 실행 비용/위험이다.
- 고정 checkout은 `.git` 이외의 미추적·ignored 파일이 하나라도 있으면 prepare/verify/실행 전후 검증을 실패시키고 해당 경로를 보고한다. APFS clone 전 원본과 색인 전 작업 사본도 검사한다. 색인 후에는 제품별 root metadata(`.codemap`, `.codegraph`, `.zvec-grep`, `.serena`, `.codebase-memory`; Graphify·rg는 없음)만 허용한다. 다른 파일을 삭제·숨기거나 Git ignore로 제외하여 통과시키지 않는다.
- 22,898개 추적 파일과 생성된 TypeScript·JSON을 동일하게 제공한다. 문항마다 같은 절대 경로에 조회 전 상태를 복원하도록 준비한다. OS 페이지 캐시는 통제하지 않는다.
- 제품군은 MCP 탐색 시도(`tools/call` 및 `resources/*`·`prompts/*`의 콘텐츠 반환 메서드)를 공통 admission lock에서 예약한다. 경로·도구 거절도 시도 예산을 소비하며 `attempt`·`terminal`·`budget` 기록에 남기되 실제 dispatch와 구분한다. 80번째는 완료와 남은 예산 내 답변 작성을 허용하고, 81번째는 dispatch 전에 거부하며 cutoff를 고정한다. 제품 RPC 동안 lock을 잡지 않는다.
- pi 제품군은 relay의 MCP admission을, pi 기준군은 wrapper admission을 권위 있는 호출 예산으로 사용한다. 잘못된 schema 때문에 pi 자체에서 거절되어 relay까지 도달하지 않은 tool event는 host 시도로만 관측한다. 기존 Codex/Claude Code CLI 경로는 보존하되 기본 campaign에서 사용하지 않는다.
- usage drain은 0초다. cutoff 즉시 SIGINT를 보내고 이후 SIGTERM·SIGKILL 및 owned process-group 회수 중 출력·사용량은 비용으로 보존한다. CLI 리더가 먼저 종료해도 같은 그룹의 자식과 독립 제품 그룹을 정리한다. wall cutoff는 늦게 감지한 시각이 아니라 `solver_start + 300초`다. 모든 backend는 cutoff 이전의 완전한 final을 입증해야 한다. pi는 `agent_settled` 관측 후 마지막 assistant text만 답변으로 인정한다. stdout/stderr를 non-blocking으로 EAGAIN까지 drain하고, 파싱한 event와 증분 usage/tool/final 상태를 메모리에 유지한다. hot loop에서 events.jsonl을 재독하지 않으며 원시 바이트는 token redaction 외 변경 없이 보존한다. 완료 시각은 해당 record의 마지막 바이트를 읽은 monotonic 시각이다. tool 종료/settlement의 record 번호·read 시각은 runtime의 `event_observations`에 남긴다. 기존 Codex 경로는 rollout timestamp도 대조한다. 늦거나 적격성을 입증하지 못한 답변은 원문·SHA256을 별도 보존하고 채점 답변에서 제외한다.
- 토큰 한도는 **캐시 읽기를 제외한** 누적 입력(캐시 쓰기 포함)과 출력의 합(`total_tokens - cached_input_tokens`)으로 판정한다. 프로필의 `token_limit_basis`가 이 기준을 고정하며 풀이 프롬프트와 CUI도 같은 기준을 표시한다. pi는 매 턴 전체 문맥을 다시 보내 캐시 읽기가 턴마다 누적되므로, 캐시 포함 합계로는 실제 문맥과 무관하게 약 20턴에서 한도에 닿기 때문이다. 캐시 포함 합계도 `observed_total_tokens_including_cache_reads`로 함께 기록한다. 이 기준의 관측 토큰이 500,000을 초과하면 조건 무효다. 종료 직전·회수 중 증가분도 재검사하며 관측값을 한도로 clamp하지 않는다. 전체 사용량이 미인증이면 완전한 비용/예산 준수를 주장하지 않는다. 단, 아래의 성공한 pi 자동 retry에 한해서는 승인된 하한 합계로 예산·채점 적격성을 판단한다. 즉시 결과와 재집계는 같은 사용량 reconciliation을 사용한다.
- 제품군 pi는 `-e <고정 pi-mcp-adapter> --mcp-config <private mcp-adapter.json>`과 `PI_MCP_CONFIG_MODE=exclusive`로 실행한다. adapter는 그 파일 하나만 읽고 `~/.config/mcp`, `~/.agents`, `.mcp.json`, `.pi/`, Pi 패키지, Claude 플러그인, 호스트 설정을 읽지 않는다. 서버는 navigation 하나이며 `lifecycle: "lazy-keep-alive"`와 `directTools: true`, 환경 변수 `MCP_DIRECT_TOOLS=navigation`이다. 일회용 shim은 연결을 한 번만 받는데 `keep-alive`는 확장 로드 때와 세션 시작 때 두 번 연결해 두 번째가 실패한다(운영 경로에서 3/3 재현). `lazy-keep-alive`는 세션 시작 때 한 번 연결해 유지하고, 캐시가 없는 `MCP_DIRECT_TOOLS` 선택은 session start가 연결·도구 등록을 끝까지 기다리게 한다. 설정은 기본값을 유지하되 `semanticSearch: false`(추가 classifier 모델 호출 차단; 7.0.0의 `jev: false`는 비활성화 키가 아님), `allowInstall: false`, `allowScriptToolSave: false`, `autoAuth: false`, `sampling: false`, `elicitation: false`, `requestTimeoutMs: 300000` 및 위 `outputGuard`를 지정한다. extraction은 `allowedServers` 미설정으로 off, MCP Tasks는 server `protocolVersion` 미설정(legacy)으로 off다. direct에서는 `--tools`에 `navigation_<도구>` 정확한 목록만 넣어 builtin·gateway·tool_search·codemode를 노출하지 않는다. codemode에서는 이 목록에 `codemode`를 더하고 아래 설정을 적용한다. 첫 모델 요청부터 이 도구만 선언되는 것을 가짜 provider로 확인했다. adapter는 TypeScript를 실행 시점에 컴파일하므로 내장 MCP보다 첫 요청이 약 0.9초 늦다(가짜 provider 실측 0.42초 → 1.35초).
- 제품이 initialize에서 준 사용 안내(`instructions`)는 Claude Code·Codex처럼 모델 문맥에 넣는다. 준비 기록(`native_inventory`의 initialize 결과, 계획 조건에 hash로 묶임)의 원문을 `# MCP Server Instructions` 절로 만들어 `--append-system-prompt`로 붙인다. 슬롯마다 실제 제품의 안내가 준비 기록과 다르면 준비 실패로 멈춘다. 안내가 없는 제품(graphify)은 붙이지 않는다.
- adapter는 연결할 때 서버가 광고한 `resources/list`·`prompts/list`를 자동으로 요청한다. 중계는 이를 `tools/list`처럼 **목록 조회(discovery)**로 기록·검증하되 80회 탐색 예산과 도구 호출 집계에서 뺀다(`client_discovery_calls`). `resources/read`·`prompts/get`은 계속 예산을 쓴다. 준비 단계에서 광고된 목록 조회가 성공하는지 확인한다. 제품은 warm host의 Seatbelt 안에서 실행하고 pi 자체는 native tools가 없는 신뢰 경계 밖 client다. 기존 Codex/Claude timeout 설정과 내부 `MCPClient` 기본 300초는 보존한다.
- 제품 원본 응답·중계 응답·실제 모델 전달 관측을 구분한다. CLI 출력만으로 모델이 모두 읽었다고 간주하지 않는다. 토큰·시간·호출·오류는 관측한 값과 원자료 경로를 남기며 미관측 값을 0으로 채우지 않는다. 손상된 JSONL은 첫 손상 전의 유효 prefix만 읽고 원본·손상 행·byte offset·불완전 사유를 보존한다. 호출 로그 완결성은 collection·relay 종료·미완료 host/제품 호출·중복 기록을 함께 확인한다. 완전히 기록한 오류 응답은 로그 손실과 구별하며 준비 probe의 transcript 완결성을 solver 로그 완결성으로 표시하지 않는다.
- pi 토큰은 assistant `message_end.usage`와 `compaction_end.result.usage`를 집계한다. pi-ai/bridge의 `input`은 cache read를 제외하므로 관측 입력은 `input + cacheRead + cacheWrite`, 총합은 여기에 `output`을 더한다. upstream total과 불일치하거나 compaction/종료/도구 로그가 불완전하면 전체 비용을 인증하지 않는다. retry/compaction event를 원자료에 남기며 pi 기본 retry·compaction 설정은 바꾸지 않는다. assistant provider/model과 노출된 upstream model field를 대조하되 bridge가 버리는 upstream modelUsage나 서버 적용 thinking까지 관측했다고 주장하지 않는다. **GPT identity는 요청 모델명 검증에 한정**된다. pi-ai의 OpenAI Responses 경로는 응답의 실제 `model`을 별도 저장하지 않으므로 요청명이 실제 serving 모델과 일치한다고 독립 인증하지 않는다.
- pi `auto_retry_start/end`로 실패 attempt 뒤 재시도가 성공하면, 실패 attempt의 보고된 사용량까지 합산하고 미보고분은 **하한 계산에만 0**으로 둔다. `token_accounting: lower_bound`, `complete: false`, retry 횟수와 실패 attempt 수를 execution/ledger/보고서 행에 표시한다. 하한이 500,000을 초과하면 무효이며, 하한이 한도 이내이고 정상 종료·완전한 수집·마지막 답변·cutoff·모델 identity·tool 완결성이 모두 맞으면 채점한다. 이는 미관측 비용이 없거나 실제 전체 비용이 500k 이하라는 인증이 아니다. 최종 답변 없는 retry 실패, usage 전체 부재, 불완전한 수집, retry로 결합되지 않은 실패/중단은 fail-closed다. grader usage에도 같은 하한 표시를 적용하되 하네스 재채점은 하지 않는다. 하한끼리의 비율은 정확한 cache ratio로 표시하지 않는다.
- 최종 답변을 고정 rubric으로 채점한다. 인용 파서는 보조 정보이며 특정 형식이나 파싱 성공으로 `supported`를 강제하지 않는다. 확장·비채점 참고 사항을 핵심 필수 항목으로 바꾸지 않는다.
- 도구·모델 식별 노출은 채점 입력 확정 전에 차단하고 원답변과 정확한 구간을 검토 기록에 남긴다. 실행 조건 무효, 실패 사유, 호출 로그 불완전도 결과에서 보존한다. 문항·비교군별 3회 측정은 반복 편차를 보여 주지만 통계적 유의성을 주장하지 않는다.

### Client 출력 한도의 고정 소스 근거

| 제품 | 고정 소스 근거 | adapter maxBytes |
|---|---|---:|
| codemap-search 0.13.0 | `codemap-v0.13.0`, `apps/codemap-search/docs/configuration.md:39–53`: 모든 도구 final delivery 기본 100,000 UTF-8 bytes; pi client와 맞추도록 명시 | 100,000 |
| Serena 2.1.0 (`5e4f5cd`) | `src/serena/resources/serena_config.template.yml:197`, `tools/tools_base.py:258–281`: 기본 150,000 Unicode 문자. 본문 최대 600,000바이트에 adapter 7의 기본 `appendStructuredContent`가 같은 result를 JSON 문자열로 덧붙인다(`tool-registrar.ts:241–253`). 두 사본 1,200,000바이트 + JSON/머리말 여유 50,000바이트. JSON escaping도 이 여유 범위 안이며 native 한도 증가는 보장하지 않음 | 1,250,000 |
| CodeGraph 1.6.2 (`6560052`) | `src/mcp/tools.ts:128,5278,8416–8420`: 일반 formatter 15,000 UTF-16 단위 + notice, explore hard ceiling 최대 25,000 + 머리말. 전체 도구 공통 byte cap은 확인되지 않음 | 1,048,576 |
| zvec-grep 0.2.2 (`b1a9148`) | `src/mcp/tools.ts:488–521`의 agent formatter 반환; `src/cli/format/context.ts:37–40`의 hit별 10줄/160자 preview. 전체 응답 byte/문자 hard cap 없음 | 1,048,576 |
| Graphify 0.9.95 (`92741d1`) | `graphify/serve.py:1120–1307,2312`: 기본 2,000-token 근사(3자/token), edge 보존·notice는 예산 초과 가능. graph_stats/get_node 및 보조 read에도 공통 byte cap 없음 | 1,048,576 |
| codebase-memory-mcp 0.12.1 (`a7158ac`) | `src/mcp/mcp.c:219–224` JSON 직렬화 및 `src/mcp/compact_out.c` formatter: 개별 개수/스캔 예산과 별개로 전체 응답 byte hard cap 없음 | 1,048,576 |

이 변경은 **client 전달 오류 수정**이며 제품 출력 예산을 늘리지 않는다. 비교 lock README와 SHA256SUMS는 부모의 새 lock 개정에서 함께 갱신한다. 현재 데이터/lock은 그대로이므로 새 데이터셋과 adapter 7.0.0 pin 전에는 운영 준비 완료를 주장하지 않는다. 85,592바이트 fake MCP 회귀 검증에서 pi까지 원문 동일·adapter 절단 0건을 확인했다. GPT의 다음 요청도 전체 전달됐지만 Claude Code 2.1.310은 별도로 persisted-output 2 KiB preview를 만들었다. 사용자 결정에 따라 이 downstream 한도(기본 50,000자 초과 시 파일 저장·2 KiB preview)는 변경하지 않는다. Claude 슬롯 정리 직전 기존 session cwd 검증을 통과한 정확한 project slug의 `<session>/tool-results/`에서 파일 내용은 읽지 않고 개수·크기만 센다. 결과·보고서의 `claude_persisted_tool_results: {count, bytes}`가 그 관측이며, 정리 실패/미관측 또는 다른 provider는 null이다. pi 이벤트만으로 모델 전체 전달을 인증하지 않는다.

### 두 측정 모드와 context 감사

- `direct`: 현행 직접 도구. `codemode`: private settings의 `defaultTools: ["+codemode"]`, `codemode: {"mode":"only"}`. pi 1.4.0의 명시적 `--tools`는 defaultTools를 덮어쓰므로 `codemode`도 명시 목록에 넣는다. 기존 private CLI flags는 유지한다.
- 두 모드 모두 같은 relay/rg admission을 사용한다. 스크립트는 `tools.navigation_read({...})`, rg는 `tools.bash({...})`를 호출한다. nested 호출은 `Promise.all`에서도 각각 80회 예산에 들어가며 81번째를 dispatch 전에 거부한다. 현재 warm-host의 product RPC는 직렬화된다. 스크립트 종료·오류·timeout으로 취소된 nested 호출은 부모 결과의 `details.calls[].status="cancelled"`와 parent/이름/인자/중복 수가 맞을 때만 종료 없는 호출을 cancelled로 정합한다. 출력을 만들거나 토큰 비용을 추정하지 않으며, 근거 없는 미완료 호출은 여전히 incomplete다. 고정 pi는 취소 ID를 `parent/?`로 남기므로 admission 전 취소는 같은 부모 안의 name+args start와 cancelled 요약이 각각 정확히 하나이고 같은 bash command(제품은 tool+arguments)의 admission이 전혀 없을 때만 인정한다. `cancelled_before_admission` 개수와 `match: parent+name+args unique` 근거를 남기며 예산은 0이다. 동일 호출 중복이나 일부만 admission되어 짝이 불명확하면 계속 fail-closed한다. 고정 pi는 nested 호출 인자를 `JSON.stringify` 기준 200자(UTF-16 단위) 미리보기로만 남기므로, 긴 인자는 같은 197자 접두어와 개수로 짝짓는다. 접두어가 같은 호출이 둘 이상이면 정합하지 않고 실패로 닫는다. 취소 뒤 warm host가 끝까지 처리하고 버린 응답은 호출 기록에 `response_discarded`로 표시하고 전달 상태를 unknown으로 둔다.
- codemode의 별도 `models.*` classifier/image API는 예산 밖 모델 호출이므로 모든 비교군에서 끈다. private `script-tools.mjs`가 pinned pi의 `createCodemodeExtension({models:false})` factory를 불러 replaceable builtin을 대체한다. 다른 codemode 기본값은 유지한다. 실제 script의 `typeof models`는 `undefined`다.
- `mode:"only"`는 도구 선언을 숨길 뿐 숨긴 active 도구의 직접 호출을 막지는 않는다. codemode 슬롯에서 `parentToolCallId` 없는 codemode 외 호출은 `direct_calls_outside_script`로 기록하고 조건 오류로 보지 않는다.
- adapter 7의 deferred `mcp__navigation__<tool>`도 도달 가능하다면 같은 relay를 거친다. 현재 cold `directTools:true`에서는 direct로 등록한 도구를 deferred로 중복 등록하지 않는다. direct에는 `tool_search`가 없고, 이 이름이나 비선언 deferred 도구를 강제로 호출해도 활성화/dispatch하지 못하는 것을 확인했다. 별도 deferred fixture에서도 80/81 경계를 확인했다.
- prepare/finalize 및 verify는 비교군×모드마다 실제 `pi_client.launch`를 loopback fake provider로 실행하고 첫 instructions/input/tools를 보존한다. native_inventory의 initialize·tool 원문을 hash-bound replay MCP로 제공하므로 제품을 재색인하거나 실제 모델·운영자 인증을 호출하지 않는다. Claude 감사만 전용 fake HOME를 쓴다.
- 모든 문항의 사용자 프롬프트, 제품 원문이 아닌 system·adapter·codemode 문구, 요청 어디에나 있는 절대 경로 구성요소를 검사한다. 금지어는 `IDENTIFYING_PATH_WORDS` + 모든 문항 ID + `smoke`, `grafana`(대소문자 무시)다. 제품 instructions/description·schema·도구 식별자 및 입증 가능한 렌더링 접두만 예외이며 절대 경로는 예외가 아니다. 감사 원자료·hash·위반 목록은 plan의 `context-audit/`, verify 재캡처는 cache `context-audits/`에 보존한다. v3 Live 문항의 Grafana 때문에 실패하는 것이 정상이며 문항을 하네스가 몰래 고치지 않는다.
- 뒤늦게 노출되는 source/cwd/TMPDIR 및 adapter·script spill 경로와 relay/wrapper 거부 문구도 준비 때 검사한다. `MCPClient` 전체 RPC·HTTP/stdio 오류, `NativeRelay`, `RelayPolicy`, `source_path`, `auxiliary_call`, shell wrapper·Seatbelt wrap의 모델 전달 오류 문구와 f-string의 고정 부분·상수·RPC error envelope를 함께 감사한다. 실행 종료 뒤에는 pi-session의 **모든 system patch와 user 메시지**를 다시 검사하고 위반/누락이면 슬롯 조건을 무효화한다(`context-audit.json`).
- 결과의 `observations`는 턴 수, 직접/중첩 호출 분포와 model-level 호출 분포, assistant `durationMs` 합, relay 또는 `bash-results.jsonl`의 실행 구간 합, adapter `outputGuard.truncated`, script 출력 절단, `include_seen=true` 요청 수(중계 거부 포함)를 기록한다. 병렬 도구 시간 합은 wall time과 더하지 않는다. SIGKILL로 wrapper 종료 기록이 없더라도 pi timeout/abort 또는 부모 cancelled 근거가 짝지어지면 `killed`로 분류하고 호출 로그 완결성은 기존 admission/이벤트 기준을 유지한다. 시간 총합은 null, 관측 부분합은 `observed_tool_seconds`, 관측·누락 호출 수는 `tool_seconds_observed_calls`/`tool_seconds_missing_calls`로 구별한다. 같은 명령이 반복되어 event를 유일하게 결합하지 못하면 killed를 추정하지 않는다. 보고서 schema 3의 `modes.direct`, `modes.codemode`에 별도 품질 집계하며 총 실행 상태만 관리용으로 함께 표시한다.

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

공통 실행 함수 `runner.execute_model`은 선택한 pi provider로 풀이를 전달하고 채점 요청은 항상 `pi_grader.execute`의 pi/Astra로 보낸다. 기존 Codex/Claude Code CLI 경로는 cleanup 후보로 보존한다. campaign 승인·ledger·scheduler·검토·보고는 무모델 대역으로 검증하며, 실제 실행은 최신 핀·제품별 OS 경계·semantic readiness·동일 live 세션/clock 통합과 새 준비 snapshot의 독립 검토 뒤에만 허용한다. 계정의 지정 모델 사용 가능 여부, 실제 모델 전달량, 채점 보정, 난도 실측은 아직 확인하지 않았다.

### pi의 private state와 인증

각 슬롯에 workspace 밖 0700 `pi-agent`, `pi-session`을 만든다. **제품 비교군의 pi cwd**와 모델이 볼 수 있는 TMPDIR·shell HOME·rg 복사본은 슬롯 경로(캠페인·문항·비교군 이름 포함) 대신 cache root 아래 opaque 16자 이름의 전용 폴더에 두고, 그 위치를 `private-root.json`에 기록한다. `--mode json -ne -ns -np -nc --no-themes -na --offline`, `PI_SKIP_VERSION_CHECK=1`, `PI_TELEMETRY=0`을 고정한다. 제품군에만 고정 pi-mcp-adapter, Claude에만 고정 bridge extension, codemode 모드에만 아래 제한 factory extension을 명시한다. 제품 pi cwd는 `<private_root>/work`, rg pi cwd는 이미 불투명한 `<cache>/<opaque>/source`이며 wrapper도 같은 곳에서 실행해 system `<cwd>`와 `pwd`가 일치한다. Claude settings 검사와 project slug 정리는 실제 pi cwd realpath를 사용한다. settings/models/mcp-adapter JSON 외 부모 설정·확장·skills·context files는 읽지 않는다. 환경은 처음부터 allowlist로 구성하고 모든 부모 PI/OPENAI/ANTHROPIC/CODEX/CLAUDE override를 버린다.

- OpenAI: solver와 grader는 같은 `pi_client.environment/operator_token/redact`를 사용한다. 신뢰하는 harness가 고정 pi의 `auth print-bearer-token --provider openai-codex --min-expiry 30m`으로 operator의 실제 agent dir에서 token을 얻는다. pi 자체의 lock/refresh만 허용하며 token은 메모리와 전용 env에만 존재한다. production private models.json은 `$CODEMAP_BENCH_PI_TOKEN` 참조만 허용하며 `baseUrl`·API·모델 override는 fail-closed한다. 로컬 검증 seam은 함수의 명시적 loopback URL 인자뿐이며 부모 env나 실행 설정으로 켤 수 없다. stdout/stderr는 저장 전 token을 가리고 종료 후 pi session/temp도 검사한다. token 획득 실패 출력은 공개하지 않는다.
- Claude: 준비와 매 launch 직전(Popen 전), 슬롯 종료 직후 raw seal 전에 user `~/.claude/settings.json`, macOS managed JSON/drop-in 및 per-user/device managed plist, workspace와 ancestor의 `.claude/settings.json`·`settings.local.json`을 검사한다. 값은 판단/로그에 쓰지 않고 key 이름과 file SHA-256(부재는 null)만 plan condition에 기록한다. bridge는 쿼리마다 model·effort·thinking을 명시하고 child를 `tools: []`, strict MCP, `disableAllHooks`로 실행하므로 그 밖의 설정은 요청을 바꾸지 못한다. 덮어쓰이지 않으면서 전송·인증 경로나 프롬프트를 바꿀 수 있는 `env`, `apiKeyHelper`, `awsAuthRefresh`, `awsCredentialExport`, `outputStyle`, `language`, `forceLoginMethod`, `forceLoginOrgUUID`만 fail-closed하며 오류에는 파일과 key 이름만 표시한다. 나머지 key는 이름과 hash만 기록한다. hash/파일 집합이 바뀌면 재준비가 필요하다. 실행 중 변경이 종료 검사에서 감지되면 해당 슬롯만 condition error로 미채점 처리하며 campaign을 취소하지 않는다. 슬롯 도중 운영자가 Claude 설정을 바꾸는 위험은 시작·종료 비교로 감지하지만, 각 Claude child launch 단위의 불변성을 보장하지 않으며 중간에 바꿨다가 원복한 설정도 검출하지 못한다. 로그인 자격 파일은 이 검사 대상이 아니다. 기존 Claude Code login을 사용하는 real HOME를 유지하고 ANTHROPIC API/base URL/token이나 CLAUDE_CONFIG_DIR를 설정하지 않는다. bridge는 pinned bundled binary, `tools: []`, `dontAsk`, strict custom-tools MCP만 사용한다. 고정 bridge의 실제 Claude Code 요청 variant는 **`claude-opus-5-5[1m]`**이며 `requested_model`로 plan condition, pi-policy, execution/runtime와 결과표 JSON에 기록한다. 이는 bridge→Claude Code 요청값이지 서버가 1M을 실제 제공했다는 관측이 아니다. SDK는 wire model을 bare ID로 변환할 수 있으며 계정의 1M 권한은 별도 hosted smoke에서 확인해야 한다. 모델 실행 후 workspace realpath의 정확한 project slug와 session cwd를 검증하고 그 project 디렉터리만 삭제한다. 다른 project나 `~/.claude.json`은 수정하지 않으며 후자는 Claude 자체가 project entry를 추가할 수 있다.

무모델 통합 검증은 unsigned dummy JWT와 로컬 Codex Responses/SSE 서버로 pi의 실제 openai-codex transport/account header·usage 경로를 실행한다. Claude도 함수 인자로 지정한 private HOME와 fake Anthropic Messages 서버에서 pinned bridge/SDK/bundled binary·custom-tools MCP·usage·project residue cleanup을 검사한다. 이 seam은 production Claude의 real HOME/login 정책을 바꾸지 않는다. 실제 hosted 인증·모델 사용 권한·서버 적용 thinking·채점 품질과 전체 Grafana 준비는 여전히 별도 검증 대상이다.

### OS 격리와 준비 시 경계 검증

- cache root는 사용자가 정한 위치를 유지한다. 그 아래 workspace 식별자는 제품/벤치 이름 없는 고정 opaque 값이다. **cache root 이름 자체는 제품 출력과 solver에게 보이므로** `prepare`는 경로에 제품·하네스 이름(`codemap`, `codegraph`, `zvec`, `graphify`, `codebase-memory`, `serena`, `bench`, `route`, `campaign`)이 있으면 시작하지 않는다. 색인과 실행은 같은 canonical 절대 경로를 사용하며 기존 이름 있는 workspace는 자동 재인증하지 않는다.
- 제품 stdio·zvec HTTP 서버·CBM shutdown과 그 자식 LSP는 모두 `sandbox-exec -f <profile>`로 실행한다. 기본 file data 읽기·쓰기·network는 거부하고 해당 설치/runtime의 read-only 접근, 해당 source/home의 읽기·쓰기, 예약 loopback 포트만 허용한다. macOS 기본 시스템 경로와 metadata/stat은 허용되므로 VM 수준의 process/Mach 격리라고 주장하지 않는다. source/home의 hard link와 외부 symlink는 시작 전에 거부한다.
- HOME/TMPDIR/XDG/npm/uv cache는 arm home 내부다. live slot은 전용 tmp를 지정한다. CBM의 짧은 runtime과 제품이 하드코딩한 `/private/tmp/cbm-*`만 예외이며 기존 항목은 지우지 않고 소유 실행 전후 새 residue를 정리·기록한다. host 공통 lease와 per-arm 직렬화가 필요하다. 준비 index 명령도 같은 경계를 쓰되 Serena 의존성 취득 단계만 network 예외를 기록한다. live 제품에는 이 예외를 허용하지 않는다.
- `product_profile(ident, state, artifact, *, slot_tmp)`는 profile 경로/hash·private env·허용 경로·포트를 반환한다. warm host/준비 probe가 이를 **client 복사본**의 `isolation_profile`에 넣어야 한다. `MCPClient`는 `product_arm`이 있는 제품의 profile 누락을 거부하며 stdio/HTTP/shutdown을 같은 profile로 감싼다. harness relay와 shim 자체는 감싸지 않는다.
- relay는 고정 release의 도구/인자 inventory를 검사한다. 신규 도구/인자는 검토 전 거부한다. codemap read의 최상위와 광고된 ranges 항목에서는 문서화된 `start_line/end_line/start/end`, camel·kebab 변형 및 문자열 숫자를 허용한다. `filePath/file-path` 등 경로 별칭의 모든 값도 같은 canonical 경로 검사와 충돌 검사를 거치며 요청은 재작성하지 않는다. 읽기 경로는 canonical tracked source와 그 ancestor로 한정하며 `.git`, `.codemap`, `.codegraph`, `.serena`, `.zvec-grep`, `.codebase-memory`, `graphify-out` 등을 읽지 못한다. metadata ignore bypass와 다른 project/root selector를 거부하며 입력이나 제품 결과를 몰래 고치지 않는다. 제품 capability는 보존하고 `resources/*`·`prompts/*`도 동일 admission/budget/log 경로를 쓴다. file URI는 source 내부만, product URI는 광고된 값만 허용하며 CBM prompt project도 준비 project에 결합한다. glob은 검색 경로 기준 필터이므로 존재하지 않는 고정 부분은 접근이 아니어서 허용하고, 실제로 존재하는 대상(외부 symlink·metadata·hard link)과 절대 경로·`..`·metadata 지목만 거부한다. 2026-10-25 스모크에서 codemap `grep`의 `{A.tsx,dir/B.tsx}` 대안 glob을 source 루트 기준 미존재 경로로 잘못 거부해 고쳤다.
- raw index의 파일 경로를 tracked/승인 synthetic/미승인으로 분류한다. Graphify의 부재 import stub만 `unresolved_reference` 개수·원래 값 목록으로 따로 기록한다. 고정 버전의 AST `dynamic_import` target이며 `confidence=EXTRACTED`, `file_type=code`, type 없음/`module`, source location·external flag가 없는 node만 대상이다. 같은 값의 일반 file/symbol node가 있으면 예외를 적용하지 않는다. source 안으로 해석되는 상대 문자열이고 `..`·절대경로·URI·backslash가 없으며 디스크에 존재하지 않을 때만 승인한다. 실제 존재하는 미추적 파일/디렉터리, 외부 경로, link의 source_file에는 이 예외가 없다. 다른 제품의 경로 감사도 완화하지 않는다. Tantivy 저장 문서, SQLite 경로 열, zvec relative/absolute 파일 목록·root manifest, Graphify nodes/links, 실행 불가능한 Serena pickle reader를 사용한다. zvec native collection은 `readOnly` open/close도 `.proxima` 바이트를 변경하므로 audit만 private tmp의 disposable APFS clone에서 수행한다. 저장된 원래 root/path 검증은 유지하며 준비된 index/baseline을 audit로 변경하지 않는다. 외부 경로·알 수 없는 index 형식은 준비 실패다. CBM의 고정 semantic-input key와 excluded-directory coverage row는 source body와 구별한다.
- 경계 probe는 하네스 소유 canary로 source·고정 rg·pi TMPDIR 허용과 hidden/session/cache/worktree/symlink/metadata 읽기, source 쓰기, network 거부를 확인한다. 실제 정답·operator 세션·다른 workspace는 존재 여부만 기록한다. pi 기본 경로에는 과거 Claude outer CLI sandbox/private-login/CONNECT proxy blocker가 적용되지 않는다. 해당 legacy 구현은 보존하며 실제 Claude login/model 전달은 별도 검증 대상이다.
