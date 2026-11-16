# 코드 탐색 벤치마크 인계: Claude 측정

GPT-6.1 Sol 측정은 끝났고 Claude 측정이 남았다. 이 문서는 Claude 측정을 시작하는 사람이 지금 상태를 파악하고, 먼저 정할 것과 고칠 것을 알고, pi 하네스로 측정을 돌릴 수 있게 쓴다. 2026-11-16 기준이다.

- GPT 결과: [`apps/codemap-docs/src/content/docs/ko/benchmarks.md`](../apps/codemap-docs/src/content/docs/ko/benchmarks.md)(영문은 [`apps/codemap-docs/src/content/docs/benchmarks.md`](../apps/codemap-docs/src/content/docs/benchmarks.md))
- 하네스 설명: [`pi/README.md`](pi/README.md), [`codex/README.md`](codex/README.md)

## 지금 상태

- **GPT-6.1 Sol**: 도구 6개(codemap-search, rg, CodeGraph, zvec-grep, Graphify, codebase-memory-mcp)를 pi direct, pi codemode, Codex code mode 세 칸에서 쟀다. 칸·도구마다 답변 9개(문항 3개 × 3회)다.
- **Claude**: 한 번도 재지 않았다.
- **Serena**: 비교 대상에서 뺐다. 색인이 1회에 34~70분 걸리기 때문이다(사용자 결정). 하네스 코드에서도 지웠다.
- **준비된 계획도 캐시도 없다.** 11/16에 폴더 구조를 바꾸면서 두 하네스의 파일이 바뀌어 그전 계획이 모두 무효가 됐고, 그 뒤 SSD의 캐시를 모두 지웠다. Claude 측정은 새 캐시를 준비하는 데서 시작한다(사용자 결정).
- **GPT 원자료도 지웠다**(사용자 결정). 개별 답변, 채점 기록, 실행 로그는 다시 볼 수 없다. 남은 것은 `results/`의 집계 2개와 공개 문서다.
- SSD에 남은 벤치마크 자료는 데이터셋 출처 자료뿐이다(`/Volumes/Samsung_990_PRO/codemap-bench-work/`, 54 MB). 하네스의 데이터셋 manifest가 이 경로를 가리킨다.
- **새 구조에서 확인한 것은 모델을 부르지 않는 검사까지다.** 오프라인 검사(pi 60개, Codex 57개)와 pi 행동 검사(절차 1)가 통과했다. 준비(`prepare`)와 실제 실행은 새 구조에서 해 보지 않았다. 처음 돌릴 때 경로 문제가 나올 수 있다.

## 폴더 구조

| 폴더 | 내용 | 실행 |
|---|---|---|
| `benchmark/pi/` | pi 하네스. GPT의 pi 두 칸을 쟀고, Claude(pi) 측정에 쓴다 | `python3 -m benchmark.pi.<모듈>` |
| `benchmark/codex/` | Codex CLI 하네스. GPT의 Codex 칸을 쟀다 | `python3 -m benchmark.codex.<모듈>` |
| `benchmark/ops/` | 운영 스크립트(준비, 실행, 승인, 집계)와 분석 스크립트 | `BENCH_CLIENT=pi` 또는 `codex`를 주고 실행 |
| `benchmark/results/` | 집계 결과(JSON) | |

- **`claude/` 폴더는 아직 없다.** Claude Code CLI로 재는 하네스가 없기 때문이다. `codex/`에 Claude Code 실행 코드가 일부 있지만 준비를 거부한다("하네스에서 고칠 것"). 만들 때 `benchmark/claude/`에 둔다.
- 두 하네스는 같은 코드에서 갈라진 변형이라 파일 이름이 대부분 같다. 한쪽을 고칠 때 다른 쪽에도 같은 수정이 필요한지 본다.
- 실행하면 생기는 폴더는 git에 넣지 않는다: 준비 증거 `<하네스>/evidence/`, 로그 `ops/logs/`.
- 명령은 모두 저장소 루트에서 실행한다.
- GPT를 잴 때의 하네스 원본은 커밋 `7f56b3960`에 있다(pi는 `benchmark-pi/`, Codex는 `benchmark/`). 지금 폴더와의 차이는 경로와 모듈 이름뿐이다.

## GPT 결과

- 사람이 읽는 결과는 위의 공개 문서다.
- `results/gpt-6.1-sol-pi.json`과 `results/gpt-6.1-sol-codex.json`은 그 수치의 집계다. `"<모델> <모드>"` 아래에 도구별로 답변 수(`answers`), 충족한 사실(`fulfilled`/`facts`), 토큰 합(`total`), 답변별 점수(`scores`)가 있다. `vs`는 codemap-search와 다른 도구의 비교, `vs_baseline`은 다른 도구와 rg의 비교다.

**읽은 규칙.** Claude 결과도 같은 규칙으로 읽어야 나란히 볼 수 있다.

- 세 칸(pi direct, pi codemode, Codex)을 따로 집계하고 합산하지 않는다.
- 지표는 답변당 충족한 사실 수(12개 중. 채점이 `correct`와 `supported`를 모두 인정한 것)와 답변당 총 토큰이다.
- 비교는 문항 층화 정확 순열 검정(양측)으로 한다. 칸별 p < 0.05일 때만 차이가 있다고 적는다. 다중 비교 보정은 하지 않는다.
- 따로 돈 배치끼리의 비교는 배치 간 비교라고 표시한다. GPT는 codemap-search·rg와 나머지 4개를 따로 쟀다.
- 부적격·중단 슬롯은 수와 원인을 적는다. 재실행했으면 결과에 표시한다.
- 범위, 지표, 읽는 규칙은 측정 전에 정한다.

## Claude 측정 전에 정할 것

사용자에게 물어 정한다.

1. **클라이언트**: pi만 할지, Claude Code CLI도 할지. CLI는 하네스 작업이 먼저 필요하다.
2. **비교군**: 6개를 한 캐시에 한 번에 준비하면 배치 간 비교 제한이 없다.
3. **모드**: pi는 direct와 codemode가 있다. GPT는 둘 다 쟀다.
4. **스모크**: Claude는 지금 pi 하네스(읽기 도구 기준군)로 돈 적이 없고, 새 폴더 구조로는 아무것도 돌지 않았다. 작은 스모크를 먼저 한다. 스모크가 끝나면 벤치마크로 넘어가기 전에 사용자 확인을 받는다.
5. **동시 실행**: 두 캠페인을 동시에 돌릴지, 차례로 돌릴지.

## 하네스에서 고칠 것

측정 구성에 따라 필요한 것만 고친다. 하네스 파일(`*.py`, `*.mjs`, `data/**`)을 하나라도 바꾸면 그 하네스로 준비한 계획이 무효가 되므로 준비 전에 고친다.

| 무엇 | 언제 필요한가 | 할 일 |
|---|---|---|
| Claude Code CLI 지원 | Claude Code CLI로도 잴 때 | `codex/`의 `prepare`가 Claude를 거부한다. Claude Code 제품 비교군에 보조 Bash가 없어서다(Codex 쪽에는 보조 셸이 있다). 같은 것을 Claude 쪽에 넣는다 |
| codebase-memory-mcp 정리 범위 | 두 캠페인을 동시에 돌릴 때 | 한 캠페인의 정리가 다른 캠페인의 `/private/tmp/cbm-*` 임시 파일을 지운다. 정리를 자기 프로세스가 만든 파일로 한정한다. 고치지 않으려면 차례로 돌린다 |

## 절차: pi로 Claude 측정

풀이는 pi 1.4.0, 제공자 `claude-bridge`, 모델 `claude-opus-5-5`, 추론 강도 medium이다. 채점은 GPT 때와 같이 `gpt-6-astra` high다.

`<캐시>`는 `/Volumes/Samsung_990_PRO` 아래에 만들 새 폴더 이름이다. 제품이나 하네스를 드러내지 않는 중립 이름으로 짓는다(예: `wsk16`). `<태그>`는 실행 이름이다(소문자, 숫자, 하이픈. 예: `claude-smoke1`). 실행마다 새 태그를 쓴다.

```bash
cd /Users/buyong/workspace/private/buyong-mcp
OPS=$PWD/benchmark/ops
PY=/opt/homebrew/bin/python3
export BENCH_CLIENT=pi
```

1. **pi 행동 검사 증거를 새로 만든다.** 모델을 부르지 않는다. 고정된 pi 도구 모음을 `<캐시>` 아래에 설치하고 검사한다. 약 5분 걸린다. 끝나면 `pi behavior evidence and coupled hashes refreshed`가 나오고 `benchmark/pi`의 세 파일(`targets.lock.json`, `SHA256SUMS`, `route_contract.py`)이 바뀐다. 이 셋을 커밋한 뒤 준비한다. 이 단계를 건너뛰면 준비가 `pi 검사 구현 변경` 또는 `행동 검증 증거가 없습니다`로 멈춘다.

   ```bash
   CODEMAP_BENCH_CACHE=/Volumes/Samsung_990_PRO/<캐시> PATH=/Users/buyong/.nvm/versions/node/v24.14.0/bin:$PATH \
     "$PY" -m benchmark.pi.pi_lock reprobe
   ```

2. **캐시를 준비한다.** 설치, 색인, 조회 점검, 계획 작성까지 한다. 모델을 부르지 않는다. 색인을 하므로 사용자가 요청한 스모크·벤치마크에서만 한다.

   ```bash
   # bench_prepare.sh <캐시> <all 또는 rg를 포함한 비교군 목록> <하네스가 고정한 평가 대상 커밋>
   BENCH_SOLVER_MODEL=all "$OPS/bench_detach.sh" bench_prepare.sh <캐시> all e727d20c6504948c823cf55398a2f68d45337f7c
   ```

   - 평가 대상은 codemap-search 0.14.2(`e727d20c6`, 태그 `trial-bench-merge-compact-output`)다.
   - 끝나면 `ops/logs/`의 출력 파일에 `prepare exit 0`과 `verify exit 0`이 남는다.
   - 준비는 한 번에 하나만 돌린다. codebase-memory-mcp 임시 파일이 겹친다.
   - Grafana 소스(`c0a598f5a`)는 준비가 GitHub에서 받는다. GPT 때는 지금은 지운 캐시에서 복제했으므로, 받는 경로는 최근에 돌려 보지 않았다.
   - GPT 때 도구 4개 + rg 준비에 pi 39분, Codex 26분이 걸렸다.
3. **Claude 토큰 파일을 확인한다.** `bench_common.sh`가 `/Users/buyong/.config/op-secrets/claude-oauth-token`을 경로로 넘긴다. 운영자 소유의 0600 파일이어야 한다. 내용은 읽거나 출력하지 않는다.
4. **실행 승인 입력을 만든다.** 출력에 계획 ID와 슬롯 수가 나온다.

   ```bash
   export BENCH_MODELS=claude-opus-5-5
   # 작은 스모크: 한 문항의 1회차만
   "$PY" -I "$OPS/bench_ops.py" inputs <캐시> <태그> all direct,codemode --slots='^git-sync-job-dispatch-.*-1$'
   # 벤치마크: 전체
   "$PY" -I "$OPS/bench_ops.py" inputs <캐시> <태그> all direct,codemode --full
   ```

5. **실행한다.** 실행, flag 검토, 재개, 채점까지 한다. 다시 실행해도 끝난 것은 건너뛴다.

   ```bash
   "$OPS/bench_detach.sh" bench_run.sh <캐시> <태그> claude-opus-5-5
   ```

   - 모델 이름을 꼭 적는다. 적지 않으면 GPT와 Claude를 차례로 돌린다.
   - `bench_detach.sh`는 세션과 분리해 띄운다. 에이전트의 백그라운드 실행으로 띄우면 세션이 끝날 때 캠페인이 같이 죽는다.
   - 끝나면 출력 파일에 `claude-opus-5-5 completed <캠페인 폴더>` 줄이 남는다.
   - `a flag needs a person`이 나오면 규칙에 없는 flag다. Grafana 소스에서 값을 확인한다. 코드 값이면 `bench_ops.py`의 `known_evidence`에 규칙을 더하고 다시 실행한다. 풀이 모델의 자기 보고이면 승인하지 않고 사용자에게 알린다. 사람 승인 없이 처리한 flag는 매번 사용자에게 알린다.
6. **ledger로 확인한다.** 로그 줄을 믿지 않는다. `<캐시>/route-preparation/campaigns/<캠페인 폴더>/ledger.json`에서 범위 안 슬롯이 `status: completed`, `grade_state: validated`, `conditions_valid: true`, `answer_eligible: true`인지 본다. `"$PY" -I "$OPS/progress.py" <캐시>`가 요약해 준다.
7. **집계한다.** `results/bench_report-<캐시>-<태그>-by-mode.json`이 생긴다. 남길 집계는 알아보기 쉬운 이름으로 바꿔 커밋한다.

   ```bash
   "$PY" -I "$OPS/bench_ops.py" report <캐시> <태그> --by-mode
   ```

8. **검사한다.**
   - 비밀 문자열: `"$PY" -I "$OPS/scan_secrets.py" <캠페인 폴더 경로> "$OPS/logs"`. 걸린 것이 `encrypted_content` 암호문 안인지 확인한다.
   - 도구 사용과 codebase-memory-mcp 정리 기록: `"$PY" -I "$OPS/tools4_slots.py" <캠페인 폴더 경로>`. 같은 방식으로 인용 형식은 `citation_style.py`, 출력 잘림은 `truncation_counts.py`로 본다.

Codex로 다시 잴 때는 `BENCH_CLIENT=codex`로 2번부터 한다(1번은 pi 전용이다). Codex 하네스는 Claude 준비를 거부하므로 `BENCH_SOLVER_MODEL=gpt-6.1-sol`로 준비한다.

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
- **캐시 안의 색인 DB를 직접 열지 않는다.** 11/16에 두 캐시의 계획이 이렇게 무효가 됐다고 추정한다. 꼭 봐야 하면 읽기 전용·불변 모드(`file:<경로>?mode=ro&immutable=1`)로 연다.
- **지금 단계가 스모크인지 벤치마크인지 매번 밝힌다.**
- **토큰·인증 파일은 읽거나 출력하지 않는다.**
- **push와 병합은 요청이 있을 때만 한다.**
- 캐시는 `/Volumes/Samsung_990_PRO/<중립 이름>`에 만든다.

## 함정

1. **`BENCH_CLIENT`를 주지 않으면 스크립트가 멈춘다.** `pi`와 `codex`는 계획도 증거 폴더도 따로다.
2. **`bench_run.sh`가 `unfinished`로 끝나도 답변은 채점돼 있을 수 있다.** 범위 안 슬롯이 하나라도 채점되지 않으면 캠페인이 `completed_with_failures`가 된다. ledger의 `answer_eligibility.reasons`와 슬롯 폴더의 `execution.json`에서 원인을 본다.
3. **캠페인 폴더 이름의 `smoke-`는 측정의 지위가 아니다.** 일부 슬롯만 도는 범위에 하네스가 붙이는 이름이다.
4. **zvec-grep 서버가 죽을 수 있다.** GPT 때 `globs`에 제외 패턴만 넣은 검색 두 번에 SIGSEGV로 죽었다. 그 슬롯은 `environment_error`가 되고 답변이 없다. 원인은 모른다.
5. **flag는 매번 나온다.** 답변에 "10초", "30초" 같은 시간 값이 있으면 채점 전에 검토를 요구한다. 스크립트는 `known_evidence`의 네 규칙만 승인한다. Claude는 표현이 달라 규칙에 안 맞을 수 있다.
6. **시스템 시계가 실행 중에 조정될 수 있다.** GPT 때 최대 2.2초 조정돼 Codex 하네스의 답변 시각 검사에 걸렸고 고쳤다. pi 하네스의 같은 코드는 Codex 실행기일 때만 돈다.
7. **외장 SSD가 잠깐 분리될 수 있다.** 11/16에 72초 동안 분리됐다. 캠페인 중이면 그때 돌던 슬롯이 실패한다.
8. **Codex CLI가 자동 갱신되면 Codex 계획이 무효가 된다.** 호스트 node는 `v24.14.0`이어야 한다.
9. macOS에는 `timeout` 명령이 없다.

## 확인하지 않은 것

- 새 폴더 구조에서의 준비와 실행. 오프라인 검사와 pi 행동 검사만 통과했다.
- 준비가 Grafana 소스를 GitHub에서 받는 경로.
- Claude를 지금 pi 하네스로 실제로 돌린 적이 없다. flag 규칙과 인용 형식이 Claude 답변에 그대로 통하는지 모른다.
- Claude 토큰이 유효한지, 계정에서 `claude-opus-5-5`와 `gpt-6-astra`를 쓸 수 있는지.
- zvec-grep 서버가 죽은 원인.
- 끊긴 캠페인을 `retry_unstarted`로 잇는 경로.
