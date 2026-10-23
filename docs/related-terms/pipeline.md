# 관련어 PoC 데이터셋 생성 과정

관련어 PoC의 세 데이터셋을 어떤 입력과 스크립트로 만들었는지 설명한다. 스크립트를 고치거나 데이터셋을 다시 만들려는 개발자가 각 단계의 입력, 출력, 명령을 찾는 데 쓴다.

**재현 범위:** 스크립트와 LLM 생성 지침은 저장소에 남아 있다. 그러나 LLM이 생성한 원자료와 중간 산출물은 보존하지 않으므로, **같은 데이터셋을 바이트 단위로 다시 만들 수는 없다.** 데이터셋은 결과물 자체를 신뢰하는 기준으로 쓴다. 외부 입력의 출처는 [원자료와 근거 저장소](sources.md)에 있다.

경로는 다음처럼 줄여 쓴다.
- `PoC/` = `apps/codemap-search/experiments/related-terms-poc/`
- `T/` = `.codemap/poc/related-terms/thesaurus-v1/`. 생성 당시의 작업 폴더이며, 지금은 관련어 데이터셋만 남아 있다

## 데이터셋 한눈에 보기

| 데이터셋 | 스키마 | 위치 | 쓰는 도구 | 만든 스크립트 |
| --- | --- | --- | --- | --- |
| 뜻 `rts3-48356e35e1f1ea28` | `rt-sense/3` | `PoC/datasets/` (입력 파일만 저장소에 포함, `dataset.sqlite3`는 로컬에만 있음) | `sense_lookup.py --sense-dataset`로 지정 | `build_sense_dataset.py`, `correct_sense_dataset.py` |
| 뜻 `rts4-86c5620af1f181ce` | `rt-sense/4` | `PoC/datasets/` (입력 파일만 저장소에 포함, `dataset.sqlite3`는 로컬에만 있음) | `sense_lookup.py --sense-dataset`로 지정 | `migrate_sense_dataset.py`, `add_topic_mappings.py` |
| 뜻 `rts4-e5083ab344e1336d` | `rt-sense/4` | `PoC/datasets/` (입력 파일만 저장소에 포함, `dataset.sqlite3`는 로컬에만 있음) | `sense_lookup.py --sense-dataset`로 지정 | `revise_topics.py`, `add_topic_mappings.py` |
| 뜻 `rts4-710c8ba0d79a776a` | `rt-sense/4` | `PoC/datasets/` (입력 파일만 저장소에 포함, `dataset.sqlite3`는 로컬에만 있음) | `sense_lookup.py --sense-dataset`로 지정 | `revise_topics.py`, `add_topic_mappings.py` |
| 뜻 `rts4-301d57d419380d08` | `rt-sense/4` | `PoC/datasets/` (입력 파일만 저장소에 포함, `dataset.sqlite3`는 로컬에만 있음) | `sense_lookup.py --sense-dataset`로 지정 | `revise_topics.py`, `add_topic_mappings.py` |
| 뜻 `rts5-75e02862daa29308` | `rt-sense/5` | `PoC/datasets/` (입력 파일만 저장소에 포함, `dataset.sqlite3`는 GitHub 릴리스 `related-terms-data-rts5-75e02862daa29308`에 공개) | `sense_lookup.py`의 기본값 | `compact_sense_dataset.py` |
| 카테고리 `rtc2-80e77d8fb9ef2b41` | `rt-category/2` | `PoC/datasets/` (저장소에 포함) | `category_lookup.py`의 기본값 | `build_category_dataset.py` |
| 관련어 `rtr1-930791c83bea6132` | `rt-related/1` | `T/datasets/` (로컬에만 있음, 약 430MiB) | `rtpoc3 related`, 두 조회 도구의 입력 해석 | `build_related_dataset.py` |

세 데이터셋은 모두 아래 1단계의 LLM 생성 자료에서 출발한다.

```text
52개 분야 정의
  └─ 1. LLM 생성: 단어(vocab) → 관련어(relations) → 함께 쓰는 단어(co-use)
        ├─ 2. 관련어 데이터셋 rtr1   (+ 외부 사전 자료, 임베딩)
        ├─ 3. 카테고리 데이터셋 rtc2 (+ 하위 분야 분할)
        └─ 4. 뜻 데이터셋 rts3·rts4  (뜻 그래프 → 검토 → 빌드 → 단어 평가 보강 → 검토 보정 → 누락 단어 추가 → 분야 연결 → 새 개념 추가 → 빠진 단어 재검토 → 구조 정리 → 분야 매핑 확장 → 주제 재분류 → 틀린 뜻 수정·빠진 뜻 연결)
```

## 1. LLM으로 단어와 관련어 생성

분야 정의는 `PoC/contracts/thesaurus-categories.json`(52개 분야)에 있다. 생성 지침은 `PoC/generation/`에 있으며, LLM에 배치마다 입력·출력 경로를 붙여 전달했다.

| 단계 | 지침 | 출력(당시 위치) | 생성 주체 |
| --- | --- | --- | --- |
| 단어 생성 | `generation/vocab.md` | `T/vocab/<분야>.jsonl`. 분야마다 150~300개, 종류는 word·phrase·abbreviation·proper_noun·extension·identifier | LLM |
| 관련어 생성 | `generation/relations.md`, 출력 검사 `generation/validate_relations.py` | `T/relations/batch-NNN.jsonl`. 관계 종류는 synonym·abbreviation·form·family·near·identifier | Claude Sonnet(표제어 11,041개), Codex `gpt-6.1-sol` xhigh(남은 표제어 806개, `*.codex.jsonl`) |
| 함께 쓰는 단어 생성 | `generation/co-use.md` | `T/co-use-1/batches/co-use-NN.jsonl`(116개). 관계 종류 `co_use` | LLM |

- 지침 안의 `{vocab_dir}`는 단어 출력 폴더 자리다.
- 분야별 단어를 하나로 합친 `vocab-merged.json`과 배치 분할 파일은 남아 있지 않다. 그것을 만든 스크립트도 없다.
- 생성 어휘의 `repo-common` 표시(수집 출처 표시)도 마찬가지다. 그 근거인 저장소 단어 목록과 그것을 만든 스크립트는 남아 있지 않다.
- `batch-009`와 `batch-013`의 원출력은 그 배치에만 쓴 정리 스크립트로 중복과 형식 오류를 고친 뒤 반영했다. 처음에는 표제어당 관계 수를 70·50개로 잘랐으나, 관련어 데이터셋 `rtr1-930791c83bea6132`에서는 자르지 않고 반영했다. 이 정리 스크립트는 원출력과 함께 보존하지 않는다.

## 2. 관련어 데이터셋 `rtr1`

[`PoC/build_related_dataset.py`](../../apps/codemap-search/experiments/related-terms-poc/build_related_dataset.py)가 다음 입력을 하나의 표기·관계 표로 합친다.

| 입력 | 관계 수 |
| --- | ---: |
| LLM 관련어(1단계, 표제어 11,847개) | 536,861 |
| 4라운드 v2 정제 사전 | 10,356 |
| 1라운드 원본 사전 `rt-dataset/1`(Open English WordNet, GitHub Explore, Wikidata, abbrcode, 소프트웨어 공학 약어. 반의어 제외) | 402,473 |

- 모든 표기에 `jinaai/jina-embeddings-v2-base-code` 768차원 벡터를 `int8`로 양자화해 저장한다. 조회할 때 모델을 실행하지 않는다.
- 결과는 표기 364,074개, 관계 916,718개다.
- 키 규칙 `rtk-1`은 `PoC/lookup-v3/src/related_keys.rs`와 같고, `rtpoc3 related-check`로 확인한다.
- 실행에는 `fastembed`·`numpy` 환경과 임베딩 모델이 필요하다.
- v2 사전과 원본 사전은 보존하지 않는다.

```sh
python3 build_related_dataset.py --thesaurus <생성 자료 폴더> \
  --v2-dataset <v2 dataset.sqlite3> --original-dataset <원본 dataset.sqlite3> \
  --models <임베딩 모델 캐시> --output-root <출력 폴더>
```

조회 방법과 알려진 한계는 [`PoC/README.related.ko.md`](../../apps/codemap-search/experiments/related-terms-poc/README.related.ko.md)에 있다.

## 3. 카테고리 데이터셋 `rtc2`

1. `subdivide_categories.py prepare`: 52개 상위 분야마다 그 분야로 생성된 단어 목록을 쓴다.
2. LLM이 분야마다 `parents/<분야>.json`을 작성한다. 내용은 하위 분야 목록과, 그 분야에 뜻이 없어 뺄 단어 목록이다.
3. `subdivide_categories.py check`: 위 결과를 빌드와 같은 규칙으로 검사한다.
4. `build_category_dataset.py`: `vocab-merged.json`, 분야 정의, 분할 결과로 데이터셋을 만든다.
   - 기호나 대소문자·구분자만 다른 표기(`$state`, `AbortSignal`과 `abort signal`)는 `term_canonical.word_key`로 한 단어로 합친다.
   - 조회 단위는 하위 분야다.

## 4. 뜻 데이터셋 `rts3`

단어의 뜻(persona)은 개념(concept)에 속하고, 개념은 주제(topic)에, 주제는 1~3개의 분야(field)에 속한다. 14단계의 `rt-sense/4`부터는 persona가 주제에 바로 속하고, 개념은 없어도 된다. 19단계의 `rt-sense/5`는 `rt-sense/4`와 같은 행을 더 작게 저장한다. 관련어의 종류는 `relation_kinds`의 번호로, 표기는 키와 다를 때만 저장하고, 조회가 읽지 않는 인덱스 두 개와 `term_legacy_categories`가 없다.

| 순서 | 단계 | 스크립트·지침 | 결과 |
| --- | --- | --- | --- |
| 1 | 뜻 그래프 유도. 단어마다 관련어를 2차 문맥으로 묶어 뜻을 나누고, Louvain으로 개념과 주제를 만든다 | `induce_sense_graph.py --thesaurus … --co-use …` | `T/sense-graph-1` |
| 2 | 주제 이름과 분야 지정(LLM) | `build_sense_dataset.py carry-names`, `check-topics`, `topic-labels`, `check-fields` | 그래프의 `names/` |
| 3 | 소속 검토. 뜻마다 개념 소속을 두 번 독립 판정하고, 일치한 변경만 적용한다 | `review_membership.py`, 지침 `generation/membership-review.md` | 검토된 그래프 |
| 4 | 주제 세분. 같은 코드가 함께 쓰는 개념끼리만 주제를 다시 나눈다 | `subdivide_topics.py prepare`, `check`, `fields` | `T/sense-graph-2-split` |
| 5 | 2차 소속 검토 | `review_membership.py` | `T/sense-graph-2-split-review2` |
| 6 | 빌드 | `build_sense_dataset.py build --graph … --thesaurus … --co-use …` | `rts2-82363b4a9b18baf7` |
| 7 | 입력 뜻 추가. 자기 뜻이 없는 관련어를, 그 관련어가 생성된 뜻을 통해 분류한다 | `build_sense_dataset.py add-input-senses` | `rts2-32469ce3ab670de2` |
| 8 | 단어 평가 보강. 단어 평가에서 빠진 것으로 판정한 단어·뜻을 추가한다 | `build_sense_dataset.py add-entries --entries <entries.json>` | `rts2-309e9bbaa19f7eb5` |
| 9 | 검토 보정. 의미 검토 결과를 여러 차례 적용하고, 입력별 관련어를 담는 `rt-sense/3` 확장을 더한다 | `correct_sense_dataset.py`(`reviewed_relations.py`, `reviewed_graph_edits.py`) | `rts3-9e01eaf1eadf960a` |
| 10 | 누락 단어 추가. 분야마다 빠진 단어를 조사해, 데이터셋에 없거나 분야 없이 있거나 다른 분야에만 있는 단어를 그 분야의 주제에 넣는다 | `build_sense_dataset.py add-entries --entries <entries.json>` | `rts3-25df1cf41d0d8247` |
| 11 | 분야 연결. 누락 단어 조사에서 어떤 분야에 속한다고 본, 관련어로만 있는 단어를 그 분야의 기존 개념에 있는 뜻 하나를 통해 분류한다 | `correct_sense_dataset.py`(`input_bindings`) | `rts3-08b1780cc2880c26` |
| 12 | 새 개념 추가. 분야 연결에서 맞는 개념이 없어 연결하지 않은 단어를 그 분야의 새 개념으로 넣는다 | `build_sense_dataset.py add-entries --entries <entries.json>` | `rts3-5fba9ba33d7c7c7f` |
| 13 | 빠진 단어 재검토. 11·12단계에서 넣지 않은 단어를, 설명이 닿는 모든 분야에 연결한다 | `build_sense_dataset.py add-entries --entries <entries.json>` | `rts3-48356e35e1f1ea28` |
| 14 | 구조 정리. 단어 이름만 붙인 개념을 없애고 단어를 주제에 바로 연결한다(`rt-sense/4`). 조회 결과는 바꾸지 않는다 | `migrate_sense_dataset.py` | `rts4-3b9284a3b42c2917` |
| 15 | 분야 매핑 확장. 분야가 하나뿐인 단어에, 설명에 대상이 든 다른 분야의 주제를 연결한다 | `add_topic_mappings.py --mappings <mappings.json>` | `rts4-86c5620af1f181ce` |
| 16 | 주제 재분류. 주제의 분야와 이름을 다시 매기고, 대상이 같은 주제를 합치고, 잘못 들어간 단어를 옮기고, 빈 주제를 지우고, 빠진 주제를 더한다 | `revise_topics.py --plan <revisions.json>` 뒤 `add_topic_mappings.py --mappings <mappings.json>` | `rts4-0fb3d5bc20c215ba` → `rts4-e5083ab344e1336d` |
| 17 | 틀린 뜻 수정과 빠진 뜻 연결. 틀린 뜻은 관련어를 맞는 뜻으로 옮기거나 뜻에서 떼어내 고치고, 흔한 단어에 빠진 뜻을 연결한다. 두 번 따로 판정해 둘 다 틀렸다고 한 뜻만 고친다 | `revise_topics.py --plan <revisions.json>` 뒤 `add_topic_mappings.py --mappings <mappings.json>` | `rts4-71cffacbc6f7c0d8` → `rts4-912fe1c9f62131b2` |
| 18 | 남은 여러 단어 표현의 틀린 뜻 수정. 17단계와 같은 방식으로 판정하고 고친다 | `revise_topics.py --plan <revisions.json>` | `rts4-301d57d419380d08` |
| 19 | 저장 방식 최적화. 같은 행을 `rt-sense/5`로 옮겨 저장한다. 조회 결과는 바꾸지 않는다 | `compact_sense_dataset.py` | `rts5-75e02862daa29308`(현재 기본값) |

- 1~5단계의 정확한 순서는 5단계만 기록으로 확인했다. 5단계의 입력 그래프는 `T/sense-graph-2-split`이었다. 그 앞 단계는 그래프 폴더 이름(`sense-graph-1` → `sense-graph-2` → `sense-graph-2-reviewed` → `sense-graph-2-split`)으로만 알 수 있다.
- 6~8단계는 각 데이터셋의 `sense_meta`(`graph`, `base_dataset`, `entries`)로 확인했다.
- 8단계 입력 `entries.json`은 [코드 단어 수집과 단어 평가](word-evaluation.md)의 보강 단계에서 만들었다. 주제 47개, 개념 1,265개, 뜻 3,358개, 기본형 뜻 이어받기 272건이 들어 있었다.
- 9단계의 보정 계획은 마지막 단계의 것만 `rts3-9e01eaf1eadf960a` 폴더의 `corrections.json`에 남아 있다. 보정 계획의 형식은 `correct_sense_dataset.py`의 설명문에 있다.
- 10단계 입력 `entries.json`은 `rts3-25df1cf41d0d8247` 폴더에 있다. 단어마다 그 단어 이름의 새 개념을 만들었고, 주제 130개, 개념 1,442개, 새 표제어 738개가 들어 있다. 관계어는 더하지 않았다.
- 11단계의 보정 계획(`corrections.json`)과 단어별 근거(`bindings-review.json`)는 데이터셋 폴더에 있다. 대상은 누락 단어 조사에서 `관련어로만 있음`으로 적었지만 그 분야와 연결이 없던 4,111쌍이다.
  - 같은 주제나 분야에 그 단어를 관련어로 둔 뜻이 있는 1,148쌍은 그 뜻을 통해 연결했다.
  - 나머지 2,963쌍은 Claude Sonnet 5.5(high)가 분야마다 판정했다. 1,881쌍은 기존 개념의 뜻에 연결했다. 1,082쌍은 연결하지 않았다. 이유는 맞는 개념 없음 1,060, 그 분야의 뜻이 아님 19, 표기 문제 3이다.
  - 중복을 합친 연결은 3,021건, 대상 단어는 2,953개다. 새 개념·주제와 관계어는 더하지 않았다.
  - 연결한 단어는 용어 행이 새로 생긴다. 그 표기는 조회 도구가 이전에 보여 주던 관련어 표기를 그대로 썼다([K7](known-defects.md#k7-표제어-표기가-바뀜) 방지).
- 12단계 입력 `entries.json`과 단어별 판단(`concepts-review.json`)은 `rts3-5fba9ba33d7c7c7f` 폴더에 있다. 대상은 11단계에서 맞는 개념이 없어 연결하지 않은 1,060쌍이다.
  - Claude Sonnet 5.5(high)가 분야마다 판정했다. 1,033쌍은 넣었고 27쌍은 넣지 않았다. 넣지 않은 이유는 그 분야의 용어가 아님 14, 너무 일반적임 9, 이미 있는 단어의 표기 변형 3, 맞는 주제 없음 1이다.
  - 넣은 단어는 10단계처럼 단어마다 그 단어 이름의 개념을 만들었다. 같은 것을 가리키는 단어(`bracket matching`과 `balanced parentheses` 등)만 한 개념으로 묶었다.
  - 맞는 기존 주제가 없으면 새 주제를 만들었다. 주제 120개, 개념 1,005개, 새 표제어 969개, 뜻 1,032개가 들어 있다. 관계어는 더하지 않았다.
  - 단어 하나뿐인 새 주제에 넣으면 그 단어의 조회 결과가 비는 경우가 있었다. 그래서 `ANSI escape code`는 기존 주제 `t554`(String literals)로 옮겼다. `v-on`은 Vue 지시자 주제가 없어 넣지 않았다.
  - 표기는 11단계와 같이 조회 도구가 이전에 보여 주던 표기를 그대로 썼다.
- 13단계 입력 `entries.json`과 단어별 판단(`rereview.json`)은 `rts3-48356e35e1f1ea28` 폴더에 있다.
  - 대상은 11단계에서 연결하지 않은 22단어(맞는 개념 없음은 제외)와 12단계에서 넣지 않은 27단어, 모두 49단어다.
  - 11·12단계는 다른 분야가 더 주된 분야이면 "그 분야의 용어가 아님"으로 뺐다. 13단계는 단어의 설명에 그 분야의 대상이 들어 있으면 연결한다. 예를 들어 `nvidia-smi`는 설명에 GPU가 있으므로 `graphics-media`에도 연결했다.
  - Claude Sonnet 5.5(high)가 단어마다 39개 분야를 모두 보고 적합도가 "매우 높음" 또는 "높음"인 분야를 골랐다. 단어 전체의 뜻이 이미 있는 분야는 뺐다.
  - 이미 뜻이 있는 분야라도 그 뜻의 주제가 단어와 맞지 않는 4건은 검토자가 직접 추가했다. 예를 들어 `Spectre`의 `security` 뜻은 Webhook signature verification 주제여서, 취약점 주제에 따로 연결했다.
  - 연결은 59건이다. 연결마다 그 주제에 단어 이름의 새 개념을 만들었다. 새 주제는 4개다.
  - 47단어가 한 곳 이상 연결됐다. `requireNonNull`과 `negative testing`은 연결할 분야가 남지 않았다. 둘 다 원래 분야(오류 처리, 테스트)에는 뜻이 이미 있다.
- 남은 결함은 [알려진 결함](known-defects.md)에 있다.
- 14~19단계의 과정과 검증은 [뜻 데이터셋 구조 정리와 분야 매핑 확장](sense-mapping-log/README.md)에 있다. 15단계의 단어별 판정은 `rts4-86c5620af1f181ce` 폴더의 `mappings.json`과 `mapping-decisions-a.txt`, `mapping-decisions-b.txt`에 있다. 16단계의 판정은 `rts4-0fb3d5bc20c215ba` 폴더의 `revisions.json`과 `rts4-e5083ab344e1336d` 폴더의 `mappings.json`에 항목마다 이유와 함께 있다. 17단계의 판정은 `rts4-71cffacbc6f7c0d8` 폴더의 `revisions.json`과 `rts4-912fe1c9f62131b2` 폴더의 `mappings.json`에, 18단계의 판정은 `rts4-301d57d419380d08` 폴더의 `revisions.json`에 항목마다 이유와 함께 있다([작업 이력 12단계](sense-mapping-log/12-sense-data-fixes.md), [14단계](sense-mapping-log/14-recheck-and-remaining-n6.md)). 19단계는 판정 없이 저장 방식만 바꿨다([작업 이력 16단계](sense-mapping-log/16-compact-storage.md)).
- 17단계는 처음에 판정을 한 번만 해서 `rts4-78debdbe7f7bab0b` → `rts4-710c8ba0d79a776a`를 만들었다. 재검증에서 두 판정이 엇갈린 수정 15개를 빼고 다시 만든 것이 지금의 17단계 판이다. 처음 판은 앞 기본값이라 저장소에 남아 있다.

새 보정을 적용할 때는 다음을 지킨다.
- 직전 데이터셋 폴더를 `--base`로 준다. 기존 데이터셋은 바뀌지 않고 새 폴더가 생긴다.
- 새 개념·주제가 필요하면 `add-entries`를 먼저 실행한다.
- `add-entries`와 `correct_sense_dataset.py`는 `rt-sense/3` 판만 받는다. 현재 기본값은 `rt-sense/5` 판이라 이 두 도구로 고칠 수 없다. `rt-sense/4`·`rt-sense/5` 판에 단어 → 주제 연결을 더하는 도구는 `add_topic_mappings.py`다. 주제의 분야·이름을 고치고 주제를 합치거나 지우는 도구는 `revise_topics.py`다. `revise_topics.py`는 관련어 하나를 같은 단어의 다른 뜻으로 옮기거나 뜻에서 떼어내는 연산과, 검토로 만든 입력 연결을 끊는 연산도 받는다. 두 도구는 출발 판과 같은 판(`rts4-` 또는 `rts5-`)을 만든다. `migrate_sense_dataset.py`는 `rt-sense/4`를 만들고, `compact_sense_dataset.py`가 이를 `rt-sense/5`로 옮긴다.
- 결과를 기본값으로 쓰려면 `sense_lookup.py`의 `DEFAULT_SENSE_DATASET`을 바꾼다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/correct_sense_dataset.py \
  --base <직전 데이터셋 폴더> --corrections <plan.json> --output-root <출력 폴더>
```

## 5. 조회 도구

| 도구 | 하는 일 | 필요한 것 |
| --- | --- | --- |
| `PoC/sense_lookup.py` | 입력 단어 → 뜻 → 그 뜻의 관련어, 같은 개념·주제의 단어 | 뜻 데이터셋, 관련어 데이터셋, `rtpoc3` |
| `PoC/category_lookup.py` | 입력 단어 → 하위 분야 → 그 분야의 단어 | 카테고리 데이터셋, 관련어 데이터셋, `rtpoc3` |
| `PoC/lookup-v3`의 `rtpoc3 related` | 관련어 데이터셋의 유사도순 관련어 | 관련어 데이터셋 |

두 Python 조회 도구는 입력 단어의 표기를 해석할 때 `rtpoc3 related`를 실행한다. `sense_lookup.py`는 생성 횟수가 같은 뜻을 관련어 데이터셋에 저장된 단어 임베딩으로 질의와 비슷한 순서로 정렬한다(`--no-similarity-order`로 끈다). `rtpoc3`가 없으면 다음 명령으로 빌드한다.

```sh
cargo build --offline --release \
  --manifest-path apps/codemap-search/experiments/related-terms-poc/lookup-v3/Cargo.toml
```

관련어 데이터셋은 저장소에 없으므로, 새로 받은 저장소에서는 두 Python 조회 도구가 동작하지 않는다.

## 6. 검증 스크립트

| 스크립트 | 확인하는 것 | 실행 |
| --- | --- | --- |
| `verify_sense_additions.py` | `add-entries` 반복 빌드, 이전 판 보존 | 인자 없음. 합성 SQLite로 실행 |
| `verify_reviewed_relations.py` | 관계 보정의 출처·검증·호환성·게시 | 인자 없음. 합성 데이터로 실행 |
| `verify_input_bindings.py` | 입력 분류 보정과 최종 조회 | 인자 없음. 합성 데이터로 실행 |
| `verify_spelling_aliases.py` | 코드 근거가 있는 표기 별칭 | 인자 없음. 합성 데이터와 실제 조회 진입점으로 실행 |
| `verify_related.py` | 관련어 데이터셋의 정규화·분해·정렬·회귀 | 동결된 `rtpoc2`, v2 사전, 정책, 입력 목록이 필요하다. 이 입력은 보존하지 않으므로 지금은 실행할 수 없다 |
| `rtpoc3 related-check --dataset <rtr1>` | 관련어 데이터셋의 키 정합성 | 관련어 데이터셋만 있으면 된다 |
| `cargo test --offline --manifest-path …/lookup-v3/Cargo.toml` | `rtpoc3` 단위 테스트 | Rust 도구와 의존성 캐시 |

단어 평가 스크립트와 그 검증은 [코드 단어 수집과 단어 평가](word-evaluation.md)에 있다.
