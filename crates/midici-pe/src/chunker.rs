//! Split PE header+body across SysEx-sized chunks. // M2-101 §8.3 / M2-103 §5.2
//!
//! ## Example
//!
//! ```
//! // Split a header+body into PE chunk payloads for a 128-byte negotiated SysEx.
//! let chunks = midici_pe::split(1, br#"{"status":200}"#, &[b'x'; 500], 128).unwrap();
//! assert!(chunks.len() > 1, "500 property bytes don't fit one 128-byte SysEx");
//! // Each chunk is a self-describing PE payload (requestId + framing + data).
//! for c in &chunks {
//!     let chunk = midici_pe::PeChunk::decode(c).unwrap();
//!     assert_eq!(chunk.request_id, 1);
//! }
//! ```

use alloc::vec::Vec;

use midici_core::spec::CI_HEADER_LEN;

use crate::error::PeError;
use crate::frame::{PeChunk, PE_FRAMING_LEN, REQUEST_ID_MAX};
use crate::wire::U14_MAX;

/// Inclusive lower bound for negotiated max SysEx size (bytes, F0..=F7). // M2-101 §5.5.3 / ARD §4
pub const MAX_SYSEX_MIN: u32 = 128;
/// Inclusive upper clamp applied by the chunker. // ARD §4
pub const MAX_SYSEX_MAX: u32 = 4096;

/// Clamp a Discovery Receivable Maximum SysEx Message Size into the chunker range.
#[inline]
pub fn clamp_max_sysex(negotiated: u32) -> u32 {
    negotiated.clamp(MAX_SYSEX_MIN, MAX_SYSEX_MAX)
}

/// Bytes available for PE fields inside a full SysEx of `max_sysex` bytes (F0 through F7).
///
/// Layout: `F0` + CI header (`CI_HEADER_LEN`) + PE payload + `F7`.
#[inline]
pub fn pe_payload_budget(max_sysex: u32) -> usize {
    let max_sysex = clamp_max_sysex(max_sysex);
    (max_sysex as usize).saturating_sub(2 + CI_HEADER_LEN)
}

/// Maximum property bytes that fit in the first chunk alongside `header`.
#[inline]
pub fn first_chunk_property_capacity(max_sysex: u32, header_len: usize) -> Result<usize, PeError> {
    let budget = pe_payload_budget(max_sysex);
    let overhead = PE_FRAMING_LEN + header_len;
    if overhead > budget {
        return Err(PeError::HeaderTooLarge);
    }
    Ok(budget - overhead)
}

/// Maximum property bytes in chunk 2+ (header length is zero). // M2-101 §8.3.1
#[inline]
pub fn later_chunk_property_capacity(max_sysex: u32) -> usize {
    pe_payload_budget(max_sysex).saturating_sub(PE_FRAMING_LEN)
}

/// Split `header` + `body` into PE chunk payloads sized for `max_sysex`.
///
/// - Clamps `max_sysex` to `128..=4096`.
/// - Places the entire header in chunk 1 only. // M2-101 §8.3.1
/// - Header-only messages use `numChunks = chunkNum = 1`. // M2-101 §8.3.2
///
/// Returns the encoded PE payloads (CI header not included).
pub fn split(
    request_id: u8,
    header: &[u8],
    body: &[u8],
    max_sysex: u32,
) -> Result<Vec<Vec<u8>>, PeError> {
    if request_id > REQUEST_ID_MAX {
        return Err(PeError::BadField);
    }
    if header.iter().any(|b| *b > 0x7F) {
        return Err(PeError::BadField);
    }
    if header.len() > U14_MAX as usize {
        return Err(PeError::HeaderTooLarge);
    }

    let max_sysex = clamp_max_sysex(max_sysex);
    let first_cap = first_chunk_property_capacity(max_sysex, header.len())?;

    // Header-only: must not multi-chunk. // M2-101 §8.3.2 / M2-103 §5.2
    if body.is_empty() {
        let chunk = PeChunk {
            request_id,
            header: header.to_vec(),
            num_chunks: 1,
            chunk_num: 1,
            property: Vec::new(),
        };
        return Ok(alloc::vec![chunk.to_vec()?]);
    }

    let later_cap = later_chunk_property_capacity(max_sysex);
    if later_cap == 0 {
        return Err(PeError::HeaderTooLarge);
    }

    let first_len = body.len().min(first_cap);
    let remaining = body.len() - first_len;
    let later_chunks = remaining.div_ceil(later_cap);
    let num_chunks = 1 + later_chunks;
    if num_chunks > U14_MAX as usize {
        return Err(PeError::Oversize);
    }
    let num_chunks_u = num_chunks as u16;

    let mut out = Vec::with_capacity(num_chunks);
    let mut offset = 0usize;

    let first_prop = &body[offset..offset + first_len];
    offset += first_len;
    out.push(
        PeChunk {
            request_id,
            header: header.to_vec(),
            num_chunks: num_chunks_u,
            chunk_num: 1,
            property: first_prop.to_vec(),
        }
        .to_vec()?,
    );

    let mut chunk_num: u16 = 2;
    while offset < body.len() {
        let take = (body.len() - offset).min(later_cap);
        let prop = &body[offset..offset + take];
        offset += take;
        if prop.len() > U14_MAX as usize {
            return Err(PeError::Oversize);
        }
        out.push(
            PeChunk {
                request_id,
                header: Vec::new(),
                num_chunks: num_chunks_u,
                chunk_num,
                property: prop.to_vec(),
            }
            .to_vec()?,
        );
        chunk_num = chunk_num.saturating_add(1);
    }

    debug_assert_eq!(out.len(), num_chunks);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::PeChunk;

    #[test]
    fn clamp_range() {
        assert_eq!(clamp_max_sysex(64), 128);
        assert_eq!(clamp_max_sysex(512), 512);
        assert_eq!(clamp_max_sysex(10_000), 4096);
    }

    #[test]
    fn header_only_is_single_chunk() {
        let chunks = split(1, b"{\"resource\":\"DeviceInfo\"}", &[], 128).unwrap();
        assert_eq!(chunks.len(), 1);
        let c = PeChunk::decode(&chunks[0]).unwrap();
        assert_eq!(c.num_chunks, 1);
        assert_eq!(c.chunk_num, 1);
        assert!(c.property.is_empty());
    }

    #[test]
    fn each_chunk_fits_budget() {
        let header = b"{\"status\":200}";
        let body = alloc::vec![0x20u8; 3000];
        for size in [128u32, 256, 512, 1024, 4096] {
            let budget = pe_payload_budget(size);
            let chunks = split(3, header, &body, size).unwrap();
            for raw in &chunks {
                assert!(
                    raw.len() <= budget,
                    "size={size} len={} budget={budget}",
                    raw.len()
                );
                let _ = PeChunk::decode(raw).unwrap();
            }
        }
    }
}
