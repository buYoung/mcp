export interface PiiGroupItem {
    scope: string;
    items: string;
}

export interface ConfigPeekModel {
    closeLabel: string;
    jumpSectionLabel: string;
    openPageLabel: string;
    piiTitle: string;
    piiIntro: string;
    piiHref: string;
    piiGroups: PiiGroupItem[];
}

export function getConfigPeekModel(locale: "en" | "ko"): ConfigPeekModel {
    const isKo = locale === "ko";
    return {
        closeLabel: isKo ? "닫기 (Esc)" : "Close (Esc)",
        jumpSectionLabel: isKo ? "아래 상세 열기 ↘" : "Jump to section ↘",
        openPageLabel: isKo ? "전체 문서 열기 ↗" : "Open full page ↗",
        piiTitle: isKo ? "내장 PII 마스킹 유형 (90종)" : "Built-in PII Redaction Types (90)",
        piiIntro: isKo
            ? "`[output.redact].pii_entities` 배열에 필요한 유형 문자열을 넣어 활성화합니다. 기본값 `[]`에서는 인증정보만 마스킹합니다."
            : "Add type names to `[output.redact].pii_entities` to mask them. With the default `[]`, only credentials are masked.",
        piiHref: isKo ? "/ko/configuration/pii-redaction/" : "/configuration/pii-redaction/",
        piiGroups: [
            {
                scope: isKo ? "국제 공통 (9)" : "International (9)",
                items: "CREDIT_CARD, CRYPTO, DATE_TIME, EMAIL_ADDRESS, IBAN_CODE, IP_ADDRESS, MAC_ADDRESS, URL, UUID",
            },
            {
                scope: isKo ? "한국 (5)" : "Korea (5)",
                items: "KR_BRN, KR_DRIVER_LICENSE, KR_FRN, KR_PASSPORT, KR_RRN",
            },
            {
                scope: isKo ? "미국 (15)" : "United States (15)",
                items: "ABA_ROUTING_NUMBER, MEDICAL_LICENSE, US_BANK_NUMBER, US_DRIVER_LICENSE, US_HEALTH_INSURANCE_MEMBER_ID, US_PRIOR_AUTHORIZATION_NUMBER, US_CLAIM_NUMBER, US_PRESCRIPTION_NUMBER, US_REFERRAL_NUMBER, US_PROVIDER_TAX_ID, US_ITIN, US_MBI, US_NPI, US_PASSPORT, US_SSN",
            },
            {
                scope: isKo ? "유럽·영국 (35)" : "Europe & UK (35)",
                items: "DE_BSNR, DE_FUEHRERSCHEIN, DE_HANDELSREGISTER, DE_HEALTH_INSURANCE, DE_ID_CARD, DE_KFZ, DE_LANR, DE_PASSPORT, DE_PLZ, DE_SOCIAL_SECURITY, DE_TAX_ID, DE_TAX_NUMBER, DE_VAT_ID, ES_NIE, ES_NIF, ES_PASSPORT, FI_PERSONAL_IDENTITY_CODE, IT_DRIVER_LICENSE, IT_FISCAL_CODE, IT_IDENTITY_CARD, IT_PASSPORT, IT_VAT_CODE, PL_PESEL, SE_ORGANISATIONSNUMMER, SE_PERSONNUMMER, TR_LICENSE_PLATE, TR_NATIONAL_ID, UK_DRIVING_LICENCE, UK_NHS, UK_NINO, UK_PASSPORT, UK_POSTCODE, UK_VEHICLE_REGISTRATION",
            },
            {
                scope: isKo ? "아시아·오세아니아·기타 (26)" : "APAC, Americas & Africa (26)",
                items: "AU_ABN, AU_ACN, AU_MEDICARE, AU_TFN, CA_POSTAL_CODE, CA_SIN, IN_AADHAAR, IN_GSTIN, IN_PAN, IN_PASSPORT, IN_VEHICLE_REGISTRATION, IN_VOTER, NG_NIN, NG_VEHICLE_REGISTRATION, PH_PASSPORT, PH_TIN, PH_UMID, SG_NRIC_FIN, SG_UEN, TH_TNIN, ZA_COMPANY_REGISTRATION, ZA_DRIVER_LICENSE, ZA_ID_NUMBER, ZA_INCOME_TAX_NUMBER, ZA_LICENSE_PLATE, ZA_PASSPORT, ZA_TRAFFIC_REGISTER_NUMBER, ZA_VAT_NUMBER",
            },
        ],
    };
}
