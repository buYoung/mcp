# 개발 언어 전체 공개 저장소 검증

Rust·Go 검증을 포함해 25개 개발 언어, 50개 언어·저장소 조합을 대상으로 검증을 확대했다. 저장소는 49개다. PostgreSQL을 C와 SQL에 각각 사용했다. 모든 대상의 커밋 수는 1,000개 이상이며, 사용자가 승인한 코드 규모 예외는 10개 조합에 적용했다.

호출 오연결, 컴포넌트 본문의 가짜 호출자, PowerShell 선언 종류, Dart 본문 범위, 대형 저장소의 파서 오류를 수정했다. 확대 검증에서 상수 참조 문맥 누락과 일부 선언·검색 문제도 추가로 확인했다. 이 문서는 통과한 검사와 남은 실패·판정 보류를 구분한다. 전체 언어의 무오류 판정이나 전수 코드 검토 결과가 아니다.

## 대상과 고정 조건

전체 주소·Git SHA·코드 줄 수·커밋 수·측정 자료 해시는 [development-corpus.json](../validation/development-corpus.json)에 있다. 후보 및 선택 기준은 [development-languages.json](../validation/development-languages.json)에 고정했다. Rust·Go의 최초 기준선과 수정 과정은 [별도 보고서](public-repository-validation.ko.md)에 보존한다.

| 언어 | 저장소 A: 코드 줄 / 커밋 수 | 저장소 B: 코드 줄 / 커밋 수 |
| --- | --- | --- |
| Rust | [bevyengine/bevy](https://github.com/bevyengine/bevy): 455,742 / 12,180 | [nushell/nushell](https://github.com/nushell/nushell): 358,894 / 11,853 |
| Go | [prometheus/prometheus](https://github.com/prometheus/prometheus): 289,673 / 18,628 | [kubernetes/kubernetes](https://github.com/kubernetes/kubernetes): 2,201,458 / 141,099 |
| Python | [django/django](https://github.com/django/django): 433,323 / 34,925 | [sympy/sympy](https://github.com/sympy/sympy): 659,157 / 62,895 |
| TypeScript | [microsoft/vscode](https://github.com/microsoft/vscode): 3,193,465 / 165,248 | [microsoft/TypeScript](https://github.com/microsoft/TypeScript): 518,272 / 39,365 |
| JavaScript | [nodejs/node](https://github.com/nodejs/node): 806,188 / 48,437 | [react/react](https://github.com/react/react): 535,511 / 21,694 |
| Java | [spring-projects/spring-framework](https://github.com/spring-projects/spring-framework): 858,662 / 35,561 | [apache/kafka](https://github.com/apache/kafka): 1,107,688 / 18,226 |
| C# | [dotnet/roslyn](https://github.com/dotnet/roslyn): 5,229,013 / 146,675 | [AvaloniaUI/Avalonia](https://github.com/AvaloniaUI/Avalonia): 476,820 / 28,078 |
| PHP | [symfony/symfony](https://github.com/symfony/symfony): 1,770,874 / 83,411 | [magento/magento2](https://github.com/magento/magento2): 1,929,569 / 161,785 |
| Ruby | [rails/rails](https://github.com/rails/rails): 401,984 / 99,650 | [discourse/discourse](https://github.com/discourse/discourse): 1,090,891 / 68,218 |
| Lua | [Kong/kong](https://github.com/Kong/kong): 266,015 / 11,269 | [tarantool/tarantool](https://github.com/tarantool/tarantool): 365,561 / 20,081 |
| Kotlin | [JetBrains/kotlin](https://github.com/JetBrains/kotlin): 2,557,291 / 142,729 | [signalapp/Signal-Android](https://github.com/signalapp/Signal-Android): 496,937 / 20,644 |
| Swift | [signalapp/Signal-iOS](https://github.com/signalapp/Signal-iOS): 611,023 / 40,356 | [TelegramMessenger/Telegram-iOS](https://github.com/TelegramMessenger/Telegram-iOS): 1,639,964 / 30,744 |
| Dart | [flutter/flutter](https://github.com/flutter/flutter): 1,864,700 / 91,505 | [dart-lang/sdk](https://github.com/dart-lang/sdk): 4,186,990 / 115,328 |
| Scala | [apache/spark](https://github.com/apache/spark): 1,407,813 / 50,101 | [scala/scala3](https://github.com/scala/scala3): 628,257 / 52,831 |
| Groovy | [gradle/gradle](https://github.com/gradle/gradle): 875,000 / 136,852 | [apache/groovy](https://github.com/apache/groovy): 371,459 / 23,596 |
| PowerShell | [Azure/azure-powershell](https://github.com/Azure/azure-powershell): 533,674 / 41,126 | [dataplat/dbatools](https://github.com/dataplat/dbatools): 194,329† / 18,258 |
| C | [postgres/postgres](https://github.com/postgres/postgres): 997,582 / 65,353 | [openssl/openssl](https://github.com/openssl/openssl): 638,735 / 41,069 |
| C++ | [godotengine/godot](https://github.com/godotengine/godot): 1,204,399 / 86,363 | [bitcoin/bitcoin](https://github.com/bitcoin/bitcoin): 281,258 / 50,560 |
| ASM | [FFmpeg/FFmpeg](https://github.com/FFmpeg/FFmpeg): 160,807† / 126,526 | [gnutools/glibc](https://github.com/gnutools/glibc): 238,951 / 44,041 |
| SQL | [ClickHouse/ClickHouse](https://github.com/ClickHouse/ClickHouse): 366,503 / 283,463 | [postgres/postgres](https://github.com/postgres/postgres): 138,343† / 65,353 |
| Bash | [git/git](https://github.com/git/git): 301,939 / 82,209 | [rear/rear](https://github.com/rear/rear): 25,018† / 7,088 |
| Zsh | [ohmyzsh/ohmyzsh](https://github.com/ohmyzsh/ohmyzsh): 27,374† / 7,919 | [zdharma-continuum/zinit](https://github.com/zdharma-continuum/zinit): 7,738† / 4,008 |
| Vue | [gitlabhq/gitlabhq](https://github.com/gitlabhq/gitlabhq): 362,693 / 118,161 | [opentiny/tiny-vue](https://github.com/opentiny/tiny-vue): 318,992 / 3,109 |
| Astro | [withastro/astro](https://github.com/withastro/astro): 24,071† / 15,029 | [withastro/starlight](https://github.com/withastro/starlight): 4,773† / 3,754 |
| Svelte | [sveltejs/svelte](https://github.com/sveltejs/svelte): 47,882† / 11,399 | [immich-app/immich](https://github.com/immich-app/immich): 36,788† / 10,980 |

`†`는 200,000줄 미만의 승인된 규모 예외다. 줄 수는 Git 추적 파일의 대상 확장자를 `tokei 14.0.0`으로 측정한 값이다. 공백·주석, 의존성·빌드 디렉터리, 명시적인 생성 표식과 심볼릭 링크를 제외하고 사람이 작성한 테스트는 포함한다. 생성 표식이 없는 생성 코드를 전부 식별했다는 뜻은 아니다.

Vue·Astro·Svelte 줄 수에는 마크업과 포함된 스크립트·스타일이 함께 들어간다. 이를 JS/TS 코드만의 크기로 해석하면 안 된다. 셸 지원은 검증 설정에서 명시적으로 켰다. 문서, JSON/TOML/YAML, HTML/CSS 계열, HCL, Dockerfile, Proto/GraphQL, Make/CMake/Starlark/Nix는 이번 **언어별 선정 대상**에서 제외했다. 저장소 전체를 색인할 때 설정상 허용된 다른 형식이 함께 처리되는 것은 유지했다.

ASM 후보 `pret/pokecrystal`은 RGBDS 문법을 사용해 현재 GAS/Intel 계열 문법과 대조하기에 부적합했다. 최초 결과는 지우지 않고 선정 이유를 바꿔 FFmpeg와 [GNU glibc 미러](https://github.com/gnutools/glibc)를 사용했다. Linux 후보는 macOS 작업 트리에서 대소문자가 충돌하는 추적 경로가 확인돼 사용하지 않았다.

## 독립 대조와 검사 범위

확대 대상은 저장소마다 코드가 있는 적격 파일을 최대 20개 선정했다. 표본 순서는 `SHA-256("codemap-languages-v1:<저장소>:<경로>")`로 고정한다. Git 제외 파일, 1 MiB 초과 파일은 표본에서 뺀다. Zinit은 적격 파일이 9개라 그 9개를 선정했지만, 색인 준비 시간 초과 때문에 파일별 대조까지 진행하지 못했다.

| 검사 | 독립 기준과 범위 |
| --- | --- |
| `overview` | 표본의 선언 이름·범위가 실제 원문에 존재하는지 검사하고, 별도 파서가 수집한 이름 있는 함수·메서드의 누락과 끝 줄을 대조한다. |
| `read`·`grep`·`find` | 저장소별 첫 세 표본에서 원문의 8줄, 토큰이 있는 모든 줄, 실제 파일 경로를 대조한다. 선행 UTF-8 BOM 제거는 `read` 계약에 맞춘다. |
| `search` | 첫 세 표본의 독립 선언 이름과 파일명을 조합한 질의에서 해당 파일이 출력되는지 측정한다. 이름이 흔하거나 대조기가 만든 이름이면 검색 품질 또는 검사 입력의 문제로 추가 검토한다. |
| 호출 문맥 | 작은 대조 파일을 각 저장소에 추가해 실제 호출 위치, 문자열·주석의 비호출, 미확정 수신 객체의 `unresolved` 표시를 검사한다. |
| 컴포넌트 | script 내부의 실제 호출은 유지하고 마크업의 `target()` 텍스트는 호출자로 연결하지 않는지 확인한다. |
| 제외 | `node_modules`, 사용자 제외 디렉터리, `.gitignore` 경로의 기본 제외와 `include_ignored=true`의 명시적 우회를 검사한다. 검색 결과에도 제외 경로가 없어야 한다. |
| 상수 문맥 | 별도의 작은 대조에서 언어 문법상 상수·불변 바인딩의 정의 위치와 값이 참조 목록에 나오는지 확인한다. Python/Lua/PowerShell/셸의 대문자 일반 변수는 상수로 판정하지 않는다. |

각 저장소에서 `navigation_context_default`와 `navigation_store_references`가 모두 꺼진 `default`, 모두 켜진 `structural` 설정을 사용한다. 나머지는 [검증 설정](../validation/config.toml)을 공통으로 사용하며, 자동 문맥에서는 테스트 코드를 제외한다. 이 명칭은 모든 설정이 제품 기본값이라는 뜻은 아니다. Rust·Go는 별도로 테스트 포함·사용자 규칙 교체·복원까지 검사한다.

| 언어 | 별도 파서 |
| --- | --- |
| Rust / Go | `rust-analyzer parse --json` / 표준 `go/parser` |
| Python | Python `ast` |
| TypeScript / JavaScript | TypeScript 6.0.3 컴파일러의 구문 트리 |
| Swift | `swiftc -frontend -dump-parse` |
| Dart | Dart 3.11.4 + analyzer 13.0.0의 [`parseString`](https://pub.dev/documentation/analyzer/latest/dart_analysis_utilities/parseString.html) |
| Scala | [Scalameta](https://scalameta.org/docs/trees/guide.html) 4.17.3, Scala 2.13/3 구문을 각각 시도 |
| Groovy | Groovy 3.0.25의 [`SourceUnit`](https://docs.groovy-lang.org/latest/html/gapi/org/codehaus/groovy/control/SourceUnit.html) `parse`·`convert` |
| Vue / Astro / Svelte | 별도 HTML/frontmatter 분리기와 TypeScript 컴파일러로 명시적 JS/TS script 블록 대조 |
| 나머지 개발 언어 | Universal Ctags 6.2.1 |

대조 파서는 제품의 Tree-sitter 결과를 복사하지 않는다. 다만 Ctags는 전체 언어 컴파일러가 아니고, 컴포넌트 대조기도 템플릿 전체의 정답기가 아니다. 타입 검사·매크로 확장·런타임 동적 바인딩을 전수 검증하지 않는다. 공개 저장소의 빌드·테스트·프로젝트 스크립트는 실행하지 않았다. Groovy는 프로젝트 변환을 실행하는 `CompilationUnit`을 사용하지 않으며, 파서 도우미 의존성은 제품 런타임과 분리한 캐시에 준비했다.

## 실행 결과

기계가 읽을 수 있는 원본 집계는 [development-results.json](../validation/development-results.json)에 있다. 확대 대상의 실제 표본 900개를 두 설정에서 대조했다.

| 확대 언어 | 완료 설정 / 예정 설정 | 통과 | 실패 | 보류 |
| --- | ---: | ---: | ---: | ---: |
| Python | 4 / 4 | 244 | 2 | 0 |
| TypeScript | 4 / 4 | 242 | 2 | 6 |
| JavaScript | 4 / 4 | 234 | 4 | 10 |
| Java | 4 / 4 | 248 | 0 | 2 |
| C# | 4 / 4 | 232 | 0 | 18 |
| PHP | 4 / 4 | 242 | 2 | 2 |
| Ruby | 4 / 4 | 236 | 0 | 8 |
| Lua | 4 / 4 | 224 | 0 | 24 |
| Kotlin | 4 / 4 | 218 | 4 | 26 |
| Swift | 4 / 4 | 236 | 0 | 16 |
| Dart | 4 / 4 | 228 | 0 | 24 |
| Scala | 4 / 4 | 202 | 14 | 34 |
| Groovy | 4 / 4 | 220 | 2 | 28 |
| PowerShell | 4 / 4 | 248 | 0 | 0 |
| C | 4 / 4 | 246 | 2 | 4 |
| C++ | 4 / 4 | 214 | 0 | 36 |
| ASM | 4 / 4 | 170 | 8 | 66 |
| SQL | 4 / 4 | 234 | 0 | 0 |
| Bash | 4 / 4 | 238 | 0 | 0 |
| Zsh | 2 / 4 | 110 | 0 | 10 |
| Vue | 4 / 4 | 222 | 2 | 18 |
| Astro | 4 / 4 | 240 | 0 | 0 |
| Svelte | 4 / 4 | 212 | 4 | 24 |
| **합계** | **90 / 92** | **5140** | **46** | **356** |

Rust·Go는 공유 파서 변경 후 관련 사례 **82/82**, 작은 회귀 **12/12**를 통과했다. 기존 전체 검사 470항목과 구분한 재검증이다. 확대 언어의 별도 작은 대조는 호출·제외 등 **484항목 통과**, 상수 문맥 **4항목 통과·22항목 실패**다. 상수 실패는 11개 언어를 두 설정으로 검사한 결과다.

최종 소스의 기존 검사는 라이브러리 184개, e2e 165개, 추출 스냅샷 21개, 파일 형식 12개, 탐색 fixture 1개로 **383개 모두 통과**했다. `cargo check`, 전체 target Clippy(`-D warnings`), 서식 검사와 커밋 훅도 통과했다. 설치 바이너리 SHA-256은 `f82ab62bc259263c33d40dbbfbfcd8e8d6b21fed2abb0e7c3c95f092b4055732`이며, Azure 최종 재검증 바이너리와 같다.

| 재검증 실행 | 범위 | 바이너리 SHA-256 앞 12자리 |
| --- | --- | --- |
| `development-final-a-20260913` | Python·TS/JS·Java·C#·PHP·Ruby·Lua | `9d9367fc85ab` |
| `development-final-b-20260913` | 나머지 언어, ClickHouse 반복 순회 수정 포함 | `7d403d42b673` |
| `development-final-c-20260913` | Flutter·Dart SDK 본문 범위/색인 성능 재검증 | `29a72be7d93a` |
| `development-final-d-20260913` | Azure PowerShell 캐시 유지 수정 재검증 | `f82ab62bc259` |


서로 다른 수정 단계의 바이너리를 한 버전의 전수 검사처럼 합치지 않는다. 결과 요약에는 저장소·설정별 선택한 실행 ID와 바이너리 SHA-256을 보존한다. 같은 사례의 수정 전 응답도 원래 실행 디렉터리에 남긴다. `pass`는 해당 검사 조건의 통과, `fail`은 대조 조건 불충족, `unverified`는 파서 차이나 누락에 대한 판정 보류다. 실행 시간 초과와 실행하지 못한 후속 설정은 검사 통계 밖에 별도로 표시한다.

최초 확대 기준선 `all-development-baseline-20260913`에는 Ctags가 지원하지 않는 `--` 인자, 유효하지 않은 검색 범위, BOM 비교, 컴포넌트의 호출 지원 여부를 잘못 가정한 검사도 있었다. 이 항목은 제품 오류와 구분해 실행기를 보정하고 재실행했다. 최초 응답·실패 기록을 성공 결과로 덮어쓰지 않았다.

## 수정한 문제

- **호출 정의 오연결:** Rust·Go의 원문 기반 모듈 판정을 유지하고, 나머지 개발 언어는 같은 파일의 유효 범위와 명시적인 상대 JS/TS import를 확인한다. 매개변수·지역 바인딩·클래스 범위가 다른 동명 후보, 확정되지 않은 수신 객체는 정의 링크 없이 `unresolved`로 표시한다. 일반적인 프로젝트 전체 타입 추론을 추가한 것은 아니다.
- **Vue·Astro·Svelte:** 확인한 script/frontmatter 구간의 실제 구문 트리를 사용한다. 본문의 `target()` 문자열과 서로 다른 script 블록을 같은 호출 범위로 합치지 않는다.
- **PowerShell:** 함수가 `unknown`으로 나오던 query capture를 수정하고, 전역 변수의 범위를 개별 대입문으로 제한했다. 정적인 멤버 호출 이름은 수집하되 동적인 멤버 이름은 확정하지 않는다.
- **Dart:** 메서드 signature만 색인하던 query를 본문·생성자 초기화 목록을 포함한 선언으로 바꿨다. Flutter·Dart SDK 재검증에서 매칭된 선언의 끝 줄 불일치는 없어졌다. getter·연산자 등의 누락은 남아 있다.
- **ClickHouse 색인 종료:** 공유 타입 선언 탐색에서 깊은 구문 트리를 재귀 순회하다 stack overflow가 발생했다. 반복 순회와 파일당 타입 선언 조회표로 바꿨다. 실제 ClickHouse 두 설정에서 각각 58항목을 통과했다. 저장소 전체 색인 중 발생한 오류이며 SQL 문법 자체가 원인이라고 단정하지 않는다.
- **Dart SDK 색인 지연:** 참조마다 조상 노드를 반복 탐색하던 경로를 파일당 한 번의 범위 조회표로 바꿨다. 구조 설정이 1,200초 제한을 넘기던 저장소가 수정 후 약 62초에 준비됐다. 기존 중첩 함수 범위는 회귀 검사로 대조했다.
- **PowerShell 반복 요청 지연:** 테스트 영역 캐시가 1,024개에서 전체 초기화돼 다음 요청마다 다시 파싱하던 문제를 수정했다. 현재 색인 파일의 캐시를 유지하고 삭제된 파일은 제거하며, 색인 밖 파일은 추가 1,024개 범위 안에서 보관한다. 같은 MCP 프로세스의 문제 `grep`은 180초 초과에서 약 3.7초로 줄었다. 첫 요청의 약 154초 비용은 남아 있다.

PowerShell·Dart 추출 형식 변경은 `v21-dart-member-ranges`로 반영해 이전 색인을 한 번 재추출한다. 저장소의 설정 계약이나 공개 JSON 키를 바꾸지는 않았다.

시간은 이 macOS arm64 환경에서 관측한 값이다. OS 캐시와 CPU 부하를 격리하지 않았고 재검증의 작업 트리·색인 재사용 여부도 실행별로 다르다. 일반적인 성능 배수로 해석하지 않으며, 각 설정의 `ready_seconds`와 요청별 시간을 원시 자료에 보존한다.

## 남은 결함과 대조기의 한계

| 분류 | 재현 근거와 현재 상태 |
| --- | --- |
| 주요: Zsh 파서 지연 | Zinit의 기본 설정에서 1,200초 안에 색인이 준비되지 않았다. 프로세스 표본은 `tree_sitter_zsh`의 파싱 내부에 머물렀다. 구조 설정은 실행하지 못했으며 이 저장소를 통과로 계산하지 않는다. Oh My Zsh는 두 설정을 실행했다. |
| 주요: 첫 자동 문맥의 지연 | Azure PowerShell 첫 `read`는 캐시가 비어 있을 때 약 154초가 걸렸다. 캐시 유지 수정은 반복 파싱을 해결하지만, 첫 전체 테스트 영역 분석의 비용을 해결하지 않는다. `cm`은 명령마다 새 MCP 프로세스를 시작하므로 큰 저장소에서 이 비용이 다시 발생할 수 있다. |
| 상수 참조 지원 누락 | Java·C#·PHP·Ruby·Kotlin·Swift·Dart·Scala·Groovy·C·C++ 대조에서 참조 위치·값이 나오지 않았다. 클래스의 멤버 목록에 상수 이름이 나오는 것과 참조 문맥은 구분한다. Rust/Go/JS/TS의 통과 범위를 다른 언어로 일반화하지 않는다. |
| 선언 종류 누락 | Avalonia `ImmutableExperimentalAcrylicMaterial.cs:63,68`의 연산자, Flutter `box_border.dart:118,504,742`의 getter·연산자, Tarantool `test/sql-luatest/gh_6422_autoinc_ids_reset_test.lua:15`의 테이블에 대입한 함수 등을 확인했다. Swift에서도 연산자 선언의 차이가 남아 있다. |
| Flow 문법 범위 | React의 Flow 파일에서 함수 누락·범위 차이를 확인했다. `ReactFlightServerConfigDebugNoop.js:15`와 `getComponentNameFromType.js:57`을 보류 자료에 보존한다. 일반 JavaScript의 정확도 문제와 지원하지 않는 확장 문법을 구분해야 한다. |
| 마지막 줄 표시 | Scala 3 표본 여섯 파일과 OpenSSL `test/radix/quic_ops.c:1870`에서 선언의 끝 줄이 실제 파일 줄 수보다 1 크게 표시됐다. 줄바꿈 뒤의 배타적 파서 끝 좌표를 표시 범위와 혼동한 것으로 보이며, 원래 실패를 그대로 유지한다. |
| 검색 오류 | Gradle의 두 긴 식별자로만 구성된 검색 질의가 `Only excluding terms given` 오류를 반환했다. 원문 질의는 결과 JSON에 보존했다. 다른 자동 질의의 목표 파일 미노출은 검색 품질·출력 한도·대조기 이름을 추가 검토할 항목이다. |
| 입력 인코딩 | glibc `sysdeps/i386/fpu/e_atanhl.S`는 UTF-8이 아닌 바이트가 있어 `overview`에서 색인되지 않았다고 응답했다. `read`의 손실 허용 디코딩과 색인의 입력 범위 차이다. |
| 대조기 오분류 | Ctags가 Java record/일부 C# primary constructor 헤더를 메서드로 분류하거나 Lua의 `~=`를 `~` 함수로 보고했다. C# 명시적 인터페이스 이름 표기, JS/TS 함수 표현식·화살표 함수, 중첩 선언도 대조 범위를 맞춰야 한다. 보류 전체를 제품 버그 수로 계산하지 않는다. |

작은 대조의 통과율은 언어 전체 정확도가 아니다. 특히 외부 import, 타입 추론, 오버로드, 가상 호출, monkey patching, Lua 테이블의 변경, 언어별 매크로·새 문법은 이번 검사로 보장하지 않는다. SQL·Bash·Zsh는 호출 관계를 지원하지 않는 현재 경계를 확인했고, ASM은 같은 파일의 호출 명령과 label을 대조했으며 함수 전체 범위를 추정하지 않았다.

## 다시 실행하기

명령의 작업 디렉터리는 모노레포 루트다. 공개 저장소 이력과 파서 의존성 다운로드에는 네트워크·디스크 공간이 필요하다. 새 실행 ID 또는 새 출력 디렉터리를 사용한다. 완료되지 않은 작업 트리와 색인을 동시에 사용하면 안 된다.

```sh
# 선택한 고정 저장소의 줄 수와 커밋 수 측정
python3 apps/codemap-search/scripts/qualify_development_languages.py \
  --cache /Users/buyong/tmp/codemap-public-validation --jobs 2

# Dart·Scala·Groovy 독립 파서 도우미 준비
python3 apps/codemap-search/scripts/prepare_validation_oracles.py \
  --cache /Users/buyong/tmp/codemap-public-validation

# Rust·Go 외 전체 언어: 새 작업 트리와 색인으로 실행
python3 apps/codemap-search/scripts/validate_development_languages.py \
  --cache /Users/buyong/tmp/codemap-public-validation \
  --binary /Users/buyong/.local/bin/codemap-search \
  --run-id development-quality-next --jobs 2

# 특정 언어·저장소만 재검증
python3 apps/codemap-search/scripts/validate_development_languages.py \
  --cache /Users/buyong/tmp/codemap-public-validation \
  --binary /Users/buyong/.local/bin/codemap-search \
  --run-id flutter-quality-next --language dart --repository flutter/flutter

# 큰 저장소 없이 호출·제외·상수 문맥 검사
python3 apps/codemap-search/scripts/probe_development_languages.py \
  --binary /Users/buyong/.local/bin/codemap-search \
  --output /Users/buyong/tmp/codemap-public-validation/runs/local-quality-next
```

실제 전체 실행의 바이너리·설정·소스·파서 해시, 원시 `mcp.jsonl`, 오류 로그, 파일별 독립 파서 결과는 `/Users/buyong/tmp/codemap-public-validation/runs/`에 보존한다. 중단된 검사도 실행기가 `partial-results.json`에 완료한 항목과 실패 요청을 남긴다. 이 보존 개선 전의 기준선은 원시 요청·응답과 최상위 오류 기록을 사용한다.

`cm read`·`cm grep`과 CLI `parse`·`codemap`·`search`로 직접 비교하는 명령은 [품질 확인 명령](development-language-commands.ko.md)에 정리했다.
