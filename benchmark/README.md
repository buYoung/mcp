# codemap-search 하네스 V2

Grafana의 고정 코드에 대해 기본 탐색 도구(A)와 codemap-search(B)를 비교한다. **완전 정답률을 먼저 평가하고 토큰·호출 비용은 별도로 보고한다.** 품질과 비용을 합친 점수는 만들지 않는다.

구현과 데이터는 [v2](v2/)에 있다. [고정 데이터셋](v2/data/dataset.json)은 준비 6문제와 본평가 30문제, 질문별 핵심 사실·원문·정답/부분 정답/오답 대조 사례를 포함한다. [선택된 PR 출처](v2/data/provenance/)에는 본문, 병합 커밋, 변경 파일, 패치, 고정 커밋의 조상 관계를 보존한다. 원문 행과 정답의 의미는 함께 검토해야 한다. 구조 검사만으로 자연어 정답의 의미가 입증되는 것은 아니다.

2026-09-07 검증에서는 자동검사 23개, A/B 런타임 검사, 준비 실행 12회와 Astra 채점·보고 생성을 수행했다. 제한 종료 4회의 전체 토큰을 확정하지 못해 본평가 120회는 시작하지 않았다. [검증 기록](v2/data/verification-status.json)과 로컬 [준비 보고서](v2/artifacts/preparation/report/report.md)를 확인할 수 있다.

## 고정 실행 조건

| 항목 | 값 |
| --- | --- |
| Codex CLI | `0.153.4`, 새 `codex exec` 세션 |
| 실행 모델 | `gpt-5.6-luna`, `medium` |
| 채점 모델 | `gpt-6-astra`, `low` |
| Grafana | `c6fad8695a96577eb466d425e6ac4a759ca30f47` |
| B 제품 | `0.7.0`, 소스 `146d779a7d186327e765c2837637ad0543f802d1` |
| A 도구 | `rg`, `grep`, `find`, `read` |
| B 도구 | `initial_instructions`, `overview`, `search`, `grep`, `read`, `find` |
| 공통 호스트 | MCP 중계, `exec`·`wait` 래퍼. 다른 도구 namespace 제외 |
| 제한 | 실행당 300초, 실제 탐색 80회, 누적 입력+출력 500,000토큰 |
| 실행 수 | 준비 12회, 본평가 120회 |
| 순서 | 시드 `20260907`, 최대 2개 동시 실행, 2회차 A/B 순서 반전 |
| 통계 | 난이도 안에서 문제 단위 10,000회 부트스트랩, 95% 구간 |

`core.py`의 `SPEC`이 기계적 고정 계약이다. 모델이나 추론 강도를 자동으로 바꾸지 않는다. 각 실행은 독립된 Codex 설정 디렉터리, 소스 복사본, MCP 프로세스, 제품 전역 설정을 사용한다. 기존 Codex 로그인 파일은 실행 중 링크로 참조하고 링크를 제거한다. 인증 내용을 읽어 출력하거나 결과에 복사하지 않는다.

현재 구현은 Python 3.12 이상, macOS 또는 Unix 환경을 대상으로 한다. Python 외부 패키지는 필요 없다. `codex`, `gh`, `git`, `rg`, `grep`, `find`, Rust/Cargo와 정상적인 Codex 로그인이 필요하다. macOS에서는 `cp -cR`로 소스를 복제하고, 다른 Unix에서는 일반 복사를 사용한다.

## 실행 순서

모든 명령의 작업 디렉터리는 저장소 루트다. 큰 소스·색인·실행 결과는 Git에서 제외한 `benchmark/v2/artifacts/` 아래에 저장한다. 이미 실행한 결과 디렉터리와 고정 데이터셋 파일을 덮어쓰지 않는다.

### 1. 공개 PR과 소스 준비

현재 선택한 문제는 80개 후보를 검토한 결과다. 추가 후보 수집의 상한은 400개다. 수집은 `gh api`로 공개 Grafana 자료를 읽으며 외부 시스템을 변경하지 않는다.

```sh
python3 -m benchmark.v2 collect --output benchmark/v2/artifacts/collection --max-candidates 80
git clone --filter=blob:none --no-checkout https://github.com/grafana/grafana.git benchmark/v2/artifacts/grafana
git -C benchmark/v2/artifacts/grafana checkout --detach c6fad8695a96577eb466d425e6ac4a759ca30f47
```

`collect`는 병합 기간·조상 관계·백포트 제목·코드 파일 유무를 선별한다. 실제 코드에서 동작이 유지되는지와 변경/이슈 중복은 문제 작성 단계에서 확인한다. 검색 API 페이지는 최초 응답 그대로 보존하므로 나중의 검색 순서 변화를 소급 적용하지 않는다.

### 2. 데이터셋 검증 또는 새로 생성

포함된 `dataset.json`을 사용할 때는 재생성하지 않고 검증한다.

```sh
python3 -m benchmark.v2 verify --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana
```

새 질문을 작성할 때 [selection.json](v2/data/selection.json)의 형식을 따른다. `requirement`는 질문 안의 실제 부분문자열이고, `evidence_sets`는 해당 사실을 완전히 증명하는 원문 묶음들이다. 대체 묶음도 허용한다. 본평가는 각 난이도에 TypeScript/TSX 5개와 Go 5개가 필요하다. 준비/본평가는 PR·변경을 중복 사용하지 않으며 명시적으로 닫는 이슈도 교차 검사한다.

아래 명령의 출력 파일은 아직 존재하지 않아야 한다. 원문과 대조 사례가 검증되지 않으면 생성에 실패한다.

```sh
python3 -m benchmark.v2 build-dataset --selection benchmark/v2/data/selection.json --collection benchmark/v2/artifacts/collection --source benchmark/v2/artifacts/grafana --output benchmark/v2/artifacts/new-dataset.json
```

### 3. 자동검사·제품 빌드·실제 런타임 검사

```sh
python3 -m benchmark.v2 verify --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --build-product benchmark/v2/artifacts/pinned-product --runtime-probe benchmark/v2/artifacts/probe-final
```

`--build-product`는 고정 제품 커밋을 별도 경로에 추출하여 `cargo build --release --locked`로 빌드하고 소스 아카이브·명령·바이너리 해시를 기록한다. 설치된 MCP와 제품 작업 트리는 바꾸지 않는다. 빌드 및 런타임 검사 경로는 새 경로여야 한다.

오프라인 검사에는 요청별 토큰 중복 제거, 캐시 이중 합산 방지, 루프·병렬 요청 집계, 범위 밖 읽기, 잘린 출력의 근거 판정, 결측 분모, 대조 사례와 참조, 25% 경계, 통계 재현성, 보고서 생성이 포함된다. 실제 런타임 검사는 A/B 도구 목록, 직접 파일·네트워크 API 부재, 허용/차단 읽기, 최종 답변과 요청별 토큰을 확인한다. 이 검사는 준비 문제 12회의 대체가 아니다.

`verify`를 옵션 없이 다시 실행하면 오프라인 검사만 기록한다. 정식 실행에는 현재 코드 버전의 제품 빌드와 실제 런타임 검증 기록이 필요하다. 실행 옵션이 같은 기존 기록은 모델 호출 없이 다시 검사할 수 있다.

```sh
python3 -m benchmark.v2 verify --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --product-build benchmark/v2/artifacts/pinned-product/build.json --runtime-evidence benchmark/v2/artifacts/probe-sealed
```

### 4. 준비 실행 12회, 이후 채점·보고

```sh
python3 -m benchmark.v2 run --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --product benchmark/v2/artifacts/pinned-product/target/release/codemap-search --experiment benchmark/v2/artifacts/preparation --phase preparation
python3 -m benchmark.v2 grade --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/preparation
python3 -m benchmark.v2 report --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/preparation
```

실행 전에 예정 슬롯 전체와 명세·데이터셋·하네스·제품 해시를 고정한다. 실행 실패를 삭제하거나 모델을 자동 재실행하지 않는다. `run`을 같은 디렉터리에서 다시 호출하면 아직 `scheduled`인 슬롯만 시작한다. `running` 상태라도 종료된 모델의 `execution.json`이 남아 있으면 원자료 후처리만 복구하며 이전 상태를 `run-before-recovery.json`에 보존한다. 모델 종료 근거가 없으면 자동 성공·재실행으로 바꾸지 않는다.

모든 실행의 종료 상태가 있어야 `grade`가 진행한다. A/B 이름과 비용을 가린 결정적 묶음에 공통 원문과 사전 판정 대조 사례를 넣는다. 한 묶음은 최대 10문제·실제 답변 40개·160KiB다. 채점 실패·참조 오류·대조 사례 실패는 묶음 전체의 불완전 상태로 보존하며 Luna를 다시 실행하지 않는다.

### 5. 본평가 120회

준비 보고서의 `harness.ready_for_main`이 true일 때만 시작한다. 모델 실행 코드·명세·정답 계약을 바꿨다면 이전 고정 명세로 이어 실행할 수 없다. 순수 집계 수정은 모델 실행 함수·도구 중계·공통 계약의 서명이 동일한지 확인하고 별도의 `aggregation_sha256`을 기록해 기존 원자료에 적용한다. 초기 고정본에 실행 서명이 없으면 `harness-at-freeze/`가 원래 하네스 해시와 일치해야 한다.

```sh
python3 -m benchmark.v2 run --dataset benchmark/v2/data/dataset.json --source benchmark/v2/artifacts/grafana --product benchmark/v2/artifacts/pinned-product/target/release/codemap-search --experiment benchmark/v2/artifacts/main --phase main --preparation-experiment benchmark/v2/artifacts/preparation
python3 -m benchmark.v2 grade --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/main
python3 -m benchmark.v2 report --dataset benchmark/v2/data/dataset.json --experiment benchmark/v2/artifacts/main
```

## 지표와 판정

완전 정답은 유효한 최종 답변, 모든 핵심 사실의 정확성, 각 사실의 유효한 원문 근거, 중대한 오류 없음이 모두 충족될 때다. Astra는 사실·근거·오류만 반환하고 Python이 정답/부분 정답/오답/답변 없음/판정 불가를 도출한다. `import`나 파일명 일치만으로 소비 관계를 인정하지 않는다. 충분한 검색 발췌가 있으면 추가 `read`가 필수는 아니다.

전체 평균은 각 문제의 반복 평균을 구한 뒤 문제를 동일하게 가중한다. 실패와 제한 종료를 비용 분모에서 빼지 않는다. 환경·계측·채점 결측은 `null`과 사유로 남기고, 확인된 부분 합계는 `observed_partial_sum`으로만 표시한다. 토큰이 누락되어도 유효한 품질 자료는 별도로 보고한다. 캐시 입력은 입력 토큰의 부분집합이다.

총토큰은 `token_usage_record.response_id`별 입력+출력의 합으로 계산하고 누적 이벤트와 대조한다. 식별자가 같은 상충 기록은 무효다. 종료되지 않은 모델 요청의 비용을 확정할 수 없으면 전체 토큰을 `null`로 표시한다. 실행 시간은 실제 경과 시간이며 도구 시간을 합산하지 않는다.

탐색 호출은 중계기가 받은 A/B 요청별로 센다. 한 `exec`에 요청 6개가 있으면 6회다. 오류·재호출·B 초기 안내도 포함한다. 모델에게 실제 전달된 `custom_tool_call_output`을 제품 원출력과 구분하며, 일치하지 않는 변형·생략·잘림의 근거는 관측 판별 불가로 남긴다. 파일명만으로 원문 노출을 인정하지 않는다. 검색/개요의 제품 내부 잘림 표시를 확정할 수 없는 경우에도 `null`을 사용한다.

A/B 정확도 차이는 pp, 비용 변화는 **평균의 비율**이다. A 평균이 0이면 변화율 대신 절대 차이를 기록한다. 평균 총토큰 또는 탐색 호출이 반올림 전 25% 이상 증가하면 비용 급증이다. 정확도 부트스트랩 구간이 0을 포함하는 상승은 확정 근거 부족으로 표시한다. 문제별 퇴행·중대 오류와 양의 추가 토큰 기여도를 따로 공개한다.

양쪽 모두 정답인 동일 문제·반복의 비용과 전체 유효 토큰/완전 정답 수는 보조 지표다. 포함된 대응쌍·문제 수를 표시하며 재시도 기대 비용으로 해석하지 않는다. 30문제 결과는 이 데이터셋에 한정한다.

## 결과 확인과 보존

각 실험의 `report/report.md`에 전체, 난이도·언어, 문제별, 비용 증가·실패, 하네스 유효성의 다섯 표가 생성된다. `report/summary.json`과 `normalized.json`에는 값·단위·분자·분모·유효성·결측 사유·원자료 위치가 있다. 재집계는 검증한 원시 로그에서 관측값을 다시 만들며 봉인한 기록을 덮어쓰지 않는다.

Codex 종료 뒤 이미 요청된 도구의 결과가 도착할 수 있다. 원래 해시와 정확히 일치하는 JSONL 접두사, 기존 미완료 요청 ID, 종료 뒤 시각이 모두 확인된 추가분만 `terminal-seal.json`으로 별도 고정한다. 이를 모델이 받은 결과로 세지 않는다. 기존 바이트 수정이나 새 요청 추가는 계속 검증 실패다.

| 경로 | 내용 |
| --- | --- |
| `frozen.json`, `schedule.json` | 고정 명세와 빠짐없는 예정 슬롯 |
| `preparation/` | 별도 색인 준비 시간·크기·소스 해시 |
| `runs/<id>/` | 원시 Codex/중계 기록, 답변, 실행 상태, 계측, 파일 해시 |
| `grading/` | 블라인드 묶음, 비공개 대응표, 실제 Astra 기록·비용, 대조 검사 |
| `report/` | 실행별 정규화 지표, 집계, 불확실성, 판정, 보고서 |

하네스 구축/사전 검증 완료 여부와 제품의 정확도·효율 목표 달성 여부는 별도 결론이다. 보고서 생성 자체가 제품 개선을 뜻하지 않는다. 자료가 불완전하면 해당 비교를 보류한다. 비용 원인에 대해서는 관측된 반복·출력·오류와 성공의 연관성을 인과관계로 단정하지 않는다.

기존 하네스 폐기 내역은 [폐기 기록](RETIRED.md), 제품 후보 패치는 [패치 보관 안내](experiments/codex-codemap/README.md)에 남긴다. Codex 설정의 외부 참고는 [공식 설정 문서](https://developers.openai.com/codex/config-reference/)이며, 실제 옵션·이벤트 형식은 고정된 로컬 CLI의 검증 기록을 따른다.
