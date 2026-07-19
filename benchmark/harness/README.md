# 검색 품질 벤치마크 하네스

이 하네스는 같은 14개 질문으로 B1과 B2를 각각 3회 실행하고, 각 실행의 대화·도구 호출·토큰·최종 답을 봉인한 뒤 세 명의 독립 채점 결과와 결합한다.

처음부터 실행하려면 [상세 실행 안내서](../RUNBOOK.md)를 따른다. 이 문서는 하네스 폴더의 구성과 보호 장치를 설명한다.

## 처리 흐름

```text
고정 입력 확인
→ B1/B2 실행 조건 봉인
→ 42개 과제·반복 묶음을 최대 3개씩 실행
→ 84개 세션의 원자료와 자동 지표 봉인
→ 답의 정확성 채점
→ 검색 과정 채점
→ 252개 판정 병합
→ B1/B2 지표 집계
```

한 묶음은 같은 과제·같은 반복의 B1과 B2 두 세션이다. 두 세션은 같은 묶음 안에서 차례로 실행되고, 어떤 조건이 먼저 실행되는지는 3회 반복에 걸쳐 균형을 맞춘다.

## 폴더별 역할

| 경로 | 역할 | 생성물 여부 |
|---|---|---:|
| `config/` | B1/B2 설정, 실행 제한, 채점 기준 | 고정 입력 |
| `templates/` | 모든 과제에 공통으로 적용되는 질문 틀과 macOS 격리 틀 | 고정 입력 |
| `schemas/` | 실행 자료, 자동 지표, 채점, 최종 결과의 JSON 형식 | 고정 입력 |
| `scripts/` | 준비 확인, 실행, 복구, 봉인, 채점, 집계 | 코드 |
| `provenance/` | B2 기준 실행 파일의 검토 패치와 당시 증명 기록 | 고정 증거 |
| `reports/` | 자체검사, 사전점검, 실행 큐 보고서 | 실행 중 생성 |
| `runs/` | 세션별 원자료, 종료 상태, 자동 지표와 전체 장부 | 실행 중 생성 |
| `work/` | 아직 발행되지 않은 세션 준비 폴더 | 임시 자료 |
| `runtime-auth/` | 세션마다 복사되는 최소 인증 자료 | 임시 자료 |
| `scoring/` | 가린 채점 묶음과 3인 판정 | 채점 중 생성 |
| `analysis-inputs/` | 집계 입력의 경로·해시 봉인 | 집계 전 생성 |

생성물 경로 대부분은 `benchmark/.gitignore`에서 제외한다.

## 주요 실행 코드

| 파일 | 역할 |
|---|---|
| `preflight.py` | 외부 모델 호출 전 입력·권한·해시·프로세스 상태 확인 |
| `generation.py` | 한 번의 정식 측정을 구성하는 입력과 정책을 봉인 |
| `scheduler.py` | 14개 과제 × 3회 × B1/B2의 84세션 순서 생성 |
| `run_queue.py` | 최대 3개 묶음 병렬 실행, 중단 복구, 제한된 재시도 |
| `run-session.sh` | 한 세션의 격리·실행·원자료 기록·자동 지표 봉인 |
| `protocol.py` | 실행 자리와 시도 상태를 `ledger.json`에 기록 |
| `make_scoring_bundles.py` | 조건 이름을 숨긴 정확성·검색 과정 채점 자료 생성 |
| `validate_judgment.py` | 사람이 만든 판정 JSON을 고정 형식과 원자료에 대조 |
| `scoring_pipeline.py` | 두 채점 단계를 봉인하고 최종 판정으로 병합 |
| `build_analysis_inputs.py` | 집계에 쓰는 모든 파일 경로와 해시를 하나로 봉인 |

## 정식 실행에 필요한 별도 자료

```text
benchmark/runtime/opencode
benchmark/b2/source/
benchmark/b2/build-evidence/target-build1/release/codemap-search
benchmark/b2/target/release/codemap-search
benchmark/corpus/directus-index-golden/.codemap/
```

두 B2 실행 파일은 같은 소스와 Rust 환경에서 만든 바이트 단위 동일 결과여야 한다. `config/b2-runtime.json`의 경로·SHA-256과 `provenance/b2-clean-runtime-attestation.json`도 실제 파일과 일치해야 한다.

현재 커밋의 B2 설정과 증명 기록에는 최초 측정 당시 `/private/tmp` 절대 경로가 남아 있다. 이는 당시 기록을 보존하기 위한 것이며 새 작업 경로에서 바로 실행 가능한 설정이 아니다. 새 환경에서는 전용 작업 트리에서 B2 자료를 다시 만들고 `probe_b2.py`, `attest_b2.py`로 다시 증명해야 한다. 정확한 B2 소스와 고정 색인 생성 자료가 없다면 정식 실행을 시작하지 않는다.

## 하네스가 지키는 경계

- B1에는 MCP가 없고 B2에는 `codemap-search` 하나만 있다.
- B2의 MCP 사용을 프롬프트로 강제하지 않는다.
- 모델은 `ollama-cloud/deepseek-v4-flash`로 고정한다.
- Directus 소스와 색인은 세션에서 읽기 전용이다.
- 정답, 채점 기준, 다른 세션, 제품 저장소는 모델이 읽지 못하도록 macOS `sandbox-exec`으로 막는다.
- 한 세션은 600초, 모델 단계 30회, 보존 출력 2MiB로 제한한다.
- 세션 원자료를 먼저 봉인한 뒤 자동 지표를 만든다.
- 원자료가 바뀌거나 쓰기 가능하면 집계 대상에서 제외한다.
- 정확성 채점이 끝나기 전에는 도구 사용 과정을 채점자에게 보여주지 않는다.
- 각 세션은 서로 다른 세 명에게 채점되어 총 252개 판정이 만들어진다.

## 저장소만으로 가능한 검사

저장소 루트에서 실행한다. 외부 모델, 빌드, 색인을 사용하지 않는다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/scoring_selftest.py
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/analysis_inputs_selftest.py
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/analysis_tools_selftest.py
```

`selftest.py`는 실행 하네스 전체를 검사하지만 B2 소스, 두 번의 빌드 증거, 실행 파일, 고정 색인까지 요구한다. 이 자료가 없으면 실패하는 것이 정상이다.

## 실행 중 생성되는 핵심 파일

| 파일 | 확인할 값 |
|---|---|
| `reports/preflight-latest.json` | 모든 필수 검사가 `pass`인지 |
| `reports/queue-latest.json` | `all_completed=true`, `sealed_valid_slot_count=84`인지 |
| `runs/<generation-id>/ledger.json` | `state=completed`인지 |
| `runs/<generation-id>/automatic-metrics/*/automatic-run-metrics.json` | 도구 호출·토큰·출력량의 자동 측정 |
| `scoring/<generation-id>/final-judgments/final-seal.json` | 252개 판정의 최종 봉인 |
| `analysis-inputs/<generation-id>/analysis-input-seal.json` | 모든 집계 입력의 경로·해시 봉인 |

실패 원인, 재실행 조건, 채점과 집계 명령은 [상세 실행 안내서](../RUNBOOK.md)에 정리돼 있다.
