# 개발 관련 의미 큐레이션 준비 계약

이 단계는 **후보 packet 준비만** 한다. 기존 `rtd1-8efd9676107a983d`/`rtn-2`, 평가 `223af460741fb6930e7c48b7b8c840216bc3b33924f25cbe567b885f9ec952b2`, builder/evaluator/인계/README는 변경하지 않는다. 새 서빙 데이터셋 빌드·활성화는 별도 승인 작업이다. 조회 시 LLM·네트워크·소스 스캔을 넣지 않는다.

## 1. 입력·선별 규칙

정확한 목록과 regex는 [development-curation-rules.json](development-curation-rules.json)에 고정한다. 규칙·스크립트·본 계약·입력 manifest/JSONL·소스 아카이브 해시를 packet manifest에 기록한다. 기존 `cases.json`, 평가 라벨, 저장소 출현/통계는 선별에서 읽지 않는다.

- operations seed 295 lemma: **동사 `v`의 모든 sense**를 후보로 수집한다.
- state seed 145 lemma: **형용사 `a`/`s`의 모든 sense**를 후보로 수집한다.
- seed에 등록되어도 원본에 없는 품사/sense는 새로 만들지 않는다. lemma 등록은 의미 승인이 아니다. 동명 명사의 실제 기술 의미는 technical 경로에서 수집한다.
- technical: 직접 기술 domain, 명시적인 컴퓨팅/개발·정량·전자 용어의 gloss/표기 regex, 다의적인 일반 기술 표제어의 **개별 표기 정확 일치**를 수집한다. `computer/software/programming/algorithm/database/compiler/operating system/source code/cryptography` 등 강한 단어와 `type/function/process/tree/graph/memory/file/storage/signal/port/thread/stack/queue/vector/matrix/web` 등 다의적 단어는 별도 목록이다. 후자는 다른 개념의 gloss에 출현한다는 이유로 수집하지 않는다. 정확 일치는 기존 `exact_key`(소문자·공백 정리)이며 stemming/관계 전이 확장은 없다.
- catalogs: GitHub 원본 topic page 1,278개와 기존 Wikidata item 870개를 **모두 후보**로 수집한다. 소스 이름은 포함 근거가 아니다.
- abbreviations: 두 원본 약어 자료의 664개 항목을 **모두 후보**로 수집한다. 일반어 약어도 직접 의미 판정해야 한다.
- gloss/원본 페이지 없이 `related_topic`만 있는 GitHub topic-ref 1,387개는 승인된 기본 제외다.
- 위 경로에 도달하지 않은 WordNet concept은 **직접 미검토 기본 제외**다. 이를 의미적으로 전수 검토했다고 표현하지 않는다.
- 한 concept이 여러 규칙에 맞으면 `operations → technical → catalogs → abbreviations` 우선순위로 한 packet에만 둔다. rule별 raw overlap과 우선순위 적용 후 개수는 별도 집계한다. 모든 적격 후보를 보존하며 상위 N개로 자르지 않는다.

이전 미승인 초안(모든 품사 + 정의문 일반어 출현)은 총 11,413개였다. 부모 지시에 따른 품사·기술 규칙 정교화로 바뀐 이동/제외 개수를 manifest의 `draft_to_approved_transitions`에 기록한다. 기존 참고값 6,191에 수를 맞추지 않는다. 이는 최종 개발 어휘 coverage 측정이 아니다.

## 2. packet 산출물과 해시

루트: `.codemap/poc/related-terms/development-curation/packets/`

- `wordnet-operations.jsonl`
- `wordnet-technical.jsonl`
- `catalogs.jsonl`
- `abbreviations.jsonl`
- `default-reject.jsonl`: 원본 전체에서 후보와 분리된 concept/lexicalization ID, 기본 제외 이유, 미검토 상태의 전수 ledger.
- `manifest.json`: 원본/후보/기본 제외의 concept·표현 분모, 원본 종류별 수, 규칙/이유, 파일 해시·바이트·개수, 150행 기준 배치 계획, 직접 검토 0개, 제한사항.

JSONL은 UTF-8 canonical JSON(키 정렬, 불필요한 공백 없음)이고 concept ID 순서다. 네 후보 packet은 concept와 lexicalization 기준으로 비중복이다. 각 record는 다음을 보존한다.

- `concept`: 기존 canonical concept 전체(source/synset/item ID, 원래 gloss/context 등).
- `lexicalizations`: 그 concept의 **기존 모든 lexicalization**(원문 표기, sense key, ID, 원본 locator, role/rank). seed 표기만 골라 숨기지 않는다.
- `source_input`: pinned source archive에서 읽은 원문. WordNet은 synset의 모든 정의·예문·원본 관계 및 각 member의 원본 sense/entry metadata, GitHub와 SE 약어는 원문 페이지/Markdown, Wikidata는 고정 revision의 entity, abbrcode는 해당 word/abbr/context/degree 원문이다. WordNet canonical dataset에서 생략됐던 예문도 packet에 복구한다.
- `original_relations`: 그 concept에 닿는 기존 관계의 **전체 레코드**. `similar/antonym/related_topic`도 관측/판독을 위해 보존하지만, 개발용 서빙에는 사용하지 않는다. 두 후보 concept을 잇는 원본 관계는 각 concept record에 반복 관측될 수 있다. 이것은 신규 관계나 concept 중복이 아니다.
- `selection_reasons`: 자동 선별 경로이며 의미 승인 근거가 아니다.
- `input_sha256`: 이 필드 자체를 뺀 record 전체의 canonical JSON UTF-8 SHA-256(끝 newline 없음). 원문/표기/관계가 바뀌면 판정 재사용을 막는다.

packet 전체 SHA-256은 newline 포함 실제 파일 바이트의 해시다. 같은 packet 산출물이 이미 있으면 임시 디렉터리에서 다시 생성·검증·완전 파일 해시 대조 후 **기존 것을 유지**한다. 다르면 덮어쓰지 않고 오류다. 기존 dataset SQLite는 열거나 생성하지 않는다.

## 3. 에이전트 직접 판정 기준

각 record의 source sense/gloss/context/예문과 **모든 개별 표기**를 읽고 판단한다. 개발에 직접 쓰는 일반 동사·상태의 해당 의미, 소프트웨어·컴퓨팅·네트워크·데이터·보안·개발 도구는 포함 가능하다. 수학/통계/논리/전자 하드웨어는 개발에 직접 관련되는 의미만 포함한다. 비개발 의미와 불확실한 항목은 제외한다.

키워드, domain tag, 소스 이름, 저장소 출현 자체는 **최종 포함 근거가 될 수 없다**. 개발에서 쓰는 구체적 의미를 원문과 연결해 설명해야 한다. 개발용 sense라 하더라도 같은 concept의 오래된/부적합한 표현은 개별 제외할 수 있다. 표현을 새로 만들거나 의미·관계를 생성/전이하지 않는다.

최종 서빙 관계는 원본 `synonym`, `alias`, `abbreviation_expansion`, `abbreviation_of`만 가능하며 두 endpoint의 concept/lexicalization이 모두 승인·유지된 원본 관계만 남긴다. `related_topic/similar/antonym`은 제외한다. 이 단계에는 그 필터를 적용한 데이터셋 빌드가 없다.

## 4. 판정 출력 계약

각 입력 packet record당 아래 **정확한 9개 필드**를 갖는 canonical JSON 한 줄을 출력한다. 판정 파일은 ignored curation 루트 아래에서 관리하며 기존 인계에 연결하지 않는다.

| 필드 | 요구사항 |
| --- | --- |
| `schema_version` | `development-curation-decision/1` |
| `packet_name` | 입력과 정확 일치 |
| `concept_id` | 등록된 입력 concept ID |
| `input_sha256` | 해당 입력 record 해시와 정확 일치 |
| `decision` | `include` / `exclude` / `uncertain` |
| `keep_lexicalization_ids` | 중복 없는 기존 ID 목록. include는 1개 이상, exclude/uncertain은 빈 목록 |
| `development_rationale` | 원문 sense의 개발 직접 관련성 또는 제외/불확실 근거. 비어 있을 수 없음 |
| `lexicalization_rationales` | 입력의 **모든 lexicalization ID**를 정확히 한 번 key로 갖는 객체. 각 표기의 유지/제외 이유를 비어 있지 않은 문자열로 설명 |
| `review_basis` | `direct_source_sense_and_individual_surface` — 직접 판독했다는 선언 |

같은 concept의 일부 표현만 유지할 수 있다. 유지하지 않은 표기도 설명을 생략할 수 없다. uncertain은 보수적 제외이며 서빙 승인으로 해석하지 않는다. 자동 후보 선별은 직접 판독 선언을 대신하지 못한다.

검증기는 누락/중복/미등록 concept·표현 ID, 빈 근거, 입력 해시 불일치, 판정 값·필드 불일치를 오류로 처리한다. 전체 모드는 모든 후보, `--batch-id`는 manifest에 등록된 정확한 한 배치만 요구한다. 임의 부분 집합은 완수로 인정하지 않는다. **기계 검증은 의미 판단의 진실성·정확성을 증명하지 않으므로 별도 독립 검토가 필수다.**

## 5. 명령과 인계

아래 명령의 cwd는 모두 `/Users/buyong/workspace/private/buyong-mcp`다. 새 테스트 파일/사례/의존성은 추가하지 않는다. `-B`로 기존 실험의 `__pycache__` 쓰기도 피한다.

```bash
python3 -B apps/codemap-search/experiments/related-terms-poc/prepare_development_curation.py prepare
python3 -B apps/codemap-search/experiments/related-terms-poc/prepare_development_curation.py verify
python3 -B apps/codemap-search/experiments/related-terms-poc/prepare_development_curation.py verify-preservation
python3 -B apps/codemap-search/experiments/related-terms-poc/prepare_development_curation.py validate-decisions .codemap/poc/related-terms/development-curation/decisions.jsonl
# 한 배치 검증은 --batch-id <manifest.batch_plan의 정확한 batch_id> 추가
```

`verify-preservation`은 준비 시작 전 저장한 `development-curation/preservation-baseline.json`의 기존 파일 해시를 다시 확인한다. 기존 실험·브리프·원본·dataset SQLite·평가·실제 workspace/index/statistics 11,910파일과 HEAD·staging을 확인한다. 이 baseline은 시작 시점 이후 보존 증거이지, 이전 untracked 내용의 변경 유무를 소급 증명하는 자료가 아니다.

`manifest.batch_plan`의 `start_line`과 `record_count`로 100~200행 직접 판독을 진행한다(기본 150행, 마지막 잔여 배치는 작을 수 있음). 원문 포함 bytes와 lexicalization 수가 함께 있으므로 실제 읽기 부담도 확인한다. 직접 판독은 이 준비 단계에서 **아직 시작하지 않았다**. 판정 수·배치 완료·미검토 제외를 분리해서 보고하고 전수 검토를 과장하지 않는다.
