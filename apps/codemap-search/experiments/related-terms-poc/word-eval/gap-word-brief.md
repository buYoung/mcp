# 코드에서 쓰는 단어를 개발 용어 사전에 넣을지 정하고, 넣을 자리 고르기

오픈소스 저장소 12곳의 코드에서 나온 단어 목록을 받습니다. 개발 용어 사전은 **분야 → 주제 → 개념 → 단어**로 묶여 있습니다. 한 단어는 뜻마다 다른 개념에 들어갈 수 있습니다. 단어마다 사전에 넣을지 정하고, 넣는다면 어느 개념에 넣을지 골라 주세요.

## 입력 항목
- `text`: 단어 또는 문구(소문자)
- `kind`
  - `word`: 사전에 없거나(`missing`) 다른 단어의 관련어로만 있는(`relation_only`) 단어
  - `sense_gap`: 사전에 뜻이 있지만, 코드에서의 쓰임이 그 뜻들과 다를 수 있는 단어
- `dataset_status`: `missing`, `relation_only`, `head`(자기 뜻 있음), `input`(다른 단어의 뜻을 통해 분류됨)
- `repositories`: 이 단어가 나온 저장소와, 다른 판정자가 본 사전 응답의 일치 정도(`match`, `partial`, `none`, `no_result`)
- `fields`, `senses`, `expansions`: 다른 분석가가 코드 쓰임을 보고 적은 분야·뜻·약어 풀이(참고용)
- `identifiers`, `code`: 이 단어가 나온 식별자와 실제 코드 줄. 의미는 이 쓰임을 보고 판단합니다.
- `current_senses`(`sense_gap`, `input`): 지금 사전에 있는 뜻. 뜻마다 분야, 주제, 개념 id·이름, 같은 개념의 단어
- `relation_evidence`(`relation_only`): 이 단어를 관련어로 둔 사전 단어와 그 단어의 주제·개념

## 사전 찾아보기
사전 목록은 `{catalog}/`에 있습니다. 셸에서 `rg`로 찾습니다.
- `concepts.jsonl`: 개념 하나당 한 줄(`concept` id, `name`, `topic`, `topic_label`, `fields`, `words`). 예: `rg -i '"websocket' {catalog}/concepts.jsonl`
- `topics.jsonl`: 주제 하나당 한 줄(`topic` id, `label`, `scope`, `fields`, `concepts`)
- `fields.json`: 분야 39개(`field`, `label`, `scope`)
- `terms-with-senses.txt`: 자기 뜻이 있는 사전 단어 목록(한 줄에 하나). 대소문자·구분자 차이는 같은 단어로 봅니다.

## 1. 유형과 판정 (`kind: word`)
| `type` | 뜻 | `decision` | 예 |
|---|---|---|---|
| `external_api` | 외부 라이브러리·플랫폼·API·제품 이름 | add | `winrt`, `dxgi`, `d3d11`, `aspnetcore`, `autofixture`, `resteasy`, `hstring` |
| `product_name` | 여러 단어로 된 제품·서비스 이름 | add | `entity framework core`, `kinesis firehose` |
| `library_api_name` | 표준·널리 쓰는 라이브러리의 타입·함수 이름 | add | `unordered map`, `wstring view`, `did not receive` |
| `project_name` | 이 저장소 고유의 프로젝트·모듈·내부 자료구조 이름 | skip | `snuba`, `seer`, `cascadia`, `conhost`, `listpack`, `sds`, `robj`, `vrl` |
| `abbreviation_common` | 여러 프로젝트에서 널리 쓰는 약어 | add, `expansion`에 원래 말 | `pfn`(function pointer), `lparam`, `ciba`, `caep`, `oid4vc`, `cidrs` |
| `abbreviation_local` | 이 저장소 안에서만 쓰는 약어 | skip | `sval`, `mstr`, `slen`, `targ`, `evts` |
| `derived` | 파생어·활용형(복수형, 과거형, `-er`/`-or`/`-able`/`-tion`, `de-`/`un-`/`re-` 등) | add, `root`에 기본형 | `deselect`→`select`, `invalidator`→`invalidate`, `woken`→`wake`, `families`→`family` |
| `business_term` | 업무 분야 명사 | add | `subscription`, `invoice`, `coupon`, `taxes`, `cohorts` |
| `fragment_clear` | 한 글자·숫자·코드 조각 중 코드에서의 뜻이 분명한 것 | add | `p`(HTML 문단), `g`(SVG 그룹), `mm256`(AVX 256비트) |
| `fragment_unclear` | 뜻이 분명하지 않은 조각: 관례적 변수 이름, 숫자, 버전·포트·날짜, 티켓 번호 | skip | `2020`, `16`, `pm35215`, `80x24` |
| `general_term` | 위에 들지 않는 개발 용어나 일반 단어로, 코드 쓰임이 개발 분야로 분류되는 것 | add | `inspectable`, `orphanage`, `waiters` |
| `not_categorizable` | 그 쓰임에서 개발 개념을 가리키지 않음 | skip | |

- `derived`는 어근이 사전에 없더라도 `derived`로 적고 `root`를 씁니다. `abbreviation_common`도 같습니다.
- 유형이 애매하면 가장 가까운 것을 고르고 `note`에 이유를 씁니다.

## 2. 뜻 보강 판정 (`kind: sense_gap`)
- `current_senses` 가운데 하나가 코드 쓰임과 **같은 대상**을 가리키면 `decision: covered`이고 `covered_by`에 그 개념 id를 적습니다. 분야 이름이 달라도 같은 대상이면 covered입니다.
- 어느 뜻도 코드 쓰임을 가리키지 않으면 `decision: add`이고 아래 3번처럼 새 뜻의 자리를 고릅니다. `type`은 `sense_gap`으로 둡니다.
- 저장소마다 쓰임이 다르면 쓰임마다 따로 판단합니다. 하나라도 빠졌으면 add이고, 빠진 쓰임만 `senses`에 넣습니다.
- 빠진 쓰임이 1번 표의 skip 유형(`project_name`, `abbreviation_local`, `fragment_unclear`, `not_categorizable`)에 해당하면 그 쓰임은 넣지 않습니다. 예: 한 저장소 안의 지역 변수 약어(`bi` = block index), 그 저장소 고유의 자료구조 이름(`lp` = Redis listpack).
  - 빠진 쓰임이 모두 skip 유형이면 `decision: skip`이고, `type`에 그 유형을 적고 `note`에 쓰임을 적습니다.
  - 일부만 skip 유형이면 `decision: add`로 하고 나머지 쓰임만 `senses`에 넣습니다. 뺀 쓰임은 `note`에 적습니다.

## 3. 넣을 자리 고르기 (`decision: add`)
`senses`에 코드 쓰임의 뜻마다 하나씩 적습니다. 대부분 하나입니다.
- `derived`는 `root`가, `abbreviation_common`은 `expansion`이 `terms-with-senses.txt`에 있으면 그 단어의 뜻을 그대로 이어받습니다. 이때는 `senses`를 비워 둡니다. 없으면 직접 자리를 고릅니다.
- 단, 기본형·원래 말의 사전 뜻 가운데 코드 쓰임과 같은 대상이 하나도 없으면(예: `disarm`의 `arm`은 "무장·작동 준비"인데 사전의 `ARM`은 CPU 아키텍처뿐) 이어받지 않습니다. 이때는 `senses`에 직접 자리를 적고 `note`에 이유를 씁니다. 기본형의 사전 뜻은 `concepts.jsonl`에서 `rg -i '"<기본형>"'`로 확인합니다.
- 기본형의 사전 뜻이 일부 쓰임과 맞고, 맞지 않는 다른 쓰임도 있으면 `"inherit": true`를 쓰고 `senses`에는 맞지 않는 쓰임만 적습니다. 예: `trimmed`는 문자열 공백 제거는 `trim`에서 이어받고, Redis 키 제거 쓰임만 `senses`에 적습니다.

자리는 아래 순서로 찾습니다.
1. **기존 개념**: 개념의 단어들과 사실상 같은 것을 가리키거나 늘 함께 쓰이는 묶음이면 그 개념에 넣습니다. `{"concept": 663}`
2. **기존 주제 안의 새 개념**: 맞는 주제는 있지만 맞는 개념이 없을 때입니다. `{"topic": "t193.2", "new_concept": "websocket server"}`
3. **새 주제**: 맞는 주제가 없을 때입니다. 분야는 1~3개 고르고 첫째가 주 분야입니다. `{"new_topic": {"key": "windows-runtime-types", "label": "Windows Runtime types", "scope": "한 줄 범위 설명(영어)", "fields": ["mobile-desktop", "type-systems"]}, "new_concept": "winrt"}`
4. **새 분야**: 맞는 분야가 39개 중에 없을 때만 씁니다. `{"new_field": {"field": "game-development", "label": "Game development", "scope": "..."}}`를 같은 뜻의 `new_topic.fields`에 그 id로 함께 씁니다.

- 같은 묶음 안에서 여러 단어가 같은 새 개념·주제에 들어가면 같은 `new_concept` 이름과 같은 `new_topic.key`를 씁니다.
- 각 뜻에 `meaning`(영어 10단어 안팎)을 적습니다.

## 결과 형식
JSON 배열 하나를 결과 파일에 씁니다. 입력 항목 하나당 한 항목, 입력 순서대로:
```
{"text": "winrt", "kind": "word", "type": "external_api", "decision": "add",
 "senses": [{"meaning": "Windows Runtime API surface used from C++/WinRT", "attach": {"new_topic": {...}, "new_concept": "winrt"}}]}
{"text": "deselect", "kind": "word", "type": "derived", "decision": "add", "root": "select", "senses": []}
{"text": "sval", "kind": "word", "type": "abbreviation_local", "decision": "skip", "senses": [], "note": "string value in redis config parsing"}
{"text": "recorder", "kind": "sense_gap", "type": "sense_gap", "decision": "add",
 "senses": [{"meaning": "test helper that records HTTP responses", "attach": {"concept": 1234}}]}
{"text": "uri", "kind": "sense_gap", "type": "sense_gap", "decision": "covered", "covered_by": 812, "senses": []}
{"text": "bi", "kind": "sense_gap", "type": "abbreviation_local", "decision": "skip", "senses": [], "note": "block index local variable in redis"}
```
- `root`, `expansion`, `inherit`, `covered_by`, `note`는 해당할 때만 씁니다.
- `attach`의 개념 id·주제 id는 목록 파일에 실제로 있는 값만 씁니다.

## 작업 규칙
- 배정된 입력 파일과 `{catalog}/`의 목록만 읽습니다. 의미 확인이 필요하면 `code`에 적힌 저장소 파일을 열어 봐도 됩니다(저장소 루트: `{checkouts}/<저장소>`). 그 밖의 파일·디렉터리는 열지 않습니다.
- 판정은 직접 합니다. 다른 분석 결과를 찾아보지 않습니다.
- 입력 항목을 하나도 빠뜨리지 않습니다. 마지막에 입력 수와 결과 수, 순서가 같은지, `attach`의 id가 목록에 있는지 확인합니다.
- 마지막 답변에는 항목 수, 새로 만든 주제·분야 수, 판단이 특히 애매했던 경우만 짧게 적습니다.
