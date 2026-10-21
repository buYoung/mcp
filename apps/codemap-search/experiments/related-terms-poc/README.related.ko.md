# 관련어 사전 PoC 조회하기

로컬 개발자·검토자가 `rtpoc3 related`로 단어, 구, 코드 식별자의 관련어를 조회하는 안내입니다. 현재 데이터셋 `rtr1-930791c83bea6132`에는 **364,074개 표기와 916,718개 관계**가 있습니다. 조회기는 저장된 관계에서 후보를 모아 유사도순으로 반환합니다. 사용자 결정에 따라 넓은 범주를 먼저 구성하며, 본격적인 의미 필터링은 이 PoC 이후 단계로 남겨 두었습니다. 제품 MCP 통합은 이번 작업 범위에 포함되지 않습니다.

이 명령은 SQLite만 읽습니다. 조회할 때 저장소 색인, 정책 파일, 임베딩 모델 실행이나 외부 API 호출이 필요하지 않습니다. 기존 `lookup` 명령과 그 평가 기록은 별도입니다.

## PoC 종합 판정

**이번 기준에서는 불합격입니다.** 동작 검증은 통과했지만, 의미 품질과 실제 검색 효과가 합격선에 못 미쳤습니다. 아래 수치는 이 데이터셋을 평가한 결과이며, 평가에 쓴 스크립트와 원본 기록은 보존하지 않습니다.

| 항목 | 결과 | 합격 기준 |
| --- | --- | --- |
| 유용한 관련어 | 151/480쌍, 31.5%. 95% 신뢰구간 하한 27.5% | 추천 기준: 80% 이상이며 하한 75% 이상 |
| 의미 오류 | 314/480쌍, 65.4% | 추천 기준: 15% 이하 |
| Grafana 검색 성공률 | 관련어 없음 57.5% → 있음 55.5%, −2.0%p | +10%p 초과 개선 |
| codemap-search 검색 성공률 | 관련어 없음 66.0% → 있음 68.0%, +2.0%p | +10%p 초과 개선 |

의미 품질은 직접 연결된 개발 개념도 유용한 관계로 인정한 결과입니다. 기존 기준인 유용 비율의 95% 하한 75%도 충족하지 못했습니다. 검색 효과는 기준 검색이 놓친 32문제에서 `gpt-6-luna`와 `gpt-6.1-sol`을 조건별 10회씩 실행해 측정했습니다. 품질 라벨은 에이전트의 단일 판정이며, 문제집은 기존 자료를 재사용했습니다.

전체 구·부분 단어의 다른 뜻과 경쟁 기술 관계는 정제하지 않았습니다([데이터 구성과 알려진 한계](#데이터-구성과-알려진-한계) 참고).

## 바로 조회하기

저장소 루트에서 Bash 또는 Zsh로 실행합니다. 이 checkout에 보존된 `rtpoc3` 실행 파일과 아래 SQLite가 있어야 합니다. `.codemap/`의 데이터와 검증 원본은 로컬 산출물이므로 새 clone만으로는 준비되지 않습니다.

```sh
B=apps/codemap-search/experiments/related-terms-poc/lookup-v3/target/release/rtpoc3
D=.codemap/poc/related-terms/thesaurus-v1/datasets/rtr1-930791c83bea6132/dataset.sqlite3

"$B" related --dataset "$D" --format text \
  '$eventLoop' .net 'c++' k8s retry addEventListener
```

구는 `'event loop'`처럼 한 인자로 전달합니다. `$eventLoop`도 작은따옴표로 감싸 셸의 변수 치환을 막습니다. 정상적으로 후보를 찾으면 입력마다 `# 입력  [ok]`와 일치한 표기, 유사도 계산 기준, 관련어 목록이 나옵니다.

실제 `addEventListener` 결과의 첫 후보는 다음과 같습니다.

```text
   90.7%  event-listener  [query_part] <- event listener
```

한 줄은 `유사도%  관련어  [관계 종류] <- 관련어를 찾은 입력 쪽 표기`입니다. 백분율은 저장된 벡터의 **코사인 유사도 × 100**을 소수점 한 자리로 표시한 값이며, 정답 확률이 아닙니다.

실행 파일이 없을 때는 Rust 도구 체인과 의존성 캐시가 준비된 환경에서 아래 명령으로 빌드할 수 있습니다. 이 명령은 SQLite를 생성하지 않습니다. `--offline` 실행 중 캐시 부족 오류가 나면 필요한 의존성이 로컬에 없는 상태입니다.

```sh
cargo build --offline --release \
  --manifest-path apps/codemap-search/experiments/related-terms-poc/lookup-v3/Cargo.toml
```

## JSON으로 받거나 여러 입력을 파일로 처리하기

앞 절의 저장소 루트와 `B`, `D` 설정을 사용합니다. `--format`을 생략하거나 `--format json`을 지정하면 입력 하나당 JSON 객체 한 줄인 JSONL을 출력합니다.

```sh
"$B" related --dataset "$D" svelte 'event loop'
```

입력 파일은 한 줄에 한 입력을 담습니다. 빈 줄은 무시하며, `--terms-file`을 지정하면 위치 인자보다 입력 파일을 우선합니다. 다음 예시는 새 임시 디렉터리에 입력과 결과를 저장합니다.

```sh
RT_OUTPUT_DIR=$(mktemp -d /tmp/related-terms.XXXXXX) || exit 1
printf '%s\n' '$eventLoop' '.net' 'c++' 'addEventListener' \
  > "$RT_OUTPUT_DIR/terms.txt"

"$B" related --dataset "$D" \
  --terms-file "$RT_OUTPUT_DIR/terms.txt" \
  --output "$RT_OUTPUT_DIR/related.jsonl"

printf '결과 파일: %s\n' "$RT_OUTPUT_DIR/related.jsonl"
```

`--output`을 생략하면 표준 출력으로 보냅니다. 지정한 파일이 이미 있으면 덮어쓰며, 상위 디렉터리는 자동 생성하지 않습니다. `--format text`와 함께 쓰면 같은 파일에 텍스트 결과를 저장합니다.

JSON의 `schema`는 `rt-related-lookup/1`입니다. 주요 필드는 다음과 같습니다.

| 필드 | 읽는 방법 |
| --- | --- |
| `dataset_id`, `keys_version` | 사용한 데이터셋과 정규화 규칙 버전. 현재 값은 `rtr1-930791c83bea6132`, `rtk-1` |
| `status` | `ok`: 후보 있음. `no_match`: 사전에 일치한 표기 없음. `matched_without_relations`: 일치했으나 반환 후보 없음 |
| `matches` | 입력에서 일치한 부분 `span`, 사전 표기 `term`, 일치 방식 `match`, 범위 `scope` |
| `skipped` | 부분 분해에서 생략한 조각과 이유. `non_ascii`, `digits_only`, `single_letter` |
| `query_vector` | 유사도의 기준. 전체 표기를 쓰면 `whole_term`, 일치한 부분들의 벡터 평균이면 `mean_of_parts`, 기준이 없으면 `none` |
| `related` | 유사도 내림차순 후보. 표시 점수가 같으면 표기 문자열순 |
| `related[].similarity_pct` | 코사인 유사도 × 100. 계산할 벡터가 없으면 `null`; 텍스트에서는 `n/a%` |
| `related[].kinds` | 관계 종류. `reverse:` 접두사는 저장된 관계를 역방향으로 따라온 후보, `query_part`는 입력의 일치한 부분 자체 |
| `related[].sources`, `related[].via` | 후보의 자료 출처와 후보를 찾게 한 일치 표기. 출처 확인에는 JSON을 사용 |

일치 항목이 없다는 상태는 명령 실행 오류와 다릅니다. 파일을 열 수 없거나 스키마가 맞지 않는 경우에는 `dataset unavailable:` 또는 `dataset incompatible:` 오류가 납니다. `related`에는 `rt-related/1` SQLite를 전달해야 하며, 과거 `rt-dataset/1` SQLite는 사용할 수 없습니다.

## 표기와 관련어를 찾는 범위

전체 입력을 먼저 찾고, 식별자와 구를 분해해 부분 일치도 수집합니다. 대소문자, 공백·구분자, 붙여 쓴 표기와 영어 활용형을 처리합니다. 예를 들어 `eventLoop`, `event_loop`, `'event loop'`, `eventloop`를 찾고 `event`, `loop`도 조회합니다. 일치한 표기에 연결된 정방향·역방향 관계를 한 단계 따라가 후보를 모읍니다.

- `.net`, `c++`, `c#`처럼 기호에 의미가 있는 등록 표기는 보호하며, 기호를 제거한 `net`, `c`로 바꾸어 조회하지 않습니다.
- `k8s`, `i18n`처럼 문자와 숫자가 섞인 표기는 유지합니다.
- 카테고리 어휘에서 카멜 표기의 고유명사로 분류한 320개 항목은 분해하지 않습니다. `JavaScript`, `GitHub`가 여기에 해당합니다. `addEventListener`는 `add`, `event`, `listener`까지 분해됩니다.
- 비ASCII 표기는 사전에 등록된 전체 표기로 일치할 수 있지만 부분 단어 분해에서는 생략합니다. `café`를 `caf`로 잘라 조회하지 않습니다.
- 전체 구는 길이와 관계없이 조회하고, 부분 구 조회는 최대 6단어 범위로 제한합니다.

전체 입력에 일치한 첫 표기가 있으면 그 저장 벡터를 유사도 기준으로 씁니다. 없으면 서로 겹치지 않는 긴 일치 부분을 우선 선택하고 보호 표기를 포함해 벡터 평균을 사용합니다. 후보 수 상한과 유사도 하한은 없으며, 같은 사전 항목으로 모인 근거는 합칩니다. 저장소에서 실제 사용하는 단어인지도 필터링하지 않습니다.

## 데이터 구성과 알려진 한계

현재 SQLite는 **451,076,096 bytes(약 430.2 MiB)**입니다. `jinaai/jina-embeddings-v2-base-code`의 768차원 벡터를 `int8`로 양자화해 각 표기와 함께 저장합니다. 생성 시점의 입력과 집계는 [빌드 기록](../../../../.codemap/poc/related-terms/thesaurus-v1/datasets/rtr1-930791c83bea6132/build-report.json)에 있습니다.

52개 카테고리에서 수집한 어휘를 바탕으로 생성한 관련어를 기존 두 사전과 합쳤습니다. 최종 관계 수는 방향·관계 종류·출처를 구분한 행 수이며, 고유한 무방향 단어 쌍 수와는 다릅니다. `antonym`으로 표시된 관계는 제외했습니다.

| 생성 자료 | 표제어 수 | 관계 수 | 표제어당 평균 관계 수 |
| --- | ---: | ---: | ---: |
| Claude | 11,041 | 519,202 | 47.02 |
| Codex | 806 | 17,659 | 21.91 |
| 합계 | 11,847 | 536,861 | 45.32 |

나머지 자료 출처는 Open English WordNet, GitHub Explore, abbrcode, 소프트웨어 공학 약어, Wikidata입니다. LLM 자료에는 `synonym`, `abbreviation`, `form`, `family`, `near`, `identifier` 관계가 있으며, 기존 자료에서 `alias`, `similar`, `related_topic`도 들어옵니다.

**경쟁 제품·프레임워크 관계가 남아 있습니다.** 현재 `svelte`를 조회하면 `Blazor`가 73.9%로 반환됩니다. 원본은 `github-explore-c8d7bc5`의 `Blazor → Svelte` 관계이며 종류는 `related_topic`입니다. 역방향 조회 결과에는 `reverse:related_topic`으로 표시됩니다. 동의어로 해석하기 어려운 이 종류의 관계가 후속 필터링 대상입니다. 유사도 정렬만으로 이런 관계가 제거되지는 않습니다.

**생성 과정에서 잘렸던 12,179개 관계를 복원했습니다.** 이전 데이터셋 `rtr1-133e54e99405e270`에는 `batch-009`의 표제어당 70개, `batch-013`의 50개 제한이 남아 있었습니다. 두 배치의 LLM 원출력을 다시 정리하면서 중복 제거와 오류 교정은 유지하고, 개수를 자르는 처리만 없앴습니다.

| 배치 | 이전 관계 수 | 복원 후 관계 수 | 추가 관계 수 |
| --- | ---: | ---: | ---: |
| `009` | 18,389 | 23,297 | 4,908 |
| `013` | 14,147 | 21,418 | 7,271 |

기존에 남아 있던 항목은 모두 보존했습니다. `duration`의 생성 관계는 70개에서 157개, `.mdf`는 50개에서 167개가 됐고, 새 관계가 실제 CLI 출력에 포함되는 것도 확인했습니다.

개수 제한을 제거했다고 생성 자료가 완전하다는 뜻은 아닙니다. Codex 자료의 평균 관계 수도 Claude 자료보다 적습니다. 서로 다른 표제어 집합을 담당했으므로 이 수치만으로 모델의 품질 차이를 판정할 수 없습니다. 위 종합 평가의 의미 품질은 검색 과제에서 반환된 후보를 표본으로 측정한 값입니다. 생성 관계 전체의 의미 정확도와 관련어 누락률을 전수 측정한 것은 아닙니다.

데이터를 다시 만드는 코드는 [build_related_dataset.py](build_related_dataset.py)입니다. 실행에는 LLM 관계 JSONL과 어휘, 기존 v2·원본 SQLite, `fastembed`·`numpy` 환경과 임베딩 모델이 필요합니다. 이 입력 자료는 보존하지 않으므로 같은 데이터셋을 다시 만들 수 없습니다. 입력의 출처는 [원자료와 근거 저장소](../../../../docs/related-terms/sources.md)에 있습니다. 기존 SQLite를 조회하는 데 재생성은 필요하지 않습니다.

## 검증 결과 확인하기

관계 복원 후 새 데이터셋으로 전체 검증을 다시 실행한 결과는 다음과 같습니다. 원본 검증 기록은 보존하지 않습니다. 이 검사는 정규화·분해·정렬·회귀 동작을 확인하며, 관련어의 의미 정확도를 판정하지 않습니다.

| 검사 | 이번 결과 |
| --- | --- |
| Python 생성 키와 Rust 조회 키 | 364,074개 전수 검사, 불일치 0 |
| 표기 변형 | 28,312개. 기대 일치 누락 0; 이 중 16개는 별도의 보호 표기로 처리 |
| 기호를 붙인 입력의 결과 | 21,478개 동일. 122개는 기호를 포함한 표기 자체가 사전 항목 |
| 고정 사례 | `.net`, `c++`, `HTTP/2`, `café`, `JavaScript`, `addEventListener` 등 24개 통과 |
| 출력 정렬·점수 | 출력 28,336개에서 정렬 위반 0, 점수 없는 후보를 포함한 출력 0 |
| 기존 `lookup` 회귀 | 1,187개 입력에서 동결된 `rtpoc2`와 비교 대상 출력 동일 |
| 동결 자료 | 12개 파일의 SHA-256 변경 없음 |
| 복원 데이터 반영 | 두 배치의 관계 44,715개 전수 확인, SQLite 누락 0 |
| SQLite 무결성 | `integrity_check`: `ok`, 외래 키 오류 0 |

같은 시점에 `lookup-v3`의 Rust 단위 테스트 23개도 통과했습니다.

저장소 루트에서 앞의 `B`, `D`를 설정한 뒤 키 정합성만 다시 확인할 수 있습니다.

```sh
"$B" related-check --dataset "$D"
```

현재 데이터셋의 정상 결과는 `status: "pass"`, `terms_checked: 364074`, `mismatches: 0`입니다. 이 명령은 키 검사이며 의미 품질이나 SQLite 전체 무결성 검사를 대신하지 않습니다.

기존 단위 테스트는 다음 명령으로 실행합니다.

```sh
cargo test --offline \
  --manifest-path apps/codemap-search/experiments/related-terms-poc/lookup-v3/Cargo.toml
```

전체 변형·회귀 검증의 실행 인자는 [verify_related.py](verify_related.py)에 정의되어 있습니다. 재실행할 때는 동결된 `rtpoc2`, v2 데이터셋, 정책, 보존된 입력 목록과 `method-freeze.json`이 필요합니다. 이 입력은 보존하지 않으므로 지금은 다시 실행할 수 없습니다. `--work`에는 기존 증거를 덮어쓰지 않을 새 작업 디렉터리를 지정합니다. 검증 항목이 실패하면 종료 코드 1, 모두 통과하면 0을 반환합니다. 실패 원인은 작업 디렉터리의 `verification.json`에서 확인합니다.
