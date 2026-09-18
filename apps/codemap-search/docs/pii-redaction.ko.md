# PII 마스킹

한국어 | [English](./pii-redaction.md)

설정 버전 16부터 MCP 출력에 PII 마스킹을 선택 적용할 수 있습니다. 인증정보 마스킹의 기존 기본값은 유지하며, `pii_entities`의 기본값은 `[]`입니다. 저장소에 필요한 종류만 정확한 이름으로 지정합니다.

```toml
[tool_output]
is_redact_enabled = true

[redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@presidio.site" }]
```

| 설정 | 동작 |
| --- | --- |
| `redact.pii_entities` | 아래 표의 이름을 대소문자까지 정확히 지정. 와일드카드·국가 자동 선택 없음 |
| 키 생략 | 전역 목록을 상속하며 전역값도 없으면 `[]` |
| 명시적 `[]` | 상속된 선택을 포함해 추가 PII 종류를 해제 |
| 잘못된 목록 | 값을 출력하지 않고 경고한 뒤 해당 키만 하위 설정으로 대체. 다른 유효한 키는 유지 |
| `tool_output.is_redact_enabled = false` | 인증정보·PII 마스킹을 모두 끔 |
| `redact.exceptions` | 규칙 ID와 탐지한 원문 값 전체가 정확히 일치할 때만 제외. `CREDIT_CARD`의 ID는 `pii.credit-card`이며 나머지도 소문자 변환·밑줄을 하이픈으로 바꾸는 규칙을 따름 |

특정 규칙의 예외로 다른 규칙이 탐지한 겹치는 범위까지 해제하지 않습니다. 저장소 목록은 전역 목록을 대체합니다. 변경은 설정을 다시 읽은 뒤 후속 요청부터 적용하며 색인을 다시 만들 필요가 없습니다.

## 지원 종류

국제 공통 9종과 18개 국가의 식별번호 81종, 총 90종을 포함합니다. IBAN 검증에는 76개 국가 형식을 사용합니다. 국가 표시는 식별번호 형식의 범위이며 언어 모델이나 해당 국가의 모든 개인정보 지원을 의미하지 않습니다.

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

## 탐지 의미와 한계

선택한 종류는 정규식 후보와 형식·체크섬 검증을 사용합니다. 숫자·영숫자만으로 된 모호한 식별번호, 짧은 날짜, 스키마 없는 도메인 등은 관련 필드명·라벨이 있어야 가립니다. 예를 들어 `timeoutMs = 10000`은 유지하고 `postalCode = '10000'`은 가립니다. 종류별·패턴별 적용 기준은 [catalog.json](../src/redact/pii/catalog.json)의 `requires_context`와 `context`에 기록합니다. Presidio 엔진, 신뢰도 점수, 자연어 모델은 사용하지 않습니다.

라벨은 종류 이름 자체, 통상적인 필드명과 원본의 여러 언어 표현을 사용하며, `postalCode`·`postal_code` 같은 표기도 인식합니다. 같은 필드·문장의 앞쪽 최대 160 UTF-8 바이트를 확인하고, 대입 기호나 콜론 다음에 값이 줄바꿈된 경우에는 직전 한 줄도 확인합니다. 문맥이 없는 모호한 값은 숨기지 않으므로 라벨 없는 목록이나 생략·변형된 표현은 탐지하지 못할 수 있습니다. 실제 개인에게 속한 값인지 판정하지 않으며, 선택한 URL·IP·날짜 등의 공개 값도 해당 조건에 맞으면 숨깁니다.

국가 접두사를 생략한 `fiscalCode`·`identityCardNumber` 같은 필드명도 라벨로 인식합니다. 의료 행정번호는 원본의 라벨 뒤 값 패턴을 `field_regex`로 함께 기록하여 `prescriptionNumber = '1234567'`처럼 코드 필드에 들어간 값도 탐지합니다. 이 보조 패턴은 관련 라벨이 있는 원문에서만 적용하며 일반 숫자 필드나 서식이 붙은 최종 출력에는 문맥을 추정해 다시 적용하지 않습니다.

원본 기본 검증에서 명시적으로 거부한 후보는 제외합니다. 한국 등록번호·독일 VAT처럼 체크섬 검증이 확정되지 않아도 허용하는 식별번호에는 라벨 조건을 추가합니다. 한국 등록번호의 신규 형식은 기존 체크섬과 달라도 관련 라벨이 있으면 후보로 유지하며, 독일 VAT는 원본의 비엄격 기본 검증과 라벨 조건을 함께 적용합니다. IBAN은 국가 형식의 정확한 길이를 먼저 선택하고 원본의 접두부 형식·전체 체크섬 검사를 보조적으로 유지하여 인접한 IBAN을 놓치지 않습니다. 현재 연도·생년월일을 판정하는 검증은 UTC 날짜를 사용합니다.

이메일은 `tldextract`나 외부 공개 접미사 자료를 조회하지 않고 로컬에서 주소·호스트 형식을 확인합니다. 따라서 등록되지 않은 접미사를 사용하는 주소도 형식이 맞으면 가릴 수 있습니다. `CRYPTO`는 이 규칙들이 제공하는 Bitcoin 주소를 대상으로 합니다. 숫자 정규화는 검증에만 사용하며 탐지·마스킹 좌표는 원문 UTF-8 바이트 기준을 유지합니다.

`PHONE_NUMBER`와 남아프리카 전화번호 탐지기는 포함하지 않습니다. 정규식 목록만으로 이식할 수 없으며 외부 `phonenumbers` 탐지기와 번호 체계 자료에 의존하기 때문입니다. 사람 이름·지명·집단 이름·의료 서술의 모델 또는 외부 서비스 기반 인식도 이 목록에 포함하지 않습니다. 미지원 종류는 설정 검증에서 거부합니다.

MCP 초기화·도구 정의 응답의 프로토콜 협상 필드, 내장 안내문, 도구 이름과 스키마는 원형을 유지합니다.

치환은 기존 `[REDACTED]`·별표 방식을 사용합니다. 암호화·복원용 매핑·다른 치환 연산자·Python 프로세스·자연어 모델·네트워크 서비스를 추가하지 않습니다. 원본 파일·로컬 색인·검색 일치·일반 CLI 출력은 [기존 적용 범위](./configuration.ko.md#민감값-마스킹)를 유지합니다.

## 출처와 검증

규칙과 합성 입력·출력 사례는 [Presidio](https://github.com/data-privacy-stack/presidio)를 참고했습니다. [catalog.json](../src/redact/pii/catalog.json)의 각 항목에 원본 탐지기 링크가 있고, [cases.jsonl](../src/redact/pii/cases.jsonl)의 각 행에 원본 테스트와 사례 번호가 있습니다. 이식한 자료에는 [MIT 고지](../vendor/presidio/LICENSE)를 함께 둡니다. 이 기록은 갱신 주기를 정하거나 실행 의존성을 고정하지 않습니다.

Rust 검증에는 90종 전체의 원본 패턴·검증 사례 1,653개, 반복·경계 조합 3,913개, Unicode 좌표, 라벨 조건, 정상 코드 보존, 선택 활성화 기본값, 정확한 예외, 키별 대체와 최종 MCP 출력 확인이 포함됩니다. 원본 사례는 후보 탐지의 호환성을 확인하며, 실제 마스킹에는 위 라벨 조건을 별도로 적용합니다. Python 프레임워크, 자연어 점수 임계값과 기본값이 아닌 엄격 체크섬 모드의 테스트는 가져오지 않았습니다.

대용량 검증은 실행 가능한 [판매자 등록 서비스 예제](../tests/fixtures/redact_app/merchant_service.ts)를 사용합니다. 타입·요청 검증, 저장소, 심사, 결제·환불·정산, 내보내기, 이벤트 재시도 로직과 10개 테넌트·180개 사업자 입력이 연결된 1만 줄입니다. 90종의 값 900개에 대한 [예상 위치](../tests/fixtures/redact_app/merchant_service.pii.json)를 출력과 독립적으로 기록하며, 각 값이 해당 종류의 규칙으로 탐지되는지도 확인합니다. MCP 검증은 마스킹한 값 외의 9,100줄, 줄 번호, 원본 파일을 비교합니다. 예제의 `runAllScenarios()`는 업무 흐름 자체를 검증하며, [rebuild.py](../tests/fixtures/redact_app/rebuild.py)는 서비스 로직을 유지하면서 입력과 예상 위치를 재생성합니다.

같은 조건의 예제를 네 언어로 추가 제공합니다. 각 파일은 독립적인 1만 줄이며, 10개 테넌트·180개 사업자·90종 PII 900개를 포함합니다. 필드명은 각 언어의 관례를 따르고, 빈 문장이나 반복 주석으로 줄 수를 채우지 않습니다.

| 예제 | 실행 흐름 | 예상 위치 |
| --- | --- | --- |
| [Go](../tests/fixtures/redact_app/merchant_service.go) | HTTP 요청·JSON 변환, 등록·심사·결제·환불·정산, CSV 내보내기 | [Go 위치](../tests/fixtures/redact_app/merchant_service.go.pii.json) |
| [Rust](../tests/fixtures/redact_app/merchant_service.rs) | 국가별 문서 enum, 입력 검증, 심사·결제·복식 원장·CSV 내보내기 | [Rust 위치](../tests/fixtures/redact_app/merchant_service.rs.pii.json) |
| [Java](../tests/fixtures/redact_app/MerchantService.java) | DTO·record, 명령 처리, 심사·결제·원장·CSV 내보내기 | [Java 위치](../tests/fixtures/redact_app/MerchantService.java.pii.json) |
| [HTML](../tests/fixtures/redact_app/merchant_dashboard.html) | 실제 입력 폼, 테넌트 필터, 등록·심사·결제 버튼, 상태·잔액 표시, CSV 내보내기 | [HTML 위치](../tests/fixtures/redact_app/merchant_dashboard.html.pii.json) |

[rebuild_languages.py](../tests/fixtures/redact_app/rebuild_languages.py)는 네 예제의 입력과 위치를 재생성하며 `--check`로 재현성을 검사합니다. [verify_languages.py](../tests/fixtures/redact_app/verify_languages.py)는 설치된 Go·Rust·Java 컴파일러로 빌드·실행하고, HTML 구조와 JavaScript 구문 및 폼에서 추출한 값의 업무 로직을 검사합니다. 외부 라이브러리를 설치하지 않습니다. HTML의 Node 검증은 브라우저 DOM·이벤트·화면 검증을 대신하지 않습니다. 허용된 브라우저에서 HTML 파일에 `?verify`를 붙여 열면 별도의 화면 동작 검증 결과를 페이지에 표시합니다.
