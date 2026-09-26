# Codex MCP 클라이언트가 사용하는 서버 데이터

이 문서는 이 저장소에서 MCP 서버를 만들 때 참고하는 문서입니다. OpenAI Codex가 서버의 어떤 데이터를 모델(LLM)에 넘기는지, 어떤 데이터를 무시하는지 항목별로 찾아볼 수 있습니다. 목적은 Codex가 쓰지 않는 기능은 최소한으로만 구현하고, 모델에 실제로 전달되는 부분에 노력을 모으는 것입니다.

- **기준:** Codex `codex-rs` main 브랜치 스냅샷입니다(워크스페이스 버전 `0.0.0`, CHANGELOG `Unreleased`). 2026-09-26에 소스 코드로 확인했습니다.
- **범위:** Codex 클라이언트만 다룹니다. Claude Code 같은 다른 MCP 클라이언트는 확인하지 않았습니다. 그래서 여기서 "생략 가능"으로 분류한 항목도 다른 클라이언트에서는 쓰일 수 있습니다.
- **갱신:** Codex는 자주 바뀝니다. 새 버전을 기준으로 삼을 때는 [출처](#출처)에 적힌 위치를 다시 확인하세요.

## 요약

**공들일 것.** 모델이 직접 읽는 항목입니다.

- 초기화 응답의 `instructions`
- 도구의 `name`, `description`, `inputSchema`
- 도구 호출 결과 중 `content`의 텍스트

**정확히 설정할 것.** 모델에는 보이지 않지만 실행 방식을 바꾸는 항목입니다.

- `annotations`의 `readOnlyHint`, `destructiveHint`, `openWorldHint`: 사용자 승인이 필요한지와 병렬 호출이 가능한지를 정합니다.

**생략하거나 최소화해도 되는 것.** Codex에서는 효과가 없는 항목입니다.

- 도구의 `outputSchema`, `title`, `icons`, `_meta`, `annotations.idempotentHint`
- 호출 결과의 `structuredContent`와 결과 `_meta`. `structuredContent`는 넣으면 오히려 `content`가 버려집니다. 결과의 `isError`도 모델에 전달되지 않으므로 오류 표시를 이 값에만 맡기면 안 됩니다.
- prompts, 리소스 구독, 목록 변경 알림, 로그 알림, 진행률 알림
- `tools/list` 페이지네이션
- sampling, roots. Codex가 이 기능을 지원하지 않습니다.

## 빠른 참조

"권장" 열은 다음 기준으로 표시했습니다.

- **필수:** 모델 동작에 직접 영향을 줍니다.
- **정확히:** 승인이나 실행 방식에 영향을 줍니다.
- **선택:** 일부 노출 방식이나 상황에서만 효과가 있습니다.
- **생략 가능:** Codex에서는 효과가 없습니다.

### 초기화(`initialize`)

| 항목 | Codex 처리 | 모델 전달 | 권장 |
|---|---|---|---|
| `instructions` | 도구 네임스페이스 설명과 `tool_search` 출처 목록으로 사용 | 예 | 필수: [세부 규칙](#서버-instructions) |
| `serverInfo` | 상태·UI 표시 | 아니오 | 생략 가능(이름·버전만) |
| `capabilities`의 `tools`·`resources`·`prompts` | 선언 여부를 확인하지 않음 | 아니오 | MCP 규격에 필요한 만큼만 |

### 도구 목록(`tools/list`)

| 항목 | Codex 처리 | 모델 전달 | 권장 |
|---|---|---|---|
| `name` | 영문·숫자·`_` 외 문자를 `_`로 바꿔 모델용 이름 생성 | 예 | 필수: [도구 이름](#도구-이름) |
| `description` | 그대로 전달 | 예 | 필수. 인자 제약도 여기에 적음 |
| `inputSchema` | 일부 키워드만 남기고 축소 | 일부 | 필수: [입력 스키마](#입력-스키마) |
| `annotations.readOnlyHint` | 승인 생략, 병렬 호출 허용 | 아니오 | 정확히: [승인과 병렬 호출](#승인과-병렬-호출) |
| `annotations.destructiveHint`, `openWorldHint` | 승인 판단. 값이 없으면 `true`로 봄 | 아니오 | 정확히 |
| `annotations.idempotentHint` | 사용 안 함 | 아니오 | 생략 가능 |
| `title` | 지연 노출 방식의 검색 대상, 승인 UI | 아니오 | 선택 |
| `outputSchema` | 코드 모드의 TypeScript 결과 타입에만 사용 | 코드 모드에서만 | 선택 |
| `icons`, `execution` | 사용 안 함 | 아니오 | 생략 가능 |
| `_meta` | `ui.visibility`가 있는데 `"model"`이 없으면 도구를 숨김. 나머지 키는 OpenAI 앱 커넥터 전용 | 아니오 | 생략 가능 |
| `nextCursor` | 기본 모드에서는 무시하고 첫 페이지만 읽음 | — | 생략 가능. 모든 도구를 한 번에 반환 |
| `notifications/tools/list_changed` | 로그만 남기고 목록을 다시 읽지 않음 | — | 생략 가능 |

### 도구 호출(`tools/call`)

| 항목 | Codex 처리 | 모델 전달 | 권장 |
|---|---|---|---|
| `content`의 `text` 블록 | `input_text`로 변환 | `structuredContent`가 없을 때 | 필수. 주 출력 채널 |
| `content`의 `image`·`audio` 블록 | data URL로 변환. 지원하지 않는 모델에는 안내 문구로 대체 | `structuredContent`가 없을 때 | 선택 |
| `content`의 `resource`·`resource_link` 블록 | 블록 JSON 전체를 텍스트로 전달 | 예(JSON 원문) | 선택. 가능하면 `text`로 풀어서 제공 |
| `structuredContent` | 있으면 이것만 JSON 문자열로 전달하고 `content`는 버림 | 예 | 생략 권장: [결과 변환](#결과-변환) |
| `isError` | 내부 성공 여부와 UI 상태에만 사용 | 아니오 | 오류 사실은 텍스트에 적음 |
| 결과 `_meta` | 원격 측정, OpenAI 앱 전용 처리 | 아니오 | 생략 가능 |
| `notifications/progress` | 로그만 남김 | 아니오 | 생략 가능 |
| 요청 `_meta`(Codex가 보냄) | `callId`, 턴 메타데이터, 스레드·세션 ID, 추적 헤더 등을 담아 보냄 | — | 서버는 무시해도 됨 |

### 그 밖의 MCP 기능

| 기능 | Codex 처리 | 모델 전달 | 권장 |
|---|---|---|---|
| 리소스 조회(`resources/list`, `resources/templates/list`, `resources/read`) | 내장 도구 3개를 통해 모델이 직접 호출 | 예(JSON 원문) | 선택: [리소스](#리소스) |
| `resources/subscribe`, `notifications/resources/*` | 구독을 요청하지 않고, 알림은 로그만 남김 | 아니오 | 생략 가능 |
| prompts(`prompts/list`, `prompts/get`) | 호출 안 함 | 아니오 | 생략 가능 |
| sampling, roots | 클라이언트 기능으로 선언하지 않음 | 아니오 | 사용 불가 |
| elicitation | 사용자 UI나 자동 검토 모델로 전달 | 아니오 | 사용자 확인이 꼭 필요할 때만 |
| `notifications/message`(로그) | Codex 내부 추적 로그에만 기록 | 아니오 | 생략 가능 |
| `logging/setLevel` | 호출 안 함 | — | 생략 가능 |

## 모델에 전달되는 항목의 세부 규칙

### 서버 instructions

- 네임스페이스 `mcp__<서버>`의 `description`으로 들어갑니다. 요청 최상위 지시문이나 개발자 메시지에는 들어가지 않습니다.
- 지연 노출 방식에서는 `tool_search` 도구 설명의 출처 목록에도 `- <서버 이름>: <instructions>` 형식으로 붙고, 검색 대상에 포함됩니다.
- 최대 512KiB입니다. 에이전트 플러그인 서버는 1,000바이트에서 자릅니다. 비어 있으면 `Tools in the mcp__<서버> namespace.`가 들어갑니다.
- 도구 정의와 함께 매 요청에 들어가므로 짧을수록 좋습니다.

### 도구 이름

- 영문·숫자·`_` 외의 문자는 `_`로 바뀝니다. 예를 들어 서버 `server.one`의 도구 `tool.two-three`는 네임스페이스 `mcp__server_one`, 도구 이름 `tool_two_three`가 됩니다.
- 네임스페이스와 도구 이름을 `__`로 이은 길이가 128자를 넘으면 잘라내고 해시를 붙입니다. 정제한 뒤 이름이 겹치는 경우에도 SHA-1 앞 12자리를 접미사로 붙입니다.
- 서버를 실제로 호출할 때는 원본 이름을 씁니다. 처음부터 `snake_case`로 지으면 모델이 보는 이름과 서버의 이름이 같아집니다.
- 사용자 설정의 `enabled_tools`/`disabled_tools`는 원본 이름을 기준으로 적용됩니다.

### 입력 스키마

- 모델에 남는 키워드는 `type`, `description`, `enum`, `items`, `minItems`, `properties`, `required`, `additionalProperties`, `anyOf`, `oneOf`, `allOf`, `$ref`, `$defs`, `definitions`뿐입니다.
- `format`, `pattern`, `minimum`, `maximum`, `minLength`, `default`, `title`, `examples`, `$schema` 같은 키워드는 경고 없이 빠집니다. 모델이 알아야 할 제약은 `description`에 문장으로 적어야 합니다.
- 누락된 부분은 자동으로 보정합니다. `properties`가 없으면 `{}`를 넣고, `type`이 없으면 다른 키워드로 추론하며, `const`는 값이 하나인 `enum`으로 바꿉니다.
- 스키마가 5,000바이트를 넘으면 기준 아래로 내려갈 때까지 다음 순서로 압축합니다.
  1. 모든 `description`을 제거합니다.
  2. `$defs` 같은 정의를 제거합니다.
  3. 깊이 3 이상을 접습니다.
  4. `anyOf`·`oneOf`·`allOf`를 정리합니다.

  첫 단계에서 인자 설명이 모두 사라지므로 스키마를 5,000바이트 아래로 유지하는 편이 좋습니다. 이 기준은 사용자가 서버별 `tool_input_schema_max_bytes` 설정으로 바꿀 수 있습니다.
- 스키마는 항상 `strict: false`로 전달됩니다. 모델이 스키마를 정확히 지킨다고 가정하지 말고 서버에서 인자를 검증해야 합니다.

아래는 코드에서 재구성한 예시입니다. 실제 요청을 캡처한 것은 아닙니다. 서버 `my-server`의 `instructions`가 `"Weather tools. Prefer metric units."`이고 도구 정의가 다음과 같다고 가정합니다.

```json
{"name":"get-forecast","title":"Get Forecast","description":"Get a forecast for a city",
 "inputSchema":{"type":"object","required":["city"],
   "properties":{"city":{"type":"string","description":"City name","minLength":1},
                 "days":{"type":"integer","minimum":1,"maximum":7,"default":3},
                 "units":{"const":"metric"}}},
 "outputSchema":{"type":"object","properties":{"temp":{"type":"number"}}},
 "annotations":{"readOnlyHint":true}}
```

직접 노출 방식에서 모델이 받는 도구 정의는 다음과 같습니다. `title`, `outputSchema`, `annotations`, `minLength`, `minimum`, `maximum`, `default`는 사라집니다.

```json
{"type":"namespace","name":"mcp__my_server","description":"Weather tools. Prefer metric units.",
 "tools":[{"type":"function","name":"get_forecast","description":"Get a forecast for a city","strict":false,
   "parameters":{"type":"object",
     "properties":{"city":{"type":"string","description":"City name"},
                   "days":{"type":"integer"},
                   "units":{"type":"string","enum":["metric"]}},
     "required":["city"]}}]}
```

### 승인과 병렬 호출

Codex의 기본 승인 모드인 `Auto`는 아래 조건을 위에서부터 차례로 적용합니다.

| 조건 | 사용자 승인 |
|---|---|
| `destructiveHint: true` | 필요 |
| `readOnlyHint: true` | 필요 없음 |
| `destructiveHint`와 `openWorldHint`가 모두 `false` | 필요 없음 |
| 그 외(값이 없으면 `true`로 봄) | 필요 |

- 사용자는 `[mcp_servers.<서버>.tools.<도구>]`의 `approval_mode`로 모드를 바꿀 수 있습니다.
  - `writes`: `readOnlyHint: true`가 아니면 승인을 받습니다.
  - `prompt`: 항상 승인을 받습니다.
  - `approve`: 승인을 받지 않습니다.
  - 전체 승인 정책에 따라 승인이 생략될 수도 있습니다.
- `readOnlyHint: true`인 도구는 병렬 호출이 허용됩니다.
- 따라서 읽기 전용 도구에는 `readOnlyHint: true`를 붙여야 승인 창 없이 병렬로 실행됩니다. `annotations`를 비워 두면 `Auto` 모드에서 모든 호출에 승인을 요구합니다.

### 결과 변환

모델이 받는 `function_call_output`의 `output`은 다음 규칙으로 정해집니다.

| 조건 | 모델이 받는 `output` |
|---|---|
| `structuredContent`가 있고 `null`이 아님 | `structuredContent`의 JSON 문자열 하나. `content`는 텍스트와 이미지 모두 버림 |
| 그 외 | `content` 블록을 변환한 배열 |

예외가 하나 있습니다. `content`의 `text` 블록에 Codex 전용 표시 `_meta["codex/encryptedContent"]: true`가 있으면 `structuredContent`보다 `content` 배열이 우선합니다. 일반 서버와는 관계없는 경우입니다.

`content` 블록은 다음과 같이 변환됩니다.

| MCP 블록 | 모델 입력 항목 |
|---|---|
| `text` | `{"type":"input_text","text":…}` |
| `image` | `{"type":"input_image","image_url":"data:<mimeType>;base64,<data>","detail":"high"}`. `detail`은 블록의 `_meta["codex/imageDetail"]`로 바꿀 수 있음 |
| `audio` | `{"type":"input_audio","audio_url":"data:<mimeType>;base64,<data>"}` |
| `resource`, `resource_link`, 알 수 없는 형식 | 블록 전체의 JSON 문자열을 `input_text`로 |

변환 뒤에는 다음 후처리를 거칩니다.

- 맨 앞에 `Wall time: <초> seconds\nOutput:` 머리글이 붙습니다.
- 출력은 [한도](#한도와-기본값)에 맞춰 잘립니다.
  - 텍스트는 가운데를 잘라내고 `…N tokens truncated…`로 표시합니다.
  - 배열이면 예산을 넘은 텍스트 항목을 `[omitted N text items ...]`로 바꿉니다. 이미지는 예산에 포함하지 않습니다.
- 이미지 입력을 지원하지 않는 모델에는 이미지 블록 대신 `<image content omitted because you do not support image input>`이 들어갑니다.
- `isError`는 전달되지 않습니다. 성공과 실패가 같은 모양으로 전달되므로, 실패했다면 텍스트에 원인을 분명히 적어야 합니다.

아래는 코드에서 재구성한 예시입니다. 서버가 다음 결과를 돌려준다고 가정합니다.

```json
{"content":[{"type":"text","text":"Rendered chart for Q3"},
            {"type":"image","data":"iVBORw0KGgo...","mimeType":"image/png"}],
 "structuredContent":{"quarter":"Q3","total":1234},
 "isError":false}
```

`structuredContent`가 있으면 텍스트와 이미지가 모두 사라집니다.

```json
{"type":"function_call_output","call_id":"call_abc123",
 "output":"Wall time: 0.0421 seconds\nOutput:\n{\"quarter\":\"Q3\",\"total\":1234}"}
```

같은 결과에서 `structuredContent`를 빼면 다음과 같습니다.

```json
{"type":"function_call_output","call_id":"call_abc123",
 "output":[{"type":"input_text","text":"Wall time: 0.0421 seconds\nOutput:"},
           {"type":"input_text","text":"Rendered chart for Q3"},
           {"type":"input_image","image_url":"data:image/png;base64,iVBORw0KGgo...","detail":"high"}]}
```

### 리소스

- 모델이 쓰는 내장 도구는 세 개입니다: `list_mcp_resources(server?, cursor?)`, `list_mcp_resource_templates(server?, cursor?)`, `read_mcp_resource(server, uri)`.
- MCP 서버가 하나라도 설정돼 있으면 등록됩니다. 서버가 `resources` 기능을 선언했는지는 확인하지 않습니다.
- `server`를 생략하면 준비된 모든 서버에 병렬로 요청해 모든 페이지를 모읍니다. 이때 실패한 서버는 결과에서 조용히 빠지며, 리소스를 구현하지 않은 서버도 여기에 포함됩니다. `server`를 지정하면 한 페이지와 `nextCursor`를 돌려줍니다.
- 결과는 `_meta`와 `annotations`를 포함한 JSON 문자열 그대로 전달되고, 도구 출력과 같은 한도로 잘립니다. `blob`은 base64 문자열 그대로라 이미지로 변환되지 않습니다.
- `tool_search` 도구 설명은 모델에게 MCP 도구를 찾을 때 리소스 도구 대신 `tool_search`를 쓰라고 안내합니다.
- 모델이 꼭 봐야 할 정보는 리소스보다 도구 결과로 제공하는 편이 확실합니다.

## 도구 노출 방식

Codex는 모델 설정에 따라 MCP 도구를 세 가지 방식으로 보여 줍니다. 어느 방식이든 핵심은 `name`, `description`, `inputSchema`이고, 일부 필드는 방식에 따라 효과가 달라집니다.

| 방식 | 적용 조건 | 모델이 보는 것 | 추가로 쓰이는 필드 |
|---|---|---|---|
| 직접 노출 | 모델이 검색 도구를 지원하지 않음 | 요청 `tools` 배열의 네임스페이스와 함수 정의 | — |
| 지연 노출(`tool_search`) | 모델이 검색 도구를 지원(`supports_search_tool`) | `tool_search` 도구 하나. 모델이 검색하면 일치한 도구 정의를 받음 | 검색 대상: 이름, `title`, `description`, `instructions`, 인자 이름 |
| 코드 모드 | 모델의 `tool_mode`가 코드 모드 | `exec` 도구 안의 TypeScript 선언 | `outputSchema`가 결과 타입으로 표시됨 |

기준 스냅샷에 들어 있는 모델 10개는 모두 검색 도구를 지원합니다. 그중 9개가 `code_mode_only`이고 `gpt-5.5`만 예외입니다. 따라서 실제로는 지연 노출과 코드 모드가 기본 경로입니다.

## 한도와 기본값

| 항목 | 기본값 | 바꾸는 설정(사용자 쪽) | 넘으면 |
|---|---|---|---|
| 서버 기동(연결·초기화) | 30초 | `startup_timeout_sec` | 서버 시작 실패 |
| 도구 호출 | 300초 | `tool_timeout_sec` | 모델에 `timed out awaiting tools/call after …` 오류 텍스트가 전달됨 |
| 네임스페이스+도구 이름 길이 | 128자 | — | 잘라내고 해시 접미사를 붙임 |
| 입력 스키마 크기 | 5,000바이트 | `tool_input_schema_max_bytes`(서버별) | 설명부터 제거하며 압축 |
| `instructions` 크기 | 512KiB(에이전트 플러그인 1,000바이트) | — | 잘라냄 |
| 도구 출력 크기 | 모델의 잘라내기 정책 × 1.2(기준 스냅샷의 모델은 모두 10,000토큰) | `output_token_limit`(도구별), `tool_output_token_limit`(전역) | 가운데를 잘라냄 |

## 출처

모든 경로는 Codex 저장소의 `codex-rs/` 기준이며, 줄 번호는 기준 스냅샷의 것입니다.

| 주제 | 위치 |
|---|---|
| 초기화 요청, `instructions` 저장 | `codex-mcp/src/rmcp_client.rs:1105-1128`, `codex-mcp/src/rmcp_client.rs:840-860` |
| 네임스페이스 설명 생성 | `core/src/tools/handlers/mcp.rs:496-529` |
| `tool_search` 출처 목록 | `core/src/tools/handlers/mcp.rs:161-185`, `core/src/tools/handlers/tool_search_spec.rs:87` |
| 도구 목록 페이지네이션 | `codex-mcp/src/rmcp_client.rs:665-685` |
| `ui.visibility` 필터 | `codex-mcp/src/connection_manager/tool_catalog.rs:37-61` |
| 이름 정제와 길이 제한 | `codex-mcp/src/mcp/mod.rs:575-590`, `codex-mcp/src/tools.rs:113-316` |
| 입력 스키마 변환 | `tools/src/mcp_tool.rs:38-78`, `tools/src/json_schema/types.rs:36-75`, `tools/src/json_schema/compaction.rs:15-38`, `tools/src/responses_api.rs:36-44`, `tools/src/responses_api.rs:164-173` |
| 승인과 병렬 호출 | `core/src/mcp_tool_call.rs:2442-2473`, `core/src/tools/handlers/mcp.rs:148-159`, `config/src/mcp_types.rs:28-34`, `config/src/mcp_types.rs:85-93` |
| 결과 변환 | `protocol/src/models.rs:2250-2263`, `protocol/src/models.rs:2302-2444`, `core/src/tools/context.rs:185-214`, `core/src/mcp_tool_call.rs:927-964` |
| 리소스 도구 | `core/src/tools/spec_plan.rs:1132-1145`, `core/src/tools/handlers/mcp_resource.rs:354-371` |
| 알림 처리 | `rmcp-client/src/logging_client_handler.rs:52-140` |
| 도구 노출 방식 | `core/src/mcp_tool_exposure.rs:85-89`, `models-manager/models.json` |
| 한도 상수 | `codex-mcp/src/rmcp_client.rs:105-106`, `codex-mcp/src/tools.rs:226`, `core/src/tools/handlers/mcp.rs:48-49`, `tools/src/json_schema/compaction.rs:15` |
