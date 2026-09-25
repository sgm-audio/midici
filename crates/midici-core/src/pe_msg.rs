//! Generic chunked PE message wrapper: CI header + opaque PE chunk payload.
//!
//! Applies to Get/Set/Subscription inquiries and replies and to the
//! (deprecated, receive-only) Notify — they share the exact same chunk
//! framing. // M2-101 §8.7–§8.13 Tables 33–39

use alloc::vec::Vec;

use crate::error::CiError;
use crate::header::CiHeader;
use crate::spec::{is_pe_chunked_sub_id, CI_HEADER_LEN};

/// Any chunked PE message: CI header followed by PE chunk fields
/// (requestId, header len/data, numChunks/chunkNum, data len/data).
/// // M2-101 §8.7–§8.13 Tables 33–39
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeMessage {
    pub header: CiHeader,
    /// PE payload after the fixed CI header (requestId…property).
    pub pe_payload: Vec<u8>,
}

impl PeMessage {
    /// Decode any chunked PE message (Sub-ID#2 0x34–0x39 or 0x3F).
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, rest) = CiHeader::decode(body)?;
        if !is_pe_chunked_sub_id(header.sub_id2) {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        Ok(Self {
            header,
            pe_payload: rest.to_vec(),
        })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let total = CI_HEADER_LEN + self.pe_payload.len();
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        self.header.encode(out)?;
        out[CI_HEADER_LEN..total].copy_from_slice(&self.pe_payload);
        Ok(total)
    }

    /// Encode into a new buffer.
    pub fn to_vec(&self) -> Result<Vec<u8>, CiError> {
        let mut out = alloc::vec![0u8; CI_HEADER_LEN + self.pe_payload.len()];
        let n = self.encode(&mut out)?;
        out.truncate(n);
        Ok(out)
    }
}
