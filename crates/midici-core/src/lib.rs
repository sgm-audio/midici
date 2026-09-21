//! Sans-io MIDI Capability Inquiry (MIDI-CI) state machines.
//!
//! Phase 3 exposes the ARD §3 `CiEngine` surface for Management flows.
//! Phase 4 adds PE Capabilities / Get message codecs (`pe_caps`, `pe_get`).
//! Protocol constants cite M2-101-UM sections (`AGENTS.md` §6).
//!
//! Inbound contract matches ARD §3: feed complete SysEx7 bodies with F0/F7 stripped.

#![no_std]

extern crate alloc;

pub mod config;
pub mod engine;
pub mod error;
pub mod event;
pub mod header;
pub mod mgmt;
pub mod muid;
pub mod pe_caps;
pub mod pe_get;
pub mod pe_msg;
pub mod spec;

pub use config::{CiConfig, PeerState};
pub use engine::CiEngine;
pub use error::CiError;
pub use event::{CapFlags, CiEvent, DeviceIdentity, NakCode, OutboundSysex};
pub use header::CiHeader;
pub use mgmt::{
    mgmt_header, Ack, AckNakBody, Discovery, EndpointInquiry, EndpointReply, InvalidateMuid,
    MgmtMessage, Nak, ReplyToDiscovery,
};
pub use muid::Muid;
pub use pe_caps::{pe_caps_inquiry, pe_caps_reply, PeCapabilities};
pub use pe_get::PeGetMessage;
pub use pe_msg::PeMessage;

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
