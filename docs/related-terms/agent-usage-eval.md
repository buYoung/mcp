# 에이전트의 조회 도구 사용 평가

코딩 에이전트가 사용법만 받고 뜻 조회 CLI(`sense_lookup.py`)를 제대로 쓰는지 보는 블라인드 평가다. 핵심 질문은 두 가지다.

- 식별자를 단어로 나눠 각 단어를 입력으로 넣는가. 예를 들어 `appVersion`이면 `app`, `version`을 넣는가.
- CLI가 낸 관련어를 답에 쓰는가.

이 도구의 목적은 식별자를 단어로 나눠 단어마다 유의어를 받는 것이다. 그래서 위 두 가지를 잰다.

작업일: 2026-10-17. 뜻 데이터셋: `rts4-e5083ab344e1336d`. 뜻 순서: [작업 이력 10단계](sense-mapping-log/10-sense-order-rule.md)의 규칙 A.

## 1. 스크립트

`apps/codemap-search/experiments/related-terms-poc/sense-eval/agent_usage_eval.py`

```sh
# 기본: 두 모델 × 식별자 12개 × 기본 사용법(agent-usage/usage-short.md)
python3 apps/codemap-search/experiments/related-terms-poc/sense-eval/agent_usage_eval.py

# 사용법 여러 개 비교, 식별자 직접 지정
python3 .../agent_usage_eval.py --usage .../agent-usage/usage.md .../agent-usage/usage-short.md
python3 .../agent_usage_eval.py --identifiers appVersion,UsePasswordManager

# 채점 규칙만 바꿨을 때 이전 실행 다시 채점
python3 .../agent_usage_eval.py --rescore .codemap/poc/related-terms/agent-usage/<실행>
```

| 항목 | 내용 |
| --- | --- |
| 에이전트 | `pi -p --mode json`. bash 도구만 켜고 세션은 저장하지 않는다. 문맥 파일, 스킬, MCP, 프롬프트 템플릿은 끈다. 확장은 켠다(`claude-bridge` 공급자가 확장이다) |
| 모델 | `claude-bridge/claude-haiku-5-5`, `openai-codex/gpt-6-luna`(`--models`로 바꿈) |
| 사용법 | 시스템 프롬프트 끝에 붙인다(`--append-system-prompt`). CLI는 `synonyms`라는 짧은 명령으로 부를 수 있게 감싼다 |
| 과제 | "`<식별자>`와 같은 개념을 다른 이름으로 쓴 코드까지 grep하려 한다. grep 키워드를 10개까지 한 줄에 하나씩 답하라." 평가 세트의 식별자는 코드 줄을 함께 준다 |
| 블라인드 | 과제는 CLI 사용을 언급하지 않는다. 에이전트는 저장소 밖 임시 폴더의 빈 폴더에서 돌린다 |
| 식별자 | `appVersion`과, 평가 세트에서 두 단어 이상인 식별자 11개(시드 20261017) |
| 출력 | 세션 원본은 `.codemap/poc/related-terms/agent-usage/<실행>/runs/`, 채점 결과는 같은 폴더의 `results.json`(git 추적 안 함) |

채점 항목(실행마다):

| 항목 | 뜻 |
| --- | --- |
| `called` | `synonyms`(또는 `sense_lookup.py`)를 실행했다 |
| `coverage` | 식별자 단어 중 따로 입력한 비율. 단어는 사전 조회와 같은 규칙(`syntax_parts.subparts`)으로 나눈다. `get`, `has`, `use` 같은 접두어와 숫자는 빼고, 단수·복수는 같은 단어로 본다 |
| `split` | `coverage`가 1이다 |
| `raw` / `phrase` | 식별자를 통째로(`appVersion`) / 단어를 한 따옴표로 묶어(`'app version'`) 넣었다 |
| `from_tool` | 답의 키워드(식별자 단어 제외) 중 CLI 출력에 있는 비율 |
| 사용법 토큰 | "Reply with OK." 한 줄 요청의 프롬프트 토큰을 사용법 있을 때와 없을 때 비교한 차이 |

## 2. 사용법 판

| 판 | 내용 | 토큰(haiku / luna) |
| --- | --- | ---: |
| `usage-bare.md` | 기능, 쓸 때, 인자 규칙. 식별자 분리 지시 없음 | 123 / 82 |
| `usage.md` | bare + "식별자를 단어로 나눠 넣는다"와 예시 | 145 / 97 |
| `usage-compact.md` | 줄인 판. 쓸 때를 "다른 이름이 필요할 때"로만 적음 | 108 / 72 |
| `usage-short.md`(기본) | 쓸 때 "(search keywords, naming)"는 남기고 출력 설명을 뺌 | 104 / 67 |

`usage-short.md` 전문:

```text
`synonyms TERM...`: code thesaurus. Use it whenever you need other names for a code concept (search keywords, naming).
One arg per term. Split identifiers into words: getUserName → `synonyms get user name`. Quote multi-word terms: 'abort signal'.
```

## 3. 결과

세 번 실행했다(`run-1`, `run-2`, `run-3`). 한 번에 모델마다 식별자 12개다. `usage.md`는 세 번, `usage-bare.md`는 두 번, 나머지는 한 번 돌렸다.

| 모델 | 사용법 | 실행 수 | 호출 | 분리 | 단어 포함률 | 통째 입력 | 답에 쓴 출력 | 도구 출력(자) | 프롬프트 토큰 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| haiku | bare | 24 | 88% | 4% | 0.17 | 33% | 0.31 | 13,136 | 21,761 |
| haiku | compact | 12 | 33% | 17% | 0.30 | 0% | 0.18 | 7,692 | 13,492 |
| haiku | `usage.md` | 36 | 92% | 64% | 0.84 | 0% | 0.41 | 29,053 | 28,363 |
| haiku | short | 12 | 83% | 67% | 0.71 | 0% | 0.39 | 16,781 | 23,406 |
| luna | bare | 24 | 67% | 8% | 0.12 | 25% | 0.31 | 22,145 | 7,995 |
| luna | compact | 12 | 42% | 33% | 0.40 | 0% | 0.21 | 8,583 | 3,845 |
| luna | `usage.md` | 36 | 89% | 56% | 0.76 | 0% | 0.38 | 29,005 | 9,608 |
| luna | short | 12 | 92% | 58% | 0.79 | 0% | 0.30 | 46,414 | 8,426 |

- **분리 지시가 없으면 거의 나누지 않는다.** bare에서 단어로 다 나눈 실행은 4~8%다. 대신 식별자를 통째로 넣거나(25~33%), `'min version'`, `'bank account'`처럼 자기가 고른 구를 넣는다.
- **쓸 때를 구체적으로 적지 않으면 부르지 않는다.** compact는 "(search keywords, naming)"를 뺐더니 호출이 33~42%로 떨어졌다. 첫 시험판은 쓸 때를 아예 적지 않았는데, `appVersion` 네 번 중 세 번은 도구를 부르지 않았다.
- **short는 `usage.md`와 같은 수준이고 토큰은 28~31% 적다.** 그래서 short를 기본으로 했다. 다만 실행 수가 모델마다 12번이고, 같은 `usage.md`도 실행에 따라 haiku 분리율이 83%, 67%, 42%로 흔들렸다. 차이가 몇 %p라면 구별되지 않는다.
- **도구 출력이 토큰을 대부분 차지한다.** 사용법은 70~150토큰인데, 도구 출력은 실행마다 평균 8,000~46,000자다. 단어로 나눠 넣을수록 단어마다 모든 뜻의 관련어가 나와 출력이 커진다. 예를 들어 `app version`을 따로 넣으면 관련어가 1,611개 나온다.
- **답에 쓴 출력은 30~40%다.** 에이전트는 CLI 출력 일부만 쓰고 나머지 키워드는 스스로 만든다.
- **분리가 잘 안 되는 식별자가 따로 있다.**
  - 긴 snake_case(`test_should_get_all_rule_objects_for_a_store`): 일부 단어를 뺀다.
  - 약어가 붙은 이름(`LS3_LockingShift`, `DECKPNM_KeypadNumericMode`): `'locking shift'`처럼 구로 넣는다.
- `gpt-6-luna`는 키워드를 스스로 더 생각해 입력에 많이 덧붙인다(한 호출에 8~20개). 그만큼 출력이 커진다.

## 4. 한계

- 과제가 "grep 키워드 만들기" 한 종류다. 이름 짓기나 코드 탐색 중의 사용은 재지 않았다.
- 모델당 사용법당 12~36번이라 몇 %p 차이는 판별할 수 없다.
- 확장을 켜 둔 상태다. 사용자 환경의 `pi` 확장(예: 명령 검토 확장)이 결과에 영향을 줄 수 있다.
- 첫 실행(`run-1`)은 작업 폴더가 저장소 안이었다. 실행 하나가 폴더를 뒤졌지만 평가 자료에는 닿지 않았다. 이후 작업 폴더를 임시 폴더로 옮겼다.
- 키워드가 맞는지(답의 질)는 판정하지 않았다. `from_tool`은 출력을 썼는지만 본다.
