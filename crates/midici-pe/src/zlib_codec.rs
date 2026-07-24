//! `zlib+Mcoded7` mutualEncoding. // M2-103 §6.1.6 Table 12 / §6.2.2

use alloc::vec::Vec;

use crate::error::PeError;
use crate::mcoded7;

/// Compress with zlib (RFC 1950) then Mcoded7-encode. // M2-103 §6.2.2
pub fn encode(input: &[u8]) -> Result<Vec<u8>, PeError> {
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(input, 6);
    Ok(mcoded7::encode(&compressed))
}

/// Mcoded7-decode then zlib-decompress. // M2-103 §6.2.2
pub fn decode(input: &[u8]) -> Result<Vec<u8>, PeError> {
    let coded = mcoded7::decode(input)?;
    miniz_oxide::inflate::decompress_to_vec_zlib(&coded).map_err(|_| PeError::Zlib)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zlib_mcoded7_roundtrip() {
        let data = b"{\"resource\":\"DeviceInfo\"}";
        let enc = encode(data).unwrap();
        assert!(enc.iter().all(|b| *b <= 0x7F));
        assert_eq!(decode(&enc).unwrap(), data);
    }
}
