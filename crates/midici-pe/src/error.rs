//! Errors for PE encodings and chunk reassembly.

/// Property Exchange encode/decode and reassembly errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeError {
    /// A 7-bit field contained a value with bit 7 set, or a value out of range.
    BadField,
    /// Buffer shorter than required by the PE chunk framing. // M2-101 Tables 33–35
    Truncated,
    /// Output buffer is too small.
    BufferTooSmall,
    /// Header cannot fit entirely in the first chunk at the negotiated size.
    /// // M2-101 §8.3.1 / M2-103 §5.2
    HeaderTooLarge,
    /// Property payload exceeds the 64 KiB per-transaction cap. // ARD §6 / §7
    Oversize,
    /// More than four concurrent reassemblies for one peer. // ARD §6 / §7
    TooManyConcurrent,
    /// Fixed peer table is full and every row still has an active reassembly.
    ///
    /// Distinct from [`Self::TooManyConcurrent`] (per-peer request cap). Callers
    /// must not map this to MIDI-CI busy/445. // ARD §6 memory model
    PeerTableFull,
    /// `numChunks` / `chunkNum` inconsistent with prior chunks of the transaction.
    InconsistentChunking,
    /// Mcoded7 input length is not a valid encoding (empty group remainder).
    BadMcoded7,
    /// zlib inflate/deflate failed (`zlib` feature).
    #[cfg(feature = "zlib")]
    Zlib,
}
