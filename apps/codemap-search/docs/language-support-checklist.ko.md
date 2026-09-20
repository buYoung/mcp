# 지원 언어와 파일 형식

한국어 | [English](./language-support-checklist.md)

codemap-search가 색인에 포함할 수 있는 파일, 추출하는 구조, 정적 분석의 한계를 정리합니다. 시작 방법은 [README](../README.ko.md), 선택형 지원과 제외 규칙은 [설정 문서](./configuration.ko.md)를 참고하세요.

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

프로그래밍 언어에서는 선언과 검색 가능한 심볼·설명·리터럴을 추출합니다. 호출 관계 지원은 언어와 대상 확인 가능 여부에 따라 다릅니다. SQL은 선언·리터럴을 추출하며 호출자·호출 대상 관계를 만들지 않습니다.

## 데이터·마크업·컴포넌트

| 형식 | 확장자 | 추출 구조와 한계 |
|---|---|---|
| JSON / JSONC | `.json`, `.jsonc` | 키와 중첩 키 경로. JSONC 키는 따옴표가 필요하며 배열·스칼라 값은 텍스트로 검색 |
| TOML | `.toml` | 키와 중첩 키 경로. 값은 텍스트로 검색 |
| YAML | `.yaml`, `.yml` | 키와 중첩 키 경로. 값은 텍스트로 검색 |
| HTML | `.html`, `.htm` | 태그, `id`, `class` |
| XML과 파생 형식 | `.xml`, `.xsd`, `.xsl`, `.xslt`, `.plist`, `.csproj`, `.props`, `.targets` | XML 태그·속성. 파생 형식은 XML 문법 수준으로 분석 |
| CSS | `.css` | 선택자, 사용자 정의 속성, 키프레임 이름 |
| Less | `.less` | 스타일 구조와 믹스인. 식별 가능한 `#identifier(...)` 정의 포함 |
| Sass | `.sass` | 들여쓰기 Sass 구조. 복구할 수 없는 오류가 있으면 유효한 앞부분까지만 추출 |
| Vue / Astro / Svelte | `.vue`, `.astro`, `.svelte` | 마크업과 내부 JavaScript/TypeScript·CSS/Sass/Less. 원본 줄·열을 유지하고 중복 심볼 병합 |

데이터와 독립 마크업·스타일 형식은 일반 호출 관계를 만들지 않습니다. **JSON5와 SCSS는 지원 형식에 등록되어 있지 않습니다.** 컴포넌트 지원으로 SCSS 추출이 활성화되지는 않습니다.

## 선택형 지원 그룹

다섯 설정 모두 `[index.language_support]`에서 기본값이 `false`입니다.

| 키 | 그룹 |
|---|---|
| `is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

비활성 상태인 파일은 색인, search, overview, codemap, 파일 변경 갱신에서 제외합니다. 실시간 `find`/`grep`/`read`와 CLI의 직접 `parse`는 계속 사용할 수 있습니다. 값을 바꾸면 설정을 다시 읽은 뒤 전체 색인 갱신을 요청하며 소스 파일을 편집할 필요가 없습니다.

| 형식 | 추출 구조 | 한계 |
|---|---|---|
| Markdown / MDX | 전체 본문, ATX·Setext 제목, 인라인·참조·자동 링크, 울타리·들여쓰기 코드 블록, 원본 범위 | 링크 관계·import·참조·호출 관계를 만들지 않고 코드 블록을 재파싱하지 않음. MDX의 JSX·JavaScript 표현식은 텍스트 검색만 지원 |
| Bash / Zsh | 함수, 변수, 환경 변수, 경로가 명시된 `source`·`.` import | 정적 호출도 일반 호출 그래프에 넣지 않음. 동적 실행·`eval`·간접 확장·변수나 명령 치환이 포함된 import는 호출·참조·import로 기록하지 않음 |
| HCL / Terraform | 선언, 리소스, 이름 없는 `terraform` 블록, 확인 가능한 `var`/`local`/`module`/`data` 참조, 경로가 명시된 module `source` | 동적·모호한 관계 생략. 일반 호출 그래프 없음 |
| Dockerfile | ARG, ENV, LABEL, 단계, 기반 이미지 의존성 | 정확한 파일명 `Dockerfile` 지원. 일반 호출 그래프 없음 |
| Nix | 정적 속성 경로, `let`, `inherit`, 함수 연결, 직접 `derivation`/`mkDerivation` 대상, 명시된 `import`/`builtins.import`/`callPackage` 경로, 정적 참조·직접 함수 적용 | 확인된 직접 함수 적용은 `precise` 호출 가능. 평가기 실행·동적 속성이나 import 해석·flake `inputs`/`outputs`와 nixpkgs 관용구의 별도 의미 해석은 미지원 |
| Protocol Buffers | 선언, 서비스·타입, `import`, 필드·RPC 타입 | 동적·모호한 관계 생략. 일반 호출 그래프 없음 |
| GraphQL | 타입, 프래그먼트, 이름 없는 `schema` 정의, 프래그먼트 전개, 이름 있는 타입 참조 | 동적·모호한 관계 생략. 일반 호출 그래프 없음 |
| Make / CMake / Starlark-Bazel | 대상, 규칙, 변수, 대상 간 의존성 | 빌드 대상 의존성과 일반 함수 호출을 구분 |

## 언어별 추출 규칙

호출 관계는 기본적으로 추정값입니다. `navigation_context_default = true`이면 단일 호출 대상을 확인한 경우 `precise`로 표시합니다. 리플렉션을 포함해 실행 중에 결정되는 경로나 호출 대상은 확인된 관계로 취급하지 않습니다. `navigation_store_references`는 함수 호출 외의 참조 위치를 선택적으로 저장합니다. [설정 문서](./configuration.ko.md#출력과-호출-관계)를 참고하세요.

| 언어 | 공개·테스트·폐기 표시 규칙 |
|---|---|
| Go | 대문자로 시작하면 공개. `*_test.go`와 `Test`/`Benchmark`/`Example`/`Fuzz`는 테스트, 설명의 `// Deprecated:`는 폐기 표시 |
| Java | `public`, `@Test` / `*Test.java`, `@Deprecated` / javadoc `@deprecated` |
| Kotlin | `private`/`internal`/`protected`가 아니면 공개. `@Test`, `@Deprecated` 인식 |
| C / C++ | `static`은 파일 내부용. C++ 멤버는 접근 지정자를 따르며 기본값은 struct 공개·class 비공개 |
| Assembly / GAS | `.globl` / `.global`에 지정한 심볼을 공개로 표시 |
| C# | 명시적 `public`과 인터페이스의 암시적 공개 멤버 |
| PHP | 최상위 선언과 `private`/`protected`가 아닌 멤버는 공개 |
| Ruby | class/module의 공개 범위 구역 적용 |
| Lua | `local`을 포함한 파일 수준 선언 표시 |

C#·PHP·Ruby·Lua는 정적으로 확인되는 선언·경로가 명시된 import·참조·호출을 추출하고, 지원 범위의 테스트 경로·이름 관례와 폐기 속성·주석을 인식합니다. Swift·Dart·Scala·Groovy·PowerShell도 정적 선언·import·참조·호출과 언어별 공개·테스트·폐기 규칙을 적용합니다. PowerShell의 동적 실행은 확인된 호출로 표시하지 않습니다.

`.gradle`에서는 명시된 `task`/`tasks.register`/`tasks.create` 대상, 작업 간 의존·순서 관계, 플러그인 ID, `group:artifact:version` 좌표를 추가로 추출합니다. 보간된 값과 사용자 정의 DSL은 구조화하지 않습니다. `.gradle.kts`는 일반 Kotlin 지원을 사용합니다.

## 색인에서 제외하는 파일

다음 규칙은 지원 확장자보다 우선하며 ASCII 대소문자를 구분하지 않습니다.

| 구분 | 이름 또는 패턴 |
|---|---|
| 일반 텍스트 | `.txt` |
| 잠금 파일 | `*.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `Gemfile.lock`, `composer.lock`, `poetry.lock`, `Pipfile.lock` |
| 소스 맵 | `*.map` |
| 압축 파일 | `*.min.js`, `*.min.mjs`, `*.min.cjs`, `*.min.css`, `*.min.html` |
| 번들 | `*.bundle.js`, `*.bundle.mjs`, `*.bundle.cjs`, `*.bundle.css` |

예를 들어 JSON을 지원해도 `package-lock.json`은 색인하지 않습니다. 이 제외 규칙은 색인·코드맵·호출자 탐색에서 해제할 수 없습니다. `find`/`grep`은 `include_ignored: true`로 포함할 수 있고 직접 `read`/`parse`도 가능합니다. 저장소별 추가 제외는 `.codemapignore`에 작성하세요.

디렉터리 제외, 무시 파일, `max_file_size`도 색인 범위에 영향을 줍니다. 버전 관리 내부 폴더와 codemap 자체 폴더는 `include_ignored`를 켜도 제외합니다. 선택적·필수 규칙의 차이는 [디렉터리 제외 규칙](./configuration.ko.md#디렉터리-제외-규칙)을 참고하세요.

사용자 홈 자체는 MCP 작업공간이나 명시적 색인 대상이 될 수 없습니다. `~/work/project`는 허용하고 `~`는 거부합니다. 심볼릭 링크와 `..`를 포함해 경로를 정규화한 뒤 비교합니다. `HOME`/`USERPROFILE`을 사용하며 저장소 설정·색인 데이터를 만들기 전에 확인합니다. 홈 경로를 모두 확인할 수 없으면 stderr 경고 후 계속 실행합니다. 전역 설정인 `~/.codemap`은 허용합니다.

파싱 오류 때문에 MCP 서버나 백그라운드 색인 전체가 중단되지는 않습니다. 해당 파일의 구조는 일부만 추출되거나 없을 수 있습니다.
