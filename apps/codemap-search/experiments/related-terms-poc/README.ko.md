# 유의어 데이터셋과 로컬 조회 PoC (1단계)

이 디렉터리는 1단계 실험만 다룬다. 출처를 보존한 유의어 데이터셋을 만들 수 있는지, 그리고 실제 로컬 조회가 쓸 만한 유의어를 돌려주는지 확인한다. 문맥 기반 LLM 판정(2단계)과 제품 통합 실험(3단계)은 하지 않았다. 디렉터리·파일·필드 이름은 내부 실험용이며 제품 API, MCP 도구, 설정 키가 아니다.

- 데이터셋 구축: 이 문서 [1장](#1-데이터셋)
- **이미 만들어진 데이터로 바로 조회:** [2.5절 명령](#25-조회-명령과-호환성-동작). 데이터셋 빌드·다운로드·재인덱싱 없이 단어와 구를 넣어 실행한다.
- 인덱싱 시점 저장소 통계와 로컬 조회: [2장](#2-인덱싱-시점-통계와-로컬-조회)
- 실제 반환 평가와 후속 단계 조건: [3장](#3-평가-결과와-후속-단계-조건)

원본 아카이브, 정규 JSONL, SQLite, 저장소 인덱스·어휘 통계는 모두 ignored 경로인 `.codemap/poc/related-terms/` 아래에 있다. 추적 대상은 스크립트, 계약, 고정 manifest, 저작자 표시, 요약 결과, 인계 파일이다.

## 1. 데이터셋

### 1.1 원본과 고정 revision

| 출처 | 고정 | 라이선스 | 들여온 것 |
| --- | --- | --- | --- |
| [Open English WordNet](https://github.com/globalwordnet/english-wordnet) | 릴리스 `2025-edition`, 커밋 `dc343f26`, JSON ZIP SHA-256 `7d749f6e…4b51` | CC BY 4.0 (+ Princeton WordNet 고지) | synset 멤버·sense key, 품사, 첫 정의문, domain 라벨, `similar`, sense 반의어 |
| [GitHub Explore](https://github.com/github/explore) | 커밋 `c8d7bc51`, tree `caf8332d` | CC BY 4.0 | topic slug, `display_name`, `aliases`, `related`, `short_description` |
| [Wikidata](https://www.wikidata.org) | 항목마다 `lastrevid` (870개) | CC0 1.0 (구조화 데이터) | 영어 라벨·별칭·설명 |
| [abbreviations-in-code](https://github.com/abbrcode/abbreviations-in-code) | 커밋 `bd33b1c7` | MIT | word·abbr·degree·context |
| [software-engineering-abbreviations-acronyms](https://github.com/wuyichen24/software-engineering-abbreviations-acronyms) | 커밋 `8e1d77e9` | CC0 1.0 | 약어, 전체 표기, 첫 개요 문장 |

정확한 아카이브 경로·SHA-256·revision은 [sources.lock.json](sources.lock.json)에 있다. git 원본은 고정 커밋의 blob만 읽어 메타데이터를 고정한 tar로 묶었고, 같은 입력에서 두 번 묶어 같은 해시를 확인했다. Wikidata는 변하는 URL과 조회 날짜가 아니라 항목별 revision으로 고정했다. `Special:EntityData/{QID}.json?revision={lastrevid}`로 진단 항목 3개와 시드 표본 20개를 다시 읽어 23개 모두 라벨·별칭·설명이 같음을 확인했다.

ConceptNet·Stack Overflow 태그 동의어(CC BY-SA), SCANL(라이선스 없음), SEthesaurus·SWordNet(라이선스 미확인), Moby Thesaurus, Wikipedia 본문은 쓰지 않았다. Wikipedia는 Explore 링크의 리다이렉트 제목을 Wikidata 항목으로 해석하는 데만 썼다. 공개 코드 말뭉치의 공기(co-occurrence)는 채굴하지 않았으므로 데이터셋에 말뭉치 지지 수가 없다.

### 1.2 데이터 계약

레코드 종류, ID 규칙, 정규화 `rtn-2`, 관계 의미, 선택 정책은 [contracts/dataset.md](contracts/dataset.md)에 있다. 핵심은 다음과 같다.

- 표현(expression)과 의미(concept)를 분리한다. 같은 철자라도 sense가 다르면 다른 개념이다.
- `synonym`은 같은 WordNet synset 멤버만, `alias`는 같은 GitHub topic·Wikidata 항목의 이름만, 약어 관계는 같은 약어 항목만 근거로 삼는다. 이 세 종류는 양 끝이 항상 같은 개념이므로 다의어를 거친 전이 확장이 데이터에 없다.
- `similar`(WordNet 유사 형용사), `antonym`, `related_topic`(GitHub related)은 별도 종류로 남긴다. 동의어로 바꿔 부르지 않는다.
- 출처 점수·신뢰도·말뭉치 지지 필드가 없다. 여러 출처가 같은 쌍을 지지해도 관계 레코드가 여러 개일 뿐이다.

### 1.3 재현 명령

원본 다운로드는 다음 명령으로 수행했다. `acquire_sources.py`는 네트워크를 쓰지 않는다.

```bash
# 저장소 루트에서
RAW=.codemap/poc/related-terms/sources/raw
UA="codemap-search-related-terms-poc/0.1 (https://github.com/buYoung/mcp; offline dataset research)"
mkdir -p "$RAW/wordnet"
curl -sSL -o "$RAW/wordnet/english-wordnet-2025-json.zip" \
  https://github.com/globalwordnet/english-wordnet/releases/download/2025-edition/english-wordnet-2025-json.zip
curl -sSL -o "$RAW/wordnet/LICENSE.md" https://raw.githubusercontent.com/globalwordnet/english-wordnet/2025-edition/LICENSE.md
curl -sSL -o "$RAW/wordnet/WNDB_License.txt" https://raw.githubusercontent.com/globalwordnet/english-wordnet/2025-edition/WNDB_License.txt
git clone -q --filter=blob:none --no-checkout https://github.com/github/explore.git "$RAW/explore"
git -C "$RAW/explore" sparse-checkout init --no-cone
git -C "$RAW/explore" sparse-checkout set --no-cone '/topics/*/index.md' '/LICENSE.txt' '/notices.md'
git -C "$RAW/explore" checkout -q c8d7bc516928a4f17845b767884a80cc53affd57
git clone -q https://github.com/abbrcode/abbreviations-in-code.git "$RAW/abbrcode"
git -C "$RAW/abbrcode" checkout -q bd33b1c7bbfea159275c88d223482f5da9a01e58
git clone -q https://github.com/wuyichen24/software-engineering-abbreviations-acronyms.git "$RAW/se-abbr"
git -C "$RAW/se-abbr" checkout -q 8e1d77e99b184106d50a180ba52cb3557273af6d

# Wikidata: 요청 URL을 만든 뒤 curl로 받고, 응답 해시를 기록한다.
cd apps/codemap-search/experiments/related-terms-poc
W=../../../../.codemap/poc/related-terms/sources/raw/wikidata
fetch() { (cd "$W" && while read -r file url; do curl -sS --fail -A "$UA" -o "$file" "$url"; sleep 1; done < "$1"); }
python3 acquire_sources.py plan-wikidata && fetch urls.txt && python3 acquire_sources.py record-wikidata
python3 acquire_sources.py plan-wikidata-redirects && fetch redirect-urls.txt && python3 acquire_sources.py record-wikidata
python3 acquire_sources.py plan-wikidata-resolved && fetch resolved-urls.txt && python3 acquire_sources.py record-wikidata
python3 acquire_sources.py pack      # 결정적 tar + sources.lock.json
python3 acquire_sources.py verify    # 아카이브 해시 대조
```

`plan-wikidata`를 나중에 다시 실행하면 그 시점의 최신 revision을 받는다. 이 PoC와 같은 데이터를 재현하려면 lock에 고정된 `wikidata-sel1.tar`(SHA-256 대조)를 써야 한다. revision 경로 감사는 `plan-revision-audit` → `audit-urls.txt`를 curl로 받기 → `compare-revision-audit` 순서다.

데이터셋 빌드와 검증은 오프라인이다. 아래 빌드 명령은 **최초 준비 또는 명시적 재생성 때만** 실행한다. 일반 조회와 `evaluate.py run`은 인계 파일이 가리키는 기존 SQLite를 재사용하며, 파일이 없거나 해시가 다르면 중단한다. 자동 빌드·다운로드로 대체하지 않는다.

```bash
cd apps/codemap-search/experiments/related-terms-poc
python3 build_dataset.py build                    # 아카이브 → JSONL → SQLite → verify → results/dataset-stats.json
python3 build_dataset.py sqlite <dataset-dir>     # JSONL만으로 SQLite 재생성
python3 build_dataset.py verify <dataset-dir>
python3 build_dataset.py handoff <dataset-dir>    # results/dataset-handoff.json
```

`build`는 임시 디렉터리에서 JSONL·SQLite를 모두 생성하고 검증한 뒤 새 데이터셋을 공개한다. 같은 ID의 데이터셋이 있으면 정규 JSONL과 논리 내용의 일치 여부를 검사하고 기존 디렉터리·SQLite를 그대로 유지한다. 생성이나 검증이 실패해도 기존 파일을 삭제하지 않는다. 이 보호 동작은 임시 소형 데이터셋에서 실패를 주입해 확인했다([검토 수정 검증](results/review-fixes-verification.json)).

### 1.4 산출물과 무결성

| 항목 | 값 |
| --- | --- |
| `dataset_id` | `rtd1-8efd9676107a983d` |
| 위치 | `.codemap/poc/related-terms/datasets/rtd1-8efd9676107a983d/` |
| 정규 JSONL | 5개 파일, 약 314 MB |
| SQLite | `dataset.sqlite3`, 227,815,424 바이트, SHA-256 `a38be13d…1d10` |
| 논리 내용 해시 | `6bdb0995…ae84` |
| 빌드 | Python 3.14.8, SQLite 3.53.4, macOS 15.7.1 arm64. 39초, 최대 RSS 약 4.0 GB |

같은 고정 입력으로 두 번 빌드해 JSONL 5개, SQLite 논리 내용, SQLite 파일 바이트가 모두 같았다. 첫 빌드 `rtd1-081f6f35d765db78`(정규화 `rtn-1`)은 child 02의 정규화 일치 검사에서 아삼어 별칭 1개의 키가 Python·Rust 사이에 달라 무효화했다. 원인과 기록은 [results/history/rtd1-081f6f35d765db78](results/history/rtd1-081f6f35d765db78/INVALIDATED.md)에 있다. 파일 바이트 동일성은 이 한 환경의 관측이며 다른 SQLite 버전·플랫폼에 대한 주장이 아니다. `verify`는 JSONL 형식·스키마, SQLite 무결성·외래 키·필수 색인·조회 색인 사용, SQLite 행과 JSONL 줄의 완전 일치, 끊긴 식별자 0, 관계 불변식 위반 0, 읽기 전용 열기 후 파일 불변을 확인했다. 결과는 [results/dataset-stats.json](results/dataset-stats.json)과 [results/dataset-handoff.json](results/dataset-handoff.json)에 있다.

### 1.5 원본 개수와 서빙 개수

원본 개수는 자료의 크기일 뿐 코드 도메인 적용 범위가 아니다.

| 원본 | 원본 개수 |
| --- | --- |
| WordNet | synset 107,519 (멤버 2개 이상 44,961), 멤버십 185,129, synset 내부 방향 쌍 285,546, similar 링크 23,176, sense 반의어 링크 7,988, domain 태그 synset 6,212 |
| GitHub Explore | topic 페이지 1,278 (별칭 있음 847), 선언 별칭 2,110, related 링크 3,386 (페이지 없는 slug 1,387), `wikipedia_url` 927 |
| Wikidata | 요청 제목 884 (URL 제외 25), 리다이렉트 해석 76, 선택 항목 870 (영어 별칭 있음 606), 영어 별칭 1,720, 해석 실패 링크 16 |
| abbrcode | 단어 280, 단어·약어 쌍 306, 다의 약어 16 |
| SE 약어 | 항목 358, 제목·파일명 약어 불일치 3, 다의 약어 9 |

| 서빙 관계 | 개수 |
| --- | ---: |
| `synonym` (WordNet) | 285,450 |
| `similar` (WordNet) | 59,902 |
| `antonym` (WordNet) | 7,986 |
| `alias` (GitHub Explore / Wikidata) | 27,084 / 9,694 |
| `related_topic` (GitHub Explore) | 19,015 |
| `abbreviation_expansion` (abbrcode / SE 약어) | 306 / 358 |
| `abbreviation_of` (abbrcode / SE 약어) | 306 / 358 |

레코드 수는 source 5, concept 111,718, expression 133,942, lexicalization 195,052, relation 410,459이다. 여러 토큰 표현 62,595개, `token_key`가 손실되거나 정의되지 않는 표현 1,399개(ASCII가 아닌 표현 포함), 둘 이상의 개념에 속한 표현 27,047개, 나가는 관계가 없는 표현 31,868개, 표현이 하나뿐인 개념 64,189개다. `token_key`가 같은 표현 사이의 관계(하이픈·공백 차이, WordNet 철자 변형)는 synonym 494, alias 852, related_topic 3개이며 동의어 적중과 따로 다뤄야 한다. 같은 표현끼리라서 만들지 않은 쌍과 제외 이유는 통계 파일의 `skipped_same_expression_pairs`, `exclusions`에 있다.

### 1.6 확인된 원본 공백

작성 단계에서 조사한 사례를 빌드 결과에서 다시 확인했다. 이 사례들은 진단용 대조군이며 독립 평가 표본이 아니다.

- `delete` → `erase`는 WordNet의 "디지털·자기 기록 정보를 지움" sense(synset `01001860-v`)에서 나온다. 이 sense의 domain 라벨은 "computer science"가 아니라 "recording"이다. 컴퓨팅 domain 태그가 붙은 synset은 수백 개뿐이어서 domain 태그로 sense를 거르지 않았다.
- `retry`의 유일한 WordNet sense는 법정 재심리(`rehear`)다. 코드의 "다시 시도"와 다른 의미이므로 반환되면 오의미(wrong sense)다.
- `deserialize`, `enqueue`, `dequeue`, `memoize`는 어느 원본에도 표현이 없다. `memoization`은 Wikidata 항목(Q1376168)으로만 있고 별칭이 없어 관계가 0개다.
- `serialize`는 Wikidata 별칭(serialization, data serialization 등)과 WordNet의 "순서대로 배열" sense(`serialise`)가 함께 나온다. 같은 표현에서 서로 다른 개념의 관계가 섞일 수 있으므로 sense·출처를 함께 보여야 한다.
- `res`, `dir`처럼 약어가 여러 전체 표기(response/result, directory/direction)를 가지며, `res`는 WordNet의 `reticuloendothelial system`과도 연결된다.
- Wikidata·GitHub 별칭에는 `container orchestrator`(k8s)처럼 범주에 가까운 이름, `advent-of-code-2015` 같은 연도별 태그가 섞여 있다. 별칭은 동의어로 집계하지 않는다.

### 1.7 저작자 표시

[ATTRIBUTION.md](ATTRIBUTION.md)가 출처별 저작자 표시·라이선스·변경 사항을 담고, 빌드할 때마다 데이터셋 디렉터리에 복사된다. 원문 라이선스 파일은 데이터셋의 `LICENSES/`에 함께 들어간다. 이 PoC는 데이터셋을 게시하거나 GitHub Release로 배포하지 않았다.

## 2. 인덱싱 시점 통계와 로컬 조회

### 2.1 구성

독립 crate [Cargo.toml](Cargo.toml)(`publish = false`, 자체 `[workspace]`)이 제품 라이브러리를 path 의존성으로 쓴다. 제품 소스·Cargo 파일은 바꾸지 않았다. [Cargo.lock](Cargo.lock)은 메인 lock에서 시작했고, 두 lock의 공통 `(name, version)` 항목 430개의 출처·체크섬이 같다(Tantivy 0.26.2, rusqlite 0.40.2 bundled).

| 모듈 | 책임 | 서빙 경로 |
| --- | --- | --- |
| [src/dataset.rs](src/dataset.rs) | 데이터셋 SQLite를 읽기 전용으로 열고 스키마·정규화 버전을 확인한 뒤 색인 조회 | 예 |
| [src/normalize.rs](src/normalize.rs) | 질의 정규화 `rtn-2`, 인덱스 analyzer와 같은 토큰화 | 예 |
| [src/stats.rs](src/stats.rs) | 공개된 통계와 준비 상태 판정 | 예 |
| [src/lookup.rs](src/lookup.rs) | 단계별 후보 결정과 출력 envelope | 예 |
| [src/vocabulary.rs](src/vocabulary.rs) | 커밋된 세대의 postings 열거, 통계 저장·공개, 독립 재계수 | 아니오 |

### 2.2 인덱싱 시점 통계

`rtpoc index`는 작업공간 cwd에서 제품 `TantivySearchEngine::index_files_changed(["."])`로 비공개 인덱스를 갱신해 커밋한다. 그 직후 같은 커밋 세대의 세그먼트를 열고 검색 대상 TEXT 필드 7개(`file_path_parts`, `symbol`, `docstring`, `literal`, `definition_body`, `reference`, `format_text`)의 모든 term·postings를 열거한다. 삭제된 문서는 건너뛰고, 단어마다 문서 ID를 필드 간 합집합으로 모아 **고유 live 파일 수**를 센다. 분모는 같은 세대의 live 문서 수이며, live 문서마다 `file_path`가 정확히 하나인지 확인한다.

1. `meta.json`의 SHA-256을 세대 ID로 삼는다. 열거 전후에 같은 값이어야 하고, reader의 세그먼트 목록이 그 `meta.json`과 같아야 한다.
2. 통계를 `stats/stats-<세대>.sqlite3`에 임시 파일로 쓴 뒤 이름을 바꾼다.
3. 마지막으로 `published.json`을 임시 파일 + rename으로 교체한다. 이 파일이 바뀌기 전까지 조회는 이전 공개본을 본다.

통계 정책: 형식 `rt-stats/1`, 필드 `rts-fields-1`, analyzer `tantivy-0.26.2/default:simple+remove_long(40)+lowercase`. 데이터셋 후보 목록과 무관하게 인덱스의 모든 단어를 저장하므로, 호환되는 데이터셋을 바꿔도 저장소를 다시 파싱할 필요가 없다. 정책 문자열이 다르면 `incompatible`로 보고한다.

### 2.3 대상 작업공간

[workspaces.json](workspaces.json)이 대상과 격리 방식을 고정한다. 각 체크아웃은 자기 cwd에서, 비어 있는 비공개 `CODEMAP_HOME`으로, 저장소 `.codemap/config.toml` 없이(제품 기본값) 인덱싱했다. 보호용 ignore 규칙을 끄지 않았다.

| 대상 | 고정 커밋 | live 파일 | 어휘 | 주 언어 파일 인덱싱 | 인덱싱 | 어휘 생성 | 통계 저장 | 통계 파일 | 최대 RSS |
| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| flask 3.1.3 | `22d92470` | 121 | 2,999 | `.py` 83/83 | 378 ms | 6.2 ms | 2.8 ms | 60 KB | 86 MB |
| gin v1.12.0 | `73726dc6` | 109 | 4,533 | `.go` 98/98 | 544 ms | 7.1 ms | 2.7 ms | 100 KB | 106 MB |
| tokio 1.53.2 | `ff0c4068` | 820 | 7,046 | `.rs` 790/790 | 2,312 ms | 15.8 ms | 4.0 ms | 124 KB | 95 MB |
| express v5.2.1 | `dbac741a` | 161 | 2,112 | `.js` 142/142 | 631 ms | 4.3 ms | 2.5 ms | 48 KB | 84 MB |

release 빌드, 단일 측정이다. 네 대상 모두 git 추적 파일이 아닌 파일이 인덱스에 들어가지 않았다. `verify-stats`는 공개된 모든 단어를 query engine의 `Count`(삭제 반영)로 다시 세어 불일치 0, 누락 live term 0, 분모 일치를 확인했다. 필드별 Tantivy `doc_freq`를 더한 값이 실제 고유 파일 수보다 큰 단어가 대상별로 1,745~4,404개였다. 필드별 합산은 정확한 파일 수가 아니다. 결과는 [results/index-stats.json](results/index-stats.json)에 있다.

### 2.4 수명주기와 오프라인 조회

flask 고정 체크아웃의 **복사본**에서 관찰했다([results/lifecycle-evidence.json](results/lifecycle-evidence.json)). 원본 체크아웃은 변경하지 않았다.

| 단계 | 관찰 |
| --- | --- |
| 변경 | 파일 A(docstring 2회 + literal 2회)와 B(literal 1회)에 탐침 단어를 넣자 2 파일로 집계됐다. 필드별 raw `doc_freq` 합은 4였다. |
| 삭제 | B 삭제 후 1 파일, 분모 121 → 120. |
| 재시작 | 새 프로세스가 같은 `published.json`을 읽어 같은 값을 반환했다. 인덱싱 명령 없이 준비 상태였다. |
| 공개 실패 | A에 새 단어를 넣고 공개 직전에 실패를 주입했다. 인덱스는 새 세대로 커밋됐지만 `published.json`은 그대로였고, 조회는 `stale`을 보고하며 새 단어를 포함한 어떤 수도 반환하지 않았다. |
| 복구 | 다시 인덱싱하자 같은 커밋 세대로 통계를 공개하고 준비 상태가 됐다. |
| 인코딩 실패 | A를 잘못된 UTF-8로 바꾸자 제품 엔진이 문서를 제외했고 A의 단어가 0, 분모 119가 됐다. |

L1~L6 동안 세그먼트에 삭제 문서 2개의 postings 3,274개가 남아 있었고, 집계는 그것을 건너뛰고도 감사를 통과했다. 파서 추출 실패와 동시 writer는 관찰하지 않았다.

오프라인 조회([results/offline-evidence.json](results/offline-evidence.json))는 macOS `sandbox-exec`로 네트워크를 막고, 체크아웃 전체와 인덱스 디렉터리(`meta.json` 제외) 읽기를 막은 상태에서 실행했다. 네 대상 모두 3회 반복 조회가 성공했고 비샌드박스 결과와 같았다. 같은 조건에서 `verify-stats`와 `index`는 실패해 차단이 실제로 작동함을 보였다. 조회 전후 데이터셋 SQLite 해시는 같았다.

### 2.5 조회 명령과 호환성 동작

**기존 데이터로 바로 조회** — 저장소 루트에서 다음을 실행한다. 현재 체크아웃에는 바이너리·데이터셋·네 대상의 통계가 준비되어 있다. 마지막의 `start "event loop" res`를 원하는 단어·구로 바꾸면 된다. 문장에서 키워드를 자동 추출하는 기능은 아니다.

```bash
cd apps/codemap-search/experiments/related-terms-poc
DATASET="../../../../$(jq -r '.sqlite_path' results/dataset-handoff.json)"
DATASET_ID="$(jq -r '.dataset_id' results/dataset-handoff.json)"
TARGET=tokio  # flask, gin, tokio, express 중 선택

./target/release/rtpoc lookup \
  --dataset "$DATASET" \
  --expect-dataset-id "$DATASET_ID" \
  --policy contracts/lookup-policy.json \
  --state "../../../../.codemap/poc/related-terms/workspaces/state/$TARGET" \
  start "event loop" res \
  | jq '{query: .input, status, candidates: [.candidates[] | select(.outcome == "final") | {term: .exact_key, kind: .primary_kind}]}'
```

`jq`는 결과를 보기 좋게 줄이는 용도다. 전체 근거·탈락 사유가 필요하면 `| jq ...` 부분을 빼면 된다. 실행 예에서 tokio의 `start`는 `begin` 등을, `event loop`는 별칭 `message loop`, `run loop`를, `res`는 약어 전개 `response`를 반환했다. `kind = synonym`은 원본 사전의 관계 종류일 뿐 **질의 문맥에서 옳다고 판정한 결과가 아니다**. 별칭·약어·관련어도 구분해서 표시된다.

저장소 필터 전의 사전 후보를 확인하려면 `--state`를 빼고 다음처럼 출력한다. 이때 `final_terms`는 비어 있으며 `not_evaluated` 후보가 사전 조회 결과다.

```bash
./target/release/rtpoc lookup \
  --dataset "$DATASET" --expect-dataset-id "$DATASET_ID" \
  --policy contracts/lookup-policy.json delete \
  | jq '{query: .input, repository: .repository.status, candidates: [.candidates[] | select(.outcome == "not_evaluated") | {term: .exact_key, kind: .primary_kind}]}'
```

아래는 **최초 준비나 명시적 인덱스·증거 갱신 때만** 쓰는 명령이다. 평소 질의를 실행할 때 반복하지 않는다. 체크아웃은 `workspaces.json`의 고정 revision과 경로로 먼저 준비해야 한다. 인덱스 또는 인계 파일을 바꾸면 기존 평가의 고정 조건이 맞지 않으므로 평가 프로토콜도 개정해야 한다.

```bash
# 위와 같은 실험 디렉터리에서
cargo build --release
python3 -B workspaces.py check && python3 -B workspaces.py index
python3 -B lifecycle.py run && python3 -B lifecycle.py offline
python3 -B lookup_handoff.py
```

출력은 입력마다 JSON 한 줄이며 계약은 [contracts/lookup.schema.json](contracts/lookup.schema.json)이다. 단계는 사전 조회 → 정규화 → 저장소 존재 → 흔함 → 중복 제거 → 한도이고, 탈락한 후보도 단계와 이유를 남긴다. 정책 `rtl-1`([contracts/lookup-policy.json](contracts/lookup-policy.json))의 값(흔함 상한 25%, 최종 10개, 반의어 제외, 정렬 순서)은 평가 전에 고정한 실험 가설이다.

| 상황 | 동작 |
| --- | --- |
| 데이터셋 파일 없음 | `dataset unavailable` 오류. 빈 DB를 만들지 않는다 |
| 스키마·정규화 버전 다름, 필수 테이블·색인 없음 | `dataset incompatible` 오류 |
| `--expect-dataset-id`와 다른 데이터셋 | 오류 |
| 통계를 공개한 적 없음 | `repository.status = missing`, 최종 후보 없음 |
| 통계 형식·필드·analyzer 정책 다름 | `incompatible`, 최종 후보 없음 |
| 인덱스가 공개 이후 새 세대로 커밋됨 | `stale`, 최종 후보 없음 (이전 수를 새 인덱스와 섞지 않음) |
| 통계 파일 해시·세대 불일치, 읽기 실패 | `unavailable`, 최종 후보 없음 |
| `--state` 생략 | `not_configured`, 사전 후보만 `not_evaluated`로 표시 |
| 준비 상태에서 후보가 모두 탈락 | `status = empty_after_filters` (빈 결과도 정상) |

서빙 경로(`lookup`, `stats-words`)는 데이터셋 SQLite, 정책 JSON, `published.json`, 통계 SQLite, 인덱스 `meta.json`만 읽는다. 모델 호출, 다운로드, 소스 탐색, postings 열거, 검색 결과 계수, 개수 재계산이 없다. 정적 감사 결과와 실제 Cargo 검사 기록은 [results/lookup-handoff.json](results/lookup-handoff.json)에 있다.

## 3. 평가 결과와 후속 단계 조건

### 3.1 무엇을 어떻게 쟀는가

입력 92개를 결과를 보기 전에 [cases.json](cases.json)으로 고정했다. 의도한 코드 의미(`intended_meaning`)와 기대어(`expected_any`)도 실행 전에 적었다. 판정 기준, 표본 규칙, 데이터셋·정책·바이너리·인덱스 세대 해시는 [results/evaluation-protocol.json](results/evaluation-protocol.json)(개정 3, `223af460…`)에 묶었고, 실행 단계는 이 해시가 바뀌면 멈춘다. 정책 `rtl-1`은 평가 전에 고정했고 이후 바꾸지 않았다.

첫 실행(프로토콜 `6513df25…`)은 대체했다. child 02 인계 파일과 오프라인 증거에 로컬 절대 경로가 기록되어 있어, 경로를 저장소 상대 경로로 바꿔 증거를 다시 만들었고 그 결과 인계 파일 해시가 바뀌었기 때문이다. 입력·정책·데이터셋·바이너리·인덱스 세대는 같다. 두 실행의 관측은 출력 바이트 수(출력에 포함된 조회 시간 숫자의 자릿수 차이로 28건이 1바이트 다름)를 빼면 같았고, 의미 지표·필터 집계도 같았다. 첫 실행 기록은 [results/history/protocol-6513df254a21](results/history/protocol-6513df254a21/SUPERSEDED.md)에 있다.

개정 3은 검토에서 발견한 재실행 보호와 통계 해석 문제를 고친 실행이다. 작업공간·실제 인덱스/통계 해시 검사, 반환 세대 검사, 관측 조합의 누락·중복 및 원시 출력 대조를 추가했다. 데이터셋은 다시 만들지 않았으며 입력·정책·바이너리·인덱스와 의미 라벨을 유지했다. 이전 개정 2의 결과와 당시 신뢰구간·빌드 비용은 [대체된 기록](results/history/protocol-c511cc5224b1/SUPERSEDED.md)에 보존했다. 현재 결과에서는 사전 지표의 입력 중복을 제거하고 비독립 관측의 신뢰구간을 삭제했다.

| 그룹 | 입력 | 성격 |
| --- | ---: | --- |
| 대조군(control) | 16 | 작성 단계·child 01에서 이미 조사한 사례(`delete`, `retry`, `event loop`, `res` 등). 독립 표본이 아니다 |
| 범주 probe | 46 | 동의어·별칭·다의 약어·구·철자/형태·미등록어·오의미 음성 대조를 채우려고 직접 고른 입력. 선택 편향이 있을 수 있다 |
| 무작위 probe | 30 | 네 저장소 중 3곳 이상에서 3개 이상 파일에 나오는 소문자 4~12자 단어 392개(불용어·키워드 제외)에서 시드 20261006으로 뽑았다. 데이터셋과 무관하게 골랐다 |

각 입력을 네 대상 저장소 모두에서 조회해 관측은 368개(92 × 4)다. 검토 대상은 어느 대상에서든 최종 후보가 된 고유 (입력, 후보) 317쌍 전부와, 정규화를 통과했지만 한 번도 최종 후보가 되지 못한 502쌍에서 시드 20261007로 뽑은 150쌍이다.

**검토자 공개**: 라벨은 에이전트(Claude)가 단독으로 붙였다. 기록된 근거(sense 정의문, 개념, 별칭, 약어 기록)와 고정된 의도한 의미만 보고 판정했다. 독립적인 사람 주석이 아니며 주석자 간 일치도도 없다. 파이프라인은 어떤 모델도 호출하지 않는다. 판정 결과는 [results/sample-review.tsv](results/sample-review.tsv)에 근거와 함께 있다.

| 라벨 | 뜻 |
| --- | --- |
| 동의어 | 의도한 코드 의미에서 바꿔 써도 된다 |
| 근접어 | 의도한 의미에 가깝지만 일부 문맥에서만 바꿔 쓸 수 있다 |
| 별칭 | 같은 개념의 다른 이름 |
| 약어 | 의도한 의미에 맞는 약어·전체 표기 |
| 철자·형태 | 철자, 하이픈, 굴절 변형 |
| 관련만 | 관련 개념이지만 같은 의미가 아니다 |
| 오의미 | 입력의 다른 의미에서만 성립한다 |

근거 sense는 다른데 후보가 우연히 의도한 의미에 맞는 4건(`verify`→`assert`, `take`→`acquire`, `append`→`add`, `route`→`path`)은 근접어로 두고 근거란에 우연이라고 적었다.

### 3.2 기술 결과

- 368개 관측 모두 저장소 통계가 `ready`였다. 통계 미준비로 인한 기술적 불가는 0건이다.
- `(입력 ID, 대상)` 368개 조합이 정확히 한 번씩 있고, 요약 관측이 원시 출력과 일치했다. 5회 반복 조회 출력도 같았다.
- **이번 실행은 기존 데이터셋을 재사용했다.** 빌드·전체 재생성 검증은 실행하지 않았다. SQLite의 해시·파일 식별자·수정 시각·크기가 실행 전후 동일했다. 재생성 일치 증거와 빌드 비용은 이전 실행 기록이다.
- 작업공간 설정, 실제 `published.json`, 인덱스 `meta.json`, 통계 SQLite를 인계 해시와 대조했다. 반환된 데이터셋·정책·인덱스 세대도 프로토콜과 일치해야 한다.
- 92개 입력 × 4개 대상을 child 02와 같은 샌드박스(네트워크·체크아웃·인덱스 세그먼트 차단)에서 다시 조회한 출력이 기록된 실행과 같았다([results/evaluation-offline.json](results/evaluation-offline.json)).
- 임시 설정·통계 파일과 메모리상의 관측을 바꿔, 설정/세대 변경·관측 누락/중복·원시 출력과의 불일치가 거부됨을 확인했다. 결과는 [results/review-fixes-verification.json](results/review-fixes-verification.json)에 있다.

### 3.3 입력 단위 결과

반환 비율은 관측(입력 × 대상) 기준이다. **사전 단계 지표는 저장소와 무관하므로 고유 입력을 한 번만 센다.** 같은 입력의 네 저장소 결과는 독립 시행이 아니고 대조군·범주 표본은 수동 선정됐으므로, 이 표에는 신뢰구간을 붙이지 않는다. JSON의 해당 `ci95`도 `null`이다. 아래 비율은 이번 표본의 관측 결과이지 일반적인 성능 보장이 아니다.

| 지표 | 전체 (입력 92 / 관측 368) | 대조군 (16 / 64) | 범주 probe (46 / 184) | 무작위 probe (30 / 120) |
| --- | --- | --- | --- | --- |
| 사전 항목 있음 — 고유 입력 기준 | 80.4% (74/92) | 75.0% (12/16) | 80.4% (37/46) | 83.3% (25/30) |
| 최종 후보가 하나 이상 | 60.1% | 48.4% | 59.2% | 67.5% |
| 엄격한 동의어 반환 | 9.2% | 6.3% | 12.5% | **5.8%** |
| 동의어·근접어 반환 | 17.1% | 6.3% | 18.5% | **20.8%** |
| 유용 반환(동의어·근접어·별칭·약어) | 35.6% | 35.9% | 41.3% | 26.7% |
| 별칭·약어만 반환 | 18.5% | 29.7% | 22.8% | 5.8% |
| 결과는 있으나 유용한 후보 없음 | 24.5% | 12.5% | 17.9% | 40.8% |
| 최종 후보에 오의미 포함 | 27.4% | 7.8% | 19.6% | **50.0%** |
| 최종 후보 없음(기권) | 39.9% | 51.6% | 40.8% | 32.5% |
| 기대어가 사전 단계에 있음 — 기대어가 있는 고유 입력 기준 | 59.5% (44/74) | 62.5% (10/16) | 73.5% (25/34) | 37.5% (9/24) |
| 기대어가 최종 단계에 있음 — 기대어가 있는 관측 기준 | 36.1% (107/296) | 35.9% | 46.3% | 21.9% (21/96) |

상태별로는 정상 221, 사전에 없음 72, 관계 없음 8, 필터 후 비어 있음 67이었다. 대상별 동의어·근접어 반환은 flask 18.5%, gin 17.4%, tokio 19.6%, express 13.0%였다.

범주별로 보면 차이가 크다.

| 범주 | 관측 | 유용 반환 | 동의어·근접어 | 오의미 포함 | 비고 |
| --- | ---: | ---: | ---: | ---: | --- |
| 명시적 동의어(직접 고른 일반 동사) | 44 | 75.0% | 72.7% | 29.5% | `start`, `stop`, `change`, `merge` 등 |
| 별칭 | 40 | 47.5% | 0% | 0% | 별칭은 동의어로 세지 않는다 |
| 다의 약어 | 40 | 75.0% | 0% | 25.0% | `res`→`result`, `dir`→`direction`, `tmp`→`t`(뉴클레오타이드) |
| 알려진 공백 | 24 | 0% | 0% | 0% | `retry`는 법정 재심리 sense뿐이고 저장소 필터에서 빠졌다 |
| 미등록어 | 24 | 0% | 0% | 0% | `debounce`, `upsert`, `middleware`, `goroutine`, `mutex` 등 5개는 사전에 없다 |
| 구 | 24 | 8.3% | 0% | 0% | `gc`, `di`만. `thread pool`, `race condition`은 사전에 없다 |
| 철자·형태 | 16 | 18.8% | 18.8% | 0% | `deleting`, `canceled`는 형태 분석이 없어 사전에 없다 |
| 오의미 음성 대조 | 32 | 25.0% | 9.4% | 56.3% | `kernel`→`center`, `thread`→`pass`, `shell`→`casing`, `handle`→`hold` |
| 무작위 어휘 | 120 | 26.7% | 20.8% | 50.0% | 아래 3.4 |

### 3.4 후보 단위 정밀도

최종 후보 고유 317쌍: 동의어 12, 근접어 33, 별칭 18, 약어 20, 철자·형태 9, 관련만 84, 오의미 141. 동의어·근접어 비율은 **14.2%**, 유용 비율 26.2%, 오의미 비율 **44.5%**다. 이번 입력에서 나온 최종 쌍 전수를 검토한 관측 비율이며, 같은 입력·sense를 공유하는 후보를 독립 표본으로 간주한 신뢰구간은 제시하지 않는다. 대상별로 중복 집계한 최종 후보 764개에서도 유용 27.6%, 오의미 43.9%였다. 무작위 probe의 최종 후보 155쌍은 동의어 3, 근접어 11, 약어 3, 철자·형태 2, 관련만 31, 오의미 105였다.

| 관계 종류 (고유 쌍) | 쌍 | 동의어·근접어 | 유용 | 오의미 |
| --- | ---: | ---: | ---: | ---: |
| `synonym` (WordNet) | 204 | 20.6% | 20.6% | 60.3% |
| `similar` (WordNet) | 29 | 10.3% | 10.3% | 44.8% |
| `alias` (GitHub·Wikidata) | 42 | 0% | 57.1% | 7.1% |
| `abbreviation_expansion` / `abbreviation_of` | 13 / 3 | — | 14/16 | 2/16 (`res`→`result`, `dir`→`direction`) |
| `related_topic` (GitHub) | 26 | 0% | 0% | 0% (모두 관련만) |

출처별로는 WordNet 233쌍의 유용 비율이 19.3%, 오의미가 58.4%였다. Wikidata 25쌍은 유용 60.0%, abbrcode 14쌍은 85.7%, GitHub Explore 38쌍은 13.2%(대부분 related)였다. 여러 출처가 같은 후보를 지지해도 지지 수로 합산하지 않았다.

### 3.5 어디서 후보를 잃는가

| 단계 | flask | gin | tokio | express |
| --- | ---: | ---: | ---: | ---: |
| 사전 관계 | 1,093 | 1,093 | 1,093 | 1,093 |
| 정규화 후 후보 | 819 | 819 | 819 | 819 |
| 저장소 존재 통과 | 237 | 213 | 334 | 180 |
| 흔함 통과 | 225 | 197 | 322 | 169 |
| 중복 제거 통과 | 224 | 197 | 316 | 169 |
| 최종(한도 10) | 193 | 170 | 243 | 158 |

정규화 단계에서는 반의어 제외 16건과 입력과 같은 표현 2건이 빠졌다. 가장 큰 손실은 저장소 부재(485~639건)였다. 흔함 상한 25%는 11~16건, 한도 10은 11~73건을 잘랐다.

필터 손실 표본 150쌍(모집단 502쌍)은 오의미 66.0% (58.1–73.1), 관련만 22.0%, 유용 12.0% (7.7–18.2)였다. 이 표본은 고유 쌍의 모집단에서 균등 무작위로 뽑았으므로 여기에만 근사 Wilson 95% 구간을 남겼다. 유한모집단 보정은 하지 않았으며, 라벨 판정 자체의 오류나 저장소 밖 일반화의 불확실성을 나타내는 구간은 아니다. 유용한데 빠진 예는 `delete`→`erase`, `k8s`→`kubernetes`, `postgres`→`postgresql`, `begin`→`commence`, `event loop`→`message pump`이며, 모두 네 저장소에 그 단어가 없어서 빠졌다. 그 저장소에서 검색어로 쓸 수 없는 단어이므로 의도한 제거이지만, 저장소마다 결과가 달라진다는 뜻이기도 하다.

저장소 존재 필터는 오의미를 일부 줄였지만 의미를 고르는 장치는 아니다. 필터에 걸린 후보의 오의미 비율(66.0%)이 최종 후보(44.5%)보다 높았으나, 최종 후보의 오의미 비율은 여전히 절반에 가깝다. `pass`, `run`, `string`, `case`, `hold`, `core` 같은 일반 영어 단어는 주석·문서·식별자에 흔해서 존재 조건을 통과한다. 통계 필터는 검색 가능성을 보장할 뿐 의도한 sense를 고르지 못한다.

구 후보(`message loop`, `run loop` 등)는 토큰별 존재만 확인했다. 저장소에 그 구가 실제로 나온다는 뜻이 아니다.

### 3.6 비용

| 항목 | 값 | 조건 |
| --- | --- | --- |
| 원본 아카이브 | WordNet ZIP 9.99 MB, 나머지 tar 합계 약 3 MB | 다운로드 시간은 측정하지 않음 |
| 이번 평가의 데이터셋 빌드 | **실행하지 않음** | 기존 SQLite 재사용, 해시 대조. 빌드 시간을 0초로 측정했다는 뜻이 아님 |
| 이전 데이터셋 빌드 | 60.3초, 최대 RSS 3.66 GiB | Python 3.14, 개정 2의 과거 측정([원본 기록](results/history/protocol-c511cc5224b1/measurements.json)) |
| 이전 데이터셋 검증 | 18.9초, 최대 RSS 3.21 GiB | 개정 2의 JSONL·SQLite 왕복 전체 비교. 이번 평가 비용과 합산하지 않음 |
| 인덱싱 / 어휘 생성 / 통계 저장 | 378~2,312 ms / 4.3~15.8 ms / 2.5~4.0 ms | 대상별 1회, 2.3절 |
| 시작(프로세스 → 준비) | 0.64~1.39 ms (데이터셋 열기 0.38~0.91 ms, 통계 열기 0.15~0.22 ms) | release, 기존 파일 사용. 사전 무결성 대조로 파일을 읽은 상태 |
| 조회 지연(첫 통과) | 중앙값 0.096~0.107 ms, p90 0.38~0.48 ms, 최대 3.24 ms | 대상별 92회 |
| 조회 지연(반복 통과) | 중앙값 0.091~0.095 ms, p90 0.36~0.41 ms, 최대 2.25 ms | 대상별 368회 |
| 조회 프로세스 최대 RSS | 20.1~25.5 MiB | 프로세스 전체 |
| 조회 출력 크기 | 중앙값 약 5.9 KB, 최대 약 125 KB | 탈락 후보·근거 전체 포함 진단 출력 |

OS 페이지 캐시는 제어하지 않았으므로 "첫 통과"는 프로세스 기준의 첫 조회일 뿐 디스크 냉시작이 아니다. 조회 하나의 메모리는 따로 측정하지 않았다. 출력 크기는 진단용 전체 envelope이며 제품 출력 한도가 아니다.

### 3.7 결론

1. **데이터를 만들 수 있는가**: 그렇다. 허용된 다섯 원본에서 sense·출처·라이선스를 보존한 데이터셋을 고정 입력으로 재현했다.
2. **저장소 통계는 정말 인덱싱 시점 작업인가**: 그렇다. 커밋된 세대에서 계산해 공개한 뒤에만 준비 상태가 되며, 조회는 공개된 통계만 읽는다. 수명주기·오프라인 증거가 이를 뒷받침한다.
3. **실제로 유의어를 돌려주는가**: 대체로 아니다. 독립 무작위 probe에서 동의어·근접어를 하나라도 돌려준 관측은 20.8%였고, 최종 후보의 44.5%가 오의미였다. 별칭·약어 출처는 정밀하지만 다루는 어휘가 좁고, 코드 고유 용어(`deserialize`, `enqueue`, `mutex`, `middleware` 등)는 사전에 없다.
4. **이 결과는 품질 통과가 아니다**: 파이프라인 구현은 끝났지만, 의미 측면의 결과는 부정적이거나 불확정이다.

### 3.8 후속 단계 진입 조건

권고 전문은 [results/next-phase.json](results/next-phase.json)에 있다. 진행 여부는 사용자가 정한다.

- **2단계(문맥 LLM 판정)**: 오의미 제거라는 정밀도 실험으로만 조사할 가치가 있다. 주 오류가 오의미이고 후보마다 sense 정의문·순위·출처가 보존되어 판정 입력이 이미 있다. 다만 판정은 사전에 없는 단어를 만들지 못하므로 재현율 공백은 그대로 남는다. 진입 전에 성공 기준(오의미 제거율, 유용 후보 보존율), 독립 평가 표본, 판정 비용 측정을 정해야 한다. 이번 92개 입력은 개발용으로만 써야 한다.
- **3단계(통합 실험)**: 아직 근거가 없다. 2단계의 독립 표본 결과, 코드 도메인 어휘 공백에 대한 대책, 실제 도구 노출·출력 한도·같은 검색 예산 비교 실험 설계가 먼저다. 배포 방향은 사용자 결정대로 GitHub Releases의 최신 데이터셋을 codemap-search 전역 디렉터리로 내려받는 방식이며, 자산 이름·최신 판정·갱신 시점·호환성·롤백·다운로드 실패 정책과 데이터셋 크기(현재 SQLite 228 MB) 기준은 그때 설계한다.

이번 단계는 LLM 판정, 제품 통합, 에이전트 검색 행동, 토큰 절감, Grafana 문제 해결을 실행하지 않았고 주장하지도 않는다.

### 3.9 확인하지 못한 것

- 라벨은 단일 에이전트 판정이다. 사람 검토와 주석자 간 일치도가 없다.
- 영어 입력 92개와 공개 저장소 4개(109~820 파일)에 한정된다. 대형·비공개 저장소와 다른 언어로 일반화하지 않는다.
- 구의 실제 출현 수, 사전 단계 전체 후보의 유용 비율(검토는 최종 후보와 필터 손실 표본만), 디스크 냉시작 지연, 조회당 메모리는 측정하지 않았다.
- 다른 플랫폼에서의 SQLite 파일 바이트 동일성과 샌드박스 동작은 확인하지 않았다.

### 3.10 재현

데이터셋·바이너리·인덱스가 현재 인계 파일과 일치하면 아래 명령만 실행한다. 빌드·재인덱싱은 하지 않는다. 지연 수치는 실행 환경에 따라 달라질 수 있다.

```bash
cd apps/codemap-search/experiments/related-terms-poc
python3 -B evaluate.py select-probes     # cases.json의 R01~R30과 같아야 함
python3 -B evaluate.py run               # 기존 SQLite 재사용 + 4개 대상 조회 + 측정
python3 -B evaluate.py offline-check     # 같은 조회를 샌드박스에서 반복해 출력 대조
python3 -B evaluate.py review-template   # 검토 표 (같은 의미 입력의 기존 라벨 보존)
python3 -B evaluate.py aggregate         # results/poc-report.json
```

평가 코드나 인계 정보가 바뀌면 기존 프로토콜로 실행하지 않는다. 먼저 변경 이유를 적어 `python3 -B evaluate.py freeze --revision-reason "변경 사유"`로 이전 결과를 보존하고 새 프로토콜을 고정한다. 데이터셋·입력 의도·정책·바이너리까지 바뀌는 경우에는 기존 의미 라벨의 자동 재사용도 허용하지 않으므로 별도 재검토가 필요하다.

원시 조회 출력(대상별 JSONL)은 ignored 경로 `.codemap/poc/related-terms/evaluation/223af460741f/`에 있고, 해시는 [results/measurements.json](results/measurements.json)에 있다. 요약 관측은 [results/observations.jsonl](results/observations.jsonl), 최종 보고는 [results/poc-report.json](results/poc-report.json)이다.
