# Authentication, identity and access 누락 단어

- 분야 id: `auth-identity`
- 원본: [../auth-identity.md](../auth-identity.md)
- 누락 단어 24개 · 사전에 없음 19 · 뜻 미배정 0 · 다른 분야에만 있음 5
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Authorization checks (`t15.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Pundit` | proper_noun | 다른 분야에만 있음: languages-compilers — Ruby development ecosystem | Rails에서 정책 클래스 기반 인가를 구현하는 라이브러리다. |

### JSON Web Tokens (`t136`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `create_access_token` | identifier | 사전에 없음 | Flask-JWT-Extended 등에서 액세스 토큰을 만드는 함수다. |
| `get_jwt_identity` | identifier | 사전에 없음 | Flask-JWT-Extended에서 토큰 주체를 읽는 함수다. |
| `jwt_required` | identifier | 사전에 없음 | Flask-JWT-Extended의 JWT 필수 데코레이터다. |
| `simplejwt` | proper_noun | 사전에 없음 | Django REST framework의 JWT 인증 패키지(djangorestframework-simplejwt)다. |

### Login sessions (`t141.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `post logout redirect` | phrase | 사전에 없음 | 로그아웃 후 이동 주소 설정을 가리킨다. |

### OAuth access tokens (`t159.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `device flow` | phrase | 사전에 없음 | OAuth 디바이스 인가 플로우를 가리킨다. |

### Role assignments (`t15.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `admin` | word | 다른 분야에만 있음: security — Administrator roles | 관리자 역할·플래그를 가리키는 가장 흔한 단어다. |
| `administrator` | word | 다른 분야에만 있음: security — Administrator roles; security — Privilege escalation | 관리자 역할을 가리키는 단어다. |

### Single sign on (`t159.7`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apereo CAS` | proper_noun | 사전에 없음 | 오픈소스 SSO 서버이다. |
| `Facebook Login` | phrase | 사전에 없음 | Facebook 로그인 연동이다. |
| `ForgeRock` | proper_noun | 사전에 없음 | ID 플랫폼 제품이다. |
| `Gluu` | proper_noun | 사전에 없음 | 오픈소스 IdP 플랫폼이다. |
| `Sign in with Apple` | phrase | 사전에 없음 | Apple 로그인 연동이다. |
| `social auth` | phrase | 사전에 없음 | 소셜 인증 패키지와 기능에 쓰인다. |
| `Zitadel` | proper_noun | 사전에 없음 | 오픈소스 ID 관리 플랫폼이다. |

### Tenant organization isolation (`t813.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `org_id` | identifier | 다른 분야에만 있음: databases-storage — Account tenant identifiers | 조직 식별자 컬럼·필드 이름이다. |

### User accounts (`t1035`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `account linking` | phrase | 사전에 없음 | 여러 ID를 한 계정에 연결하는 기능이다. |

## 새 주제가 필요한 단어

### Authenticating reverse proxies

범위: Delegating authentication to a proxy or gateway through forward-auth subrequests and identity-aware proxies.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `nginx auth_request` | phrase | 사전에 없음 | nginx auth_request 모듈 설정이다. |

### Authentication frameworks and libraries

범위: Authentication libraries and framework plugins that implement login, sessions, and strategies in web applications.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Better Auth` | proper_noun | 사전에 없음 | TypeScript 인증 프레임워크다. |
| `Lucia` | proper_noun | 사전에 없음 | TypeScript 세션 인증 라이브러리다. |
| `Passport.js` | proper_noun | 다른 분야에만 있음: web-backend — HTTP middleware | Node.js 인증 미들웨어 Passport의 표기 변형이다. |
| `Socialite` | proper_noun | 사전에 없음 | Laravel 소셜 로그인 패키지다. |
| `SuperTokens` | proper_noun | 사전에 없음 | 오픈소스 인증 프레임워크다. |
