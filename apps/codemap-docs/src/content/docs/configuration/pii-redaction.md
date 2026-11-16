---
title: PII Redaction
description: Default credential masking, custom redaction rules, and 90 built-in PII entity types for MCP output.
---

## Default settings

`[output].is_redact_enabled` defaults to `true`. Out of the box, `codemap-search` automatically replaces the following secrets with `[REDACTED]` in MCP responses:

- **API keys and tokens**: AWS (`AKIA*`, `ASIA*`), GitHub (`ghp_*`, `github_pat_*`), OpenAI (`sk-*`), Google (`AIza*`), Slack (`xox*`), Stripe, GitLab, npm, SendGrid, and JWTs (`eyJ*`)
- **Auth headers, URLs, and private keys**: `Authorization` / `Bearer` / `Basic` headers, URL passwords (`scheme://user:password@host`), webhook URLs, and PEM private keys
- **Sensitive fields**: config or code fields ending in `api_key`, `token`, `password`, `passwd`, `pwd`, `secret`, `secret_key`, `private_key`, `access_key`, or `access_key_id` (case- and separator-insensitive)
- **General PII (`[output.redact].pii_entities`)**: defaults to `[]` (off), so emails, credit cards, and national IDs remain visible unless you opt in below.

## Customizing redaction

Override `[output]` and `[output.redact]` in `.codemap/config.toml` (repository) or `~/.codemap/config.toml` (global):

```toml
[output]
is_redact_enabled = true # set false to disable all credential and PII redaction

[output.redact]
# 1. Opt in to built-in PII entity types (a repo list replaces the global list; [] clears inheritance)
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD", "KR_RRN"]

# 2. Treat additional field names as secrets
sensitive_fields = ["internalCredential", "webhookSigningKey"]

# 3. Add custom regex rules (id must start with custom.; optional (?P<secret>...) masks only that group)
rules = [
  { id = "custom.service-token", pattern = 'glsa_(?P<secret>[A-Za-z0-9_]{32,})' }
]

# 4. Exempt known test or placeholder values (PII rule IDs use pii.<lowercase-hyphenated-name>)
exceptions = [
  { rule_id = "pii.email-address", value = "admin@grafana.localhost" },
  { rule_id = "custom.service-token", value = "00000000000000000000000000000000" }
]
```

## Built-in PII entity types

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
