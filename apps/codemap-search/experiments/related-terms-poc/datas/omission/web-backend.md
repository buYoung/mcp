# HTTP servers and APIs 누락 단어

- 분야 id: `web-backend`
- 원본: [../web-backend.md](../web-backend.md)
- 누락 단어 27개 · 사전에 없음 11 · 뜻 미배정 0 · 다른 분야에만 있음 16
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Chunked file uploads (`t1017`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Multer` | proper_noun | 다른 분야에만 있음: browser-platform — Multipart upload requests | Express에서 multipart 파일 업로드를 처리하는 미들웨어 이름이다. |
| `multipart` | word | 다른 분야에만 있음: browser-platform — Multipart upload requests | 업로드 요청 본문을 파싱하는 서버 코드에서 형식 이름으로 쓰인다. |
| `multipart/form-data` | identifier | 다른 분야에만 있음: browser-platform — Multipart upload requests | 서버가 업로드 요청의 Content-Type을 검사하고 파싱하는 코드에 나온다. |

### HTTP client requests (`t282.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ky` | proper_noun | 사전에 없음 | fetch 기반 작은 JavaScript HTTP 클라이언트로 호출 코드에 쓰인다. |

### HTTP headers (`t88.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Content-Disposition` | identifier | 다른 분야에만 있음: browser-platform — Blob file downloads | 서버가 다운로드 파일 이름과 표시 방식을 지정하려고 설정하는 응답 헤더다. |

### HTTP middleware (`t829`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `CORS` | abbreviation | 다른 분야에만 있음: browser-platform — Browser cross-origin policies | 서버 미들웨어와 설정에서 교차 출처 허용 헤더를 다루는 핵심 항목으로 널리 쓰인다. |

### HTTP request handlers (`t88.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `FormValue` | identifier | 사전에 없음 | Go 핸들러가 폼이나 쿼리 값을 읽는 Request 메서드다. |
| `ParseForm` | identifier | 사전에 없음 | Go 핸들러가 폼 데이터를 파싱하는 Request 메서드다. |

### HTTP response handling (`t456.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `too many requests` | phrase | 다른 분야에만 있음: distributed-systems — Request rate limits | 429 상태 코드의 이름으로 요청 한도 응답 코드에 쓰인다. |

### Python web serving (`t933`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `QueryDict` | identifier | 사전에 없음 | Django의 요청 파라미터 모음 타입으로 뷰 코드에 나온다. |

### REST API interfaces (`t412.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `OpenAPI` | proper_noun | 다른 분야에만 있음: docs-collaboration — API schema documentation | REST API 명세 표준으로 명세 파일과 코드 생성 설정에 나온다. |
| `Swagger` | proper_noun | 다른 분야에만 있음: docs-collaboration — API schema documentation | OpenAPI 명세 도구 모음으로 문서와 어노테이션에 나온다. |

### Route parameter handling (`t76.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `reverse url` | phrase | 사전에 없음 | 경로 이름에서 URL을 거꾸로 만들어 내는 동작을 가리킨다. |

### RPC service protocols (`t636`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `tRPC` | proper_noun | 다른 분야에만 있음: os-shell — Remote procedure calls | TypeScript 타입 공유 RPC 프레임워크로 서버 라우터 코드에 나온다. |
| `XML-RPC` | proper_noun | 다른 분야에만 있음: os-shell — Remote procedure calls | XML 기반 원격 호출 프로토콜로 오래된 API 코드에 나온다. |

### Server route handlers (`t89.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `AdonisJS` | proper_noun | 사전에 없음 | Node.js 풀스택 웹 프레임워크다. |
| `Javalin` | proper_noun | 사전에 없음 | Java와 Kotlin 경량 웹 프레임워크로 라우트 등록 코드에 쓰인다. |
| `Restify` | proper_noun | 사전에 없음 | REST API 서버용 Node.js 프레임워크다. |
| `route group` | phrase | 사전에 없음 | 공통 접두사나 미들웨어로 경로를 묶는 라우터 기능이다. |

### Typed Python APIs (`t801`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Depends` | identifier | 다른 분야에만 있음: frontend-ui — Dependency relationships | FastAPI 의존성 주입 선언 함수로 핸들러 인자에 쓰이며 이 분야 고유 의미가 있다. |

## 새 주제가 필요한 단어

### Conditional requests

범위: Validating cached or concurrent resource state through ETag, Last-Modified and If-* request headers.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `conditional request` | phrase | 다른 분야에만 있음: databases-storage — HTTP cache validation | 조건 헤더로 응답이나 갱신 여부를 정하는 HTTP 요청을 가리킨다. |
| `ETag` | identifier | 다른 분야에만 있음: databases-storage — HTTP cache validation | 리소스 버전 식별 헤더로 조건부 요청과 캐시 검증 코드에 나온다. |
| `Last-Modified` | identifier | 다른 분야에만 있음: databases-storage — HTTP cache validation | 수정 시각 응답 헤더로 조건부 요청 코드에 나온다. |
| `not modified` | phrase | 다른 분야에만 있음: databases-storage — HTTP cache validation | 304 상태 코드의 이름으로 조건부 요청 응답에 쓰인다. |

### HTTP redirects

범위: Redirecting clients to another URL with 3xx status codes, Location headers and framework redirect helpers.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `redirect` | word | 다른 분야에만 있음: frontend-ui — HTTP redirects | 서버가 클라이언트를 다른 URL로 보내는 응답과 헬퍼 함수 이름으로 널리 쓰인다. |
| `trailing slash redirect` | phrase | 사전에 없음 | 끝 슬래시 유무에 따라 다른 경로로 보내는 라우터 동작이다. |

### Ruby and PHP web serving

범위: Serving Ruby on Rails, Rack and PHP applications through Puma, Passenger and PHP-FPM.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `resource routing` | phrase | 사전에 없음 | Rails에서 REST 경로 묶음을 한 번에 정의하는 라우팅 방식이다. |
