---
title: PII 마스킹
description: MCP 출력의 선택적 PII 마스킹 설정과 90종 지원 유형 목록입니다.
---

인증정보 마스킹은 기본으로 켜져 있고 개인정보(PII) 마스킹은 선택 사항입니다(`[output.redact].pii_entities` 기본값 `[]`). 필요한 유형만 `.codemap/config.toml`이나 `~/.codemap/config.toml`에 지정하세요.

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@example.com" }]
```

- 저장소의 `pii_entities` 목록은 전역 목록을 덮어쓰며, `[]`로 비우면 전역에 설정한 유형도 해제됩니다.
- `[output].is_redact_enabled = false`로 설정하면 인증정보와 PII 마스킹을 모두 끕니다.
- 예외로 남길 값은 `exceptions`에 `rule_id`(`pii.<소문자-하이픈-이름>`, 예: `pii.credit-card`)와 `value`(소스 코드 리터럴의 바깥 따옴표를 제외한 값)를 함께 적으세요.

## 지원 유형

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
