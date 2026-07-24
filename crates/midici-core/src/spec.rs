//! MIDI-CI Management constants from M2-101-UM.
//!
//! Every constant cites its specification section (`AGENTS.md` §6).

/// Universal Non-Realtime System Exclusive ID. // M2-101 §5.2.1 Table 5
pub const UNIVERSAL_NON_REALTIME: u8 = 0x7E;

/// Universal System Exclusive Sub-ID#1: MIDI-CI. // M2-101 §5.2.1 Table 5
pub const SUB_ID1_MIDI_CI: u8 = 0x0D;

/// Device ID: address whole Function Block (to/from). // M2-101 §3.2.1 / §5.2.1 Table 5
pub const DEVICE_ID_FUNCTION_BLOCK: u8 = 0x7F;

/// Device ID: address whole Group (to/from). // M2-101 §3.2.2 / §5.2.1 Table 5
pub const DEVICE_ID_GROUP: u8 = 0x7E;

/// Fixed header length after F0/F7 strip: `7E <dev> 0D <sub2> <ver> <src×4> <dst×4>`. // M2-101 §5.2.1 Table 5
pub const CI_HEADER_LEN: usize = 13;

// --- Sub-ID#2: Management Messages (Category 7) — M2-101 Appendix E / Table 4 ---

/// Sub-ID#2: Discovery. // M2-101 Appendix E / §5.5 Table 6
pub const SUB_ID2_DISCOVERY: u8 = 0x70;

/// Sub-ID#2: Reply to Discovery. // M2-101 Appendix E / §5.6 Table 8
pub const SUB_ID2_REPLY_TO_DISCOVERY: u8 = 0x71;

/// Sub-ID#2: Inquiry: Endpoint Information. // M2-101 Appendix E / §5.7 Table 9
pub const SUB_ID2_ENDPOINT_INQUIRY: u8 = 0x72;

/// Sub-ID#2: Reply to Endpoint Information. // M2-101 Appendix E / §5.8 Table 11
pub const SUB_ID2_ENDPOINT_REPLY: u8 = 0x73;

/// Sub-ID#2: MIDI-CI ACK. // M2-101 Appendix E / §5.10 Table 13
pub const SUB_ID2_ACK: u8 = 0x7D;

/// Sub-ID#2: Invalidate MUID. // M2-101 Appendix E / §5.9 Table 12
pub const SUB_ID2_INVALIDATE_MUID: u8 = 0x7E;

/// Sub-ID#2: MIDI-CI NAK. // M2-101 Appendix E / §5.11 Table 15
pub const SUB_ID2_NAK: u8 = 0x7F;

// --- Message Format Version — M2-101 §5.2.1 / §5.4 ---

/// Message Format Version used by MIDI-CI 1.1. // M2-101 §5.2.1
pub const MESSAGE_FORMAT_VERSION_1_1: u8 = 0x01;

/// Message Format Version used by MIDI-CI 1.2. // M2-101 §5.2.1
pub const MESSAGE_FORMAT_VERSION_1_2: u8 = 0x02;

/// Reserved major-revision bits (7..5) in Message Format Version. // M2-101 §5.2.1
pub const MESSAGE_FORMAT_VERSION_RESERVED_MASK: u8 = 0xE0;

/// Returns true if reserved major bits are set (must NAK with 0x02). // M2-101 §5.3 / §5.4
#[inline]
pub const fn message_format_version_reserved_set(version: u8) -> bool {
    (version & MESSAGE_FORMAT_VERSION_RESERVED_MASK) != 0
}

// --- Capability Inquiry Category Supported bitmap — M2-101 §5.5.2 Table 7 ---

/// Category bit D1: Protocol Negotiation (deprecated). // M2-101 §5.5.2 Table 7
pub const CAP_PROTOCOL_NEGOTIATION: u8 = 1 << 1;

/// Category bit D2: Profile Configuration Supported. // M2-101 §5.5.2 Table 7
pub const CAP_PROFILES: u8 = 1 << 2;

/// Category bit D3: Property Exchange Supported. // M2-101 §5.5.2 Table 7
pub const CAP_PROPERTY_EXCHANGE: u8 = 1 << 3;

/// Category bit D4: Process Inquiry Supported. // M2-101 §5.5.2 Table 7
pub const CAP_PROCESS_INQUIRY: u8 = 1 << 4;

// --- Endpoint Inquiry Status — M2-101 §5.7.1 Table 10 ---

/// Endpoint Information Status 0x00: Product Instance ID. // M2-101 §5.7.1 Table 10
pub const ENDPOINT_STATUS_PRODUCT_INSTANCE_ID: u8 = 0x00;

/// Maximum Product Instance ID length in bytes. // M2-101 §5.8.3.1
pub const PRODUCT_INSTANCE_ID_MAX_LEN: usize = 42;

// --- Function Block sentinel — M2-101 §5.6.2 ---

/// Function Block value meaning “no Function Block”. // M2-101 §5.6.2
pub const FUNCTION_BLOCK_NONE: u8 = 0x7F;

// --- ACK Status Codes — M2-101 §5.10.2 Table 14 ---

/// ACK Status 0x00: ACK (success). // M2-101 §5.10.2 Table 14
pub const ACK_STATUS_ACK: u8 = 0x00;

/// ACK Status 0x10: Timeout Wait (Status Data = wait × 100 ms). // M2-101 §5.10.2 Table 14
pub const ACK_STATUS_TIMEOUT_WAIT: u8 = 0x10;

/// ACK Status 0x11: Flow Control: Send next Chunks. // M2-101 §5.10.2 Table 14
pub const ACK_STATUS_FLOW_SEND_NEXT: u8 = 0x11;

// --- NAK Status Codes — M2-101 §5.11.2 Table 16 ---

/// NAK Status 0x00: NAK. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_NAK: u8 = 0x00;

/// NAK Status 0x01: MIDI-CI message not supported. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_NOT_SUPPORTED: u8 = 0x01;

/// NAK Status 0x02: MIDI-CI version not supported. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_VERSION_NOT_SUPPORTED: u8 = 0x02;

/// NAK Status 0x03: Channel/Group/Function Block not in use. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_ADDRESS_NOT_IN_USE: u8 = 0x03;

/// NAK Status 0x04: Profile not supported on requested address. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_PROFILE_NOT_SUPPORTED: u8 = 0x04;

/// NAK Status 0x12: Flow Control: Resend most recent Chunk. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_FLOW_RESEND_CHUNK: u8 = 0x12;

/// NAK Status 0x20: Terminates Transaction (Data 0x00 Responder / 0x01 Initiator). // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_TERMINATE_TRANSACTION: u8 = 0x20;

/// NAK Status 0x21: PE chunks out of sequence. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_PE_CHUNKS_OUT_OF_SEQUENCE: u8 = 0x21;

/// NAK Status 0x40: Error occurred, please retry. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_RETRY: u8 = 0x40;

/// NAK Status 0x41: Message was malformed. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_MALFORMED: u8 = 0x41;

/// NAK Status 0x42: Timeout has occurred. // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_TIMEOUT: u8 = 0x42;

/// NAK Status 0x43: Busy, try again (Data = wait × 100 ms). // M2-101 §5.11.2 Table 16
pub const NAK_STATUS_BUSY: u8 = 0x43;

/// Minimum receivable SysEx size for all MIDI-CI Devices. // M2-101 §5.5.3
pub const MIN_RECEIVABLE_SYSEX_SIZE: u32 = 128;
