# 파생어·약어가 기본형의 사전 뜻을 이어받아도 되는지 확인하기

오픈소스 저장소 12곳의 코드에서 나온 파생어(`deselect` → `select`)와 약어(`pfn` → `function pointer`)를 개발 용어 사전에 넣으려 합니다. 사전은 **분야 → 주제 → 개념 → 단어**로 묶여 있습니다. 파생어·약어는 기본형(원래 말)이 사전에 가진 뜻을 **모두** 이어받습니다. 그런데 기본형과 철자만 같고 대상이 다르면 엉뚱한 뜻이 붙습니다. 예를 들어 `disarm`의 `arm`은 "작동 준비"인데, 사전의 `arm`은 CPU 아키텍처 `ARM`뿐입니다.

항목마다 아래를 정해 주세요.

## 입력 항목
- `text`: 파생어·약어(소문자)
- `type`: `derived`(파생어) 또는 `abbreviation_common`(널리 쓰는 약어)
- `source`: 기본형(파생어) 또는 원래 말(약어)
- `source_senses`: 기본형이 지금 사전에 가진 뜻. 뜻마다 분야, 주제, 개념 id·이름, 같은 개념의 단어
- `identifiers`, `code`, `repositories`: 이 단어가 나온 식별자와 실제 코드 줄
- `analyst_senses`, `analyst_note`: 앞선 분석가가 적은 자리와 메모(참고용, 비어 있을 수 있음)

## 판정
1. `inherit`
   - `source_senses` 가운데 하나라도 코드 쓰임과 **같은 대상**을 가리키면 `true`입니다. 분야 이름이 달라도 같은 대상이면 `true`입니다.
   - 하나도 같은 대상을 가리키지 않으면(철자만 같은 다른 단어 등) `false`입니다.
2. `senses`: `source_senses`가 다루지 않는 코드 쓰임마다 하나씩 자리를 적습니다. 없으면 빈 배열입니다.
   - `inherit: false`이면 코드 쓰임 전부를 여기에 적어야 합니다. 비워 두면 단어가 사전에 들어가지 않습니다.
   - 그 쓰임이 한 저장소 안에서만 쓰는 약어·고유 이름이거나 뜻이 분명하지 않은 조각이면 적지 않습니다.
   - `analyst_senses`의 자리가 맞으면 그대로 써도 됩니다.
3. `note`: `inherit: false`이거나 `senses`를 적었을 때만 이유를 짧게 씁니다.

## 자리 고르기 (`senses[].attach`)
아래 순서로 찾습니다. 각 뜻에 `meaning`(영어 10단어 안팎)을 적습니다.
1. **기존 개념**: `{"concept": 663}`
2. **기존 주제 안의 새 개념**: `{"topic": "t193.2", "new_concept": "websocket server"}`
3. **새 주제**: `{"new_topic": {"key": "windows-runtime-types", "label": "Windows Runtime types", "scope": "한 줄 범위 설명(영어)", "fields": ["mobile-desktop", "type-systems"]}, "new_concept": "winrt"}`. 분야는 1~3개이며 첫째가 주 분야입니다.

사전 목록은 `{catalog}/`에 있습니다. 셸에서 `rg`로 찾습니다.
- `concepts.jsonl`: 개념 하나당 한 줄(`concept` id, `name`, `topic`, `topic_label`, `fields`, `words`)
- `topics.jsonl`: 주제 하나당 한 줄(`topic` id, `label`, `scope`, `fields`, `concepts`)
- `fields.json`: 분야 39개

## 결과 형식
JSON 배열 하나를 결과 파일에 씁니다. 입력 항목 하나당 한 항목, 입력 순서대로:
```
{"text": "deselect", "inherit": true, "senses": []}
{"text": "disarm", "inherit": false, "senses": [{"meaning": "cancel an armed drop guard so it no longer fires", "attach": {"concept": 1234}}], "note": "사전의 arm은 CPU 아키텍처 ARM뿐이다"}
{"text": "trimmed", "inherit": true, "senses": [{"meaning": "keys removed from a redis node after slot migration", "attach": {"topic": "t88.1", "new_concept": "slot trimming"}}], "note": "문자열 공백 제거는 trim에서 이어받는다"}
```
- `attach`의 개념 id·주제 id는 목록 파일에 실제로 있는 값만 씁니다.

## 작업 규칙
- 배정된 입력 파일과 `{catalog}/`의 목록만 읽습니다. 의미 확인이 필요하면 `code`에 적힌 저장소 파일을 열어 봐도 됩니다(저장소 루트: `{checkouts}/<저장소>`). 그 밖의 파일·디렉터리는 열지 않습니다.
- 판정은 직접 합니다. 다른 분석 결과를 찾아보지 않습니다.
- 입력 항목을 하나도 빠뜨리지 않습니다. 마지막에 입력 수와 결과 수, 순서가 같은지, `attach`의 id가 목록에 있는지 확인합니다.
- 마지막 답변에는 항목 수, `inherit: false` 수, 판단이 특히 애매했던 경우만 짧게 적습니다.
