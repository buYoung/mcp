---
title: PII Redaction
description: Opt-in PII redaction settings and 90 supported entity types for MCP output.
---

Credential redaction is enabled by default, while PII redaction is opt-in (`[output.redact].pii_entities` defaults to `[]`). List only the entity types your repository needs in `.codemap/config.toml` or `~/.codemap/config.toml`:

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@example.com" }]
```

- A repository `pii_entities` list replaces the global list (`[]` clears inherited entity types).
- Set `[output].is_redact_enabled = false` to disable both credential and PII redaction.
- To exempt a known safe value, add `{ rule_id, value }` to `exceptions` using `pii.<lowercase-hyphenated-name>` (for example, `pii.credit-card`) and the exact matched string (excluding surrounding quotes in source code).

## Supported entity types

| Scope | Entity types |
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

## Notes

- Ambiguous patterns such as numeric IDs and short dates are redacted only when paired with a nearby matching variable or field name (for example, `postalCode = '10000'` is redacted while `timeoutMs = 10000` is kept).
- `CRYPTO` matches Bitcoin addresses only. Phone numbers and free-text names, locations, or organizations are not supported.
