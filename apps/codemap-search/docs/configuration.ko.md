# 설정

한국어 | [English](./configuration.md)

codemap-search는 설정 파일 없이도 기본값으로 동작합니다. 변경할 키만 설정하면 나머지는 전역 설정이나 내장 기본값을 사용합니다.

`output.event_navigation`, `analysis`, `output.macro_expansion` 섹션은 생략해도 됩니다. 이벤트 탐색과 매크로 확장은 기본으로 켜지며, 분석 대상 OS를 생략하면 전역 값을 상속하고 빈 문자열은 상속한 대상을 해제합니다. 대상이 없으면 미지정 상태로 분석하며 실행 컴퓨터의 OS를 추정하지 않습니다. `is_enabled = false`를 포함해 기존에 명시한 설정은 계속 우선합니다. `analysis.jev`는 선택적 Jev 판단 단계 설정이며 명시적으로 켜기 전까지 모든 단계가 꺼져 있습니다([선택적 Jev 판단 단계](#선택적-jev-판단-단계) 참고).

## 섹션 구성과 출력 한도

동작 설정은 `config.toml`의 역할별 하위 섹션에서, 인증정보는 별도 `auth.toml`에서 관리합니다. 자동 생성 파일에는 각 항목의 의미·단위·상속·적용 시점을 설명하는 주석을 포함합니다. 실제 기본값이 있는 설정은 활성 값으로 제공합니다. search/read 한도, 전처리 설정, 이벤트 규칙과 사용자 마스킹 목록도 포함합니다. 공통·grep·클라이언트 한도 예시, 빌드 설정 경로와 분석 대상 해제 예시는 주석으로 유지합니다. 빈 목록을 포함한 활성 값은 전역 설정보다 우선하므로 상속하려면 키를 삭제하거나 주석 처리해야 합니다.

| 섹션 | 범위 |
|---|---|
| `output` | 공통 MCP 응답 바이트 한도와 가림 여부 |
| `output.client` | Claude 문자 한도, Codex 도구별 토큰 한도 |
| `output.overview` | 루트 통계와 개요 응답 |
| `output.search` | 검색 상세 파일·심볼·코드 조각 |
| `output.read` | 직접 읽기 응답 |
| `output.grep` | 일치 행·함수 본문 응답 |
| `output.context` | 호출자·호출 대상 표시 수와 관계 출력 예산 |
| `output.navigation` | 소스 구조 기반 탐색과 호출자 검색 예산 |
| `output.macro_expansion` | 매크로 전처리와 생성된 선언의 탐색 결과 |
| `output.event_navigation` | 이벤트·소스 경로 색인과 탐색 결과 |
| `output.redact` | 추가 가림 규칙과 예외 |
| `output.context.exclude` | search 호출 관계와 read/grep 자동 문맥, 이벤트·소스 경로 분석의 공통 테스트 제외 |
| `index`, `index.refresh`, `index.language_support` | 색인 저장·갱신·언어 지원 |
| `index.exclude` | 색인·overview·search·호출자 탐색·find/grep의 공통 디렉터리 제외 |
| `analysis` | Rust 분석 대상 OS |
| `analysis.jev` | 선택적 search 전용 작업 판단, 기본은 꺼짐 |

설정 위치만 구분하며 기존 적용 범위는 유지합니다. overview·search는 색인에 포함된 파일을 사용하고, find·grep은 같은 디렉터리 규칙을 공유합니다. read 원문에는 디렉터리 제외를 적용하지 않으며 자동 문맥에는 `output.context.exclude`를 적용합니다.

`output.max_bytes`는 선택 사항입니다. 응답 한도 우선순위는 **저장소 도구별 값 → 저장소 공통값 → 전역 도구별 값 → 전역 공통값 → 기존 도구 기본값**입니다. 모든 값을 생략하면 검색은 1 MiB, read와 확장 grep은 5 MiB를 유지합니다. 확장 grep은 해당 계층에 grep·공통 한도가 없으면 read 한도를 이어받습니다. `output.context.max_bytes`는 별도 관계 출력 예산으로, 검색 응답 안의 남은 공간도 함께 적용합니다.

search는 부분 결과를 표시하고 read는 더 좁은 구간을 요청합니다. overview·find·initial_instructions와 확장하지 않은 grep은 명시한 응답 한도를 넘으면 범위 축소 오류를 반환합니다. 한도는 MCP 본문 텍스트 기준이며 JSON 봉투나 오류 메시지에는 적용하지 않습니다. 일반 CLI의 parse·index 결과는 이 MCP 응답 한도로 자르지 않습니다.

### 클라이언트 전달 한도

`output.client`는 각 코딩 에이전트가 모델에 넘기는 결과의 최대 크기, 즉 codemap-search의 최종 컨텍스트 크기입니다. `max_bytes` 예산은 그보다 앞 단계인 후보 수집·Jev 입력·렌더링·페이지 나누기에 적용되며, 얼마나 크게 지정하든 모든 최종 응답은 지정한 클라이언트 한도 중 가장 작은 값을 지켜야 합니다. 한도는 UTF-8 바이트로 비교합니다. Claude 문자 수는 바이트로 세므로 실제 문자 수보다 적게 세지 않고, Codex 토큰은 토큰화 결과가 아닌 여유 추정치 `floor(토큰 수 × 3.5)`를 쓰며, pi·opencode 값은 이미 바이트입니다. 미지정 키는 한도를 추가하지 않습니다.

검사는 모든 도구에 대해 Jev 생략·마스킹·중복 제거가 끝난 최종 텍스트에서 수행합니다. 순위 검색은 검사 전에 한도에 맞춥니다. 판정 후 일치·불확실 본문을 우선하고, `min(output.search.max_bytes, 한도 − min(512, 한도 / 8))`를 넘는 낮은 우선순위 본문 전체를 정확한 read 범위로 대체하며 관계와 후속 탐색 공간도 남깁니다. 빼 둔 여유는 렌더링 뒤 붙는 마스킹·중복 표식을 위한 것입니다. 후보·Jev 입력 예산은 유지합니다. 나머지 도구는 클라이언트가 잘라낼 결과 대신 범위 축소 오류를 반환합니다. read는 더 좁은 구간을, grep은 더 작은 `head_limit`와 다음 페이지용 `offset`을 제안합니다. Claude 메타데이터와 Codex exec의 여러 도구 결과 합산 한도는 별개이며, 임의의 묶음 출력이 클라이언트 한도를 넘지 않는다는 보장은 아닙니다.

`output.client.claude_max_result_chars`는 1~500000의 문자 수이며 미지정이면 메타데이터를 보내지 않습니다. 지정하면 여섯 도구의 `tools/list` 항목에 `_meta["anthropic/maxResultSizeChars"]`로 전달합니다. 클라이언트가 도구 목록을 다시 읽도록 MCP를 재연결하세요. [Claude Code 공식 문서](https://code.claude.com/docs/en/mcp#raise-the-limit-for-a-specific-tool)

`output.client.codex_output_token_limit`는 양의 토큰 수입니다. 설정한 뒤 `codemap-search codex-config`를 실행하면 여섯 도구의 `mcp_servers.codemap-search.tools.<tool>.output_token_limit` TOML을 출력합니다. 등록한 서버 이름이 다르면 `--server-name 이름`을 지정하세요. 출력 조각을 Codex 설정에 병합해야 적용되며 명령은 클라이언트 파일을 쓰지 않습니다. [Codex 공식 문서](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options)

Codex Code Mode는 `exec` 호출마다 별도 출력 예산을 적용합니다. `initialize` 요청의 `params.clientInfo.name`이 `codex-mcp-client`인 연결에만 Codex 전용 안내를 전달하며, 다른 클라이언트나 식별 정보가 없는 연결에는 공통 안내만 전달합니다. Codex 전용 안내는 에이전트가 첫 줄에 `// @exec: {"max_output_tokens": N}`을 넣고, `N`에는 `codex_output_token_limit` 값(미지정이면 10000)을 쓰며, `wait`에도 같은 출력 예산을 적용하도록 안내합니다. 이 예산은 출력한 결과의 합계에 적용되므로 큰 묶음은 나눠야 합니다. 클라이언트의 `tool_output_token_limit`가 더 작으면 기록에 저장할 때 잘릴 수 있으며, 서버는 이 설정을 읽거나 변경하지 않습니다. 설정값을 바꾸면 MCP를 재연결해 안내를 갱신하세요. Jev 활성화 여부와 무관하게 적용되는 안내이며, 에이전트가 따른다는 보장은 없습니다. [Codex 설정 공식 문서](https://learn.chatgpt.com/docs/config-file/config-reference#configtoml)

`MCP client delivery context` 진단 로그는 `params._meta.callId` 유무, `exec-` 뒤에 하이픈을 포함한 UUID가 오는 형식에 한정한 ID 값, 응답 바이트 수와 설정된 Codex 한도를 기록합니다. 이 내부 ID 형식은 Code Mode의 단서일 뿐이며 실제 exec 예산이나 모델 문맥에 전달됐는지는 알 수 없습니다. 이 값으로 필터·출력 한도·중복 이력을 바꾸지 않습니다.

`output.client.pi_max_bytes`와 `output.client.opencode_max_bytes`는 정수 바이트 또는 크기 문자열이며 기본값은 미지정입니다. pi([pi-mcp-adapter](https://github.com/nicobailon/pi-mcp-adapter#output-guard) `settings.outputGuard.maxBytes`)와 opencode([`tool_output.max_bytes`](https://opencode.ai/v2/docs/config))는 서버 메타데이터를 읽지 않으며, 기본으로 큰 텍스트 결과의 앞 51200바이트만 남기고 나머지는 파일로 저장합니다. 클라이언트에 설정한 값을 지정하세요. codemap-search는 클라이언트 설정을 바꾸지 않습니다. 줄 수 한도(기본 2000줄)는 맞추지 않습니다.

## 설정 위치와 우선순위

키별 우선순위는 저장소 → 전역 → 내장 기본값입니다. 저장소 파일에 `[output.search].detail_file_limit`만 있으면 다른 키는 전역 설정이나 기본값을 이어받습니다.

| 범위 | 경로 |
|---|---|
| 저장소 | `<repo>/.codemap/config.toml` |
| 전역 | `$CODEMAP_HOME/config.toml`, 미지정 시 `~/.codemap/config.toml` |

### 인증정보 (`auth.toml`)

Jev API 키는 `<repo>/.codemap/auth.toml` 또는 `$CODEMAP_HOME/auth.toml`(미지정 시 `~/.codemap/auth.toml`)의 `[jev].api_key`에 저장합니다. 우선순위는 **저장소 auth → 전역 auth → `[analysis.jev].api_key_env`에 지정한 환경 변수**(기본 `TYPESAFE_API_KEY`)입니다. 키가 없거나 빈 문자열·공백뿐이면 다음 값을 사용합니다. 파일 읽기 실패·잘못된 TOML·자료형은 경고 후 다음 값으로 대체하며, 키 값이나 파서의 소스 발췌는 출력하지 않습니다. 알 수 없는 섹션·키도 값을 출력하지 않고 경고한 뒤 무시합니다.

```toml
# .codemap/auth.toml — 버전 관리에서 제외하세요
[jev]
api_key = "<발급받은 키>"
```

`config_auto_update = true`이면 MCP 시작 시 누락된 저장소 `auth.toml`을 인증정보가 없는 현지화 템플릿으로 만듭니다. 기존 파일은 덮어쓰지 않으며 환경 변수의 키를 복사하지 않습니다. Unix에서는 소유자만 읽고 쓰는 `0600` 권한으로 생성하고, Windows에서는 파일시스템의 상속 ACL을 사용합니다. 전역 인증 파일은 자동 생성하지 않습니다. 모델·활성화·요청 제한은 기존 `config.toml`의 `[analysis.jev]`에 유지합니다.

이름이 `auth.toml`인 모든 파일은 대소문자 구분 없이 색인·search·overview와 기본 `find`/`grep`에서 제외합니다. 이는 접근 차단 정책은 아닙니다. 직접 `read`/`parse`할 수 있고, `find`/`grep`의 `include_ignored: true`로 파일명 제외를 우회할 수 있습니다. `.codemap` 등 필수 제외 디렉터리는 탐색 시 계속 제외합니다. Git 제외는 직접 설정해야 하며 codemap-search는 Git 무시 파일을 수정하지 않습니다.

## 설정 읽기와 자동 작성

현재 설정 버전은 **27**이며 주석으로 표시합니다.

```toml
# codemap-config-version: 27
```

- 설정 파일은 없어도 됩니다. TOML 구문이 잘못되면 해당 파일의 설정 전체를 사용하지 않습니다. 알 수 없는 키·잘못된 자료형·허용되지 않는 값은 stderr에 경고하고 해당 키만 낮은 우선순위 설정으로 대체합니다. 저장소 값이 잘못되어도 유효한 전역값이 있으면 기본값보다 우선합니다.
- `[update].config_auto_update = true`이면 MCP 시작 시 누락된 저장소 설정을 만듭니다. 제외 배열에는 공통 폴더와 감지한 프로젝트 종류의 재귀 glob을 넣고, 다른 활성 키에는 내장 기본값을 씁니다. 활성 저장소 키는 전역값보다 우선합니다.
- 버전 표시가 없거나 6 이전인 저장소 설정은 **제외 목록을 한 번 전환**합니다. 사용자 규칙, 이전에 적용되던 제외값, 공통 폴더, 추천 재귀 glob을 배열에 명시합니다. 기존 항목과 주석은 보존하고 누락된 값만 중복 없이 추가한 뒤 현재 스키마 버전으로 바꿉니다.
- **버전 6부터 `excluded_directories`는 자동으로 만들거나 보충하지 않습니다.** 항목 삭제, `[]` 지정, 키 주석 처리, 새 프로젝트 추가 후에도 목록을 복원하지 않습니다. 수동 변경을 읽어 적용하는 동작은 계속됩니다.
- 스키마 갱신은 지원하는 이전 키를 현재 구조로 옮기되 적용값·상속·명시한 `[]`·사용자 주석을 보존합니다. 전역 파일을 포함해 이전 별칭도 계속 읽습니다. 충돌하거나 잘못된 값을 안전하게 옮길 수 없으면 파일을 유지하고 경고합니다.
- 새 키는 활성 설정이 아닌 주석 예시로 추가합니다. 생략된 키는 상속값이나 내장 기본값을 사용하며 `is_enabled=false`처럼 명시한 값은 유지합니다. Jev는 각 도구의 활성화 설정을 직접 켜야 동작합니다.
- 자동 생성 설명과 섹션 배치는 갱신할 수 있지만 비활성 설정과 사용자 메모는 보존합니다. 이미 최신인 파일은 다시 쓰지 않습니다.
- `config_auto_update = false`는 config/auth 템플릿 최초 생성과 config 전환을 모두 끕니다. 설정 읽기와 감시는 계속되며, 전역 파일은 항상 자동 생성·전환 대상에서 제외됩니다.
- 운영체제 언어가 한국어이면 한국어 주석을, 그 밖에는 영어 주석을 생성합니다. 프로젝트 감지 전 두 템플릿의 키와 값은 같습니다.

전환 중 파일이 바뀌거나 TOML 구문·쓰기 권한·지원하지 않는 테이블 구조 때문에 전환할 수 없으면 파일을 그대로 두고 경고합니다. 원인을 수정한 뒤 재시작하세요. 점으로 연결하거나 인라인으로 작성한 index 테이블에 제외 배열이 없다면 배열을 명시하고 다시 시도하세요. 설정 파일의 심볼릭 링크는 유지합니다.

### 자동 작성을 껐을 때 수동 전환

버전 6에서는 명시한 배열이 선택적 기본 목록을 대체합니다. 이전 배열은 내장 목록에 추가하는 방식이었습니다. 자동 작성을 껐다면 기존 제외를 유지하기 위해 `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor` 중 필요한 항목을 직접 넣고, 공통·프로젝트 규칙을 추가한 뒤 버전 주석을 6으로 바꾸세요. 다른 키는 전환할 필요가 없습니다. 직접 편집하기 전에 설정을 백업하세요.

전환하면서 상속받던 전역 제외값을 저장소 배열에 기록할 수 있습니다. 이후 전역 변경을 다시 상속하려면 저장소 키를 주석 처리하세요. 전역 배열도 버전 6에서는 전체 목록이며 자동으로 수정하지 않습니다.

## 디렉터리 제외 규칙

`[index.exclude].excluded_directories`는 **선택적으로 제외할 디렉터리 규칙의 전체 목록**입니다. 명시한 배열에 숨겨진 기본 목록을 더하지 않습니다. `[]`는 선택적 규칙을 해제하고, 키 생략은 전역 목록 또는 기본값을 상속합니다. 무시 파일과 필수 제외 규칙은 별도로 적용합니다.

공통 초기 목록은 다음과 같습니다.

```toml
[index.exclude]
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
```

`.idea`, `.vscode`, `.vs`는 수정 가능한 기본값입니다. 버전 관리 내부 폴더(`.git`, `.svn`, `.hg`, `.bzr`, `.jj`, `.sl`), `.codemap`, `.codemap-index`, 실제 색인 경로는 배열에서 지우거나 `include_ignored`를 켜도 탐색에서 제외합니다. 직접 `read`는 파일시스템 권한을 따릅니다.

배열을 전혀 지정하지 않으면 공통 목록에 `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`를 더한 기본값을 씁니다. 처음 생성하는 설정에는 공통 이름과 감지한 프로젝트 종류의 **재귀 glob**을 넣습니다. `**/node_modules`, `**/build`, `**/target`처럼 각 패턴을 한 번만 기록하며 옆 프로젝트를 포함해 작업공간의 모든 깊이에 적용합니다. 같은 이름의 소스 폴더를 유지하려면 해당 glob을 더 좁은 규칙으로 바꾸세요.

0.8.1에서 생성한 설정에는 `apps/web/node_modules` 같은 경로가 남아 있을 수 있습니다. 재귀 제외를 원하면 해당 항목을 `**/node_modules`로 바꾸세요. 기존 버전 6 배열은 사용자가 관리하며 자동으로 다시 쓰지 않습니다.

| 규칙 | 의미 |
|---|---|
| `build` | 모든 깊이에서 해당 이름의 폴더 |
| `**/build` | 작업공간 루트를 포함해 모든 깊이에서 해당 이름의 폴더 |
| `./build` | 작업공간 루트의 해당 폴더만 |
| `apps/web/build` | 해당 프로젝트 폴더와 그 하위 |
| `apps/api/**/__pycache__` | 해당 프로젝트의 모든 깊이에 있는 캐시 폴더 |
| `apps/native/cmake-build-*` | 해당 프로젝트에서 패턴에 맞는 CMake 출력 폴더 |

규칙은 대소문자를 구분하는 glob이며 같은 이름의 파일에는 적용하지 않습니다. 상대 경로 기준은 `find`/`grep`에 전달한 디렉터리가 아니라 현재 작업공간입니다. Windows 구분자는 `/`로 정리합니다. 절대 경로, 상위 경로 이동, 빈 문자열, 잘못된 glob이 있으면 배열 전체를 거부하고 경고한 뒤 낮은 우선순위 값을 씁니다. 단순 이름은 허용된 외부 소스 트리에도 적용하지만 작업공간에 고정된 경로는 적용하지 않습니다.

같은 규칙을 색인, 코드맵, 호출자 탐색, 파일 변경 갱신, `find`, `grep`에 적용합니다. 제외 폴더 안에서 직접 검색해도 상위 폴더의 제외 규칙을 우회하지 않습니다. `find`/`grep`의 `include_ignored: true`는 선택적 디렉터리·일반 파일 제외를 우회하지만 필수 내부 폴더·색인 폴더는 계속 제외합니다. 배열에서 규칙을 지워도 `.gitignore`와 `.codemapignore`는 적용됩니다.

### 최초 프로젝트 감지

설정 최초 생성과 버전 6 전환 때만 공통 폴더·프로젝트 종류별 추천 glob을 만듭니다. 프로젝트 파일 이름을 확인하며 빌드를 실행하거나 매니페스트 내용을 평가하거나 디렉터리 심볼릭 링크를 따라가지 않습니다. 무시 파일을 따르고 공통·생성 폴더와 프로젝트에 속하지 않는 의존성·캐시 트리는 건너뛰며 중첩 프로젝트를 지원합니다. 아직 없는 출력 폴더도 재귀 glob으로 추천 목록에 넣습니다. 발견한 프로젝트 경로는 기록하지 않으며 같은 프로젝트 종류가 여러 곳에 있어도 패턴을 중복 추가하지 않습니다. 사용자가 지정한 별도 출력 경로는 직접 추가해야 합니다.

| 프로젝트 식별 파일 | 작업공간 전체에 적용하는 재귀 glob |
|---|---|
| `package.json` | `**/node_modules`, `**/.yarn`, `**/dist`, `**/build`, `**/coverage`, `**/.next`, `**/.nuxt`, `**/.output`, `**/.svelte-kit`, `**/.astro`, `**/.turbo`, `**/.parcel-cache` |
| `pyproject.toml`, `setup.py`, `setup.cfg`, `Pipfile`, `requirements*.txt` | `**/.venv`, `**/venv`, `**/__pycache__`, `**/.pytest_cache`, `**/.mypy_cache`, `**/.ruff_cache`, `**/.tox`, `**/.nox`, `**/build`, `**/dist`, `**/*.egg-info` |
| `Cargo.toml` | `**/target` |
| `go.mod`, `go.work` | `**/vendor` |
| `pom.xml` | `**/target` |
| `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts` | `**/.gradle`, `**/build` |
| `build.sbt` | `**/target`, `**/project/target`, `**/.bloop`, `**/.metals` |
| `*.csproj`, `*.sln`, `*.slnx` | `**/bin`, `**/obj` |
| `composer.json` | `**/vendor` |
| `Gemfile`, `*.gemspec` | `**/vendor/bundle` |
| `Package.swift` | `**/.build` |
| `Podfile` / `Cartfile` | `**/Pods` / `**/Carthage/Build` (각각) |
| `pubspec.yaml` | `**/.dart_tool`, `**/build` |
| `CMakeLists.txt` | `**/build`, `**/cmake-build-*`, `**/CMakeFiles`, `**/_deps` |
| `MODULE.bazel`, `WORKSPACE`, `WORKSPACE.bazel`, `BUILD`, `BUILD.bazel` | `**/bazel-bin`, `**/bazel-out`, `**/bazel-testlogs` |
| `.tf` 파일이 있는 디렉터리 | `**/.terraform` |

SQL, Lua, PowerShell, 독립 셸·웹·설정 파일과 문서에는 출력 경로를 추측해서 추가하지 않습니다. 빌드 시스템을 감지하면 해당 종류의 패턴을 작업공간 전체에 추가합니다. 프로젝트 식별 파일을 하나도 찾지 못하면 공통 추천 목록만 생성합니다. `gradle` 소스·설정 폴더는 유지하고 `.gradle` 캐시만 제외합니다.

## 변경 적용 시점

MCP는 `[index.refresh].watch`와 별개로 시작 시 존재하는 저장소·전역 설정 디렉터리의 `config.toml`과 `auth.toml`을 감시합니다. 변경을 약 **1000ms** 동안 모은 뒤 설정을 다시 읽습니다. 디렉터리가 없었거나 감시를 시작하지 못했다면 설정 생성·편집 후 서버를 재시작하세요. CLI는 명령 실행 시 설정을 읽습니다.

| 설정 | 적용 시점 |
|---|---|
| `[index.exclude]`, `[output.context.exclude]`, 모든 `[index.language_support]` 키 | 다시 읽은 뒤 전체 색인 갱신을 요청하고, 완료되면 결과에 반영 |
| `output.client`를 제외한 출력·호출 관계 표시·응답 한도, `output.is_redact_enabled`, `[output.redact]`, 파일시스템 권한 | 설정을 다시 읽은 뒤 다음 도구 요청 |
| `index.refresh.index_staleness_ms`, `index.refresh.indexer_auto_restart` | 이후 갱신·복구 판단 |
| `index.max_file_bytes` | 이후 탐색·갱신부터 적용하며, 이 값만 바꾸면 전체 갱신을 요청하지 않음 |
| `index.store_references` | 이후 파싱부터 적용하며, 재시작해도 변경되지 않은 파일은 기존 색인을 재사용할 수 있음 |
| `index.path`, `index.refresh.watch`, `index.refresh.watch_debounce_ms` | 재시작 필요 |
| `config_auto_update` | 다음 MCP 시작 시 자동 작성 |
| `auth.toml`의 `[jev].api_key` | 다시 읽은 뒤 다음 활성 Jev 요청부터 적용하며, 키가 바뀌면 공통 HTTPS 평가기를 다시 생성 |
| `[output.client].claude_max_result_chars` | 최종 전달 한도는 재로드 후 적용. 클라이언트가 도구 목록을 갱신하도록 MCP 재연결 |
| `[output.client].codex_output_token_limit` | 최종 전달 한도는 재로드 후 적용; 클라이언트는 codex-config를 다시 병합 |
| `[output.client].pi_max_bytes`, `[output.client].opencode_max_bytes` | 최종 전달 한도는 재로드 후 적용. 클라이언트 변경 불필요 |

제외 규칙을 직접 바꾸면 파일 필터를 갱신하고 전체 색인 갱신을 요청합니다. 갱신 완료 후 제외된 파일은 결과에서 사라지고 새로 포함한 파일은 검색할 수 있습니다. 색인 기능을 사용할 수 없으면 복구하거나 서버를 재시작한 뒤 결과를 확인하세요.

## 설정 키

숫자 키는 양의 정수이며 `output.grep.max_columns`만 `0`도 허용합니다. 템플릿은 아래처럼 섹션별 키를 사용합니다. 호환성을 위해 `result_threshold = 5` 같은 기존 최상위 키도 허용하지만 같은 파일에 둘 다 있으면 섹션별 값이 우선합니다.

바이트 크기에는 정수 바이트 수 또는 `b`, `kb`, `mb`, `gb`를 붙인 양의 정수 문자열을 사용합니다. 단위는 대소문자를 구분하지 않으며 1024배 기준입니다. `"50mb"`는 `52428800`바이트이고 `"50 MB"`처럼 앞뒤·단위 앞 공백도 허용합니다. 소수, 0, 음수, 미지원 단위, 저장 자료형의 범위를 넘는 값은 경고 후 하위 설정을 상속합니다. TOML에서 단위가 붙은 값은 따옴표가 필요하므로 `50mb`만 쓰면 구문 오류입니다. 적용 키는 `index.max_file_bytes`, `output.max_bytes`, 도구별 `output.*.max_bytes`, `output.client.pi_max_bytes`, `output.client.opencode_max_bytes`, `output.macro_expansion.max_output_bytes`입니다. 개수·밀리초 설정은 계속 정수만 받습니다.

| 키 | 자료형 | 기본값 | 설명 |
|---|---|---|---|
| `[output].is_redact_enabled` | bool | `true` | MCP 응답의 탐지된 인증정보와 선택한 PII를 가림. 검색 일치와 로컬 색인은 원문 유지 |
| `[output].max_bytes` | 정수 바이트 또는 크기 문자열 | 미지정 | 공통 MCP 응답 한도. 도구별 예외가 같은 계층에서 우선 |
| `[output.client].claude_max_result_chars` | 정수(문자), 1~500000 | 미지정 | Claude tools/list 메타데이터와 최종 전달 한도(바이트 기준). 재연결 필요 |
| `[output.client].codex_output_token_limit` | 양의 정수(토큰) | 미지정 | Codex 설정 내보내기와 최종 전달 한도(토큰당 3.5바이트) |
| `[output.client].pi_max_bytes` | 정수 바이트 또는 크기 문자열 | 미지정 | pi 최종 전달 한도 |
| `[output.client].opencode_max_bytes` | 정수 바이트 또는 크기 문자열 | 미지정 | opencode 최종 전달 한도 |
| `[output.overview].is_stats_enabled` | bool | `true` | 저장소 루트와 모노레포 프로젝트 루트 `overview`에 색인 파일 언어 통계 포함. `false`면 해당 섹션 생략 |
| `[output.overview].max_bytes` | 정수 바이트 또는 크기 문자열 | 공통값 상속 | 개요 응답 한도. 공통값도 없으면 추가 제한 없음 |
| `[output.search].detail_file_limit` | 정수 | `24` | 상세 내용을 표시할 상위 파일 수 |
| `[output.search].overview_file_limit` | 정수 | `80` | 나머지 간략 목록에 표시할 최대 파일 수 |
| `[output.search].snippet_max_lines` | 정수 | `500` | 심볼마다 표시할 최대 발췌 줄 수. 긴 본문은 생략 표시 |
| `[output.search].symbol_limit` | 정수 | `100` | 파일마다 표시할 최대 심볼 수. 초과분은 생략 안내 |
| `[output.search].max_bytes` | 정수 바이트 또는 크기 문자열 | `"1mb"` (`1048576`) | 부분 출력 안내를 포함한 검색 응답 전체 크기 제한 |
| `[output.search].literal_max_chars` | 정수(문자) | `1200` | 일치한 리터럴의 최대 표시 길이. 초과분은 말줄임표로 표시 |
| `[output.search].literal_limit` | 정수 | `60` | 파일마다 표시할 최대 리터럴 수 |
| `[output.search].anchor_snippet_limit` | 정수 | `20` | 파일마다 전체 발췌를 표시할 최대 심볼 수. 나머지는 최대 3줄 선언으로 표시 |
| `[output.read].max_bytes` | 정수 바이트 또는 크기 문자열 | `"5mb"` (`5242880`) | `read`와 함수 본문으로 확장된 `grep`의 출력 한도. read 초과는 오류, grep은 페이지 분할 |
| `[output.grep].max_columns` | 정수 | `0` | `grep`의 `content` 모드 열 제한. 양수 제한 초과 시 `[Omitted long matching line]`, `0`이면 제한 해제 |
| `[output.grep].max_bytes` | 정수 바이트 또는 크기 문자열 | 공통값 상속 | grep 응답 한도. 확장 본문은 read 한도도 대체값으로 사용 |
| `[output.context].is_enabled` | bool | `true` | `search` 호출에서 `caller_context` 생략 시 호출 관계 표시 여부 |
| `[output.context].caller_limit` | 정수 | `1000` | 심볼마다 표시할 최대 호출자 또는 비호출 참조 수 |
| `[output.context].callee_limit` | 정수 | `1000` | 심볼마다 표시할 최대 호출 대상 수 |
| `[output.context].max_bytes` | 정수 바이트 또는 크기 문자열 | `"128kb"` (`131072`) | `output.search.max_bytes` 안에서 호출 관계의 출력 크기 제한 |
| `[output.context].common_name_threshold` | 정수 | `2` | 같은 이름의 정의가 이 수 이상이면 모호함 표시 |
| `[output.context].caller_omit_def_threshold` | 정수 | `5` | 같은 이름의 정의가 이 수 이상이면 추정 호출자 목록을 생략하고 `grep` 안내. 호출 대상 목록에는 미적용 |
| `[output.navigation].is_enabled` | bool | `false` | 소스 구조로 호출 대상을 확인하면 `precise`로 표시 |
| `[output.navigation].callsite_budget` | 정수 | `1000` | 이름 기반 추정으로 전환하기 전 검사할 최대 호출 위치 수 |
| `[output.navigation].scan_limit` | 정수 | `16000` | 호출자 탐색에서 이름별로 나눠 쓸 검색 건수 제한. 이름당 최소 25건 |
| `[output.redact].pii_entities` | 문자열 배열 | `[]` | 활성화할 PII 종류의 정확한 이름. [지원 목록](./pii-redaction.ko.md) 참고 |
| `[output.redact].sensitive_fields` | 문자열 배열 | `[]` | 내장 민감 필드명에 추가할 이름. 대소문자와 구분자를 정규화한 뒤 정확히 일치해야 함 |
| `[output.redact].rules` | 인라인 테이블 배열 | `[]` | 추가 정규식 규칙. 각 항목에 `id`, `pattern` 필요 |
| `[output.redact].exceptions` | 인라인 테이블 배열 | `[]` | `rule_id`와 탐지된 `value`가 모두 정확히 일치하는 예외 |
| `[index].path` | 문자열 | `".codemap/index"` | 색인 저장 위치. 절대 경로나 작업공간 루트 기준 상대 경로 |
| `[index].max_file_bytes` | 정수 바이트 또는 크기 문자열 | `"1mb"` (`1048576`) | 파싱·색인 전 건너뛸 파일 크기 기준 |
| `[index].store_references` | bool | `false` | 함수 호출 외의 참조 위치 저장 |
| `[index.refresh].watch` | bool | `true` | 파일 변경을 감시해 백그라운드 색인 갱신 |
| `[index.refresh].watch_debounce_ms` | 정수(ms) | `500` | 파일 변경을 모아서 처리하는 시간 |
| `[index.refresh].index_staleness_ms` | 정수(ms) | `5000` | 파일 감시를 쓸 수 없을 때 요청 기반 갱신 간격 |
| `[index.refresh].indexer_auto_restart` | bool | `true` | 백그라운드 색인이 중단되면 자동 복구 |
| `[index.language_support].is_document_support_enabled` | bool | `false` | `.md`/`.mdx`를 색인 기반 탐색에 포함 |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | `.sh`, `.bash`, `.zsh`를 색인 기반 탐색에 포함 |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | HCL/Terraform, Dockerfile, Nix 포함 |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Protocol Buffers, GraphQL 포함 |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Make, CMake, Starlark/Bazel 포함 |
| `[index.exclude].excluded_directories` | 문자열 배열(상대 디렉터리 glob) | 공통 + 기존 기본 이름. 생성 파일은 재귀 glob 사용 | 선택적 제외 전체 목록. [디렉터리 제외 규칙](#디렉터리-제외-규칙) 참고 |
| `[index.exclude].use_git_exclude` | bool | `true` | `.git/info/exclude` 적용 여부 |
| `[output.context.exclude].should_include_test_code` | bool | `false` | 자동 심볼·호출 관계에 테스트 코드 포함 |
| `[output.context.exclude].test_file_patterns` | 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 파일 glob. []는 경로 판별 해제 |
| `[output.context.exclude].test_attributes` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 속성·어노테이션 패턴. 언어별 목록을 상속값 대신 적용 |
| `[output.context.exclude].test_decorators` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 데코레이터 패턴. []는 해당 언어 목록 해제 |
| `[output.context.exclude].test_calls` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 호출 패턴. []는 해당 언어 목록 해제 |
| `[filesystem_permissions].find` | 문자열 | `"workspace"` | `find` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].grep` | 문자열 | `"workspace"` | `grep` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].read` | 문자열 | `"workspace"` | `read` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].allowed_roots` | 문자열 배열 | `[]` | `allowed_roots` 정책을 쓰는 도구에서 접근할 외부 루트 |
| `[update].config_auto_update` | bool | `true` | MCP 시작 시 누락된 저장소 config/auth 템플릿 생성과 config 스키마 갱신 |
| `[analysis.jev].search_filter_enabled` | bool | `false` | 등록한 목적에 맞춰 search의 완전하고 정체성이 확인된 본문을 자동 필터링하고 read 안내를 남김 |
| `[analysis.jev].read_filter_enabled` | bool | 최종 search 설정 상속 | read로 완전히 반환한 본문 판단. 명시한 false는 read만 끔 |
| `[analysis.jev].grep_filter_enabled` | bool | 최종 search 설정 상속 | grep content의 완전한 본문 판단. 파일·개수 모드는 로컬 유지 |
| `[analysis.jev].overview_filter_enabled` | bool | 최종 search 설정 상속 | 루트 외 overview의 선언을 실제 소스 근거로 판단. 루트 지도는 항상 로컬 유지 |
| `[analysis.jev].model` | 문자열 | `"jev-1.13.0"` | 모든 응답에서 검증하는 구체적인 제공자 모델. 별칭 이름은 검증에 실패 |
| `[analysis.jev].api_key_env` | 문자열(환경 변수 이름) | `"TYPESAFE_API_KEY"` | 두 auth 파일에 키가 없을 때 요청 시점에 읽을 대체 환경 변수. 키 자체는 아님 |
| `[analysis.jev].timeout_ms` | 양의 정수(ms), 최대 7일 | `45000` | 단계 준비 시작 시점부터 계산하고 대기 시간을 포함하는 도구 호출 한 건의 절대 마감 시각 |
| `[analysis.jev].max_in_flight_requests` | 정수, 1~3 | `3` | 동시에 진행하는 HTTP 요청 수(3이 런타임 상한) |
| `[analysis.jev].request_spacing_ms` | 정수(ms), 300 이상 | `300` | 요청 시작 사이의 최소 간격(300이 런타임 하한) |
| `[analysis.jev].max_batch_bytes` | 정수 바이트 또는 크기 문자열, 1~168000 | `168000` | 배치 하나의 인코딩된 요청 바이트(168000이 런타임 상한). 들어가지 않는 질문은 명시적 실패 |
| `[analysis.jev].pool_idle_timeout_ms` | 양의 정수(ms), 최대 7일 | `30000` | 유휴 HTTPS 연결 유지 시간 |
| `[analysis.jev].search_filter_min_unrelated_probability` | 유한한 수, `0.5 < 값 <= 1.0` | `0.70`(잠정값) | 기준별 거짓 확률 임계값. all/any로 개별 판단을 조합 |

### 색인과 파일 제외

사용자 홈 자체는 MCP 작업공간이나 명시적 `index`/`benchmark` 대상이 될 수 없지만 홈 아래 프로젝트는 허용합니다. `HOME`과 `USERPROFILE`을 모두 확인할 수 없으면 경고하고 계속 실행합니다.

`auth.toml`, `.txt`, `*.lock`, 알려진 패키지 관리 잠금 파일, `*.map`, 압축·번들 파일은 대소문자 구분 없이 색인·코드맵·호출자 탐색에서 제외합니다. `find`/`grep`도 기본적으로 숨기지만 `include_ignored: true`로 볼 수 있고 직접 `read`/`parse`도 가능합니다. 전체 목록은 [지원 언어와 파일 제외](./language-support-checklist.ko.md)에 있습니다. `index.max_file_bytes`보다 큰 파일도 색인에서 제외합니다.

다섯 `[index.language_support]` 키는 색인, search, overview, codemap, 파일 변경 갱신에 적용합니다. 실시간 `find`/`grep`/`read`와 직접 `parse`를 끄지는 않습니다.

`use_git_exclude`는 `.git/info/exclude`만 제어합니다. `false`여도 `.gitignore`, 전역 Git 무시 규칙, `.codemapignore`는 적용됩니다.

### 매크로 확장

설치된 Clang으로 C/C++ 및 CPP를 사용하는 ASM의 매크로 확장을 자동으로 시도합니다. NASM `.asm`은 확장 목록에서 생성된 label을 원본 호출 줄에 연결합니다. 이 섹션은 작성할 필요가 없으며, 외부 전처리기 실행을 끄려면 `is_enabled = false`를 지정합니다. 도구가 없으면 원문 선언과 미해결 사유를 표시합니다. [clangd의 컴파일 설정 처리](https://clangd.llvm.org/design/compile-commands)를 참고했으며, clangd 실행이나 `.clangd` 설정 읽기를 추가한 기능은 아닙니다.

```toml
[output.macro_expansion]
is_enabled = true
compilation_database = "build/compile_commands.json"
clang_path = "clang"
nasm_path = "nasm"
clang_flags = ["-Iinclude", "-DFEATURE=1"]
nasm_flags = ["-Iinclude/", "-felf64"]
timeout_ms = 5000
max_output_bytes = "8mb"
```

| `[output.macro_expansion]`의 키 | 자료형 / 기본값 | 적용 |
| --- | --- | --- |
| `is_enabled` | bool / `true` | C/C++/ASM 색인과 직접 `parse`·`codemap`에 자동 전처리 적용. 명시적인 `false`로 끔 |
| `compilation_database` | 비어 있지 않은 문자열 / 생략 | 저장소 기준 상대 경로 또는 절대 경로. JSON 파일·디렉터리 또는 `compile_flags.txt` 파일 |
| `clang_path`, `nasm_path` | 비어 있지 않은 문자열 / `"clang"`, `"nasm"` | 설치된 실행 파일의 이름 또는 경로 |
| `clang_flags`, `nasm_flags` | 문자열 배열 / `[]` | 빌드 설정 뒤에 추가하는 인자. 저장소 배열은 전역 배열을 대체 |
| `timeout_ms` | 양의 정수 / `5000` | 외부 프로세스 한 번의 밀리초 제한. NASM은 전처리·확장 목록 생성 두 번 실행 |
| `max_output_bytes` | 정수 바이트 또는 크기 문자열 / `"8mb"` (`8388608`) | 전처리 결과 또는 NASM 확장 목록의 바이트 제한 |

경로를 생략하면 소스의 부모 디렉터리에서 저장소 루트까지 `compile_commands.json`·`compile_flags.txt`를 찾습니다. JSON에서는 정규화한 파일 경로가 정확히 일치하는 첫 항목을 사용합니다. 이름이 비슷한 파일의 빌드 옵션으로 헤더 설정을 추정하지 않습니다. 데이터베이스가 없으면 사용자 인자와 설치된 도구의 기본값을 사용합니다. 데이터베이스에 대상 파일이 없으면 항목을 추가하거나 `compilation_database`에 `compile_flags.txt` 파일을 명시해야 합니다.

[컴파일 데이터베이스](https://clang.llvm.org/docs/JSONCompilationDatabase.html)의 작업 디렉터리, 헤더 경로, 매크로 정의, 대상 플랫폼과 언어 표준을 반영합니다. `arguments` 배열을 `command` 문자열보다 우선하고, 문자열은 셸 평가 없이 인자로 분리합니다. 설정한 Clang/NASM만 실행하며 데이터베이스의 래퍼나 프로젝트 빌드 명령을 실행하지 않습니다. 기록된 컴파일러 이름의 `++`와 명시적인 `-x`는 C/C++ 파서 선택에도 반영합니다. 출력·의존성 파일 생성 인자는 제거합니다. 지원하지 않는 옵션·언어, 응답 파일, 컴파일러 플러그인은 미해결 사유로 표시합니다. 별도 툴체인의 시스템 헤더·대상 플랫폼은 사용자가 공급해야 하며, query-driver 자동 탐색과 모든 컴파일러 옵션 호환성은 구현하지 않았습니다.

확장에 성공하면 활성 선언으로 목록을 갱신하고 원문에서 추출한 매크로 정의도 보존합니다. 생성된 선언에는 `macro expansion`과 원본 파일·줄 범위를 표시하며, 헤더 선언을 포함한 소스의 선언으로 오연결하지 않습니다. 확장 토큰의 열 위치와 호출·상수 참조 연결은 **미해결**입니다. 이 파일에서 추정한 정의 링크나 `(precise)`를 출력하지 않습니다. `read`·`grep`의 원문은 그대로 유지합니다.

도구·헤더 누락, 시간·출력 제한, 지원하지 않는 확장 결과, `#line`·`%line` 재지정은 `Macro expansion unresolved`에 이유를 표시하고 원문 선언을 유지합니다. NASM은 `-Le -Lm -Lf` 지원 버전이 필요하며 2.16.03으로 확인했습니다. 확장 목록을 얻는 조립 단계의 출력은 null 장치로 보내며 생성된 프로그램을 실행하지 않습니다. 조립기가 검증한 label·global만 선언 입력으로 변환하며, 일반 ASM 파서가 명령어의 피연산자를 다시 해석하지 않습니다. 자동 생성된 `..@` 매크로 지역 label은 제외합니다. 고정된 FFmpeg `x86inc.asm`의 매크로도 명시한 아키텍처·형식 인자로 확인했으며, FFmpeg 전체 빌드를 검증했다는 뜻은 아닙니다. GNU assembler 고유 `.macro` 확장, 다른 ASM 방언과 모든 저장소의 빌드 설정을 검증한 것은 아닙니다.

설정 변경은 전체 색인 갱신을 요청합니다. 활성화된 동안 작업 공간 파일 변경도 C/C++/ASM을 다시 대조하며, 기록한 외부 헤더·빌드 설정은 도구 요청 시 최대 1초 간격으로 변경 여부를 확인합니다. 전처리에 실패한 뒤 외부 의존성을 새로 준비했다면 갱신 또는 재시작이 필요할 수 있습니다. 제한은 파일별 프로세스 기준이며 저장소 전체 제한이 아닙니다. macOS arm64에서 검증했고 Windows/Linux 실행은 미확인입니다.

### 민감값 마스킹

`[output].is_redact_enabled = true`이면 MCP 도구 응답에서 탐지한 인증정보와 선택한 PII를 가립니다. 원문 보기, 색인의 문자열 값, 선언·상수 미리보기와 오류 메시지에도 적용합니다. 치환 표시는 `[REDACTED]`이며, 이 표시보다 짧은 값은 별표로 가립니다. 원문의 줄바꿈을 유지하고 응답 크기를 늘리지 않습니다. `false`로 끌 수 있으며, 설정을 다시 읽은 뒤 색인 재생성 없이 적용합니다.

PII 규칙은 기본적으로 꺼져 있습니다. `[output.redact].pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE"]`처럼 사용할 종류를 지정합니다. 이름은 대소문자를 포함해 정확히 일치해야 합니다. 미지원 이름이나 문자열이 아닌 항목이 있으면 이 키 전체를 무시하고 하위 설정을 사용합니다. 명시적인 `[]`는 상속된 PII 선택을 해제하며 인증정보 규칙은 유지합니다. 전체 종류·예외·탐지 한계는 [PII 마스킹](./pii-redaction.ko.md)을 참고하세요.

Tree-sitter로 해석할 수 있는 대입·필드·기본 인수에서는 리터럴 값과 변수 참조·타입을 구분합니다. 예를 들어 `password: string = externalValue`는 유지하고 `password: string = "hardcoded-value"`는 값을 가립니다. 문자열 연결과 보간의 정적 부분도 검사합니다. 해석하지 못한 문법·오류 구간·일반 텍스트에는 이름·패턴 검사를 적용합니다. ENV/INI의 따옴표 없는 값은 세미콜론과 공백을 포함해 검사합니다.

탐지 결과는 원문 UTF-8 바이트 범위, 규칙 식별자와 종류로 관리하며 원문 비밀값을 메타데이터에 보관하지 않습니다. 읽기 범위·발췌·문자열을 자르기 전에 가리므로 여러 줄 문자열·YAML 블록·PEM 내부 줄에도 적용됩니다. 검색 리터럴은 현재 소스의 문자열 노드와 위치가 대응할 때 해당 범위만 가립니다. 같은 줄의 안전한 문자열은 유지하며, 위치를 확인할 수 없거나 변경된 값은 가립니다. grep 일치·건수·열 제한은 원문 기준입니다. 다시 읽은 원문이 변경되거나 없어지면 해당 grep 표시값을 가립니다.

내장 규칙 식별자는 다음과 같습니다. 알려진 형식을 탐지하며 자격 증명의 실제 유효성을 확인하지는 않습니다.

| 규칙 식별자 | 탐지 대상 |
|---|---|
| `field.sensitive` | `API_KEY`, `access_token`, `client_secret`, `password` 등 민감 필드의 값 |
| `token.aws-access-key`, `token.github`, `token.openai`, `token.google` | 해당 접두사 형태의 토큰 |
| `token.slack`, `token.jwt` | Slack 토큰과 JWT 형태 |
| `token.stripe`, `token.gitlab`, `token.npm`, `token.sendgrid` | Stripe 비밀·제한 키, GitLab·npm·SendGrid 토큰 |
| `credential.authorization`, `credential.bearer`, `credential.url-password` | Authorization의 Bearer/Basic 값, Bearer 값, URL 비밀번호 |
| `private-key.pem` | PEM 비밀키 블록. 끝 표시가 없으면 남은 원문도 가림 |

사용자 규칙 예시:

```toml
[output.redact]
sensitive_fields = ["internalCredential"]
rules = [{ id = "custom.acme", pattern = 'ACME_[A-Z0-9]+' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

`internalCredential`은 `internal_credential`과도 일치합니다. 추가 필드명은 정확히 일치해야 하며, 내장 필드명은 기존 접미사 규칙을 유지합니다. 정규식 `id`는 고유한 `custom.` 접두사 이름이며 영숫자·점·밑줄·하이픈을 사용할 수 있습니다. Rust `regex` 문법을 사용하므로 역참조·둘러보기는 지원하지 않습니다. `(?P<secret>...)` 그룹이 해당 일치에 참여하면 그 범위만, 그렇지 않으면 전체 일치를 가립니다. 여러 줄 검사는 `(?s)` 등을 패턴에 명시합니다.

예외는 규칙 식별자와 **탐지된 원문 값 전체**가 정확히 같을 때만 해당 탐지를 제외합니다. `ACME_EXAMPLEPLUS`는 제외하지 않으며, `password = "ACME_EXAMPLE"`는 별도의 `field.sensitive` 규칙이 계속 가립니다. 파일·경로 전체 제외나 부분 문자열 예외는 지원하지 않습니다. 따옴표 안의 값은 바깥 따옴표를 뺀 원문 표기와 비교하므로 이스케이프를 디코딩하지 않습니다.

각 목록은 저장소 → 전역 → 기본값 순으로 결정하며, 명시한 목록은 해당 상속 목록 전체를 대체합니다. `[]`는 해당 목록을 비우고 내장 인증정보 규칙은 유지하며, `pii_entities = []`는 추가 PII 종류를 해제합니다. 잘못된 목록·중복 사용자 규칙 식별자·유효하지 않거나 빈 문자열과 일치하는 정규식은 해당 설정 키 전체를 무시하고 하위 설정을 적용합니다. 경고에는 패턴·예외 값·정규식 오류 원문을 출력하지 않습니다. 설정 버전 15는 사용자 설정용 세 목록을, 버전 16은 PII 선택 목록을 추가합니다. 전역 목록을 상속하려면 저장소의 해당 키를 주석 처리합니다.

검색 이유·일치 근거 지도·하단 추가 결과의 리터럴도 마스킹 후에 출력하거나 자릅니다. 검색 결과가 없을 때에는 문맥을 확인할 수 없는 비밀값 조각이 재노출되지 않도록 입력 검색어를 안내문에 되풀이하지 않습니다.

최종 JSON-RPC 텍스트에는 토큰·인증정보·문맥 없이 판정하는 선택 PII·사용자 정규식을 다시 적용합니다. 관련 필드명·라벨이 필요한 PII와 소스 문맥 판정은 원문을 가진 출력 단계에서 수행합니다. 서식을 붙인 뒤 라벨 옆에 나타난 줄 번호나 참조에 약한 규칙을 다시 적용하지 않습니다. 프로토콜 식별자·객체 키·숫자·불리언과 부모 객체·배열의 처리 계약은 유지합니다.

별도의 검사 바이트·후보 수·시간 한도는 추가하지 않습니다. 기존 Tree-sitter 파싱 제한(5000ms)과 도구 입출력 제한을 유지하며, 파싱이 끝나지 않으면 텍스트 검사로 보완합니다. 전체 문맥 검사를 위해 일치한 파일을 메모리에서 읽으므로 파일 크기에 따라 작업량과 메모리 사용량이 증가합니다.

알려지지 않은 이름·형식, 인코딩 값이나 함수 호출을 통해 조립된 값은 놓칠 수 있고 일반 예제도 가릴 수 있습니다. 원본 파일, 저장된 색인, 검색 일치·순위, 일반 CLI 명령(`parse` 등)의 출력과 stderr 로그는 마스킹 대상이 아닙니다. CLI의 `mcp` 명령으로 주고받는 JSON-RPC 응답은 적용 대상입니다. 다른 파일 읽기 도구에는 적용하지 않습니다.

### 출력과 호출 관계

저장소 루트와 모노레포 프로젝트 루트 `overview`는 기본적으로 색인 파일 언어 통계를 포함합니다. `[output.overview].is_stats_enabled = false`이면 해당 섹션을 생략합니다. 예를 들어 선택 가능한 프로젝트 경로인 `overview(path="apps/api")`는 해당 프로젝트에 속한 색인 파일만 집계합니다. 그 외 폴더와 파일에는 통계를 붙이지 않습니다. 사라지거나 변경된 파일은 집계 불가로 표시합니다. 요청당 새로 집계할 수 있는 256개 파일 또는 64 MiB를 넘으면 대기 상태로 표시하며, 이후 요청은 완료된 집계를 재사용합니다.

`search`는 상위 파일의 상세 내용 뒤에 나머지 파일을 간략 목록으로 표시합니다. 크기 설정은 출력을 제한하며 제외 파일을 바꾸지 않습니다. 일반 질의에서는 생성 파일과 번역 리소스의 순위를 낮추지만 정확한 경로·심볼·리소스 키·인용 원문으로 해당 파일을 지정할 수 있습니다.

`output.search.max_bytes`에는 부분 출력 안내도 포함합니다. 결과가 잘리면 질의를 좁히거나 안내된 범위를 읽으세요. `search`에는 페이지 위치 인자가 없습니다. `output.search.anchor_snippet_limit`는 파일마다 전체 발췌를 표시할 심볼 수를 제한하며 나머지는 최대 3줄 선언으로 표시합니다.

`output.read.max_bytes`에는 줄 번호·문맥·제목을 포함하며 함수 본문으로 확장된 `grep`에도 적용합니다. `read` 출력이 초과하면 더 좁은 `offset`/`limit`를 안내하는 오류를 반환합니다. 확장된 `grep`은 본문을 조용히 자르는 대신 크기 초과 또는 페이지 분할을 안내합니다. `read`에서 함수 확장을 끄고 `limit`를 생략하면 별도의 전체 파일 256 KiB 제한도 적용합니다. `grep_max_columns = 0`은 긴 줄 제한을 끄고, 그 외에는 초과 줄을 `[Omitted long matching line]`으로 표시합니다. `grep`의 부분 결과에는 `next_offset`이 있습니다.

읽기 한도를 늘려도 검색·관계 문맥·파싱의 별도 제한은 유지합니다. 검색 응답은 기본 1 MiB, 호출 관계는 그 안에서 128 KiB입니다. 호출자 수집은 `scan_cap = 16000`을 검색할 이름들이 나누어 사용하며, caller/callee 1000은 표시 가능한 최대 개수이므로 실제 1000건 출력을 보장하지 않습니다. 실시간 `read`/`grep` 문맥에는 공유 16 KiB 고정 상한도 있습니다. 함수 확장 파싱은 `min(max_file_size, 4 MiB)`이며 기본값으로는 1 MiB입니다. 한도를 높이면 허용 응답 크기와 출력 처리량이 늘지만, 이 값으로 소비 클라이언트의 응답 한도까지 확인할 수는 없습니다.

내장 기본값이 바뀌어도 기존 저장소·전역 설정에 명시한 값은 유지합니다. 새 기본값을 쓰려면 이전 덮어쓰기 값을 삭제·주석 처리하거나 직접 바꿔야 하며, 재시작만으로 교체되지는 않습니다.

`output.context.is_enabled`는 `search` 호출에서 `caller_context`를 생략했을 때만 적용하며 명시한 인자가 우선합니다. 호출 관계는 기본적으로 추정값입니다. `navigation_context_default = true`이면 소스 구조·import·지역 변수 연결로 단일 호출 대상을 확인한 경우 `precise`로 표시합니다. 확인할 수 없는 호출은 추정 결과를 사용합니다. `output.navigation.callsite_budget`은 이름 기반 탐색으로 전환하기 전 검사할 호출 위치 수를 제한합니다.

`index.store_references`는 함수 호출 외의 참조 위치를 저장하며 호출 대상 확인에 필수인 설정은 아닙니다. 일부 구조화 형식은 참조를 항상 저장합니다. 파싱 시 적용하므로 재시작만으로 변경되지 않은 파일을 다시 파싱하지 않을 수 있습니다.

대상이 정해진 `calls` 항목에는 추정 모드와 `precise` 모드 모두 `이름 — 파일:줄` 형식으로 정의 위치를 붙입니다. 대상이 모호하면 이름만 유지합니다. MCP `read`/`grep`은 표시한 함수의 직접 식별자 참조를 `references (same-file constants, approximate)`로 함께 보여줍니다. 소스 구문 트리와 색인의 상수 선언(JavaScript/TypeScript의 `const` 바인딩 포함)을 확인하므로 `navigation_store_references = false`여도 동작합니다. 코드를 실행하지 않고 정의 위치와 초기값 원문을 표시하며, 240자를 넘는 값은 줄여서 표시합니다. 주석·문자열, 경로를 붙이거나 import한 참조, 매크로 토큰, 중복 이름, 지역 바인딩이 있는 이름은 생략합니다. 테스트 제외 규칙과 문맥 출력 크기 제한도 적용하며, 크기 제한으로 생략한 항목은 안내합니다.

`output.navigation.scan_limit`은 검색할 이름들이 나누어 사용하며 이름당 최소 25건을 허용합니다. `output.context.caller_limit`과 `output.context.callee_limit`은 심볼별 관계 수를 제한합니다. `output.context.max_bytes`은 `output.search.max_bytes` 안에서 호출 관계의 총 출력 크기를 바이트로 제한합니다. 원문 발췌가 우선하며 생략한 관계는 안내합니다.

같은 이름의 정의가 `output.context.common_name_threshold` 이상이면 추정 관계에 모호함을 표시합니다. `output.context.caller_omit_def_threshold` 이상이면 추정 호출자 목록 대신 안내와 `grep` 제안을 표시합니다. 호출 대상 목록이나 확인된 대상의 표시를 막지는 않습니다.

### 테스트 코드 문맥

`excluded_directories`와 `use_git_exclude`는 `[index.exclude]`, 테스트 문맥의 `should_include_test_code`, `test_file_patterns`, `test_attributes`, `test_decorators`, `test_calls`는 `[output.context.exclude]`에서 관리합니다. 같은 파일에서는 유효한 새 위치의 값이 이전 `[exclude]`와 `[caller_context]`, `[index]`, 최상위 별칭보다 우선하며, 언어별 표는 누락한 언어를 상속합니다. 파일 간 저장소 → 전역 → 내장 우선순위는 동일합니다. 두 디렉터리 제외 설정은 색인·overview·search·호출자 탐색·`find`/`grep`이 공유하며, 어느 쪽을 바꿔도 설정 재로드 후 전체 색인 갱신을 요청합니다. 직접 `read`에는 디렉터리 제외를 적용하지 않습니다.

`should_include_test_code = false`이면 설정한 테스트 영역을 `read`/`grep`의 자동 심볼 문맥과 `search`의 호출 관계에서 제외합니다. 같은 이름의 정의 개수, 호출 대상 조회, 호출자 탐색 한도를 계산하기 전에 적용합니다. 직접 `read`/`grep`한 원문, 검색 결과와 저장된 색인은 유지됩니다. `true`이면 테스트 문맥을 포함하지만 디렉터리·무시 파일 제외 규칙은 별도로 적용됩니다.

네 종류의 목록을 모두 수정할 수 있습니다. 명시한 목록은 상속한 목록을 **대체**하며 숨겨진 내장 목록에 합쳐지지 않습니다. 언어별 표는 각 언어마다 저장소 → 전역 → 내장 순으로 선택합니다. 언어를 생략하면 상속하고 `[]`이면 해당 종류의 판별을 끕니다. 기본값을 복사해 항목을 추가하거나 삭제할 수도 있습니다. `{}`는 모든 언어 항목을 상속합니다. 잘못된 목록은 경고 후 상속하며, 알 수 없는 언어 이름은 경고 후 무시합니다. 언어 이름은 `rust`, `python`, `typescript`, `csharp` 같은 등록된 이름을 사용합니다.

- `test_file_patterns`: 작업공간 기준의 대소문자를 구분하는 glob입니다. `/`가 없으면 깊이에 관계없이 파일 이름과 비교합니다. 절대 경로, 상위 경로 이동, 빈 패턴, `!` 부정은 허용하지 않습니다.
- `test_attributes`: `#[...]`·`@`를 뺀 속성·어노테이션 이름 glob입니다. 전체 이름과 마지막 어노테이션·데코레이터 이름을 확인하며 Rust의 `::` 경로는 전체 이름으로 비교합니다. Rust의 `cfg(test)` 항목은 `all`·`any`·`not` 조건 중 테스트 모드가 필요한 영역도 판별합니다. `cfg(not(test))`는 유지합니다.
- `test_decorators`: `@`와 인자 값을 뺀 데코레이터 이름 glob입니다.
- `test_calls`: `test`, `describe.*` 같은 호출 표현식 이름 glob입니다. 일치한 표현식과 그 콜백 본문을 테스트 영역으로 취급합니다.

규칙은 OR로 적용합니다. 특정 마커를 꺼도 파일 패턴이나 다른 테스트 영역에 포함된 코드는 계속 제외됩니다. 테스트 영역 안의 헬퍼는 함께 제외하지만 그 영역에서 호출하는 일반 함수까지 테스트로 분류하지는 않습니다. 소스 구문을 검사하며 import·별칭 해석이나 사용자 매크로 확장은 수행하지 않습니다. 별칭은 해당 이름을 직접 등록하세요. 판별할 수 없는 구문과 읽을 수 없거나 크기 제한을 넘는 소스의 미분류 문맥은 유지합니다.

설정을 다시 읽은 뒤 다음 요청부터 적용되며 검색 색인을 다시 만들 필요는 없습니다. 크기가 제한된 캐시는 소스 메타데이터나 규칙 변경 시 갱신합니다. 명시한 목록에 새 내장 기본값을 자동으로 보충하지 않습니다.

다음 예시는 내장 규칙 일부를 유지하고 사용자 마커를 추가하면서 Java 속성과 TypeScript 호출 이름 판별을 끕니다. 다른 활성 규칙은 계속 적용됩니다.

```toml
[output.context.exclude]
should_include_test_code = false
# Replace the complete path list with the patterns you want.
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[output.context.exclude.test_calls]
typescript = []
```

기본 목록입니다. 표에 없는 언어는 해당 종류의 내장 항목이 없습니다.

```toml
[output.context.exclude]
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]

[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[output.context.exclude.test_calls]
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
dart = ["test", "group", "testWidgets"]
ruby = ["describe", "context", "it", "specify"]
powershell = ["Describe", "Context", "It"]
```

### 파일시스템 권한

`read`, `find`, `grep`에 각각 다음 정책을 지정합니다.

- `workspace`: 현재 작업공간 안만 접근합니다(기본값).
- `allowed_roots`: 작업공간과 지정한 외부 루트에 접근합니다.
- `anywhere`: 서버 프로세스가 접근할 수 있는 모든 경로에 접근합니다.

`allowed_roots` 항목은 비어 있지 않은 문자열이어야 합니다. 상대 경로는 작업공간 루트 기준이며 절대 경로도 허용합니다. 존재하는 경로 부분은 심볼릭 링크를 포함해 실제 경로로 해석하고, 아직 없는 뒷부분은 유지합니다. 상위 경로 이동으로 허용된 범위를 넓힐 수는 없습니다. 이 권한은 `search`·`overview`의 색인 범위를 넓히지 않습니다.

### 색인 갱신

`watch = true`이면 파일 변경을 백그라운드에서 반영합니다. `index.refresh.watch_debounce_ms` 동안 발생한 변경은 한 번에 갱신합니다. 감시를 끄거나 사용할 수 없으면 `search`/`overview`가 `index.refresh.index_staleness_ms` 간격에 따라 백그라운드 갱신을 요청하고 마지막 결과를 즉시 반환합니다.

`indexer_auto_restart = true`이면 백그라운드 색인 중단 후 다음 `search`/`overview`가 복구를 시도합니다. 서버 실행 중 복구 횟수에는 제한이 있습니다. 끄면 재시작 전까지 결과가 고정됩니다. 실시간 `read`/`find`/`grep`은 어느 경우에도 사용할 수 있습니다.

## `config.toml` 예시

주요 값을 명시한 예시입니다. 테스트 규칙 목록은 위의 별도 예시를 참고하세요. 실제 파일에는 변경할 키만 남겨도 됩니다.

```toml
# codemap-config-version: 27
# 이 저장소의 codemap-search 설정입니다. 여기 적은 값이 전역 설정보다 우선합니다.
# 키를 지우거나 주석 처리하면 전역 설정이나 기본값을 씁니다.
# 목록은 전역 설정의 목록과 합치지 않고 이 값으로 바꿉니다. []로 비울 수 있습니다.
# 개수·시간·크기는 1 이상의 정수로 적습니다. output.grep.max_columns만 0을 쓸 수 있습니다.
# 바이트 크기에는 "50mb"처럼 b/kb/mb/gb를 붙일 수 있습니다(1kb = 1024바이트).
# 파일을 저장하면 서버가 설정을 다시 읽습니다. 반영되지 않으면 서버를 재시작하세요.

[output]
# MCP 응답에 나온 인증정보를 가립니다. false이면 가리기를 모두 끕니다.
is_redact_enabled = true

# 모든 도구에 공통으로 적용할 응답 최대 크기입니다(기본: 제한 없음).
# 도구별 max_bytes를 지정하면 그 값이 우선합니다.
# max_bytes = "1mb"

[output.client]
# Claude Code가 받는 결과의 최대 문자 수입니다(1~500000, 기본 25000토큰).
# 바꾼 뒤에는 Claude Code에서 MCP를 다시 연결하세요.
# claude_max_result_chars = 200000

# Codex가 받는 도구 결과의 최대 토큰 수입니다(기본: 모델마다 다름).
# Codex에도 같은 값을 설정하세요. `codemap-search codex-config`가 ~/.codex/config.toml에 넣을 내용을 출력합니다.
# codex_output_token_limit = 50000

# pi가 받는 MCP 결과의 최대 바이트 수입니다(기본 51200).
# pi-mcp-adapter의 settings.outputGuard.maxBytes와 같은 값을 넣으세요.
# pi_max_bytes = 51200

# opencode가 받는 도구 결과의 최대 바이트 수입니다(기본 51200).
# opencode.json의 tool_output.max_bytes와 같은 값을 넣으세요.
# opencode_max_bytes = 51200

[output.overview]
# overview로 저장소 루트나 하위 프로젝트 루트를 볼 때 언어별 파일 통계를 함께 보여 줍니다.
is_stats_enabled = true

[output.search]
# 상세 결과로 표시할 최대 파일 수입니다. 나머지 결과는 간략 목록으로 표시합니다.
detail_file_limit = 24

# 간략 결과 목록의 최대 파일 수입니다.
overview_file_limit = 80

# 검색된 정의(함수, 클래스 등)마다 표시할 코드의 최대 줄 수입니다.
snippet_max_lines = 500

# 상세 결과에서 파일마다 표시할 최대 정의 수입니다.
symbol_limit = 100

# 코드에서 추출한 문자열 값 중 검색어와 일치한 값을 표시할 최대 문자 수입니다.
literal_max_chars = 1200

# 파일마다 표시할 문자열 값의 최대 개수입니다.
literal_limit = 60

# 파일마다 코드 전체를 표시할 최대 정의 수입니다. 나머지는 선언부를 3줄까지만 표시합니다.
anchor_snippet_limit = 20

# search 응답의 최대 크기입니다.
max_bytes = "1mb"

[output.read]
# read 응답의 최대 크기입니다.
max_bytes = "5mb"

[output.grep]
# grep 결과에서 한 줄의 최대 길이(바이트)입니다. 0이면 제한이 없습니다.
# 더 긴 줄은 `[Omitted long matching line]` 같은 생략 표시로 바뀝니다.
max_columns = 0

# grep 응답의 최대 크기입니다(기본: 제한 없음).
# 함수 본문을 함께 보여 주는 grep은 이 값과 output.max_bytes가 없으면 output.read.max_bytes를 씁니다.
# max_bytes = "5mb"

[output.context]
# search 결과에 호출자와 호출 대상을 보여 줍니다. search 호출에 caller_context를 넘기면 그 값이 우선합니다.
is_enabled = true

# 정의마다 표시할 최대 호출자 수입니다. 호출이 아닌 참조도 포함합니다.
caller_limit = 1000

# 정의마다 표시할 최대 호출 대상 수입니다.
callee_limit = 1000

# search 응답에서 호출 관계가 차지할 최대 크기입니다. 코드를 먼저 넣고, 호출 관계는 남은 공간에 넣습니다.
max_bytes = "128kb"

# 같은 이름의 정의가 이 개수 이상이면 이름으로 추정한 호출 관계를 모호하다고 표시합니다.
common_name_threshold = 2

# 같은 이름의 정의가 이 개수 이상이면 추정한 호출자 목록 대신 grep 검색을 안내합니다.
caller_omit_def_threshold = 5

[output.navigation]
# import 문과 코드 구조를 분석해 실제 호출 대상을 찾습니다. 찾은 대상에는 `precise`를 붙입니다.
# false이면 이름만으로 추정합니다.
is_enabled = false

# 이 개수만큼 호출 위치를 확인한 뒤에는 이름으로 추정합니다.
callsite_budget = 1000

# 호출자를 찾을 때 확인할 최대 검색 결과 수입니다.
scan_limit = 16000

[output.macro_expansion]
# 설치된 Clang/NASM으로 C/C++/어셈블리 코드의 매크로를 펼쳐 분석합니다.
# 실패하면 원래 선언을 그대로 보여 줍니다.
is_enabled = true

# compile_commands.json(또는 그 폴더)이나 compile_flags.txt의 경로입니다. 상대 경로는 작업공간 루트 기준입니다.
# 지정하지 않으면 소스 파일 폴더에서 작업공간 루트까지 올라가며 찾습니다.
# compilation_database = "build/compile_commands.json"

# Clang 실행 파일 이름(PATH에서 찾음) 또는 경로입니다.
clang_path = "clang"

# NASM 실행 파일 이름(PATH에서 찾음) 또는 경로입니다.
nasm_path = "nasm"

# 빌드 설정 뒤에 추가할 Clang 인자입니다.
clang_flags = []

# 빌드 설정 뒤에 추가할 NASM 인자입니다.
nasm_flags = []

# 전처리 한 번에 허용할 최대 시간(밀리초)입니다.
timeout_ms = 5000

# 전처리 결과의 최대 크기입니다.
max_output_bytes = "8mb"

[output.event_navigation]
# 이벤트를 보내고 받는 코드와 값이 전달되는 경로를 분석해 탐색 결과에 보여 줍니다.
is_enabled = true

# EventEmitter의 on/emit 같은 기본 이벤트 규칙을 사용합니다. false여도 직접 추가한 규칙은 적용합니다.
use_builtin_rules = true

# 직접 추가할 이벤트 규칙입니다. 형식과 예시는 docs/configuration.ko.md를 참고하세요.
rules = []

[output.redact]
# 추가로 가릴 개인정보 유형입니다. 예: ["EMAIL_ADDRESS", "CREDIT_CARD"]
# 지원하는 유형은 docs/pii-redaction.ko.md를 참고하세요.
pii_entities = []

# 값을 가릴 키나 변수 이름을 추가합니다(예: "internalCredential"). 대소문자와 _, - 같은 기호는 무시하고 비교합니다.
sensitive_fields = []

# 직접 추가할 정규식 규칙입니다. 항목마다 id와 pattern을 적습니다.
# id는 custom.으로 시작해야 하며 다른 규칙과 겹치면 안 됩니다. 예시는 docs/configuration.ko.md를 참고하세요.
rules = []

# 가리지 않을 예외입니다. rule_id와 value가 모두 정확히 일치할 때만 적용합니다.
exceptions = []

[output.context.exclude]
# 호출 관계와 자동으로 붙는 관련 정보에 테스트 코드를 포함합니다.
# false여도 search/overview의 선언과 read/grep 원문에는 테스트 코드가 그대로 나옵니다.
should_include_test_code = false

# 테스트 파일로 볼 경로 패턴입니다(*, ** 사용 가능). 작업공간 기준이며, /가 없으면 파일 이름과 비교합니다.
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[output.context.exclude.test_attributes]
# #[test]나 @Test처럼 테스트 코드에 붙는 표시를 언어별로 적습니다. #[...]와 @는 빼고 적으며 *를 쓸 수 있습니다.
# 적지 않은 언어는 기본 목록을 씁니다. cfg(test)는 all/any/not으로 조합한 조건도 인식합니다.
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]

java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]

kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]

csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]

swift = ["Test", "Suite"]

php = ["Test"]

[output.context.exclude.test_decorators]
# @pytest.fixture처럼 테스트 코드에 붙는 Python 표시입니다. @는 빼고 적습니다.
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[output.context.exclude.test_calls]
# describe(...)나 it(...)처럼 테스트를 정의하는 함수 이름입니다.
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]

typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]

dart = ["test", "group", "testWidgets"]

ruby = ["describe", "context", "it", "specify"]

powershell = ["Describe", "Context", "It"]

[index]
# 색인을 저장할 경로입니다. 상대 경로는 작업공간 루트 기준입니다. 바꾼 뒤에는 서버를 재시작하세요.
path = ".codemap/index"

# 색인할 파일의 최대 크기입니다. 더 큰 파일도 read/find/grep으로는 볼 수 있습니다.
# 값을 늘리면 CPU, 메모리, 디스크 사용량이 늘어날 수 있습니다.
max_file_bytes = "1mb"

# 함수 호출이 아닌 참조 위치도 색인에 저장합니다.
# 이미 색인한 파일은 내용이 바뀌어야 반영됩니다.
store_references = false

[index.exclude]
# 색인과 find/grep에서 제외할 폴더입니다. read로 직접 여는 파일에는 적용하지 않습니다.
# "build"나 "**/build"는 모든 위치의 build를, "./build"는 작업공간 루트의 build만 제외합니다.
# "apps/web/build"처럼 작업공간 기준 경로도 쓸 수 있으며, 절대 경로와 ..은 쓸 수 없습니다.
# 처음에는 감지한 프로젝트에 맞는 폴더가 들어가고, 이후에는 직접 관리합니다.
# .git 같은 버전 관리 폴더, .codemap, 색인 폴더는 항상 제외합니다.
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]

# `.git/info/exclude`에 적힌 경로도 제외합니다. `.gitignore`, 전역 gitignore, `.codemapignore`는 항상 적용합니다.
use_git_exclude = true

[index.refresh]
# 파일이 바뀌면 색인을 자동으로 갱신합니다. false이면 search/overview를 호출할 때 갱신합니다.
# 바꾼 뒤에는 서버를 재시작하세요.
watch = true

# 파일 변경을 모아서 처리할 시간(밀리초)입니다. 길수록 갱신 횟수는 줄고 반영은 늦어집니다.
# 바꾼 뒤에는 서버를 재시작하세요.
watch_debounce_ms = 500

# 파일 감시를 쓰지 않을 때 색인을 갱신하는 최소 간격(밀리초)입니다.
index_staleness_ms = 5000

# 색인 작업이 멈추면 다음 search/overview 때 다시 시작합니다(횟수 제한 있음).
# false이면 서버를 재시작할 때까지 search/overview 결과가 갱신되지 않습니다.
indexer_auto_restart = true

[index.language_support]
# 아래 파일 종류를 색인할지 정합니다. 꺼 두어도 read/find/grep으로는 볼 수 있습니다.
# Markdown: `.md`, `.mdx`
is_document_support_enabled = false

# 셸 스크립트: `.sh`, `.bash`, `.zsh`
is_shell_support_enabled = false

# 인프라: HCL/Terraform(`.hcl`, `.tf`, `.tfvars`), Dockerfile, Nix(`.nix`)
is_infrastructure_support_enabled = false

# 인터페이스 정의: Protocol Buffers(`.proto`), GraphQL(`.graphql`, `.gql`)
is_interface_support_enabled = false

# 빌드 파일: Makefile, `.mk`, CMakeLists.txt, `.cmake`, BUILD, BUILD.bazel, `.bzl`
is_build_support_enabled = false

[analysis]
# Rust 코드의 cfg(target_os) 조건을 판단할 OS입니다(기본: 지정 안 함). 예: "linux", "macos", "windows"
# ""로 두면 전역 설정의 값을 쓰지 않습니다.
# target_os = ""

[analysis.jev]
# TypeSafe Jev로 작업과 관계없는 코드 본문을 결과에서 뺍니다(기본: 꺼짐).
# 쓰려면 initial_instructions(task_query, questions)로 작업을 먼저 등록해야 합니다.

# search 결과에 적용합니다. 뺀 본문의 소스 범위를 남기며 활성화된 read 필터는 그대로 적용합니다.
# Jev를 쓰지 못하면 원래 결과를 그대로 돌려줍니다.
# search_filter_enabled = false

# read 결과에 적용합니다(기본: search_filter_enabled 값).
# read_filter_enabled = false

# grep 결과에 적용합니다(기본: search_filter_enabled 값).
# grep_filter_enabled = false

# 루트 외 overview 선언을 판단합니다(기본: search_filter_enabled 값). 루트 지도는 로컬로 유지합니다.
# overview_filter_enabled = false

# 사용할 Jev 모델 버전입니다(기본 jev-1.13.0). jev-latest 같은 별칭은 쓸 수 없습니다.
# model = "jev-1.13.0"

# TypeSafe API 키는 여기가 아닌 auth.toml의 [jev].api_key에 저장하세요.
# 두 auth 파일에 키가 없을 때 사용할 환경 변수 이름입니다(기본 TYPESAFE_API_KEY).
# api_key_env = "TYPESAFE_API_KEY"

# 도구 호출 한 번에 Jev가 쓸 수 있는 최대 시간(밀리초)이며 대기 시간도 포함합니다(최대 7일, 기본 45000).
# timeout_ms = 45000

# 동시에 보낼 최대 요청 수(1~3, 기본 3)와 요청 사이의 최소 간격(밀리초, 300 이상, 기본 300)입니다.
# max_in_flight_requests = 3
# request_spacing_ms = 300

# 질문 묶음 하나의 최대 크기(바이트, 1~168000, 기본 168000)입니다. 질문 하나가 이보다 크면 실패합니다.
# max_batch_bytes = 168000

# 쓰지 않는 HTTPS 연결을 유지할 시간(밀리초, 최대 7일, 기본 30000)입니다.
# pool_idle_timeout_ms = 30000

# 질문의 답을 '아니오'로 판단할 최소 확률입니다(0.5 초과 1.0 이하, 기본 0.70).
# 높일수록 더 확실한 경우에만 본문을 뺍니다.
# search_filter_min_unrelated_probability = 0.70

[filesystem_permissions]
# find, grep, read가 파일에 접근할 수 있는 범위입니다.
# "workspace"는 작업공간만, "allowed_roots"는 작업공간과 allowed_roots의 경로, "anywhere"는 모든 경로를 허용합니다.
find = "workspace"

grep = "workspace"

read = "workspace"

# "allowed_roots"를 쓰는 도구에 추가로 허용할 경로입니다. 상대 경로는 작업공간 루트 기준입니다.
# 심볼릭 링크는 실제 경로를 기준으로 판단합니다.
allowed_roots = []

[update]
# 서버가 시작될 때 이 파일이 없으면 만들고, 새 설정이 생기면 주석으로 추가합니다.
# 직접 바꾼 값과 주석은 그대로 둡니다.
config_auto_update = true
```

### 기본 작업공간 권한

```toml
[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

### 지정한 외부 루트 허용

`find`와 `grep`에 공유 소스 트리를 허용하고 `read`는 작업공간에 제한합니다.

```toml
[filesystem_permissions]
find = "allowed_roots"
grep = "allowed_roots"
read = "workspace"
allowed_roots = ["G:/shared/source", "D:/vendor-src"]
```

### 전체 디스크 접근

한 도구에 전체 디스크 접근을 허용하는 예시입니다. 서버 실행 환경이 별도로 격리되어 있지 않다면 `allowed_roots`를 우선 사용하세요.

```toml
[filesystem_permissions]
find = "anywhere"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

## `.codemap/`과 무시 파일

`.codemap/index/` 색인과 `.codemap/config.toml` 설정은 저장소의 `.codemap/`에 있습니다. 이 폴더는 탐색·색인에서 항상 제외합니다. `git status`에서도 숨기려면 `.gitignore` 또는 커밋하지 않는 로컬 규칙인 `.git/info/exclude`에 `.codemap/`을 추가하세요. 도구는 Git 파일을 수정하지 않습니다.

저장소의 `.codemapignore`는 gitignore 문법으로 색인·`find`·`grep`에서 숨길 경로를 추가합니다.

색인은 UTF-8 소스만 허용합니다. 잘못된 UTF-8 파일은 이전 색인 항목도 제거하며, `overview`·`read`에 최초 오류 바이트 위치와 제외 이유를 표시합니다. `read`의 대체 문자 표시 정책은 유지하고 원본 파일은 변경하지 않습니다.

## read·grep 요청별 출력 제어

다음은 TOML 설정이 아닌 요청 인자입니다. 내용 모드 `grep`은 기본으로 함수 본문까지 확장하고, `read`는 기본으로 요청한 줄 범위를 유지합니다. `search.caller_context`에는 영향을 주지 않습니다.

| 인자 | 기본값 | 동작 |
| --- | --- | --- |
| `view` | `"full"` | `full`: 심볼과 원문. `source`: 원문만 반환하며 색인 문맥·관계 준비를 생략. `definitions`: 선언만. `relations`: 대상·소유자와 호출·상수 참조만 표시하고 원문은 생략. |
| `unresolved` | `"list"` | `count`는 같은 미해결 총수만, `list`는 총수와 제한된 이름 목록을 표시. full/relations에 적용. |
| `expand` | 내용 모드 `grep`: `"callable"`; `read`: `"none"` | `callable`은 읽은 UTF-8 원문에서 이름 있는 함수·메서드의 경계를 확인. 오래된 색인 좌표나 추정 본문을 사용하지 않음. 명시적인 `none`은 일치 행 또는 요청한 읽기 범위를 유지. |

`read` 확장은 유효한 offset/start 별칭을 기준으로 하며 limit/end보다 우선합니다. 지원하는 함수가 없으면 기존 요청 범위와 불가 사유를 반환합니다. 파싱 입력은 `min(max_file_size, 4 MiB)`로 제한하며 너무 큰 파일은 파싱 전에 거절합니다. 붙어 있는 속성·데코레이터를 포함하고 중첩된 이름 있는 함수는 가장 안쪽을 선택합니다. 익명 클로저는 감싸는 이름 있는 함수가 대상입니다. 복합 파일, 파싱 오류, 본문 없는 선언, 다른 코드와 경계 줄을 공유하는 함수는 불가 사유를 표시합니다.

내용 모드 `grep`은 `source`를 포함한 모든 view에서 기본으로 함수 본문까지 확장합니다. 확장은 `-A/-B/-C`를 무시하고 검색식·경로·glob·대소문자·유형·제외 규칙을 유지합니다. 한 파일의 같은 바이트로 검색·파싱·출력을 수행합니다. 경로·줄 순서로 중복 함수를 제거하며, offset/head_limit/next_offset의 단위는 원문 줄이 아닌 함수 묶음 또는 확장 불가 일치 줄입니다. `head_limit=0`도 바이트 한도는 유지합니다. 일치 행, 행 단위 페이지 이동, `-A/-B/-C` 문맥이 필요하면 `expand="none"`을 지정합니다. count/files_with_matches는 옵션을 생략하면 확장하지 않으며, 명시적인 `expand="callable"`은 오류로 안내합니다.

read와 확장된 grep은 `output.read.max_bytes`, 관계 문맥은 기존 하위 예산을 지킵니다. 큰 함수를 조용히 자르지 않으며, 표시된 범위에 `expand=none`과 작은 offset/limit을 사용하도록 안내합니다. grep 열 제한으로 생략한 본문도 불완전하다고 표시합니다. 매크로·인코딩·테스트 제외 안내는 해당 문맥 모드에 유지하고, source에는 원문과 확장 처리 안내만 표시합니다. full/relations에서는 관련 이벤트 관계를 자동으로 표시합니다.

## Rust 분석 대상 지정

```toml
[analysis]
target_os = "macos"
```

`target_os`는 선택적인 OS 식별자이며 실행 컴퓨터의 OS를 사용하지 않습니다. 저장소 값이 전역 값보다 우선하고, 빈 문자열은 상속된 대상을 해제합니다. 잘못된 자료형·식별자는 경고 후 하위 설정을 사용합니다. 설정을 다시 읽으면 다음 요청이 새 원문·조건 해석기를 사용하므로 대상 변경만으로 색인을 다시 만들 필요가 없습니다.

Rust의 `target_os="값"`, `all(...)`, `any(...)`, `not(...)`, true/false를 참·거짓·미확정으로 평가합니다. 따라서 `not(target_os="macos")`는 명시한 모든 비-macOS 대상에 적용하며 Windows로 좁히지 않습니다. 대상 미지정, 다른 키·플래그, `cfg_attr`, 지원하지 않는 문자열·토큰은 미확정입니다. 단독 `cfg(test)`는 기존 테스트 문맥 필터가 관리하며 테스트 포함을 실제 Cargo 테스트 빌드로 표현하지 않습니다. 구문 의미는 [Rust 조건부 컴파일 문서](https://doc.rust-lang.org/reference/conditional-compilation.html)를 따릅니다.

import·재수출·선언·호출 위치와 확인 가능한 부모 모듈의 조건을 대조한 뒤 정의를 연결합니다. 제한된 경로 모델에서 모듈 소속을 확인할 수 없으면 미해결로 남깁니다. 원문에서 확인한 호출자는 같은 이름의 정의 개수와 관계없이 유지하고, 모호한 이름 매칭에만 생략 기준을 적용합니다. 호출 위치·별칭 예산이 끝나도 이미 확인한 연결은 유지하며 불완전하다고 표시합니다. 별칭은 최대 16단계·256개 이름의 후보 수집에만 사용하며, 각 링크는 별도로 정의 신원을 확인합니다. 명시한 분석 대상은 관계 출력에 표시하지만 실제 빌드·실행을 보장하지 않습니다.

한 응답 안에서는 같은 설정을 사용합니다. 처리 도중 설정이 갱신되면 후속 요청부터 적용합니다.

## 선택적 Jev 판단 단계

```toml
[analysis.jev]
search_filter_enabled = true
# api_key_env = "TYPESAFE_API_KEY"
# search_filter_min_unrelated_probability = 0.70
```

키는 별도 [`auth.toml`](#인증정보-authtoml)에 저장하세요. 두 auth 파일에 키가 없으면 기존 환경 변수 방식도 그대로 동작합니다.

```sh
export TYPESAFE_API_KEY="<발급받은 키>"   # 요청 시점에 읽는 대체값이며 auth.toml로 복사하지 않습니다
```

Jev는 기본으로 꺼져 있습니다. `search_filter_enabled=true`이면 search와, 별도 값을 지정하지 않은 read·grep·루트 외 overview 필터를 켭니다. `read_filter_enabled`, `grep_filter_enabled`, `overview_filter_enabled`는 도구별 명시 값이 우선하며 생략하면 최종 search 설정을 따릅니다. 루트 `overview`, `find`, `analyze`, 작업 등록과 CLI는 Jev를 호출하지 않습니다.

### 작업 질문 등록

Jev 필터가 하나라도 켜져 있으면 주 에이전트가 사용자의 작업에서 집중된 예/아니오 질문을 만들고 `initial_instructions`로 한 번 등록합니다. `task_query`와 `questions`가 필수이며, 기존 텍스트만 등록하는 방식은 오류 결과(`isError: true`)를 반환합니다. 작업이 바뀌면 다시 등록합니다. `search.query`는 검색어이며 등록한 목적을 바꾸지 않습니다. Jev가 꺼져 있으면 `initial_instructions({})`를 그대로 사용할 수 있습니다.

```json
{
  "task_query": "응답 바이트 상한은 어디에서 적용되는가?",
  "questions": [{
    "question": "이 함수가 응답 바이트 상한을 구현하거나 구체적으로 지원하는가?",
    "when_true": "제공된 소스가 해당 응답 바이트 예산을 계산·예약·제한하거나 전달한다.",
    "when_false": "제공된 사실이 해당 예산과 구체적 관계가 없는 별개 동작임을 보여준다. 파일 간 근거가 없다는 이유만으로는 아니오가 아니며 불확실하다."
  }],
  "match": "all"
}
```

| 필드 | 계약 |
| --- | --- |
| `task_query` | 비어 있지 않은 목적. 전체 등록 크기 제한 적용 |
| `questions` | 독립적인 예/아니오 기준 1~8개. 요청한 대상·방향·범위를 보존 |
| `id` | 기존 호출 호환용 선택 필드이며 무시함. 생략하면 서버가 연결용 ID를 생성 |
| `question`, `when_true`, `when_false` | 비어 있지 않은 필수 문자열. 개별 바이트 상한 없이 전체 등록·평가 예산 적용. true는 기준에 해당한다는 뜻 |
| `match` | `all`(기본) 또는 `any`. 중첩 표현식은 없음 |
| 전체 등록 | 인코딩한 JSON 최대 65,536바이트. 질문 필드에 중첩 값은 허용하지 않음 |

등록은 연결 안에서 원자적으로 교체합니다. 잘못된 교체는 이전 작업을 지우며 `initialize`도 초기화합니다. 질문 생성용 별도 서비스를 호출하지 않습니다. 등록 누락은 인수 오류이고 인증정보 부재나 평가 실패는 해당 도구의 일반 출력을 보존합니다. 공통 중복 제거는 그 뒤에 독립적으로 적용됩니다.

켜진 search·read·grep·overview는 각 도구별로 `openWorldHint: true`를 알리며 모든 도구는 읽기 전용입니다. 루트 overview·find·analyze는 Jev를 호출하지 않습니다. 새 `overview_filter_enabled`와 달리 폐기한 `overview_enabled` 설정은 경고 후 무시합니다. 생략 안내의 read 위치는 활성화된 read 필터를 우회하지 않습니다. `include_seen=true`는 별도 공통 중복 제거만 우회합니다. Find는 중복 제거 대상이 아닙니다.

### 질문 판단과 근거

판단 가능한 함수마다 등록한 모든 기준을 Noul로 평가합니다. 함수 본문은 묶음 공통 상태에 한 번 넣고 정체성·소스 범위·완전성·마스킹 상태와 제한된 정적 호출 근거를 함께 제공합니다. 파일 간 연결이나 채널 근거가 없으면 명시하며, 근거 부재를 무관함의 증명으로 취급하지 않습니다. 각 질문은 지정한 후보의 같은 흐름을 판단합니다. 별개 속성 두 개가 참이라고 연결이 증명되지는 않습니다.

관련성을 묻는 기준에서는 여러 기능이 섞인 함수도 관련된 분기나 콜백 하나로 기여가 확인될 수 있습니다. 위임·초기화 함수는 소스로 연결이 확인되면 전체 흐름을 구현하거나 대상 이름을 포함하지 않아도 기여할 수 있습니다. 공통 정책은 사용자 작업과 검색 인자를 구분하고 후보 본문부터 확인한 뒤 연결 근거를 판단하도록 합니다. 각 질문에 요청한 대상·관계·범위를 독립적으로 명시합니다. 등록 안내는 질문 하나가 일관된 관계를 판정하도록 하며 같은 의미의 질문과 표기 변형만 묶습니다. 질문 수를 줄이려는 목적으로 독립적으로 필요한 역할을 긴 체크리스트로 합치지 않습니다. 질문 1~8개는 허용 범위이며 채워야 할 목표가 아닙니다.

`search_filter_min_unrelated_probability`는 부정 판단의 의미를 유지합니다. 기본 `0.70`은 기준이 거짓일 확률이 이 값 이상이어야 아니오로 판단한다는 뜻입니다. 예는 긍정 답변에 같은 임계값을 적용하고 그 사이는 불확실로 둡니다. `all`은 하나라도 명확한 아니오면 불일치이고 모두 예일 때만 일치합니다. `any`는 하나라도 예면 일치이고 모두 아니오일 때만 불일치합니다. 나머지는 불확실입니다. 이는 개별 판단의 조합이며 보정된 결합 확률이 아닙니다. 임계값은 잠정 정책값입니다.

부분·오래된·과대·판단 불가 마스킹·정체성 미확인 본문과 불확실한 판단은 보존합니다. 호출 불가 선언·소스 위치·읽기 안내도 유지합니다. 생략 안내는 [읽기 동작 지표](./analysis.ko.md#횟수와-바이트-해석)의 전달된 원문 바이트에 포함하지 않습니다.

전송 전에 최대 후보 8개씩 근거를 묶습니다. 평가기와 같은 직렬화·사전 검사로 `max_batch_bytes`와 운영 예산을 적용합니다. 운영 예산은 state+가장 긴 질문 28,000 추정 토큰, state+모든 질문 56,000 추정 토큰으로, 제공자 한도 32k/64k에서 12.5% 여유를 둡니다. 바이트 기반 추정은 실제 제공자 토크나이저가 아닙니다. 질문을 추가할 때 바이트·토큰 중 어느 예산이든 넘기기 전에 분할합니다. 후보 하나가 들어가지 않으면 그 후보를 보존하고 나머지를 평가합니다. 본문과 직접 호출자·피호출자 소스는 그룹 안에서 중복하지 않습니다. 최대 128개 그룹이 절대 마감·호출자 취소 신호·연결 풀을 공유합니다. 필수 답변 누락이나 실제 평가 실패는 해당 도구의 전체 기본 결과를 보존합니다. 정확한 `event_key` 맵은 평가하지 않습니다.

### 렌더링 전 선택

검색은 일반 출력 예산 안에서 같은 순위의 후보와 소스 범위를 먼저 계획합니다. Jev가 소스 블록을 선택한 뒤 최종 코드 블록과 큰 결과 문자열을 조립합니다. 같은 입력의 Jev 비활성 출력은 바이트 단위로 유지하고, 평가가 실패하면 일반 소스 계획 전체를 반환합니다.

불확실·부분·정체성 미확인 소스는 보존합니다. 관련성이 확인된 함수는 직접 연결된 지원 본문을 보존할 수 있고, 겹치는 보존 소스가 있으면 이를 포함한 본문도 유지합니다. 불확실한 함수·작은 함수·이미 보호된 함수에서 연결 요소 전체로 보존을 전파하지 않습니다.

평가를 건너뛰는 조건은 해당 본문과 이웃 후보의 지원 판단 모두에서 이득이 없는 경우입니다. 생략 안내보다 작고 고립된 본문이나 무조건 보존되는 상위 본문이 이미 포함한 근거 등이 해당합니다. 작은 함수라도 판단 결과가 다른 본문의 보존에 영향을 주면 평가합니다. 건너뜀 사유와 평가 가능·생략·보호 개수를 기록합니다.

준비된 분석 주석과 해당 범위의 이벤트 근거를 후보별로 확보합니다. 검색 전체에서 앞쪽 후보부터 16 KiB를 차감하거나 각 부분을 1 KiB로 자르는 제한은 없습니다. 직접 호출자·피호출자의 실제 소스는 일반 read 권한과 전체 파일 마스킹을 거치고, 선언 정체성과 반환 범위의 완전성을 확인합니다. 같은 소스는 `state.supporting_sources`에 한 번만 넣고 후보에서 이름으로 참조하며 실제 런타임 호출 대상을 증명했다고 표현하지 않습니다. 공통 근거 정책도 한 번 저장하고 각 질문이 정책과 해당 후보를 명시적으로 참조합니다. 완전한 후보 요청을 인코딩한 크기가 한도를 넘으면 해당 후보는 판단하지 않고 보존합니다. 부가 주석이나 이벤트 근거는 예산에 맞지 않으면 제외하고 `is_context_clipped`를 Jev에 전달하며, 이 표시만으로 본문을 보존하지 않습니다. 본문만의 사전 한도는 전체 state+질문 예산에서 계산하고, 최종 여부는 실제 인코딩한 요청으로 검사합니다. `caller_context=false`이면 추가 호출자·피호출자 읽기와 호출자 이벤트 지점 수집을 하지 않고, `include_events=false`이면 이벤트 근거를 추가하지 않습니다. 준비 중·중단·갱신 실패 스냅샷은 평가하지 않습니다.

필터가 성공하면 작업 기준 조합에서 관련성이 확인된 함수에 한해, 표시한 원문 밖에서 같은 호출 이름이 색인된 위치를 후속 후보로 추가할 수 있습니다. 근거 부족이나 불확실성 때문에 보존한 선언은 확장의 시작점으로 사용하지 않습니다. 연결이 확인된 호출 관계나 전달된 원문으로 취급하지 않습니다. 남은 출력 공간과 주석 예산 안에서 최대 4 KiB, 이름마다 호출자 표시 한도 이내의 최대 8곳, 설정한 호출 지점 분석 예산을 사용합니다. 작업 범위와 문맥 제외 규칙을 적용하고 목록이 잘리면 grep으로 전체 호출 위치를 확인하도록 안내합니다. 비활성·실패·오래된 색인·기본 출력 잘림·`caller_context=false` 검색에는 추가하지 않습니다.

짧은 생략 안내에 소스 위치와 read가 Jev를 우회하지 않는다는 사실을 남깁니다. 생략으로 공간을 확보한 경우 요약에 보존한 불확실성도 표시하며 점수표용 공간을 예약하지 않습니다. 진단 바이트는 계획한 기본 출력·실제 반환 텍스트·전달한 원문을 구분하고, 생략 안내를 원문 관측에 포함하지 않습니다. 평가 전 `jev candidate evidence` 로그는 그룹·후보 번호를 마스킹된 경로와 소스 범위, 본문·문맥 크기 및 해시, 근거 확보 상태에 연결합니다. 소스·사용자 질문 원문은 기록하지 않으며 메인 도구 응답에도 포함하지 않습니다.

### read·grep의 실제 원문 판정

Read·grep은 실제로 읽은 같은 버퍼에서 함수 경계와 반환 행을 확보합니다. `full`, `source`, grep의 `source_grouped` 보기에서 완전히 반환된 함수·메서드 본문만 판단합니다. 부분 범위, 열 잘림, 구문·정체성 미확인, 너무 큰 본문은 보존합니다. 선언·관계 보기와 grep의 파일 목록·개수 모드는 판단하지 않습니다.

본문의 완전성과 보조 문맥의 확보 범위는 구분합니다. 선택적인 참조 색인 설정과 독립적으로 같은 파싱 버퍼에서 값·타입 참조를 수집합니다. 같은 파일의 호출자·피호출자·참조 선언과 소속 타입의 멤버 선언을 마스킹된 근거로 제공합니다. 후보 자신의 바인딩·선언을 먼저 처리하고 기존 예산 안에서 가까운 연결부터 너비 우선으로 수집하여, 호출자의 의존성 연쇄가 후보를 처리하기 전에 예산을 소진하지 않게 합니다. import, 미해결 호출, 모호한 선언, 분석 공백과 보조 근거 예산 초과는 근거 설명과 `has_missing_context`, `is_context_clipped`로 Jev에 전달합니다. 이 상태만으로 평가를 제외하거나 본문을 보존하지 않습니다. Jev가 각 공백이 등록된 판단 기준에 필요한지 평가하며, 기준과 무관한 공백은 확정적인 판단을 막지 않습니다.

분류 전에 색인 후보에서 파일 간 호출자·피호출자·import 선언의 근거를 제한적으로 확보합니다. 후보 소스의 명시적인 호출·import 의존성을 먼저 확보하고 남은 예산을 호출자 후보와 공유합니다. 이미 읽은 버퍼를 재사용하며 다른 파일은 일반 read 권한·크기 한도 안에서 최대 8개만 읽습니다. 현재 소스의 호출 또는 선언을 색인 위치와 대조하고 문맥 제외 규칙, 탐색·연결·바이트 예산, 호출자의 절대 마감과 취소를 적용합니다. 발췌에는 본문 또는 부분임을 표시한 선언 정보가 포함됩니다. 소속 타입의 멤버 선언은 후보가 참조하는 멤버를 우선합니다. 보조 선언의 import는 제공한 발췌에서 참조한 바인딩으로 좁히되, 개별 이름만으로 범위를 판단할 수 없는 glob·무명 import는 유지합니다. 이름·import 단서는 미확정 대상 후보이며 그 자체로 과제 관련성을 인정하지 않습니다. 발췌는 Jev 전용으로, 메인 응답의 원문 범위를 넓히지 않습니다.

식별된 완전한 본문은 인코딩한 요청이 한도 안에 들어오면 기존 Noul 판정을 받습니다. 보조 근거에 공백이 있어도 Jev의 확정적인 불일치 판정으로 생략될 수 있으며, Jev는 제공된 사실과 공백이 해당 기준에 미치는 영향을 판단해야 합니다. 부분·정체성 미확인 본문, 요청 크기 초과, 모델의 불확실 판정, 겹치거나 직접 연결된 보존 본문의 기존 보호는 별도로 유지합니다. 진단은 실제 평가한 후보를 세고 `matched`, `no_match`, `judged_uncertain`, `unjudged_bodies`를 최종 `protected`·`linked` 결과와 구분합니다. 이는 연결·정책 동작의 명세이며 실제 분류 정확도나 토큰 개선을 측정한 결과가 아닙니다.

현재 과제·질문, all/any 조합, 불확실 보존, 연결된 본문 보호, 전송 예산과 절대 마감을 search와 공유합니다. 원래 페이지와 렌더링을 먼저 확정한 뒤 생략 부분만 바꾸므로, 평가 도중 파일이 바뀌어도 새 버퍼와 섞지 않고 생략한 만큼 다음 페이지를 채우지 않습니다. 공통 중복 제거는 Jev 뒤에 적용하며 find는 제외합니다.

### TypeSafe에 전송하는 데이터

활성화된 search·read·grep이 마스킹한 목적·등록 질문·도구 인수·판단 대상 함수 본문과 제공된 지원 근거를 전송합니다. 전송 전에 마스킹하지만 모든 민감값을 탐지할 수는 없습니다. API 키는 HTTPS 인증에만 사용합니다. 기본 제공자 모델은 `jev-1.13.0`입니다.

### 전송 한도

| 설정 또는 제한 | 계약 |
| --- | --- |
| `max_in_flight_requests` | 동시 요청 1~3개 |
| `request_spacing_ms` | 최소 300ms |
| `max_batch_bytes` | 인코딩 1~168,000바이트. 명시한 더 작은 값은 유지 |
| 토큰 추정 | 운영 기본값: state+가장 긴 질문 28,000, state+전체 질문 56,000. `ceil(인코딩 바이트/3)`은 실제 사용량이 아닌 추정값 |
| `timeout_ms` | 양수, 최대 7일; 준비 과정과 모든 배치를 하나의 마감으로 제한 |
| `pool_idle_timeout_ms` | 양수, 최대 7일 |
| 배치/응답 제한 | 평가당 최대 8,192개 질문·128개 배치, 응답당 최대 4 MiB |
| 재시도/리다이렉트 | 없음 |

잘못된 설정값은 경고 후 하위 설정이나 기본값으로 대체합니다. 각 단계는 시작 시점의 설정을 고정하며 설정 변경은 후속 요청에만 적용하고 진행 중 마감을 늘리지 않습니다.

런타임은 3바이트당 1토큰으로 추정해 제공자의 64k(상태와 모든 질문)·32k(상태와 가장 긴 질문) 한도를 확인합니다. 실제 토큰 수를 보장하지는 않습니다. 제공자가 거부하면 근거를 조용히 자르지 않고 기본 출력을 보존합니다.

### 결과와 진단

| 결과 | 의미 |
| --- | --- |
| `applied` | 평가 완료; 보존 규칙에 따라 모든 본문을 유지할 수도 있음 |
| `bypassed` | 인증정보 없음·완전한 본문 없음·색인 준비 중·출력 공간 부족 등으로 평가하지 않음 |
| `fallback` | 마감 초과·요청 제한·잘못되거나 불완전한 답변·불완전한 근거 구성 등 평가 실패 |

건너뜀·실패·모든 본문 유지 시 기본 출력을 바이트 단위로 그대로 반환합니다. 필요한 작업 등록이 없으면 대신 MCP 인수 오류를 반환합니다. 자동 필터링은 적격 근거가 없는 호출까지 HTTP 요청을 보낸다는 뜻이 아닙니다.

실행한 단계마다 stderr에 `info` 수준의 `jev stage` 줄을 기록합니다(`codemap_search::mcp::jev`). 도구·결과/사유·모델과 근거 버전·평가 범위/판단/생략 개수·알려진 토큰 사용량·사용량 미보고 응답 수·요청 시도 수·경과/HTTP/대기 시간을 담습니다. 작업 목적·소스 근거·인증정보·제공자 오류 본문은 기록하지 않습니다. 꺼진 단계는 아무것도 기록하지 않습니다.

Rust에서 직접 사용할 때는 [`codemap_search::jev`](../src/jev/mod.rs)와 [판단 예제](../examples/jev_decisions.rs)를 참고하세요. 예제의 `--mock`은 오프라인이며 `--live`는 API 키로 실제 요청을 보냅니다.

### 루트 외 overview의 Jev 적용

`overview_filter_enabled`는 파일과 하위 폴더의 선언 행을 판단합니다. 저장소 루트와 그 별칭·절대 경로는 등록이나 외부 호출 없이 기존 지도를 반환합니다. 모노레포의 하위 워크스페이스는 비루트 경로에 해당합니다. 폴더는 바로 포함된 파일 최대 8개의 선언을 판단하며 하위 디렉터리 지도와 파일 항목, 색인 통계는 유지합니다.

판정용 소스는 파일 읽기 권한·마스킹·테스트 코드 제외 규칙을 따르고 색인 당시의 소스 해시와 현재 소스를 대조합니다. 판정 완료 후 파일을 다시 읽지 않습니다. 판단할 근거가 없거나 한도를 넘긴 선언은 미판정으로 남으며, 이를 분류 성공으로 집계하지 않습니다. 불확실 선언은 유지하고 일치하지 않는 선언만 제거합니다. 부모 선언이 남아도 그 자식 전체가 자동 보존되지는 않으며, 남은 자식의 구조를 나타내는 부모는 유지합니다.

판정에 사용한 소스 본문은 overview 응답이나 공통 중복 제거 이력에 등록하지 않습니다. 이후 read에서 그 본문이 이미 전달됐다고 잘못 접히지 않습니다. read·grep과 overview의 보조 근거에는 같은 클래스 범위에서 확인한 멤버 대입·사용 및 콜백 참조도 최대 8개 연결까지 포함합니다. 이는 관련성 판단을 위한 근거이며 자동 보존 조건이나 런타임 연결 증명은 아닙니다.

## 이벤트 관계 탐색

이벤트 탐색은 기본으로 켜져 있으며 별도 요청 옵션이 필요하지 않습니다. 필요한 소스를 심볼과 함께 저장하고, 같은 색인 세대의 이벤트 지도를 만든 뒤 한 번에 공개합니다. 애플리케이션 핸들러나 빌드 스크립트를 실행하지 않습니다. 아래 설정은 기본값을 보여주는 예시이며, 기능을 켜기 위해 작성할 필요는 없습니다.

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = []
```

18개 개발 언어의 저장·소비 관계는 별도 `Source routes`로 표시합니다. 이 분석은 내장·사용자 이벤트 규칙 없이도 동작하며 콜백 호출 후보, 인자 전달, 데이터 소비를 구분합니다. `include_events=false`와 `is_enabled=false`는 이벤트 지도와 소스 경로를 함께 끕니다. `use_builtin_rules=false`와 `rules=[]`는 소스 경로를 끄지 않으며, `event_key`는 규칙 기반 이벤트 지도만 조회합니다. 소스 경로의 조건·언어 범위·상한은 [소스 경로 탐색 계약](source-routes.ko.md)을 참고하세요.

`is_enabled`와 `use_builtin_rules`의 기본값은 모두 `true`입니다. 명시적인 `is_enabled=false`는 이벤트 수집과 자동 출력을 끄며, 기존에 지정한 값도 유지합니다. 저장소 설정이 전역 설정보다 우선합니다. `rules` 배열은 하위 계층의 배열 전체를 대체하며, `[]`로 상속된 사용자 규칙을 제거할 수 있습니다. `use_builtin_rules=false`로 내장 규칙도 끌 수 있습니다. 잘못된 규칙 배열은 경고 후 하위 계층 값으로 돌아갑니다.

| 요청 | 동작 |
| --- | --- |
| `read` / content `grep`: `include_events` 생략 또는 `true` | `view=full`, `view=relations`에서 반환한 줄에 이벤트 지점이나 관련 정의가 있을 때만 표시합니다. 관련 이벤트가 없으면 이벤트 섹션·빈 결과 안내를 생략하며 출력 공간도 줄이지 않습니다. `source`, `definitions`에서는 조회를 생략합니다. |
| `search`: `include_events` 생략 또는 `true` | 검색된 파일에 관련 이벤트가 있을 때만 자동으로 추가합니다. `caller_context`와 독립적이며, 이벤트가 없으면 기존 출력 공간을 그대로 사용합니다. |
| `read` / `grep` / `search`: `include_events: false` | 이번 요청의 자동 이벤트 문맥을 생략합니다. content 이외의 grep은 기존 결과 형태를 유지하며, 명시적인 `true`는 content 모드에서만 허용합니다. |
| `search`: `event_key: "saved"` | 일반 순위 검색 대신 정확히 일치하는 이벤트 키를 조회합니다. 기존 `query` 필드는 필요하며 작업공간 선택·`workspace_scope`도 적용합니다. |

자동 표시는 `on`·`emit` 문자열 검색이 아닌 색인된 API와 원문 위치 근거로 판단합니다. 인식한 이벤트의 키나 버스가 미해결이면 사유를 표시합니다. 읽은 근거가 제외되거나 오래된 경우에는 다른 지점만 유효하다고 관계를 표시하지 않습니다. 명시적인 `event_key` 조회는 빈 결과·비활성화·색인 준비 상태를 계속 안내하므로 분석 범위를 점검할 때 사용할 수 있습니다.

발행 위치, 구독 등록 위치, 핸들러 정의 위치는 각각 표시합니다. 파일·줄 번호와 API 규칙, 버스·키 근거, 조건을 함께 볼 수 있습니다. 이 관계는 정적으로 확인한 등록 경로이며 실행·전달·순서·활성 구독 수를 보장하지 않습니다. `once`, 제거 호출, 조건부 등록도 이 제한을 유지합니다. `unresolved=list/count`는 직접 호출의 미해결 이름을 제어하며 이벤트의 불확실성은 별도로 표시합니다.

첫 내장 규칙은 이름을 지정한 ESM import를 사용하는 Node `EventEmitter`(`events`, `node:events`)의 `on`, `addListener`, `once`, `emit`, `off`, `removeListener`와 Tauri 프런트엔드 `listen`, `once`, `emit`, `emitTo`를 다룹니다. Rust Tauri의 `AppHandle`, `App`, `Window`, `WebviewWindow`, `Webview` 호출은 타입·import와 `Emitter`/`Listener` 근거가 있어야 인식하며, 불투명한 핸들만으로 같은 앱 인스턴스라고 연결하지 않습니다. API 의미는 [Node 문서](https://nodejs.org/api/events.html), [Tauri 프런트엔드 문서](https://v2.tauri.app/reference/javascript/api/namespaceevent/), [Tauri Emitter 문서](https://docs.rs/tauri/latest/tauri/trait.Emitter.html)를 기준으로 합니다.

버스 연결에는 같은 불변 모듈 할당 위치와 상대 ESM import·별칭·재수출 근거가 필요합니다. 같은 타입의 다른 할당은 분리합니다. 변경 가능한 변수, 가려진 변수, 함수 내부 할당, 매개변수·팩터리 반환값은 이름만으로 합치지 않습니다. 키는 이스케이프 없는 리터럴, 불변 상수의 별칭, 명시적 TypeScript 문자열 enum 초기값을 지원합니다. Rust 상수는 기존 소스 해석기와 명시한 `[analysis].target_os` 정책을 따릅니다.

동적 키, 지원하지 않는 모듈 별칭·이스케이프, 알 수 없는 핸들러·대상은 미해결로 남습니다. 임의의 래퍼 데이터 흐름, 런타임 DI, 외부 브로커, Flow 문법, 언어를 넘는 실제 전달 경로는 자동 해석하지 않습니다. Tauri 프런트엔드 범위는 가장 가까운 색인된 `src-tauri/tauri.conf.json` 또는 `package.json`을 기준으로 하며, 여러 프로세스가 실제 같은 버스를 쓴다는 증거가 아닙니다. 대상·채널은 정확히 같은 값끼리만 묶고 전체 대상과 특정 대상의 전달 호환성을 추정하지 않습니다.

### 사용자 규칙

`src/known.ts`에 정의된 `KnownBus`를 사용하는 예시입니다.

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

규칙은 `language` + `module` + `symbol` + 선택적 `method`가 정확히 맞아야 적용합니다. `module`은 작업공간 기준 정의 파일 경로나 지원하는 외부 import 모듈입니다. 언어는 `typescript`, `javascript`, `rust`입니다. 같은 선택자의 사용자 규칙은 내장 규칙을 대체합니다. 중복 ID·선택자, 와일드카드 선택자, 알 수 없는 필드·역할, 잘못된 인자 위치는 배열 전체를 거부합니다.

| 필드 | 의미 |
| --- | --- |
| `role` | `publish`, `subscribe`, `unsubscribe` |
| `event_arg` / `event_key` | 0부터 시작하는 키 인자 위치 또는 규칙으로 선언하는 고정 키 중 정확히 하나 |
| `handler_arg` | 콜백 인자 위치. `subscribe`에서 필수이며 정의가 불분명하면 미해결로 표시 |
| `bus="receiver"` | `method`와 확인된 불변 할당이 필요 |
| `bus="argument"`, `bus_arg` | 해당 인자에서 확인한 버스 할당 사용 |
| `bus="fixed"`, `bus_identity` | 확인한 래퍼의 공유 버스를 사용자가 명시. 설정에 따른 가정으로 표시 |
| `bus="framework"` | 인식된 Tauri 프런트엔드 API와 색인된 애플리케이션 범위에만 허용 |
| `target_arg` / `target` | 대상 label 인자 또는 `any` 같은 고정 대상 중 선택. 둘을 함께 쓸 수 없으며 알려진 Tauri 프런트엔드 API에는 고정 대상 덮어쓰기를 허용하지 않음 |
| `channel` | 정확한 채널 이름. 기본값 `default` |
| `is_once` | 한 번 등록임을 기록하며 실행을 시뮬레이션하지 않음 |

인자 위치는 16 미만이고 사용자 규칙은 최대 64개입니다. 규칙 식별자·대상은 최대 256바이트, 이벤트 키는 비어 있지 않은 최대 256바이트이며 줄바꿈·NUL을 허용하지 않습니다.

예를 들어 Rust `src/shared/output/mod.rs`의 `dispatch_event_json(key, payload)`를 확인했다면 해당 정의 파일을 `module`, 함수명을 `symbol`로 지정하고 `role="publish"`, `event_arg=0`, `bus="fixed"`, `bus_identity="application-events"`, `target="any"`를 사용할 수 있습니다. 프런트엔드 규칙에 같은 버스 ID를 명시하면 설정에 따른 연결로 표시합니다. 고정 버스·키·대상은 모두 설정에 따른 가정이라는 표시를 유지하며 자동으로 증명한 전달 경로로 표현하지 않습니다.

### 갱신과 상한

UTF-8 소스만 저장하며 색인·Git·디렉터리 제외 규칙을 적용합니다. 테스트 코드 포함 규칙은 질의 시 양쪽 이벤트 위치와 근거·핸들러 위치에 적용하며, 소스 경로는 색인 분석 전에도 제외 영역을 가립니다. 테스트 포함 설정 변경은 새 세대를 요청합니다. 소스, import한 버스·키·핸들러, 규칙, 분석 대상, 제외 설정이 바뀌면 새 세대를 만듭니다. 근거 파일이 바뀐 이전 연결은 갱신 전에도 숨깁니다. 원본 애플리케이션 파일은 변경하지 않습니다.

소스는 파일당 512 KiB, 세대당 64 MiB·4,096파일입니다. 이벤트와 소스 경로는 파일당 256개, 세대당 8,192개의 끝점 한도를 공유합니다. 규칙 기반 이벤트의 개별 직렬화 자료는 8 KiB까지입니다. JS/TS 바인딩 해석은 32단계, Rust 값 해석은 16단계로 제한합니다. 질의마다 후보 512개·출력 128개, 최신 소스 확인 128파일·4 MiB를 넘지 않습니다. 기존 read/search 출력 바이트 상한을 유지하고 입력 누락, 추출·질의 상한, 오래된 근거, 출력 생략 수를 표시합니다. 질의마다 작업공간 전체의 이벤트 관계를 다시 훑지 않습니다.

## 이전 설정 이름

기존 최상위 키와 이전 섹션은 계속 읽습니다. 같은 파일에 새 이름도 있으면 새 이름이 우선하며, 잘못된 새 값은 낮은 설정 계층으로 대체합니다. 자동 이전은 저장소 파일에만 적용하고 전역 파일은 수정하지 않습니다.

| 이전 경로 | 새 경로 |
|---|---|
| `tool_output.is_redact_enabled` | `output.is_redact_enabled` |
| `tool_output.is_overview_stats_enabled` | `output.overview.is_stats_enabled` |
| `search.result_threshold` | `output.search.detail_file_limit` |
| `search.search_overview_file_limit` | `output.search.overview_file_limit` |
| `search.search_detail_snippet_max_lines` | `output.search.snippet_max_lines` |
| `search.search_detail_symbol_limit` | `output.search.symbol_limit` |
| `search.search_detail_byte_cap` | `output.search.max_bytes` |
| `search.search_literal_max_len` | `output.search.literal_max_chars` |
| `search.search_literal_limit` | `output.search.literal_limit` |
| `search.search_anchor_snippet_limit` | `output.search.anchor_snippet_limit` |
| `tool_output.read_output_byte_cap` | `output.read.max_bytes` |
| `tool_output.grep_max_columns` | `output.grep.max_columns` |
| `caller_context.caller_context_default` | `output.context.is_enabled` |
| `caller_context.caller_list_cap` | `output.context.caller_limit` |
| `caller_context.callee_list_cap` | `output.context.callee_limit` |
| `caller_context.annotation_sub_budget` | `output.context.max_bytes` |
| `caller_context.common_name_threshold` | `output.context.common_name_threshold` |
| `caller_context.caller_omit_def_threshold` | `output.context.caller_omit_def_threshold` |
| `index.index_path` | `index.path` |
| `index.max_file_size` | `index.max_file_bytes` |
| `caller_context.navigation_store_references` | `index.store_references` |
| `refresh.watch` | `index.refresh.watch` |
| `refresh.watch_debounce_ms` | `index.refresh.watch_debounce_ms` |
| `refresh.index_staleness_ms` | `index.refresh.index_staleness_ms` |
| `refresh.indexer_auto_restart` | `index.refresh.indexer_auto_restart` |
| `language_support.is_document_support_enabled` | `index.language_support.is_document_support_enabled` |
| `language_support.is_shell_support_enabled` | `index.language_support.is_shell_support_enabled` |
| `language_support.is_infrastructure_support_enabled` | `index.language_support.is_infrastructure_support_enabled` |
| `language_support.is_interface_support_enabled` | `index.language_support.is_interface_support_enabled` |
| `language_support.is_build_support_enabled` | `index.language_support.is_build_support_enabled` |
| `caller_context.navigation_context_default` | `output.navigation.is_enabled` |
| `caller_context.navigation_callsite_budget` | `output.navigation.callsite_budget` |
| `caller_context.scan_cap` | `output.navigation.scan_limit` |
| `redact.*` | `output.redact.*` |
| `macro_expansion.*` | `output.macro_expansion.*` |
| `event_navigation.*` | `output.event_navigation.*` |
| `analysis.navigation.*` (v18) | `output.navigation.*` |
| `analysis.macro_expansion.*` (v18) | `output.macro_expansion.*` |
| `analysis.event_navigation.*` (v18) | `output.event_navigation.*` |

### 검색 표시 압축과 grep 원문 묶음

검색은 Jev 근거를 구성한 뒤 표시용 주석만 압축합니다. 비호출 참조는 반환된 모든 파일·행 위치를 유지하고 원문 미리보기를 줄이며, 미해결 호출 이름은 한 줄로 묶습니다. 반복 분석 안내는 처음에 `[N1]`처럼 정의하고 이후 해당 위치에서 참조합니다. 코드 본문은 압축하지 않습니다. 확인된 관계의 위치 형식은 유지하며, 완전한 본문에 이미 보이는 리터럴 행의 중복 안내만 생략합니다. 전달 예산 때문에 미표시한 본문은 관련성 판정과 구분됩니다.

`grep`의 `view="source_grouped"`는 모든 반환 파일의 제목 아래에 원문 행을 묶습니다. `expand="none"`이면 `42:일치`와 `43-문맥`을 유지하고, callable 확장도 지원합니다. 파일·행·페이지 순서, 마스킹, 줄 길이 및 응답 한도를 유지하며 선언·관계 분석은 수행하지 않습니다. 기존 `view="source"`의 행별 경로 형식은 그대로입니다. 새 보기는 content 모드 전용이며 read에는 적용되지 않습니다.
