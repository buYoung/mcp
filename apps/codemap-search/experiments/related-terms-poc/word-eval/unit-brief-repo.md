# 코드 식별자를 개념 단위로 나누고, 분류 가능 여부 판정하기

{repo}({description}) 저장소에 나오는 식별자 목록을 받습니다. 언어: {languages}. 식별자마다 의미 단위로 나누고, 각 단위가 개발 분야의 개념으로 **분류 가능한지** 판정해 주세요.

## 입력 항목
- `name`: 식별자
- `where`: `defined`(이 저장소가 정의), `used_only`(쓰기만 함. 표준·외부 라이브러리 이름 등)
- `kinds`: 정의 종류별 개수(function, method, class, variable, parameter, field, struct ...)
- `qualifiers`: `http.StatusOK`의 `http`처럼 앞에 붙은 패키지·모듈·수신자·객체
- `count`, `examples`: 등장 횟수와 실제 코드 줄 예시. 의미는 이 쓰임을 보고 판단합니다.

## 나누는 규칙
(아래 예시는 Go 웹 프레임워크 코드에서 가져온 것입니다. 이 저장소 언어의 이름 관례에 맞게 적용합니다.)
1. 식별자를 빠짐없이 단위로 나눕니다. 단위를 이어 붙이면 식별자 전체가 됩니다(대소문자·구분자 차이는 무시).
2. 대문자 연속이나 숫자 때문에 기계적으로 잘못 나뉘는 것을 바로잡습니다.
   예: `MIMEJSON` → `mime` + `json`, `IPv6` → `ipv6`, `int64` → `int64`, `RFC3339` → `rfc3339`, `utf8` → `utf8`
3. 여러 단어가 함께 하나의 개념을 가리키면 하나의 문구 단위로 둡니다.
   예: `RemoteIPHeaders` → `remote ip` + `headers`, `BasicAuthForRealm` → `basic auth` + `for` + `realm`, `ContentType` → `content type`
   단어가 각자 따로 의미를 가지면 나눕니다: `ShouldBindBodyWith` → `should` + `bind` + `body` + `with`
4. 약어는 그대로 단위로 두고 `expansion`에 원래 말을 적습니다. 예: `req` → request, `ctx` → context, `tsr` → trailing slash redirect

## 분류 가능 판정
- **분류 가능(true)**: 이 코드에서의 쓰임으로 보아, 아래 개발 분야 중 하나에서 특정한 기술적 의미를 가집니다.
  예: `bind`(요청 데이터 바인딩), `render`, `redirect`, `hijacker`(HTTP 연결 가로채기), `json`, `mime`, `ssv`(공백 구분 값),
  `teapot`(HTTP 418 상태), `get`(`MethodGet`, `router.GET`처럼 HTTP 메서드로 쓰인 경우), `err`(오류)
- **분류 불가(false)**: 그 쓰임에서 개발 개념을 가리키지 않습니다.
  - 일반 동사·조동사·전치사·접두사: `should`, `has`, `is`, `must`, `with`, `for`, `to`,
    접근자 접두사로 쓰인 `get`/`set`(`GetHeader`의 get)
  - 관례만 있는 한 글자 이름(`c`, `w`, `t`, `i`), 의미 없는 자리표시 이름(`foo`, `bar`, `hoge`)
  - 일반 영어 단어로만 읽히고 개발 분야에 넣을 수 없는 것
- 같은 단어도 쓰임에 따라 판정이 달라집니다. 이 식별자 안에서의 쓰임으로 판정합니다.
- 애매하면 `note`에 이유를 적습니다.

## 분야 (`field` 값, 분류 가능일 때 하나 고름)
- `algorithms-data-structures` — Algorithms and data structures
- `auth-identity` — Authentication, identity and access
- `browser-platform` — Browser and web platform
- `build-release` — Build, packaging and release
- `business-domains` — Business and product domains
- `cloud-infrastructure` — Cloud, containers and infrastructure
- `concurrency-async` — Concurrency and async
- `configuration` — Configuration and environments
- `data-engineering` — Data engineering and analytics
- `data-formats` — Data formats and serialization
- `data-visualization` — Charts and data visualization
- `databases-storage` — Databases, caching and search
- `distributed-systems` — Distributed systems and messaging
- `docs-collaboration` — Documentation and project collaboration
- `editors-ide` — Editors, IDEs and code intelligence
- `error-handling` — Errors and exception handling
- `files-io` — Files and I/O
- `frontend-ui` — Frontend frameworks and UI components
- `functional-programming` — Functional programming
- `graphics-media` — Graphics, images and media
- `language-constructs` — Core language constructs
- `languages-compilers` — Language ecosystems, compilers and runtimes
- `math-numerics` — Math, numbers and statistics
- `ml-ai` — Machine learning and LLMs
- `mobile-desktop` — Mobile and desktop apps
- `networking` — Networking and protocols
- `observability-debugging` — Observability, debugging and performance
- `os-shell` — Operating systems, processes and shell
- `security` — Security and cryptography
- `software-design` — Software design and architecture
- `systems-low-level` — Memory, hardware and low-level systems
- `testing` — Testing
- `text-processing` — Text and strings
- `time-locale` — Dates, time and localization
- `type-systems` — Type systems
- `version-control` — Version control and code review
- `web-backend` — HTTP servers and APIs
- `web-styling` — Styling and visual design

## 결과 형식
JSON 배열 하나를 결과 파일에 씁니다. 입력 항목 하나당 한 항목, 입력 순서대로:
```
{"name": "RemoteIPHeaders", "where": "defined",
 "units": [
   {"text": "remote ip", "categorizable": true, "field": "networking", "sense": "client IP address seen by the server"},
   {"text": "headers", "categorizable": true, "field": "web-backend", "sense": "HTTP request headers"}
 ]}
```
- `text`: 소문자, 문구는 단어 사이를 공백 하나로
- `categorizable`: true/false
- `field`: 분류 가능이면 위 분야 값 하나, 아니면 null
- `sense`: 분류 가능이면 이 코드에서의 뜻을 영어로 짧게(10단어 안팎), 아니면 null
- `expansion`, `note`: 해당할 때만

## 작업 규칙
- 배정된 입력 파일만 읽습니다. 의미 확인이 필요하면 예시에 적힌 {repo} 저장소 파일을 열어 봐도 됩니다(저장소 루트: `{checkouts}/{repo}`). 그 밖의 파일·디렉터리는 열지 않습니다.
- 판정은 직접 합니다. 사전·분류 데이터·다른 분석 결과를 찾아보지 않습니다.
- 입력 항목을 하나도 빠뜨리지 않습니다. 마지막에 입력 수와 결과 수가 같은지 확인합니다.
- 마지막 답변에는 항목 수와 판단이 특히 애매했던 경우만 짧게 적습니다.
