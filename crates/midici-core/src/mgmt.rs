//! Management message encode/decode (Sub-ID#2 `0x70`–`0x7F`). // M2-101 Category 7

use crate::error::CiError;
use crate::header::CiHeader;
use crate::muid::Muid;
use crate::spec::{
    CI_HEADER_LEN, DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2,
    PRODUCT_INSTANCE_ID_MAX_LEN, SUB_ID2_ACK, SUB_ID2_DISCOVERY, SUB_ID2_ENDPOINT_INQUIRY,
    SUB_ID2_ENDPOINT_REPLY, SUB_ID2_INVALIDATE_MUID, SUB_ID2_NAK, SUB_ID2_REPLY_TO_DISCOVERY,
};

fn read_u16_le7(bytes: &[u8]) -> Result<u16, CiError> {
    if bytes.len() < 2 || bytes[0] > 0x7F || bytes[1] > 0x7F {
        return Err(CiError::BadField);
    }
    Ok(u16::from(bytes[0]) | (u16::from(bytes[1]) << 7))
}

fn write_u16_le7(v: u16, out: &mut [u8]) -> Result<(), CiError> {
    if out.len() < 2 || v > 0x3FFF {
        return Err(CiError::BadField);
    }
    out[0] = (v & 0x7F) as u8;
    out[1] = ((v >> 7) & 0x7F) as u8;
    Ok(())
}

fn read_u32_le7(bytes: &[u8]) -> Result<u32, CiError> {
    if bytes.len() < 4 {
        return Err(CiError::Truncated);
    }
    for b in &bytes[..4] {
        if *b > 0x7F {
            return Err(CiError::BadField);
        }
    }
    Ok(u32::from(bytes[0])
        | (u32::from(bytes[1]) << 7)
        | (u32::from(bytes[2]) << 14)
        | (u32::from(bytes[3]) << 21))
}

fn write_u32_le7(v: u32, out: &mut [u8]) -> Result<(), CiError> {
    if out.len() < 4 || v > 0x0FFF_FFFF {
        return Err(CiError::BadField);
    }
    out[0] = (v & 0x7F) as u8;
    out[1] = ((v >> 7) & 0x7F) as u8;
    out[2] = ((v >> 14) & 0x7F) as u8;
    out[3] = ((v >> 21) & 0x7F) as u8;
    Ok(())
}

fn require_7bit_bytes(bytes: &[u8]) -> Result<(), CiError> {
    if bytes.iter().any(|b| *b > 0x7F) {
        Err(CiError::BadField)
    } else {
        Ok(())
    }
}

/// Discovery message fields (after header). // M2-101 §5.5 Table 6
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Discovery {
    pub header: CiHeader,
    /// Device Manufacturer (System Exclusive ID Number), 3 bytes. // M2-101 §5.5.1
    pub manufacturer: [u8; 3],
    /// Device Family (LSB first). // M2-101 §5.5.1
    pub family: u16,
    /// Device Family Model Number (LSB first). // M2-101 §5.5.1
    pub model: u16,
    /// Software Revision Level (device-specific 4 bytes). // M2-101 §5.5.1
    pub software_revision: [u8; 4],
    /// Capability Inquiry Category Supported bitmap. // M2-101 §5.5.2 Table 7
    pub category_supported: u8,
    /// Receivable Maximum SysEx Message Size (LSB first, ≥128). // M2-101 §5.5.3
    pub max_sysex_size: u32,
    /// Initiator's Output Path ID (Message Format Version ≥ 2). // M2-101 §5.5 Table 6 / §5.5.4
    pub output_path_id: u8,
}

impl Discovery {
    /// Decode Discovery from a full F0/F7-stripped body. // M2-101 §5.5 Table 6
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_DISCOVERY {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        // v1 payload 16 bytes; v2 adds Output Path ID. // M2-101 §5.5 Table 6
        let need = if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
            17
        } else {
            16
        };
        if data.len() < need {
            return Err(CiError::BadLength);
        }
        require_7bit_bytes(&data[..need])?;
        let manufacturer = [data[0], data[1], data[2]];
        let family = read_u16_le7(&data[3..5])?;
        let model = read_u16_le7(&data[5..7])?;
        let software_revision = [data[7], data[8], data[9], data[10]];
        let category_supported = data[11];
        let max_sysex_size = read_u32_le7(&data[12..16])?;
        let output_path_id = if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
            data[16]
        } else {
            0
        };
        Ok(Self {
            header,
            manufacturer,
            family,
            model,
            software_revision,
            category_supported,
            max_sysex_size,
            output_path_id,
        })
    }

    /// Encode into `out` (F0/F7 stripped). Returns length written.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let v2 = self.header.version >= MESSAGE_FORMAT_VERSION_1_2;
        let total = CI_HEADER_LEN + if v2 { 17 } else { 16 };
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_DISCOVERY;
        h.encode(out)?;
        let d = &mut out[CI_HEADER_LEN..];
        require_7bit_bytes(&self.manufacturer)?;
        require_7bit_bytes(&self.software_revision)?;
        d[0..3].copy_from_slice(&self.manufacturer);
        write_u16_le7(self.family, &mut d[3..5])?;
        write_u16_le7(self.model, &mut d[5..7])?;
        d[7..11].copy_from_slice(&self.software_revision);
        d[11] = self.category_supported & 0x7F;
        write_u32_le7(self.max_sysex_size, &mut d[12..16])?;
        if v2 {
            d[16] = self.output_path_id & 0x7F;
        }
        Ok(total)
    }
}

/// Reply to Discovery. // M2-101 §5.6 Table 8
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyToDiscovery {
    pub header: CiHeader,
    pub manufacturer: [u8; 3],
    pub family: u16,
    pub model: u16,
    pub software_revision: [u8; 4],
    pub category_supported: u8,
    pub max_sysex_size: u32,
    /// Echo of Initiator's Output Path ID (v2+). // M2-101 §5.6.1
    pub output_path_id: u8,
    /// Function Block number, or `0x7F` if none (v2+). // M2-101 §5.6.2
    pub function_block: u8,
}

impl ReplyToDiscovery {
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_REPLY_TO_DISCOVERY {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        let need = if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
            18
        } else {
            16
        };
        if data.len() < need {
            return Err(CiError::BadLength);
        }
        require_7bit_bytes(&data[..need])?;
        Ok(Self {
            header,
            manufacturer: [data[0], data[1], data[2]],
            family: read_u16_le7(&data[3..5])?,
            model: read_u16_le7(&data[5..7])?,
            software_revision: [data[7], data[8], data[9], data[10]],
            category_supported: data[11],
            max_sysex_size: read_u32_le7(&data[12..16])?,
            output_path_id: if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
                data[16]
            } else {
                0
            },
            function_block: if header.version >= MESSAGE_FORMAT_VERSION_1_2 {
                data[17]
            } else {
                DEVICE_ID_FUNCTION_BLOCK
            },
        })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let v2 = self.header.version >= MESSAGE_FORMAT_VERSION_1_2;
        let total = CI_HEADER_LEN + if v2 { 18 } else { 16 };
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_REPLY_TO_DISCOVERY;
        h.encode(out)?;
        let d = &mut out[CI_HEADER_LEN..];
        require_7bit_bytes(&self.manufacturer)?;
        require_7bit_bytes(&self.software_revision)?;
        d[0..3].copy_from_slice(&self.manufacturer);
        write_u16_le7(self.family, &mut d[3..5])?;
        write_u16_le7(self.model, &mut d[5..7])?;
        d[7..11].copy_from_slice(&self.software_revision);
        d[11] = self.category_supported & 0x7F;
        write_u32_le7(self.max_sysex_size, &mut d[12..16])?;
        if v2 {
            d[16] = self.output_path_id & 0x7F;
            d[17] = self.function_block & 0x7F;
        }
        Ok(total)
    }
}

/// Inquiry: Endpoint Information. // M2-101 §5.7 Table 9
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointInquiry {
    pub header: CiHeader,
    /// Status selects target property. // M2-101 §5.7.1 Table 10
    pub status: u8,
}

impl EndpointInquiry {
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_ENDPOINT_INQUIRY {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        if data.len() != 1 {
            return Err(CiError::BadLength);
        }
        if data[0] > 0x7F {
            return Err(CiError::BadField);
        }
        Ok(Self {
            header,
            status: data[0],
        })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let total = CI_HEADER_LEN + 1;
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_ENDPOINT_INQUIRY;
        h.encode(out)?;
        out[CI_HEADER_LEN] = self.status & 0x7F;
        Ok(total)
    }
}

/// Reply to Endpoint Information (zero-copy `information`). // M2-101 §5.8 Table 11
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointReply<'a> {
    pub header: CiHeader,
    pub status: u8,
    /// Information Data. // M2-101 §5.8.3
    pub information: &'a [u8],
}

impl<'a> EndpointReply<'a> {
    pub fn decode(body: &'a [u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_ENDPOINT_REPLY {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        if data.len() < 3 {
            return Err(CiError::Truncated);
        }
        let status = data[0];
        if status > 0x7F {
            return Err(CiError::BadField);
        }
        let lid = read_u16_le7(&data[1..3])? as usize;
        if data.len() != 3 + lid {
            return Err(CiError::BadLength);
        }
        let information = &data[3..3 + lid];
        require_7bit_bytes(information)?;
        if status == 0x00 && information.len() > PRODUCT_INSTANCE_ID_MAX_LEN {
            return Err(CiError::BadField);
        }
        Ok(Self {
            header,
            status,
            information,
        })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let lid = self.information.len();
        if lid > 0x3FFF {
            return Err(CiError::BadField);
        }
        let total = CI_HEADER_LEN + 3 + lid;
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_ENDPOINT_REPLY;
        h.encode(out)?;
        require_7bit_bytes(self.information)?;
        out[CI_HEADER_LEN] = self.status & 0x7F;
        write_u16_le7(lid as u16, &mut out[CI_HEADER_LEN + 1..CI_HEADER_LEN + 3])?;
        out[CI_HEADER_LEN + 3..total].copy_from_slice(self.information);
        Ok(total)
    }
}

/// Invalidate MUID. // M2-101 §5.9 Table 12
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidateMuid {
    pub header: CiHeader,
    /// Target MUID to invalidate. // M2-101 §5.9 Table 12
    pub target: Muid,
}

impl InvalidateMuid {
    pub fn decode(body: &[u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_INVALIDATE_MUID {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        if data.len() != 4 {
            return Err(CiError::BadLength);
        }
        let target = Muid::decode_7bit_le(data.try_into().unwrap())?;
        Ok(Self { header, target })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        let total = CI_HEADER_LEN + 4;
        if out.len() < total {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_INVALIDATE_MUID;
        // Destination shall be Broadcast. // M2-101 §5.9 Table 12
        h.dest = Muid::BROADCAST;
        h.encode(out)?;
        out[CI_HEADER_LEN..total].copy_from_slice(&self.target.encode_7bit_le());
        Ok(total)
    }
}

/// Shared ACK/NAK trailer layout (Message Format Version 2 fields). // M2-101 §5.10 / §5.11
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AckNakBody<'a> {
    /// Original Transaction Sub-ID#2 Classification. // M2-101 §5.10.1 / §5.11.1
    pub original_sub_id2: u8,
    pub status_code: u8,
    pub status_data: u8,
    /// 5 detail bytes for the MIDI-CI branch. // M2-101 §5.10.3 / §5.11.3
    pub details: [u8; 5],
    /// Message text (7-bit). // M2-101 §5.10.4 / §5.11.4
    pub message: &'a [u8],
}

fn decode_ack_nak_body(data: &[u8]) -> Result<AckNakBody<'_>, CiError> {
    // Fixed trailer before text: 1+1+1+5+2 = 10 bytes. // M2-101 §5.10 Table 13 / §5.11 Table 15
    if data.len() < 10 {
        return Err(CiError::Truncated);
    }
    require_7bit_bytes(&data[..10])?;
    let ml = read_u16_le7(&data[8..10])? as usize;
    if data.len() != 10 + ml {
        return Err(CiError::BadLength);
    }
    let message = &data[10..10 + ml];
    require_7bit_bytes(message)?;
    Ok(AckNakBody {
        original_sub_id2: data[0],
        status_code: data[1],
        status_data: data[2],
        details: [data[3], data[4], data[5], data[6], data[7]],
        message,
    })
}

fn encode_ack_nak_body(body: &AckNakBody<'_>, out: &mut [u8]) -> Result<usize, CiError> {
    let ml = body.message.len();
    if ml > 0x3FFF {
        return Err(CiError::BadField);
    }
    let total = 10 + ml;
    if out.len() < total {
        return Err(CiError::BufferTooSmall);
    }
    require_7bit_bytes(body.message)?;
    require_7bit_bytes(&body.details)?;
    out[0] = body.original_sub_id2 & 0x7F;
    out[1] = body.status_code & 0x7F;
    out[2] = body.status_data & 0x7F;
    out[3..8].copy_from_slice(&body.details);
    write_u16_le7(ml as u16, &mut out[8..10])?;
    out[10..total].copy_from_slice(body.message);
    Ok(total)
}

/// MIDI-CI ACK. // M2-101 §5.10 Table 13
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack<'a> {
    pub header: CiHeader,
    pub body: AckNakBody<'a>,
}

impl<'a> Ack<'a> {
    pub fn decode(body: &'a [u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_ACK {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        // ACK Status fields exist in this document's table for current format. // M2-101 §5.10
        Ok(Self {
            header,
            body: decode_ack_nak_body(data)?,
        })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        if out.len() < CI_HEADER_LEN {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_ACK;
        h.encode(out)?;
        let n = encode_ack_nak_body(&self.body, &mut out[CI_HEADER_LEN..])?;
        Ok(CI_HEADER_LEN + n)
    }
}

/// MIDI-CI NAK. // M2-101 §5.11 Table 15
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nak<'a> {
    pub header: CiHeader,
    pub body: AckNakBody<'a>,
}

impl<'a> Nak<'a> {
    pub fn decode(body: &'a [u8]) -> Result<Self, CiError> {
        let (header, data) = CiHeader::decode(body)?;
        if header.sub_id2 != SUB_ID2_NAK {
            return Err(CiError::UnsupportedSubId2(header.sub_id2));
        }
        // Status fields were added in Message Format Version 2. // M2-101 §5.11 Table 15
        if header.version < MESSAGE_FORMAT_VERSION_1_2 {
            if !data.is_empty() {
                return Err(CiError::BadLength);
            }
            return Ok(Self {
                header,
                body: AckNakBody {
                    original_sub_id2: 0,
                    status_code: 0,
                    status_data: 0,
                    details: [0; 5],
                    message: &[],
                },
            });
        }
        Ok(Self {
            header,
            body: decode_ack_nak_body(data)?,
        })
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        if out.len() < CI_HEADER_LEN {
            return Err(CiError::BufferTooSmall);
        }
        let mut h = self.header;
        h.sub_id2 = SUB_ID2_NAK;
        h.encode(out)?;
        if h.version < MESSAGE_FORMAT_VERSION_1_2 {
            return Ok(CI_HEADER_LEN);
        }
        let n = encode_ack_nak_body(&self.body, &mut out[CI_HEADER_LEN..])?;
        Ok(CI_HEADER_LEN + n)
    }
}

/// Any Management message decoded from a stripped SysEx body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MgmtMessage<'a> {
    Discovery(Discovery),
    ReplyToDiscovery(ReplyToDiscovery),
    EndpointInquiry(EndpointInquiry),
    EndpointReply(EndpointReply<'a>),
    InvalidateMuid(InvalidateMuid),
    Ack(Ack<'a>),
    Nak(Nak<'a>),
}

impl<'a> MgmtMessage<'a> {
    /// Decode a Management MIDI-CI body (F0/F7 stripped). // M2-101 §5.2 / ARD §3
    pub fn decode(body: &'a [u8]) -> Result<Self, CiError> {
        let (header, _) = CiHeader::decode(body)?;
        match header.sub_id2 {
            SUB_ID2_DISCOVERY => Ok(Self::Discovery(Discovery::decode(body)?)),
            SUB_ID2_REPLY_TO_DISCOVERY => {
                Ok(Self::ReplyToDiscovery(ReplyToDiscovery::decode(body)?))
            }
            SUB_ID2_ENDPOINT_INQUIRY => Ok(Self::EndpointInquiry(EndpointInquiry::decode(body)?)),
            SUB_ID2_ENDPOINT_REPLY => Ok(Self::EndpointReply(EndpointReply::decode(body)?)),
            SUB_ID2_INVALIDATE_MUID => Ok(Self::InvalidateMuid(InvalidateMuid::decode(body)?)),
            SUB_ID2_ACK => Ok(Self::Ack(Ack::decode(body)?)),
            SUB_ID2_NAK => Ok(Self::Nak(Nak::decode(body)?)),
            other => Err(CiError::UnsupportedSubId2(other)),
        }
    }

    /// Encode into `out`. Returns length written.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        match self {
            Self::Discovery(m) => m.encode(out),
            Self::ReplyToDiscovery(m) => m.encode(out),
            Self::EndpointInquiry(m) => m.encode(out),
            Self::EndpointReply(m) => m.encode(out),
            Self::InvalidateMuid(m) => m.encode(out),
            Self::Ack(m) => m.encode(out),
            Self::Nak(m) => m.encode(out),
        }
    }
}

/// Helpers shared by tests/proptest for building version-2 headers.
pub fn mgmt_header(sub_id2: u8, source: Muid, dest: Muid) -> CiHeader {
    CiHeader {
        device_id: DEVICE_ID_FUNCTION_BLOCK,
        sub_id2,
        version: MESSAGE_FORMAT_VERSION_1_2,
        source,
        dest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{
        CAP_PROPERTY_EXCHANGE, ENDPOINT_STATUS_PRODUCT_INSTANCE_ID, MESSAGE_FORMAT_VERSION_1_1,
        NAK_STATUS_NOT_SUPPORTED,
    };
    use proptest::prelude::*;

    fn sample_discovery() -> Discovery {
        Discovery {
            header: mgmt_header(
                SUB_ID2_DISCOVERY,
                Muid::ordinary(0x0102_0304).unwrap(),
                Muid::BROADCAST,
            ),
            manufacturer: [0x7D, 0x00, 0x00],
            family: 0x0001,
            model: 0x0002,
            software_revision: [0x01, 0x00, 0x00, 0x00],
            category_supported: CAP_PROPERTY_EXCHANGE,
            max_sysex_size: 512,
            output_path_id: 0,
        }
    }

    #[test]
    fn discovery_roundtrip() {
        let msg = sample_discovery();
        let mut buf = [0u8; 64];
        let n = msg.encode(&mut buf).unwrap();
        let decoded = Discovery::decode(&buf[..n]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn invalidate_forces_broadcast_dest() {
        let msg = InvalidateMuid {
            header: mgmt_header(
                SUB_ID2_INVALIDATE_MUID,
                Muid::ordinary(1).unwrap(),
                Muid::ordinary(2).unwrap(),
            ),
            target: Muid::ordinary(0x0A0B_0C0D).unwrap(),
        };
        let mut buf = [0u8; 32];
        let n = msg.encode(&mut buf).unwrap();
        let decoded = InvalidateMuid::decode(&buf[..n]).unwrap();
        assert!(decoded.header.dest.is_broadcast());
        assert_eq!(decoded.target, msg.target);
    }

    #[test]
    fn endpoint_reply_zero_copy() {
        let info = b"SERIAL123";
        let msg = EndpointReply {
            header: mgmt_header(
                SUB_ID2_ENDPOINT_REPLY,
                Muid::ordinary(3).unwrap(),
                Muid::ordinary(4).unwrap(),
            ),
            status: ENDPOINT_STATUS_PRODUCT_INSTANCE_ID,
            information: info,
        };
        let mut buf = [0u8; 64];
        let n = msg.encode(&mut buf).unwrap();
        let decoded = EndpointReply::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.information, info);
    }

    #[test]
    fn nak_not_supported_roundtrip() {
        let msg = Nak {
            header: mgmt_header(
                SUB_ID2_NAK,
                Muid::ordinary(5).unwrap(),
                Muid::ordinary(6).unwrap(),
            ),
            body: AckNakBody {
                original_sub_id2: SUB_ID2_ENDPOINT_INQUIRY,
                status_code: NAK_STATUS_NOT_SUPPORTED,
                status_data: 0,
                details: [0; 5],
                message: b"unsupported",
            },
        };
        let mut buf = [0u8; 64];
        let n = msg.encode(&mut buf).unwrap();
        let decoded = Nak::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.body.status_code, NAK_STATUS_NOT_SUPPORTED);
        assert_eq!(decoded.body.message, b"unsupported");
    }

    proptest! {
        #[test]
        fn discovery_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            mfr0 in 0u8..=0x7F,
            mfr1 in 0u8..=0x7F,
            mfr2 in 0u8..=0x7F,
            family in 0u16..=0x3FFF,
            model in 0u16..=0x3FFF,
            r0 in 0u8..=0x7F,
            r1 in 0u8..=0x7F,
            r2 in 0u8..=0x7F,
            r3 in 0u8..=0x7F,
            caps in 0u8..=0x7F,
            max_sysex in 128u32..=0x0FFF_FFFF,
            path in 0u8..=0x7F,
        ) {
            let msg = Discovery {
                header: mgmt_header(SUB_ID2_DISCOVERY, Muid::ordinary(src).unwrap(), Muid::BROADCAST),
                manufacturer: [mfr0, mfr1, mfr2],
                family,
                model,
                software_revision: [r0, r1, r2, r3],
                category_supported: caps,
                max_sysex_size: max_sysex,
                output_path_id: path,
            };
            let mut buf = [0u8; 64];
            let n = msg.encode(&mut buf).unwrap();
            let decoded = Discovery::decode(&buf[..n]).unwrap();
            prop_assert_eq!(decoded, msg);
        }

        #[test]
        fn header_proptest_roundtrip(
            device_id in prop_oneof![0u8..=0x0F, Just(0x7E), Just(0x7F)],
            sub in 0x70u8..=0x7F,
            ver in prop_oneof![Just(MESSAGE_FORMAT_VERSION_1_1), Just(MESSAGE_FORMAT_VERSION_1_2)],
            src in 0u32..Muid::ORDINARY_END,
            dst in 0u32..=Muid::BROADCAST_VALUE,
        ) {
            let h = CiHeader {
                device_id,
                sub_id2: sub,
                version: ver,
                source: Muid::ordinary(src).unwrap(),
                dest: Muid::from_u32(dst).unwrap(),
            };
            let mut buf = [0u8; CI_HEADER_LEN];
            h.encode(&mut buf).unwrap();
            let (decoded, rest) = CiHeader::decode(&buf).unwrap();
            prop_assert!(rest.is_empty());
            prop_assert_eq!(decoded, h);
        }

        #[test]
        fn reply_to_discovery_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            dst in 0u32..Muid::ORDINARY_END,
            mfr0 in 0u8..=0x7F,
            family in 0u16..=0x3FFF,
            model in 0u16..=0x3FFF,
            caps in 0u8..=0x7F,
            max_sysex in 128u32..=0x0FFF_FFFF,
            path in 0u8..=0x7F,
            fb in 0u8..=0x7F,
        ) {
            let msg = ReplyToDiscovery {
                header: mgmt_header(
                    SUB_ID2_REPLY_TO_DISCOVERY,
                    Muid::ordinary(src).unwrap(),
                    Muid::ordinary(dst).unwrap(),
                ),
                manufacturer: [mfr0, 0, 0],
                family,
                model,
                software_revision: [1, 0, 0, 0],
                category_supported: caps,
                max_sysex_size: max_sysex,
                output_path_id: path,
                function_block: fb,
            };
            let mut buf = [0u8; 64];
            let n = msg.encode(&mut buf).unwrap();
            prop_assert_eq!(ReplyToDiscovery::decode(&buf[..n]).unwrap(), msg);
        }

        #[test]
        fn endpoint_inquiry_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            dst in 0u32..Muid::ORDINARY_END,
            status in 0u8..=0x7F,
        ) {
            let msg = EndpointInquiry {
                header: mgmt_header(
                    SUB_ID2_ENDPOINT_INQUIRY,
                    Muid::ordinary(src).unwrap(),
                    Muid::ordinary(dst).unwrap(),
                ),
                status,
            };
            let mut buf = [0u8; 32];
            let n = msg.encode(&mut buf).unwrap();
            prop_assert_eq!(EndpointInquiry::decode(&buf[..n]).unwrap(), msg);
        }

        #[test]
        fn invalidate_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            target in 0u32..=Muid::BROADCAST_VALUE,
        ) {
            let msg = InvalidateMuid {
                header: mgmt_header(
                    SUB_ID2_INVALIDATE_MUID,
                    Muid::ordinary(src).unwrap(),
                    Muid::BROADCAST,
                ),
                target: Muid::from_u32(target).unwrap(),
            };
            let mut buf = [0u8; 32];
            let n = msg.encode(&mut buf).unwrap();
            let decoded = InvalidateMuid::decode(&buf[..n]).unwrap();
            prop_assert_eq!(decoded.target, msg.target);
            prop_assert!(decoded.header.dest.is_broadcast());
        }

        #[test]
        fn ack_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            dst in 0u32..Muid::ORDINARY_END,
            orig in 0u8..=0x7F,
            code in 0u8..=0x7F,
            data in 0u8..=0x7F,
            msg_byte in 0x20u8..=0x7E,
        ) {
            let text = [msg_byte];
            let msg = Ack {
                header: mgmt_header(
                    SUB_ID2_ACK,
                    Muid::ordinary(src).unwrap(),
                    Muid::ordinary(dst).unwrap(),
                ),
                body: AckNakBody {
                    original_sub_id2: orig,
                    status_code: code,
                    status_data: data,
                    details: [0; 5],
                    message: &text,
                },
            };
            let mut buf = [0u8; 64];
            let n = msg.encode(&mut buf).unwrap();
            let decoded = Ack::decode(&buf[..n]).unwrap();
            prop_assert_eq!(decoded.body.original_sub_id2, orig);
            prop_assert_eq!(decoded.body.status_code, code);
            prop_assert_eq!(decoded.body.status_data, data);
            prop_assert_eq!(decoded.body.message, &text);
        }

        #[test]
        fn nak_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            dst in 0u32..Muid::ORDINARY_END,
            orig in 0u8..=0x7F,
            code in 0u8..=0x7F,
            data in 0u8..=0x7F,
        ) {
            let msg = Nak {
                header: mgmt_header(
                    SUB_ID2_NAK,
                    Muid::ordinary(src).unwrap(),
                    Muid::ordinary(dst).unwrap(),
                ),
                body: AckNakBody {
                    original_sub_id2: orig,
                    status_code: code,
                    status_data: data,
                    details: [1, 2, 3, 4, 5],
                    message: &[],
                },
            };
            let mut buf = [0u8; 64];
            let n = msg.encode(&mut buf).unwrap();
            let decoded = Nak::decode(&buf[..n]).unwrap();
            prop_assert_eq!(decoded.body.original_sub_id2, orig);
            prop_assert_eq!(decoded.body.status_code, code);
            prop_assert_eq!(decoded.body.details, [1, 2, 3, 4, 5]);
        }

        #[test]
        fn endpoint_reply_proptest_roundtrip(
            src in 0u32..Muid::ORDINARY_END,
            dst in 0u32..Muid::ORDINARY_END,
            info in prop::collection::vec(0x20u8..=0x7E, 0..=42),
        ) {
            let msg = EndpointReply {
                header: mgmt_header(
                    SUB_ID2_ENDPOINT_REPLY,
                    Muid::ordinary(src).unwrap(),
                    Muid::ordinary(dst).unwrap(),
                ),
                status: ENDPOINT_STATUS_PRODUCT_INSTANCE_ID,
                information: &info,
            };
            let mut buf = [0u8; 128];
            let n = msg.encode(&mut buf).unwrap();
            let decoded = EndpointReply::decode(&buf[..n]).unwrap();
            prop_assert_eq!(decoded.status, ENDPOINT_STATUS_PRODUCT_INSTANCE_ID);
            prop_assert_eq!(decoded.information, info.as_slice());
        }
    }
}
