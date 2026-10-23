# 코드에서 나온 문구를 하나의 이름으로 볼지, 단어로 나눠 볼지 판정하기

오픈소스 저장소 12곳의 식별자에서 나온 여러 단어 문구 목록을 받습니다. 문구마다 **하나의 이름으로 유지(keep)**할지, **구성 단어로 나눌지(split)** 판정하고 유형을 적어 주세요.

## 입력 항목
- `text`: 문구(소문자, 단어 사이 공백 하나)
- `dataset_status`: `missing`(사전에 없음), `relation_only`(사전에 다른 단어의 관련어로만 있음)
- `repositories`: 이 문구가 나온 저장소
- `fields`, `senses`, `expansions`: 다른 분석가가 코드 쓰임을 보고 적은 분야·뜻·약어 풀이(참고용)
- `identifiers`, `code`: 이 문구가 나온 식별자와 실제 코드 줄. 의미는 이 쓰임을 보고 판단합니다.

## 유형과 판정
| `type` | 뜻 | 판정 | 예 |
|---|---|---|---|
| `product_name` | 여러 단어로 된 제품·서비스·도구 이름 | keep | `entity framework core`, `github copilot`, `kinesis firehose` |
| `library_api_name` | 표준 라이브러리나 널리 쓰는 외부 라이브러리의 타입·함수·속성 이름을 쪼갠 것 | keep | `wstring view`(`std::wstring_view`), `unordered map`(`std::unordered_map`), `block in place`(`tokio::task::block_in_place`), `did not receive`(NSubstitute `DidNotReceive`), `strict equal`(`assert.strictEqual`) |
| `project_api_name` | 이 저장소(또는 널리 쓰이지 않는 라이브러리)가 정의한 타입·함수 이름 | split | `read buf`, `value path`, `handlers chain` |
| `established_term` | 업계에서 굳어진 기술 용어. 문서·표준·API에 그 이름으로 나옴 | `dataset_status`가 `relation_only`면 keep, `missing`이면 split | `post type`(WordPress), `state machine`, `append only file`(Redis AOF), `pseudo console`(Windows ConPTY), `graphics rendition`(SGR), `inner blocks`(Gutenberg), `blocking pool` |
| `project_feature` | 이 프로젝트 내부의 기능·화면·설정 이름 | split | `emergency access`(Bitwarden), `new tab menu`(Terminal), `tools panel`(Gutenberg), `required action`(Keycloak) |
| `composition` | 각 단어가 제 뜻 그대로 합쳐진 문구. 고유 이름 + 일반 단어도 여기에 듭니다 | split | `redis module`, `sentry app`, `subscription schedule`, `price increase`, `organization user` |

- 유형이 둘 이상에 걸치면 판정이 keep인 유형을 우선합니다. 단 `established_term`은 위 상태 조건을 따릅니다.
- 어디에도 맞지 않으면 `type`을 `other`로 하고 `note`에 이유를 적습니다. 판정은 keep/split 중 하나를 고릅니다.
- "널리 쓰는 외부 라이브러리"는 공개 문서가 있고 여러 프로젝트가 쓰는 라이브러리를 뜻합니다.

## 나눌 때의 구성 단어(`components`)
- split이면 문구를 구성 단어로 나눕니다. 문구에 있는 단어만 쓰고, 순서를 지킵니다. 약어를 풀거나 단어를 바꾸지 않습니다.
- 문구 안에 keep 유형에 해당하는 더 짧은 문구가 있으면 그것은 하나로 둡니다. 예: `entity framework core migrations` → `["entity framework core", "migrations"]`.
- keep이면 `components`는 빈 배열입니다.

## 결과 형식
JSON 배열 하나를 결과 파일에 씁니다. 입력 항목 하나당 한 항목, 입력 순서대로:
```
{"text": "redis module", "type": "composition", "decision": "split", "components": ["redis", "module"]}
{"text": "unordered map", "type": "library_api_name", "decision": "keep", "components": [], "note": "std::unordered_map"}
```
- `note`는 애매했던 경우와 `library_api_name`·`product_name`의 원래 이름을 적을 때만 씁니다.

## 작업 규칙
- 배정된 입력 파일만 읽습니다. 의미 확인이 필요하면 `code`에 적힌 저장소 파일을 열어 봐도 됩니다(저장소 루트: `{checkouts}/<저장소>`). 그 밖의 파일·디렉터리는 열지 않습니다.
- 판정은 직접 합니다. 사전·분류 데이터·다른 분석 결과를 찾아보지 않습니다.
- 입력 항목을 하나도 빠뜨리지 않습니다. 마지막에 입력 수와 결과 수, 순서가 같은지 확인합니다.
- 마지막 답변에는 항목 수와 판단이 특히 애매했던 경우만 짧게 적습니다.
