//! Sans-io MIDI Capability Inquiry (MIDI-CI) state machines.
//!
//! Phase 1 exposes Management message framing only (`docs/ARD-001.md` §4).
//! Protocol constants cite M2-101-UM sections (`AGENTS.md` §6).
//!
//! Inbound contract matches ARD §3: feed complete SysEx7 bodies with F0/F7 stripped.

#![no_std]

pub mod error;
pub mod header;
pub mod mgmt;
pub mod muid;
pub mod spec;

pub use error::CiError;
pub use header::CiHeader;
pub use mgmt::{
    mgmt_header, Ack, AckNakBody, Discovery, EndpointInquiry, EndpointReply, InvalidateMuid,
    MgmtMessage, Nak, ReplyToDiscovery,
};
pub use muid::Muid;

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
