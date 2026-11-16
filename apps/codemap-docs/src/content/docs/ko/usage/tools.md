---
title: 탐색 도구 사용
description: initial_instructions, search, find, grep, read, analyze, 선택형 synonyms 사용법과 read ranges, workspace_scope, 중복 제거, 제외 규칙, 선택적 Jev 판단 단계를 설명합니다.
---

## 탐색 도구 요약

| 도구 | 용도 | 주요 인자 |
|---|---|---|
| `initial_instructions` | 탐색 안내를 불러오고, 선택적 Jev 관련성 필터가 켜져 있으면 사용자 작업도 등록 | `task`(Jev가 켜져 있을 때 필수), 그 밖에는 없음 |
| `search` | 순위가 매겨진 심볼과 코드 발췌로 구현 위치 찾기 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context` |
| `find` | glob 또는 파일명 정규식으로 파일과 폴더 찾기(최근 수정 순) | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | 실제 파일을 정규식으로 검색 | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | 줄 번호와 함께 실제 파일의 줄 범위 읽기, 또는 `view=definitions`로 선언 목록 보기 | `file_path`, `offset`, `limit` 또는 `ranges`; `view` |
| `analyze` | 색인 용량이나 기록된 읽기 동작을 압축 JSON으로 분석 | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (선택형) | 로컬 뜻 사전에서 분야와 주제별로 대체 이름 조회 | `terms`: 검색 단어, 짧은 복합어 또는 단일 개념 기술 용어 1–16개 |

## `search`, `grep`, `find`, `read` 선택 기준

- `search`: 정확한 이름이나 위치를 모른 채 동작이나 개념으로 구현을 찾을 때 씁니다.
- `grep`: 정확한 식별자, 주석, 방금 수정한 코드를 찾을 때 씁니다. `grep.pattern`은 정규식이므로 코드의 특수문자를 그대로 찾으려면 JSON 이스케이프와 별도로 정규식 이스케이프를 해야 합니다. 기본 출력은 줄 번호가 붙는 `content`이며 `files_with_matches`와 `count`는 경로 목록이나 개수를 돌려줍니다.
- `find`: 파일 내용이 아니라 파일명이나 경로 패턴으로 파일과 폴더를 찾을 때 씁니다(최근 수정 순 정렬).
- `read`: 실제 파일의 줄 범위를 읽거나 `view=definitions`로 선언 목록을 볼 때 씁니다. `file_path`, `offset`, `limit` 외에 `path`, `file`, `query`와 1부터 시작하는 양끝 포함 `start_line`/`end_line`, `start`/`end` 별칭(camelCase, kebab-case 변형 포함)도 받습니다.

### 일괄 `read ranges` 읽기

알려진 여러 구간은 최상위 `file_path`(별칭 포함) 대신 `ranges`에 1~20개 항목을 넣어 한 번에 읽을 수 있습니다.

```json
{
  "ranges": [
    { "file_path": "src/main.rs", "offset": 10, "limit": 20 },
    { "file_path": "src/lib.rs", "offset": 40, "limit": 15 }
  ],
  "view": "source"
}
```

각 항목은 단일 `read`와 같은 경로와 줄 범위 별칭을 지원하며, 최상위 `view`, `expand`, `include_seen` 등 표시 옵션은 모든 항목에 공통으로 적용됩니다. 결과는 경로와 요청 범위 제목을 붙여 입력 순서대로 반환하며, 한 항목에서 오류가 나도 뒤 항목을 계속 처리합니다. 전체 응답이 읽기 한도나 클라이언트 바이트 한도를 넘으면 남은 항목을 생략하고, 처음 생략한 범위를 안내합니다. 자세한 내용은 [read·grep 요청별 출력 제어](/ko/configuration/#readgrep-요청별-출력-제어)를 참고하세요.

### 간결한 `grep` 원문, 전달 한도 및 관계 보기

`read`와 `grep`은 파일마다 선언 제목을 붙이고 그 아래에 원문을 한 번만 묶어 보여 줍니다. 호출 대상이 확인되면 정의 파일과 줄 번호를 함께 표시하며, 같은 파일의 상수 참조에는 정의 위치와 초기값 미리보기를 붙입니다(이름이 모호하면 생략). 색인에서 가져오는 선언과 호출 관계 문맥은 최근 편집 내용을 아직 반영하지 않았을 수 있습니다.

간결한 `grep` 원문만 필요할 때는 `view="source_grouped"`, `expand="none"`으로 요청하세요. 반복되던 줄별 경로 대신 파일 제목 아래에 원문 행과 줄 번호를 묶어 보여 줍니다(`source_grouped`는 content 모드 `grep` 전용이라 `read`에는 적용되지 않으며, `view="source"`는 기존 줄별 경로 형식을 유지합니다).

- `search`는 중복된 표시용 주석을 압축하고, 응답 크기가 `[output.client]`에 설정된 최종 바이트 상한을 넘으면 우선순위가 낮은 본문을 정확한 `read` 범위 안내로 대체합니다.
- 관련 `search` 결과에는 이벤트 지도와, 저장 위치를 콜백·인자·데이터 소비 위치에 연결하는 `Source routes`도 정적 후보로 함께 표시될 수 있으며, 근거 파일이 바뀐 연결은 재색인 전까지 숨깁니다.
- 자세한 내용은 [클라이언트 전달 한도](/ko/configuration/#클라이언트-전달-한도), [검색 표시 압축과 grep 원문 묶음](/ko/configuration/#검색-표시-압축과-grep-원문-묶음), [출력과 호출 관계](/ko/configuration/#출력과-호출-관계), [이벤트 관계 탐색](/ko/configuration/#이벤트-관계-탐색)을 참고하세요.

## 모노레포와 외부 `workspace_scope`

모노레포에서는 `initial_instructions`가 선택 가능한 작업공간 범위를 안내합니다.

- 그중 하나를 `workspace_scope`로 넘기면 `search`가 해당 디렉터리 안만 검색합니다(파일을 넘기면 부모 폴더 선택).
- 생략하거나 `all` 또는 `전체`를 넘기면 저장소 전체를 검색합니다. 어느 패키지에서 구현했는지 모를 때는 저장소 전체 검색으로 시작해 반환된 경로를 보고 범위를 좁히세요.
- `search`는 지정한 범위를 임의로 넓히지 않습니다. 상위 결과는 상세 발췌로 보여 주고(응답이 32 KiB를 넘으면 일치한 줄과 읽을 범위 표시), 나머지 결과는 제한된 목록으로 이어집니다.

`workspace_scope`에는 서버 시작 폴더 밖의 폴더(절대 경로 또는 시작 폴더 기준 상대 경로)나 시작 폴더 안의 연결 worktree를 넘길 수도 있습니다.

- `search`는 해당 폴더가 속한 저장소의 색인(worktree 자체이거나 `.git`, `.hg`, `.jj`가 있는 가장 가까운 상위 폴더, 홈 디렉터리와 그 위 폴더는 제외)을 사용하고 결과를 그 폴더로 좁힙니다.
- 외부 색인은 그 저장소의 설정을 따르되 `[filesystem_permissions]`만 현재 세션의 설정을 사용하므로, 세션의 `[filesystem_permissions].read`가 그 저장소 전체를 허용해야 합니다.
- 외부 저장소를 처음 호출하면 색인을 생성하며, 색인이 끝날 때까지 준비 중 안내를 반환하고 결과를 절대 경로로 출력합니다. 이후 같은 외부 저장소 파일에 `read`나 `grep`을 호출할 때 이미 색인이 만들어져 있으면 선언과 호출자에 그 색인을 재사용합니다.
- 외부 색인은 한 번에 하나만 열어 두고 10분 동안 쓰지 않으면 닫습니다. 다른 형식의 색인이 이미 있으면 `Index unavailable:`을 알리고 그대로 둔 채 `read`, `grep`, `find`로 실시간 파일 결과를 돌려줍니다.

### 순위 밖 구절 일치

일반 `search` 질의에서는 파일의 docstring이나 색인된 문자열 리터럴에 따옴표 구절이나 인접한 두 단어가 그대로 들어 있으면 순위 결과 밖의 파일도 `## Exact phrase matches outside the ranking` 아래에 표시할 수 있습니다(예: ``- src/events/publisher.rs (82 lines) — docstring of EventPublisher [L23-26]``).

- 순위 결과가 쓰고 남은 파일 행 수(`output.search.overview_file_limit`)와 출력 바이트(`output.search.max_bytes`)만 사용하므로 순위 파일이나 관계, 발췌는 바뀌지 않으며 선택한 `workspace_scope` 밖의 파일은 넣지 않습니다.
- 단일 식별자 질의, Boolean 연산자나 필드 필터를 쓴 질의, Jev 필터가 켜진 검색, 부분 출력, 색인이 준비되지 않은 상태에서는 추가하지 않습니다.
- 글자 그대로 일치한 결과만 알려 주므로 실제 질문에 맞는지는 파일을 열어 확인하세요.

## 공통 중복 제거 (`include_seen`)

`search`, `read`, `grep`은 공통 불리언 인자 `include_seen`을 받으며 기본값은 `false`입니다.

- 같은 연결에서 이미 전달한 변경 없는 원문은 짧은 구간이나 경계 줄, 한 응답 안의 중복까지 생략하고 응답마다(또는 `read ranges`의 각 범위마다) `<--removed duplicated-->` 표시를 한 번만 남깁니다.
- 컨텍스트 손실 등으로 원문을 다시 받아야 할 때는 `include_seen=true`로 호출해 중복 제거만 건너뛸 수 있습니다. 마스킹, 잘림 안내, 출력 바이트 상한은 계속 적용됩니다.
- 전달 이력은 연결별로 유지되다가 초기화, 작업 등록, 설정 다시 읽기 때 비워지며, 수정된 파일은 자동으로 새 원문을 반환합니다.

## 선택형 `synonyms`

`related_terms.enabled = true`로 `synonyms`를 켤 수 있습니다. 처음 사용할 때 고정된 뜻 사전 데이터셋을 한 번 내려받으며, `related_terms.dataset_path`로 로컬 `rt-sense/5` 파일을 지정할 수도 있습니다.

- **입력 용어**: `terms`에는 복합 식별자나 연결어를 빼고 `1–16`개의 핵심 검색어만 넘기세요(예: `AvatarService`는 `{"terms":["avatar"]}`). `abort signal`처럼 한 개념을 이루는 복합 용어는 쪼개지 말고 한 항목으로 넘겨야 구 전체의 뜻을 먼저 찾습니다. 응답의 분야와 주제는 분류 표기일 뿐 입력 필터가 아닙니다.
- **준비 상태와 출력 형식**: 사전 준비는 다른 도구를 막지 않고 백그라운드에서 진행됩니다. 응답은 준비 상태를 먼저 표시한 뒤 최대 `6`개 표시 그룹(그룹당 직접 관련어 `20`개, 가까운 말 `10`개, 같은 주제 단어 `10`개)을 보여 주고 잘림을 알립니다.
- **코드베이스 근거 우선순위**: 코드 근거가 준비되면 선언 이름(TypeScript, JavaScript, C, C++ 데이터 멤버 포함), import, 호출 이름에 등장하는 뜻과 단어를 먼저 배치합니다. 직접 관련어 일치는 `code uses "abort signal"`로 표시하고, 여러 단어로 나뉘어 해석된 구는 모든 구성 단어가 코드에 있을 때만 근거로 인정합니다(같은 주제 단어만 있는 경우는 제외). 각 단어 줄에는 표시된 단어 중 코드에 등장하는 개수도 함께 적습니다.
- **기본 순서와 갱신**: 근거를 준비 중이거나 코드에서 찾은 뜻이 없으면 사전 순서를 유지하며, 근거는 색인 갱신과 브랜치 전환을 따라갑니다.
- **조회 안내**: 조회 안내 문구는 입력 외의 추가 정보가 있을 때만 최대 `8`줄까지 표시하고, 영어가 아닌 입력이 사전에 없으면 영어 코드 용어로 다시 조회하도록 안내합니다.

## 제외 목록과 출력 설정

설정은 키 단위로 `<repo>/.codemap/config.toml` → `$CODEMAP_HOME/config.toml`(기본 `~/.codemap/config.toml`) → 내장 기본값 순으로 우선 적용됩니다. 전역이나 기본값을 쓰려면 저장소 파일에서 해당 키를 주석 처리하세요.

- **민감값 마스킹**: 탐지한 API 키, 토큰, 비밀번호, 비밀키는 기본으로 가립니다(`output.is_redact_enabled = false`로 끌 수 있습니다). 검색 일치와 순위는 원문 기준으로 계산합니다. 자세한 내용은 [민감값 마스킹](/ko/configuration/#민감값-마스킹)을 참고하세요.
- **테스트 코드 문맥**: 자동 심볼과 호출 관계에서는 테스트 영역을 기본 제외합니다(`output.context.exclude.should_include_test_code = true`로 포함할 수 있습니다). 직접 `read`나 `grep`으로 읽는 원문은 영향을 받지 않습니다. 자세한 내용은 [테스트 코드 문맥](/ko/configuration/#테스트-코드-문맥)을 참고하세요.
- **디렉터리 제외**: `index.exclude.excluded_directories`와 `.gitignore`, `.git/info/exclude`, `.codemapignore`가 빌드 및 캐시 폴더를 색인에서 제외합니다. `find`와 `grep`은 `include_ignored: true`로 선택적 제외를 건너뛸 수 있으며, `read`는 `[filesystem_permissions]`를 따릅니다. 자세한 내용은 [디렉터리 제외 규칙](/ko/configuration/#디렉터리-제외-규칙)을 참고하세요.
- **변경 반영**: 이미 존재하는 설정 디렉터리의 변경은 약 1초 뒤 자동 반영됩니다. 제외 목록이나 언어 지원 설정을 바꾸면 전체 색인을 다시 갱신하며, `index.path`와 감시 설정을 바꿨다면 서버를 재시작해야 합니다. 전체 키, 기본값, 설정 전환 방법은 [설정 레퍼런스](/ko/configuration/)를 참고하세요.

## 선택적 Jev 판단 단계

Jev는 `search`, `read`, `grep`이 돌려주는 코드가 현재 작업과 관련 있는지 걸러 주는 선택적 단계이며 기본값은 꺼짐입니다. `search`에서는 완전한 함수 본문을, `read`와 `grep`에서는 읽기 구간에 잘린 함수 본문과 함수 밖의 줄 묶음까지 함께 판단합니다.

- **활성화와 인증**: `output.jev.enabled = true`로 켜고 `output.jev.scope`로 적용 도구를 고릅니다(기본 `["search", "read", "grep"]`, `[]`이면 미적용). TypeSafe API 키는 `.codemap/auth.toml`의 `[jev].api_key`, `$CODEMAP_HOME/auth.toml`(`~/.codemap/auth.toml`), `TYPESAFE_API_KEY` 순서로 읽습니다. OpenRouter decision 모델(`openai/gpt-6-luna-decisions` 등)을 쓰려면 `output.jev.model`(기본 `jev-1.14.0`)을 지정하고 `[openrouter].api_key` 또는 `OPENROUTER_API_KEY`를 설정하세요. decision을 지원하지 않는 모델은 거부됩니다. 자세한 내용은 [OpenRouter decision 모델](/ko/configuration/#openrouter-decision-모델)을 참고하세요.
- **작업 등록**: 활성화된 필터를 쓰기 전에 사용자 요청 원문을 `initial_instructions`의 `task` 인자로 한 번 등록하고, 작업이 바뀌면 다시 호출해야 합니다. 비어 있거나 잘못된 `task`는 오류를 내고 이전 작업도 지우며, 검색 질의는 작업을 대신할 수 없습니다.
- **필터링과 대체**: 켜진 `search`, `read`, `grep` 호출은 마스킹한 `task`와 판단 대상 코드 블록을 제공자에 전송해 무관한 코드를 생략합니다. 판단이 불확실한 블록은 원문을 그대로 남기고, 생략된 블록은 원래 위치(줄 범위)를 남겨 두므로 생략된 범위를 정확히 지정해 다시 읽으면 원문을 받을 수 있습니다. 제공자 요청이 실패하면 일반 미필터 출력으로 대체되며 클라이언트 전달 한도는 그대로 유지됩니다.
- **넓은 구간 우회**: 더 넓은 구간을 Jev 생략 없이 읽으려면 `output.jev.scope`에서 `"read"`를 빼거나 Jev를 끄세요(`include_seen=true`는 중복 제거만 우회합니다). 상세 규칙은 [선택적 Jev 판단 단계](/ko/configuration/#선택적-jev-판단-단계)를 참고하세요.
