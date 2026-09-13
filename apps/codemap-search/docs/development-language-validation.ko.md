# 개발 언어 전체 공개 저장소 검증

Rust·Go 검증을 포함해 25개 개발 언어, 50개 언어·저장소 조합을 대상으로 검증을 확대했다. 저장소는 49개다. PostgreSQL을 C와 SQL에 각각 사용했다. 모든 대상의 커밋 수는 1,000개 이상이며, 사용자가 승인한 코드 규모 예외는 10개 조합에 적용했다.

호출·상수 문맥과 제외 규칙 개선에 이어 Groovy·Zsh·Kotlin 파서, C++ 변환 연산자, 파일명 검색과 PowerShell 문맥 준비를 수정했다. C/C++·ASM에는 선택적으로 켜는 Clang/NASM 매크로 확장을 추가했다. 비 UTF-8 소스는 색인하지 않고 제외 이유를 표시한다. 공개 검증의 실패 4건과 선언 대조 보류 216건은 아래에 구분해 보존한다. 사용자가 선택한 대로 **Flow 지원은 별도 작업으로 분리**한다. 이 문서는 통과한 검사와 남은 실패·판정 보류를 구분하며, 언어 전체의 무오류 판정이나 전수 코드 검토 결과가 아니다.

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

확대 대상은 저장소마다 코드가 있는 적격 파일을 최대 20개 선정했다. 표본 순서는 `SHA-256("codemap-languages-v1:<저장소>:<경로>")`로 고정한다. Git 제외 파일, 1 MiB 초과 파일은 표본에서 뺀다. Zinit은 적격 파일 9개를 모두 선정했다. 초기 파서 무한 처리는 제한 시간 도입 후 스캐너 자체를 수정했다. 최종 공개 재실행에서는 두 설정 모두 완료했지만, 지원하지 않는 문법과 선언 대조 보류는 남는다.

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

Dart의 getter·setter는 독립 analyzer의 `isGetter`·`isSetter`로 `property`를 구분한다. C# 대조에서는 Ctags의 `operator ==`와 명시적 인터페이스 접두어를 제품의 이름 표기와 맞추되 선언 위치와 끝 줄 조건은 유지하고 원래 이름을 보존한다.

대조 파서는 제품의 Tree-sitter 결과를 복사하지 않는다. 다만 Ctags는 전체 언어 컴파일러가 아니고, 컴포넌트 대조기도 템플릿 전체의 정답기가 아니다. 타입 검사·런타임 동적 바인딩을 전수 검증하지 않는다. 공개 저장소 전체 검증은 매크로 확장을 끈 설정이며, 새 매크로 기능은 별도 Clang/NASM 대조와 실제 FFmpeg 헤더로 확인했다. 저장소별 빌드 설정을 모두 준비해 매크로 확장을 켠 전수 검증은 아니다. 공개 저장소의 빌드·테스트·프로젝트 스크립트는 실행하지 않았다. Groovy는 프로젝트 변환을 실행하는 `CompilationUnit`을 사용하지 않으며, 파서 도우미 의존성은 제품 런타임과 분리한 캐시에 준비했다.

## 실행 결과

기계가 읽을 수 있는 집계는 [development-results.json](../validation/development-results.json), 수정·보류·지원 한계의 분류는 [development-issues.json](../validation/development-issues.json)에 있다. 확대 대상의 표본 909개를 두 설정으로 대조했다. 전체 92개 설정을 새 작업 트리와 색인으로 다시 실행했고, 마지막 Groovy 수정 후 해당 네 설정을 다시 대조해 아래 집계에 반영했다. **최종 설치 바이너리 하나로 모든 공개 저장소를 재실행한 표는 아니다.**

| 확대 언어 | 완료 설정 / 예정 설정 | 통과 | 실패 | 보류 |
| --- | ---: | ---: | ---: | ---: |
| Python | 4 / 4 | 246 | 0 | 0 |
| TypeScript | 4 / 4 | 244 | 0 | 10 |
| JavaScript | 4 / 4 | 248 | 0 | 6 |
| Java | 4 / 4 | 252 | 0 | 2 |
| C# | 4 / 4 | 242 | 0 | 12 |
| PHP | 4 / 4 | 248 | 0 | 2 |
| Ruby | 4 / 4 | 248 | 0 | 0 |
| Lua | 4 / 4 | 236 | 0 | 12 |
| Kotlin | 4 / 4 | 238 | 2 | 12 |
| Swift | 4 / 4 | 248 | 0 | 8 |
| Dart | 4 / 4 | 254 | 0 | 2 |
| Scala | 4 / 4 | 246 | 0 | 8 |
| Groovy | 4 / 4 | 254 | 0 | 0 |
| PowerShell | 4 / 4 | 248 | 0 | 0 |
| C | 4 / 4 | 234 | 0 | 22 |
| C++ | 4 / 4 | 232 | 0 | 24 |
| ASM | 4 / 4 | 178 | 2 | 64 |
| SQL | 4 / 4 | 234 | 0 | 0 |
| Bash | 4 / 4 | 238 | 0 | 0 |
| Zsh | 4 / 4 | 180 | 0 | 16 |
| Vue | 4 / 4 | 228 | 0 | 16 |
| Astro | 4 / 4 | 240 | 0 | 0 |
| Svelte | 4 / 4 | 244 | 0 | 0 |
| **합계** | **92 / 92** | **5460** | **4** | **216** |

Rust·Go의 네 저장소 × 두 설정은 **470/470**, 별도 회귀는 **12/12** 통과했다. 위 확대 표와 별개다. 전체 공개 실행의 실행 오류는 0이며, 실패·보류가 있으므로 검증 명령의 종료 코드는 1이다. Groovy만 마지막으로 재실행한 결과는 **254 통과·0 실패·0 보류**다.

최종 설치본으로 실행한 25개 언어의 작은 대조는 **522/522 통과**했다. 상수 문맥 26개도 모두 통과했다. 프로젝트 검증은 라이브러리 **208개**, 추출 스냅샷 **21개**, 파일 형식 **12개**, 탐색 fixture **1개**가 통과했다. 전체 e2e 실행에서는 168개가 통과하고 설정 스키마의 이전 버전 9를 기대하던 3개가 실패했다. 기대값을 현재 버전 10에 맞춘 후 관련 제외 검사 4개를 모두 다시 통과했으며, NASM 변경 후 매크로 e2e 1개를 별도로 통과했다. 마지막 입력 크기 제한 수정 후에는 매크로 단위 검사 6개를 다시 통과했다. 이 결과를 최종 소스에서 전체 e2e를 한 번에 통과한 결과로 표현하지 않는다.

`cargo check`, 전체 target Clippy(`-D warnings`), release 빌드, 소스 패키징과 패키지를 풀어 수행한 오프라인 `cargo check`가 통과했다. 설치한 `cm read`·`cm grep`에서 설정 함수의 호출 정의·상수, C 생성 함수, 매크로가 붙인 `static`, FFmpeg NASM 생성 label을 확인했다. 이 공개 저장소 검증에 사용한 설치 바이너리 SHA-256은 `b1fd0c0b97dd14ccccacc526419b9b47d011b8c9f3dfafd96313de57ed840bb1`이다.

| 실행 | 범위 | 바이너리 SHA-256 앞 12자리 |
| --- | --- | --- |
| `verify-20260913T140647072947-93020-rust-go` | Rust·Go 네 저장소, 두 설정과 회귀 | `1320b6ae1f97` |
| `verify-20260913T140647072947-93020-languages` | 확대 대상 92개 설정: 5450 통과·4 실패·226 보류 | `1320b6ae1f97` |
| `remaining-groovy-checked-20260913` | Groovy 네 설정을 최종 집계에서 교체: 254 통과 | `2b6ddb2cdea2` |
| `verify-20260913T152110253688-22358` | 최종 설치본 25개 언어 작은 대조: 522 통과 | `b1fd0c0b97dd` |

첫 기준선 **90/92 설정, 5140통과·46실패·356보류**는 JSON의 `improvement_baseline`에, 직전 커밋 `16192bc8a`의 **92/92 설정, 5276통과·30실패·324보류**는 `remaining_work_baseline`에 보존했다. 대조기 개선으로 검사 수와 보류가 늘어난 언어도 있으므로 단순 통과율을 언어 정확도로 해석하지 않는다. Groovy 내부 생성자의 `$` 한정 이름은 원문 선언 줄이 짧은 생성자 이름과 일치할 때만 비교용으로 정규화하며, 원래 대조기 이름은 유지한다.

각 설정에는 실행 ID·바이너리·원시 결과·MCP 요청/응답의 SHA-256이 있다. `pass`는 해당 검사 조건 통과, `fail`은 조건 불충족, `unverified`는 선언 누락·범위 차이에 대한 판정 보류다. 최초 실행기의 Ctags 인자·BOM·검색 범위·컴포넌트 검사 보정 이력과 수정 전 응답도 원래 실행 디렉터리에 남긴다. 이번 집계에서 이전 실패를 근거 없이 성공으로 바꾸지 않았다.

## 수정한 문제

- **호출 정의 오연결:** Rust·Go의 원문 기반 모듈 판정을 유지하고, 나머지 개발 언어는 같은 파일의 유효 범위와 명시적인 상대 JS/TS import를 확인한다. 매개변수·지역 바인딩·클래스 범위가 다른 동명 후보, 확정되지 않은 수신 객체는 정의 링크 없이 `unresolved`로 표시한다. 일반적인 프로젝트 전체 타입 추론을 추가한 것은 아니다.
- **Vue·Astro·Svelte:** 확인한 script/frontmatter 구간의 실제 구문 트리를 사용한다. 본문의 `target()` 문자열과 서로 다른 script 블록을 같은 호출 범위로 합치지 않는다.
- **PowerShell:** 함수가 `unknown`으로 나오던 query capture를 수정하고, 전역 변수의 범위를 개별 대입문으로 제한했다. 정적인 멤버 호출 이름은 수집하되 동적인 멤버 이름은 확정하지 않는다.
- **Dart:** 메서드 signature만 색인하던 query를 본문·생성자 초기화 목록을 포함한 선언으로 바꿨다. Flutter·Dart SDK 재검증에서 매칭된 선언의 끝 줄 불일치는 없어졌다. 이번에는 getter·setter·연산자와 external 함수 선언도 추가했다. 이번 공개 재실행에서는 Flutter와 Dart SDK 모두 두 설정을 완료했다. Dart의 선언 대조 보류 2건은 별도 기록에 남긴다.
- **ClickHouse 색인 종료:** 공유 타입 선언 탐색에서 깊은 구문 트리를 재귀 순회하다 stack overflow가 발생했다. 반복 순회와 파일당 타입 선언 조회표로 바꿨다. 실제 ClickHouse 두 설정에서 각각 58항목을 통과했다. 저장소 전체 색인 중 발생한 오류이며 SQL 문법 자체가 원인이라고 단정하지 않는다.
- **Dart SDK 색인 지연:** 참조마다 조상 노드를 반복 탐색하던 경로를 파일당 한 번의 범위 조회표로 바꿨다. 구조 설정이 1,200초 제한을 넘기던 저장소가 수정 후 약 62초에 준비됐다. 기존 중첩 함수 범위는 회귀 검사로 대조했다.
- **PowerShell 반복 요청 지연:** 테스트 영역 캐시가 1,024개에서 전체 초기화돼 다음 요청마다 다시 파싱하던 문제를 수정했다. 현재 색인 파일의 캐시를 유지하고 삭제된 파일은 제거하며, 색인 밖 파일은 추가 1,024개 범위 안에서 보관한다. 같은 MCP 프로세스의 문제 `grep`은 180초 초과에서 약 3.7초로 줄었다. 이번에는 언어별로 무관한 후보를 먼저 제외하고, 심볼의 테스트 영역 판정을 필요한 시점에 수행한다. 전체 공개 재실행에서 일반 코드의 첫 문맥은 default **3.572초**, structural **3.746초**로 관측됐다. 이전 설치본은 각각 25.588·39.071초였다. 제외된 테스트 파일의 첫 `read`는 약 2.8 ms다. 새 색인 준비 시간은 각각 **480.157·558.842초**이며 요청 시간과 구분해야 한다.

- **11개 언어의 상수 문맥:** Java·C#·PHP·Ruby·Kotlin·Swift·Dart·Scala·Groovy·C·C++의 구문상 상수·불변 바인딩을 확인해 참조 목록에 정의 위치와 값을 표시한다. C#/Kotlin 선언의 중간 주석은 초기값에서 제외하고, 문자열 안 공백은 보존한다. 다른 객체·namespace·동명 바인딩으로 연결하지 않는다. PHP 클래스 상수·비블록 namespace, C/C++ 포인터 constness는 추정하지 않는다.
- **선언 누락:** Avalonia의 연산자, Swift 연산자, Tarantool의 테이블 필드 대입 함수를 수집한다. Lua 다중 대입은 각각의 우변 함수와 좌변 이름을 짝지으며 계산된 인덱스는 건너뛴다.
- **검색과 문맥 출력:** camelCase를 분리하기 전에 소문자화해 긴 식별자가 검색 오류를 일으키던 문제를 고쳤다. 파일명을 직접 지정한 테스트 파일은 일괄 감점하지 않는다. 파일명·확장자 없는 이름의 정확한 후보를 원래 질의 조건 안에서 보완하고 경로 관련도를 반영한다. ASM 명령 이름은 검색용 참조로만 수집하며 함수 선언이나 호출로 만들지 않는다. 클래스 요약에 포함된 짧은 메서드가 중복으로 제거돼 호출 문맥까지 사라지던 문제도 수정했다.
- **끝 줄 표시:** Scala 3 여섯 표본과 OpenSSL 매크로의 파일 끝 좌표를 확인했다. 배타적인 내부 끝 좌표를 유지하면서 출력과 줄 단위 포함 판정에서만 실제 마지막 줄을 사용한다.
- **Groovy:** 정적인 따옴표 메서드, 줄바꿈으로 끝나는 필드·생성자 위임, 괄호 없는 호출·직접 필드 접근·trait 문법을 수정했다. 동적 문자열 이름은 확정하지 않는다. Gradle·Apache Groovy의 최종 공개 대조는 보류 없이 통과했다.
- **Zsh:** 문자 집합 안 괄호의 스캐너 처리, 길이 0인 토큰, 깊이 카운터를 수정해 최소 입력 `c=${x//[^)]}`의 무한 처리를 해소했다. 콜론 함수 이름과 일부 리다이렉션·축약 조건문도 처리한다. 지원하지 않는 복합 문법의 오류 복구 노드를 선언으로 확정하지 않는다.
- **Kotlin·C++·선언 범위:** Kotlin 애너테이션 함수가 중위 표현식으로 해석되던 문제와 C++ 변환 연산자 누락을 수정했다. Scala·Groovy 선언의 마지막 코드 줄을 사용해 뒤쪽 주석·빈 줄이 범위에 섞이지 않게 했다.
- **비 UTF-8 소스:** 사용자가 선택한 UTF-8 전용 색인 정책을 유지하면서 최초 오류 바이트와 제외 이유를 표시한다. 이전에 색인됐던 파일이 비 UTF-8로 바뀌면 오래된 심볼도 제거한다. `read`의 대체 문자와 원본 바이트는 유지한다.
- **매크로 확장:** [clangd의 컴파일 설정 모델](https://clangd.llvm.org/design/compile-commands)을 참고해 `compile_commands.json`·`compile_flags.txt`·사용자 플래그에서 전처리 문맥을 얻는다. Clang의 전처리 줄 표식과 NASM의 확장 목록을 이용해 생성 선언을 원본 줄에 연결한다. FFmpeg `x86inc.asm`의 `cglobal`이 만든 `ff_cm_public_probe`가 호출한 파일의 3줄에 연결되는 것을 설치한 `cm`으로 확인했다. 매크로가 추가한 `static`도 반영한다. 설정·헤더 변경은 재색인에 반영하며, 실패 시 이유를 표시한다. 자세한 키와 범위는 [설정 안내](configuration.ko.md#매크로-확장)에 있다.
- **설정 전환:** TOML 문자열 안의 `[section]`·버전 주석을 실제 설정으로 오인하지 않게 했다. 사용자 문자열과 주석을 보존하면서 버전 10의 비활성 매크로 설정 안내를 추가한다.

추출 형식은 `v27-native-declarations-and-groovy`, 설정 스키마는 10이다. 이전 색인은 한 번 다시 추출한다. 매크로 설정은 기본 비활성이며, 파싱 결과에는 확장 상태·입력 파일 메타데이터가 추가된다. Groovy·Zsh·Kotlin 수정 파서의 소스·패치·라이선스·재생성 방법은 [vendor 안내](../vendor/README.md)에 있다.

시간은 이 macOS arm64 환경에서 관측한 값이다. OS 캐시와 CPU 부하를 격리하지 않았고 재검증의 작업 트리·색인 재사용 여부도 실행별로 다르다. 일반적인 성능 배수로 해석하지 않으며, 각 설정의 `ready_seconds`와 요청별 시간을 원시 자료에 보존한다.

## 남은 판정 보류와 지원 범위

| 분류 | 재현 근거와 현재 상태 |
| --- | --- |
| 검색 실패 2건: Kotlin 복합 입력 | `native/native.tests/testData/standalone/console/fprintf.kt`는 `// FILE:`로 C 헤더와 Kotlin 파일을 한 파일에 합친 컴파일러 테스트 입력이다. 유효한 단일 Kotlin 파일이 아니며, `main fprintf` 질의의 두 설정 실패를 유지한다. |
| 검색 실패 2건: glibc 동명 파일 | `ENTRY ____longjmp_chk`는 같은 이름의 추적 ASM 파일 16개와 다른 `ENTRY` 후보가 경쟁한다. 대상 SPARC 파일은 출력 한도 밖이다. 색인 준비 후 `ENTRY sysdeps/unix/sysv/linux/sparc/sparc64/____longjmp_chk.S` 질의로 해당 파일이 나오는 것을 확인했다. 원래 넓은 질의의 실패는 삭제하지 않는다. |
| 선언 대조 216건 보류 | 모든 사례를 제품 결함이나 대조기 문제로 확정하지 않았다. Ctags의 매크로 호출·타입 헤더 오분류와 유효하지 않은 컴파일러 테스트 입력이 포함된다. 원시 누락 목록·범위 차이는 JSON에 보존한다. |
| JS/TS의 함수 표현식 | 블록·함수 안 지역 변수 선언은 현재 모듈 변수 수집 범위에서 제외된다. 객체 속성의 화살표 함수와 prototype에 대입한 함수도 일반 함수 선언과 동일하게 지원하지 않는다. 이름 있는 선언 대조와 이 정책 차이를 구분해야 한다. |
| Zsh 나머지 문법 | 무한 처리는 수정했지만 모든 축약 조건문을 지원하지 않는다. 공개 재실행의 보류는 Oh My Zsh 3개·Zinit 5개씩 두 설정, 총 16건이다. 오류 복구로 만들어진 선언은 신뢰하지 않는다. |
| Flow 지원 분리 | React의 `ReactFlightServerConfigDebugNoop.js`·`getComponentNameFromType.js` 등 확장 문법은 사용자 결정에 따라 분리했다. 일반 JavaScript 통과로 합치지 않는다. |
| 매크로 확장 경계 | 빌드 설정·설치된 도구가 필요하다. clangd를 실행하거나 `.clangd`를 읽지 않는다. 헤더의 임의 컴파일 명령 추정, 모든 컴파일러 옵션, 확장 토큰의 정확한 열 좌표는 지원하지 않는다. 전처리 파일의 호출·상수 참조 연결은 `unresolved`로 표시한다. GNU assembler `.macro`와 Windows/Linux 실행은 확인하지 않았다. |
| 큰 저장소의 비용 | PowerShell 일반 문맥은 이번 환경에서 약 3.6~3.7초지만 처음부터 만든 전체 색인은 약 8~9분이었다. 매크로 확장을 켜면 파일별 외부 도구 실행 비용도 추가된다. |

작은 대조의 통과율은 언어 전체 정확도가 아니다. 외부 import, 프로젝트 전체 타입 추론, 오버로드·가상 호출, 동적 바인딩을 보장하지 않는다. SQL·Bash·Zsh의 호출 관계 미지원 경계는 유지한다. ASM은 원본 label·호출 명령, 매크로 모드에서는 네이티브 도구가 검증한 label·global 선언을 다루며 함수 전체 범위를 추정하지 않는다.

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

이후 실시간 출력·Rust 연결·이벤트 탐색 브리프의 설치본과 검증 기록은 [통합 결과](../../../docs/briefs/evidence/codemap-nav/integration.json)에 분리했다. 위 공개 저장소 결과를 이후 바이너리의 전체 재검증 결과로 해석하지 않는다.
