//! PE Get Inquiry/Reply wrappers around CI header + PE chunk payload.
//! // M2-101 §8.7–§8.8 Tables 33–34

use alloc::vec::Vec;

use crate::error::CiError;
use crate::header::CiHeader;
use crate::spec::{CI_HEADER_LEN, SUB_ID2_PE_GET_INQUIRY, SUB_ID2_PE_GET_REPLY};

/// A PE Get Inquiry or Reply: CI header followed by PE chunk fields. // M2-101 Tables 33–34
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeGetMessage {
    pub header: CiHeader,
    /// PE payload after the fixed CI header (requestId…property).
    pub pe_payload: Vec<u8>,
}

impl PeGetMessage {
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, rest) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_PE_GET_INQUIRY && header.sub_id2 != SUB_ID2_PE_GET_REPLY {
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
