use super::checksums::*;

pub(super) fn is_south_african_id_valid(value: &str) -> bool {
    if value.len() != 13 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return false;
    }
    if !b"012".contains(&value.as_bytes()[10]) || !b"89".contains(&value.as_bytes()[11]) {
        return false;
    }
    let today = time::OffsetDateTime::now_utc().date();
    let suffix = number(&value[..2]) as i32;
    let year = (if suffix > today.year() % 100 {
        1900
    } else {
        2000
    }) + suffix;
    date(year, number(&value[2..4]), number(&value[4..6])).is_some_and(|birth| birth <= today)
        && is_luhn_valid(value)
}

pub(super) fn is_company_registration_valid(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    let parts: Vec<_> = value.split('/').collect();
    let year = match parts.as_slice() {
        [year, sequence, kind]
            if year.len() == 4
                && sequence.len() == 6
                && kind.len() == 2
                && sequence.bytes().all(|c| c.is_ascii_digit())
                && kind.bytes().all(|c| c.is_ascii_digit()) =>
        {
            number(year)
        }
        [prefix, sequence]
            if sequence.len() == 6 && sequence.bytes().all(|c| c.is_ascii_digit()) =>
        {
            let Some(year) = ["CK", "NR", "K", "T", "W", "B", "M", "N"]
                .iter()
                .find_map(|part| prefix.strip_prefix(part).filter(|year| year.len() == 4))
            else {
                return false;
            };
            number(year)
        }
        _ => return false,
    };
    (1800..=time::OffsetDateTime::now_utc().year() as u32).contains(&year)
}

pub(super) fn is_licence_plate_valid(value: &str) -> bool {
    let value = clean(value);
    value.len() >= 5
        && value.is_ascii()
        && ["GP", "ZN", "WP", "EC", "NC", "FS", "LP", "MP", "NW"]
            .contains(&&value[value.len() - 2..])
        && value[..value.len() - 2]
            .bytes()
            .any(|c| c.is_ascii_alphabetic())
}
