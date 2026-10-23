# 16단계: 뜻 데이터셋 저장 방식 최적화

작업일: 2026-10-17. 데이터셋: `rts4-301d57d419380d08` → `rts5-75e02862daa29308`(저장 방식만 바꿈). 검증 뒤 사용자 확인을 받아 `rts5-75e02862daa29308`을 기본값으로 바꿨다(6절).

사용자가 sqlite 파일 자체의 최적화를 요청했다. 조회 스크립트의 처리는 대상이 아니다. Claude Opus 5.5가 파일을 분석하고, 사본으로 후보마다 효과를 잰 뒤, 사용자가 고른 네 가지(A~D)를 새 스키마 판 `rt-sense/5`로 적용했다.

## 1. 분석

### 지금 파일

| 항목 | 값 |
| --- | --- |
| 크기 | 48,615,424바이트(4KB 페이지 11,869개) |
| 빈 페이지 | 0개. 페이지 안 빈 공간도 1% 미만이라 VACUUM으로 줄지 않는다 |
| `relations` 테이블 | 25.9MB(53%) |
| `relations_by_word` 인덱스 | 11.2MB(23%) |
| `input_senses` 테이블 | 6.7MB(14%) |
| 나머지 | 4.8MB(10%) |

조회 스크립트(`SenseIndex.load`)는 시작할 때 모든 테이블을 통째로 읽고, 단어를 찾을 때는 메모리만 본다. 적재 1.75초 가운데 sqlite가 쓰는 시간은 약 0.55초다(`relations` 0.33초, 외래 키 검사 0.1초, `input_senses` 0.08초). 나머지는 Python 처리다. 그래서 sqlite를 바꿔 줄일 수 있는 것은 주로 파일 크기다.

인덱스를 쓰는 곳은 데이터셋을 고치는 도구와 시트를 만드는 스크립트다. 조회 스크립트도 적재할 때 하는 검사 하나(`concept without personas`)가 `personas_by_concept`을 쓴다. 이 인덱스를 빼면 그 검사가 2.9ms에서 1,255ms로 늘어난다.

### 후보와 측정

사본마다 조회가 읽는 행이 원본과 같은지 해시로 확인했다. 시간은 5회 측정의 중앙값이다.

| 안 | 크기 변화 | 조회 영향 |
| --- | ---: | --- |
| A. `relations_by_word`, `personas_by_topic` 인덱스 빼기 | −11.8MB(−24%) | 없음. 도구의 단어별 조회는 1회 0.7ms에서 6.7ms로 느려진다. 인덱스를 다시 만드는 데는 0.2초 |
| B. `kind`를 종류 번호로 저장(종류 7개) | −3.1MB(−6%) | 없음. 조회는 `kind`를 읽지 않는다 |
| C. `surface`가 `word_key`와 같으면 저장하지 않기 | −1.75MB(−4%) | 없음. 38%가 같다 |
| D. `term_legacy_categories` 빼기 | −0.25MB | 없음. 지금 쓰는 도구가 없다 |
| A~D 함께 | −16.9MB(−35%) | 없음 |
| 단어 사전(문자열을 정수 번호로) | 인덱스를 빼면 −16.8MB | 조인 때문에 `relations` 읽기가 0.37초에서 0.62초로, 외래 키 검사가 0.1초에서 0.33초로 느려진다 |
| `relations` 기본 키를 `(term_id, rank)`로 | +1.2MB | 정렬이 없어져 읽기가 80ms 빨라진다. 대신 `(term_id, word_key)` 중복 방지가 없어지고, 이를 인덱스로 지키면 +13.6MB |
| 페이지 크기 8K·16K·64K | −0.1MB, +0.1MB, +1.3MB | 차이 없음 |
| `input_senses`를 규칙으로 다시 계산 | −6.7MB | 단순 규칙으로 81%만 재현된다. 검토 수정이 섞여 있어 저장해 둔다 |

단어 사전, 기본 키 변경, 페이지 크기 변경은 하지 않았다. `personas_by_concept`, 중복을 막는 `personas_by_mapping`, 외래 키가 가리키는 `terms.word_key`는 남겼다.

### git에 쌓이는 크기

임시 저장소에서 판 세 개(`e5083`, `710c`, `301d`)를 묶으니(pack) 18.6MB, 0.73MB, 0.84MB였다. `710c`와 `301d`는 페이지 11,869개 가운데 8,852개(75%)가 같은 자리에 그대로 있어서, git이 차이만 저장한다. 따라서 판을 바꿀 때 git 저장소가 크게 늘지는 않는다. 저장 방식을 바꾸면 페이지 배치가 달라지므로 첫 판은 처음부터 저장된다.

## 2. 바꾼 것

### `rt-sense/5`

`rt-sense/4`와 같은 행을 담는다. 달라진 것은 다음과 같다.

- `relations.kind_id`가 `relation_kinds(kind_id, kind)`를 가리킨다. 종류 이름은 한 번만 저장한다.
- `relations.surface_override`는 표기가 `word_key`와 다를 때만 값이 있다. 같은 경우를 다시 저장하지 않도록 `CHECK (surface_override <> word_key)`를 걸었다.
- 열 이름을 바꿔서, `rt-sense/4`용으로 쓴 쿼리는 이 판에서 번호나 빈 값을 이름으로 잘못 읽지 않고 바로 오류를 낸다.
- 뷰 `relation_texts`는 관련어 행을 `rt-sense/4`와 같은 모양(종류 이름, 전체 표기)으로 보여 준다.
- `relations_by_word`, `personas_by_topic`, `term_legacy_categories`가 없다.

옛 코드는 `rt-sense/5`를 모르는 판으로 보고 거부한다. 판 번호를 올린 이유다.

### 코드

| 파일 | 바뀐 것 |
| --- | --- |
| `sense_mapping_schema.py` | `rt-sense/5` 정의(`COMPACT_SQL`), 판별 데이터셋 id 접두어, 관련어 열 검사. 레이아웃 검사가 `sense_meta`의 판을 읽어 두 판을 모두 검사한다 |
| `compact_sense_dataset.py`(새 파일) | `rt-sense/4` 판을 `rt-sense/5` 판으로 복사한다. 복사한 뒤 모든 관련어 행을 출발 판과 비교하고, 하나라도 다르면 멈춘다 |
| `sense_lookup.py` | `rt-sense/5`를 읽는다. 표기는 `coalesce(surface_override, word_key)`로 읽는다 |
| `revise_topics.py`, `add_topic_mappings.py` | 두 판을 모두 받고, 출발 판과 같은 판을 만든다. 데이터셋 id 접두어도 출발 판을 따른다(`rts4-`, `rts5-`). `rt-sense/4` 출발 판의 id는 전과 같다 |
| `sense-mapping/prepare_mapping_sheets.py` | `rt-sense/5`에서는 `relation_texts`에서 종류와 표기를 읽는다 |

`migrate_sense_dataset.py`는 그대로 `rt-sense/4`를 만든다. `rt-sense/5`가 필요하면 그 뒤에 `compact_sense_dataset.py`를 실행한다.

## 3. 검증

| 확인 | 결과 |
| --- | --- |
| 변환 도구 안의 행 비교 | 관련어 592,793행이 모두 같다 |
| 사전 전체 조회 | `verify_sense_migration.py`로 출발 판이 분류하는 176,351단어를 CLI 기본값으로 한 단어씩 두 판에서 조회했다. 텍스트와 JSON 응답이 모두 같다 |
| 평가 세트 224단어 | 요약과 단어별 결과가 출발 판과 모두 같다 |
| 수정 도구 | 같은 수정 계획(관련어 옮기기·떼어내기, 주제 이름, 단어 옮기기, 주제 합치기, persona 합치기 2건)과 연결 목록을 두 판에 적용했다. 결과의 모든 테이블이 같다 |
| 시트 생성 | 시험 200단어의 시트가 같다. 시간은 3.2초에서 12.5초로 늘었다 |
| 적재 시간 | 1.67초 → 1.70초 |
| 크기 | 48,615,424바이트 → 31,715,328바이트(−34.8%). SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced` |
| git | 압축 크기 18.6MB → 13.3MB. 처음 넣는 5판은 13.3MB가 쌓인다. 수정 도구로 만든 다음 5판은 시험에서 87KB 차이로 저장됐다 |
| 파일 상태 | `PRAGMA quick_check` 통과, 빈 페이지 0개 |

## 4. 영향

- 관련어 단어나 주제로 찾는 도구는 인덱스 없이 테이블을 훑는다. `prepare_mapping_sheets.py`를 3단계 대상 전체(5,947단어)에 돌리면 몇 분이 걸릴 것으로 본다. `revise_topics.py`는 `input_detaches` 항목을 검사할 때마다 관련어 테이블을 한 번 훑는다(약 25ms).
- 수정 도구가 만든 5판은 인덱스와 테이블을 다시 만들지 않는다.
- `rt-sense/3`용 도구(`correct_sense_dataset.py`, `add-entries`)와 검증 스크립트 4개는 바뀌지 않았다.

## 5. 파일

| 위치 | 내용 |
| --- | --- |
| `datasets/rts5-75e02862daa29308/` | `dataset.sqlite3`, `build-report.json`(크기, 관련어 수, 표기를 따로 저장한 관련어 수, 종류 목록) |
| `sense-eval/results/rts5-75e02862daa29308.json` | 평가 세트 채점 결과 |

분석에 쓴 사본과 측정 스크립트, 수정 도구 비교 결과는 저장소 밖 작업 폴더에 있고 저장소에 넣지 않았다.

## 6. 기본값 전환

- `sense_lookup.py`의 `DEFAULT_SENSE_DATASET`을 `rts5-75e02862daa29308`로 바꿨다. 이 판의 `dataset.sqlite3`(31.7MB)를 저장소에 넣었다.
- 이전 기본값 `rts4-301d57d419380d08`의 `dataset.sqlite3`는 이미 저장소 기록에 있으므로 그대로 두었다.
- 바꾼 뒤 `verify_input_bindings.py`, `verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_spelling_aliases.py`가 모두 통과했다.
- `README.sense.ko.md`의 예시(`unshare` 관련어 24개, `'abort signal' idx` 988개, `activate`의 `ReLU`)는 그대로다. `unshare`의 텍스트 출력은 앞 판과 글자까지 같다. 판 설명, 파일 표, 판 표를 고쳤다.
- [데이터셋 생성 과정](../pipeline.md)에 19단계와 `rt-sense/5` 설명을 더했다. [알려진 결함](../known-defects.md)과 [단어 평가](../word-evaluation.md)의 현재 기본값 표기도 고쳤다.
