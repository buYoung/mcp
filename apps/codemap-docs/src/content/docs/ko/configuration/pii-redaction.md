---
title: PII 마스킹
description: 기본 인증정보 마스킹 동작, 커스텀 규칙별 출력 예시, 내장 PII 유형 90종 목록입니다.
---

## 기본 설정

`[output].is_redact_enabled`의 기본값은 `true`이며, 별도 설정을 하지 않아도 MCP 출력에서 다음 민감값을 자동으로 가립니다. 원본 길이가 `10`바이트 이상이면 `[REDACTED]`로, `10`바이트 미만이면 같은 길이의 `*`로 치환해 응답 바이트 상한과 줄바꿈을 유지합니다.

- **API 키·토큰**: AWS(`AKIA*`, `ASIA*`), GitHub(`ghp_*`, `github_pat_*`), OpenAI(`sk-*`), Google(`AIza*`), Slack(`xox*`), Stripe, GitLab, npm, SendGrid, JWT(`eyJ*`)
- **인증 헤더·URL·개인키**: `Authorization` / `Bearer` / `Basic` 헤더, URL 내 비밀번호(`scheme://user:********@host`), 웹훅 URL, PEM 개인키 블록
- **민감 필드 값**: `api_key`, `token`, `password`, `passwd`, `pwd`, `secret`, `secret_key`, `private_key`, `access_key`, `access_key_id`로 끝나는 설정·코드 키(대소문자·구분자 무관)
- **개인정보(`[output.redact].pii_entities`)**: 기본값은 `[]`(비활성)이므로 이메일·카드번호·주민등록번호 같은 일반 PII는 필요한 유형을 추가했을 때만 가립니다.

## 마스킹 출력 예시

실시간 설정 변경은 [설정 플레이그라운드](/ko/configuration/playground/)에서도 바로 시험할 수 있습니다.

### 1. 기본 설정 출력 (`is_redact_enabled = true`, `pii_entities = []`)

원본 파일 (`pkg/services/notifications/mailer.go`, `conf/defaults.ini`):

```go
admin_password = "admin"
secret_key     = "SW2YcwTIb9zpOO5EhMmB8x7K"
github_token   = "ghp_7xQm9Lp2Vw4Kz8Rn1Yt6Bc3Df5Hj0Su2Ae4"
auth_header    = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.grafana.sig"
db_url         = "postgres://grafana:s3cr3tPass!@10.24.8.15:5432/grafana"
pem_key        = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgw...\n-----END PRIVATE KEY-----"
from_address   = "alerts@grafana.example.com"
kr_rrn         = "900101-1234567"
```

기본 설정에서 `read` / `search` / `grep` 출력:

```go
admin_password = "*****"
secret_key     = "[REDACTED]"
github_token   = "[REDACTED]"
auth_header    = "Authorization: Bearer [REDACTED]"
db_url         = "postgres://grafana:********@10.24.8.15:5432/grafana"
pem_key        = "[REDACTED]"
from_address   = "alerts@grafana.example.com"
kr_rrn         = "900101-1234567"
```

### 2. 내장 PII 유형 활성화 (`pii_entities`)

`.codemap/config.toml`에 `pii_entities = ["EMAIL_ADDRESS", "IP_ADDRESS", "CREDIT_CARD", "KR_RRN", "DE_PLZ"]`를 설정했을 때의 출력입니다. 짧은 숫자 패턴(`DE_PLZ` 등)은 주변 변수명이 해당 유형을 가리킬 때만 가립니다.

```go
// 입력 원문:
// db_url       = "postgres://grafana:s3cr3tPass!@10.24.8.15:5432/grafana"
// from_address = "alerts@grafana.example.com"
// kr_rrn       = "900101-1234567"
// billing_card = "4532-0151-1283-0366"
// postalCode   = "10115"
// timeoutMs    = 10115

db_url       = "postgres://grafana:********@[REDACTED]:5432/grafana"
from_address = "[REDACTED]"
kr_rrn       = "[REDACTED]"
billing_card = "[REDACTED]"
postalCode   = "*****"
timeoutMs    = 10115
```

### 3. 민감 필드·사용자 정규식·예외 규칙 적용

`sensitive_fields`는 대소문자와 `_`, `-` 구분자를 정규화해 비교합니다. `rules`에서 `(?P<secret>...)` 캡처 그룹을 쓰면 접두어는 남기고 그룹 부분만 가리며, `exceptions`는 지정한 규칙과 값이 정확히 일치할 때만 마스킹에서 제외합니다.

```go
// 입력 원문:
// webhook_signing_key = "whsec_live_98a7bc65d4ef3210"
// service_token       = "glsa_8Kp2Lm9Qx4Vz7Rn1Yt6Bc3Df5Hj0Su2A"
// dev_token           = "glsa_00000000000000000000000000000000"
// local_sender        = "admin@grafana.localhost"
// prod_sender         = "oncall@grafana.example.com"

webhook_signing_key = "[REDACTED]"
service_token       = "glsa_[REDACTED]"
dev_token           = "glsa_00000000000000000000000000000000"
local_sender        = "admin@grafana.localhost"
prod_sender         = "[REDACTED]"
```

## 커스텀 설정

저장소의 `.codemap/config.toml`이나 전역 `~/.codemap/config.toml`에서 마스킹 대상과 예외를 위 예시처럼 지정합니다.

```toml
[output]
is_redact_enabled = true # false로 바꾸면 인증정보·PII 마스킹 전체 비활성화

[output.redact]
# 1. 내장 PII 유형 선택 활성화 (저장소 목록은 전역 목록을 덮어쓰며, []는 상속 해제)
pii_entities = ["EMAIL_ADDRESS", "IP_ADDRESS", "CREDIT_CARD", "KR_RRN"]

# 2. 비밀값으로 취급할 필드 이름 추가
sensitive_fields = ["internalCredential", "webhookSigningKey"]

# 3. 사용자 정규식 규칙 추가 (id는 custom.으로 시작, (?P<secret>...) 그룹 지정 시 해당 부분만 마스킹)
rules = [
  { id = "custom.service-token", pattern = 'glsa_(?P<secret>[A-Za-z0-9_]{32,})' }
]

# 4. 테스트·예시용 안전 값 예외 지정 (PII 규칙 ID는 pii.<소문자-하이픈-이름>)
exceptions = [
  { rule_id = "pii.email-address", value = "admin@grafana.localhost" },
  { rule_id = "custom.service-token", value = "00000000000000000000000000000000" }
]
```

## 내장 PII 유형

| 범위 | 유형 이름 |
| --- | --- |
| 국제 공통 | `CREDIT_CARD`, `CRYPTO`, `DATE_TIME`, `EMAIL_ADDRESS`, `IBAN_CODE`, `IP_ADDRESS`, `MAC_ADDRESS`, `URL`, `UUID` |
| 호주 | `AU_ABN`, `AU_ACN`, `AU_MEDICARE`, `AU_TFN` |
| 캐나다 | `CA_POSTAL_CODE`, `CA_SIN` |
| 핀란드 | `FI_PERSONAL_IDENTITY_CODE` |
| 독일 | `DE_BSNR`, `DE_FUEHRERSCHEIN`, `DE_HANDELSREGISTER`, `DE_HEALTH_INSURANCE`, `DE_ID_CARD`, `DE_KFZ`, `DE_LANR`, `DE_PASSPORT`, `DE_PLZ`, `DE_SOCIAL_SECURITY`, `DE_TAX_ID`, `DE_TAX_NUMBER`, `DE_VAT_ID` |
| 인도 | `IN_AADHAAR`, `IN_GSTIN`, `IN_PAN`, `IN_PASSPORT`, `IN_VEHICLE_REGISTRATION`, `IN_VOTER` |
| 이탈리아 | `IT_DRIVER_LICENSE`, `IT_FISCAL_CODE`, `IT_IDENTITY_CARD`, `IT_PASSPORT`, `IT_VAT_CODE` |
| 한국 | `KR_BRN`, `KR_DRIVER_LICENSE`, `KR_FRN`, `KR_PASSPORT`, `KR_RRN` |
| 나이지리아 | `NG_NIN`, `NG_VEHICLE_REGISTRATION` |
| 필리핀 | `PH_PASSPORT`, `PH_TIN`, `PH_UMID` |
| 폴란드 | `PL_PESEL` |
| 싱가포르 | `SG_NRIC_FIN`, `SG_UEN` |
| 남아프리카공화국 | `ZA_COMPANY_REGISTRATION`, `ZA_DRIVER_LICENSE`, `ZA_ID_NUMBER`, `ZA_INCOME_TAX_NUMBER`, `ZA_LICENSE_PLATE`, `ZA_PASSPORT`, `ZA_TRAFFIC_REGISTER_NUMBER`, `ZA_VAT_NUMBER` |
| 스페인 | `ES_NIE`, `ES_NIF`, `ES_PASSPORT` |
| 스웨덴 | `SE_ORGANISATIONSNUMMER`, `SE_PERSONNUMMER` |
| 태국 | `TH_TNIN` |
| 튀르키예 | `TR_LICENSE_PLATE`, `TR_NATIONAL_ID` |
| 영국 | `UK_DRIVING_LICENCE`, `UK_NHS`, `UK_NINO`, `UK_PASSPORT`, `UK_POSTCODE`, `UK_VEHICLE_REGISTRATION` |
| 미국 | `ABA_ROUTING_NUMBER`, `MEDICAL_LICENSE`, `US_BANK_NUMBER`, `US_DRIVER_LICENSE`, `US_HEALTH_INSURANCE_MEMBER_ID`, `US_PRIOR_AUTHORIZATION_NUMBER`, `US_CLAIM_NUMBER`, `US_PRESCRIPTION_NUMBER`, `US_REFERRAL_NUMBER`, `US_PROVIDER_TAX_ID`, `US_ITIN`, `US_MBI`, `US_NPI`, `US_PASSPORT`, `US_SSN` |

## 참고 사항

- 숫자 식별자나 짧은 날짜처럼 모호한 패턴은 주변에 해당 유형을 가리키는 변수명·필드명이 있을 때만 가립니다(예: `postalCode = '10115'`은 마스킹, `timeoutMs = 10115`은 유지).
- `CRYPTO`는 비트코인 주소만 다루며, 전화번호와 자유 서술 문장 속 인명·지명·기관명 인식은 지원하지 않습니다.
