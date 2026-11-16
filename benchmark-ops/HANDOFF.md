# 코드 탐색 벤치마크 인계: Claude 측정

GPT-6.1 Sol 측정은 끝났고 Claude 측정이 남았다. 이 문서는 Claude 측정을 시작하는 사람이 지금 상태를 파악하고, 먼저 정할 것과 고칠 것을 알고, pi 하네스로 측정을 돌릴 수 있게 쓴다. 2026-11-16 기준이다.

- GPT 결과: [`apps/codemap-search/docs/benchmarks.ko.md`](../apps/codemap-search/docs/benchmarks.ko.md)(영문은 `benchmarks.md`)
- 하네스 설명: [`benchmark/README.md`](../benchmark/README.md), [`benchmark-pi/README.md`](../benchmark-pi/README.md)

## 지금 상태

- **GPT-6.1 Sol**: 도구 6개(codemap-search, rg, CodeGraph, zvec-grep, Graphify, codebase-memory-mcp)를 pi direct, pi codemode, Codex code mode 세 칸에서 쟀다. 칸·도구마다 답변 9개(문항 3개 × 3회)다.
- **Claude**: 한 번도 재지 않았다. pi 하네스에 Claude용 계획이 준비돼 있지만 지금 쓸 수 있는 것은 `wsk12`(codemap-search, rg)뿐이다.
- **Serena**: 비교 대상에서 뺐다. 색인이 1회에 34~70분 걸리기 때문이다(사용자 결정). 하네스 코드에서도 지웠다.
- 지금 돌고 있는 측정은 없고, 하네스용 worktree도 없다.

## 저장소에 있는 것

| 폴더 | 내용 |
|---|---|
| `benchmark/` | Codex CLI 하네스. GPT의 Codex 칸을 쟀다 |
| `benchmark-pi/` | pi 하네스. GPT의 pi 두 칸을 쟀고, Claude(pi) 측정에 쓴다. 커밋 `cb89d6886`의 `benchmark/`와 바이트까지 같다 |
| `benchmark-ops/` | 운영 스크립트(준비, 실행, 승인, 집계), 분석 스크립트, 이 문서 |

- 두 하네스는 같은 Python 패키지(`benchmark`)의 서로 다른 변형이다. 합치면 25개 파일이 충돌해서 합치지 않고 나란히 넣었다.
- **`benchmark-pi/`는 그 자리에서 실행되지 않는다.** 하네스가 패키지 이름 `benchmark`와 "상위 폴더가 저장소 루트"라는 가정(`settings.REPOSITORY`)에 묶여 있다. 실행하려면 `benchmark/`가 pi 하네스인 커밋으로 worktree를 만든다(절차 2).
- pi 하네스 브랜치(`chore/bench-pi-read-tools`, `chore/bench-smoke-rg-codemap`)는 병합해서 이력이 `main`에 들어 있다.

## SSD에 있는 것

모두 `/Volumes/Samsung_990_PRO` 아래다. 이 측정에 쓰는 코드는 모두 저장소에 있고, SSD에는 데이터만 둔다.

### 캐시와 계획

캐시는 색인, 설치한 도구, 준비된 계획(`route-preparation/plans/<ID>`), 캠페인 결과(`route-preparation/campaigns/<폴더>`)를 담는다. **계획은 준비한 하네스 커밋에서만 통한다.**

| 캐시 | 하네스 커밋 | 비교군 | GPT 계획 | Claude 계획 | GPT 캠페인 | 계획 검증 |
|---|---|---|---|---|---|---|
| `wsk12` | pi `ee1441b04` | codemap-search, rg | `c0aebd395e8021313e51635d5dfbf5ea` | `a4119a667964b056713fb2e61af99839` (36슬롯) | `882f05d31f8643919d9e709b8dc3ce86` | 통과 |
| `wsk13` | Codex `e39ae6c8f` | codemap-search, rg | `8b9f61b9c728e172723c539e1af0e70d` | 없음 | `3ff959feab08469791e8d0a85221902d` | 통과 |
| `wsk14` | pi `cb89d6886` | CodeGraph, zvec-grep, Graphify, codebase-memory-mcp, rg | `468f0e605e38bfd9c42a51704e471b3d` | `e0c4f2149a613135d61a9a0227282626` (90슬롯) | `smoke-eae6f5a598494135800bb99039e8091a` | **실패** |
| `wsk15` | Codex `cb2f1914a` | 같은 5개 | `7052e4fcf48839e2687855fd81cb2535` | 없음 | `smoke-b0100651e76c4605aa2cc83f16115608`, `smoke-b661102ee2f4413dba610b4c561cfff1` | **실패** |

- **`wsk14`·`wsk15`의 계획은 지금 쓸 수 없다.** CodeGraph 색인 폴더(`.codegraph`)가 11/16에 바뀌어 검증이 `native 초기 상태 변경`으로 멈춘다. GPT 결과에는 영향이 없다(측정은 11/15에 끝났다). 대처는 "하네스에서 고칠 것"에 있다.
- **캐시를 지우지 않는다.** `wsk12`~`wsk15`의 캠페인 폴더는 GPT 결과의 원본이고, Claude 결과와 나란히 집계할 때 읽는다. pi 하네스의 행동 검사 증거도 `wsk12`와 `wsk14`를 가리킨다.
- **보호 캐시 `wsv4`, `wst6b`, `wst6a1`에는 쓰지 않는다.** `wst6a1`은 읽기만 한다(새 캐시의 Grafana 소스를 여기서 복제한다).
- 캠페인 폴더 이름의 `smoke-`는 측정의 지위가 아니다. rg를 빼고 도는 범위에 하네스가 붙이는 이름이다.

### 작업 폴더 `codemap-bench-work/`

| 항목 | 내용 |
|---|---|
| `bench-out/` | 운영 스크립트가 쓰는 로그(`logs/`)와 집계 파일(`bench_report-*.json`). GPT 집계 4개가 들어 있다. 하네스가 이 작업 폴더를 풀이 모델에게 숨기므로(`isolation.hidden_references`) 로그를 여기에 둔다 |
| `decisions.md` | 사전 등록과 판단의 기록. 덧붙이기만 한다 |
| 그 밖 | 데이터셋 출처 자료(`v4/`, `v5/`, `v6/`, `dataset-v*/`)와 이전 작업의 임시 폴더. 이 측정에는 쓰지 않는다 |

## GPT 결과를 읽은 규칙

Claude 결과도 같은 규칙으로 읽어야 나란히 볼 수 있다.

- 세 칸(pi direct, pi codemode, Codex)을 따로 집계하고 합산하지 않는다.
- 지표는 답변당 충족한 사실 수(12개 중. 채점이 `correct`와 `supported`를 모두 인정한 것)와 답변당 총 토큰이다.
- 비교는 문항 층화 정확 순열 검정(양측)으로 한다. 칸별 p < 0.05일 때만 차이가 있다고 적는다. 다중 비교 보정은 하지 않는다.
- 따로 돈 배치끼리의 비교는 배치 간 비교라고 표시한다. GPT는 codemap-search·rg와 나머지 4개를 따로 쟀다.
- 부적격·중단 슬롯은 수와 원인을 적는다. 재실행했으면 결과에 표시한다.
- 측정 전에 범위, 지표, 읽는 규칙을 `decisions.md`에 먼저 적는다.

## Claude 측정 전에 정할 것

사용자에게 물어 정한다.

1. **클라이언트**: pi만 할지, Claude Code CLI도 할지. CLI는 하네스 작업이 먼저 필요하다.
2. **배치**: 6개 비교군을 새 캐시에 한 번에 준비할지(권장), 지금 캐시를 쓸지. 한 번에 돌리면 배치 간 비교 제한이 없고 `wsk14`를 고칠 필요도 없다. 준비는 색인을 하므로 사용자가 요청한 스모크·벤치마크에서만 한다.
3. **모드**: pi는 direct와 codemode가 있다. GPT는 둘 다 쟀다.
4. **스모크**: Claude는 지금 pi 하네스(읽기 도구 기준군)로 돈 적이 없으므로 작은 스모크를 먼저 한다. 스모크가 끝나면 벤치마크로 넘어가기 전에 사용자 확인을 받는다.
5. **동시 실행**: 두 캠페인을 동시에 돌릴지, 차례로 돌릴지.

## 하네스에서 고칠 것

측정 구성에 따라 필요한 것만 고친다. 하네스 파일(`*.py`, `*.mjs`, `data/**`)을 하나라도 바꾸면 그 하네스로 준비한 계획이 모두 무효가 되므로 다시 준비한다.

| 무엇 | 언제 필요한가 | 할 일 |
|---|---|---|
| Claude Code CLI 지원 | Claude Code CLI로도 잴 때 | `benchmark/`의 `prepare`가 Claude를 거부한다. Claude Code 제품 비교군에 보조 Bash가 없어서다(Codex 쪽에는 보조 셸이 있다). 같은 것을 Claude 쪽에 넣는다 |
| 두 하네스 통합 | 저장소 한 곳에서 pi와 CLI를 함께 돌리려 할 때 | `benchmark-pi/`를 `benchmark/`에 합치거나, 패키지 이름과 저장소 루트 가정을 풀어 그 자리에서 돌게 한다. 고치지 않아도 `cb89d6886` worktree에서는 돈다 |
| `wsk14`·`wsk15`의 CodeGraph 상태 | 이 캐시를 다시 쓸 때 | 같은 자리 재준비만으로는 안 된다(`freeze_baseline`이 기존 `baseline.json`을 그대로 쓴다). `<캐시>/route-preparation/baselines/codegraph/`를 지우고 같은 자리에서 다시 준비한다. 시험하지 않았다. 지우기 전에 사용자 확인을 받는다 |
| codebase-memory-mcp 정리 범위 | 두 캠페인을 동시에 돌릴 때 | 한 캠페인의 정리가 다른 캠페인의 `/private/tmp/cbm-*` 임시 파일을 지운다. 정리를 자기 프로세스가 만든 파일로 한정한다. 고치지 않으려면 차례로 돌린다 |

고치지 않고 지금 돌릴 수 있는 것은 **`wsk12`에서 Claude(pi)로 codemap-search와 rg를 재는 것**(36슬롯)이다. 아래 절차가 이 경우다.

## 절차: pi로 Claude 측정

풀이는 pi 1.4.0, 제공자 `claude-bridge`, 모델 `claude-opus-5-5`, 추론 강도 medium이다. 채점은 GPT 때와 같이 `gpt-6-astra` high다. pi와 부속은 캐시 안에 고정 설치돼 있다.

`<태그>`는 실행 이름이다(소문자, 숫자, 하이픈. 예: `claude-smoke1`). 실행마다 새 태그를 쓴다.

```bash
OPS=/Users/buyong/workspace/private/buyong-mcp/benchmark-ops
W=/Volumes/Samsung_990_PRO/worktrees/buyong-mcp
PY=/opt/homebrew/bin/python3
```

1. **사전 등록**: `decisions.md`에 범위, 지표, 읽는 규칙을 적는다.
2. **pi 하네스 worktree를 만들고 계획을 검증한다.** 모델을 부르지 않는다. 3~11분 걸린다.

   ```bash
   # restore_worktree.sh <worktree 이름> <커밋> <캐시> <GPT 계획> [<Claude 계획>]
   /bin/bash "$OPS/restore_worktree.sh" bench-pi-read-tools ee1441b04 wsk12 \
     c0aebd395e8021313e51635d5dfbf5ea a4119a667964b056713fb2e61af99839
   export BENCH_HARNESS="$W/bench-pi-read-tools"
   . "$OPS/bench_common.sh" wsk12
   cd "$BENCH_HARNESS" && "$BENCH_PY" -m benchmark.route_prepare verify --solver-model all
   ```

   통과하면 `prepared_not_executed`와 계획 ID 두 개가 나온다. 실패하면 문구가 원인을 말한다("준비 이후 하네스 변경", "pi toolchain 변경", "native 초기 상태 변경" 등). 그때는 다시 준비한다.
3. **Claude 토큰 파일을 확인한다.** `bench_common.sh`가 `/Users/buyong/.config/op-secrets/claude-oauth-token`을 경로로 넘긴다. 운영자 소유의 0600 파일이어야 한다. 내용은 읽거나 출력하지 않는다.
4. **실행 승인 입력을 만든다.** 출력에 계획 ID와 슬롯 수가 나온다.

   ```bash
   export BENCH_MODELS=claude-opus-5-5
   # 작은 스모크: 한 문항의 1회차만 (4슬롯)
   "$PY" -I "$OPS/bench_ops.py" inputs wsk12 <태그> codemap-search,rg direct,codemode --slots='^git-sync-job-dispatch-.*-1$'
   # 벤치마크: 전체 36슬롯
   "$PY" -I "$OPS/bench_ops.py" inputs wsk12 <태그> codemap-search,rg direct,codemode --full
   ```

5. **실행한다.** 실행, flag 검토, 재개, 채점까지 한다. 다시 실행해도 끝난 것은 건너뛴다.

   ```bash
   "$OPS/bench_detach.sh" bench_run.sh wsk12 <태그> claude-opus-5-5
   ```

   - 모델 이름을 꼭 적는다. 적지 않으면 GPT와 Claude를 차례로 돌린다.
   - `bench_detach.sh`는 세션과 분리해 띄운다. 에이전트의 백그라운드 실행으로 띄우면 세션이 끝날 때 캠페인이 같이 죽는다.
   - 끝나면 출력 파일에 `claude-opus-5-5 completed <캠페인 폴더>` 줄이 남는다.
   - `a flag needs a person`이 나오면 규칙에 없는 flag다. Grafana 소스에서 값을 확인한다. 코드 값이면 `bench_ops.py`의 `known_evidence`에 규칙을 더하고 다시 실행한다. 풀이 모델의 자기 보고이면 승인하지 않고 사용자에게 알린다. 사람 승인 없이 처리한 flag는 매번 사용자에게 알린다.
6. **ledger로 확인한다.** 로그 줄을 믿지 않는다. `<캠페인 폴더>/ledger.json`에서 범위 안 슬롯이 `status: completed`, `grade_state: validated`, `conditions_valid: true`, `answer_eligible: true`인지 본다. `"$PY" -I "$OPS/progress.py" wsk12`가 요약해 준다.
7. **집계한다.** GPT 캠페인 폴더를 함께 주면 나란히 나온다. 집계 파일은 `bench-out/`에 생긴다.

   ```bash
   "$PY" -I "$OPS/bench_ops.py" report wsk12 --campaigns --by-mode --label=<이름> \
     <Claude 캠페인 폴더> 882f05d31f8643919d9e709b8dc3ce86
   ```

8. **검사한다.**
   - 비밀 문자열: `"$PY" -I "$OPS/scan_secrets.py" <캠페인 폴더 경로> "$BENCH_LOGS"`. 걸린 것이 `encrypted_content` 암호문 안인지 확인한다.
   - 도구 사용과 codebase-memory-mcp 정리 기록: `"$PY" -I "$OPS/tools4_slots.py" <캠페인 폴더 경로>`. 같은 방식으로 인용 형식은 `citation_style.py`, 출력 잘림은 `truncation_counts.py`로 본다.
9. **결과를 `decisions.md`에 덧붙이고** worktree를 지울지 사용자에게 묻는다.

### 새 캐시를 준비할 때

사용자가 요청한 스모크·벤치마크에서만 한다. 준비된 계획이 없으므로 `restore_worktree.sh` 대신 worktree를 직접 만든다. 이 순서로 새 worktree에서 준비해 본 적은 없다.

```bash
REPO=/Users/buyong/workspace/private/buyong-mcp
git -C "$REPO" worktree add --detach "$W/bench-pi-read-tools" cb89d6886   # Serena가 없는 pi 하네스
ln -s "$REPO/node_modules" "$W/bench-pi-read-tools/node_modules"
export BENCH_HARNESS="$W/bench-pi-read-tools"
# bench_prepare.sh <새 캐시 이름> <all 또는 rg를 포함한 비교군 목록> <하네스가 고정한 평가 대상 커밋>
BENCH_SOLVER_MODEL=all "$OPS/bench_detach.sh" bench_prepare.sh <새 캐시 이름> all e727d20c6504948c823cf55398a2f68d45337f7c
```

- 캐시 이름은 제품이나 하네스를 드러내지 않는 중립 이름으로 짓는다(예: `wsk16`).
- 평가 대상은 codemap-search 0.14.2(`e727d20c6`, 태그 `trial-bench-merge-compact-output`)다.
- 준비는 한 번에 하나만 돌린다. codebase-memory-mcp 임시 파일이 겹친다.
- GPT 때 도구 4개 + rg 준비에 pi 39분, Codex 26분이 걸렸다.
- `prepare`는 CUI 검사를 돌리므로 worktree에 `node_modules/`가 필요하다. `pnpm install --frozen-lockfile`은 하네스 커밋에서 실패하므로 위처럼 `main` checkout의 `node_modules/`에 링크를 건다.
- 끝나면 출력 파일에 `prepare exit 0`과 `verify exit 0`이 남는다.

## 걸리는 시간 (GPT 실측)

| 일 | 시간 |
|---|---|
| pi 36슬롯(codemap-search·rg, 두 모드) 풀이 + 채점 | 약 2시간 |
| pi 72슬롯(다른 도구 4개, 두 모드) 풀이 + 채점 | 약 2시간 40분 |
| Codex 18슬롯 / 36슬롯 풀이 + 채점 | 약 1시간 5분 / 1시간 15분 |
| 작은 스모크(도구 4개, 한 문항 1회차) | pi 8슬롯 25분, Codex 4슬롯 10분 |

Claude로는 재지 않았다.

## 지켜야 할 것

- **준비한 뒤 하네스를 바꾸지 않는다.** 한 줄만 바꿔도 계획이 무효가 된다.
- **캐시 안의 색인 DB를 직접 열지 않는다.** `wsk14`·`wsk15`의 계획이 이렇게 무효가 됐다고 추정한다. 꼭 봐야 하면 읽기 전용·불변 모드(`file:<경로>?mode=ro&immutable=1`)로 연다.
- **지금 단계가 스모크인지 벤치마크인지 매번 밝힌다.**
- **토큰·인증 파일은 읽거나 출력하지 않는다.**
- **push와 병합은 요청이 있을 때만 한다.**
- 캐시는 `/Volumes/Samsung_990_PRO/<중립 이름>`에, 하네스 worktree는 `/Volumes/Samsung_990_PRO/worktrees/buyong-mcp/<이름>`에 만든다. 하네스가 이 worktree 폴더를 풀이 모델에게 숨긴다.

## 함정

1. **`BENCH_HARNESS`를 지정하지 않으면 스크립트가 든 checkout이 하네스가 된다.** 그 checkout의 `benchmark/`는 Codex CLI 하네스다. pi로 돌릴 때는 꼭 지정한다.
2. **`bench_run.sh`가 `unfinished`로 끝나도 답변은 채점돼 있을 수 있다.** 범위 안 슬롯이 하나라도 채점되지 않으면 캠페인이 `completed_with_failures`가 된다. ledger의 `answer_eligibility.reasons`와 슬롯 폴더의 `execution.json`에서 원인을 본다.
3. **zvec-grep 서버가 죽을 수 있다.** GPT 때 `globs`에 제외 패턴만 넣은 검색 두 번에 SIGSEGV로 죽었다. 그 슬롯은 `environment_error`가 되고 답변이 없다. 원인은 모른다.
4. **flag는 매번 나온다.** 답변에 "10초", "30초" 같은 시간 값이 있으면 채점 전에 검토를 요구한다. 스크립트는 `known_evidence`의 네 규칙만 승인한다. Claude는 표현이 달라 규칙에 안 맞을 수 있다.
5. **시스템 시계가 실행 중에 조정될 수 있다.** GPT 때 최대 2.2초 조정돼 Codex 하네스의 답변 시각 검사에 걸렸고 `cb2f1914a`에서 고쳤다. pi 하네스의 같은 코드는 Codex 실행기일 때만 돈다.
6. **외장 SSD가 잠깐 분리될 수 있다.** 11/16에 72초 동안 분리됐다. 캠페인 중이면 그때 돌던 슬롯이 실패한다.
7. **Codex CLI가 자동 갱신되면 Codex 계획만 무효가 된다.** 계획은 0.164.0으로 준비했다. 호스트 node는 `v24.14.0`이어야 한다.
8. macOS에는 `timeout` 명령이 없다.

## 확인하지 않은 것

- Claude를 지금 pi 하네스로 실제로 돌린 적이 없다. flag 규칙과 인용 형식이 Claude 답변에 그대로 통하는지 모른다.
- Claude 토큰이 유효한지, 계정에서 `claude-opus-5-5`와 `gpt-6-astra`를 쓸 수 있는지.
- `wsk14`·`wsk15`의 CodeGraph baseline을 지우고 다시 준비하는 대처가 통하는지.
- pi 캐시의 같은 자리 재준비 시간. Codex 캐시 `wsk15`는 7~9분이었다.
- zvec-grep 서버가 죽은 원인.
- 끊긴 캠페인을 `retry_unstarted`로 잇는 경로.
