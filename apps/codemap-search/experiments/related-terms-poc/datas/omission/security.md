# Security and cryptography 누락 단어

- 분야 id: `security`
- 원본: [../security.md](../security.md)
- 누락 단어 29개 · 사전에 없음 22 · 뜻 미배정 0 · 다른 분야에만 있음 7
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Administrator roles (`t44.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `privileged account` | phrase | 사전에 없음 | 관리자 등 높은 권한을 가진 계정을 가리키는 보안 점검과 정책 문서에 쓰입니다. |

### CodeQL queries (`t768.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `qlpack` | identifier | 사전에 없음 | CodeQL 쿼리 묶음 정의 파일 이름으로 쿼리 저장소 구성에 쓰입니다. |

### Content integrity hashes (`t173.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `sha512sum` | identifier | 사전에 없음 | SHA-512 체크섬을 계산하고 검증하는 명령입니다. |

### Direnv approval (`t14.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `.envrc` | extension | 다른 분야에만 있음: `configuration` — Directory environment loading | direnv가 승인 후 불러오는 파일로 승인 절차 문서에 쓰입니다. |

### Hardcoded credentials (`t80.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `default credentials` | phrase | 사전에 없음 | 바꾸지 않은 기본 계정 정보를 가리키며 보안 점검 규칙에 쓰입니다. |
| `embedded credentials` | phrase | 사전에 없음 | 코드와 바이너리에 포함된 자격 증명을 가리키는 보안 점검 용어입니다. |
| `hardcoded api key` | phrase | 사전에 없음 | 소스에 박힌 API 키를 가리키는 코드 리뷰와 정적 분석 용어입니다. |

### JSON hijacking defenses (`x30`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cross site script inclusion` | phrase | 사전에 없음 | 다른 출처 스크립트 포함으로 데이터를 훔치는 XSSI 공격의 풀어쓴 이름입니다. |

### Message digest APIs (`t765`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `CC_SHA256` | identifier | 사전에 없음 | Apple CommonCrypto의 SHA-256 계산 함수 이름입니다. |

### Penetration testing (`t251.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ctf` | abbreviation | 사전에 없음 | 보안 경연 대회 형식으로 취약점 실습 저장소와 문서에 쓰입니다. |
| `fuzzing` | word | 다른 분야에만 있음: `testing` — Fuzz property testing | 보안 분야에서는 입력 변조로 취약점을 찾는 침투 테스트 기법을 가리킵니다. |

### Privacy consent policies (`t368.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `data minimization` | phrase | 사전에 없음 | 필요한 만큼만 수집하는 개인정보 보호 원칙으로 정책 문서에 쓰입니다. |
| `data protection officer` | phrase | 사전에 없음 | GDPR에서 지정하는 개인정보 보호 책임자로 정책 문서에 쓰입니다. |
| `data subject access request` | phrase | 사전에 없음 | 정보 주체가 자기 데이터 열람을 요구하는 절차로 대응 기능과 문서에 쓰입니다. |
| `lgpd` | abbreviation | 사전에 없음 | 브라질 개인정보 보호법으로 규정 대응 코드와 문서에 쓰입니다. |
| `privacy impact assessment` | phrase | 사전에 없음 | 개인정보 영향 평가를 가리키는 GDPR 대응 문서 용어입니다. |

### Privilege escalation (`t44.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `confused deputy` | phrase | 사전에 없음 | 권한 있는 프로그램이 속아 권한을 남용하는 취약점 유형으로 설계 문서에 쓰입니다. |

### Prompt injection defenses (`t410.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `tool poisoning` | phrase | 사전에 없음 | 도구 설명에 악성 지시를 숨기는 MCP 공격을 가리키는 용어입니다. |

### Request forgery (`t965`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cross site request forgery` | phrase | 다른 분야에만 있음: `auth-identity` — Cookie session security | 요청 위조 공격 자체를 가리키는 풀어쓴 이름으로 이 분야의 핵심 취약점입니다. |
| `csrf` | abbreviation | 다른 분야에만 있음: `auth-identity` — Cookie session security | SSRF와 함께 다루는 대표 요청 위조 취약점으로 보안 문서와 방어 코드에 쓰입니다. |

### Secret leak scanning (`t20.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ggshield` | proper_noun | 사전에 없음 | GitGuardian의 비밀 값 탐지 명령줄 도구입니다. |
| `push protection` | phrase | 사전에 없음 | 비밀 값이 포함된 푸시를 막는 GitHub 기능입니다. |

### Secret redaction (`t20.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `filter_parameters` | identifier | 사전에 없음 | Rails가 로그에서 가릴 요청 매개변수를 지정하는 설정 키입니다. |

### Secret stores (`t20.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `1password` | proper_noun | 사전에 없음 | 비밀 값을 보관하는 비밀번호 관리자로 CLI 연동과 CI 설정에 쓰입니다. |
| `infisical` | proper_noun | 사전에 없음 | 오픈소스 비밀 값 관리 서비스로 CLI와 CI 설정에 쓰입니다. |

### Static security analysis (`t251.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `flawfinder` | proper_noun | 사전에 없음 | C 코드의 위험 함수 사용을 찾는 정적 분석 도구입니다. |

## 새 주제가 필요한 단어

### Secure random generation

범위: Generating unpredictable random values and tokens with cryptographically secure random number generators.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `csprng` | abbreviation | 다른 분야에만 있음: `algorithms-data-structures` — Cryptographic random generation | 이 분야의 범위에 명시된 항목으로 보안 코드에서는 키와 토큰 생성 요건으로 쓰입니다. |
| `secure random` | phrase | 다른 분야에만 있음: `algorithms-data-structures` — Cryptographic random generation | 보안 토큰과 키 생성에 암호학적 난수를 쓰라는 코드 리뷰와 정적 분석 규칙에 쓰입니다. |
| `urandom` | identifier | 다른 분야에만 있음: `algorithms-data-structures` — Cryptographic random generation | 운영체제 난수 소스로 키와 토큰 생성 코드에서 안전한 난수 출처로 쓰입니다. |
