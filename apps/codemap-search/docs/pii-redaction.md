# PII redaction

[한국어](./pii-redaction.ko.md) | English

Schema 16 adds opt-in PII masking to MCP output. Credential masking stays enabled by default; `pii_entities` defaults to `[]`. Select the exact types needed for your repository.

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@presidio.site" }]
```

| Setting | Behavior |
| --- | --- |
| `redact.pii_entities` | Case-sensitive entity names from the table below. No wildcard or automatic country selection. |
| Omitted key | Inherits the global list; otherwise `[]`. |
| Explicit `[]` | Disables additional PII types, including an inherited selection. |
| Invalid list | Warns without printing its values and falls back for this key. Other valid keys still apply. |
| `tool_output.is_redact_enabled = false` | Disables credential and PII masking. |
| `redact.exceptions` | Exact rule ID plus entire original matched value. `CREDIT_CARD` uses `pii.credit-card`; other IDs follow the same lowercase, underscore-to-hyphen conversion. |

An exception for one rule cannot exempt an overlapping detection by another rule. Repository lists replace global lists. Changes apply to subsequent requests after config reload; no index rebuild is needed.

## Supported types

The catalog contains 90 types: nine international types and 81 country-specific types across 18 countries. IBAN validation includes 76 country formats. Country labels describe identifier formats, not language models or exhaustive national PII coverage.

| Scope | Accepted entity names |
| --- | --- |
| International | `CREDIT_CARD`, `CRYPTO`, `DATE_TIME`, `EMAIL_ADDRESS`, `IBAN_CODE`, `IP_ADDRESS`, `MAC_ADDRESS`, `URL`, `UUID` |
| Australia | `AU_ABN`, `AU_ACN`, `AU_MEDICARE`, `AU_TFN` |
| Canada | `CA_POSTAL_CODE`, `CA_SIN` |
| Finland | `FI_PERSONAL_IDENTITY_CODE` |
| Germany | `DE_BSNR`, `DE_FUEHRERSCHEIN`, `DE_HANDELSREGISTER`, `DE_HEALTH_INSURANCE`, `DE_ID_CARD`, `DE_KFZ`, `DE_LANR`, `DE_PASSPORT`, `DE_PLZ`, `DE_SOCIAL_SECURITY`, `DE_TAX_ID`, `DE_TAX_NUMBER`, `DE_VAT_ID` |
| India | `IN_AADHAAR`, `IN_GSTIN`, `IN_PAN`, `IN_PASSPORT`, `IN_VEHICLE_REGISTRATION`, `IN_VOTER` |
| Italy | `IT_DRIVER_LICENSE`, `IT_FISCAL_CODE`, `IT_IDENTITY_CARD`, `IT_PASSPORT`, `IT_VAT_CODE` |
| Korea | `KR_BRN`, `KR_DRIVER_LICENSE`, `KR_FRN`, `KR_PASSPORT`, `KR_RRN` |
| Nigeria | `NG_NIN`, `NG_VEHICLE_REGISTRATION` |
| Philippines | `PH_PASSPORT`, `PH_TIN`, `PH_UMID` |
| Poland | `PL_PESEL` |
| Singapore | `SG_NRIC_FIN`, `SG_UEN` |
| South Africa | `ZA_COMPANY_REGISTRATION`, `ZA_DRIVER_LICENSE`, `ZA_ID_NUMBER`, `ZA_INCOME_TAX_NUMBER`, `ZA_LICENSE_PLATE`, `ZA_PASSPORT`, `ZA_TRAFFIC_REGISTER_NUMBER`, `ZA_VAT_NUMBER` |
| Spain | `ES_NIE`, `ES_NIF`, `ES_PASSPORT` |
| Sweden | `SE_ORGANISATIONSNUMMER`, `SE_PERSONNUMMER` |
| Thailand | `TH_TNIN` |
| Turkey | `TR_LICENSE_PLATE`, `TR_NATIONAL_ID` |
| United Kingdom | `UK_DRIVING_LICENCE`, `UK_NHS`, `UK_NINO`, `UK_PASSPORT`, `UK_POSTCODE`, `UK_VEHICLE_REGISTRATION` |
| United States | `ABA_ROUTING_NUMBER`, `MEDICAL_LICENSE`, `US_BANK_NUMBER`, `US_DRIVER_LICENSE`, `US_HEALTH_INSURANCE_MEMBER_ID`, `US_PRIOR_AUTHORIZATION_NUMBER`, `US_CLAIM_NUMBER`, `US_PRESCRIPTION_NUMBER`, `US_REFERRAL_NUMBER`, `US_PROVIDER_TAX_ID`, `US_ITIN`, `US_MBI`, `US_NPI`, `US_PASSPORT`, `US_SSN` |

## Detection semantics and limits

Selected types use regex candidates and format/checksum validation. Ambiguous numeric/alphanumeric identifiers, short dates and scheme-free domains additionally require a related field name or label. For example, `timeoutMs = 10000` stays visible while `postalCode = '10000'` is masked. The `requires_context` and `context` fields in [catalog.json](../src/redact/pii/catalog.json) record the policy for each pattern. No Presidio engine, confidence scoring or NLP model is used.

Labels include the entity name, common field names and upstream multilingual terms, accepting forms such as `postalCode` and `postal_code`. Context is limited to the preceding 160 UTF-8 bytes within the same field/statement; one preceding line is considered when a value continues after an assignment or colon. Ambiguous values without such labels remain visible, so unlabeled lists and abbreviated or altered labels may not be detected. The rules do not establish that a value belongs to a real person: selected URLs, IP addresses and dates may also mask public values that meet the conditions.

Country-neutral field names such as `fiscalCode` and `identityCardNumber` also supply label context. Healthcare administration rules retain the value portion of their original labelled patterns as `field_regex`, covering assignments such as `prescriptionNumber = '1234567'`. These adapters require a related label in the original input; they do not classify ordinary numeric fields or infer new context in formatted output.

Explicit upstream validation failures reject candidates; unverified identifiers additionally require label context. Korean registration numbers with non-legacy suffixes can therefore remain candidates with a related label, and German VAT combines upstream non-strict validation with the label requirement. IBAN prefers the exact country-format length, retaining the upstream prefix-format and whole-candidate checksum as a fallback so adjacent IBANs are not skipped. Calendar-dependent validators use the current UTC date.

Email validation checks local address/host syntax without consulting `tldextract` or downloading public-suffix data. Consequently it may mask syntactically valid addresses under unregistered suffixes. Bitcoin is the only `CRYPTO` address family supplied by these rules. Digit normalization happens only during validation; detection and masking retain original UTF-8 byte coordinates.

`PHONE_NUMBER` and the South African phone recognizers are not included: their implementation depends on the external `phonenumbers` matcher and numbering-plan metadata, rather than a portable pattern catalog. Model/service-based recognition of people, locations, group names and medical narrative is also outside this catalog. Unsupported entity names are rejected by configuration validation.

MCP initialization and tool definitions preserve protocol negotiation fields, bundled instructions, tool names and schemas verbatim.

Masking uses the existing `[REDACTED]`/asterisk replacement. No encryption, reversible mapping, replacement operators, Python process, NLP model or network service is added. Source files, local indexes, matching, and ordinary CLI output retain the [existing scope](./configuration.md#credential-redaction).

## Sources and verification

Rules and synthetic input/output cases are adapted from [Presidio](https://github.com/data-privacy-stack/presidio). Each entry in [catalog.json](../src/redact/pii/catalog.json) links to its recognizer source; each row in [cases.jsonl](../src/redact/pii/cases.jsonl) names its original test and case index. [MIT attribution](../vendor/presidio/LICENSE) accompanies the adapted material. These references impose no update schedule or frozen runtime dependency.

The Rust suite checks 1,653 upstream pattern/validation cases covering all 90 types, 3,913 repetition/boundary combinations, Unicode offsets, label requirements, normal code preservation, opt-in defaults, exact exceptions, per-key fallback and final MCP output. Upstream examples check candidate compatibility; actual masking separately applies the label policy above. Python framework tests, NLP score-threshold cases and non-default strict-checksum modes are not imported.

The large fixture is an executable [merchant service](../tests/fixtures/redact_app/merchant_service.ts): 10,000 lines connecting request validation, repositories, review, payment/refund/settlement, exports and event retries to 10 tenants and 180 merchant inputs. An independent [span manifest](../tests/fixtures/redact_app/merchant_service.pii.json) identifies 900 values across all 90 types; each value is also checked against its own entity rule. MCP tests compare the remaining 9,100 lines, line numbers and files on disk. `runAllScenarios()` exercises the application workflows, while [rebuild.py](../tests/fixtures/redact_app/rebuild.py) preserves the service logic and regenerates scenario data and expected spans.

Four additional language fixtures use the same conditions. Each independent file has 10,000 lines, 10 tenants, 180 merchants and 900 sensitive values across all 90 types. Bindings follow language conventions; empty statements and repeated comments do not pad the line count.

| Fixture | Application flow | Expected spans |
| --- | --- | --- |
| [Go](../tests/fixtures/redact_app/merchant_service.go) | HTTP requests and JSON transport, registration/review/payment/refund/settlement, CSV export | [Go spans](../tests/fixtures/redact_app/merchant_service.go.pii.json) |
| [Rust](../tests/fixtures/redact_app/merchant_service.rs) | Country document enums, input validation, review/payments, double-entry ledger, CSV export | [Rust spans](../tests/fixtures/redact_app/merchant_service.rs.pii.json) |
| [Java](../tests/fixtures/redact_app/MerchantService.java) | DTOs and records, command dispatch, review/payments, ledger, CSV export | [Java spans](../tests/fixtures/redact_app/MerchantService.java.pii.json) |
| [HTML](../tests/fixtures/redact_app/merchant_dashboard.html) | Input forms, tenant filtering, registration/review/payment buttons, status/balance display, CSV export | [HTML spans](../tests/fixtures/redact_app/merchant_dashboard.html.pii.json) |

[rebuild_languages.py](../tests/fixtures/redact_app/rebuild_languages.py) regenerates these inputs and spans; `--check` verifies reproducibility. [verify_languages.py](../tests/fixtures/redact_app/verify_languages.py) builds and executes them using installed Go, Rust and Java compilers, then checks HTML structure, JavaScript syntax and the business model against values extracted from the forms. It installs no external libraries. The HTML checks under Node do not establish browser DOM, event or rendering compatibility. Opening the HTML with `?verify` in a permitted browser displays a separate browser interaction check report.
