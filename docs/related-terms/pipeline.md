# 관련어 PoC 데이터셋 생성 과정

관련어 PoC의 세 데이터셋을 어떤 입력과 스크립트로 만들었는지 설명한다. 스크립트를 고치거나 데이터셋을 다시 만들려는 개발자가 각 단계의 입력, 출력, 명령을 찾는 데 쓴다.

**재현 범위:** 스크립트와 LLM 생성 지침은 저장소에 남아 있다. 그러나 LLM이 생성한 원자료와 중간 산출물은 보존하지 않으므로, **같은 데이터셋을 바이트 단위로 다시 만들 수는 없다.** 데이터셋은 결과물 자체를 신뢰하는 기준으로 쓴다. 외부 입력의 출처는 [원자료와 근거 저장소](sources.md)에 있다.

경로는 다음처럼 줄여 쓴다.
- `PoC/` = `apps/codemap-search/experiments/related-terms-poc/`
- `T/` = `.codemap/poc/related-terms/thesaurus-v1/`. 생성 당시의 작업 폴더이며, 지금은 관련어 데이터셋만 남아 있다

## 데이터셋 한눈에 보기

| 데이터셋 | 스키마 | 위치 | 쓰는 도구 | 만든 스크립트 |
| --- | --- | --- | --- | --- |
| 뜻 `rts3-9e01eaf1eadf960a` | `rt-sense/3` | `PoC/datasets/` (저장소에 포함) | `sense_lookup.py`의 기본값 | `build_sense_dataset.py`, `correct_sense_dataset.py` |
| 카테고리 `rtc2-80e77d8fb9ef2b41` | `rt-category/2` | `PoC/datasets/` (저장소에 포함) | `category_lookup.py`의 기본값 | `build_category_dataset.py` |
| 관련어 `rtr1-930791c83bea6132` | `rt-related/1` | `T/datasets/` (로컬에만 있음, 약 430MiB) | `rtpoc3 related`, 두 조회 도구의 입력 해석 | `build_related_dataset.py` |

세 데이터셋은 모두 아래 1단계의 LLM 생성 자료에서 출발한다.

```text
52개 분야 정의
  └─ 1. LLM 생성: 단어(vocab) → 관련어(relations) → 함께 쓰는 단어(co-use)
        ├─ 2. 관련어 데이터셋 rtr1   (+ 외부 사전 자료, 임베딩)
        ├─ 3. 카테고리 데이터셋 rtc2 (+ 하위 분야 분할)
        └─ 4. 뜻 데이터셋 rts3       (뜻 그래프 → 검토 → 빌드 → 단어 평가 보강 → 검토 보정)
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

단어의 뜻(persona)은 개념(concept)에 속하고, 개념은 주제(topic)에, 주제는 1~3개의 분야(field)에 속한다.

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
| 9 | 검토 보정. 의미 검토 결과를 여러 차례 적용하고, 입력별 관련어를 담는 `rt-sense/3` 확장을 더한다 | `correct_sense_dataset.py`(`reviewed_relations.py`, `reviewed_graph_edits.py`) | `rts3-9e01eaf1eadf960a`(현재 기본값) |

- 1~5단계의 정확한 순서는 5단계만 기록으로 확인했다. 5단계의 입력 그래프는 `T/sense-graph-2-split`이었다. 그 앞 단계는 그래프 폴더 이름(`sense-graph-1` → `sense-graph-2` → `sense-graph-2-reviewed` → `sense-graph-2-split`)으로만 알 수 있다.
- 6~8단계는 각 데이터셋의 `sense_meta`(`graph`, `base_dataset`, `entries`)로 확인했다.
- 8단계 입력 `entries.json`은 [코드 단어 수집과 단어 평가](word-evaluation.md)의 보강 단계에서 만들었다. 주제 47개, 개념 1,265개, 뜻 3,358개, 기본형 뜻 이어받기 272건이 들어 있었다.
- 9단계의 보정 계획은 마지막 단계의 것만 데이터셋 폴더의 `corrections.json`에 남아 있다. 보정 계획의 형식은 `correct_sense_dataset.py`의 설명문에 있다.
- 남은 결함은 [알려진 결함](known-defects.md)에 있다.

새 보정을 적용할 때는 다음을 지킨다.
- 직전 데이터셋 폴더를 `--base`로 준다. 기존 데이터셋은 바뀌지 않고 새 폴더가 생긴다.
- 새 개념·주제가 필요하면 `add-entries`를 먼저 실행한다.
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

두 Python 조회 도구는 입력 단어의 표기를 해석할 때 `rtpoc3 related`를 실행한다. `rtpoc3`가 없으면 다음 명령으로 빌드한다.

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
