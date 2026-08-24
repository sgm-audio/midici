//! Property Exchange Capabilities message encode/decode. // M2-101 §8.5–§8.6

use crate::error::CiError;
use crate::header::CiHeader;
use crate::spec::{
    CI_HEADER_LEN, MESSAGE_FORMAT_VERSION_1_2, PE_VERSION_MAJOR, PE_VERSION_MINOR,
    SUB_ID2_PE_CAPS_INQUIRY, SUB_ID2_PE_CAPS_REPLY,
};

/// Inquiry / Reply to Property Exchange Capabilities. // M2-101 Tables 30 / 32
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeCapabilities {
    pub header: CiHeader,
    /// Number of Simultaneous Property Exchange Requests Supported. // M2-101 §8.5
    pub simultaneous_requests: u8,
    /// Property Exchange Major Version (v2+ field). // M2-101 §8.5 Table 31
    pub pe_major: u8,
    /// Property Exchange Minor Version (v2+ field). // M2-101 §8.5 Table 31
    pub pe_minor: u8,
}

impl PeCapabilities {
    /// Decode Inquiry or Reply (same payload layout). // M2-101 Tables 30 / 32
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_PE_CAPS_INQUIRY && header.sub_id2 != SUB_ID2_PE_CAPS_REPLY {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        let need = if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
            3
        } else {
            1
        };
        if data.len() < need {
            return Err(CiError::BadLength);
        }
        if data[..need].iter().any(|b| *b > 0x7F) {
            return Err(CiError::BadField);
        }
        let simultaneous_requests = data[0];
        let (pe_major, pe_minor) = if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
            (data[1], data[2])
        } else {
            (PE_VERSION_MAJOR, PE_VERSION_MINOR)
        };
        Ok(Self {
            header,
            simultaneous_requests,
            pe_major,
            pe_minor,
        })
    }

    /// Encode Inquiry or Reply based on `header.sub_id2`.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let v2 = self.header.version >= MESSAGE_FORMAT_VERSION_1_2;
        let total = CI_HEADER_LEN + if v2 { 3 } else { 1 };
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        if self.simultaneous_requests > 0x7F || self.pe_major > 0x7F || self.pe_minor > 0x7F {
            return Err(CiError::BadField);
        }
        self.header.encode(out)?;
        out[CI_HEADER_LEN] = self.simultaneous_requests;
        if v2 {
            out[CI_HEADER_LEN + 1] = self.pe_major;
            out[CI_HEADER_LEN + 2] = self.pe_minor;
        }
        Ok(total)
    }
}

/// Build a PE Capabilities Inquiry header+payload. // M2-101 §8.5 Table 30
pub fn pe_caps_inquiry(source: crate::Muid, dest: crate::Muid, simultaneous: u8) -> PeCapabilities {
    PeCapabilities {
        header: crate::mgmt::mgmt_header(SUB_ID2_PE_CAPS_INQUIRY, source, dest),
        simultaneous_requests: simultaneous,
        pe_major: PE_VERSION_MAJOR,
        pe_minor: PE_VERSION_MINOR,
    }
}

/// Build a PE Capabilities Reply. // M2-101 §8.6 Table 32
pub fn pe_caps_reply(source: crate::Muid, dest: crate::Muid, simultaneous: u8) -> PeCapabilities {
    PeCapabilities {
        header: crate::mgmt::mgmt_header(SUB_ID2_PE_CAPS_REPLY, source, dest),
        simultaneous_requests: simultaneous,
        pe_major: PE_VERSION_MAJOR,
        pe_minor: PE_VERSION_MINOR,
    }
}
