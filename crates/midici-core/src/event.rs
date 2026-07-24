//! Application-facing engine events and outbound SysEx. // ARD §3

use alloc::vec::Vec;

use crate::muid::Muid;
use crate::spec::{
    NAK_STATUS_ADDRESS_NOT_IN_USE, NAK_STATUS_BUSY, NAK_STATUS_FLOW_RESEND_CHUNK,
    NAK_STATUS_MALFORMED, NAK_STATUS_NAK, NAK_STATUS_NOT_SUPPORTED,
    NAK_STATUS_PE_CHUNKS_OUT_OF_SEQUENCE, NAK_STATUS_PROFILE_NOT_SUPPORTED, NAK_STATUS_RETRY,
    NAK_STATUS_TERMINATE_TRANSACTION, NAK_STATUS_TIMEOUT, NAK_STATUS_VERSION_NOT_SUPPORTED,
};

/// Capability Inquiry Category Supported bitmap. // M2-101 §5.5.2 Table 7
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CapFlags(pub u8);

impl CapFlags {
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// Device identity fields shared by Discovery / Reply. // M2-101 §5.5.1
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub manufacturer: [u8; 3],
    pub family: u16,
    pub model: u16,
    pub software_revision: [u8; 4],
}

/// Typed NAK Status Code. // M2-101 §5.11.2 Table 16
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NakCode {
    /// 0x00 NAK
    Nak,
    /// 0x01 MIDI-CI message not supported
    NotSupported,
    /// 0x02 MIDI-CI version not supported (reserved version bits). // M2-101 §5.3
    VersionNotSupported,
    /// 0x03 Channel/Group/Function Block not in use
    AddressNotInUse,
    /// 0x04 Profile not supported on requested address
    ProfileNotSupported,
    /// 0x12 Flow Control: Resend most recent Chunk
    FlowResendChunk,
    /// 0x20 Terminates Transaction
    TerminateTransaction,
    /// 0x21 PE chunks out of sequence
    PeChunksOutOfSequence,
    /// 0x40 Error occurred, please retry
    Retry,
    /// 0x41 Message was malformed
    Malformed,
    /// 0x42 Timeout has occurred
    Timeout,
    /// 0x43 Busy, try again
    Busy,
    /// Unrecognized status code from the wire
    Other(u8),
}

impl NakCode {
    /// Parse a wire status code. // M2-101 §5.11.2 Table 16
    pub const fn from_u8(code: u8) -> Self {
        match code {
            NAK_STATUS_NAK => Self::Nak,
            NAK_STATUS_NOT_SUPPORTED => Self::NotSupported,
            NAK_STATUS_VERSION_NOT_SUPPORTED => Self::VersionNotSupported,
            NAK_STATUS_ADDRESS_NOT_IN_USE => Self::AddressNotInUse,
            NAK_STATUS_PROFILE_NOT_SUPPORTED => Self::ProfileNotSupported,
            NAK_STATUS_FLOW_RESEND_CHUNK => Self::FlowResendChunk,
            NAK_STATUS_TERMINATE_TRANSACTION => Self::TerminateTransaction,
            NAK_STATUS_PE_CHUNKS_OUT_OF_SEQUENCE => Self::PeChunksOutOfSequence,
            NAK_STATUS_RETRY => Self::Retry,
            NAK_STATUS_MALFORMED => Self::Malformed,
            NAK_STATUS_TIMEOUT => Self::Timeout,
            NAK_STATUS_BUSY => Self::Busy,
            other => Self::Other(other),
        }
    }

    /// Wire status code byte.
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Nak => NAK_STATUS_NAK,
            Self::NotSupported => NAK_STATUS_NOT_SUPPORTED,
            Self::VersionNotSupported => NAK_STATUS_VERSION_NOT_SUPPORTED,
            Self::AddressNotInUse => NAK_STATUS_ADDRESS_NOT_IN_USE,
            Self::ProfileNotSupported => NAK_STATUS_PROFILE_NOT_SUPPORTED,
            Self::FlowResendChunk => NAK_STATUS_FLOW_RESEND_CHUNK,
            Self::TerminateTransaction => NAK_STATUS_TERMINATE_TRANSACTION,
            Self::PeChunksOutOfSequence => NAK_STATUS_PE_CHUNKS_OUT_OF_SEQUENCE,
            Self::Retry => NAK_STATUS_RETRY,
            Self::Malformed => NAK_STATUS_MALFORMED,
            Self::Timeout => NAK_STATUS_TIMEOUT,
            Self::Busy => NAK_STATUS_BUSY,
            Self::Other(c) => c,
        }
    }
}

/// Application-facing events. // ARD §3
///
/// Property Exchange / subscription variants are deferred until the PE engine
/// wires them (Phase 4+); this phase emits management events only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CiEvent {
    PeerDiscovered {
        muid: Muid,
        info: DeviceIdentity,
        caps: CapFlags,
    },
    PeerInvalidated {
        muid: Muid,
    },
    Nak {
        peer: Muid,
        original: u8,
        code: NakCode,
    },
}

/// One outbound SysEx body (F0/F7 stripped) tagged with UMP group. // ARD §3
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboundSysex {
    pub group: u8,
    pub body: Vec<u8>,
}
