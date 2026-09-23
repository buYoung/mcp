# codemap-search

한국어 | [English](./README.md)

코딩 에이전트용 독립 실행형 MCP stdio 서버와 CLI입니다. 저장소 구조를 보고, 심볼·설명·문자열을 BM25로 검색한 뒤 내장 `read`, `find`, `grep`으로 원문을 확인합니다. Tree-sitter 문법, Tantivy와 ripgrep 라이브러리를 하나의 Rust 바이너리에 포함하므로 시스템 `rg`, 언어 서버, 별도 런타임, 계정이나 API 키가 필요하지 않습니다. 직접 켜는 [Jev 판정](#선택-기능-jev-판정)만 API 키를 사용합니다.

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
| `overview` | 저장소·폴더·파일 구조 확인; 저장소 루트와 모노레포 프로젝트 루트는 기본적으로 색인 파일 언어 통계 포함 | `path`, `format`, `task_query` |
| `search` | 순위가 매겨진 심볼과 발췌로 구현 찾기 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context`, `task_query` |
| `find` | glob 또는 basename 정규식으로 파일·폴더 찾기, 최근 수정 순 | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | 실제 파일을 정규식으로 검색 | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | 줄 번호와 함께 원문 읽기 | `file_path`, `offset`, `limit` |
| `analyze` | 인덱스 용량 또는 기록된 읽기 동작을 압축 JSON으로 분석 | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |

저장소 루트와 모노레포 프로젝트 루트 `overview`는 기본적으로 색인 파일 언어 통계를 포함합니다. 프로젝트 루트는 해당 프로젝트의 색인 파일만 집계합니다. `[output.overview].is_stats_enabled = false`이면 해당 섹션을 생략합니다. 집계 불가·대기 파일이 있으면 부분 결과로 표시합니다.

동작이나 구현 위치를 찾을 때는 `search`, 정확한 식별자·주석·방금 수정한 내용에는 `grep`을 사용합니다. `grep.pattern`은 정규식이므로 코드의 특수문자를 그대로 찾으려면 이스케이프해야 합니다. JSON 문자열 이스케이프는 별도입니다. `grep` 기본 출력은 줄 번호가 있는 `content`이고, `files_with_matches`와 `count`는 경로 또는 개수를 반환합니다. `read`는 `path`/`file`, 1부터 시작하는 양끝 포함 `start_line`/`end_line` 별칭도 받습니다.

모노레포에서 `overview`로 선택한 폴더는 이후 `search`의 범위가 됩니다. 파일은 부모 폴더를 선택합니다. 명시적 `workspace_scope`가 우선하며 `all`/`전체`는 저장소 전체입니다. 구현 위치를 모르면 읽기 전용 전체 검색으로 시작해 실제 경로를 확인한 뒤 좁힙니다. 선택된 범위는 임의로 확대하지 않습니다. 상위 결과는 상세 발췌, 나머지는 제한된 목록으로 표시합니다. 출력이 잘리면 질의를 좁히거나 안내된 줄 범위를 읽으세요.

MCP `read`·`grep`은 원문과 함께 해당 범위를 감싸는 선언과 호출 관계를 표시합니다. 호출 대상이 정해지면 정의 파일과 줄 번호를 붙입니다. 같은 파일의 상수 참조에는 정의 위치와 초기값 미리보기를 표시하고, 이름이 모호하면 생략합니다. 색인 정보는 최근 편집을 아직 반영하지 않았을 수 있습니다.

도구는 설정된 파일시스템 범위를 읽기 전용으로 다룹니다. 서버 자체는 색인과 파일 내용 반환 기록을 저장하고, 자동 업데이트가 켜져 있으면 저장소 설정을 생성·전환합니다. MCP 리소스와 프롬프트는 등록하지 않습니다. `task_query`는 [Jev 모드](#선택-기능-jev-판정)에서만 사용합니다. 모드가 꺼진 도구(기본값)는 이 값을 사용하지 않으며 네트워크로 아무것도 보내지 않습니다.

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

**버전 6 이전 설정의 제외 목록은 한 번 전환합니다. `codemap-config-version: 6`부터는 이 배열을 자동 갱신하지 않습니다. 새 규칙이 필요하면 직접 관리하세요.** 삭제한 항목은 재시작해도 복원하지 않습니다. `config_auto_update`는 설정 생성과 일반 스키마 추가를 제어하며, 버전 6 이후 제외 배열을 자동 보충하는 옵션이 아닙니다. 자동 쓰기를 껐다면 [수동 전환 안내](./docs/configuration.ko.md#자동-작성을-껐을-때-수동-전환)를 따르세요.

`build`는 모든 깊이의 해당 폴더, `./build`는 루트만, `apps/web/build`는 지정 프로젝트만 제외합니다. 명시한 배열은 선택적 기본 목록을 대체합니다. `[]`는 선택적 제외 해제, 키 생략은 전역/기본값 상속입니다. `.gitignore`, 전역 Git ignore, `.git/info/exclude`, `.codemapignore`는 별도로 적용합니다. VCS 내부, `.codemap`, `.codemap-index`, 실제 색인 위치는 배열과 무관하게 탐색에서 제외합니다. `find`·`grep`의 `include_ignored: true`는 선택적 제외를 우회하며, 직접 `read`는 파일시스템 권한을 따릅니다.

MCP는 시작 시 존재하는 설정 디렉터리를 감시해 약 1000ms 후 재읽기합니다. 제외 배열이나 언어 지원을 직접 바꾸면 전체 색인 갱신을 요청하고, 출력 상한·파일시스템 권한은 다음 요청에 적용합니다. `index.path`, `index.refresh.watch`, `index.refresh.watch_debounce_ms`를 바꿨거나 설정 감시를 사용할 수 없었다면 서버를 재시작하세요. 모든 키와 공통 목록, 프로젝트 감지 규칙, 유효값·권한·적용 시점은 [설정 상세 문서](./docs/configuration.ko.md)에 있습니다.

## 선택 기능: Jev 판정

두 가지 독립 모드에서 TypeSafe Jev API를 사용할 수 있습니다. 두 모드는 기본으로 꺼져 있으며, 모드를 켜고 호출에 `task_query`를 전달하기 전에는 아무것도 보내지 않습니다.

- **overview 추천** (`[analysis.jev].is_overview_enabled`): 저장소 루트 `overview` 뒤에 추천 색인 파일을 최대 24개 추가하고, 선언과 `read` 구간을 함께 표시합니다.
- **search 필터** (`[analysis.jev].is_search_filter_enabled`): `search`에서 작업과 무관하다고 판정한 표시 본문을 생략합니다. 기본 기준인 P(unrelated) >= 0.70은 보정하지 않은 잠정값입니다. 파일 제목·선언 행·순위 꼬리 목록은 유지하며, 생략한 본문은 `read`할 범위와 함께 나열합니다.

MCP 서버를 실행하는 환경에 키를 내보낸 뒤 전역 설정(`$CODEMAP_HOME/config.toml`, 기본 `~/.codemap/config.toml`)에서 모드를 켭니다. 키를 담을 환경 변수는 전역 파일의 `api_key_env`에서만 바꿀 수 있습니다.

```bash
export TYPESAFE_API_KEY=...
```

```toml
[analysis.jev]
is_overview_enabled = true
is_search_filter_enabled = true
```

검색어로 바꾸지 말고 사용자의 원래 요청을 전달하세요.

```json
{"name": "search", "arguments": {"query": "upload retry attempts", "task_query": "업로드 재시도가 세 번째 시도 뒤에 멈추는 이유는?"}}
```

켜진 모드는 도구 출력과 같은 마스킹을 거친 `task_query`와 제한된 색인 정보(overview) 또는 표시된 선언 본문(search)을 `api.typesafe.ai`로 보냅니다. `task_query`의 자유 문장은 인증정보 형식이 아니면 그대로 전송되므로 비밀값을 넣지 마세요. 켜진 호출은 끝에 `applied`, `bypassed`, `fallback` 중 한 줄을 추가하며, `applied`와 `fallback` 줄은 요청 수·입력/출력 토큰·경과 시간을 따로 보고합니다. `task_query`나 키가 없거나 색인 준비 중이거나 오류·시간 초과가 나면 일반 출력을 유지합니다. 판정기는 Rust에서 `codemap_search::jev`로 직접 호출할 수도 있으며, `cargo run --example jev_decisions -- --mock`은 Score·Choice·Noul 질문을 오프라인으로 보여 줍니다. 활성 조건, 전송 데이터, 한도와 모든 설정은 [선택 기능: Jev 판정](./docs/configuration.ko.md#선택-기능-jev-판정)을 참고하세요.

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

`codemap-search analyze index`는 저장된 인덱스를 즉시 분석하고, `codemap-search analyze reads`는 실행 시점의 최근 7일 읽기 동작을 분석합니다. 기본 출력은 사람이 읽기 쉬운 표입니다. 읽기 동작은 업데이트한 바이너리로 MCP를 다시 연결한 뒤부터 기록되며, 보고서는 명령을 실행할 때 생성됩니다.

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --offset 20 --limit 20 --sort bytes
codemap-search analyze reads --view summary --format json
codemap-search analyze index --help
codemap-search analyze reads --help
```

| 구분 | 출력 내용 | 집계 기준 |
| --- | --- | --- |
| 인덱스 용량 | 커밋된 파일·세그먼트·삭제 문서 수, 디스크 용량, 저장 JSON 크기, 정적 호출·참조 지점 수 | 기존 Tantivy 스냅샷을 읽어 메모리 SQLite에서 집계 |
| 언어·심볼 | 언어별 파일·행·심볼·공개 심볼·리터럴·문서 문자열 수, 심볼 종류별 테스트·문서화 플래그 | 저장된 추출 결과; 전체 저장소 파일이나 최신 소스 분석 결과와 다를 수 있음 |
| 파일·최신성 | 큰 저장 레코드의 경로·파일 크기·행·심볼·리터럴·최대 리터럴 크기, 변경·삭제·접근 불가 상태 | 크기와 변경 여부만 현재 파일 메타데이터로 확인; 소스 재파싱·인덱스 갱신 없음 |
| 현재/직전 기간 | 호출·오류·내용 반환 응답·고유 파일·읽은 횟수·응답량과 증감률 | 기본 최근 168시간과 직전 168시간; 기준값이 없으면 `n/a` |
| 도구·일별 | 도구별 호출·오류·내용 반환·고유 파일·읽은 횟수·응답량 비중·평균/최대 처리 시간, UTC 날짜별 추이 | `read`·`search`·`grep`의 기록만 집계; 날짜 표의 양끝은 하루 일부일 수 있음 |
| 파일별 읽기 | 경로·최신 기록 크기·전체/도구별 읽은 횟수·결과 반환량·비중·활동일·마지막 시각, 반복 조회 요약 | 한 성공 응답에서 같은 파일은 한 번만 계산 |

`--section` 대신 `index` 또는 `reads` 하위 명령을 선택합니다. 기본 정렬은 인덱스의 저장 JSON 크기, 읽기의 반환 횟수이며 파일 20개를 표시합니다. 자세한 옵션은 각 하위 명령의 `--help`에서 확인할 수 있습니다.

| 옵션 | 적용 대상 | 동작 |
| --- | --- | --- |
| `--path DIR` | 공통 | 분석할 저장소와 해당 저장소의 인덱스 설정·기록 DB 선택 |
| `--limit N`, `-n N` | 공통 | 파일 표시 수; 기본 20, `0`이면 전부 |
| `--offset N` | 공통 | 필터·정렬 이후 파일 행을 N개 건너뛰기; 출력의 다음 페이지 안내 사용 |
| `--sort KEY`, `-s KEY` | 공통 | 위 명령별 정렬 키 선택 |
| `--order asc\|desc` | 공통 | 정렬 방향; 기본은 경로 오름차순, 나머지는 내림차순 |
| `--filter TEXT`, `-f TEXT` | 공통 | 경로에 포함된 대소문자 구분 문자열; glob 아님 |
| `--view summary\|files\|full` | 공통 | 요약·그룹, 요약·파일, 전체 상세 보기; CLI 기본 `full` |
| `--format table\|json` | 공통 | 사람이 보는 표 또는 열 이름을 공유하는 압축 JSON |
| `--language NAME`, `-l NAME` | `index` | 언어 필터 |
| `--days N`, `-d N` | `reads` | 최근 1~30일; 기본 7일 |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | 특정 도구만 집계 |
| `--no-compare` | `reads` | 직전 동일 길이 기간 비교 생략; 16일 이상은 30일 보존 범위를 넘으므로 비교 불가 안내 |

합계는 표시 페이지가 아닌 필터에 맞는 전체 파일을 포함합니다. 읽기 경로 필터는 해당 파일을 반환한 호출을 선택합니다. 이때 `Response`는 선택된 호출의 전체 응답량이며 `Results`는 일치한 파일의 반환량입니다. 파일 관측이 없는 오류는 경로 필터 결과에 포함되지 않습니다. 인덱스의 전체 디스크 용량·세그먼트 지표는 파일 필터와 무관한 전체 인덱스 값입니다.

MCP에서는 `analyze` 도구를 직접 호출합니다. CLI와 같은 집계 결과를 사용하며 현재 서버의 작업공간만 분석합니다.

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

`target`은 `index|reads`이며 `limit`, `offset`, `sort`, `order`, `filter`, `view`를 지원합니다. `index`는 `language`, `reads`는 `days`, `tool`, `compare`도 받습니다. MCP 기본은 `view=files`, 파일 10개이며 인덱스는 `stored`, 읽기는 `bytes` 순입니다. 더 넓은 분석은 `view=full`로 요청합니다. `limit`은 1~100이고 `page.next_offset`으로 이어서 조회합니다.

응답은 긴 구분선·정렬 공백 없이 `summary`와 표별 `columns`·`rows` 배열을 사용합니다. 바이트·시간 단위를 키에 표시하고, 숫자·`null`을 유지하며, 해석상 주의점은 짧게 한 번만 제공합니다. 최대 8 KiB 또는 더 작은 `output.max_bytes` 안에서 완전한 행 단위로 줄입니다. 잘림은 `truncated`, 생략한 표는 `omitted_tables`로 알리며 합계와 다음 페이지 위치는 유지합니다. 민감정보 마스킹은 JSON 직렬화 전에 적용합니다. `analyze` 호출 자체는 읽기 기록에 추가되지 않습니다.

`Reads`는 성공 응답에 원문 행이나 검색 발췌가 포함된 파일마다 1회입니다. 경로 목록·선언/관계 전용 보기·실패 호출은 호출/응답량에는 포함되지만 읽은 횟수에는 포함되지 않습니다. `find`·`overview`·`analyze`와 내부 색인 읽기는 기록 대상이 아닙니다. 반복 조회는 동일 파일의 첫 반환 이후 횟수이며, 다른 구간이나 변경된 내용일 수 있으므로 낭비된 토큰으로 해석하면 안 됩니다.

`File size`는 분석 기간 안에서 마지막으로 기록한 디스크 크기입니다. 알 수 없으면 `?`로 표시하고 크기 합계에서 제외합니다. `Results`는 마스킹 전 파일별 원문/발췌 결과 블록의 UTF-8 바이트 수로 행 번호·경로 접두사·해당 블록의 안내도 포함합니다. `Response`는 마스킹 후 최종 응답 본문 또는 오류 메시지의 UTF-8 바이트 수로 선언·관계·헤더를 포함하고 JSON 포장은 제외합니다. 처리 시간은 서버의 요청 처리 시간이며 SQLite 기록과 클라이언트/네트워크 시간은 제외합니다. 클라이언트의 추가 잘림과 모델의 실제 소비량은 관측하지 않으므로 토큰 수나 물리적 디스크 읽기 횟수가 아닙니다.

호출과 파일별 관측은 `.codemap/analysis.sqlite3`에 저장합니다. 질의·소스·응답 내용은 저장하지 않습니다. **보존 기간은 30일 고정이며 연장할 수 없습니다.** MCP 시작·새 기록·분석 시, 그리고 MCP 실행 중 매분 만료 호출과 연결된 파일 기록을 함께 삭제합니다. MCP가 꺼져 있는 동안 만료된 기록은 다음 실행 또는 분석 시 삭제됩니다. `secure_delete`와 삭제형 rollback journal을 사용해 삭제된 행을 DB 여유 페이지나 상시 WAL에 남기지 않습니다. 이 정책은 관리 대상 DB에 적용되며 별도 백업까지 삭제하지는 않습니다.

`codemap-search mcp --no-call-log`는 새 기록만 끄며 기존 DB의 만료 정리는 계속합니다. 저장/정리 실패는 stderr에 알리고 MCP 응답은 유지하며 다음 작업에서 재시도합니다. 분석 명령은 DB에 접근하지 못하면 오류를 보고합니다. 기록 중단 기간은 복원할 수 없고, 직전 7일 비교도 연속 수집을 보장하지 않습니다. 이전 JSONL 기록은 가져오지 않으며 `--log` 옵션은 사용하지 않습니다.

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
