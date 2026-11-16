---
title: PII Redaction
description: Opt-in PII masking settings, 90 supported entity types across 18 countries, exceptions, and detection limits for MCP output.
---

Credential masking is enabled by default, while PII masking is opt-in. `[output.redact].pii_entities` defaults to `[]`, so list only the entity types your repository needs:

```toml
[output]
is_redact_enabled = true

[output.redact]
pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE", "US_SSN"]
exceptions = [{ rule_id = "pii.email-address", value = "info@example.com" }]
```

| Setting | Behavior |
| --- | --- |
| `[output.redact].pii_entities` | Case-sensitive entity names from the table below; no wildcards or automatic country selection |
| `[output.redact].pii_entities` omitted | Inherits the global list; otherwise defaults to `[]` |
| `[output.redact].pii_entities = []` | Disables opt-in PII types, including inherited ones |
| Invalid `[output.redact].pii_entities` value | Logs a warning without printing values and falls back to the global list or `[]` for this key only |
| `[output].is_redact_enabled = false` | Disables both credential and PII masking |
| `[output.redact].exceptions` | Exempts matches only when both `rule_id` (`pii.<lowercase-hyphenated-name>`, e.g., `pii.credit-card`) and the unquoted matched value match exactly; never exempts overlapping matches from another rule |

Repository lists replace global lists instead of merging. Configuration changes apply on the next request after reload without rebuilding the index.

## Supported types

90 entity types are supported across International (9) and 18 country scopes (81). Country labels denote identifier formats, not full national PII coverage.

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

## Detection behavior and limits

- **Format and checksum matching**: Enabled types validate format and checksums, replacing matches with `[REDACTED]` (or asterisks for short values).
- **Label context for ambiguous values**: Numeric IDs, short dates, and scheme-free domains require a nearby field name or label (within 160 UTF-8 bytes before the value on the same statement, or the previous line after `=` or `:`). For example, `postalCode = '10000'` is masked while `timeoutMs = 10000` stays visible.
- **Type-specific scope**:
  - `EMAIL_ADDRESS` validates address/host syntax, so unregistered suffixes can match.
  - `CRYPTO` covers Bitcoin addresses only.
  - `URL`, `IP_ADDRESS`, and `DATE_TIME` can mask public values that fit their patterns.
  - Birth and issue year checks in national ID validators compare against the current UTC date.
- **Unsupported items**: Unlabeled lists, abbreviated custom labels, phone numbers (`PHONE_NUMBER`), and model-based entity recognition (names, places, organizations, or medical narrative) are not supported.
- **Unmasked locations**: Source files on disk, local indexes, search matching, and ordinary CLI output remain unmasked (see [credential redaction](/configuration/#credential-redaction)).
