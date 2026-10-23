# 7단계: 재분류 조회 검증

작업일: 2026-10-17. 전 판: `rts4-86c5620af1f181ce`(현재 기본값). 중간 판: `rts4-0fb3d5bc20c215ba`(재분류만). 후 판: `rts4-e5083ab344e1336d`(재분류 + 매핑).

[6단계](06-reclassification.md)의 재분류와 매핑이 조회를 나아지게 했는지, 다른 단어의 조회를 해치지 않았는지 확인했다. 4단계와 달리 이번 단계는 주제를 합치고 분야를 바꾸므로, 뜻이 "같은 뜻"인지는 계획대로 바뀐 모습(합친 주제, 후 판의 주 분야)으로 비교했다.

## 1. 확인한 것

| 확인 | 방법 | 결과 파일 |
| --- | --- | --- |
| 평가 세트 점수 | `sense-eval/score_eval_set.py --sense-dataset <판>/dataset.sqlite3` | `sense-eval/results/rts4-0fb3d5bc20c215ba.json`, `sense-eval/results/rts4-e5083ab344e1336d.json` |
| 매핑한 단어 | `sense-eval/compare_lookups.py --revisions revisions.json --mapped-by mappings.json` | 보존하지 않음 |
| 재분류한 주제에 속한 단어 | `sense-eval/compare_lookups.py --revisions revisions.json --mapped-by revisions.json --affected` | 보존하지 않음 |
| 무작위 표본 | `sense-eval/compare_lookups.py --revisions revisions.json --sample 3000` | 보존하지 않음 |
| 결함 단어와 예시 단어 | 4단계와 같은 20단어(`--words-file`) | 보존하지 않음 |
| 기존 검증 스크립트 4개 | `verify_input_bindings.py`, `verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_spelling_aliases.py` | 모두 통과 |
| 크기와 적재 시간 | 파일 크기와 `SenseIndex.load` 시간 | |

## 2. 평가 세트

전 판 → 중간 판 → 후 판.

| 집단 | 단어 | 적중 | 주 분야 적중 | 첫 뜻 적중 | 관련어 수 중앙값 | 관련어 수 평균 |
| --- | ---: | --- | --- | --- | --- | --- |
| 전체 | 224 | 84.8% → 85.3% → 85.3% | 75.9% → 77.7% → 78.1% | 64.3% → 67.9% → 67.9% | 129 → 129.5 → 129.5 | 215.9 → 216.8 → 216.8 |
| `target` | 113 | 93.8% → 92.9% → 92.9% | 80.5% → 82.3% → 83.2% | 85.8% → 87.6% → 87.6% | 93 → 93 → 93 | 105.5 → 105.8 → 105.8 |
| `multi` | 65 | 86.2% → 87.7% → 87.7% | 83.1% → 84.6% → 84.6% | 43.1% → 50.8% → 50.8% | 372 → 372 → 372 | 443.9 → 445.9 → 446.0 |
| `input` | 46 | 60.9% → 63.0% → 63.0% | 54.3% → 56.5% → 56.5% | 41.3% → 43.5% → 43.5% | 95.5 → 95.5 → 95.5 | 164.9 → 165.6 → 165.6 |

뜻 수와 분야 수의 중앙값은 모든 집단에서 그대로다. 응답이 바뀌지 않은 단어는 224개 중 141개다.

| 변화(전 판 → 후 판) | 단어 |
| --- | --- |
| 적중을 얻음(4) | `commercial`, `prefix`, `set selected`, `skip` |
| 주 분야 적중을 얻음(7) | 위 4개와 `stress`, `coinit`, `kinesis firehose`(`kinesis firehose`만 매핑에서 얻었다) |
| 첫 뜻 적중을 얻음(8) | `commercial`, `selected`, `filter`, `set selected`, `skip`, `search bar`, `fragment`, `weak` |
| 적중을 잃음(3) | `tf`, `amt`, `navigation component` |
| 주 분야 적중을 잃음(2) | `tf`, `navigation component` |
| 첫 뜻 적중을 잃음 | 없음 |

적중을 잃은 3단어는 주 분야 판정의 결과다. 평가 세트의 답을 보고 고치지 않았다.

- `tf`(기대 `time-locale`): 뜻 Input method composition(`t315.11`, `time-locale`)을 같은 대상의 Keyboard input composition(`t680`, `frontend-ui`)에 합쳤다.
- `amt`(기대 `general-wording`·`math-numerics`): Money amounts(`x251`)의 주 분야를 `business-domains`로 바꿨다. `math-numerics`는 보조 분야다.
- `navigation component`(기대 `frontend-ui`): Mobile navigation stacks의 주 분야를 `mobile-desktop`으로 바꿨다.

## 3. 전체 조회 비교

| 대상 | 단어 | 같음 | 잃지 않음 | 뜻을 얻음 | 뜻을 잃음 | 관련어를 잃음 | 관련어 증감(바뀐 단어) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 매핑한 단어 | 51 | 4 | 47 | 43 | 0 | 0 | 중앙값 +3, 최대 +57 |
| 무작위 표본 | 3,000 | 2,328 | 644 | 3 | 2 | 28 | 중앙값 +1, -9 ~ +66 |
| 재분류한 주제에 속한 단어 | 51,939 | 16,455 | 34,132 | 188 | 162 | 1,352 | 중앙값 +1, -35 ~ +89 |
| 결함 단어와 예시 단어 | 20 | 5 | 15 | 5 | 0 | 0 | 중앙값 +5, 최대 +52 |

"같음"은 응답 전체가 같은 것이다. 주제 이름만 바뀌어도 같지 않다. "잃지 않음"은 계획대로 바꾼 뜻을 하나도 잃지 않고 관련어도 잃지 않은 것이다.

- **뜻을 잃은 162단어:** 모두 6단계에서 옮긴 단어 자신이거나, 입력 연결로 그 단어의 persona를 읽는 단어다. 예를 들어 `T.proc`, `T.let`, `# typed: strict` 같은 Sorbet 문법 단어는 Python typing 뜻을 잃고 Ruby development ecosystem 뜻을 얻었다. COBOL 문법 단어(`DATA DIVISION`, `JCL` 등)는 Code refactoring 뜻 대신 `x307` 뜻을 얻었다. 얻은 뜻 없이 잃기만 한 단어(`Kafka`의 커밋 기록 뜻, `Raft`의 클라우드 배포 위치 뜻 등)는 옮긴 곳의 뜻을 이미 갖고 있었다.
- **관련어를 잃은 1,352단어:** 1,192단어는 잃은 관련어가 모두 옮긴 단어다(`Sorbet` 203단어, `inb`·`outb` 198단어, `AWS CodeBuild` 137단어 등). 나머지 160단어는 뜻을 잃은 위 162단어에 들어 있고, 잘못 들어갔던 주제의 단어를 잃었다. 그 밖의 이유로 관련어를 잃은 단어는 없다.
- **매핑한 단어 중 같음 4단어와 뜻을 얻지 않은 4단어:** `CalDAV`, `Orleans`, `bezier curve`, `watermarking`, `SNTP`, `chronyd`, `pgx`, `vimscript`. 별칭이나 입력 연결로 그 주제의 뜻을 이미 갖고 있었다. 매핑으로 그 단어 자신의 뜻이 됐다.
- **결함 단어:** 13단계 5단어는 모두 원래 제안된 분야의 뜻을 얻었다. `requireNonNull`의 새 뜻은 `t635.2`의 주 분야인 `web-backend`로 보인다(6단계 4절). 결함 문서의 다른 단어는 뜻을 얻거나 잃지 않았다.

## 4. 뜻 순서

| 대상 | 순서 변화 | 첫 뜻 변화 | 관련어가 자리를 옮긴 단어 | 자리를 옮긴 관련어 |
| --- | ---: | ---: | ---: | ---: |
| 매핑한 단어 | 1 | 0 | 4 | 9 |
| 무작위 표본 | 112 | 47 | 34 | 71 |
| 재분류한 주제에 속한 단어 | 5,764 | 2,484 | 1,414 | 2,505 |
| 결함 단어와 예시 단어 | 7 | 1 | 5 | 11 |

재분류한 주제에 속한 단어에서 첫 뜻이 바뀐 2,484단어를 원인별로 나눴다.

| 원인 | 단어 |
| --- | ---: |
| 가중치가 같은 뜻 사이의 순서가 새 분야 이름으로 다시 정해짐 | 2,113 |
| 합치기로 뜻의 가중치가 커짐(전 첫 뜻보다 커진 122단어, 같아진 뒤 순서가 다시 정해진 23단어) | 145 |
| 단어를 옮겨 첫 뜻의 주제가 바뀜 | 118 |
| 별칭이나 입력 연결의 뜻이 앞으로 옴 | 108 |

- 조회는 뜻을 가중치(persona 수) 순으로 놓고, 가중치가 같으면 분야 이름의 알파벳순으로 놓는다(`sense_order`). 가중치가 1인 뜻이 많아서, 주제의 주 분야를 바꾸면 그 주제의 뜻이 다른 뜻의 앞이나 뒤로 옮겨 간다. 예: `#fff`의 첫 뜻이 `graphics-media`의 Color channels에서 `general-wording`의 White terminology로 바뀌었다. White terminology의 분야를 `general-wording`으로 바꿨고, 알파벳순으로 `graphics-media`보다 앞이기 때문이다.
- 전 판의 순서도 같은 규칙으로 정해진 것이라, 이 2,113단어의 첫 뜻이 전보다 나아졌거나 나빠졌다고 볼 근거는 없다. 평가 세트에서는 첫 뜻 적중이 8단어 늘고 잃은 단어는 없었다.
- 합치기로 가중치가 커진 예: `Argo CD`의 첫 뜻이 `t103` Argo delivery workflows(Argo CD 단어 주제를 합쳐 가중치 2)가 됐다.

## 5. 크기와 적재 시간

| 판 | 크기 | 적재 시간 |
| --- | ---: | ---: |
| `rts4-86c5620af1f181ce` | 48,631,808바이트 | 약 1.48~1.62초 |
| `rts4-e5083ab344e1336d` | 48,611,328바이트 | 약 1.46~1.68초 |

같은 때 같은 장비에서 `SenseIndex.load`를 세 번씩 잰 범위다.

## 6. 기본 데이터셋

`sense_lookup.py`의 기본값은 `rts4-86c5620af1f181ce` 그대로 두었다. 5단계에서 기본값은 사용자가 결과를 보고 정했다. 후 판은 `--sense-dataset`로 지정해 쓴다. 이 절은 7단계 당시의 판단이다. 기본값을 바꾸고 뜻 순서 규칙을 고친 것은 [8단계](08-default-and-sense-order.md)다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py \
  --sense-dataset apps/codemap-search/experiments/related-terms-poc/datasets/rts4-e5083ab344e1336d/dataset.sqlite3 token retry
```

후 판은 `rt-sense/4`라 5단계에서 고친 word-eval 스크립트가 그대로 읽는다. 기본값을 바꾸면 `DEFAULT_SENSE_DATASET` 한 줄과 문서의 판 이름만 바꾸면 된다.

## 7. 한계

- 평가 세트는 분야 단위로만 맞았는지 본다. 합친 주제나 새 이름이 맞는지는 재지 않는다.
- 첫 뜻이 바뀐 단어의 대부분은 가중치가 같은 뜻의 순서 규칙 때문이다(4절). 순서 규칙을 바꾸지 않는 한, 분야를 고칠 때마다 첫 뜻이 이렇게 옮겨 간다.
- 판정에 쓴 시트에는 단어가 12개까지만 보였다. 6단계 7절의 한계가 이 검증에도 그대로 남는다.
