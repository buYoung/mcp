# 유의어 PoC 데이터셋 계약 (`rt-dataset/1`, 정규화 `rtn-2`)

이 문서는 실험 내부 계약이다. 디렉터리 이름, 필드 이름, 파일 이름은 제품 API·MCP 도구·설정 키가 아니다. 정규 데이터는 JSONL이며 SQLite는 JSONL에서만 다시 생성한다. 기계 검증용 레코드 스키마는 [dataset.schema.json](dataset.schema.json), SQLite 스키마는 [../schema.sql](../schema.sql)이다.

## 1. 산출물과 식별자

| 항목 | 규칙 |
| --- | --- |
| 위치 | `.codemap/poc/related-terms/datasets/<dataset_id>/` (ignored) |
| 정규 파일 | `sources.jsonl`, `concepts.jsonl`, `expressions.jsonl`, `lexicalizations.jsonl`, `relations.jsonl` |
| 생성 파일 | `dataset.sqlite3` (journal_mode=DELETE, WAL·journal 동반 파일 없음) |
| 동반 파일 | `manifest.json`, `ATTRIBUTION.md`, `LICENSES/` (원문 라이선스 사본) |
| 줄 형식 | 키 정렬, 공백 없는 UTF-8 JSON 한 줄. 파일마다 기본 키 오름차순, 중복 없음 |
| `dataset_id` | `rtd1-` + SHA-256(`rt-dataset/1\nrtn-2\n` + 각 JSONL의 `이름\tSHA-256\n`)의 앞 16자 |

`dataset_id`는 정규 JSONL 내용, 스키마 버전, 정규화 버전으로만 정해진다. 절대 경로, 빌드 시각, 실행 환경은 들어가지 않는다. 실행 환경은 `results/dataset-stats.json`의 `environment`에 따로 기록한다.

SQLite 파일 SHA-256은 산출물 무결성 확인용이다. 같은 JSONL에서 만든 SQLite의 동일성은 **논리 내용 해시**(`logical_sqlite_sha256`: 모든 테이블을 기본 키 순서로 정규 JSON으로 되돌린 값의 해시)로 판단한다. 같은 SQLite 버전·플랫폼에서 파일 바이트가 같게 나온 관측은 있지만, 다른 SQLite 버전·플랫폼의 바이트 동일성은 보장하지 않는다.

## 2. 레코드 종류

| 종류 | 기본 키 | 의미 |
| --- | --- | --- |
| `source` | `source_id` | 원본 스냅샷. 라이선스(SPDX), 저작자 표시 문구, 고정 revision, 아카이브 SHA-256 |
| `concept` | `concept_id` | 의미 단위. WordNet synset, GitHub topic, Wikidata item, 약어 항목 하나 |
| `expression` | `expression_id` | 표현 정체성. `x:` + `exact_key`. 원래 철자 목록(`surfaces`)을 함께 보존 |
| `lexicalization` | `lexicalization_id` | 표현이 특정 개념을 나타낸다는 원본 기록. 원래 철자, 역할, 원본 레코드 위치, 순위 |
| `relation` | `relation_id` | 서빙 관계. 방향, 종류, 근거 종류, 양 끝의 표현·lexicalization·개념, 출처 |

### 2.1 식별자 네임스페이스

ID는 원본 스냅샷으로 네임스페이스를 붙인다. 같은 철자라도 의미가 다르면 개념이 다르다.

| 출처 | `concept_id` | `lexicalization_id` |
| --- | --- | --- |
| WordNet | `oewn-2025:synset:<synset-id>` | `oewn-2025:sense:<sense-key>` |
| GitHub Explore | `github-explore-c8d7bc5:topic:<slug>`, 페이지 없는 related slug는 `…:topic-ref:<slug>` | `<concept>#slug`, `#display_name`, `#alias:<i>` |
| Wikidata | `wikidata-sel1:<QID>@r<lastrevid>` | `<concept>#label`, `#alias:<i>` |
| abbrcode | `abbrcode-bd33b1c:entry:<word>\|<abbr>` | `<concept>#abbreviation`, `#expansion` |
| SE 약어 | `se-abbr-8e1d77e:entry:<dictionary path>` | `<concept>#abbreviation`, `#expansion` |

`relation_id`는 `r:` + SHA-256(정규 JSON `[kind, evidence_kind, from_lexicalization_id, to_lexicalization_id]`)의 앞 24자다.

## 3. 정규화 `rtn-2`

| 키 | 정의 | 용도 |
| --- | --- | --- |
| `surface` | 원본 문자열에서 앞뒤 공백만 제거 | 표시·근거 보존 |
| `exact_key` | 소문자화(Unicode lowercase) → 공백 연속을 공백 하나로 → 앞뒤 공백 제거. 구두점 보존 | 표현 정체성, 정확 일치 조회 |
| `token_key` | 영숫자(Unicode alphanumeric)가 아닌 문자에서 분리 → 각 조각에 `split_identifier`(camel·숫자 경계) 적용 → 소문자 토큰을 공백 하나로 연결 | 정규화 일치 조회 |
| `token_key_lossy` | 공백, `_ - . / :` 이외의 비영숫자(`+ # ' , & ( )` 등)를 버렸거나 토큰이 없으면 참 | 참이면 정규화 일치에 쓰지 않음 (`c++`가 `c`와 섞이지 않게) |
| ASCII가 아닌 표현 | `token_key`는 빈 문자열, `token_key_lossy`는 참 | 정확 일치만 허용 |

- 데이터셋 쪽 `token_key`는 `exact_key`(소문자)에서 계산한다. 따라서 데이터셋 표현의 camelCase는 나누지 않는다. 질의 쪽은 원래 대소문자로 `split_identifier`를 적용하므로 `eventLoop`, `event_loop`, `event-loop`가 `event loop`와 정규화 일치한다.
- ASCII가 아닌 표현을 정확 일치로 제한한 이유: Python `str.isalnum()`은 Unicode Other_Alphabetic 결합 부호(예: 벵골 문자 모음 부호)를 영숫자로 보지 않지만 Rust `char::is_alphanumeric()`은 영숫자로 본다. 표준 라이브러리만으로 두 구현을 일치시킬 수 없어 `rtn-1` 빌드(`rtd1-081f6f35d765db78`)에서 1개 표현의 키가 달랐다. 그 빌드는 [results/history/rtd1-081f6f35d765db78](../results/history/rtd1-081f6f35d765db78/INVALIDATED.md)에 무효 기록으로 남겼다. `café`처럼 악센트가 있는 단어도 정확 일치만 된다.
- Unicode 호환 정규화(NFKC), 형태소 분석(복수형·활용형), 영미 철자 통일은 하지 않는다. `deleting`이 `delete`로 바뀌지 않는다.
- `split_identifier`는 제품 `codemap_search::parser::split_identifier`의 동작을 따른다. Python 포트와 Rust 함수의 일치는 child 02의 `normalize-check`로 모든 표현에 대해 확인한다.
- 저장소 통계 쪽 토큰(Tantivy 기본 analyzer: 비영숫자 분리, 40바이트 이상 토큰 제거, 소문자화)은 데이터셋에 저장하지 않는다. 데이터셋은 저장소·analyzer와 무관하게 공유 가능해야 하기 때문이다.

## 4. 관계 의미

| `relation_kind` | 근거 (`evidence_kind`) | 범위 | 방향 | 해석 |
| --- | --- | --- | --- | --- |
| `synonym` | `wordnet_same_synset` | 같은 synset 안 | `symmetric_pair` | 그 synset의 sense에서 같은 의미. 다른 sense로 일반화하지 않음 |
| `alias` | `github_topic_same_topic`, `wikidata_same_item` | 같은 topic·item 안 | `symmetric_pair` | 같은 개념의 다른 이름. 동의어 적중으로 집계하지 않음 |
| `abbreviation_expansion` | `abbrcode_entry`, `se_abbr_entry` | 같은 약어 항목 안 | `declared` | 약어 → 전체 표기 |
| `abbreviation_of` | 위와 같음 | 같은 약어 항목 안 | `declared` | 전체 표기 → 약어 |
| `similar` | `wordnet_similar_synset` | synset 간 | `declared` | 형용사 유사 관계. 근접어이지 동의어가 아님 |
| `antonym` | `wordnet_sense_antonym` | sense 간 | `declared` | 반의어. 서빙 후보에서 제외하는 것은 조회 정책의 결정 |
| `related_topic` | `github_topic_related` | topic 간 | `declared` | 관련 주제. 동의어·별칭이 아님 |

불변식(빌더 `verify`가 검사):

1. `synonym`, `alias`, 약어 관계는 양 끝의 `concept_id`가 같다. 다의 표현을 거친 전이 확장(A의 sense 1 → B, B의 sense 2 → C)은 데이터에 존재하지 않는다.
2. 관계 종류마다 허용된 `evidence_kind`만 쓴다. 공기(co-occurrence)나 저장소 출현은 어떤 관계의 근거도 아니며, 이 버전에는 그런 근거 종류가 없다.
3. 같은 `expression_id`끼리는 관계를 만들지 않는다(대소문자만 다른 철자). 건너뛴 쌍은 근거 종류별로 센다.
4. `token_key`가 같은 서로 다른 표현 사이의 관계(예: 하이픈·공백 차이, `serialise`/`serialize`처럼 원본이 같은 synset에 넣은 철자 변형)는 남기되 별도 집계한다. 평가에서 동의어 적중과 구분해야 한다.
5. 여러 출처·개념이 같은 표현 쌍을 지지하면 관계 레코드가 여러 개다. 출처 수를 저장소 지지 수나 확률로 해석하지 않는다. 출처 점수·신뢰도 필드는 없다.
6. 공개 코드 말뭉치 지지(corpus support)는 측정하지 않았다. 해당 필드를 만들거나 임의 값으로 채우지 않는다.

## 5. 원본별 선택 정책

이 정책은 child 03 평가 전에 고정했다. 평가 결과를 보고 바꾸려면 새 `dataset_id`로 다시 빌드하고 이전 평가를 무효로 표시해야 한다.

| 출처 | 포함 | 제외와 이유 |
| --- | --- | --- |
| Open English WordNet 2025 core JSON | 모든 synset·sense(도메인 태그 유무와 무관), 첫 정의문, `domain_topic` 라벨, `similar`, sense `antonym` | `form` 변형(4,473개, 형태 변화라 동의어 근거 아님), 예문, hypernym·derivation 등 다른 관계(유의어 근거 아님) |
| GitHub Explore 커밋 `c8d7bc5` | 모든 `topics/*/index.md`(1,278개)의 slug, `display_name`, `aliases`, `related`, `short_description`. 별칭이 없는 topic도 단일 표현 개념으로 기록 | logo·created_by 등 비어휘 필드. topic 본문 |
| Wikidata (선택 1) | W1: Explore `wikipedia_url`(영문 문서, 섹션 링크 제외)과 enwiki sitelink가 같은 항목. Wikipedia 리다이렉트 제목으로 해석한 경우 포함, 섹션 리다이렉트는 제외. W2: 작성 단계 진단 항목 3개(Q1127410, Q1349935, Q1376168) | 영어 이외 라벨, 설명 이외 문장, Wikipedia 본문(CC BY-SA). 해석되지 않은 링크 16개 |
| abbrcode 커밋 `bd33b1c` | `data/abbrs/.json`의 모든 word·abbr 쌍과 `degree`, `context` | 번역 데이터 |
| SE 약어 커밋 `8e1d77e` | `dictionary/**/*.md` 전체. 약어는 파일 이름, 전체 표기는 제목 줄 | 제목 약어가 파일 이름과 다른 3건은 파일 이름을 쓰고 `title_abbreviation_conflict`로 표시 |

W2 진단 항목은 작성 단계에서 이미 조사한 사례다. 이 항목에서 만든 평가 입력은 독립 표본이 아니라 대조군(control)으로 표시해야 한다.

## 6. SQLite 생성물

- 테이블은 정규 레코드 종류와 1:1이다. 중첩 객체(`revision`, `attributes`, `domain_labels`, `surfaces`)는 정규 JSON 텍스트 열로 저장한다.
- `dataset_meta`에는 `dataset_id`, `schema_version`, `normalization_version`, JSONL SHA-256, 레코드 수를 둔다. 판독기는 이 값을 확인한 뒤 조회한다.
- 조회 색인: `relations_by_from_expression(from_expression_id, relation_kind)`, `expressions_by_token_key(token_key) WHERE token_key_lossy = 0`, `lexicalizations_by_expression`, `lexicalizations_by_concept`. `expressions.exact_key`는 UNIQUE 제약의 자동 색인을 쓴다.
- SQLite 생성은 임시 파일 쓰기 → 외래 키 검사·`VACUUM` → 닫기 → 이름 변경 순서다. 전체 `build` 명령은 별도 임시 디렉터리에서 JSONL·SQLite를 모두 생성하고 전체 검증을 통과한 뒤 새 데이터셋 디렉터리를 공개한다. 같은 ID가 이미 있으면 JSONL·논리 내용의 일치를 확인하고 기존 디렉터리와 SQLite를 유지한다. 실패 시 기존 파일을 삭제하지 않는다.
- 판독기는 `mode=ro`로 열며 파일을 만들거나 바꾸지 않는다. 내용이 바뀔 수 있는 파일에 `immutable=1`을 쓰지 않는다. 일반 조회와 평가 실행은 기존 산출물만 사용하며, 빌드는 명시적인 준비 명령으로 분리한다.

## 7. 재현과 검증

```bash
cd apps/codemap-search/experiments/related-terms-poc
python3 acquire_sources.py verify          # 아카이브 해시 대조
python3 build_dataset.py build             # JSONL → SQLite → verify → results/dataset-stats.json
python3 build_dataset.py verify <dataset-dir>
python3 build_dataset.py sqlite <dataset-dir>   # JSONL만으로 SQLite 재생성
```

`verify`는 다음을 모두 통과해야 한다: JSONL 해시·`dataset_id` 일치, 정규 JSON 줄 형식, 기본 키 정렬·유일성, 레코드 스키마, SQLite `integrity_check`·외래 키·journal 모드·필수 색인·관계 조회의 색인 사용, SQLite 행 → JSONL 줄 완전 일치, 끊긴 식별자 0, 관계 불변식 위반 0, 읽기 전용 열기 후 파일 목록 불변.
