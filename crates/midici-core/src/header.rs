//! Universal SysEx MIDI-CI header encode/decode (F0/F7 stripped). // M2-101 §5.2.1 Table 5

use crate::error::CiError;
use crate::muid::Muid;
use crate::spec::{CI_HEADER_LEN, SUB_ID1_MIDI_CI, UNIVERSAL_NON_REALTIME};

/// Fixed MIDI-CI header fields present on every message. // M2-101 §5.2.1 Table 5
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CiHeader {
    /// Device ID (source or destination depending on message). // M2-101 §5.2.1
    pub device_id: u8,
    /// Universal System Exclusive Sub-ID#2. // M2-101 §5.2.1 Table 5
    pub sub_id2: u8,
    /// MIDI-CI Message Version/Format. // M2-101 §5.2.1
    pub version: u8,
    /// Source MUID (LSB-first 7-bit encoding on the wire). // M2-101 §5.2.1
    pub source: Muid,
    /// Destination MUID (LSB-first 7-bit encoding on the wire). // M2-101 §5.2.1
    pub dest: Muid,
}

impl CiHeader {
    /// Decode a CI header from a SysEx body with F0/F7 stripped. // M2-101 §5.2 / ARD §3
    ///
    /// Returns `(header, remainder)` where `remainder` is the Data field.
    pub fn decode(body: &[u8]) -> Result<(Self, &[u8]), CiError> {
        if body.len() < CI_HEADER_LEN {
            return Err(CiError::Truncated);
        }
        if body[0] != UNIVERSAL_NON_REALTIME {
            return Err(CiError::NotUniversalSysex);
        }
        if body[2] != SUB_ID1_MIDI_CI {
            return Err(CiError::NotMidiCi);
        }
        let device_id = body[1];
        let sub_id2 = body[3];
        let version = body[4];
        let source = Muid::decode_7bit_le(body[5..9].try_into().unwrap())?;
        let dest = Muid::decode_7bit_le(body[9..13].try_into().unwrap())?;
        Ok((
            Self {
                device_id,
                sub_id2,
                version,
                source,
                dest,
            },
            &body[CI_HEADER_LEN..],
        ))
    }

    /// Encode the fixed header into `out`, returning bytes written (always [`CI_HEADER_LEN`]).
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, CiError> {
        if out.len() < CI_HEADER_LEN {
            return Err(CiError::BufferTooSmall);
        }
        out[0] = UNIVERSAL_NON_REALTIME;
        out[1] = self.device_id;
        out[2] = SUB_ID1_MIDI_CI;
        out[3] = self.sub_id2;
        out[4] = self.version;
        out[5..9].copy_from_slice(&self.source.encode_7bit_le());
        out[9..13].copy_from_slice(&self.dest.encode_7bit_le());
        Ok(CI_HEADER_LEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{MESSAGE_FORMAT_VERSION_1_2, SUB_ID2_DISCOVERY};

    #[test]
    fn header_roundtrip() {
        let h = CiHeader {
            device_id: 0x7F,
            sub_id2: SUB_ID2_DISCOVERY,
            version: MESSAGE_FORMAT_VERSION_1_2,
            source: Muid::ordinary(0x0012_3456).unwrap(),
            dest: Muid::BROADCAST,
        };
        let mut buf = [0u8; CI_HEADER_LEN];
        assert_eq!(h.encode(&mut buf).unwrap(), CI_HEADER_LEN);
        let (decoded, rest) = CiHeader::decode(&buf).unwrap();
        assert!(rest.is_empty());
        assert_eq!(decoded, h);
        assert_eq!(&buf[9..13], &[0x7F, 0x7F, 0x7F, 0x7F]);
    }
}
