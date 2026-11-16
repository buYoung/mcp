---
title: PII 마스킹
description: MCP 출력의 선택적 PII 마스킹 설정, 국제 공통 및 18개국 90종 지원 유형, 예외 처리, 탐지 한계 안내입니다.
---

인증정보 마스킹은 기본으로 켜져 있고 개인정보(PII) 마스킹은 선택 사항입니다. `[output.redact].pii_entities`의 기본값은 `[]`이므로 저장소에 필요한 유형만 정확한 이름으로 지정하세요.

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@example.com" }]
```

| 설정 | 동작 |
| --- | --- |
| `[output.redact].pii_entities` | 아래 표의 대소문자 구분 유형 이름(와일드카드나 국가 자동 선택 없음) |
| `[output.redact].pii_entities` 생략 | 전역 목록을 상속하며 전역값도 없으면 `[]` |
| `[output.redact].pii_entities = []` | 상속된 선택을 포함해 추가 PII 유형 해제 |
| 잘못된 `[output.redact].pii_entities` 값 | 값을 출력하지 않고 경고한 뒤 해당 키만 전역 목록 또는 `[]`로 대체 |
| `[output].is_redact_enabled = false` | 인증정보와 PII 마스킹을 모두 끔 |
| `[output.redact].exceptions` | `rule_id`(`pii.<소문자-하이픈-이름>`, 예: `pii.credit-card`)와 따옴표를 뺀 전체 탐지 값이 모두 일치할 때만 제외하며, 다른 규칙이 겹쳐 탐지한 범위까지 풀리지는 않음 |

저장소 목록은 전역 목록을 통째로 대체하며, 설정을 다시 읽으면 색인 재구축 없이 다음 요청부터 반영됩니다.

## 지원 유형

국제 공통 9종과 18개국 식별번호 81종을 합쳐 총 90종을 지원합니다. 국가 구분은 식별번호 형식의 범위일 뿐 해당 국가의 모든 개인정보 지원을 뜻하지 않습니다.

| 범위 | 허용하는 유형 이름 |
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

## 탐지 방식과 한계

- **형식과 체크섬 검증**: 선택한 유형은 형식과 체크섬을 검증해 일치하는 값을 `[REDACTED]`(짧은 값은 별표)로 가립니다.
- **모호한 값의 문맥 라벨 조건**: 숫자 식별자, 짧은 날짜, 스키마 없는 도메인은 주변 필드명이나 라벨이 있을 때만 가립니다(같은 문장 앞쪽 160 UTF-8 바이트 이내 또는 `=`/`:` 뒤 줄바꿈된 직전 1줄). 예를 들어 `postalCode = '10000'`은 가리고 `timeoutMs = 10000`은 남깁니다.
- **유형별 범위**:
  - `EMAIL_ADDRESS`는 주소와 호스트 문법만 검사하므로 미등록 접미사도 형식이 맞으면 가립니다.
  - `CRYPTO`는 비트코인 주소만 다룹니다.
  - `URL`, `IP_ADDRESS`, `DATE_TIME`은 조건에 맞는 공개 값도 가릴 수 있습니다.
  - 국가 식별번호의 생년월일 및 발급 연도 검증은 현재 UTC 날짜를 기준으로 비교합니다.
- **미지원 항목**: 라벨 없는 목록, 줄여 쓴 사용자 라벨, 전화번호(`PHONE_NUMBER`), 모델 기반 개체명 인식(인명, 지명, 기관명, 의료 서술)은 지원하지 않습니다.
- **마스킹 제외 대상**: 디스크의 원본 파일, 로컬 색인, 검색 일치, 일반 CLI 출력은 마스킹하지 않습니다([민감값 마스킹](/ko/configuration/#민감값-마스킹) 참고).
