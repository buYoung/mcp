//! Candidate validation only. None retains Presidio's unverified-match semantics;
//! it is deliberately distinct from a rejected candidate (Some(false)).
mod africa;
mod asia;
mod checksums;
mod crypto;
mod europe;
mod international;
mod numerals;

use checksums::*;
pub(super) use international::has_exact_iban_length;
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
pub(super) enum Validator {
    #[serde(rename = "")]
    None,
    AbaRoutingRecognizer,
    AuAbnRecognizer,
    AuAcnRecognizer,
    AuMedicareRecognizer,
    AuTfnRecognizer,
    CaSinRecognizer,
    CreditCardRecognizer,
    CryptoRecognizer,
    DeBsnrRecognizer,
    DeHealthInsuranceRecognizer,
    DeIdCardRecognizer,
    DeLanrRecognizer,
    DePassportRecognizer,
    DeSocialSecurityRecognizer,
    DeTaxIdRecognizer,
    DeVatIdRecognizer,
    EmailRecognizer,
    EsNieRecognizer,
    EsNifRecognizer,
    FiPersonalIdentityCodeRecognizer,
    IbanRecognizer,
    InAadhaarRecognizer,
    InGstinRecognizer,
    InVehicleRegistrationRecognizer,
    IpRecognizer,
    ItFiscalCodeRecognizer,
    ItVatCodeRecognizer,
    KrBrnRecognizer,
    KrDriverLicenseRecognizer,
    KrFrnRecognizer,
    KrRrnRecognizer,
    MacAddressRecognizer,
    MedicalLicenseRecognizer,
    NgNinRecognizer,
    NhsRecognizer,
    PhTinRecognizer,
    PlPeselRecognizer,
    SeOrganisationsnummerRecognizer,
    SePersonnummerRecognizer,
    SgUenRecognizer,
    ThTninRecognizer,
    TrLicensePlateRecognizer,
    TrNationalIdRecognizer,
    UkDrivingLicenceRecognizer,
    UkVehicleRegistrationRecognizer,
    UsNpiRecognizer,
    UsSsnRecognizer,
    UuidRecognizer,
    ZaCompanyRegistrationRecognizer,
    ZaDriverLicenseRecognizer,
    ZaIdNumberRecognizer,
    ZaIncomeTaxNumberRecognizer,
    ZaLicensePlateRecognizer,
    ZaPassportRecognizer,
    ZaTrafficRegisterNumberRecognizer,
    ZaVatNumberRecognizer,
}

pub(super) fn is_candidate_accepted(validator: &Validator, value: &str) -> bool {
    validator.evaluate(&numerals::normalize(value)) != Some(false)
}

impl Validator {
    fn evaluate(&self, raw: &str) -> Option<bool> {
        use Validator::*;
        let value = clean(raw);
        let numbers = digits(&value);
        let is_numeric = value.len() == numbers.len();
        match self {
            None => Option::None,
            CreditCardRecognizer => Some(is_luhn_valid(&value)),
            CryptoRecognizer => Some(crypto::is_valid(raw)),
            EmailRecognizer => Some(international::is_email_valid(raw)),
            IbanRecognizer => Some(international::is_iban_valid(raw)),
            IpRecognizer => Some(international::is_ip_valid(raw)),
            MacAddressRecognizer => Some(international::is_mac_valid(raw)),
            UuidRecognizer => Some(international::is_uuid_valid(raw)),
            AuAbnRecognizer => Some(
                numbers.len() == 11 && is_numeric && {
                    let sum =
                        weighted(&numbers, &[10, 1, 3, 5, 7, 9, 11, 13, 15, 17, 19]) as i64 - 10;
                    sum.rem_euclid(89) == 0
                },
            ),
            AuAcnRecognizer => Some(
                numbers.len() == 9
                    && is_numeric
                    && (10 - weighted(&numbers[..8], &[8, 7, 6, 5, 4, 3, 2, 1]) % 10) % 10
                        == numbers[8],
            ),
            AuMedicareRecognizer => Some(
                numbers.len() == 10
                    && is_numeric
                    && weighted(&numbers[..8], &[1, 3, 7, 9, 1, 3, 7, 9]) % 10 == numbers[8],
            ),
            AuTfnRecognizer => Some(
                numbers.len() == 9
                    && is_numeric
                    && weighted(&numbers, &[1, 4, 3, 7, 5, 8, 6, 9, 10]).is_multiple_of(11),
            ),
            CaSinRecognizer => {
                Some(!(raw.contains('-') && raw.contains(' ')) && is_luhn_valid(&value))
            }
            FiPersonalIdentityCodeRecognizer => Some(europe::is_finnish_id_valid(raw)),
            DeBsnrRecognizer => {
                if numbers.len() == 9 && is_numeric && value != "000000000" {
                    Option::None
                } else {
                    Some(false)
                }
            }
            DeHealthInsuranceRecognizer => Some(europe::is_german_health_id_valid(raw)),
            DeIdCardRecognizer => europe::validate_german_document(raw, true),
            DePassportRecognizer => europe::validate_german_document(raw, false),
            DeLanrRecognizer => Some(
                numbers.len() == 9
                    && is_numeric
                    && (10 - weighted(&numbers[..6], &[4, 9, 4, 9, 4, 9]) % 10) % 10 == numbers[6],
            ),
            DeSocialSecurityRecognizer => Some(europe::is_german_social_id_valid(raw)),
            DeTaxIdRecognizer => Some(europe::is_german_tax_id_valid(raw)),
            DeVatIdRecognizer => europe::validate_german_vat(raw),
            InAadhaarRecognizer => Some(
                numbers.len() == 12
                    && is_numeric
                    && numbers[0] >= 2
                    && value.bytes().ne(value.bytes().rev())
                    && is_verhoeff_valid(&value),
            ),
            InGstinRecognizer => Some(asia::is_gstin_valid(raw)),
            InVehicleRegistrationRecognizer => asia::validate_india_vehicle_registration(raw),
            ItFiscalCodeRecognizer => europe::validate_italian_fiscal_code(raw),
            ItVatCodeRecognizer => {
                Some(value.len() == 11 && value != "00000000000" && is_luhn_valid(&value))
            }
            KrBrnRecognizer => Some(asia::is_korean_business_number_valid(raw)),
            KrDriverLicenseRecognizer => Some(asia::is_korean_driver_license_valid(raw)),
            KrRrnRecognizer => asia::validate_korean_registration(raw, false),
            KrFrnRecognizer => asia::validate_korean_registration(raw, true),
            NgNinRecognizer => Some(numbers.len() == 11 && is_numeric && is_verhoeff_valid(&value)),
            PhTinRecognizer => Some(
                matches!(numbers.len(), 9 | 12)
                    && is_numeric
                    && weighted(&numbers[..8], &[9, 8, 7, 6, 5, 4, 3, 2]) % 11 == numbers[8],
            ),
            PlPeselRecognizer => Some(
                numbers.len() == 11
                    && is_numeric
                    && (10 - weighted(&numbers[..10], &[1, 3, 7, 9, 1, 3, 7, 9, 1, 3]) % 10) % 10
                        == numbers[10],
            ),
            SgUenRecognizer => Some(asia::is_singapore_uen_valid(raw)),
            EsNieRecognizer => Some(europe::is_spanish_id_valid(raw, true)),
            EsNifRecognizer => Some(europe::is_spanish_id_valid(raw, false)),
            SeOrganisationsnummerRecognizer => Some(europe::is_swedish_id_valid(raw, true)),
            SePersonnummerRecognizer => Some(europe::is_swedish_id_valid(raw, false)),
            ThTninRecognizer => Some(
                numbers.len() == 13 && is_numeric && {
                    let sum = weighted(&numbers[..12], &[13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2]);
                    (11 - sum % 11) % 10 == numbers[12]
                },
            ),
            TrLicensePlateRecognizer => value
                .get(..2)
                .map(|prefix| (1..=81).contains(&number(prefix))),
            TrNationalIdRecognizer => Some(asia::is_turkish_id_valid(raw)),
            UkDrivingLicenceRecognizer => europe::validate_uk_driver_license(raw),
            UkVehicleRegistrationRecognizer => europe::validate_uk_vehicle_registration(raw),
            NhsRecognizer => Some(
                numbers.len() == 10
                    && is_numeric
                    && weighted(&numbers, &[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]).is_multiple_of(11),
            ),
            AbaRoutingRecognizer => Some(
                numbers.len() == 9
                    && is_numeric
                    && weighted(&numbers, &[3, 7, 1, 3, 7, 1, 3, 7, 1]).is_multiple_of(10),
            ),
            MedicalLicenseRecognizer => Some(
                value.len() == 9 && {
                    let numbers = digits(&value[2..]);
                    numbers.len() == 7
                        && weighted(&numbers[..6], &[1, 2, 1, 2, 1, 2]) % 10 == numbers[6]
                },
            ),
            UsNpiRecognizer => Some(
                numbers.len() == 10
                    && is_numeric
                    && !numbers[..9].iter().all(|digit| *digit == numbers[0])
                    && is_luhn_valid(&format!("80840{value}")),
            ),
            UsSsnRecognizer => Some(is_ssn_valid(raw)),
            ZaCompanyRegistrationRecognizer => Some(africa::is_company_registration_valid(raw)),
            ZaDriverLicenseRecognizer => Some(
                (8..=15).contains(&value.len()) && value.bytes().any(|c| c.is_ascii_alphabetic()),
            ),
            ZaIdNumberRecognizer => Some(africa::is_south_african_id_valid(raw)),
            ZaIncomeTaxNumberRecognizer => {
                Some(value.len() == 10 && is_numeric && b"01239".contains(&value.as_bytes()[0]))
            }
            ZaLicensePlateRecognizer => Some(africa::is_licence_plate_valid(raw)),
            ZaPassportRecognizer => Some(
                value.len() == 9
                    && value.is_ascii()
                    && b"ADMT".contains(&value.as_bytes()[0])
                    && value.as_bytes()[1..].iter().all(u8::is_ascii_digit),
            ),
            ZaTrafficRegisterNumberRecognizer => {
                Some(value.len() == 13 && is_numeric && !africa::is_south_african_id_valid(raw))
            }
            ZaVatNumberRecognizer => {
                Some(value.len() == 10 && is_numeric && value.starts_with('4'))
            }
        }
    }
}

fn is_ssn_valid(raw: &str) -> bool {
    let value = clean(raw);
    if value.len() != 9 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let delimiter_count = ['.', '-', ' '].iter().filter(|&&c| raw.contains(c)).count();
    delimiter_count <= 1
        && !value.bytes().all(|c| c == value.as_bytes()[0])
        && !matches!(&value[..3], "000" | "666")
        && &value[3..5] != "00"
        && &value[5..] != "0000"
        && !matches!(value.as_str(), "123456789" | "987654320" | "078051120")
}
