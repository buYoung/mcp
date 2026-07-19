# 검색 품질 벤치마크 상세 실행 안내서

이 문서는 현재 저장된 하네스로 B1과 B2를 같은 조건에서 측정하고, 토큰·도구 호출·답의 정확성·검색 과정을 집계하는 순서를 설명한다. 모든 명령은 별도 표시가 없으면 제품 저장소 루트에서 실행한다.

```bash
cd /absolute/path/to/buyong-mcp
```

## 1. 현재 준비 상태

깨끗한 저장소 복제본만으로 가능한 작업과 불가능한 작업을 먼저 구분한다.

| 작업 | 현재 저장소만으로 가능 | 이유 |
|---|---:|---|
| 질문·정답·Directus 소스 확인 | 가능 | 모두 커밋됨 |
| 채점 코드 자체검사 | 가능 | 합성 검사 자료 포함 |
| 집계 코드 자체검사 | 가능 | 합성 검사 자료 포함 |
| B1/B2 설정 내용 확인 | 가능 | 설정 틀 포함 |
| 84개 외부 모델 세션 실행 | 불가능 | OpenCode 실행 파일, 인증, 고정 색인, B2 실행 파일 없음 |
| B2 실행 파일 재현 | 불완전 | 검토 패치는 있으나 원본과 같은 B2 소스 준비 절차·빌드 환경 자료가 완전하지 않음 |
| Directus 고정 색인 재현 | 불가능 | 색인 파일과 그 색인을 만드는 고정 명령·환경 기록이 없음 |

정식 실행은 마지막 세 항목을 해결하고 `preflight.py`가 모두 통과한 뒤에만 시작한다. 해시만 임의로 바꾸거나 현재 제품 실행 파일을 B2로 대신하면 기준선이 달라진다.

## 2. 고정 측정 조건

### 실행 수

```text
14개 과제 × 3회 반복 × 2개 조건 = 84세션
14개 과제 × 3회 반복 = 42개 B1/B2 묶음
84세션 × 3명 채점 = 252개 판정
```

`run_queue.py`는 서로 다른 묶음을 최대 3개까지 동시에 실행한다. 한 묶음 안의 B1과 B2는 동시에 실행하지 않고 정해진 순서대로 실행한다. 각 반복에서 B1이 먼저인 과제와 B2가 먼저인 과제가 7개씩 되도록 순서를 바꾼다.

### 조건 차이

| 항목 | B1 | B2 |
|---|---|---|
| 모델 | `ollama-cloud/deepseek-v4-flash` | 동일 |
| 질문 | 같은 과제·같은 반복의 질문 | 동일 |
| 소스 | 고정 Directus 스냅샷 | 동일 |
| OpenCode 기본 도구 | 사용 | 동일 |
| MCP | 없음 | `codemap-search` 하나 |
| MCP 사용 강제 | 없음 | 없음 |
| 셸·웹·하위 에이전트 | 금지 | 동일 |

### 세션 제한

| 제한 | 값 | 제한에 도달했을 때 |
|---|---:|---|
| 전체 실행 | 600초 | 유효한 관찰 종료로 기록 가능 |
| 완료 모델 단계 | 30회 | 유효한 관찰 종료로 기록 가능 |
| 보존 출력 | 2,097,152바이트 | 유효한 관찰 종료로 기록 가능 |

시간 초과나 단계·출력 제한을 제공자 장애로 바꾸어 재시도하지 않는다.

## 3. 실행 환경

현재 하네스는 다음 macOS 기능을 직접 사용한다.

- `sandbox-exec`
- APFS 복제 복사인 `/bin/cp -cRp`
- `/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`의 기본 명령
- Python 3
- B2를 재현할 때 필요한 Git, Rust, Cargo

`generation.py seal`은 질문, 하네스, 소스, 색인, 실행 파일을 읽기 전용으로 잠근다. 개발 중인 기본 작업 트리 대신 정식 측정 전용 작업 트리에서 실행하는 것이 안전하다. 봉인 뒤 파일을 다시 쓰기 가능하게 바꾸면 그 실행 세대는 이어서 사용할 수 없다.

## 4. 고정 입력 확인

### 4.1 Directus 소스

고정 정보는 다음과 같다.

```text
commit: 9f2f73aee7d8647d3f187dac43f724fe617763f5
tree:   0beb7bd5187e9131aba4a582effb3630d378eb4c
path:   benchmark/corpus/directus
```

질문과 정답은 다음 경로에 각각 14개가 있어야 한다.

```text
benchmark/benchmark/questions/development/*.json
benchmark/benchmark/answers/development/*.json
```

공개 목록과 비공개 목록은 다음 파일이다.

```text
benchmark/benchmark/manifests/public.json
benchmark/benchmark/manifests/private.json
```

정답 폴더와 비공개 목록은 OpenCode가 실행되는 격리 영역에서 보이지 않아야 한다.

### 4.2 OpenCode

하네스가 요구하는 위치와 고정값은 다음과 같다.

```text
path:    benchmark/runtime/opencode
version: 1.17.18
sha256:  652a34cab759c0fa348f107aa737df86355a49b1576834864e89ee43c059b25d
```

실행 권한을 부여한 뒤 실제 해시가 다르면 진행하지 않는다.

```bash
chmod u+x benchmark/runtime/opencode
shasum -a 256 benchmark/runtime/opencode
```

### 4.3 Ollama Cloud 인증

하네스는 기본적으로 다음 파일에서 `ollama-cloud` 인증을 읽는다.

```text
${XDG_DATA_HOME:-$HOME/.local/share}/opencode/auth.json
```

인증값을 출력하지 않고 항목 존재 여부만 확인한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/auth_runtime.py \
  validate "${XDG_DATA_HOME:-$HOME/.local/share}/opencode/auth.json" ollama-cloud
```

각 세션은 필요한 인증 항목만 `harness/runtime-auth/`에 임시 복사하고 종료 시 제거한다. 인증 파일을 저장소에 복사하거나 커밋하지 않는다.

### 4.4 Directus 고정 색인

하네스가 요구하는 위치는 다음과 같다.

```text
benchmark/corpus/directus-index-golden/.codemap/
```

기대 트리 해시는 `8678205ff1b19a03da85c02812aefca993b88a7688bb3db48fdf1c9b746c0a96`이다. 현재 저장소에는 이 색인이나 같은 해시를 만드는 완전한 생성 절차가 없다. 기존에 검증된 색인을 복원하지 못하면 정식 실행을 중단한다. 현재 제품으로 새로 색인한 뒤 기대 해시를 바꾸는 것은 B1/B2 기준선 재현이 아니라 새 기준선 설계 작업이다.

### 4.5 B2 소스와 두 번의 빌드

필요한 경로는 다음과 같다.

```text
benchmark/b2/source/
benchmark/b2/build-evidence/target-build1/release/codemap-search
benchmark/b2/build-evidence/target-build1/.rustc_info.json
benchmark/b2/target/release/codemap-search
benchmark/b2/target/.rustc_info.json
```

B2 검사는 다음을 모두 요구한다.

- 기준 제품 커밋 `c160dee10f400950eb141e09e284d4d930f44ce6`
- Git 트리 `522317026e29186a704c370cbcee161f20a3e3e8`
- 검토된 읽기 전용 시작 패치만 적용
- 두 빌드의 실행 파일이 SHA-256과 바이트 단위로 동일
- 두 빌드와 현재 `rustc -Vv`가 동일
- `Cargo.lock`이 기준 Git 객체와 동일
- 후보 코드나 후보 환경 변수가 없음

검토 패치는 `benchmark/harness/provenance/b2-clean-runtime.patch`에 있다. 하지만 현재 저장소만으로는 원래 B2 전체 소스의 파일 시각을 포함한 고정 tar 해시까지 재현하는 절차가 완전하지 않다. 추측으로 소스를 만들거나 현재 제품 코드를 복사하지 않는다.

## 5. 저장소만으로 가능한 자체검사

외부 모델, 빌드, 색인을 사용하지 않는 검사다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/scoring_selftest.py
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/analysis_inputs_selftest.py
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/analysis_tools_selftest.py
```

각 명령은 JSON 보고서를 `benchmark/harness/reports/`에 쓴다. `passed`가 `true`인지 확인한다.

```text
scoring-selftest.json
analysis-inputs-selftest.json
analysis-tools-selftest.json
```

`selftest.py`는 실행 하네스 전체 검사다. B2 소스·두 빌드·실행 파일·증명 기록이 모두 준비된 뒤 실행한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/selftest.py
```

## 6. B2 증명과 사전점검 준비

이 절은 4장의 모든 외부 자료가 준비된 경우에만 진행한다.

### 6.1 B2 실행 파일 계약 임시 설정

`benchmark/harness/config/b2-runtime.json`에 두 번째 빌드 실행 파일의 절대 경로와 SHA-256을 기록하고 상태를 `binary-supplied-awaiting-probe`로 둔다. 이 파일은 최초 측정 경로를 담은 기록이므로 전용 작업 트리에서만 갱신한다.

### 6.2 모델 호출 없는 B2 확인

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/probe_b2.py
```

이 검사는 외부 모델을 호출하지 않는다. 정확히 여섯 개 도구가 노출되는지, 검색·읽기·개요가 동작하는지, 색인이 바뀌지 않는지 확인하고 다음 보고서를 만든다.

```text
benchmark/harness/reports/b2-mcp-probe.json
```

### 6.3 B2 재현 증명

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/attest_b2.py
```

이 명령은 두 빌드를 새로 만들지 않는다. 이미 준비한 두 빌드의 실행 파일과 Rust 기록을 비교하고, B2 소스·패치·Cargo.lock·확인 보고서를 검증한다. 성공하면 `b2-runtime.json`과 `b2-clean-runtime-attestation.json`을 현재 절대 경로에 맞게 갱신한다.

### 6.4 APFS 복제 확인

작은 합성 자료로 APFS 복제 기능을 확인한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/test_clonefile.py
```

실제 Directus 소스와 색인을 한 번 복제해 전체 경로도 확인한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/clone_source.py \
  benchmark/corpus/directus \
  benchmark/harness/synthetic/full-clone-materialization \
  benchmark/harness/reports/full-clone-materialization.json
```

보고서를 보존한 뒤 합성 복사본만 제거한다. 복사본은 읽기 전용이므로 소유자 쓰기 권한을 먼저 되돌려야 한다.

```bash
chmod -R u+w benchmark/harness/synthetic/full-clone-materialization
rm -rf benchmark/harness/synthetic/full-clone-materialization
```

## 7. 실행 세대 만들기

실행 세대는 질문, 답, 소스, 색인, OpenCode, B2 실행 파일, 하네스 코드, 제한을 하나의 해시 묶음으로 고정한다.

먼저 다음 네 자체검사 보고서가 모두 있어야 한다.

```text
benchmark/harness/reports/offline-selftest.json
benchmark/harness/reports/scoring-selftest.json
benchmark/harness/reports/analysis-inputs-selftest.json
benchmark/harness/reports/analysis-tools-selftest.json
```

모든 외부 호출 전 사전점검을 실행한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/preflight.py
```

실패가 하나라도 있으면 봉인하거나 모델을 호출하지 않는다. 모두 통과하면 실행 세대를 만든다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/generation.py \
  seal benchmark/harness/reports/baseline-3x-generation.json
```

봉인 확인:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/generation.py \
  verify-execution benchmark/harness/reports/baseline-3x-generation.json

PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/preflight.py \
  benchmark/harness/reports/baseline-3x-generation.json
```

`generation.py plan`은 B2 실행 가능 상태를 요구하지 않는 계획용 스냅샷이지만 외부 세션 실행에는 사용할 수 없다. 정식 실행에는 반드시 `seal`로 만든 `execution_ready=true` 파일을 사용한다.

## 8. 84개 세션 실행

### 8.1 비용 발생 확인

다음 명령은 Ollama Cloud의 외부 모델을 최대 84세션 호출한다. 사용자의 외부 호출 승인과 인증 준비를 확인한 뒤에만 두 환경 변수를 설정한다.

```bash
export BASELINE_3X_EXTERNAL_APPROVED=1
export BASELINE_3X_AUTH_READY=1
```

### 8.2 전체 큐 실행

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/run_queue.py \
  benchmark/harness/reports/baseline-3x-generation.json
```

큐는 최대 3개 B1/B2 묶음을 병렬로 처리한다. 같은 묶음의 두 조건은 차례로 실행한다. 실행 도중 중단되면 같은 명령을 다시 실행한다. 시작할 때 미완료 시도와 고아 상태를 복구하고, 이미 봉인된 유효 세션은 건너뛴다.

완료 조건은 다음 두 파일에서 확인한다.

```text
benchmark/harness/reports/queue-latest.json
benchmark/harness/runs/<generation-id>/ledger.json
```

필수값:

```text
queue-latest.json: all_completed = true
queue-latest.json: sealed_valid_slot_count = 84
ledger.json: state = completed
```

## 9. 실패와 재실행 규칙

| 상황 | 하네스 처리 | 같은 실행 세대 재개 |
|---|---|---:|
| 제공자 일시 오류 | 원시 시도 보존 후 재시도 | 가능 |
| 네트워크 일시 오류 | 원시 시도 보존 후 재시도 | 가능 |
| 인증 일시 오류 | 원시 시도 보존 후 재시도 | 가능 |
| 시간 초과 | 유효 관찰 결과로 보존 | 자동 재시도 안 함 |
| 모델 단계 30회 도달 | 유효 관찰 결과로 보존 | 자동 재시도 안 함 |
| 출력 2MiB 도달 | 유효 관찰 결과로 보존 | 자동 재시도 안 함 |
| MCP·코드·설정·질문·색인 변경 | 실행 세대 오염 | 불가능 |
| 자동 지표 생성 중 중단 | 봉인된 세션에서 지표 재개 | 가능 |
| 원자료·해시·권한 불일치 | 집계 불가 | 원인에 따라 새 세대 필요 |

한 실행 자리의 최대 시도는 3회다. 두 번째 시도 전 5초, 세 번째 시도 전 15초를 기다린다. 제공자가 알려 준 임의의 재시도 시간을 기준선 정책에 섞지 않는다.

실패한 시도 폴더를 삭제하지 않는다. `ledger.json`은 모든 시도를 보존하며 집계기는 각 자리의 최종 집계 가능 시도만 사용한다.

## 10. 자동 지표

`run-session.sh`는 세션을 발행한 뒤 자동 지표를 만든다. 위치는 다음과 같다.

```text
benchmark/harness/runs/<generation-id>/automatic-metrics/<run-id>/automatic-run-metrics.json
```

자동으로 기록하는 주요 값:

- 완료된 전체 도구 호출 수와 도구별 호출 수
- 도구 입력·출력 바이트
- OpenCode가 기록한 입력·출력·추론·캐시 토큰
- 전체 실행 시간과 완료 도구 시간
- 범위 없는 읽기 후보
- 같은 의미로 보이는 반복 검색 후보
- 최종 답과 종료 상태
- 집계 가능 여부와 제외 이유

정답 여부, 정답 발견 네 단계, 검색 과정의 최초 오류는 자동으로 추측하지 않는다. 11장의 가린 채점에서 판정한다.

## 11. 가린 3인 채점

채점은 두 단계로 나눈다. `scorer-1`, `scorer-2`, `scorer-3`은 서로의 결과를 보지 않는 독립 채점자여야 한다.

먼저 공통 변수를 정한다.

```bash
GENERATION=benchmark/harness/reports/baseline-3x-generation.json
GENERATION_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["generation_id"])' "$GENERATION")"
SCORING_ROOT="benchmark/harness/scoring/$GENERATION_ID"
ASSIGNMENTS="$SCORING_ROOT/coordinator-only/assignments.json"
```

### 11.1 1단계: 답의 정확성

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/make_scoring_bundles.py \
  correctness "$GENERATION"
```

각 채점자는 다음 파일 중 자기 묶음만 읽는다.

```text
benchmark/harness/scoring/<generation-id>/phase1-correctness/scorer-1/bundle.json
benchmark/harness/scoring/<generation-id>/phase1-correctness/scorer-2/bundle.json
benchmark/harness/scoring/<generation-id>/phase1-correctness/scorer-3/bundle.json
```

이 단계에는 도구 사용 과정, B1/B2 이름, 반복 번호, 실행 순서가 들어 있지 않다. 채점자는 최종 답과 정답 계약만 보고 판정 초안을 만든다.

각 초안은 `validate_judgment.py phase1`으로 검사해 다음 고정 경로에 저장한다.

```text
<phase1-input>/scorer-1/<review-id>.json
<phase1-input>/scorer-2/<review-id>.json
<phase1-input>/scorer-3/<review-id>.json
```

한 판정 검증 명령의 형태:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/validate_judgment.py \
  phase1 <draft.json> <phase1-input/scorer-N/review-id.json> \
  "$ASSIGNMENTS" correctness-only
```

252개 파일이 모두 준비되면 봉인한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/scoring_pipeline.py \
  seal-phase1 "$GENERATION" "$ASSIGNMENTS" <phase1-input> \
  <phase1-input/phase1-seal.json>
```

### 11.2 2단계: 검색 과정

1단계 봉인 뒤에만 검색 과정 묶음을 만든다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/make_scoring_bundles.py \
  process "$GENERATION" <phase1-input/phase1-seal.json>
```

각 채점자는 자기 `phase2-process/scorer-N/bundle.json`만 읽고 다음을 판정한다.

1. 검색 결과에 정답 근거가 나타났는가
2. 모델이 다음 행동에서 그 근거를 선택했는가
3. 원본 코드를 읽고 확인했는가
4. 최종 답에서 올바르게 사용했는가
5. 검색 과정이 처음 잘못된 호출은 무엇인가

각 초안은 `validate_judgment.py phase2`로 검사한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/validate_judgment.py \
  phase2 <draft.json> <phase2-input/scorer-N/review-id.json> \
  "$ASSIGNMENTS" <phase1-input/phase1-seal.json> process-only
```

252개 파일이 모두 준비되면 2단계를 봉인하고 두 단계를 합친다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/scoring_pipeline.py \
  seal-phase2 "$GENERATION" "$ASSIGNMENTS" \
  <phase1-input/phase1-seal.json> <phase2-input> \
  <phase2-input/phase2-seal.json>

PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/scoring_pipeline.py \
  merge "$GENERATION" "$ASSIGNMENTS" \
  <phase1-input/phase1-seal.json> <phase2-input/phase2-seal.json> \
  "$SCORING_ROOT/final-judgments"
```

최종 파일은 다음 위치에 생긴다.

```text
benchmark/harness/scoring/<generation-id>/final-judgments/final-seal.json
```

## 12. 집계 입력 봉인과 B1/B2 비교 결과 생성

84세션 장부와 252개 최종 판정이 모두 끝난 뒤 실행한다.

```bash
RUNS_ROOT="benchmark/harness/runs/$GENERATION_ID"
LEDGER="$RUNS_ROOT/ledger.json"
FINAL_SCORING="$SCORING_ROOT/final-judgments/final-seal.json"
ANALYSIS_ROOT="benchmark/harness/analysis-inputs/$GENERATION_ID"

PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/build_analysis_inputs.py \
  create "$GENERATION" "$ASSIGNMENTS" "$FINAL_SCORING" "$ANALYSIS_ROOT"
```

생성된 입력 봉인을 다시 확인한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/build_analysis_inputs.py \
  verify "$ANALYSIS_ROOT/analysis-input-seal.json"
```

최종 집계:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/analysis-tools/aggregate_baseline_metrics.py \
  --generation "$GENERATION" \
  --ledger "$LEDGER" \
  --metrics-index "$ANALYSIS_ROOT/metrics-index.json" \
  --mapping "$ASSIGNMENTS" \
  --scoring-manifest "$FINAL_SCORING" \
  --judgments-index "$ANALYSIS_ROOT/judgments-index.json" \
  --analysis-input-seal "$ANALYSIS_ROOT/analysis-input-seal.json" \
  --output benchmark/harness/reports/baseline-aggregate.json
```

집계 결과에서 최소한 다음을 함께 비교한다.

- 답의 정확성 점수와 완전 정답 비율
- 전체 토큰과 입력·출력·추론·캐시 토큰
- 전체 도구 호출과 검색·읽기·grep·find·overview 호출
- 도구 출력 바이트
- 범위 없는 읽기와 반복 검색 후보
- 정답 발견 네 단계의 성공 비율
- 최초 오류 유형과 그 이후 추가 호출·출력·토큰
- 과제별 3회 분산과 B2−B1 차이
- 집계에서 제외된 시도 수와 이유

## 13. 사전점검에서 확인하는 항목

`preflight.py`는 다음을 확인한 뒤 JSON 보고서에 각 항목을 `pass` 또는 `fail`로 기록한다.

- Python 캐시가 없는가
- 질문 틀, B1/B2 설정, 제한 파일의 해시가 고정값과 같은가
- OpenCode 실행 파일과 해시가 같은가
- 질문·정답이 정확히 14개이고 바이트 해시가 같은가
- 연습·봉인 과제가 섞이지 않았는가
- 84세션 순서가 7대7로 균형 잡혔는가
- Directus 소스와 고정 색인이 바뀌지 않았는가
- 소스와 색인이 읽기 전용인가
- APFS 복제 복사가 실제로 동작하는가
- B1과 B2의 차이가 MCP 하나뿐인가
- B2 소스·두 빌드·실행 파일·확인 보고서가 서로 연결되는가
- Ollama Cloud 인증 항목이 있는가
- 이전 실행의 인증 복사본이나 관련 프로세스가 남지 않았는가
- 봉인된 실행 세대와 현재 입력이 같은가

보고서 경로:

```text
benchmark/harness/reports/preflight-latest.json
```

## 14. 실행 종료 후 확인

다음 상태를 확인해야 측정이 끝난 것이다.

- `queue-latest.json`의 `all_completed`가 `true`
- `ledger.json`의 `state`가 `completed`
- 유효한 84개 자리 모두 자동 지표가 봉인됨
- `runtime-auth/`에 세션 인증 복사본이 남지 않음
- `work/`에 발행되지 않은 작업이 남지 않음
- OpenCode 또는 `codemap-search mcp` 관련 프로세스가 남지 않음
- 252개 1단계 판정과 252개 2단계 판정이 각각 봉인됨
- 최종 판정 252개가 병합됨
- 집계 입력 봉인 검사가 통과함
- 최종 집계가 종료값 0으로 끝남

세션이 중단되었을 때는 자료를 먼저 삭제하지 말고 같은 `run_queue.py` 명령을 다시 실행한다. 하네스가 복구하지 못한 이유를 `queue-latest.json`, `ledger.json`, 각 실행의 `attempt-classification.json`, `postprocess-status.json`에서 확인한다.

## 15. 새 기준선이 필요한 경우

다음 중 하나라도 바뀌면 기존 결과에 이어 붙이지 않고 새 실행 세대를 만든다.

- 질문이나 정답
- Directus 소스
- `.codemap` 색인
- OpenCode 버전이나 실행 파일
- 모델 또는 제공자 설정
- B1/B2 OpenCode 설정
- `codemap-search` 실행 파일
- 프롬프트 틀
- 세션 제한
- 하네스 실행·측정·채점 코드
- 자동 지표 또는 채점 JSON 형식

새 실행 세대는 기존 실행 폴더를 덮어쓰지 않는다. 무엇이 바뀌었는지 별도로 기록하고 B1과 B2 전체 84세션을 다시 측정한다.
