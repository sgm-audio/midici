//! PE chunk payload framing (after the fixed MIDI-CI header).
//! // M2-101 Tables 33–35 / M2-103 §3.3

use alloc::vec::Vec;

use crate::error::PeError;
use crate::wire::{read_u14_le7, write_u14_le7, U14_MAX};

/// Fixed PE framing bytes excluding variable header/property payloads:
/// `requestId(1) + nh(2) + numChunks(2) + chunkNum(2) + nd(2)`.
pub const PE_FRAMING_LEN: usize = 1 + 2 + 2 + 2 + 2;

/// Request ID is a 7-bit value `0..=127`. // M2-103 §5.3
pub const REQUEST_ID_MAX: u8 = 0x7F;

/// Decode the fixed PE framing fields of a chunk without copying.
/// Returns `(request_id, header, num_chunks, chunk_num, property)`
/// borrowed from `data`. Shared by [`PeChunk::decode`] and
/// [`PeChunkRef::decode`] so validation stays in one place.
fn decode_fields(data: &[u8]) -> Result<(u8, &[u8], u16, u16, &[u8]), PeError> {
    if data.len() < PE_FRAMING_LEN {
        return Err(PeError::Truncated);
    }
    let request_id = data[0];
    if request_id > REQUEST_ID_MAX {
        return Err(PeError::BadField);
    }
    let nh = read_u14_le7(&data[1..3])? as usize;
    let mut off = 3;
    if data.len() < off + nh + 6 {
        return Err(PeError::Truncated);
    }
    let header = &data[off..off + nh];
    if header.iter().any(|b| *b > 0x7F) {
        return Err(PeError::BadField);
    }
    off += nh;
    let num_chunks = read_u14_le7(&data[off..off + 2])?;
    off += 2;
    let chunk_num = read_u14_le7(&data[off..off + 2])?;
    off += 2;
    let nd = read_u14_le7(&data[off..off + 2])? as usize;
    off += 2;
    if data.len() < off + nd {
        return Err(PeError::Truncated);
    }
    if data.len() != off + nd {
        return Err(PeError::BadField);
    }
    let property = &data[off..off + nd];
    // Header only in chunk 1. // M2-101 §8.3.1
    if chunk_num != 1 && nh != 0 {
        return Err(PeError::InconsistentChunking);
    }
    Ok((request_id, header, num_chunks, chunk_num, property))
}

/// Zero-copy view of one PE chunk's fields, borrowed from its buffer.
///
/// The hot inbound path decodes chunks as [`PeChunkRef`] so the feed path
/// performs no heap copies until (and unless) the transaction completes
/// inside the pre-reserved reassembler buffers. // ARD §3
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeChunkRef<'a> {
    pub request_id: u8,
    pub header: &'a [u8],
    /// `0` means number of chunks is unknown. // M2-101 §8.3
    pub num_chunks: u16,
    /// Chunk numbers start at `1`. `0` is the early-termination sentinel. // M2-101 §8.3
    pub chunk_num: u16,
    pub property: &'a [u8],
}

impl<'a> PeChunkRef<'a> {
    /// Decode PE fields from a CI-header-stripped SysEx body remainder
    /// (no allocation).
    pub fn decode(data: &'a [u8]) -> Result<Self, PeError> {
        let (request_id, header, num_chunks, chunk_num, property) = decode_fields(data)?;
        Ok(Self {
            request_id,
            header,
            num_chunks,
            chunk_num,
            property,
        })
    }

    /// Materialize an owned [`PeChunk`] (copies header/property).
    pub fn to_owned(self) -> PeChunk {
        PeChunk {
            request_id: self.request_id,
            header: self.header.to_vec(),
            num_chunks: self.num_chunks,
            chunk_num: self.chunk_num,
            property: self.property.to_vec(),
        }
    }
}

/// Decoded view of one PE chunk's fields after the CI header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeChunk {
    pub request_id: u8,
    pub header: Vec<u8>,
    /// `0` means number of chunks is unknown. // M2-101 §8.3
    pub num_chunks: u16,
    /// Chunk numbers start at `1`. `0` is the early-termination sentinel. // M2-101 §8.3
    pub chunk_num: u16,
    pub property: Vec<u8>,
}

impl PeChunk {
    /// Decode PE fields from a CI-header-stripped SysEx body remainder.
    pub fn decode(data: &[u8]) -> Result<Self, PeError> {
        let (request_id, header, num_chunks, chunk_num, property) = decode_fields(data)?;
        Ok(Self {
            request_id,
            header: header.to_vec(),
            num_chunks,
            chunk_num,
            property: property.to_vec(),
        })
    }

    /// Encode PE fields into `out`, returning bytes written.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, PeError> {
        if self.request_id > REQUEST_ID_MAX {
            return Err(PeError::BadField);
        }
        if self.header.len() > U14_MAX as usize || self.property.len() > U14_MAX as usize {
            return Err(PeError::BadField);
        }
        if self.num_chunks > U14_MAX || self.chunk_num > U14_MAX {
            return Err(PeError::BadField);
        }
        if self.header.iter().any(|b| *b > 0x7F) {
            return Err(PeError::BadField);
        }
        if self.chunk_num != 1 && !self.header.is_empty() {
            return Err(PeError::InconsistentChunking);
        }
        let need = PE_FRAMING_LEN + self.header.len() + self.property.len();
        if out.len() < need {
            return Err(PeError::BufferTooSmall);
        }
        out[0] = self.request_id;
        write_u14_le7(self.header.len() as u16, &mut out[1..3])?;
        let mut off = 3;
        out[off..off + self.header.len()].copy_from_slice(&self.header);
        off += self.header.len();
        write_u14_le7(self.num_chunks, &mut out[off..off + 2])?;
        off += 2;
        write_u14_le7(self.chunk_num, &mut out[off..off + 2])?;
        off += 2;
        write_u14_le7(self.property.len() as u16, &mut out[off..off + 2])?;
        off += 2;
        out[off..off + self.property.len()].copy_from_slice(&self.property);
        off += self.property.len();
        Ok(off)
    }

    /// Encode into a new `Vec`.
    pub fn to_vec(&self) -> Result<Vec<u8>, PeError> {
        let need = PE_FRAMING_LEN + self.header.len() + self.property.len();
        let mut out = alloc::vec![0u8; need];
        let n = self.encode(&mut out)?;
        out.truncate(n);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::split;

    #[test]
    fn borrowed_ref_matches_owned_decode() {
        let header = b"{\"resource\":\"DeviceInfo\",\"resId\":\"x\"}";
        let body = alloc::vec![0x21u8; 500];
        for size in [128u32, 4096] {
            for raw in split(7, header, &body, size).unwrap() {
                let owned = PeChunk::decode(&raw).unwrap();
                let borrowed = PeChunkRef::decode(&raw).unwrap();
                assert_eq!(borrowed.to_owned(), owned);
                assert_eq!(borrowed.request_id, 7);
                assert_eq!(borrowed.header, owned.header.as_slice());
                assert_eq!(borrowed.property, owned.property.as_slice());
            }
        }
        // Borrowed and owned decodes share validation (same error, same order).
        let truncated = [0x01u8, 0x00, 0x00, 0x00];
        assert_eq!(PeChunkRef::decode(&truncated), Err(PeError::Truncated));
        assert_eq!(PeChunk::decode(&truncated), Err(PeError::Truncated));
        // Header bytes must stay 7-bit in both.
        let mut bad = [0u8; 16];
        bad[3] = 0x80;
        assert_eq!(PeChunkRef::decode(&bad), Err(PeError::BadField));
        assert_eq!(PeChunk::decode(&bad), Err(PeError::BadField));
    }

    #[test]
    fn roundtrip_ref_and_owned() {
        let chunk = PeChunk {
            request_id: 3,
            header: b"{} ".to_vec(),
            num_chunks: 2,
            chunk_num: 1,
            property: alloc::vec![0x42; 10],
        };
        let raw = chunk.to_vec().unwrap();
        assert_eq!(PeChunkRef::decode(&raw).unwrap().to_owned(), chunk);
    }
}
