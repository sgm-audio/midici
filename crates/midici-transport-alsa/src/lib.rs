//! ALSA UMP sequencer transport adapter (Linux).
//!
//! Phase 5 / ARD §2 / §9 Slice 0–1: virtual UMP endpoint, SysEx7 bridging, 10 ms
//! control loop feeding [`midici_pe::ResponderEngine`].
//!
//! # ALSA binding decision
//!
//! - `alsa` 0.12: rawmidi `Ump` open/read/write only — **no** sequencer
//!   `set_ump_endpoint_info` / `ump_event_*` wrappers.
//! - `alsa-sys` 0.6: exposes the full UMP sequencer symbol set used here.
//!
//! Therefore this crate wraps `alsa-sys` UMP sequencer symbols in a thin safe
//! module (`ump_seq`). No hand-rolled ioctls.

#![cfg(target_os = "linux")]

pub mod control_loop;
pub mod error;
pub mod sysex7_ump;
pub mod ump_seq;

pub use control_loop::{run_responder_loop, LoopOptions, CONTROL_TICK};
pub use error::{Result, TransportError};
pub use ump_seq::{sequencer_available, EndpointConfig, UmpSeqEndpoint};

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_nonempty() {
        assert!(!crate::VERSION.is_empty());
    }
}
