# codemap-search

한국어 | [English](./README.md)

코딩 에이전트용 독립 실행형 MCP stdio 서버와 CLI입니다. 저장소 구조를 보고, 심볼·설명·문자열을 BM25로 검색한 뒤 내장 `read`, `find`, `grep`으로 원문을 확인합니다. Tree-sitter 문법, Tantivy와 ripgrep 라이브러리를 하나의 Rust 바이너리에 포함하므로 시스템 `rg`, 언어 서버, 별도 런타임, 계정이나 API 키가 필요하지 않습니다.

## 설치

Rust/Cargo가 설치되어 있다면 다음 명령을 사용합니다.

```sh
cargo install codemap-search
codemap-search --version
```

`~/.cargo/bin`이 `PATH`에 있어야 합니다. macOS/Linux에서 운영체제에 맞는 설치 파일을 받으려면 [설치 스크립트 안내](./docs/distribution/curl-installer.ko.md)를 따르세요. 소스 빌드, 버전 선택, Homebrew·WinGet 제공 상태는 [설치 채널 개요](./docs/distribution/index.ko.md)에 있습니다.


간결한 grep 원문은 `view="source_grouped"`, `expand="none"`으로 요청합니다. 반복 경로를 파일 제목으로 묶고 원문 행과 줄 번호는 유지합니다. 기존 `view="source"` 형식은 바뀌지 않습니다. 검색 주석은 Jev 판정 후 표시용 복사본에서 압축하며, 명시한 Codex 출력 한도에 따른 전달 보호가 미표시 본문의 읽기 범위를 남깁니다. [설정 문서](./docs/configuration.ko.md#클라이언트-전달-한도)를 참고하세요.
## MCP 클라이언트 등록

클라이언트가 **탐색할 저장소를 작업 디렉터리로 지정**해 `codemap-search mcp`를 실행해야 합니다. 사용자 전역 등록으로 같은 바이너리를 여러 프로젝트에서 재사용할 수 있지만, 실행 위치는 클라이언트 설정을 확인하세요. 사용자 홈 자체는 거부하며 `~/work/project` 같은 하위 프로젝트는 허용합니다.

### Claude Code

사용자 계정의 여러 프로젝트에서 사용하려면:

```sh
claude mcp add --scope user codemap-search -- codemap-search mcp
```

팀과 공유하는 `.mcp.json`에 등록하려면 해당 프로젝트에서:

```sh
claude mcp add --scope project codemap-search -- codemap-search mcp
```

`--scope`를 생략하면 기본값은 `local`입니다. 현재 프로젝트에서 본인만 사용하며, `~/.claude.json`의 해당 프로젝트 경로 아래에 저장됩니다. 공유용 `project` 범위와 다릅니다. 자세한 차이는 [Claude Code 공식 안내](https://code.claude.com/docs/en/mcp)를 참고하세요.

### Codex

```sh
codex mcp add codemap-search -- codemap-search mcp
```

또는 `~/.codex/config.toml`에 같은 서버 항목을 추가합니다.

```toml
[mcp_servers.codemap-search]
command = "codemap-search"
args = ["mcp"]
```

### OpenCode

전역 `~/.config/opencode/opencode.json` 또는 프로젝트의 `opencode.json`에 추가합니다.

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "codemap-search": {
      "type": "local",
      "command": ["codemap-search", "mcp"],
      "enabled": true
    }
  }
}
```

[OpenCode MCP 안내](https://opencode.ai/docs/mcp-servers/)의 설정 형식입니다. 다른 설정 스키마를 사용하는 버전이라면 해당 클라이언트 버전의 문서를 확인하세요.

## 첫 연결 확인

1. 클라이언트에서 `initial_instructions`를 한 번 호출합니다. 탐색 안내와 루트 개요를 반환하며, 모노레포에서는 선택 가능한 범위와 언어를 표시합니다. [Jev 단계](#선택적-jev-판단-단계)를 하나라도 켰다면 `task_query`와 집중된 `questions`를 아래 계약에 맞춰 등록합니다.
2. 표시된 경로가 원하는 저장소인지 확인합니다. 색인 준비 중 안내가 나오면 완료 후 `overview`를 다시 호출합니다.
3. 알고 있는 소스 파일을 `find`로 찾고 `read`로 읽습니다. 색인이 완료된 뒤 알려진 심볼을 `search`로 검색해 같은 파일이 나오는지 확인합니다.

바이너리를 찾지 못하면 클라이언트의 `PATH`를, 다른 저장소가 나오면 작업 디렉터리를 확인하세요. 시작·설정 오류는 stderr 로그에 표시하며 stdout은 MCP JSON-RPC 전용입니다.

## 탐색 도구 사용

| 도구 | 용도 | 주요 인자 |
|---|---|---|
| `initial_instructions` | 탐색 안내와 Jev 작업 목적 등록 | `task_query`, `questions`, `match` (Jev search를 켜면 목적·질문은 필수) |
| `overview` | 저장소·폴더·파일 구조 확인; 저장소 루트와 모노레포 프로젝트 루트는 기본적으로 색인 파일 언어 통계 포함 | `path`, `format` |
| `search` | 순위가 매겨진 심볼과 발췌로 구현 찾기 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context` |
| `find` | glob 또는 basename 정규식으로 파일·폴더 찾기, 최근 수정 순 | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | 실제 파일을 정규식으로 검색 | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | 줄 번호와 함께 원문 읽기 | `file_path`, `offset`, `limit` |
| `analyze` | 인덱스 용량 또는 기록된 읽기 동작을 압축 JSON으로 분석 | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |

MCP 도구 `search`·`read`·`grep`은 공통 불리언 옵션 `include_seen`을 제공하며 기본값은 `false`입니다. 이미 전달한 변경 없는 소스와 같은 응답 안의 반복 소스를 생략합니다. 새 원문은 유지하고 응답마다 `<--removed duplicated-->` 안내를 한 번만 남깁니다. `include_seen=true`는 이 중복 제거만 우회하며 Jev·마스킹·출력 상한은 그대로 적용합니다. `find`는 중복 제거에서 제외하며 매번 현재 결과를 반환합니다. 전달 이력은 연결별로 관리하고 초기화·작업 등록·설정 변경 시 지웁니다. 파일이 바뀌면 새 원문을 반환하며 Jev가 꺼져 있어도 중복 제거는 적용됩니다.

저장소 루트와 모노레포 프로젝트 루트 `overview`는 기본적으로 색인 파일 언어 통계를 포함합니다. 프로젝트 루트는 해당 프로젝트의 색인 파일만 집계합니다. `[output.overview].is_stats_enabled = false`이면 해당 섹션을 생략합니다. 집계 불가·대기 파일이 있으면 부분 결과로 표시합니다.

동작이나 구현 위치를 찾을 때는 `search`, 정확한 식별자·주석·방금 수정한 내용에는 `grep`을 사용합니다. `grep.pattern`은 정규식이므로 코드의 특수문자를 그대로 찾으려면 이스케이프해야 합니다. JSON 문자열 이스케이프는 별도입니다. `grep` 기본 출력은 줄 번호가 있는 `content`이고, `files_with_matches`와 `count`는 경로 또는 개수를 반환합니다. `read`는 `path`/`file`, 1부터 시작하는 양끝 포함 `start_line`/`end_line` 별칭도 받습니다.

모노레포에서 `overview`로 선택한 폴더는 이후 `search`의 범위가 됩니다. 파일은 부모 폴더를 선택합니다. 명시적 `workspace_scope`가 우선하며 `all`/`전체`는 저장소 전체입니다. 구현 위치를 모르면 읽기 전용 전체 검색으로 시작해 실제 경로를 확인한 뒤 좁힙니다. 선택된 범위는 임의로 확대하지 않습니다. 상위 결과는 상세 발췌, 나머지는 제한된 목록으로 표시합니다. 출력이 잘리면 질의를 좁히거나 안내된 줄 범위를 읽으세요.

MCP `read`·`grep`은 원문과 함께 해당 범위를 감싸는 선언과 호출 관계를 표시합니다. 호출 대상이 정해지면 정의 파일과 줄 번호를 붙입니다. 같은 파일의 상수 참조에는 정의 위치와 초기값 미리보기를 표시하고, 이름이 모호하면 생략합니다. 색인 정보는 최근 편집을 아직 반영하지 않았을 수 있습니다.

관련 결과에는 이벤트 지도와 저장 위치를 콜백·인자·데이터 소비 위치에 연결하는 `Source routes`도 표시합니다. 정적 후보이지 실제 실행·전달의 증명은 아니며, 근거 파일이 바뀐 연결은 갱신 전까지 숨깁니다. `debug`는 출력을 바꾸지 않습니다. 지원 관계와 한계는 [탐색 출력 계약](./docs/value-navigation.ko.md)과 [소스 경로 탐색 계약](./docs/source-routes.ko.md)을 참고하세요.

도구는 설정된 파일시스템 범위를 읽기 전용으로 다룹니다. 서버 자체는 색인과 파일 내용 반환 기록을 저장하고, 자동 업데이트가 켜져 있으면 저장소 설정을 생성·전환합니다. MCP 리소스와 프롬프트는 등록하지 않습니다.

## 제외 목록과 출력 설정

키별 우선순위는 `<repo>/.codemap/config.toml` → `$CODEMAP_HOME/config.toml` (기본 `~/.codemap/config.toml`) → 내장 기본값입니다. 활성화된 저장소 키가 전역값보다 우선하며, 전역값을 상속하려면 해당 키를 주석 처리합니다.

MCP 응답에서 탐지한 API 키·토큰·비밀번호·비밀키를 기본으로 가립니다. `[output].is_redact_enabled = false`로 끌 수 있습니다. 검색 일치·순위는 원문을 기준으로 유지하며 파일과 로컬 색인은 변경하지 않습니다. Tree-sitter와 패턴 검사를 함께 사용하며, `[output.redact]`에서 민감 필드명·정규식·정확한 값 예외를 추가할 수 있습니다. 알려지지 않은 형식은 누락할 수 있습니다. 적용 범위와 한계는 [민감값 마스킹](./docs/configuration.ko.md#민감값-마스킹)을 참고하세요.

자동 심볼·호출 관계에서는 기본적으로 테스트 영역을 제외합니다. `[output.context.exclude].should_include_test_code = true`이면 포함합니다. `test_file_patterns`와 언어별 `test_attributes`, `test_decorators`, `test_calls` 목록에서 사용자 규칙을 추가하거나 내장 항목을 지울 수 있습니다. 명시한 목록은 상속값을 대체하고 `[]`이면 해당 목록을 끕니다. 직접 `read`·`grep`한 원문은 유지합니다. 기본값과 예시는 [테스트 코드 문맥](./docs/configuration.ko.md#테스트-코드-문맥)을 참고하세요.

첫 MCP 실행에서 설정 파일이 없으면 **공통 제외 폴더와 감지한 프로젝트 종류의 재귀 glob**을 생성합니다. 공통 목록에는 `.git`, `.idea`, `.vscode`, `.vs`, `.codemap`과 지원하는 다른 VCS 내부 폴더가 포함됩니다. JS/TS 프로젝트는 `**/node_modules`, `**/dist`, `**/build`, 프레임워크 출력과 캐시의 glob을 추가하고, Python·Rust 등도 해당 종류의 glob을 추가합니다. 같은 패턴은 한 번만 기록하며 프로젝트를 발견한 폴더와 관계없이 작업공간 전체에 적용합니다.

```toml
[index.exclude]
# 혼합 저장소 예시입니다. 필요한 항목을 관리하세요.
excluded_directories = [
    ".git", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index",
    "**/node_modules", "**/dist", "**/build",
    "**/.venv", "**/__pycache__",
    "**/target",
]
```

생성된 제외 배열은 직접 관리합니다. 삭제한 항목은 재시작해도 복원하지 않습니다. `config_auto_update`는 설정 생성과 스키마 갱신을 제어하며, 이 배열을 계속 보충하는 옵션이 아닙니다. 버전 6 이전 설정을 갱신한다면 [설정 전환 안내](./docs/configuration.ko.md#설정-읽기와-자동-작성)를 따르세요.

`build`는 모든 깊이의 해당 폴더, `./build`는 루트만, `apps/web/build`는 지정 프로젝트만 제외합니다. 명시한 배열은 선택적 기본 목록을 대체합니다. `[]`는 선택적 제외 해제, 키 생략은 전역/기본값 상속입니다. `.gitignore`, 전역 Git ignore, `.git/info/exclude`, `.codemapignore`는 별도로 적용합니다. VCS 내부, `.codemap`, `.codemap-index`, 실제 색인 위치는 배열과 무관하게 탐색에서 제외합니다. `find`·`grep`의 `include_ignored: true`는 선택적 제외를 우회하며, 직접 `read`는 파일시스템 권한을 따릅니다.

MCP는 시작 시 존재하는 설정 디렉터리를 감시해 약 1000ms 후 재읽기합니다. 제외 배열이나 언어 지원을 직접 바꾸면 전체 색인 갱신을 요청하고, 출력 상한·파일시스템 권한은 다음 요청에 적용합니다. `index.path`, `index.refresh.watch`, `index.refresh.watch_debounce_ms`를 바꿨거나 설정 감시를 사용할 수 없었다면 서버를 재시작하세요. 모든 키와 공통 목록, 프로젝트 감지 규칙, 유효값·권한·적용 시점은 [설정 상세 문서](./docs/configuration.ko.md)에 있습니다.

## 선택적 Jev 판단 단계

Jev는 search·read·grep의 완전한 함수 본문을 등록한 작업 질문으로 판단하며 기본으로 꺼져 있습니다. `output.jev.enabled=true`로 켜고 `scope`로 적용 도구를 선택합니다(기본 `["overview", "search", "read", "grep"]`). `enabled=false`이면 모든 필터를 끄고, `scope=[]`이면 아무 도구에도 적용하지 않습니다. API 키는 `.codemap/auth.toml`의 `[jev].api_key`에 저장합니다. 전역 `$CODEMAP_HOME/auth.toml`(미지정 시 `~/.codemap/auth.toml`)도 지원하며, 저장소 auth → 전역 auth → 고정된 `TYPESAFE_API_KEY` 환경 변수 순서로 읽습니다. 인증 파일은 색인과 기본 find/grep에서 제외합니다.

주 에이전트는 작업의 대상·방향·범위를 보존한 `task_query`와 집중된 예/아니오 질문 목록 `questions`를 `initial_instructions`에 한 번 등록합니다. 질문마다 `question`, `when_true`, `when_false`를 넣습니다. ID는 서버가 생성하며 `match`는 `all`(기본) 또는 `any`로 지정합니다. 작업이 바뀌면 다시 등록하며, 검색어가 작업 질문을 대체하지 않습니다. 켜진 상태에서는 텍스트만 등록하면 인수 오류를 반환합니다.

켜진 search·read·grep 필터는 마스킹한 목적·질문·함수 근거를 TypeSafe에 전송하고 개별 판단을 코드에서 조합합니다. 불확실한 근거는 유지하고 생략한 본문에는 원래 소스 위치를 남깁니다. 루트 외 overview는 같은 기준으로 선언 목록을 걸러내며, 판정용 본문은 응답과 중복 제거 이력에 넣지 않습니다. 루트 overview·find는 로컬 도구로 유지합니다. Jev 생략 없이 read로 복구하려면 `output.jev.scope`에서 `"read"`를 빼거나 `output.jev.enabled=false`로 끕니다. `include_seen=true`는 공통 중복 제거만 우회합니다. 제공자 실패 시 일반 선택 결과를 유지하며, 독립적인 MCP 중복 전달 규칙은 그대로 적용됩니다. 등록 예시·한도·불확실성·진단은 [Jev 참조](./docs/configuration.ko.md#선택적-jev-판단-단계)를 참고하세요.

## 지원 언어와 형식

| 언어 | 확장자 |
|---|---|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript / TSX | `.ts`, `.tsx`, `.mts`, `.cts` |
| JavaScript / JSX | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Go | `.go` |
| Java | `.java` |
| Kotlin | `.kt`, `.kts` |
| C | `.c` |
| C++ | `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` |
| C# | `.cs` |
| PHP | `.php` |
| Ruby | `.rb` |
| Lua | `.lua` |
| Assembly / GAS | `.s`, `.S`, `.asm` |
| Swift | `.swift` |
| Dart | `.dart` |
| Scala | `.scala`, `.sc` |
| Groovy / Gradle | `.groovy`, `.gradle` |
| PowerShell | `.ps1`, `.psm1` |
| SQL | `.sql` |

JSON/JSONC, TOML, YAML, HTML/XML 파생 형식, CSS/Less, Sass와 Vue·Astro·Svelte 컴포넌트를 지원합니다. 이 버전의 지원 등록부에는 JSON5와 SCSS가 없습니다. SQL은 선언과 리터럴을 추출하며 호출 관계는 만들지 않습니다.

다음 선택형 그룹은 `[index.language_support]`에서 기본값이 모두 `false`입니다.

| 키 | 그룹 |
|---|---|
| `index.language_support.is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `index.language_support.is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `index.language_support.is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `index.language_support.is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `index.language_support.is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

이 설정은 색인 기반 탐색과 감시 갱신을 제어합니다. 그룹이 비활성 상태여도 실시간 `find`, `grep`, `read`와 CLI의 직접 `parse`는 사용할 수 있습니다. 언어별 공개·테스트·폐기 표시, 정적 관계와 한계는 [추출 세부 규칙](./docs/language-support-checklist.ko.md#언어별-추출-규칙)을 참고하세요.

## CLI

```text
codemap-search mcp [--no-call-log]
codemap-search analyze index [--path DIR] [--sort stored|size|lines|symbols|literals|path] [OPTIONS]
codemap-search analyze reads [--path DIR] [--days 1..30] [--sort reads|bytes|size|last|path] [OPTIONS]
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

### 인덱스와 최근 읽기 동작 분석

소스를 다시 파싱하지 않고 커밋된 색인이나 기록된 MCP 읽기 동작을 확인합니다.

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
```

MCP `analyze` 도구도 현재 작업공간에 같은 분석을 제공합니다. 읽기 기록은 질의·소스·응답 내용 대신 경로와 지표를 저장하며 30일 후 만료됩니다. `codemap-search mcp --no-call-log`는 새 기록만 끄고 만료 정리는 유지합니다. 반환 바이트는 토큰 수가 아니며 반복 읽기도 서로 다른 구간일 수 있습니다. CLI/MCP 옵션·집계·보존 정책은 [분석 참조](./docs/analysis.ko.md)를 참고하세요.

## 개발 중 검증

소스 저장소의 `apps/codemap-search`에서 실행합니다. `./verify`는 현재 코드를 release로 빌드한 뒤 25개 언어의 작은 호출·제외·상수 문맥 검사를 실행합니다.

```sh
./verify
./verify --language rust --language typescript --profile structural
./verify test
./verify public --language python --repository django/django
./verify public --language rust --language go --jobs 2
./verify public --dry-run
```

`test`는 기존 `cargo check`와 `cargo test`를 실행합니다. `public`은 고정 공개 저장소의 준비·측정·검증을 연결하므로 다운로드와 별도 언어 파서가 필요할 수 있습니다. Rust·Go를 포함한 저장소별 동시 작업 수는 `--jobs`로 지정하며 기본값은 2입니다. 각 저장소 내부 검사는 순차로 실행합니다. `--binary`로 설치 바이너리를 지정하면 빌드를 생략합니다. 결과와 로그는 `--cache` 또는 `CODEMAP_VALIDATION_CACHE`가 지정한 곳에 실행별로 저장하며, 기본 경로는 `~/.cache/codemap-public-validation`입니다. 실패·판정 보류를 성공으로 처리하지 않습니다. [명령과 결과 해석](./docs/development-language-commands.ko.md)을 참고하세요.

## 색인·진단·제한

MCP 서버는 기본적으로 `.codemap/index`에 색인을 생성하거나 기존 색인을 읽습니다. 정상적인 파일 감시자는 편집 이벤트를 기본 500ms 동안 모아 해당 경로만 갱신합니다. Git HEAD 변경과 큰 변경 묶음은 전체 탐색으로 처리합니다. 감시가 꺼져 있거나 사용할 수 없으면 `search`·`overview`가 `index.refresh.index_staleness_ms`에 따른 요청 기반 갱신을 사용합니다. `read`, `find`, `grep`은 디스크를 직접 읽습니다.

- `index.max_file_bytes` 기본값인 1 MiB보다 큰 파일은 색인·코드맵에서 건너뜁니다.
- `.txt`, 잠금 파일, source map, 압축·번들 파일에는 별도 파일 제외 규칙이 있습니다. `find`·`grep`의 `include_ignored`로 우회할 수 있고, 직접 `read`·`parse`도 가능합니다.
- 실행 중에 결정되는 경로나 호출 대상은 정적 분석으로 확인할 수 없습니다. 추정한 호출 관계는 원문에서 확인하세요.
- 단일 클라이언트용 순차 stdio 서버입니다. 여러 서버를 같은 색인 디렉터리로 동시에 실행하지 마세요.

진단은 stderr에 출력하며 기본 로그 필터는 `warn,codemap_search=info`입니다.

```sh
RUST_LOG=debug codemap-search mcp
```

[벤치마크](../../benchmark/README.md)와 [Docker 검증 도구](./docker/README.ko.md)에서 측정·검증 방법을 확인할 수 있습니다.

## 라이선스

MIT. [LICENSE](./LICENSE)를 참고하세요.
