# 바이트 제한 의미 판독 projection · 판정 계약 v2

**현재 분류 출력은 `development-curation-decision/2`를 사용한다.** [development-curation.md](development-curation.md)의 원본 packet/선별/의미 판정 기준은 그대로 유지하되, 초기 decision/1 계약은 아래 v2가 대체한다. 원본 packet 4개와 그 manifest는 고정되어 변경하지 않는다. 별도 새 스크립트가 파생 projection만 만든다. 기존 builder/evaluator/인계/README/데이터셋은 그대로다.

## 1. 원본과 파생 뷰

- 원본: `.codemap/poc/related-terms/development-curation/packets/`.
- 파생 뷰: `.codemap/poc/related-terms/development-curation/projection/`.
- 새 스크립트: `apps/codemap-search/experiments/related-terms-poc/project_development_curation.py`.
- 파생 파일: 원본과 이름이 같은 네 JSONL, `manifest.json`, `pages/<packet>-<6자리 번호>.json`.
- projection 버전: `development-curation-projection/1`.

각 projected record는 원본 `concept`, `lexicalizations`, `source_input`, `selection_reasons`, `input_sha256`을 **전체 그대로** 보존한다. source ID/revision/license를 포함한 기존 `source` 레코드를 추가한다. POS/domain, 원문 gloss/모든 정의/context/예문, 원본 sense key, 모든 표기/역할/원본 locator, 약어 context/충돌 표시 및 별칭 판단에 필요한 속성이 생략되지 않는다.

반복되는 **canonical `original_relations` full record만** `original_relations_summary`로 대체한다. 종류별 개수, 전체 record 수, 원본 리스트 canonical JSON UTF-8 SHA-256(newline 제외), 원본 packet 파일·1-indexed line·field locator를 제공한다. 원본 synset/sense 내부의 관계 속성도 `source_input`에서 그대로 보존한다. summary로 신규 관계나 전이 관계를 추론하지 않는다. `similar/antonym/related_topic`은 개발용 서빙에 금지되어 있다.

Projected record의 `projection_sha256`은 자기 필드를 제외한 전체 projected record canonical JSON UTF-8 SHA-256(newline 제외)이다. 원본 packet 파일 SHA-256과 개별 원본 input hash도 함께 보존한다. projection manifest는 raw manifest·projection 스크립트·본 계약 해시, 파생 파일/페이지 해시·바이트, 각 concept의 정확한 ID 집합·입력/파생 해시·페이지 목록을 기록한다.

## 2. 페이지와 oversized record

페이지는 **actual UTF-8 serialized bytes 24KiB 이하**로 생성한다. 행 수로 자르거나 상위 N개 표현만 남기지 않는다. 페이지에는 다음이 있다.

- `page_id`, `next_page_cursor`(마지막 페이지는 null), packet 이름·해시.
- 원본 packet line 범위, concept/lexicalization ID 범위와 개수, 완전 record 수.
- `items`: 완전 projected record 또는 식별 가능한 연속 fragment.
- oversized인 경우 concept ID, projection hash, fragment index/count, 각 fragment UTF-8 SHA-256을 제공한다.

정상 record는 페이지에서 완전하게 읽는다. serialized item이 18KiB를 넘으면 oversized로 표시하고 **전체 canonical record text를** 약6KiB UTF-8 연속 조각으로 제공한다. Unicode 문자 경계로 나누며 조용한 truncate는 없다. fragment들은 모두 이어붙여 원본 projected object로 완전히 복원한다. 한 fragment 페이지의 lexicalization 개수는 전체 concept의 정체성 분모이고, 그 조각에 모든 표기가 보인다는 뜻은 아니다.

**concept에 연결된 모든 페이지와 oversized 조각을 확인하기 전에는 판정 완료로 선언하지 않는다.** `manifest.concept_projections[concept_id].page_ids`의 모든 페이지를 정확한 순서로 판정 record의 `reviewed_page_ids`에 기록해야 한다. 페이지 ID 선언은 실제 읽기의 기계적 증명이 아니며 독립 검토가 필요하다. raw의 반복 관계 레코드를 모두 에이전트가 읽었다고 주장하지 않는다.

## 3. 검증

projection `verify`는 다음을 기계적으로 대조한다.

1. 기존 packet 검증과 원본 파일·script·contract 해시.
2. projected record 전체와 raw에서 다시 계산한 record의 완전 object equality. 필수 의미 필드와 전체 lexicalization 집합이 정확히 일치해야 한다.
3. 원본 relation 리스트에서 종류별 개수·리스트 해시·raw locator를 복원해 summary와 정확히 일치해야 한다.
4. 전체 페이지 파일 해시·24KiB cap·cursor/ID 범위·record coverage.
5. 모든 fragment의 해시·순서·총 개수·중복/누락 및 완전 record 복원. 복원한 모든 object가 파생 JSONL의 전체 object와 정확히 같아야 한다.
6. 전체 후보 7,147 concept과 17,830 lexicalization 누락/중복 없음.

임시 디렉터리에서 생성·검증 후 공개한다. 기존 파생 뷰가 있으면 전체 파일 바이트 해시가 완전히 같은 경우에만 재사용하고, 다른 경우 덮어쓰지 않는다. 기존 raw packet manifest는 변경하지 않는다.

## 4. 판정 출력 `development-curation-decision/2`

[초기 계약](development-curation.md)의 9개 필드를 유지하되 `schema_version`은 **`development-curation-decision/2`**로 바꾼다. 정확한 추가 필드는 다음 5개다(전체 14개, extra field 오류).

| 필드 | 값 |
| --- | --- |
| `packet_sha256` | 해당 원본 packet 파일의 고정 SHA-256 |
| `projection_version` | `development-curation-projection/1` |
| `projection_sha256` | 해당 concept의 projected record 해시 |
| `projection_manifest_sha256` | 고정 projection manifest 파일의 실제 SHA-256 |
| `reviewed_page_ids` | `concept_projections[concept_id].page_ids` 전체와 정확히 같은 순서·내용 |

concept별 `include/exclude/uncertain`, `keep_lexicalization_ids`, 개발 직접 관련성의 `development_rationale`, **모든 원래 표기**에 대한 `lexicalization_rationales`가 필요하다. include만 유지 ID가 1개 이상이어야 하고 exclude/uncertain은 빈 목록이다. 불확실 항목은 서빙에서 제외한다. source sense와 개별 표기를 읽었다는 `review_basis=direct_source_sense_and_individual_surface`를 요구한다. 키워드/domain/소스 이름/저장소 출현만으로 포함하지 않는다.

판정 검증은 누락/중복/미등록 ID, 입력·packet·projection 해시 불일치, 불완전 페이지/조각 선언, 표기 근거 누락을 오류로 처리한다. `--packet`은 그 packet의 **모든 concept**만 검증하며 완료되지 않은 subset은 성공하지 않는다. 인계 전체 완료 검증은 네 결과를 합친 canonical JSONL에 대해 `--packet` 없이 실행한다. 의미 판단의 정확성은 기계 검증만으로 증명되지 않는다.

## 5. 네 분류 역할의 명령 · 진행/재개

cwd는 모두 `/Users/buyong/workspace/private/buyong-mcp`다. 아래는 다음 단계 인계용이며 **이 준비 단계에서 분류 에이전트를 실행하지 않는다**.

```bash
# 1회 준비/대조
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py prepare
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py verify

# 역할별 첫 페이지. 출력 next_page_cursor를 동일 명령에 전달해 계속 읽는다.
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py page wordnet-operations:000001
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py page wordnet-technical:000001
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py page catalogs:000001
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py page abbreviations:000001

# 역할별 완전 판정 검증. decisions 파일은 해당 단계에서 직접 판정 후 생성한다.
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py validate-decisions .codemap/poc/related-terms/development-curation/decisions/wordnet-operations.jsonl --packet wordnet-operations
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py validate-decisions .codemap/poc/related-terms/development-curation/decisions/wordnet-technical.jsonl --packet wordnet-technical
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py validate-decisions .codemap/poc/related-terms/development-curation/decisions/catalogs.jsonl --packet catalogs
python3 -B apps/codemap-search/experiments/related-terms-poc/project_development_curation.py validate-decisions .codemap/poc/related-terms/development-curation/decisions/abbreviations.jsonl --packet abbreviations
# 전체 merged 판정은 --packet 없이 validate-decisions <merged-path>
```

`projection/manifest.json`의 `packet_stats`는 역할별 전체 페이지/행/표현/바이트와 oversized concept를 제공한다. `pages`는 페이지별 순서·범위·해시·다음 cursor, `concept_projections`는 판정에 필요한 정확한 페이지 목록과 해시를 제공한다. 진행을 저장할 때 실제 읽은 page ID/해시와 마지막 `next_page_cursor`를 함께 저장한다. 재개할 때 먼저 `verify` 후 해당 cursor로 읽고, 미완성 oversized concept는 첫 미확인 조각부터 이어서 읽는다. 조각 누락이 의심되면 그 concept에 연결된 모든 페이지를 다시 읽는다.

원본 111,718개를 의미적으로 전수 분류한 것이 아니다. 직접 검토 대상은 자동 선별 후보 **7,147개**이며 준비 시점 직접 검토/판정은 **0개**다. 104,571개 기본 제외는 직접 미검토로 구분한다. 보존된 raw의 모든 관계를 읽었다고 주장하지 않는다. 이 단계에서 판정 파일·새 서빙 데이터셋·활성 인계는 만들지 않는다.
