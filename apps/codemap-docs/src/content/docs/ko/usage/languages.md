---
title: 지원 언어 및 색인 규칙
description: 지원하는 프로그래밍 언어와 데이터·마크업 형식, 선택형 언어 스위치, 추출 규칙, 색인 제외 파일, 색인 조건과 한도, stderr 진단을 정리합니다.
---

## 프로그래밍 언어

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

프로그래밍 언어 파일에서는 선언과 검색 가능한 심볼, 주석 문서, 문자열 리터럴을 추출합니다. 호출 관계 지원 수준은 언어와 정적 대상 확인 가능 여부에 따라 달라지며, SQL은 선언과 리터럴만 추출하고 호출자·호출 대상 관계는 만들지 않습니다.

## 데이터, 마크업, 컴포넌트

| 형식 | 확장자 | 추출 구조와 한계 |
|---|---|---|
| JSON / JSONC | `.json`, `.jsonc` | 키와 중첩 키 경로(JSONC 키는 따옴표 필수), 배열과 스칼라 값은 텍스트로 검색 |
| TOML | `.toml` | 키와 중첩 키 경로, 값은 텍스트로 검색 |
| YAML | `.yaml`, `.yml` | 키와 중첩 키 경로, 값은 텍스트로 검색 |
| HTML | `.html`, `.htm` | 태그, `id`, `class` |
| XML과 파생 형식 | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.plist`, `.csproj`, `.props`, `.targets` | XML 태그와 속성(파생 형식도 XML 문법 수준으로 분석) |
| CSS | `.css` | 선택자, 사용자 정의 속성, 키프레임 이름 |
| Less | `.less` | 스타일 규칙과 믹스인(식별 가능한 `#identifier(...)` 정의 포함) |
| Sass | `.sass` | 들여쓰기 Sass 구조(복구할 수 없는 문법 오류가 있으면 유효한 앞부분까지만 추출) |
| Vue / Astro / Svelte | `.vue`, `.astro`, `.svelte` | 마크업과 내장 JavaScript/TypeScript, CSS/Sass/Less(원본 줄과 열 범위를 유지하고 중복 심볼 병합) |

데이터 형식과 단독 마크업·스타일 형식은 함수 호출 관계를 생성하지 않습니다. **JSON5와 SCSS는 지원하지 않으며**, 컴포넌트 지원을 켜도 SCSS 추출은 활성화되지 않습니다.

## 선택형 지원 그룹

[설정 레퍼런스](/ko/configuration/)의 `[index.language_support]` 아래 다섯 스위치는 모두 기본값이 `false`입니다.

| 키 | 그룹 |
|---|---|
| `is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

비활성 그룹 파일은 색인, `search`, `codemap`, 파일 변경 감시에서 빠지지만 실시간 `find`, `grep`, `read`와 CLI `parse`는 그대로 동작합니다. 스위치를 바꾸면 설정을 다시 읽은 뒤 소스 파일을 고치지 않아도 전체 색인 갱신을 실행합니다.

| 형식 | 추출 구조 | 한계 |
|---|---|---|
| Markdown / MDX | 전체 본문, ATX 및 Setext 제목, 인라인/참조/자동 링크, 코드 블록, 원본 범위 | 링크 관계나 import, 참조, 호출 그래프는 만들지 않고 코드 블록도 재파싱하지 않으며, MDX의 JSX/JS 표현식은 텍스트 검색만 지원 |
| Bash / Zsh | 함수, 변수, 환경 변수, 리터럴 `source` 및 `.` import | 정적 셸 호출을 포함한 호출 그래프 없음. `eval`, 간접 확장, 변수나 명령 치환이 들어간 동적 import는 호출, 참조, import로 기록하지 않음 |
| HCL / Terraform | 선언, 리소스, 이름 없는 `terraform` 블록, 정적으로 확인 가능한 `var`/`local`/`module`/`data` 참조, 리터럴 module `source` | 동적이거나 모호한 관계 생략, 호출 그래프 없음 |
| Dockerfile | `ARG`, `ENV`, `LABEL`, 빌드 단계, 기반 이미지 의존성 | 정확한 파일명 `Dockerfile`만 지원, 호출 그래프 없음 |
| Nix | 정적 속성 경로, `let`, `inherit`, 함수 바인딩, 직접 `derivation`/`mkDerivation` 대상, 리터럴 `import`/`builtins.import`/`callPackage` 경로, 정적 참조와 직접 함수 적용 | 확인된 직접 함수 적용은 `precise` 호출로 표시 가능. 평가기 실행이나 동적 속성·import 해석, flake나 nixpkgs 관용구의 별도 의미 해석은 미지원 |
| Protocol Buffers | 선언, 서비스와 타입, `import`, 필드 및 RPC 타입 | 동적이거나 모호한 관계 생략, 호출 그래프 없음 |
| GraphQL | 타입, 프래그먼트, 이름 없는 `schema` 정의, 프래그먼트 전개, 이름 있는 타입 참조 | 동적이거나 모호한 관계 생략, 호출 그래프 없음 |
| Make / CMake / Starlark-Bazel | 대상, 규칙, 변수, 대상 간 의존성 | 빌드 대상 의존성을 일반 함수 호출과 분리해 표시 |

## 언어별 추출 규칙

호출 관계는 기본적으로 추정값으로 표시되며, `[output.navigation].is_enabled = true`일 때 단일 호출 대상을 확인한 경우에만 `precise`로 표시합니다. 리플렉션처럼 실행 시점에 결정되는 경로나 호출 대상은 확인된 관계로 취급하지 않습니다. 함수 호출 외의 참조 위치 저장은 `[index].store_references`로 제어합니다([출력과 호출 관계](/ko/configuration/#출력과-호출-관계) 참고).

| 언어 | 공개, 테스트, 폐기 표시 규칙 |
|---|---|
| Go | 대문자로 시작하면 공개, `*_test.go`와 `Test`/`Benchmark`/`Example`/`Fuzz`는 테스트, `// Deprecated:` 주석은 폐기 표시 |
| Java | `public`, `@Test` / `*Test.java`, `@Deprecated`와 Javadoc `@deprecated` |
| Kotlin | `private`, `internal`, `protected`가 아니면 공개, `@Test`와 `@Deprecated` 인식 |
| C / C++ | `static`은 파일 내부 전용, C++ 멤버는 접근 지정자를 따르며 기본값은 `struct` 공개, `class` 비공개 |
| Assembly / GAS | `.globl` / `.global`로 지정한 심볼을 공개로 표시 |
| C# | 명시적 `public`과 인터페이스의 암시적 공개 멤버 |
| PHP | 최상위 선언과 `private` 또는 `protected`가 아닌 멤버를 공개로 처리 |
| Ruby | class 및 module의 가시성 구역 규칙 적용 |
| Lua | `local`을 포함한 파일 수준 선언 노출 |

C#, PHP, Ruby, Lua는 정적 선언, 리터럴 import, 참조, 호출을 추출하며 관례적 테스트 경로·이름과 폐기 속성·주석을 인식합니다. Swift, Dart, Scala, Groovy, PowerShell도 정적 선언, import, 참조, 호출과 언어별 공개, 테스트, 폐기 규칙을 따릅니다(PowerShell의 동적 실행은 확인된 호출로 표시하지 않습니다).

`.gradle`에서는 리터럴 `task`/`tasks.register`/`tasks.create` 대상, 작업 간 의존과 실행 순서, 플러그인 ID, `group:artifact:version` 좌표도 함께 추출하며, 보간 문자열과 사용자 정의 DSL은 구조화하지 않습니다. `.gradle.kts`는 일반 Kotlin 지원을 따릅니다.

## 색인에서 제외하는 파일

아래 파일명과 확장자 규칙은 지원 언어 목록보다 우선하며 ASCII 대소문자를 가리지 않습니다.

| 구분 | 이름 또는 패턴 |
|---|---|
| 일반 텍스트 | `.txt` |
| 인증정보 | `auth.toml` |
| 잠금 파일 | `*.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `Pipfile.lock` |
| 소스 맵 | `*.map` |
| 압축 파일 | `*.min.js`, `*.min.mjs`, `*.min.cjs`, `*.min.css`, `*.min.html` |
| 번들 | `*.bundle.js`, `*.bundle.mjs`, `*.bundle.cjs`, `*.bundle.css` |

예를 들어 JSON 지원이 켜져 있어도 `package-lock.json`은 색인하지 않습니다. 이 규칙은 색인과 `codemap`, 호출자 탐색에서 끌 수 없지만 `find`와 `grep`에서는 `include_ignored: true`로 볼 수 있고 직접 `read`나 `parse`도 가능합니다. 저장소별 추가 제외 패턴은 `.codemapignore`에 적으세요.

디렉터리 제외 규칙, 무시 파일, `index.max_file_bytes`도 색인 대상을 걸러 줍니다. VCS 내부 폴더와 `.codemap` 폴더는 `include_ignored`를 켜도 항상 제외됩니다([디렉터리 제외 규칙](/ko/configuration/#디렉터리-제외-규칙) 참고).

사용자 홈 디렉터리 자체(`~`)는 MCP 작업공간이나 명시적 색인 대상으로 열 수 없으며 `~/work/project` 같은 하위 프로젝트만 허용됩니다. 경로는 저장소 설정이나 색인 파일을 만들기 전에 심볼릭 링크와 `..`를 정리해 `HOME` 또는 `USERPROFILE`과 비교하며, 두 변수를 모두 확인할 수 없으면 `stderr` 경고를 내고 계속 진행합니다. 전역 설정 폴더인 `~/.codemap`은 허용됩니다.

구문이나 파싱 오류가 나도 MCP 서버나 백그라운드 색인은 멈추지 않으며, 해당 파일만 구조 정보가 일부 빠지거나 비어 있습니다.

## 색인, 진단 및 제한

MCP 서버는 기본적으로 `.codemap/index`에 색인을 만들거나 기존 색인을 불러옵니다. 기본값인 `[index] mode = "auto"`에서는 첫 `search`, `read`, `grep`, `synonyms` 또는 모노레포 `initial_instructions` 호출 때 색인을 시작하며, 색인 가능한 파일이 `50,000`개 이하인 저장소 안에서만 동작합니다. 여러 저장소를 묶어 둔 상위 폴더처럼 저장소가 아닌 위치에서는 `search`가 색인하지 않은 이유를 안내하며, 그래도 색인하려면 `mode = "on"`을 설정하세요([색인 시작 시점](/ko/configuration/#색인-시작-시점) 참고).

파일 감시가 켜져 있으면 편집 이벤트를 기본 `500ms` 동안 모아 바뀐 경로만 갱신하고, 무시 규칙이 가린 경로는 건너뜁니다. Git `HEAD` 전환, ignore 파일 수정, 큰 변경 묶음이 발생하면 전체 트리를 다시 탐색합니다. 감시가 꺼져 있거나 동작하지 않으면 `search`가 `index.refresh.index_staleness_ms` 간격으로 갱신을 확인하며, `read`, `find`, `grep`은 항상 디스크에서 직접 읽습니다.

- `index.max_file_bytes`(기본 `1 MiB`)보다 큰 파일은 색인과 `codemap` 대상에서 건너뜁니다.
- `.txt`, 잠금 파일, 소스 맵, 압축 파일, 번들 파일은 별도 제외 규칙을 따릅니다(`find`와 `grep`에서는 `include_ignored`로 우회 가능하며, 직접 `read`와 `parse`도 가능합니다).
- 실행 시점에 결정되는 경로나 호출 대상은 정적 분석으로 확정할 수 없으므로 추정한 호출 관계는 원문에서 직접 확인하세요.
- `codemap-search mcp`는 단일 클라이언트용 순차 stdio 서버이므로 같은 색인 디렉터리에 여러 서버를 동시에 띄우지 마세요.

진단 로그는 `stderr`로 출력되며 기본 로그 필터는 `warn,codemap_search=info`입니다.

```sh
RUST_LOG=debug codemap-search mcp
```
