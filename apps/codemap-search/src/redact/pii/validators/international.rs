use super::checksums::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::OnceLock;

fn iban_formats() -> &'static BTreeMap<String, Regex> {
    static FORMATS: OnceLock<BTreeMap<String, Regex>> = OnceLock::new();
    FORMATS.get_or_init(|| {
        let raw: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../iban_formats.json")).unwrap();
        // Preserve the upstream default exact_match=false: checksum validation still
        // covers the entire candidate, while the country format matches its prefix.
        raw.into_iter()
            .map(|(country, pattern)| (country, Regex::new(&format!("\\A(?:{pattern})")).unwrap()))
            .collect()
    })
}

pub(in crate::redact::pii) fn has_exact_iban_length(value: &str) -> bool {
    let value = clean(value);
    value
        .get(..2)
        .and_then(|country| iban_formats().get(country))
        .and_then(|format| format.find(&value))
        .is_some_and(|found| found.end() == value.len())
}

pub(super) fn is_iban_valid(value: &str) -> bool {
    let value = clean(value);
    if value.len() < 4 || !value.is_ascii() {
        return false;
    }
    if !iban_formats()
        .get(&value[..2])
        .is_some_and(|format| format.is_match(&value))
    {
        return false;
    }
    let mut remainder = 0u32;
    for c in value[4..].bytes().chain(value[..4].bytes()) {
        if c.is_ascii_digit() {
            remainder = (remainder * 10 + (c - b'0') as u32) % 97;
        } else if c.is_ascii_uppercase() {
            remainder = (remainder * 100 + (c - b'A' + 10) as u32) % 97;
        } else {
            return false;
        }
    }
    remainder == 1
}

pub(super) fn is_ip_valid(value: &str) -> bool {
    let (host, prefix) = value
        .split_once('/')
        .map_or((value, None), |(host, prefix)| (host, Some(prefix)));
    let host = host.split_once('%').map_or(host, |(host, _)| host);
    let Ok(address) = host.parse::<IpAddr>() else {
        return false;
    };
    prefix.is_none_or(|prefix| {
        prefix
            .parse::<u8>()
            .is_ok_and(|length| length <= if address.is_ipv4() { 32 } else { 128 })
    })
}

pub(super) fn is_email_valid(value: &str) -> bool {
    // Keep host validation local. No public-suffix downloads or tldextract runtime.
    let Some((local, host)) = value.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && host.contains('.')
        && host
            .split('.')
            .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
}

pub(super) fn is_mac_valid(value: &str) -> bool {
    let value = clean(value);
    value.len() == 12
        && value.bytes().all(|c| c.is_ascii_hexdigit())
        && value != "FFFFFFFFFFFF"
        && value != "000000000000"
}

pub(super) fn is_uuid_valid(value: &str) -> bool {
    value
        .as_bytes()
        .get(14)
        .is_some_and(|c| (b'1'..=b'8').contains(c))
        && value
            .as_bytes()
            .get(19)
            .is_some_and(|c| matches!(c.to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b'))
}
