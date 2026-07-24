//! Engine configuration and per-peer negotiated state.

use crate::event::{CapFlags, DeviceIdentity};
use crate::muid::Muid;
use crate::spec::{
    FUNCTION_BLOCK_NONE, MESSAGE_FORMAT_VERSION_1_1, MESSAGE_FORMAT_VERSION_1_2,
    MIN_RECEIVABLE_SYSEX_SIZE,
};

/// Static identity / capability configuration for [`crate::engine::CiEngine`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CiConfig {
    pub identity: DeviceIdentity,
    /// Capability Inquiry Category Supported bitmap. // M2-101 §5.5.2 Table 7
    pub category_supported: CapFlags,
    /// Our receivable Maximum SysEx Message Size (≥128). // M2-101 §5.5.3
    pub max_sysex_size: u32,
    /// Function Block number, or [`FUNCTION_BLOCK_NONE`]. // M2-101 §5.6.2
    pub function_block: u8,
    /// Output Path ID used when we initiate Discovery (v1.2). // M2-101 §5.5.4
    pub output_path_id: u8,
    /// Maximum peers tracked in the peer table.
    pub max_peers: usize,
    /// UMP group used for locally originated announcements.
    pub local_group: u8,
}

impl CiConfig {
    /// Sensible responder defaults (PE capable, 512-byte SysEx, FB none).
    pub fn responder_default(identity: DeviceIdentity) -> Self {
        Self {
            identity,
            category_supported: CapFlags(crate::spec::CAP_PROPERTY_EXCHANGE),
            max_sysex_size: 512,
            function_block: FUNCTION_BLOCK_NONE,
            output_path_id: 0,
            max_peers: 16,
            local_group: 0,
        }
    }

    pub(crate) fn clamp_max_sysex(size: u32) -> u32 {
        size.max(MIN_RECEIVABLE_SYSEX_SIZE)
    }
}

/// Negotiated state for one discovered peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerState {
    pub muid: Muid,
    pub info: DeviceIdentity,
    pub caps: CapFlags,
    /// Peer's Message Format Version from Discovery/Reply. // M2-101 §5.3 / §5.4
    pub version: u8,
    /// `min(theirs, ours)` receivable max SysEx. // ARD §3 / §7
    pub max_sysex: u32,
    pub output_path_id: u8,
    pub function_block: u8,
}

impl PeerState {
    /// True if this peer is Message Format Version ≥ 1.2.
    ///
    /// v1.2-only messages (ACK, Endpoint) must not be sent to v1.1 peers. // ARD §4 / §7
    #[inline]
    pub fn supports_v1_2_messages(&self) -> bool {
        self.version >= MESSAGE_FORMAT_VERSION_1_2
    }

    /// Feature mask: whether we may send ACK toward this peer. // ARD §4
    #[inline]
    pub fn allow_ack(&self) -> bool {
        self.supports_v1_2_messages()
    }

    /// Feature mask: whether we may send Endpoint messages toward this peer. // ARD §4
    #[inline]
    pub fn allow_endpoint(&self) -> bool {
        self.supports_v1_2_messages()
    }

    pub(crate) fn from_discovery(
        muid: Muid,
        info: DeviceIdentity,
        caps: CapFlags,
        version: u8,
        their_max: u32,
        ours_max: u32,
        output_path_id: u8,
    ) -> Self {
        let version = if version < MESSAGE_FORMAT_VERSION_1_1 {
            MESSAGE_FORMAT_VERSION_1_1
        } else {
            version
        };
        Self {
            muid,
            info,
            caps,
            version,
            max_sysex: CiConfig::clamp_max_sysex(their_max)
                .min(CiConfig::clamp_max_sysex(ours_max)),
            output_path_id,
            function_block: FUNCTION_BLOCK_NONE,
        }
    }
}
