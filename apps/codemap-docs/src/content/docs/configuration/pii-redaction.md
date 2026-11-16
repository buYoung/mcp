---
title: PII Redaction
description: Opt-in PII masking settings, 90 supported entity types across 18 countries, exceptions, and detection limits for MCP output.
---

Credential masking is enabled by default, while PII masking is opt-in. `[output.redact].pii_entities` defaults to `[]`, so enable only the entity types your repository needs:

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@presidio.site" }]
```

| Setting | Behavior |
| --- | --- |
| `output.redact.pii_entities` | Case-sensitive entity names from the table below. No wildcards or automatic country selection. |
| Omitted key | Inherits the global list; otherwise defaults to `[]`. |
| Explicit `[]` | Disables additional PII types, including any inherited selection. |
| Invalid list | Logs a warning without printing values and falls back for this key only. |
| `output.is_redact_enabled = false` | Disables both credential and PII masking. |
| `output.redact.exceptions` | Requires an exact rule ID and exact full matched value. `CREDIT_CARD` maps to `pii.credit-card`; all other IDs follow the same lowercase, underscore-to-hyphen rule. |

An exception on one rule never exempts an overlapping match from another rule. Repository lists replace global lists rather than merging with them, and changes take effect on the next request after config reload without rebuilding the index.

## Supported types

The catalog includes 90 entity types: nine international types and 81 country-specific types across 18 countries, with 76 country formats for IBAN validation. Country labels reflect identifier formats rather than language models or complete national PII coverage.

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

Enabled types run regex candidate matching followed by format and checksum validation, without NLP models or confidence scoring. Ambiguous numbers, alphanumeric IDs, short dates, and scheme-free domains require a nearby field name or label so ordinary constants like `timeoutMs = 10000` stay visible while `postalCode = '10000'` is masked.

Label checks look up to 160 UTF-8 bytes back within the same field or statement, and one line above when a value follows `=` or `:`. Recognized labels include entity names, common field names across `camelCase` and `snake_case`, multilingual terms, and country-neutral names like `fiscalCode` or `identityCardNumber`. Healthcare administration rules also match `field_regex` patterns on original assignments such as `prescriptionNumber = '1234567'`; context labels are only checked in source assignments, not inferred across formatted text or plain numeric fields. See [catalog.json](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/src/redact/pii/catalog.json) for each pattern's `requires_context` and `context` settings.

Candidates that fail checksum or format checks are dropped, while unverified formats such as non-legacy Korean registration suffixes and non-strict German VAT numbers stay eligible when paired with a matching label. IBAN matching tries exact country lengths first and falls back to prefix and whole-candidate checksums so adjacent numbers are not skipped. Date validators use the current UTC date, email checks verify local/host syntax without querying public-suffix lists, and `CRYPTO` covers Bitcoin addresses only. Digit normalization applies only during validation; masking preserves original UTF-8 byte coordinates and uses `[REDACTED]` or asterisks.

Unlabeled lists, abbreviated custom labels, free-form phone numbers (`PHONE_NUMBER` and South African phone rules), and model-based entity recognition (names, places, organizations, or medical narrative) are not supported. Unsupported entity names are rejected during configuration validation, and source files, local indexes, search matching, and ordinary CLI output remain unmasked (see [credential redaction](/configuration/#credential-redaction)).
