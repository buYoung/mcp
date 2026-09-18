//! Bitcoin address checksums from Presidio's CryptoRecognizer.
use sha2::{Digest, Sha256};

pub(super) fn is_valid(value: &str) -> bool {
    if value.starts_with('1') || value.starts_with('3') {
        let Some(decoded) = decode_base58(value) else {
            return false;
        };
        if decoded.len() < 4 {
            return false;
        }
        let split = decoded.len() - 4;
        let digest = Sha256::digest(Sha256::digest(&decoded[..split]));
        decoded[split..] == digest[..4]
    } else if value.starts_with("bc1") {
        is_bech32_valid(value)
    } else {
        false
    }
}

fn decode_base58(value: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut bytes = Vec::<u8>::new();
    for byte in value.bytes() {
        let mut carry = ALPHABET.iter().position(|&candidate| candidate == byte)? as u32;
        for digit in &mut bytes {
            carry += *digit as u32 * 58;
            *digit = carry as u8;
            carry >>= 8;
        }
        while carry != 0 {
            bytes.push(carry as u8);
            carry >>= 8;
        }
    }
    bytes.extend(std::iter::repeat_n(
        0,
        value.bytes().take_while(|&c| c == b'1').count(),
    ));
    bytes.reverse();
    Some(bytes)
}

fn is_bech32_valid(value: &str) -> bool {
    const ALPHABET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    const GENERATORS: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
    if value.len() > 90
        || !value.bytes().all(|c| (33..=126).contains(&c))
        || (value != value.to_ascii_lowercase() && value != value.to_ascii_uppercase())
    {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    let Some((prefix, data)) = lower.rsplit_once('1') else {
        return false;
    };
    if prefix.is_empty() || data.len() < 6 {
        return false;
    }
    let Some(data): Option<Vec<u8>> = data
        .bytes()
        .map(|c| ALPHABET.iter().position(|&v| v == c).map(|n| n as u8))
        .collect()
    else {
        return false;
    };
    let values = prefix
        .bytes()
        .map(|c| c >> 5)
        .chain(std::iter::once(0))
        .chain(prefix.bytes().map(|c| c & 31))
        .chain(data);
    let check = values.fold(1u32, |check, value| {
        let top = check >> 25;
        let mut next = ((check & 0x1ffffff) << 5) ^ value as u32;
        for (index, generator) in GENERATORS.iter().enumerate() {
            if (top >> index) & 1 != 0 {
                next ^= generator;
            }
        }
        next
    });
    matches!(check, 1 | 0x2bc830a3)
}
