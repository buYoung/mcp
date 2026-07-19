# OpenCode 기준선 실행 자동 지표 추출기

한 개 OpenCode 실행 폴더에서 기계적으로 확인할 수 있는 원자료 기반 지표를 JSON으로 만듭니다. 네트워크, 외부 모델, 빌드, 색인을 사용하지 않고 입력 파일을 수정하지 않습니다.

이 결과는 채점 결과가 아닙니다. 실행 조건과 답을 함께 드러내는 원자료 증거이므로 `run.scorer_input_allowed=false`로 고정되며, 별도로 가린 채점 절차에는 전달하면 안 됩니다. 최종 세션 지표 형식과도 분리된 전용 스키마를 사용합니다.

## 실행법

일반 추출은 다음과 같습니다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 analysis-tools/extract_run_metrics.py /absolute/path/to/run --output /absolute/path/to/metrics.json
```

기준선 집계에 넣을 수 있는 실행만 통과시키려면 다음처럼 엄격 검사를 켭니다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 analysis-tools/extract_run_metrics.py /absolute/path/to/run --output /absolute/path/to/metrics.json --schema harness/schemas/automatic-run-metrics.schema.json --require-aggregation-eligible
```

출력 파일을 생략하면 표준 출력에 JSON을 씁니다. `--compact`를 주면 한 줄 JSON을 만듭니다.

종료값은 다음과 같습니다.

| 종료값 | 뜻 |
|---:|---|
| `0` | 일반 추출 완료, 또는 엄격 검사에서 스키마·무결성·집계 조건 모두 통과 |
| `2` | 실행 폴더나 출력 폴더 경로 오류 |
| `3` | JSON은 만들었지만 기준선 집계 조건 불충족 |
| `4` | 전용 스키마 검사 실패 |

일반 추출은 불완전하거나 무효인 실행도 증거 보존을 위해 JSON으로 남기고 `0`으로 끝날 수 있습니다. 기준선 실행 뒤에는 반드시 `--require-aggregation-eligible`를 사용해야 합니다.

## 입력 계약

실행 폴더 안에 다음 파일이 필요합니다.

- `raw/events.jsonl`
- `raw/stderr.log`
- `normalized.json`
- `wrapper.json`
- `run.manifest.json` 또는 `manifest.json`
- `attempt-classification.json`
- `postprocess-status.json`
- `auth-cleanup.json`
- `invariants.after.json`
- `artifact-manifest.json`

`finalization.json` 또는 `run.finalization.json`은 선택 항목이며, 없으면 실행 명세 안의 종료 정보를 확인합니다. 실행 폴더의 바로 위 폴더에는 발행 상태를 기록한 `ledger.json`이 있어야 합니다.

추출 시점에는 다음 순서가 이미 끝나 있어야 합니다.

1. 실행 종료와 원자료 기록
2. 정규화와 종료 상태 분류
3. 인증 정보 정리와 소스·색인 불변성 확인
4. 실행 폴더의 파일 목록·크기·SHA-256 봉인 및 읽기 전용 전환
5. 상위 `ledger.json`에 이 시도의 최종 발행 상태 기록
6. 자동 지표 추출

`artifact-manifest.json`은 자기 자신을 제외한 실행 폴더의 파일 집합, 바이트 수, SHA-256을 정확히 담아야 합니다. 파일 누락·추가, 해시·크기 불일치, 심볼릭 링크, 쓰기 가능한 봉인 파일이 하나라도 있으면 무효입니다.

## 집계 가능 조건

다음 조건을 모두 만족해야 `experiment.aggregation_eligible=true`가 됩니다.

- 시도 분류가 `measurement_status=valid`
- 실행 세대가 무효가 아니며 `generation_invalid=false`
- 대체 실행이 아니며 `replacement_allowed=false`
- 상위 장부에서 해당 과제·반복·기준선의 가장 최근 최종 발행 시도와 정확히 연결됨
- 실행 폴더 봉인 검사가 통과함
- 원자료 경계, 도구 호출, 토큰, 출력 바이트, 실행 식별자 등에서 중대한 무결성 문제가 없음

어느 조건이 실패했는지는 `run.integrity.issues`와 `experiment.aggregation_ineligible_reasons`에 남습니다. 무효 실행을 유효한 실행처럼 숫자 `0`으로 채우지 않습니다.

증거 무결성과 평균 포함 여부는 서로 다릅니다. 깨끗하게 기록된 일시적 인증·제공자·네트워크 실패는 `run.integrity.status=verified`일 수 있지만 `measurement_status=infrastructure_invalid`이므로 집계할 수 없고 재시도 대상입니다. 반대로 시간 초과, 모델 단계 제한, 출력 제한이 미리 정한 유효 종료라면 `measurement_status=valid`이고 다른 조건도 통과할 때 최종 `stop` 단계가 없어도 집계할 수 있습니다.

## 측정 규칙

### 원자료 경계

- `wrapper.json`의 `reducer_lines_accepted`까지만 측정 대상입니다.
- 그 뒤의 줄은 `tail_lines_excluded`와 꼬리 진단으로만 남고 도구 호출·토큰·답변 지표에 들어가지 않습니다.
- 수락 줄 수가 음수가 아니어야 하며 실제 파일 범위를 벗어나면 무효입니다.
- `normalized.json`이 기록한 원자료 SHA-256, 줄 수, 줄별 원문 복사본을 실제 수락 자료와 대조합니다.
- 최상위 이벤트와 바로 아래 `event`, `data`, `part`, `message` 형태만 정해진 규칙으로 읽습니다. 임의의 깊은 곳을 뒤져 그럴듯한 값을 만들지 않습니다.

### 도구 호출

- 호출 식별자는 `세션 ID:tool:호출 ID` 전체를 사용합니다. 서로 다른 세션에서 같은 호출 ID가 나와도 합치지 않습니다.
- 완료 호출은 정규화 재생 기록의 완료 식별자에 있는 경우만 집계합니다.
- 시작만 있고 끝나지 않은 호출은 `incomplete_tool_calls` 진단에 남기고 호출 수·비용 합계에서 제외합니다.
- 같은 전체 식별자의 생명주기 기록은 필드별로 합칩니다. 시작 기록의 입력·시작 시간과 종료 기록의 출력·오류·끝 시간을 결합하고, 각 값이 나온 원시 줄을 `field_provenance`에 남깁니다.
- 같은 전체 식별자의 완료 기록들이 도구명, 입력, 출력, 시간 등에 서로 다른 값을 주장하면 임의로 하나를 고르지 않습니다. 해당 호출과 영향받는 합계를 무효로 둡니다.
- 도구별 원시 줄 번호, 입력·출력 존재 여부, UTF-8 바이트, SHA-256, 시작·끝·경과 시간, 오류 여부를 남깁니다.
- 입력이나 출력이 없거나 JSON `null`이면 `null`이라는 문자열의 바이트 수를 만들지 않고 해당 바이트·해시를 `null`로 둡니다.
- 종료 기록에 명시적 오류 문구가 있고 출력만 없다면 오류 문구는 보존하고 출력 바이트·해시는 `null`로 둡니다. 출력이 없다는 사실만으로 증거 무결성을 깨지 않으며 `cost.tool_byte_measurement.output_complete=false`로 수집 범위를 표시합니다.

### 출력 바이트와 제한

- 실제 `raw/events.jsonl`과 `raw/stderr.log` 파일 크기를 `wrapper.json`의 관찰·보존·버림 바이트와 교차 확인합니다.
- 각 흐름에서 `관찰 바이트 = 보존 바이트 + 버림 바이트`가 성립해야 합니다.
- 전체 보존 바이트와 출력 제한, 잘림 상태도 함께 확인합니다.
- 불일치가 있으면 파일 봉인이 정상이어도 실행 무결성은 무효입니다.

### 토큰

- 공식 토큰 출처는 `normalized.json`의 `official_opencode.token_usage`입니다.
- 각 항목은 수락된 원자료의 `step_finish` 기록과 줄 번호 및 전체 `세션 ID:model:메시지 ID:부분 ID`로 다시 연결합니다.
- 원자료와 정규화 자료의 토큰 값은 자료형까지 정확히 같아야 합니다.
- `input`, `output`, `reasoning`, `cache_read`, `cache_write`, `total`은 모두 불리언이 아닌 음이 아닌 정수여야 합니다. 음수, 소수, 누락값은 허용하지 않습니다.
- `total`은 나머지 다섯 구성값의 합과 같아야 합니다.
- 같은 전체 모델 단계에 서로 다른 토큰 기록이 있으면 하나를 고르지 않습니다.
- 위 조건 중 하나라도 깨지면 실행 전체 토큰 합계를 `null`로 두어 일부 숫자가 기준선에 섞이지 않게 합니다.
- `stop`이 아닌 정상 제한 종료나 깨끗한 기반시설 실패에서 공식 토큰 기록 자체가 없으면 이는 실행 증거 오류가 아니라 지표 수집 누락입니다. 토큰 합계는 `null`, `coverage_complete=false`, 이유는 `coverage_reason`에 남깁니다. 낯선 단계의 토큰, 원자료 불일치, 음수·소수·합계 오류는 계속 중대한 오류입니다.

### 읽기 범위와 반복 검색

- 읽기는 양의 정수 제한 수가 있거나, 음이 아닌 정수 시작·끝이 함께 있고 끝이 시작 이상인 닫힌 범위여야 제한된 읽기로 봅니다.
- `offset`만 있거나, 범위 한쪽이 없거나, 음수·소수·역방향 범위면 범위 없는 읽기 후보로 기록합니다.
- 반복 검색은 유니코드 정규화, 대소문자 통일, 공백 정리, 단어 순서·단어 묶음 서명으로 기계적 후보만 만듭니다. 실제로 같은 뜻인지 또는 낭비인지 자동 판정하지 않습니다.

### 답변과 시간

- `stop` 종료의 최종 답은 최종 모델 메시지에 속한 모든 텍스트 부분을 원래 순서대로 이어서 기록하고, 정규화 재생 자료와 포장 실행 기록의 답과 대조합니다.
- `stop`이 아닌 종료는 최종 답이 없을 수 있습니다. 이때 텍스트·바이트·해시는 `null`, `coverage_complete=false`이며 최종 답 부재만으로 증거를 무효화하지 않습니다.
- 수락 재생이나 후처리가 실제로 불일치하거나 `protocol_failures`가 비어 있지 않으면 종료 유형과 관계없이 중대한 무결성 오류입니다.
- 실행 시작·종료 나노초와 전체 실행 밀리초는 포장 실행 기록과 실행 명세를 교차 확인합니다.
- 도구 시간은 완료 호출의 `state.time.end - state.time.start`입니다. 검색 서버 내부 처리시간은 지원 입력에 없으므로 `cost.codemap_internal_ms=null`이며 다른 시간을 대신 넣지 않습니다.
- 정답 발견 네 단계, 최초로 잘못된 지점, 정답 여부, 최초 오류 이후 비용은 자동 추출기가 추측하지 않습니다. 별도 대화 검토가 끝날 때까지 검토 전 상태 또는 `null`로 둡니다.

## 출력과 스키마 경계

최상위 키는 다음 아홉 개로 고정합니다.

```text
schema_version
run
experiment
dialogue
cost
repeated_searches
unbounded_reads
post_first_wrong
missing_data
```

전용 스키마는 `harness/schemas/automatic-run-metrics.schema.json`입니다. `additionalProperties=false`를 사용해 예상하지 못한 필드가 조용히 섞이는 것을 막습니다. 이 형식은 `session-metrics.schema.json`을 대신하지 않으며, 자동 원자료 지표를 사람 판정과 결합하기 전의 중간 증거로만 사용합니다.

## 검증

합성 자료 검사는 다음 명령으로 실행합니다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s analysis-tools/tests -p 'test_*.py' -v
```

검사 자료는 새 기준선 실행 형식으로 만든 별도 예시이며 과거 v4 실행을 복사하지 않습니다. 정상 발행 실행, 정상 시간 초과·모델 단계 제한·출력 제한, 일시적 인증·제공자·네트워크 실패, 분리된 도구 시작·완료 기록, 출력 없는 명시적 도구 오류, 누락 입력, 같은 호출 ID를 가진 다른 세션, 수락 경계 뒤 꼬리, 잘못된 대체 실행 분류, 출력 바이트 불일치, 토큰 중복·합계 불일치·음수·소수, 중첩 이벤트, 미완료 호출, 실제 통신 오류, 잘못된 읽기 범위, 동일 호출 충돌, 스키마의 추가 필드 거부, 엄격 종료값을 확인합니다.
