use super::checksums::*;

pub(super) fn is_finnish_id_valid(value: &str) -> bool {
    if value.len() != 11 || !value.is_ascii() {
        return false;
    }
    let century = match value.as_bytes()[6] {
        b'+' => 1800,
        b'-' | b'Y' | b'X' | b'W' | b'V' | b'U' => 1900,
        _ => 2000,
    };
    if date(
        century + number(&value[4..6]) as i32,
        number(&value[2..4]),
        number(&value[..2]),
    )
    .is_none()
    {
        return false;
    }
    let number = format!("{}{}", &value[..6], &value[7..10]);
    let Ok(number) = number.parse::<usize>() else {
        return false;
    };
    b"0123456789ABCDEFHJKLMNPRSTUVWXY"[number % 31] == value.as_bytes()[10].to_ascii_uppercase()
}

pub(super) fn is_german_health_id_valid(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || !bytes[0].is_ascii_uppercase()
        || !bytes[1..].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let expanded = format!("{:02}{}", bytes[0] - b'A' + 1, &value[1..9]);
    digits(&expanded)
        .iter()
        .zip([1, 2, 1, 2, 1, 2, 1, 2, 1, 2])
        .map(|(digit, weight)| {
            let product = digit * weight;
            product / 10 + product % 10
        })
        .sum::<u32>()
        % 10
        == (bytes[9] - b'0') as u32
}

pub(super) fn validate_german_document(value: &str, is_identity_card: bool) -> Option<bool> {
    let value = value.to_ascii_uppercase();
    let bytes = value.as_bytes();
    if bytes.len() != 9 || !bytes[8].is_ascii_digit() {
        return Some(false);
    }
    if is_identity_card && bytes[0] == b'T' && bytes[1..].iter().all(u8::is_ascii_digit) {
        return None;
    }
    if !is_identity_card && bytes[..8].iter().any(|c| b"ABDEIOQSU".contains(c)) {
        return Some(false);
    }
    let mut sum = 0;
    for (index, &c) in bytes[..8].iter().enumerate() {
        let digit = if c.is_ascii_digit() {
            c - b'0'
        } else if c.is_ascii_uppercase() {
            c - b'A' + 10
        } else {
            return Some(false);
        };
        sum += digit as u32 * [7, 3, 1][index % 3];
    }
    Some(sum % 10 == (bytes[8] - b'0') as u32)
}

pub(super) fn is_german_social_id_valid(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    if value.len() != 12 || !value.is_ascii() || !value.as_bytes()[8].is_ascii_uppercase() {
        return false;
    }
    let day = number(&value[2..4]);
    if !((1..=31).contains(&day) || (51..=81).contains(&day))
        || !(1..=12).contains(&number(&value[4..6]))
    {
        return false;
    }
    let expanded = format!(
        "{}{:02}{}",
        &value[..8],
        value.as_bytes()[8] - b'A' + 1,
        &value[9..11]
    );
    let numbers = digits(&expanded);
    numbers.len() == 12
        && numbers
            .iter()
            .zip([2, 1, 2, 5, 7, 1, 2, 1, 2, 1, 2, 1])
            .map(|(digit, weight)| {
                let product = digit * weight;
                product / 10 + product % 10
            })
            .sum::<u32>()
            % 10
            == number(&value[11..])
}

pub(super) fn is_german_tax_id_valid(value: &str) -> bool {
    let numbers = digits(value);
    if numbers.len() != 11 || numbers[0] == 0 {
        return false;
    }
    let mut counts = [0; 10];
    for digit in &numbers[..10] {
        counts[*digit as usize] += 1;
    }
    counts.iter().all(|&count| count <= 3) && mod_11_10(&numbers[..10]) == numbers[10]
}

pub(super) fn validate_german_vat(value: &str) -> Option<bool> {
    let value = clean(value);
    if value.len() != 11 || !value.starts_with("DE") {
        return Some(false);
    }
    let numbers = digits(&value[2..]);
    if numbers.len() != 9 {
        return Some(false);
    }
    // Upstream's default strict_checksum=false keeps an unverified candidate.
    (mod_11_10(&numbers[..8]) == numbers[8]).then_some(true)
}

pub(super) fn validate_italian_fiscal_code(value: &str) -> Option<bool> {
    let value = value.to_ascii_uppercase();
    if value.len() != 16 || !value.is_ascii() {
        return Some(false);
    }
    const ODD: [u32; 26] = [
        1, 0, 5, 7, 9, 13, 15, 17, 19, 21, 2, 4, 18, 20, 11, 3, 6, 8, 12, 14, 16, 10, 22, 25, 24,
        23,
    ];
    let mut sum = 0;
    for (index, c) in value.bytes().take(15).enumerate() {
        let n = if c.is_ascii_digit() {
            c - b'0'
        } else if c.is_ascii_uppercase() {
            c - b'A'
        } else {
            return Some(false);
        };
        sum += if index % 2 == 0 {
            ODD[n as usize]
        } else {
            n as u32
        };
    }
    // A checksum mismatch is not invalidated by the upstream recognizer.
    (b'A' + (sum % 26) as u8 == value.as_bytes()[15]).then_some(true)
}

pub(super) fn is_spanish_id_valid(value: &str, is_foreigner: bool) -> bool {
    let value = clean(value);
    if value.is_empty() || !value.is_ascii() {
        return false;
    }
    let body = if is_foreigner {
        if !(8..=9).contains(&value.len()) {
            return false;
        }
        let Some(prefix) = b"XYZ".iter().position(|&c| c == value.as_bytes()[0]) else {
            return false;
        };
        format!("{prefix}{}", &value[1..value.len() - 1])
    } else {
        value[..value.len() - 1].to_string()
    };
    let Ok(number) = body.parse::<u64>() else {
        return false;
    };
    b"TRWAGMYFPDXBNJZSQVHLCKE"[(number % 23) as usize] == *value.as_bytes().last().unwrap()
}

pub(super) fn is_swedish_id_valid(value: &str, is_organization: bool) -> bool {
    let value: String = value.chars().filter(char::is_ascii_digit).collect();
    let value = if is_organization {
        value.as_str()
    } else {
        value.get(value.len().saturating_sub(10)..).unwrap_or("")
    };
    if value.len() != 10 {
        return false;
    }
    if is_organization {
        if value.as_bytes()[2] < b'2' {
            return false;
        }
    } else {
        let mut day = number(&value[4..6]);
        if day >= 61 {
            day -= 60;
        }
        if !(1..=12).contains(&number(&value[2..4])) || !(1..=31).contains(&day) {
            return false;
        }
    }
    is_luhn_valid(value)
}

pub(super) fn validate_uk_driver_license(value: &str) -> Option<bool> {
    let Some(surname) = value.get(..5) else {
        return Some(false);
    };
    let letters = surname.trim_end_matches('9');
    if letters.is_empty() || !letters.bytes().all(|c| c.is_ascii_alphabetic()) {
        Some(false)
    } else {
        None
    }
}

pub(super) fn validate_uk_vehicle_registration(value: &str) -> Option<bool> {
    let value = clean(value);
    if value.len() == 7 && value.as_bytes()[..2].iter().all(u8::is_ascii_alphabetic) {
        if let Ok(age) = value[2..4].parse::<u32>() {
            return Some((2..=29).contains(&age) || (51..=79).contains(&age));
        }
    }
    None
}
