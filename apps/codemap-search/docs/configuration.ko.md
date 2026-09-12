# 설정

한국어 | [English](./configuration.md)

codemap-search는 설정 파일 없이도 기본값으로 동작합니다. 변경할 키만 설정하면 나머지는 전역 설정이나 내장 기본값을 사용합니다.

## 설정 위치와 우선순위

키별 우선순위는 저장소 → 전역 → 내장 기본값입니다. 저장소 파일에 `[search].result_threshold`만 있으면 다른 키는 전역 설정이나 기본값을 이어받습니다.

| 범위 | 경로 |
|---|---|
| 저장소 | `<repo>/.codemap/config.toml` |
| 전역 | `$CODEMAP_HOME/config.toml`, 미지정 시 `~/.codemap/config.toml` |

## 설정 읽기와 자동 작성

현재 설정 버전은 **7**이며 주석으로 표시합니다.

```toml
# codemap-config-version: 7
```

- 설정 파일은 없어도 됩니다. TOML 구문이 잘못되면 해당 파일의 설정 전체를 사용하지 않습니다. 알 수 없는 키·잘못된 자료형·허용되지 않는 값은 stderr에 경고하고 해당 키만 낮은 우선순위 설정으로 대체합니다. 저장소 값이 잘못되어도 유효한 전역값이 있으면 기본값보다 우선합니다.
- `[update].config_auto_update = true`이면 MCP 시작 시 누락된 저장소 설정을 만듭니다. 제외 배열에는 공통 폴더와 감지한 프로젝트 종류의 재귀 glob을 넣고, 다른 활성 키에는 내장 기본값을 씁니다. 활성 저장소 키는 전역값보다 우선합니다.
- 버전 표시가 없거나 6 이전인 저장소 설정은 **제외 목록을 한 번 전환**합니다. 사용자 규칙, 이전에 적용되던 제외값, 공통 폴더, 추천 재귀 glob을 배열에 명시합니다. 기존 항목과 주석은 보존하고 누락된 값만 중복 없이 추가한 뒤 현재 스키마 버전으로 바꿉니다.
- **버전 6부터 `excluded_directories`는 자동으로 만들거나 보충하지 않습니다.** 항목 삭제, `[]` 지정, 키 주석 처리, 새 프로젝트 추가 후에도 목록을 복원하지 않습니다. 수동 변경을 읽어 적용하는 동작은 계속됩니다.
- 일반 설정 버전 갱신은 새 키를 주석으로 추가하며 자동으로 활성화하지 않습니다. 이미 최신인 파일은 다시 쓰지 않습니다.
- `config_auto_update = false`는 최초 생성과 전환을 모두 끕니다. 설정 읽기와 감시는 계속되며, 전역 파일은 항상 자동 생성·전환 대상에서 제외됩니다.
- 운영체제 언어가 한국어이면 한국어 주석을, 그 밖에는 영어 주석을 생성합니다. 프로젝트 감지 전 두 템플릿의 키와 값은 같습니다.

전환 중 파일이 바뀌거나 TOML 구문·쓰기 권한·지원하지 않는 테이블 구조 때문에 전환할 수 없으면 파일을 그대로 두고 경고합니다. 원인을 수정한 뒤 재시작하세요. 점으로 연결하거나 인라인으로 작성한 index 테이블에 제외 배열이 없다면 배열을 명시하고 다시 시도하세요. 설정 파일의 심볼릭 링크는 유지합니다.

### 자동 작성을 껐을 때 수동 전환

버전 6에서는 명시한 배열이 선택적 기본 목록을 대체합니다. 이전 배열은 내장 목록에 추가하는 방식이었습니다. 자동 작성을 껐다면 기존 제외를 유지하기 위해 `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor` 중 필요한 항목을 직접 넣고, 공통·프로젝트 규칙을 추가한 뒤 버전 주석을 6으로 바꾸세요. 다른 키는 전환할 필요가 없습니다. 직접 편집하기 전에 설정을 백업하세요.

전환하면서 상속받던 전역 제외값을 저장소 배열에 기록할 수 있습니다. 이후 전역 변경을 다시 상속하려면 저장소 키를 주석 처리하세요. 전역 배열도 버전 6에서는 전체 목록이며 자동으로 수정하지 않습니다.

## 디렉터리 제외 규칙

`[index].excluded_directories`는 **선택적으로 제외할 디렉터리 규칙의 전체 목록**입니다. 명시한 배열에 숨겨진 기본 목록을 더하지 않습니다. `[]`는 선택적 규칙을 해제하고, 키 생략은 전역 목록 또는 기본값을 상속합니다. 무시 파일과 필수 제외 규칙은 별도로 적용합니다.

공통 초기 목록은 다음과 같습니다.

```toml
[index]
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
| `[index].excluded_directories` | 문자열 배열(상대 디렉터리 glob) | 공통 + 기존 기본 이름. 생성 파일은 재귀 glob 사용 | 선택적 제외 전체 목록. [디렉터리 제외 규칙](#디렉터리-제외-규칙) 참고 |
| `[index].use_git_exclude` | bool | `true` | `.git/info/exclude` 적용 여부 |
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
| `[caller_context].should_include_test_code` | bool | `false` | 자동 심볼·호출 관계에 테스트 코드 포함 |
| `[caller_context].test_file_patterns` | 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 파일 glob. []는 경로 판별 해제 |
| `[caller_context].test_attributes` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 속성·어노테이션 패턴. 언어별 목록을 상속값 대신 적용 |
| `[caller_context].test_decorators` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 데코레이터 패턴. []는 해당 언어 목록 해제 |
| `[caller_context].test_calls` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 호출 패턴. []는 해당 언어 목록 해제 |
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
[caller_context]
should_include_test_code = false
# Replace the complete path list with the patterns you want.
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[caller_context.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[caller_context.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[caller_context.test_calls]
typescript = []
```

기본 목록입니다. 표에 없는 언어는 해당 종류의 내장 항목이 없습니다.

```toml
[caller_context]
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[caller_context.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]

[caller_context.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[caller_context.test_calls]
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
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
# Initial generation also adds recursive globs for detected project types; edit them manually from v6 onward.
use_git_exclude = true

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

[caller_context]
caller_context_default = true
should_include_test_code = false
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
