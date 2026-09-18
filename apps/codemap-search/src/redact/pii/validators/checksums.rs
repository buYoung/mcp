//! Shared arithmetic and canonicalization for identifier validation.
pub(super) fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '_' | ':' | '.'))
        .flat_map(char::to_uppercase)
        .collect()
}

pub(super) fn digits(value: &str) -> Vec<u32> {
    value.chars().filter_map(|c| c.to_digit(10)).collect()
}

pub(super) fn weighted(digits: &[u32], weights: &[u32]) -> u32 {
    digits
        .iter()
        .zip(weights)
        .map(|(digit, weight)| digit * weight)
        .sum()
}

pub(super) fn is_luhn_valid(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_digit())
        && value
            .bytes()
            .rev()
            .enumerate()
            .map(|(index, digit)| {
                let n = (digit - b'0') as u32 * if index % 2 == 1 { 2 } else { 1 };
                if n > 9 {
                    n - 9
                } else {
                    n
                }
            })
            .sum::<u32>()
            % 10
            == 0
}

pub(super) fn mod_11_10(digits: &[u32]) -> u32 {
    let product = digits.iter().fold(10, |product, digit| {
        let sum = (product + digit) % 10;
        (if sum == 0 { 10 } else { sum }) * 2 % 11
    });
    (11 - product) % 10
}

pub(super) fn is_verhoeff_valid(value: &str) -> bool {
    const MULTIPLICATION_TABLE: [[usize; 10]; 10] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
        [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
        [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
        [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
        [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
        [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
        [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
        [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
        [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    ];
    const PERMUTATION_TABLE: [[usize; 10]; 8] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
        [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
        [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
        [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
        [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
        [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
        [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
    ];
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    value
        .bytes()
        .rev()
        .enumerate()
        .fold(0, |check, (index, digit)| {
            MULTIPLICATION_TABLE[check][PERMUTATION_TABLE[index % 8][(digit - b'0') as usize]]
        })
        == 0
}

pub(super) fn date(year: i32, month: u32, day: u32) -> Option<time::Date> {
    let month = time::Month::try_from(u8::try_from(month).ok()?).ok()?;
    time::Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok()
}

pub(super) fn number(value: &str) -> u32 {
    value.parse().unwrap_or(u32::MAX)
}
