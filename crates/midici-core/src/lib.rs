//! Sans-io MIDI Capability Inquiry (MIDI-CI) state machines.
//!
//! Phase 3 exposes the ARD §3 `CiEngine` surface for Management flows.
//! Phase 4 adds PE Capabilities / Get message codecs (`pe_caps`, `pe_get`).
//! Protocol constants cite M2-101-UM sections (`AGENTS.md` §6).
//!
//! Inbound contract matches ARD §3: feed complete SysEx7 bodies with F0/F7 stripped.
//!
//! ## Example: answer a peer's Discovery
//!
//! ```
//! use midici_core::{CiConfig, CiEngine, CiEvent, DeviceIdentity, Discovery, Muid, mgmt_header};
//!
//! // Any `rand_core::RngCore` works; deterministic here for the docs.
//! struct Seeded(u32);
//! impl rand_core::RngCore for Seeded {
//!     fn next_u32(&mut self) -> u32 {
//!         self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
//!         self.0
//!     }
//!     fn next_u64(&mut self) -> u64 {
//!         (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
//!     }
//!     fn fill_bytes(&mut self, buf: &mut [u8]) {
//!         let mut n = 0;
//!         while n < buf.len() {
//!             let b = self.next_u32().to_le_bytes();
//!             let take = b.len().min(buf.len() - n);
//!             buf[n..n + take].copy_from_slice(&b[..take]);
//!             n += take;
//!         }
//!     }
//!     fn try_fill_bytes(&mut self, b: &mut [u8]) -> Result<(), rand_core::Error> {
//!         self.fill_bytes(b);
//!         Ok(())
//!     }
//! }
//!
//! let identity = DeviceIdentity {
//!     manufacturer: [0x7D, 0, 0],
//!     family: 1,
//!     model: 2,
//!     software_revision: [1, 0, 0, 0],
//! };
//! let mut eng = CiEngine::new(CiConfig::responder_default(identity), Seeded(1234));
//!
//! // A peer's broadcast Discovery arrives (SysEx body, F0/F7 stripped).
//! let peer = Muid::ordinary(0x01020304).unwrap();
//! let discovery = Discovery {
//!     header: mgmt_header(midici_core::spec::SUB_ID2_DISCOVERY, peer, Muid::BROADCAST),
//!     manufacturer: [0x7D, 0, 0],
//!     family: 5,
//!     model: 6,
//!     software_revision: [1, 0, 0, 0],
//!     category_supported: midici_core::spec::CAP_PROPERTY_EXCHANGE,
//!     max_sysex_size: 512,
//!     output_path_id: 0,
//! };
//! let mut buf = [0u8; 64];
//! let n = discovery.encode(&mut buf).unwrap();
//! eng.feed_sysex(0, &buf[..n]).unwrap();
//!
//! // The peer is discovered, and a Reply to Discovery is queued.
//! assert!(matches!(eng.next_event(), Some(CiEvent::PeerDiscovered { muid, .. }) if muid == peer));
//! assert!(eng.next_outbound().is_some());
//! ```

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
