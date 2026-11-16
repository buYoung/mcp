---
title: PII 마스킹
description: 기본 인증정보 마스킹 동작, 커스텀 규칙 추가 방법, 내장 PII 유형 90종 목록입니다.
---

## 기본 설정

`[output].is_redact_enabled`의 기본값은 `true`이며, 별도 설정을 하지 않아도 MCP 출력에서 다음 민감값을 자동으로 `[REDACTED]` 처리합니다.

- **API 키·토큰**: AWS(`AKIA*`, `ASIA*`), GitHub(`ghp_*`, `github_pat_*`), OpenAI(`sk-*`), Google(`AIza*`), Slack(`xox*`), Stripe, GitLab, npm, SendGrid, JWT(`eyJ*`)
- **인증 헤더·URL·개인키**: `Authorization` / `Bearer` / `Basic` 헤더, URL 내 비밀번호(`scheme://user:password@host`), 웹훅 URL, PEM 개인키 블록
- **민감 필드 값**: `api_key`, `token`, `password`, `passwd`, `pwd`, `secret`, `secret_key`, `private_key`, `access_key`, `access_key_id`로 끝나는 설정·코드 키(대소문자·구분자 무관)
- **개인정보(`[output.redact].pii_entities`)**: 기본값은 `[]`(비활성)이므로 이메일·카드번호·주민등록번호 같은 일반 PII는 아래처럼 필요한 유형을 추가했을 때만 가립니다.

## 커스텀 설정

저장소의 `.codemap/config.toml`이나 전역 `~/.codemap/config.toml`에서 마스킹 대상과 예외를 추가할 수 있습니다.

```toml
[output]
is_redact_enabled = true # false로 바꾸면 인증정보·PII 마스킹 전체 비활성화

[output.redact]
# 1. 내장 PII 유형 선택 활성화 (저장소 목록은 전역 목록을 덮어쓰며, []는 상속 해제)
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD", "KR_RRN"]

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

- 숫자 식별자나 짧은 날짜처럼 모호한 패턴은 주변에 해당 유형을 가리키는 변수명·필드명이 있을 때만 가립니다(예: `postalCode = '10000'`은 마스킹, `timeoutMs = 10000`은 유지).
- `CRYPTO`는 비트코인 주소만 다루며, 전화번호와 자유 서술 문장 속 인명·지명·기관명 인식은 지원하지 않습니다.
