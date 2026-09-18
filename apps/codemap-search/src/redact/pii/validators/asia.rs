use super::checksums::*;
use serde_json::Value;
use std::sync::OnceLock;

fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../validation_data.json")).unwrap())
}

pub(super) fn is_gstin_valid(value: &str) -> bool {
    let value = clean(value);
    if value.len() != 15 || !value.is_ascii() {
        return false;
    }
    let bytes = value.as_bytes();
    (1..=37).contains(&number(&value[..2]))
        && bytes[2..7]
            .iter()
            .filter(|c| c.is_ascii_alphabetic())
            .count()
            >= 3
        && bytes[7..11].iter().all(u8::is_ascii_digit)
        && bytes[11].is_ascii_alphabetic()
        && bytes[12].is_ascii_alphanumeric()
        && bytes[13] == b'Z'
        && bytes[14].is_ascii_alphanumeric()
}

pub(super) fn validate_india_vehicle_registration(value: &str) -> Option<bool> {
    let value = clean(value);
    if value.len() < 8 || !value.is_ascii() {
        return None;
    }
    let profile = &data()["InVehicleRegistrationRecognizer"];
    let prefix = &value[..2];
    let is_known_prefix = profile["two_factor_registration_prefix"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry.as_str() == Some(prefix));
    if !is_known_prefix {
        return None;
    }
    if value.as_bytes()[2].is_ascii_digit() {
        let district = if value.as_bytes()[3].is_ascii_digit() {
            &value[2..4]
        } else {
            &value[2..3]
        };
        let serial = number(&value[value.len() - 4..]);
        let districts = &profile["state_rto_district_map"][prefix];
        let unpadded = number(district).to_string();
        if (1..=9999).contains(&serial)
            && districts.as_array().is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .as_str()
                        .is_some_and(|entry| entry == district || entry == unpadded)
                })
            })
        {
            return Some(true);
        }
    }
    // Upstream retains other matching formats as unverified, rather than rejecting them.
    None
}

pub(super) fn validate_korean_registration(value: &str, is_foreigner: bool) -> Option<bool> {
    let value = clean(value);
    let numbers = digits(&value);
    if numbers.len() != 13 || value.len() != 13 {
        return Some(false);
    }
    let sum = weighted(&numbers[..12], &[2, 3, 4, 5, 6, 7, 8, 9, 2, 3, 4, 5]);
    let modulus = if is_foreigner { 13 } else { 11 };
    let is_valid = number(&value[7..9]) <= 95 && (modulus - sum % 11) % 10 == numbers[12];
    // Newer registrations use random suffixes: failed legacy checksums stay candidates.
    is_valid.then_some(true)
}

pub(super) fn is_korean_business_number_valid(value: &str) -> bool {
    let numbers = digits(value);
    if numbers.len() != 10 {
        return false;
    }
    let last = numbers[8] * 5;
    let sum = weighted(&numbers[..8], &[1, 3, 7, 1, 3, 7, 1, 3]) + last + last / 10;
    (10 - sum % 10) % 10 == numbers[9]
}

pub(super) fn is_korean_driver_license_valid(value: &str) -> bool {
    let value = clean(value);
    value.len() == 12
        && value.bytes().all(|c| c.is_ascii_digit())
        && data()["KrDriverLicenseRecognizer"]["REGION_CODES"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry.as_str() == value.get(..2))
}

pub(super) fn is_singapore_uen_valid(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    if !value.is_ascii() || !matches!(value.len(), 9 | 10) {
        return false;
    }
    let profile = &data()["SgUenRecognizer"];
    let body = &value[..value.len() - 1];
    let check = value.as_bytes()[value.len() - 1];
    if value.len() == 9 {
        let numbers = digits(body);
        numbers.len() == 8
            && b"XMKECAWLJDB"[(weighted(&numbers, &[10, 4, 9, 3, 8, 2, 7, 1]) % 11) as usize]
                == check
    } else if value.as_bytes()[0].is_ascii_digit() {
        if number(&value[..4]) > time::OffsetDateTime::now_utc().year() as u32 {
            return false;
        }
        let numbers = digits(body);
        numbers.len() == 9
            && b"ZKCMDNERGWH"[(weighted(&numbers, &[10, 8, 6, 4, 9, 7, 5, 3, 1]) % 11) as usize]
                == check
    } else {
        if !b"TSR".contains(&value.as_bytes()[0])
            || !profile["UEN_FORMAT_C_ENTITY_TYPE"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry.as_str() == Some(&value[3..5]))
        {
            return false;
        }
        let alphabet = b"ABCDEFGHJKLMNPQRSTUVWX0123456789";
        let Some(numbers): Option<Vec<u32>> = body
            .bytes()
            .map(|c| alphabet.iter().position(|&v| v == c).map(|n| n as u32))
            .collect()
        else {
            return false;
        };
        let index = (weighted(&numbers, &[4, 3, 5, 3, 10, 2, 2, 5, 7]) as i32 - 5).rem_euclid(11);
        alphabet[index as usize] == check
    }
}

pub(super) fn is_turkish_id_valid(value: &str) -> bool {
    let numbers = digits(value);
    if numbers.len() != 11 || numbers[0] == 0 {
        return false;
    }
    let odd = (0..9).step_by(2).map(|index| numbers[index]).sum::<u32>();
    let even = (1..8).step_by(2).map(|index| numbers[index]).sum::<u32>();
    (odd as i32 * 7 - even as i32).rem_euclid(10) as u32 == numbers[9]
        && numbers[..10].iter().sum::<u32>() % 10 == numbers[10]
}
