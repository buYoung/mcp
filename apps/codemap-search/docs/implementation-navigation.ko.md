# 추상 선언·구현 탐색

`read`·`grep`·MCP `search`는 관련 코드에 `## Implementations`를 자동으로 붙입니다. 별도 활성화 설정이나 요청 옵션은 없습니다. 정의 위치가 확인된 관계와 실행 대상의 확정 여부를 구분합니다.

| 출력 | 의미 |
| --- | --- |
| `[abstract declaration]` | 추상 메서드 또는 본문 없는 인터페이스·trait·protocol 요구사항 |
| `implementation candidate` | 선언된 상속·구현 관계와 메서드 서명으로 연결한 구현 위치 |
| `implements/overrides declaration` | 구현 메서드에서 거슬러 올라간 선언 위치 |
| `declaration reference` | 타입이 확인된 수신 객체로 이 선언을 참조하는 호출 위치 |
| `call declaration` | 호출 지점의 선언 위치. 실제로 실행될 구현을 확정했다는 뜻은 아님 |
| `runtime target: unresolved` | 수신 객체의 실행 시점 인스턴스까지 증명하지 못함 |
| `implementation: unresolved` | 현재 유효한 색인에서 증명된 구현이 없음. 프로그램에 구현이 없다는 뜻은 아님 |

## 적용 대상

| 언어 | 수집·연결하는 기본 문법 |
| --- | --- |
| TypeScript·JavaScript | 클래스 상속·메서드 재정의, TypeScript 추상 메서드·명시적 인터페이스 구현 |
| Java·C#·Kotlin·Scala·Groovy | 추상 클래스·인터페이스·trait의 명시적 상속 및 메서드 구현 |
| Swift | protocol 요구사항, 명시적 준수와 클래스·확장 메서드 |
| Dart·PHP | 추상 클래스·인터페이스와 명시적 상속·구현 |
| Python | 클래스 상속, `abc.abstractmethod`, 명시적으로 상속한 `typing.Protocol` |
| Ruby·PowerShell | 클래스 상속과 메서드 재정의 |
| C++ | 원본 소스의 가상·순수 가상 메서드 및 명시적 상속 |
| Rust | trait 요구사항과 해당 trait의 명시적 `impl`; inherent impl과 구분 |
| Go | 기본 인터페이스의 전체 메서드 집합과 수신 타입의 구현. 포인터가 필요한 경우 `*T method set` 표시 |
| Vue·Astro·Svelte | 기존 파서가 분리한 JavaScript·TypeScript 코드 영역 |

총 19개 적용 대상 프로필의 기본 문법을 회귀 검증합니다. 지원 개발 언어 중 C·Lua·ASM·SQL·Bash·Zsh에는 이 기능의 언어 내장 상속 계약을 만들지 않습니다. 문서·Docker 등 인프라 형식은 대상이 아닙니다. 컴포넌트의 마크업·문자열·주석도 관계를 만들지 않습니다.

## 자동 표시와 제외 규칙

- `read`·내용형 `grep`은 반환한 줄에 관련 선언·구현·호출이 있을 때 표시합니다. 타입 선언의 첫 줄은 해당 타입의 관련 메서드도 선택합니다. 관련 없는 코드에는 빈 구현 섹션을 붙이지 않습니다.
- `view="source"`·`view="definitions"`와 경로·개수형 `grep`에는 구현 관계를 붙이지 않습니다. 원문은 그대로 유지합니다.
- MCP `search`는 결과 파일과 선택된 `workspace_scope` 안에서 관계를 표시합니다. `caller_context=false`는 선언 참조·호출 문맥을 숨기고 선언↔구현 관계는 유지합니다. CLI `codemap-search search`의 파일 목록 출력과는 다릅니다.
- `[index.exclude]`의 디렉터리·Git 규칙과 `[output.context.exclude]`의 테스트 코드 규칙을 적용합니다. 상속을 증명한 중간 선언과 Go 전체 메서드 집합의 근거도 제외·최신성 검사를 통과해야 합니다.
- 파일 내용의 해시가 색인과 다르면 그 파일에 의존한 관계를 숨깁니다. 바뀐 파일의 재색인이 끝나야 새 관계가 나옵니다. 해시 확인은 요청당 최대 1,024개 파일·64 MiB, 파일당 8 MiB까지이며 초과한 근거는 연결하지 않습니다.
- 읽기·검색의 기존 바이트 한도를 따릅니다. 관계가 많으면 잘림 안내를 붙이며, 언어·파일·줄 범위를 좁혀 확인할 수 있습니다.

## 해석 범위

이 기능은 컴파일러나 실행 추적기가 아닙니다. 같은 이름만으로 전역 후보를 연결하지 않습니다. 같은 파일의 선언 범위, 직접 상대경로 import, 일부 언어의 같은 디렉터리·패키지 범위, Rust의 기존 모듈·별칭 해석으로 확인한 타입만 연결합니다. Rust 모듈 해석은 기존의 제한된 색인 소스 입력을 공유하며, 입력이 없으면 관계를 추측하지 않습니다.

다음은 미해결 또는 생략할 수 있습니다.

- 제네릭 인자 치환·특수화·제약, 커스텀 타입의 파일 간 서명 동등성, 모호한 오버로드.
- 기본 export·재export·와일드카드·경로 별칭 등 현재 타입 해석기가 증명하지 못한 import. Rust의 지원되는 모듈·별칭·재export 해석은 기존 해석기를 사용합니다.
- Go의 임베딩·타입 집합·제네릭 인터페이스, 빌드 태그·OS/아키텍처 파일 조건. 빈 인터페이스에 모든 타입을 나열하지 않습니다.
- 조건부 선언과 확인되지 않은 Rust `cfg`. Rust는 명시한 `analysis.target_os`를 사용하며 실행 컴퓨터의 OS로 추정하지 않습니다.
- C++ 전처리를 적용한 파일의 구현 관계. 기존 매크로 생성 선언 탐색은 유지하지만 확장 토큰의 타입 근거까지 원본에 매핑하지 않습니다.
- 동적 믹스인·구조적 타입의 암묵적 만족·기본 구현의 상속만으로 생기는 새 구현 후보, DI·팩토리·리플렉션에 따른 실제 실행 대상.
- 기존 언어 파서가 복구 노드로 처리한 문법. 예를 들어 일부 Kotlin 한 줄 선언은 파서 제약이 남아 있으며, 관계를 억지로 복원하지 않습니다.

조건을 증명하지 못한 경우에도 직접 읽은 원문과 기존 심볼 탐색은 사용할 수 있습니다. 파싱 JSON에는 `navigation.implementations`가 추가됩니다. 색인 저장 형식은 `v29-indexed-implementations`로 변경되며 이전 색인은 한 번 재구축합니다. 설정 스키마와 기존 요청 인자는 바꾸지 않습니다.

## 반복 검증

패키지 디렉터리에서 실행합니다. 언어별 문법·동명 타입·서명·문자열 대조와 실제 MCP의 출력·제외·재시작·편집 동작을 검증합니다.

```sh
cd /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search
cargo test --locked --lib implementations::tests::
cargo test --locked --test e2e_tests test_implementation_
```

구현 근거는 `src/implementations/`, 파서 통합은 `src/parser/mod.rs`, 최종 소비자는 `src/tools/live_symbols.rs`와 `src/tools/search/mod.rs`입니다. 실제 CLI 확인 명령은 [개발 언어 품질 확인 명령](development-language-commands.ko.md#추상-선언과-구현-후보)을 참고하세요.
