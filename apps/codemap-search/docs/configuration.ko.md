# 설정

한국어 | [English](./configuration.md)

codemap-search는 설정 파일 없이도 기본값으로 동작합니다. 변경할 키만 설정하면 나머지는 전역 설정이나 내장 기본값을 사용합니다.

## 매크로 확장

설치된 Clang으로 C/C++ 및 CPP를 사용하는 ASM의 매크로를 확장합니다. NASM `.asm`은 확장 목록에서 생성된 label을 원본 호출 줄에 연결합니다. [clangd의 컴파일 설정 처리](https://clangd.llvm.org/design/compile-commands)를 참고했으며, clangd 실행이나 `.clangd` 설정 읽기를 추가한 기능은 아닙니다.

```toml
[macro_expansion]
is_enabled = true
compilation_database = "build/compile_commands.json"
clang_path = "clang"
nasm_path = "nasm"
clang_flags = ["-Iinclude", "-DFEATURE=1"]
nasm_flags = ["-Iinclude/", "-felf64"]
timeout_ms = 5000
max_output_bytes = 8388608
```

| `[macro_expansion]`의 키 | 자료형 / 기본값 | 적용 |
| --- | --- | --- |
| `is_enabled` | bool / `false` | C/C++/ASM 색인과 직접 `parse`·`codemap`에 전처리 적용 |
| `compilation_database` | 비어 있지 않은 문자열 / 생략 | 저장소 기준 상대 경로 또는 절대 경로. JSON 파일·디렉터리 또는 `compile_flags.txt` 파일 |
| `clang_path`, `nasm_path` | 비어 있지 않은 문자열 / `"clang"`, `"nasm"` | 설치된 실행 파일의 이름 또는 경로 |
| `clang_flags`, `nasm_flags` | 문자열 배열 / `[]` | 빌드 설정 뒤에 추가하는 인자. 저장소 배열은 전역 배열을 대체 |
| `timeout_ms` | 양의 정수 / `5000` | 외부 프로세스 한 번의 밀리초 제한. NASM은 전처리·확장 목록 생성 두 번 실행 |
| `max_output_bytes` | 양의 정수 / `8388608` | 전처리 결과 또는 NASM 확장 목록의 바이트 제한 |

경로를 생략하면 소스의 부모 디렉터리에서 저장소 루트까지 `compile_commands.json`·`compile_flags.txt`를 찾습니다. JSON에서는 정규화한 파일 경로가 정확히 일치하는 첫 항목을 사용합니다. 이름이 비슷한 파일의 빌드 옵션으로 헤더 설정을 추정하지 않습니다. 데이터베이스가 없으면 사용자 인자와 설치된 도구의 기본값을 사용합니다. 데이터베이스에 대상 파일이 없으면 항목을 추가하거나 `compilation_database`에 `compile_flags.txt` 파일을 명시해야 합니다.

[컴파일 데이터베이스](https://clang.llvm.org/docs/JSONCompilationDatabase.html)의 작업 디렉터리, 헤더 경로, 매크로 정의, 대상 플랫폼과 언어 표준을 반영합니다. `arguments` 배열을 `command` 문자열보다 우선하고, 문자열은 셸 평가 없이 인자로 분리합니다. 설정한 Clang/NASM만 실행하며 데이터베이스의 래퍼나 프로젝트 빌드 명령을 실행하지 않습니다. 기록된 컴파일러 이름의 `++`와 명시적인 `-x`는 C/C++ 파서 선택에도 반영합니다. 출력·의존성 파일 생성 인자는 제거합니다. 지원하지 않는 옵션·언어, 응답 파일, 컴파일러 플러그인은 미해결 사유로 표시합니다. 별도 툴체인의 시스템 헤더·대상 플랫폼은 사용자가 공급해야 하며, query-driver 자동 탐색과 모든 컴파일러 옵션 호환성은 구현하지 않았습니다.

확장에 성공하면 활성 선언으로 목록을 갱신하고 원문에서 추출한 매크로 정의도 보존합니다. 생성된 선언에는 `macro expansion`과 원본 파일·줄 범위를 표시하며, 헤더 선언을 포함한 소스의 선언으로 오연결하지 않습니다. 확장 토큰의 열 위치와 호출·상수 참조 연결은 **미해결**입니다. 이 파일에서 추정한 정의 링크나 `(precise)`를 출력하지 않습니다. `read`·`grep`의 원문은 그대로 유지합니다.

도구·헤더 누락, 시간·출력 제한, 지원하지 않는 확장 결과, `#line`·`%line` 재지정은 `Macro expansion unresolved`에 이유를 표시하고 원문 선언을 유지합니다. NASM은 `-Le -Lm -Lf` 지원 버전이 필요하며 2.16.03으로 확인했습니다. 확장 목록을 얻는 조립 단계의 출력은 null 장치로 보내며 생성된 프로그램을 실행하지 않습니다. 조립기가 검증한 label·global만 선언 입력으로 변환하며, 일반 ASM 파서가 명령어의 피연산자를 다시 해석하지 않습니다. 자동 생성된 `..@` 매크로 지역 label은 제외합니다. 고정된 FFmpeg `x86inc.asm`의 매크로도 명시한 아키텍처·형식 인자로 확인했으며, FFmpeg 전체 빌드를 검증했다는 뜻은 아닙니다. GNU assembler 고유 `.macro` 확장, 다른 ASM 방언과 모든 저장소의 빌드 설정을 검증한 것은 아닙니다.

설정 변경은 전체 색인 갱신을 요청합니다. 활성화된 동안 작업 공간 파일 변경도 C/C++/ASM을 다시 대조하며, 기록한 외부 헤더·빌드 설정은 도구 요청 시 최대 1초 간격으로 변경 여부를 확인합니다. 전처리에 실패한 뒤 외부 의존성을 새로 준비했다면 갱신 또는 재시작이 필요할 수 있습니다. 제한은 파일별 프로세스 기준이며 저장소 전체 제한이 아닙니다. macOS arm64에서 검증했고 Windows/Linux 실행은 미확인입니다.

## 설정 위치와 우선순위

키별 우선순위는 저장소 → 전역 → 내장 기본값입니다. 저장소 파일에 `[search].result_threshold`만 있으면 다른 키는 전역 설정이나 기본값을 이어받습니다.

| 범위 | 경로 |
|---|---|
| 저장소 | `<repo>/.codemap/config.toml` |
| 전역 | `$CODEMAP_HOME/config.toml`, 미지정 시 `~/.codemap/config.toml` |

## 설정 읽기와 자동 작성

현재 설정 버전은 **12**이며 주석으로 표시합니다.

```toml
# codemap-config-version: 12
```

- 설정 파일은 없어도 됩니다. TOML 구문이 잘못되면 해당 파일의 설정 전체를 사용하지 않습니다. 알 수 없는 키·잘못된 자료형·허용되지 않는 값은 stderr에 경고하고 해당 키만 낮은 우선순위 설정으로 대체합니다. 저장소 값이 잘못되어도 유효한 전역값이 있으면 기본값보다 우선합니다.
- `[update].config_auto_update = true`이면 MCP 시작 시 누락된 저장소 설정을 만듭니다. 제외 배열에는 공통 폴더와 감지한 프로젝트 종류의 재귀 glob을 넣고, 다른 활성 키에는 내장 기본값을 씁니다. 활성 저장소 키는 전역값보다 우선합니다.
- 버전 표시가 없거나 6 이전인 저장소 설정은 **제외 목록을 한 번 전환**합니다. 사용자 규칙, 이전에 적용되던 제외값, 공통 폴더, 추천 재귀 glob을 배열에 명시합니다. 기존 항목과 주석은 보존하고 누락된 값만 중복 없이 추가한 뒤 현재 스키마 버전으로 바꿉니다.
- **버전 6부터 `excluded_directories`는 자동으로 만들거나 보충하지 않습니다.** 항목 삭제, `[]` 지정, 키 주석 처리, 새 프로젝트 추가 후에도 목록을 복원하지 않습니다. 수동 변경을 읽어 적용하는 동작은 계속됩니다.
- 버전 8은 최상위 또는 `[caller_context]`의 활성 테스트 설정을 `[exclude]`로 옮깁니다. 적용값과 사용자 주석을 보존하며 자동 작성 여부는 `config_auto_update`를 따릅니다. 전역 파일을 포함해 기존 위치도 계속 읽습니다. 잘못되거나 충돌하는 값을 동작 변경 없이 옮길 수 없으면 파일을 유지하고 경고합니다.
- 버전 9는 `[index]` 또는 최상위의 `excluded_directories`, `use_git_exclude`도 `[exclude]`로 옮깁니다. 기존 배열, 명시한 `[]`, 불리언 값과 주석을 보존하며 이 위치 전환으로 디렉터리 규칙을 추가하지 않습니다. 같은 파일에서는 유효한 `[exclude]` 값이 우선합니다.
- 버전 12는 `[event_navigation].is_enabled` 주석을 추가했습니다. 해당 버전에서는 이벤트 색인이 기본으로 꺼져 있었습니다.
- 버전 13부터 이벤트 색인과 관련 탐색 결과를 기본으로 제공합니다. 기존 `is_enabled=false` 값은 유지하며, 요청별로 `include_events=false`를 지정하면 이벤트 문맥을 생략합니다.
- 버전 11은 `[analysis].target_os` 주석을 추가합니다. 생략하거나 빈 값이면 분석 대상을 추정하지 않습니다.
- 버전 10은 `[macro_expansion]` 주석 섹션을 추가합니다. 사용자가 켜기 전에는 외부 전처리기를 실행하지 않습니다. 전환 시 TOML 문자열 안의 섹션 이름·버전 주석을 실제 설정 구조로 오인하지 않습니다.
- 일반 설정 버전 갱신은 새 키를 주석으로 추가하며 자동으로 활성화하지 않습니다. 이미 최신인 파일은 다시 쓰지 않습니다.
- `config_auto_update = false`는 최초 생성과 전환을 모두 끕니다. 설정 읽기와 감시는 계속되며, 전역 파일은 항상 자동 생성·전환 대상에서 제외됩니다.
- 운영체제 언어가 한국어이면 한국어 주석을, 그 밖에는 영어 주석을 생성합니다. 프로젝트 감지 전 두 템플릿의 키와 값은 같습니다.

전환 중 파일이 바뀌거나 TOML 구문·쓰기 권한·지원하지 않는 테이블 구조 때문에 전환할 수 없으면 파일을 그대로 두고 경고합니다. 원인을 수정한 뒤 재시작하세요. 점으로 연결하거나 인라인으로 작성한 index 테이블에 제외 배열이 없다면 배열을 명시하고 다시 시도하세요. 설정 파일의 심볼릭 링크는 유지합니다.

### 자동 작성을 껐을 때 수동 전환

버전 6에서는 명시한 배열이 선택적 기본 목록을 대체합니다. 이전 배열은 내장 목록에 추가하는 방식이었습니다. 자동 작성을 껐다면 기존 제외를 유지하기 위해 `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor` 중 필요한 항목을 직접 넣고, 공통·프로젝트 규칙을 추가한 뒤 버전 주석을 6으로 바꾸세요. 다른 키는 전환할 필요가 없습니다. 직접 편집하기 전에 설정을 백업하세요.

전환하면서 상속받던 전역 제외값을 저장소 배열에 기록할 수 있습니다. 이후 전역 변경을 다시 상속하려면 저장소 키를 주석 처리하세요. 전역 배열도 버전 6에서는 전체 목록이며 자동으로 수정하지 않습니다.

## 디렉터리 제외 규칙

`[exclude].excluded_directories`는 **선택적으로 제외할 디렉터리 규칙의 전체 목록**입니다. 명시한 배열에 숨겨진 기본 목록을 더하지 않습니다. `[]`는 선택적 규칙을 해제하고, 키 생략은 전역 목록 또는 기본값을 상속합니다. 무시 파일과 필수 제외 규칙은 별도로 적용합니다.

공통 초기 목록은 다음과 같습니다.

```toml
[exclude]
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

MCP는 `[refresh].watch`와 별개로 시작 시 존재하는 저장소·전역 설정 디렉터리를 감시합니다. 변경을 약 **1000ms** 동안 모은 뒤 설정을 다시 읽습니다. 디렉터리가 없었거나 감시를 시작하지 못했다면 설정 생성·편집 후 서버를 재시작하세요. CLI는 명령 실행 시 설정을 읽습니다.

| 설정 | 적용 시점 |
|---|---|
| `excluded_directories`, 모든 `[language_support]` 키 | 다시 읽은 뒤 전체 색인 갱신을 요청하고, 완료되면 결과에 반영 |
| 검색 출력, 호출 관계 표시, 도구 출력 제한, 파일시스템 권한 | 설정을 다시 읽은 뒤 다음 도구 요청 |
| `index_staleness_ms`, `indexer_auto_restart` | 이후 갱신·복구 판단 |
| `max_file_size`, `use_git_exclude` | 이후 탐색·갱신부터 적용하며, 이 값만 바꾸면 전체 갱신을 요청하지 않음 |
| `navigation_store_references` | 이후 파싱부터 적용하며, 재시작해도 변경되지 않은 파일은 기존 색인을 재사용할 수 있음 |
| `index_path`, `watch`, `watch_debounce_ms` | 재시작 필요 |
| `config_auto_update` | 다음 MCP 시작 시 자동 작성 |

제외 규칙을 직접 바꾸면 파일 필터를 갱신하고 전체 색인 갱신을 요청합니다. 갱신 완료 후 제외된 파일은 결과에서 사라지고 새로 포함한 파일은 검색할 수 있습니다. 색인 기능을 사용할 수 없으면 복구하거나 서버를 재시작한 뒤 결과를 확인하세요.

## 설정 키

숫자 키는 양의 정수이며 `grep_max_columns`만 `0`도 허용합니다. 템플릿은 아래처럼 섹션별 키를 사용합니다. 호환성을 위해 `result_threshold = 5` 같은 기존 최상위 키도 허용하지만 같은 파일에 둘 다 있으면 섹션별 값이 우선합니다.

| 키 | 자료형 | 기본값 | 설명 |
|---|---|---|---|
| `[update].config_auto_update` | bool | `true` | 누락된 저장소 설정 생성과 시작 시 새 설정 주석 추가 |
| `[index].index_path` | 문자열 | `".codemap/index"` | 색인 저장 위치. 절대 경로나 작업공간 루트 기준 상대 경로 |
| `[index].max_file_size` | 정수(바이트) | `1048576` (1 MiB) | 파싱·색인 전 건너뛸 파일 크기 기준 |
| `[exclude].excluded_directories` | 문자열 배열(상대 디렉터리 glob) | 공통 + 기존 기본 이름. 생성 파일은 재귀 glob 사용 | 선택적 제외 전체 목록. [디렉터리 제외 규칙](#디렉터리-제외-규칙) 참고 |
| `[exclude].use_git_exclude` | bool | `true` | `.git/info/exclude` 적용 여부 |
| `[language_support].is_document_support_enabled` | bool | `false` | `.md`/`.mdx`를 색인 기반 탐색에 포함 |
| `[language_support].is_shell_support_enabled` | bool | `false` | `.sh`, `.bash`, `.zsh`를 색인 기반 탐색에 포함 |
| `[language_support].is_infrastructure_support_enabled` | bool | `false` | HCL/Terraform, Dockerfile, Nix 포함 |
| `[language_support].is_interface_support_enabled` | bool | `false` | Protocol Buffers, GraphQL 포함 |
| `[language_support].is_build_support_enabled` | bool | `false` | Make, CMake, Starlark/Bazel 포함 |
| `[refresh].watch` | bool | `true` | 파일 변경을 감시해 백그라운드 색인 갱신 |
| `[refresh].watch_debounce_ms` | 정수(ms) | `500` | 파일 변경을 모아서 처리하는 시간 |
| `[refresh].index_staleness_ms` | 정수(ms) | `5000` | 파일 감시를 쓸 수 없을 때 요청 기반 갱신 간격 |
| `[refresh].indexer_auto_restart` | bool | `true` | 백그라운드 색인이 중단되면 자동 복구 |
| `[search].result_threshold` | 정수 | `5` | 상세 내용을 표시할 상위 파일 수 |
| `[search].search_overview_file_limit` | 정수 | `12` | 나머지 간략 목록에 표시할 최대 파일 수 |
| `[search].search_detail_snippet_max_lines` | 정수 | `80` | 심볼마다 표시할 최대 발췌 줄 수. 긴 본문은 생략 표시 |
| `[search].search_detail_symbol_limit` | 정수 | `20` | 파일마다 표시할 최대 심볼 수. 초과분은 생략 안내 |
| `[search].search_detail_byte_cap` | 정수(바이트) | `32768` | 부분 출력 안내를 포함한 검색 응답 전체 크기 제한 |
| `[search].search_literal_max_len` | 정수(문자) | `200` | 일치한 리터럴의 최대 표시 길이. 초과분은 말줄임표로 표시 |
| `[search].search_literal_limit` | 정수 | `10` | 파일마다 표시할 최대 리터럴 수 |
| `[search].search_anchor_snippet_limit` | 정수 | `3` | 파일마다 전체 발췌를 표시할 최대 심볼 수. 나머지는 최대 3줄 선언으로 표시 |
| `[tool_output].grep_max_columns` | 정수 | `500` | `grep`의 `content` 모드 열 제한. 초과 시 `[Omitted long matching line]`, `0`이면 제한 해제 |
| `[tool_output].read_output_byte_cap` | 정수(바이트) | `102400` | `read` 출력 크기 제한. 초과 시 더 좁은 범위를 안내하는 오류 |
| `[filesystem_permissions].find` | 문자열 | `"workspace"` | `find` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].grep` | 문자열 | `"workspace"` | `grep` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].read` | 문자열 | `"workspace"` | `read` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].allowed_roots` | 문자열 배열 | `[]` | `allowed_roots` 정책을 쓰는 도구에서 접근할 외부 루트 |
| `[caller_context].caller_context_default` | bool | `true` | `search` 호출에서 `caller_context` 생략 시 호출 관계 표시 여부 |
| `[exclude].should_include_test_code` | bool | `false` | 자동 심볼·호출 관계에 테스트 코드 포함 |
| `[exclude].test_file_patterns` | 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 파일 glob. []는 경로 판별 해제 |
| `[exclude].test_attributes` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 속성·어노테이션 패턴. 언어별 목록을 상속값 대신 적용 |
| `[exclude].test_decorators` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 데코레이터 패턴. []는 해당 언어 목록 해제 |
| `[exclude].test_calls` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 호출 패턴. []는 해당 언어 목록 해제 |
| `[caller_context].navigation_context_default` | bool | `false` | 소스 구조로 호출 대상을 확인하면 `precise`로 표시 |
| `[caller_context].navigation_callsite_budget` | 정수 | `1000` | 이름 기반 추정으로 전환하기 전 검사할 최대 호출 위치 수 |
| `[caller_context].navigation_store_references` | bool | `false` | 함수 호출 외의 참조 위치 저장 |
| `[caller_context].scan_cap` | 정수 | `500` | 호출자 탐색에서 이름별로 나눠 쓸 검색 건수 제한. 이름당 최소 25건 |
| `[caller_context].caller_list_cap` | 정수 | `5` | 심볼마다 표시할 최대 호출자 또는 비호출 참조 수 |
| `[caller_context].callee_list_cap` | 정수 | `5` | 심볼마다 표시할 최대 호출 대상 수 |
| `[caller_context].annotation_sub_budget` | 정수(바이트) | `8192` | `search_detail_byte_cap` 안에서 호출 관계의 출력 크기 제한 |
| `[caller_context].common_name_threshold` | 정수 | `2` | 같은 이름의 정의가 이 수 이상이면 모호함 표시 |
| `[caller_context].caller_omit_def_threshold` | 정수 | `5` | 같은 이름의 정의가 이 수 이상이면 추정 호출자 목록을 생략하고 `grep` 안내. 호출 대상 목록에는 미적용 |

### 색인과 파일 제외

사용자 홈 자체는 MCP 작업공간이나 명시적 `index`/`benchmark` 대상이 될 수 없지만 홈 아래 프로젝트는 허용합니다. `HOME`과 `USERPROFILE`을 모두 확인할 수 없으면 경고하고 계속 실행합니다.

`.txt`, `*.lock`, 알려진 패키지 관리 잠금 파일, `*.map`, 압축·번들 파일은 대소문자 구분 없이 색인·코드맵·호출자 탐색에서 제외합니다. `find`/`grep`도 기본적으로 숨기지만 `include_ignored: true`로 볼 수 있고 직접 `read`/`parse`도 가능합니다. 전체 목록은 [지원 언어와 파일 제외](./language-support-checklist.ko.md)에 있습니다. `max_file_size`보다 큰 파일도 색인에서 제외합니다.

다섯 `[language_support]` 키는 색인, search, overview, codemap, 파일 변경 갱신에 적용합니다. 실시간 `find`/`grep`/`read`와 직접 `parse`를 끄지는 않습니다.

`use_git_exclude`는 `.git/info/exclude`만 제어합니다. `false`여도 `.gitignore`, 전역 Git 무시 규칙, `.codemapignore`는 적용됩니다.

### 출력과 호출 관계

`search`는 상위 파일의 상세 내용 뒤에 나머지 파일을 간략 목록으로 표시합니다. 크기 설정은 출력을 제한하며 제외 파일을 바꾸지 않습니다. 일반 질의에서는 생성 파일과 번역 리소스의 순위를 낮추지만 정확한 경로·심볼·리소스 키·인용 원문으로 해당 파일을 지정할 수 있습니다.

`search_detail_byte_cap`에는 부분 출력 안내도 포함합니다. 결과가 잘리면 질의를 좁히거나 안내된 범위를 읽으세요. `search`에는 페이지 위치 인자가 없습니다. `search_anchor_snippet_limit`는 파일마다 전체 발췌를 표시할 심볼 수를 제한하며 나머지는 최대 3줄 선언으로 표시합니다.

`read_output_byte_cap`에는 줄 번호·문맥·제목을 포함합니다. 초과하면 더 좁은 `offset`/`limit`를 안내하는 오류를 반환합니다. `limit`를 생략하면 별도의 전체 파일 256 KiB 제한도 적용합니다. `grep_max_columns = 0`은 긴 줄 제한을 끄고, 그 외에는 초과 줄을 `[Omitted long matching line]`으로 표시합니다. `grep`의 부분 결과에는 `next_offset`이 있습니다.

`caller_context_default`는 `search` 호출에서 `caller_context`를 생략했을 때만 적용하며 명시한 인자가 우선합니다. 호출 관계는 기본적으로 추정값입니다. `navigation_context_default = true`이면 소스 구조·import·지역 변수 연결로 단일 호출 대상을 확인한 경우 `precise`로 표시합니다. 확인할 수 없는 호출은 추정 결과를 사용합니다. `navigation_callsite_budget`은 이름 기반 탐색으로 전환하기 전 검사할 호출 위치 수를 제한합니다.

`navigation_store_references`는 함수 호출 외의 참조 위치를 저장하며 호출 대상 확인에 필수인 설정은 아닙니다. 일부 구조화 형식은 참조를 항상 저장합니다. 파싱 시 적용하므로 재시작만으로 변경되지 않은 파일을 다시 파싱하지 않을 수 있습니다.

대상이 정해진 `calls` 항목에는 추정 모드와 `precise` 모드 모두 `이름 — 파일:줄` 형식으로 정의 위치를 붙입니다. 대상이 모호하면 이름만 유지합니다. MCP `read`/`grep`은 표시한 함수의 직접 식별자 참조를 `references (same-file constants, approximate)`로 함께 보여줍니다. 소스 구문 트리와 색인의 상수 선언(JavaScript/TypeScript의 `const` 바인딩 포함)을 확인하므로 `navigation_store_references = false`여도 동작합니다. 코드를 실행하지 않고 정의 위치와 초기값 원문을 표시하며, 240자를 넘는 값은 줄여서 표시합니다. 주석·문자열, 경로를 붙이거나 import한 참조, 매크로 토큰, 중복 이름, 지역 바인딩이 있는 이름은 생략합니다. 테스트 제외 규칙과 문맥 출력 크기 제한도 적용하며, 크기 제한으로 생략한 항목은 안내합니다.

`scan_cap`은 검색할 이름들이 나누어 사용하며 이름당 최소 25건을 허용합니다. `caller_list_cap`과 `callee_list_cap`은 심볼별 관계 수를 제한합니다. `annotation_sub_budget`은 `search_detail_byte_cap` 안에서 호출 관계의 총 출력 크기를 바이트로 제한합니다. 원문 발췌가 우선하며 생략한 관계는 안내합니다.

같은 이름의 정의가 `common_name_threshold` 이상이면 추정 관계에 모호함을 표시합니다. `caller_omit_def_threshold` 이상이면 추정 호출자 목록 대신 안내와 `grep` 제안을 표시합니다. 호출 대상 목록이나 확인된 대상의 표시를 막지는 않습니다.

### 테스트 코드 문맥

`should_include_test_code`, `test_file_patterns`, `test_attributes`, `test_decorators`, `test_calls`는 `excluded_directories`, `use_git_exclude`와 함께 `[exclude]`에서 관리합니다. 같은 파일에서는 유효한 `[exclude]` 값이 이전 `[caller_context]`, `[index]`, 최상위 별칭보다 우선하며, 언어별 표는 누락한 언어를 상속합니다. 파일 간 저장소 → 전역 → 내장 우선순위는 동일합니다. 두 작업공간 제외 설정은 색인·코드맵·호출자 탐색·`find`/`grep`에 공통으로 적용하며, 어느 쪽을 바꿔도 설정 재로드 후 전체 색인 갱신을 요청합니다.

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
[exclude]
should_include_test_code = false
# Replace the complete path list with the patterns you want.
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[exclude.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[exclude.test_calls]
typescript = []
```

기본 목록입니다. 표에 없는 언어는 해당 종류의 내장 항목이 없습니다.

```toml
[exclude]
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[exclude.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]

[exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[exclude.test_calls]
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

`watch = true`이면 파일 변경을 백그라운드에서 반영합니다. `watch_debounce_ms` 동안 발생한 변경은 한 번에 갱신합니다. 감시를 끄거나 사용할 수 없으면 `search`/`overview`가 `index_staleness_ms` 간격에 따라 백그라운드 갱신을 요청하고 마지막 결과를 즉시 반환합니다.

`indexer_auto_restart = true`이면 백그라운드 색인 중단 후 다음 `search`/`overview`가 복구를 시도합니다. 서버 실행 중 복구 횟수에는 제한이 있습니다. 끄면 재시작 전까지 결과가 고정됩니다. 실시간 `read`/`find`/`grep`은 어느 경우에도 사용할 수 있습니다.

## `config.toml` 예시

주요 값을 명시한 예시입니다. 테스트 규칙 목록은 위의 별도 예시를 참고하세요. 실제 파일에는 변경할 키만 남겨도 됩니다.

```toml
# Every setting is optional; omitted settings use the defaults above.

[update]
config_auto_update = true

[index]
index_path = ".codemap/index"
max_file_size = 1048576   # 1 MiB

[language_support]
is_document_support_enabled = false
is_shell_support_enabled = false
is_infrastructure_support_enabled = false
is_interface_support_enabled = false
is_build_support_enabled = false

[refresh]
watch = true
watch_debounce_ms = 500
index_staleness_ms = 5000
indexer_auto_restart = true

[search]
result_threshold = 5
search_overview_file_limit = 12
search_detail_snippet_max_lines = 80
search_detail_symbol_limit = 20
search_detail_byte_cap = 32768            # 32 KiB
search_literal_max_len = 200
search_literal_limit = 10
search_anchor_snippet_limit = 3

[tool_output]
grep_max_columns = 500
read_output_byte_cap = 102400             # 100 KiB

[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []

[exclude]
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
# Initial generation also adds recursive globs for detected project types; edit them manually from v6 onward.
use_git_exclude = true
should_include_test_code = false

[caller_context]
caller_context_default = true
navigation_context_default = false
navigation_callsite_budget = 1000
navigation_store_references = false
scan_cap = 500
caller_list_cap = 5
callee_list_cap = 5
annotation_sub_budget = 8192
common_name_threshold = 2
caller_omit_def_threshold = 5
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

다음은 TOML 설정이 아닌 요청 인자입니다. 생략하면 기존 출력을 유지하며, `search.caller_context`에는 영향을 주지 않습니다.

| 인자 | 기본값 | 동작 |
| --- | --- | --- |
| `view` | `"full"` | `full`: 심볼과 원문. `source`: 원문만 반환하며 색인 문맥·관계 준비를 생략. `definitions`: 선언만. `relations`: 대상·소유자와 호출·상수 참조만 표시하고 원문은 생략. |
| `unresolved` | `"list"` | `count`는 같은 미해결 총수만, `list`는 총수와 제한된 이름 목록을 표시. full/relations에 적용. |
| `expand` | `"none"` | `callable`은 읽은 UTF-8 원문에서 이름 있는 함수·메서드의 경계를 확인. 오래된 색인 좌표나 추정 본문을 사용하지 않음. |

`read` 확장은 유효한 offset/start 별칭을 기준으로 하며 limit/end보다 우선합니다. 지원하는 함수가 없으면 기존 요청 범위와 불가 사유를 반환합니다. 파싱 입력은 `min(max_file_size, 4 MiB)`로 제한하며 너무 큰 파일은 파싱 전에 거절합니다. 붙어 있는 속성·데코레이터를 포함하고 중첩된 이름 있는 함수는 가장 안쪽을 선택합니다. 익명 클로저는 감싸는 이름 있는 함수가 대상입니다. 복합 파일, 파싱 오류, 본문 없는 선언, 다른 코드와 경계 줄을 공유하는 함수는 불가 사유를 표시합니다.

내용 모드 `grep`의 확장은 `-A/-B/-C`를 무시하고 검색식·경로·glob·대소문자·유형·제외 규칙을 유지합니다. 한 파일의 같은 바이트로 검색·파싱·출력을 수행합니다. 경로·줄 순서로 중복 함수를 제거하며, offset/head_limit/next_offset의 단위는 원문 줄이 아닌 함수 묶음 또는 확장 불가 일치 줄입니다. `head_limit=0`도 바이트 한도는 유지합니다. count/files_with_matches에 기본값과 다른 출력 제어를 넣으면 오류로 안내합니다.

read와 확장된 grep은 `read_output_byte_cap`, 관계 문맥은 기존 하위 예산을 지킵니다. 큰 함수를 조용히 자르지 않으며, 표시된 범위에 `expand=none`과 작은 offset/limit을 사용하도록 안내합니다. grep 열 제한으로 생략한 본문도 불완전하다고 표시합니다. 매크로·인코딩·테스트 제외 안내는 해당 문맥 모드에 유지하고, source에는 원문과 확장 처리 안내만 표시합니다. 생략된 옵션으로 이벤트 관계가 추가되지는 않습니다.

## Rust 분석 대상 지정

```toml
[analysis]
target_os = "macos"
```

`target_os`는 선택적인 OS 식별자이며 실행 컴퓨터의 OS를 사용하지 않습니다. 저장소 값이 전역 값보다 우선하고, 빈 문자열은 상속된 대상을 해제합니다. 잘못된 자료형·식별자는 경고 후 하위 설정을 사용합니다. 설정을 다시 읽으면 다음 요청이 새 원문·조건 해석기를 사용하므로 대상 변경만으로 색인을 다시 만들 필요가 없습니다.

Rust의 `target_os="값"`, `all(...)`, `any(...)`, `not(...)`, true/false를 참·거짓·미확정으로 평가합니다. 따라서 `not(target_os="macos")`는 명시한 모든 비-macOS 대상에 적용하며 Windows로 좁히지 않습니다. 대상 미지정, 다른 키·플래그, `cfg_attr`, 지원하지 않는 문자열·토큰은 미확정입니다. 단독 `cfg(test)`는 기존 테스트 문맥 필터가 관리하며 테스트 포함을 실제 Cargo 테스트 빌드로 표현하지 않습니다. 구문 의미는 [Rust 조건부 컴파일 문서](https://doc.rust-lang.org/reference/conditional-compilation.html)를 따릅니다.

import·재수출·선언·호출 위치와 확인 가능한 부모 모듈의 조건을 대조한 뒤 정의를 연결합니다. 제한된 경로 모델에서 모듈 소속을 확인할 수 없으면 미해결로 남깁니다. 원문에서 확인한 호출자는 같은 이름의 정의 개수와 관계없이 유지하고, 모호한 이름 매칭에만 생략 기준을 적용합니다. 호출 위치·별칭 예산이 끝나도 이미 확인한 연결은 유지하며 불완전하다고 표시합니다. 별칭은 최대 16단계·256개 이름의 후보 수집에만 사용하며, 각 링크는 별도로 정의 신원을 확인합니다. 명시한 분석 대상은 관계 출력에 표시하지만 실제 빌드·실행을 보장하지 않습니다.

한 응답 안에서는 같은 설정을 사용합니다. 처리 도중 설정이 갱신되면 후속 요청부터 적용합니다.


## 이벤트 관계 탐색

이벤트 탐색은 기본으로 켜져 있으며 별도 요청 옵션이 필요하지 않습니다. 필요한 소스를 심볼과 함께 저장하고, 같은 색인 세대의 이벤트 지도를 만든 뒤 한 번에 공개합니다. 애플리케이션 핸들러나 빌드 스크립트를 실행하지 않습니다. 아래 설정은 기본값을 보여주는 예시이며, 기능을 켜기 위해 작성할 필요는 없습니다.

```toml
[event_navigation]
is_enabled = true
use_builtin_rules = true
rules = []
```

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
[event_navigation]
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

UTF-8 소스만 저장하며 색인·Git·디렉터리 제외 규칙을 적용합니다. 현재 테스트 코드 포함 규칙은 질의 시 양쪽 이벤트 위치와 근거·핸들러 위치에 적용합니다. 소스, import한 버스·키·핸들러, 규칙, 분석 대상, 제외 설정이 바뀌면 새 세대를 만듭니다. 근거 파일이 바뀐 이전 연결은 갱신 전에도 숨깁니다. 원본 애플리케이션 파일은 변경하지 않습니다.

소스는 파일당 512 KiB, 세대당 64 MiB·4,096파일입니다. 이벤트는 파일당 256개, 세대당 8,192개이며 개별 직렬화 자료는 8 KiB까지입니다. JS/TS 바인딩 해석은 32단계, Rust 값 해석은 16단계로 제한합니다. 질의마다 후보 512개·출력 128개, 최신 소스 확인 128파일·4 MiB를 넘지 않습니다. 기존 read/search 출력 바이트 상한을 유지하고 입력 누락, 추출·질의 상한, 오래된 근거, 출력 생략 수를 표시합니다. 질의마다 작업공간 전체의 이벤트 관계를 다시 훑지 않습니다.
