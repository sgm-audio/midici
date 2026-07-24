//! Decode/encode errors for MIDI-CI framing.

/// Errors from parsing or serializing MIDI-CI bodies (F0/F7 stripped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiError {
    /// Buffer shorter than the fixed header. // M2-101 §5.2.1 Table 5
    Truncated,
    /// First byte is not Universal Non-Realtime SysEx (`0x7E`). // M2-101 §5.2.1 Table 5
    NotUniversalSysex,
    /// Sub-ID#1 is not MIDI-CI (`0x0D`). // M2-101 §5.2.1 Table 5
    NotMidiCi,
    /// Sub-ID#2 is not a Management message handled here. // M2-101 Appendix E
    UnsupportedSubId2(u8),
    /// Payload length does not match the message definition.
    BadLength,
    /// A field contained a value outside the range allowed by M2-101.
    BadField,
    /// Output buffer is too small to encode the message.
    BufferTooSmall,
    /// MUID value is reserved or not representable in 28 bits. // M2-101 §3.3
    InvalidMuid,
}
