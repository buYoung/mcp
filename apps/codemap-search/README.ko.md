# codemap-search

한국어 | [English](./README.md)

`codemap-search`는 코딩 에이전트를 위한 독립 실행형 MCP 표준 입출력 서버이자 CLI입니다. 하나의 Rust 바이너리 안에 ripgrep 라이브러리 크레이트, tree-sitter 문법, Tantivy 검색 엔진이 들어 있습니다. 시스템에 `rg`, 언어 서버, 외부 런타임 바이너리를 따로 설치하지 않아도 저장소 구조를 훑고, 심볼과 문서 문자열을 검색하고, 파일 내용을 정확히 확인할 수 있습니다.

기본 흐름은 좁혀 들어가기입니다.

1. `overview`로 저장소 루트, 폴더, 파일 단위 구조를 봅니다.
2. `search`로 심볼, 정의, 개념, 오류 메시지, 설정 기본값을 찾습니다.
3. `read`, `find`, `grep`으로 실제 파일 내용과 경로를 확인합니다.

벤치마크 비교는 [benchmark](../../benchmark/README.md)를 참고하세요. 모든 저장소에서 항상 이기는 단일 백엔드는 없고, 공개된 측정에서 `codemap-search`가 가장 뚜렷하게 앞선 부분은 인덱스 생성 속도와 디스크 사용량입니다. 자세한 수치는 [인덱싱](#인덱싱)에 정리했습니다.

## 빠른 시작

crates.io 릴리스 버전을 설치합니다.

```sh
cargo install codemap-search
codemap-search --version
```

또는 이 저장소의 로컬 체크아웃에서 설치합니다(로컬 HEAD/작업 트리를 빌드).

```sh
cargo install --path apps/codemap-search
```

그다음 MCP 클라이언트에 등록합니다. 서버는 실행된 작업 디렉터리를 인덱싱하므로, 클라이언트가 분석하려는 저장소에서 `codemap-search mcp`를 실행해야 합니다.

Claude Code (전역 등록 — user 스코프, 모든 프로젝트에 적용):

```sh
claude mcp add -s user codemap-search -- codemap-search mcp
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.codemap-search]
command = "codemap-search"
args = ["mcp"]
```

등록 후에는 클라이언트에서 `initial_instructions`를 한 번 호출하세요. 일부 MCP 클라이언트는 서버 수준 안내문을 표시하지 않기 때문에, 이 도구가 권장 탐색 흐름을 별도로 전달합니다.

## MCP 제공 범위

`codemap-search`는 MCP 도구만 노출합니다. MCP 리소스와 프롬프트는 등록하지 않습니다. 모든 도구는 설정된 파일시스템 범위 안에서 읽기 전용으로 동작합니다.

## 지원 언어

tree-sitter 기반 심볼 추출은 다음 확장자를 지원합니다.

| 언어 | 확장자 |
|---|---|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript / TSX | `.ts`, `.tsx` |
| JavaScript / JSX | `.js`, `.jsx` |
| Go | `.go` |
| Java | `.java` |
| Kotlin | `.kt`, `.kts` |
| C | `.c` |
| C++ | `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` |
| Assembly / GAS | `.s`, `.S`, `.asm` |

`read`, `find`, `grep`은 텍스트 파일이면 언어와 관계없이 사용할 수 있습니다.

언어별 플래그 규칙도 반영합니다. Go는 대문자로 시작하는 심볼을 내보낸 심볼로 보고, `*_test.go`와 `Test`/`Benchmark`/`Example`/`Fuzz`를 테스트로 봅니다. Java는 `public`, `@Test`, `@Deprecated`, javadoc `@deprecated`를 읽습니다. Kotlin은 `private`/`internal`/`protected`가 아니면 내보낸 심볼로 보고, `@Test`와 `@Deprecated`를 읽습니다. C/C++는 `static` 저장 클래스를 파일 내부 심볼로 처리하고, C++ 접근 지정자를 반영합니다. Assembly는 `.globl`/`.global` 지시문에 나온 심볼을 내보낸 심볼로 봅니다.

## 설치

Rust가 이미 있으면 `cargo install`이 가장 단순합니다. 로컬 컴파일을 피하고 싶으면 GitHub Release 사전 빌드 바이너리, `install.sh`, WinGet, Homebrew 경로를 사용할 수 있습니다. OS별 권장 경로와 배포 채널별 메인테이너 런북은 [docs/distribution](./docs/distribution/index.md)에 있습니다.

### crates.io

```sh
cargo install codemap-search
```

바이너리는 `~/.cargo/bin`에 설치됩니다. 이 디렉터리가 `PATH`에 있어야 합니다.

### WinGet

```powershell
winget install com.livteam.codemap-search
```

`microsoft/winget-pkgs`에 패키지가 반영된 뒤 사용할 수 있습니다. 병합 전에는 저장소 안의 매니페스트로 설치할 수 있지만, 릴리스 자산이 있고 매니페스트의 placeholder `sha256` 값이 실제 값으로 교체된 뒤에만 해시 검증을 통과합니다. 상대 경로를 쓰므로 저장소 루트에서 실행해야 합니다.

```powershell
winget install --manifest apps/codemap-search/packaging/winget
```

Windows arm64 바이너리는 x64 러너에서 크로스 빌드된 빌드 전용 산출물이며, arm64 실기기 실행 검증은 아직 아닙니다.

### Homebrew

```sh
brew install codemap-search
```

`homebrew-core`에 수용된 뒤 사용할 수 있습니다. 그 전까지 macOS에서는 `cargo install codemap-search`, GitHub Release 직접 다운로드, 또는 `install.sh`를 사용하세요. 포뮬러는 `apps/codemap-search/packaging/homebrew/codemap-search.rb`에 있습니다.

### 소스에서 설치

```sh
cargo install --path apps/codemap-search
# or, from a checkout of this repo:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# binary at target/release/codemap-search
```

### 사전 빌드 바이너리와 `install.sh`

GitHub Release에는 macOS arm64/x64, Linux x64 `musl`/`gnu`, Linux arm64 `musl`, Windows x64, Windows arm64 빌드 전용 자산이 올라갑니다. 플랫폼에 맞는 아카이브를 내려받아 압축을 풀고 `codemap-search`를 `PATH`에 있는 디렉터리에 두면 됩니다.

macOS와 Linux에서는 설치 스크립트를 사용할 수 있습니다. 스크립트는 OS와 아키텍처를 감지하고, 맞는 릴리스 아카이브를 받은 뒤, 압축을 풀기 전에 `.sha256`을 검증하고, 기본적으로 `~/.local/bin`에 설치합니다.

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

설치 디렉터리 변경:

```sh
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

버전 고정:

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh -s -- --version codemap-v0.1.6
```

Linux는 기본적으로 정적 `musl` 빌드를 받습니다. x86_64에서 glibc 빌드가 필요하면 `CODEMAP_LINUX_LIBC=gnu`를 지정하세요. 설치 디렉터리가 `PATH`에 없으면 스크립트가 현재 세션용 `export PATH=...` 안내를 출력합니다.

### 지원 플랫폼

| 플랫폼 | 변형 | 지원 수준 | 비고 |
|---|---|---|---|
| Linux x86_64, Ubuntu 22.04~26.04 | `musl` | Docker 검증 | 정적 빌드, glibc 불필요 |
| Linux x86_64, Ubuntu 22.04+ | `gnu` | Docker 검증 | glibc 2.34+ 필요 |
| Linux arm64 | `musl` | 크로스 빌드, arm64 실행 미검증 | arm64 Linux용 단일 자산 |
| macOS Sequoia 15 이상 | arm64, x86_64 | 기준 명시 | Apple Silicon과 Intel |
| Windows 11 이상 | x86_64 | 실기기 기준, 최선 지원 | Windows x64 |
| Windows 11 arm64 | arm64 | 빌드 전용 | arm64 실행 미검증 |

Linux에서는 특별한 이유가 없으면 `musl` 바이너리를 권장합니다. glibc 빌드는 Ubuntu 20.04 이하처럼 glibc 2.34 미만인 배포판에서 실행되지 않습니다.

## MCP 클라이언트 등록

`mcp` 하위 명령으로 서버를 실행합니다. 서버는 현재 작업 디렉터리를 기준으로 동작합니다. 사용자 전역 등록을 해도 클라이언트가 활성 프로젝트를 작업 디렉터리로 잡아 `codemap-search mcp`를 실행하면 저장소마다 같은 설치를 재사용할 수 있습니다.

### Claude Code

프로젝트 범위:

```sh
claude mcp add codemap-search -- codemap-search mcp
```

사용자 전역 범위:

```sh
claude mcp add -s user codemap-search -- codemap-search mcp
```

또는 프로젝트 범위는 `.mcp.json`, 사용자 범위는 `~/.claude.json`에 직접 추가합니다.

```json
{
  "mcpServers": {
    "codemap-search": { "command": "codemap-search", "args": ["mcp"] }
  }
}
```

### Codex

`~/.codex/config.toml`은 Codex의 전역 설정입니다.

```toml
[mcp_servers.codemap-search]
command = "codemap-search"
args = ["mcp"]
```

CLI로도 같은 설정을 추가할 수 있습니다.

```sh
codex mcp add codemap-search -- codemap-search mcp
```

### opencode

전역 설정은 `~/.config/opencode/opencode.json`에 있습니다. 저장소 하나에만 적용하려면 저장소 루트의 `opencode.json`을 사용하세요.

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

## MCP 도구

| 도구 | 용도 | 주요 인자 |
|---|---|---|
| `initial_instructions` | 권장 탐색 흐름을 반환합니다. 클라이언트가 서버 안내문을 표시하지 않을 때 한 번 호출합니다. | 없음 |
| `overview` | 계층형 코드맵입니다. `path`가 없으면 저장소 루트, 폴더면 해당 폴더, 파일이면 파일 안의 심볼과 줄 범위를 보여줍니다. | `path`, `format` |
| `search` | 심볼, 문서 문자열, 경로 토큰을 BM25로 검색합니다. 좁은 결과는 파일 상세, 넓은 결과는 코드맵 개요로 렌더링합니다. | `query`, `caller_context` |
| `read` | 파일을 줄 번호와 함께 읽습니다. 큰 파일은 창 단위로 읽습니다. | `file_path`, `offset`, `limit` |
| `find` | glob으로 파일을 찾습니다. 결과는 수정 시간순으로 정렬되고 상한이 있습니다. | `pattern`, `path`, `include_ignored` |
| `grep` | 디스크의 실제 파일을 정규식이나 리터럴로 검색합니다. 주석, 비코드 파일, 방금 수정한 파일 확인에 적합합니다. | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |

`read`는 `file_path` 대신 `path`/`file`, `offset`/`limit` 대신 `start_line`/`end_line` 별칭도 받습니다. `find`와 `grep`은 기본적으로 `.gitignore`, `.git/info/exclude`, `.codemapignore`를 따릅니다. 한 호출에서 모든 ignore 규칙을 우회하려면 `include_ignored: true`를 전달하세요. `.git/info/exclude`만 끄려면 `use_git_exclude` 설정을 사용합니다.

## CLI

`codemap-search`는 CLI로도 사용할 수 있습니다.

```text
codemap-search mcp
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

## 설정

설정은 선택 사항입니다. 설정 파일이 없으면 내장 기본값으로 동작합니다. TOML 설정은 저장소 레이어(`<repo>/.codemap/config.toml`)와 전역 레이어(`$CODEMAP_HOME/config.toml`, 없으면 `~/.codemap/config.toml`)에서 읽고, 키별로 `repo > global > default` 우선순위로 병합합니다.

`mcp` 시작 시 저장소 설정 파일이 없으면 동작을 바꾸지 않는 주석 템플릿을 자동 생성합니다. 파일이 이미 있으면 새 릴리스에서 추가된 키만 주석 블록으로 덧붙입니다. 기존 줄은 수정하거나 삭제하지 않습니다.

모든 키, 기본값, `.codemap/` 디렉터리 구조는 [docs/configuration.md](./docs/configuration.md)에 있습니다. `read`, `find`, `grep`을 작업공간 안으로 제한할지, 허용된 외부 루트를 열지, 전체 디스크 접근을 허용할지는 `[filesystem_permissions]`에서 제어합니다.

외부 계정, API 키, 유료 서비스는 필요하지 않습니다.

런타임 환경변수:

| 변수 | 필수 | 설명 |
|---|---|---|
| `RUST_LOG` | 아니요 | stderr 진단 로그 수준을 조정합니다. 예: `RUST_LOG=debug codemap-search mcp` |
| `CODEMAP_HOME` | 아니요 | 전역 설정 디렉터리를 바꿉니다. 기본값은 `~/.codemap`입니다. |

설치 스크립트 전용 환경변수:

| 변수 | 필수 | 설명 |
|---|---|---|
| `INSTALL_DIR` | 아니요 | `install.sh` 설치 위치를 바꿉니다. 기본값은 `~/.local/bin`입니다. |
| `CODEMAP_VERSION` | 아니요 | `install.sh`가 받을 릴리스 태그를 고정합니다. 예: `codemap-v0.1.6` |
| `CODEMAP_LINUX_LIBC` | 아니요 | Linux 자산 종류를 고릅니다. 기본은 `musl`, x86_64에서만 `gnu`를 선택할 수 있습니다. |

## 로깅

진단 로그는 stderr로만 출력합니다. stdout은 MCP JSON-RPC 스트림으로 예약되어 있습니다. 기본 로그 필터는 `warn,codemap_search=info`라서 Tantivy commit/GC 같은 의존성 `INFO` 로그는 숨깁니다. 더 자세한 로그가 필요하면 `RUST_LOG`를 올립니다.

```sh
RUST_LOG=debug codemap-search mcp
```

## 인덱싱

MCP 서버는 시작 시 저장소를 직접 인덱싱합니다. 별도 인덱스 단계, 언어 서버, 외부 서비스가 필요 없습니다. 인덱스는 저장소 안의 `.codemap/` 디렉터리에 저장되고 다음 실행에서 재사용됩니다.

파일시스템 감시자는 Linux inotify, macOS FSEvents, Windows ReadDirectoryChanges를 사용합니다. 일반 편집은 기본 500ms 디바운스 뒤 경로 단위 증분 업데이트로 처리합니다. git `HEAD` 변경이나 큰 변경 묶음은 전체 워크로 승격합니다. 감시자가 정상일 때 `search`와 `overview`는 요청마다 트리를 다시 걷지 않습니다. `read`, `find`, `grep`은 항상 디스크를 직접 읽으므로 방금 수정한 파일도 바로 보입니다.

측정 기준은 Docker, 네이티브 arm64, 빈 `.codemap`, 정확한 SHA 체크아웃입니다.

| 저장소 | 파일 수 | 콜드 전체 인덱스 | 증분 재인덱스, 1~10개 파일 | 디스크 인덱스 |
|---|---:|---:|---:|---:|
| angular | 약 10.6k | 약 4.6초 | 약 0.15초 | 16 MB |
| deno | 약 13.5k | 약 3.6초 | 약 0.13초 | 9.8 MB |

[benchmark](../../benchmark/README.md)의 같은 arm64 측정에서는 언어 서버와 그래프 백엔드의 콜드 인덱스가 약 41~62초, codegraph의 angular 콜드 인덱스가 약 80초였고, 디스크 인덱스는 약 150~450 MB 범위였습니다. 이 측정에서는 `codemap-search`가 인덱스를 대략 한 자릿수 배 빠르게 만들고 훨씬 작게 저장했습니다. 단, 저장소와 작업 흐름에 따라 가장 좋은 백엔드는 달라질 수 있습니다.

## 알려진 제한

- 심볼 추출은 컴파일된 tree-sitter 문법이 있는 언어로 제한됩니다. 다른 확장자는 `read`, `find`, `grep`으로 검색할 수 있지만 심볼 인덱스에는 들어가지 않습니다.
- `max_file_size` 기본값은 1 MiB입니다. 이보다 큰 파일은 인덱싱과 코드맵에서 건너뜁니다.
- 문자열 리터럴은 상세 보기 레이어에 표시되지만 BM25 인덱스에는 넣지 않습니다. 정확한 문자열 검색은 `grep`을 사용하세요.
- 서버는 단일 클라이언트용 순차 stdio 서버입니다. 여러 프로세스가 같은 인덱스를 잠그고 공유하는 모델은 아닙니다.

## 라이선스

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
