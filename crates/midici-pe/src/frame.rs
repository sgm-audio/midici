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
        let header = data[off..off + nh].to_vec();
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
        let property = data[off..off + nd].to_vec();
        // Header only in chunk 1. // M2-101 §8.3.1
        if chunk_num != 1 && nh != 0 {
            return Err(PeError::InconsistentChunking);
        }
        Ok(Self {
            request_id,
            header,
            num_chunks,
            chunk_num,
            property,
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
