//! Property Exchange encodings, chunker, and reassembler.
//!
//! Phase 2 implements ARD §4 (Mcoded7, optional `zlib+Mcoded7`, chunking) and
//! ARD §6/§7 reassembly caps (“chunk flood”, “stalled tx”).
//!
//! Protocol field layouts cite M2-101-UM / M2-103-UM (`AGENTS.md` §6).

#![no_std]

extern crate alloc;

pub mod chunker;
pub mod error;
pub mod frame;
pub mod mcoded7;
pub mod reassembler;
pub mod wire;

#[cfg(feature = "zlib")]
pub mod zlib_codec;

pub use chunker::{
    clamp_max_sysex, first_chunk_property_capacity, later_chunk_property_capacity,
    pe_payload_budget, split, MAX_SYSEX_MAX, MAX_SYSEX_MIN,
};
pub use error::PeError;
pub use frame::{PeChunk, PE_FRAMING_LEN, REQUEST_ID_MAX};
pub use mcoded7::{decode as mcoded7_decode, encode as mcoded7_encode};
pub use reassembler::{
    MemoryStats, ReassembleEvent, Reassembler, INACTIVITY_TIMEOUT_MS, MAX_CONCURRENT_PER_PEER,
    MAX_TX_BYTES,
};

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
