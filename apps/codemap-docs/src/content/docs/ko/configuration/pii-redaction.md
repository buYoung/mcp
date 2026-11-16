---
title: PII 마스킹
description: MCP 출력의 선택적 PII 마스킹 설정, 국제 공통 및 18개국 90종 지원 식별번호, 예외 처리, 탐지 한계 안내입니다.
---

인증정보 마스킹은 기본으로 켜져 있고, 개인정보(PII) 마스킹은 필요할 때 직접 켜는 방식입니다. `[output.redact].pii_entities`의 기본값은 `[]`이므로 저장소에 필요한 유형만 정확한 이름으로 지정합니다.

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@presidio.site" }]
```

| 설정 | 동작 |
| --- | --- |
| `output.redact.pii_entities` | 아래 표의 이름을 대소문자까지 정확히 지정. 와일드카드나 국가 자동 선택 없음 |
| 키 생략 | 전역 목록을 상속하며 전역값도 없으면 `[]` |
| 명시적 `[]` | 상속된 선택을 포함해 추가 PII 종류를 해제 |
| 잘못된 목록 | 값을 출력하지 않고 경고한 뒤 해당 키만 하위 설정으로 대체. 다른 유효한 키는 유지 |
| `output.is_redact_enabled = false` | 인증정보와 PII 마스킹을 모두 끔 |
| `output.redact.exceptions` | 규칙 ID와 탐지한 원문 값 전체가 정확히 일치할 때만 제외. `CREDIT_CARD`의 ID는 `pii.credit-card`이며 나머지도 소문자 변환 후 밑줄을 하이픈으로 바꾸는 규칙을 따름 |

한 규칙에 예외를 두더라도 다른 규칙이 겹쳐 탐지한 범위까지 풀리지는 않습니다. 저장소 목록은 전역 목록과 합쳐지지 않고 통째로 대체하며, 설정을 다시 읽으면 색인 재생성 없이 다음 요청부터 바로 적용됩니다.

## 지원 종류

국제 공통 9종과 18개국의 식별번호 81종을 합쳐 총 90종을 지원하며, IBAN 검증은 76개 국가 형식을 다룹니다. 국가 구분은 식별번호 형식의 범위일 뿐 언어 모델이나 해당 국가의 모든 개인정보 지원을 뜻하지 않습니다.

| 범위 | 허용하는 종류 이름 |
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

선택한 유형은 정규식으로 후보를 찾은 뒤 형식과 체크섬을 검증해 판별하며, 별도 NLP 모델이나 신뢰도 점수는 쓰지 않습니다. 모호한 숫자나 영숫자 식별자, 짧은 날짜, 스키마 없는 도메인은 주변 필드명이나 라벨이 있을 때만 가립니다. 예를 들어 `timeoutMs = 10000`은 그대로 두고 `postalCode = '10000'`만 마스킹합니다.

라벨은 같은 필드나 문장의 앞쪽 최대 160 UTF-8 바이트 안에서 찾으며, `=` 또는 `:` 뒤에서 줄이 바뀌면 직전 한 줄까지 함께 확인합니다. 유형 이름 자체와 `camelCase` 및 `snake_case` 필드명, 다국어 용어뿐 아니라 `fiscalCode`, `identityCardNumber` 같은 국가 중립 필드명도 라벨로 봅니다. 의료 행정번호는 `prescriptionNumber = '1234567'` 같은 원문 대입문도 `field_regex` 패턴으로 탐지합니다. 문맥 라벨은 원문 대입 구문에서만 확인하며 서식이 적용된 출력이나 일반 숫자 필드에서는 새로 추론하지 않습니다. 규칙별 `requires_context`와 `context` 기준은 [catalog.json](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/src/redact/pii/catalog.json)에서 확인할 수 있습니다.

체크섬이나 형식 검증을 통과하지 못한 후보는 제외하지만, 기존 체크섬과 다른 한국 등록번호 신규 접미사나 비엄격 독일 VAT처럼 검증을 확정할 수 없는 식별자는 관련 라벨이 있을 때 후보로 남깁니다. IBAN은 국가별 정확한 길이를 먼저 적용하고 접두사 형식과 전체 체크섬을 보조로 사용해 인접한 IBAN을 놓치지 않습니다. 날짜 검증은 현재 UTC 날짜를 기준으로 하며, 이메일은 공개 접미사 목록 조회 없이 주소와 호스트 문법만 검사하므로 미등록 접미사도 형식이 맞으면 가릴 수 있습니다. `CRYPTO`는 비트코인 주소만 다루며, 숫자 정규화는 검증 단계에만 쓰고 실제 마스킹은 원문 UTF-8 바이트 위치에 `[REDACTED]` 또는 별표를 채웁니다.

라벨 없는 목록, 줄여 쓴 사용자 라벨, 자유 형식 전화번호(`PHONE_NUMBER` 및 남아프리카 전화번호 규칙), 이름이나 지명, 기관, 의료 서술 같은 모델 기반 개체명 인식은 지원하지 않습니다. 목록에 없는 이름은 설정 검증에서 거부하며, 원본 파일과 로컬 색인, 검색 일치, 일반 CLI 출력은 마스킹하지 않습니다([민감값 마스킹](/ko/configuration/#민감값-마스킹) 참고).
