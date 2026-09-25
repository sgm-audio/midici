//! Mcoded7: 8-bit ↔ 7-bit encoding for PE Property Data. // M2-103 §6.1.7
//!
//! ## Example
//!
//! ```
//! let raw = b"binary \x00\xFF ok";
//! let wire = midici_pe::mcoded7_encode(raw);
//! assert!(wire.iter().all(|b| *b <= 0x7F)); // SysEx-safe
//! assert_eq!(midici_pe::mcoded7_decode(&wire).unwrap(), raw);
//! ```

use alloc::vec::Vec;

use crate::error::PeError;

/// Encode 8-bit octets into Mcoded7 7-bit SysEx-safe bytes. // M2-103 §6.1.7
///
/// Groups of seven input bytes become eight output bytes: the first byte holds the
/// high bits (`0ABCDEFG`), followed by the low-seven-bit payloads. A short final
/// group pads unused high-bit positions with zero.
pub fn encode(input: &[u8]) -> Vec<u8> {
    let groups = input.len() / 7;
    let rem = input.len() % 7;
    let out_len = groups * 8 + if rem == 0 { 0 } else { rem + 1 };
    let mut out = Vec::with_capacity(out_len);
    let mut i = 0;
    while i + 7 <= input.len() {
        encode_group(&input[i..i + 7], &mut out);
        i += 7;
    }
    if rem != 0 {
        encode_group(&input[i..], &mut out);
    }
    out
}

/// Decode Mcoded7 bytes back to 8-bit octets. // M2-103 §6.1.7
pub fn decode(input: &[u8]) -> Result<Vec<u8>, PeError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let remaining = input.len() - i;
        if remaining == 1 {
            // A lone high-bits byte with no data bytes is not a valid group.
            return Err(PeError::BadMcoded7);
        }
        // Full groups are 8 bytes; a short final group is 2..=8 bytes (1 high + 1..=7 data).
        let group_len = if remaining >= 8 { 8 } else { remaining };
        decode_group(&input[i..i + group_len], &mut out)?;
        i += group_len;
    }
    Ok(out)
}

fn encode_group(data: &[u8], out: &mut Vec<u8>) {
    debug_assert!(!data.is_empty() && data.len() <= 7);
    let mut hi = 0u8;
    for (idx, b) in data.iter().enumerate() {
        if b & 0x80 != 0 {
            hi |= 1 << (6 - idx);
        }
    }
    out.push(hi);
    for b in data {
        out.push(b & 0x7F);
    }
}

fn decode_group(group: &[u8], out: &mut Vec<u8>) -> Result<(), PeError> {
    // group[0] = high bits; group[1..] = low 7-bit payloads (1..=7 bytes).
    if group.len() < 2 || group.len() > 8 {
        return Err(PeError::BadMcoded7);
    }
    for b in group {
        if *b > 0x7F {
            return Err(PeError::BadField);
        }
    }
    let hi = group[0];
    let data = &group[1..];
    // Unused high-bit positions (for short groups) must be zero. // M2-103 §6.1.7 example
    let used_mask = match data.len() {
        1 => 0b0100_0000,
        2 => 0b0110_0000,
        3 => 0b0111_0000,
        4 => 0b0111_1000,
        5 => 0b0111_1100,
        6 => 0b0111_1110,
        7 => 0b0111_1111,
        _ => return Err(PeError::BadMcoded7),
    };
    if hi & !used_mask != 0 {
        return Err(PeError::BadMcoded7);
    }
    for (idx, b) in data.iter().enumerate() {
        let mut v = *b;
        if hi & (1 << (6 - idx)) != 0 {
            v |= 0x80;
        }
        out.push(v);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_roundtrip() {
        assert_eq!(encode(&[]), Vec::<u8>::new());
        assert_eq!(decode(&[]).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn full_group_high_bits() {
        // M2-103 §6.1.7 layout: seven bytes → 0ABCDEFG then seven low-7 payloads.
        let input = [0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80];
        let enc = encode(&input);
        assert_eq!(enc.len(), 8);
        assert_eq!(enc[0], 0b0101_0101);
        assert_eq!(decode(&enc).unwrap(), input);
    }

    #[test]
    fn short_group_three_bytes() {
        // M2-103 §6.1.7: AAAAaaaa BBBBbbbb CCCCcccc → 0ABC0000 then three low-7 bytes.
        let input = [0x80, 0x40, 0xC0];
        let enc = encode(&input);
        assert_eq!(enc, [0b0101_0000, 0x00, 0x40, 0x40]);
        assert_eq!(decode(&enc).unwrap(), input);
    }

    #[test]
    fn seven_byte_mixed_high_bits() {
        // M2-103 §6.1.7: high-bits byte is 0ABCDEFG (A = MSB of first octet).
        let original = [0x00, 0xFF, 0x80, 0x7F, 0x01, 0x02, 0x03];
        let encoded = encode(&original);
        assert_eq!(
            encoded,
            [0b0011_0000, 0x00, 0x7F, 0x00, 0x7F, 0x01, 0x02, 0x03]
        );
        assert_eq!(decode(&encoded).unwrap(), original);
    }
}
