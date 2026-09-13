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

1. 클라이언트에서 `initial_instructions`를 한 번 호출합니다. 탐색 안내와 루트 개요를 반환하며, 모노레포에서는 선택 가능한 범위와 언어를 표시합니다.
2. 표시된 경로가 원하는 저장소인지 확인합니다. 색인 준비 중 안내가 나오면 완료 후 `overview`를 다시 호출합니다.
3. 알고 있는 소스 파일을 `find`로 찾고 `read`로 읽습니다. 색인이 완료된 뒤 알려진 심볼을 `search`로 검색해 같은 파일이 나오는지 확인합니다.

바이너리를 찾지 못하면 클라이언트의 `PATH`를, 다른 저장소가 나오면 작업 디렉터리를 확인하세요. 시작·설정 오류는 stderr 로그에 표시하며 stdout은 MCP JSON-RPC 전용입니다.

## 탐색 도구 사용

| 도구 | 용도 | 주요 인자 |
|---|---|---|
| `initial_instructions` | 탐색 안내를 한 번 읽기 | 없음 |
| `overview` | 저장소·폴더·파일 구조 확인 | `path`, `format` |
| `search` | 순위가 매겨진 심볼과 발췌로 구현 찾기 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context` |
| `find` | glob으로 경로 찾기, 최근 수정 순 | `pattern`, `path`, `include_ignored` |
| `grep` | 실제 파일을 정규식으로 검색 | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | 줄 번호와 함께 원문 읽기 | `file_path`, `offset`, `limit` |

동작이나 구현 위치를 찾을 때는 `search`, 정확한 식별자·주석·방금 수정한 내용에는 `grep`을 사용합니다. `grep.pattern`은 정규식이므로 코드의 특수문자를 그대로 찾으려면 이스케이프해야 합니다. JSON 문자열 이스케이프는 별도입니다. `grep` 기본 출력은 줄 번호가 있는 `content`이고, `files_with_matches`와 `count`는 경로 또는 개수를 반환합니다. `read`는 `path`/`file`, 1부터 시작하는 양끝 포함 `start_line`/`end_line` 별칭도 받습니다.

모노레포에서 `overview`로 선택한 폴더는 이후 `search`의 범위가 됩니다. 파일은 부모 폴더를 선택합니다. 명시적 `workspace_scope`가 우선하며 `all`/`전체`는 저장소 전체입니다. 구현 위치를 모르면 읽기 전용 전체 검색으로 시작해 실제 경로를 확인한 뒤 좁힙니다. 선택된 범위는 임의로 확대하지 않습니다. 상위 결과는 상세 발췌, 나머지는 제한된 목록으로 표시합니다. 출력이 잘리면 질의를 좁히거나 안내된 줄 범위를 읽으세요.

MCP `read`·`grep`은 원문과 함께 해당 범위를 감싸는 선언과 호출 관계를 표시합니다. 호출 대상이 정해지면 정의 파일과 줄 번호를 붙입니다. 같은 파일의 상수 참조에는 정의 위치와 초기값 미리보기를 표시하고, 이름이 모호하면 생략합니다. 색인 정보는 최근 편집을 아직 반영하지 않았을 수 있습니다.

도구는 설정된 파일시스템 범위를 읽기 전용으로 다룹니다. 서버 자체는 색인을 저장하고, 자동 업데이트가 켜져 있으면 저장소 설정을 생성·전환합니다. MCP 리소스와 프롬프트는 등록하지 않습니다.

## 제외 목록과 출력 설정

키별 우선순위는 `<repo>/.codemap/config.toml` → `$CODEMAP_HOME/config.toml` (기본 `~/.codemap/config.toml`) → 내장 기본값입니다. 활성화된 저장소 키가 전역값보다 우선하며, 전역값을 상속하려면 해당 키를 주석 처리합니다.

자동 심볼·호출 관계에서는 기본적으로 테스트 영역을 제외합니다. `[exclude].should_include_test_code = true`이면 포함합니다. `test_file_patterns`와 언어별 `test_attributes`, `test_decorators`, `test_calls` 목록에서 사용자 규칙을 추가하거나 내장 항목을 지울 수 있습니다. 명시한 목록은 상속값을 대체하고 `[]`이면 해당 목록을 끕니다. 직접 `read`·`grep`한 원문은 유지합니다. 기본값과 예시는 [테스트 코드 문맥](./docs/configuration.ko.md#테스트-코드-문맥)을 참고하세요.

첫 MCP 실행에서 설정 파일이 없으면 **공통 제외 폴더와 감지한 프로젝트 종류의 재귀 glob**을 생성합니다. 공통 목록에는 `.git`, `.idea`, `.vscode`, `.vs`, `.codemap`과 지원하는 다른 VCS 내부 폴더가 포함됩니다. JS/TS 프로젝트는 `**/node_modules`, `**/dist`, `**/build`, 프레임워크 출력과 캐시의 glob을 추가하고, Python·Rust 등도 해당 종류의 glob을 추가합니다. 같은 패턴은 한 번만 기록하며 프로젝트를 발견한 폴더와 관계없이 작업공간 전체에 적용합니다.

```toml
[exclude]
# 혼합 저장소 예시입니다. 필요한 항목을 관리하세요.
excluded_directories = [
    ".git", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index",
    "**/node_modules", "**/dist", "**/build",
    "**/.venv", "**/__pycache__",
    "**/target",
]
```

**버전 6 이전 설정의 제외 목록은 한 번 전환합니다. `codemap-config-version: 6`부터는 이 배열을 자동 갱신하지 않습니다. 새 규칙이 필요하면 직접 관리하세요.** 삭제한 항목은 재시작해도 복원하지 않습니다. `config_auto_update`는 설정 생성과 일반 스키마 추가를 제어하며, 버전 6 이후 제외 배열을 자동 보충하는 옵션이 아닙니다. 자동 쓰기를 껐다면 [수동 전환 안내](./docs/configuration.ko.md#자동-작성을-껐을-때-수동-전환)를 따르세요.

`build`는 모든 깊이의 해당 폴더, `./build`는 루트만, `apps/web/build`는 지정 프로젝트만 제외합니다. 명시한 배열은 선택적 기본 목록을 대체합니다. `[]`는 선택적 제외 해제, 키 생략은 전역/기본값 상속입니다. `.gitignore`, 전역 Git ignore, `.git/info/exclude`, `.codemapignore`는 별도로 적용합니다. VCS 내부, `.codemap`, `.codemap-index`, 실제 색인 위치는 배열과 무관하게 탐색에서 제외합니다. `find`·`grep`의 `include_ignored: true`는 선택적 제외를 우회하며, 직접 `read`는 파일시스템 권한을 따릅니다.

MCP는 시작 시 존재하는 설정 디렉터리를 감시해 약 1000ms 후 재읽기합니다. 제외 배열이나 언어 지원을 직접 바꾸면 전체 색인 갱신을 요청하고, 출력 상한·파일시스템 권한은 다음 요청에 적용합니다. `index_path`, `watch`, `watch_debounce_ms`를 바꿨거나 설정 감시를 사용할 수 없었다면 서버를 재시작하세요. 모든 키와 공통 목록, 프로젝트 감지 규칙, 유효값·권한·적용 시점은 [설정 상세 문서](./docs/configuration.ko.md)에 있습니다.

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

다음 선택형 그룹은 `[language_support]`에서 기본값이 모두 `false`입니다.

| 키 | 그룹 |
|---|---|
| `is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

이 설정은 색인 기반 탐색과 감시 갱신을 제어합니다. 그룹이 비활성 상태여도 실시간 `find`, `grep`, `read`와 CLI의 직접 `parse`는 사용할 수 있습니다. 언어별 공개·테스트·폐기 표시, 정적 관계와 한계는 [추출 세부 규칙](./docs/language-support-checklist.ko.md#언어별-추출-규칙)을 참고하세요.

## CLI

```text
codemap-search mcp
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

## 개발 중 검증

소스 저장소의 `apps/codemap-search`에서 실행합니다. `./verify`는 현재 코드를 release로 빌드한 뒤 25개 언어의 작은 호출·제외·상수 문맥 검사를 실행합니다.

```sh
./verify
./verify --language rust --language typescript --profile structural
./verify test
./verify public --language python --repository django/django
./verify public --dry-run
```

`test`는 기존 `cargo check`와 `cargo test`를 실행합니다. `public`은 고정 공개 저장소의 준비·측정·검증을 연결하므로 다운로드와 별도 언어 파서가 필요할 수 있습니다. `--binary`로 설치 바이너리를 지정하면 빌드를 생략합니다. 결과와 로그는 `--cache` 또는 `CODEMAP_VALIDATION_CACHE`가 지정한 곳에 실행별로 저장하며, 기본 경로는 `~/.cache/codemap-public-validation`입니다. 실패·판정 보류를 성공으로 처리하지 않습니다. [명령과 결과 해석](./docs/development-language-commands.ko.md)을 참고하세요.

## 색인·진단·제한

MCP 서버는 기본적으로 `.codemap/index`에 색인을 생성하거나 기존 색인을 읽습니다. 정상적인 파일 감시자는 편집 이벤트를 기본 500ms 동안 모아 해당 경로만 갱신합니다. Git HEAD 변경과 큰 변경 묶음은 전체 탐색으로 처리합니다. 감시가 꺼져 있거나 사용할 수 없으면 `search`·`overview`가 `index_staleness_ms`에 따른 요청 기반 갱신을 사용합니다. `read`, `find`, `grep`은 디스크를 직접 읽습니다.

- `max_file_size` 기본값인 1 MiB보다 큰 파일은 색인·코드맵에서 건너뜁니다.
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
