//! Property Exchange (PE) resource model, chunking, Mcoded7, and JSON codecs.
//!
//! Phase 2: Mcoded7, chunker, reassembler.
//! Phase 4: PE Capabilities/Get pipeline, resources, JSON headers (`docs/ARD-001.md` §4–§5).
//! Phase 7: Set + Subscriptions + Notify fan-out (M2-101 §8.9–8.13, M2-103 §8.2/§11).
//!
//! ## Example: serve `DeviceInfo` from the registry
//!
//! ```
//! use midici_core::DeviceIdentity;
//! use midici_pe::{PeQuery, ResourceRegistry};
//!
//! let identity = DeviceIdentity {
//!     manufacturer: [0x7D, 0, 0],
//!     family: 1,
//!     model: 2,
//!     software_revision: [1, 0, 0, 0],
//! };
//! let registry = ResourceRegistry::with_device_info(identity);
//! let payload = registry.get("DeviceInfo", &PeQuery::default()).unwrap();
//! // Property bodies are 7-bit JSON. // M2-103 §6.2.1
//! assert!(payload.body.iter().all(|b| *b <= 0x7F));
//! assert!(registry.get("Nope", &PeQuery::default()).is_err());
//! ```
//!
//! ## Example: custom writable + subscribable resource
//!
//! ```
//! use midici_pe::{Payload, PeQuery, PeResult, PropertyResource, ResourceRegistry};
//!
//! struct Mode;
//! impl PropertyResource for Mode {
//!     fn resource(&self) -> &str { "Mode" }
//!     fn get(&self, _: &PeQuery) -> PeResult<Payload> {
//!         Ok(Payload { body: b"\"live\"".to_vec() })
//!     }
//!     // `set` defaults to read-only (405); `subscribable` defaults to false.
//! }
//! let mut registry = ResourceRegistry::new();
//! registry.register(Box::new(Mode));
//! assert!(registry.subscribable("Mode") == false);
//! assert!(registry.set("Mode", &PeQuery::default(), b"\"x\"").is_err()); // 405
//! ```

#![no_std]

extern crate alloc;

pub mod chunker;
pub mod controller;
pub mod error;
pub mod frame;
pub mod json_header;
pub mod mcoded7;
pub mod reassembler;
pub mod registry;
pub mod resource;
pub mod responder;
pub mod status;
pub mod subscriptions;
pub mod wire;

#[cfg(feature = "zlib")]
pub mod zlib_codec;

pub use chunker::{
    clamp_max_sysex, first_chunk_property_capacity, later_chunk_property_capacity,
    pe_payload_budget, split, MAX_SYSEX_MAX, MAX_SYSEX_MIN,
};
pub use controller::{InquiryKind, NotifyBody, PeController, PeEvent};
pub use error::PeError;
pub use frame::{PeChunk, PE_FRAMING_LEN, REQUEST_ID_MAX};
pub use json_header::{
    encode_inquiry_header, encode_reply_header, encode_set_inquiry_header, encode_sub_reply_header,
    encode_subscription_header, parse_get_inquiry_header, parse_notify_header,
    parse_set_inquiry_header, parse_subscription_header, GetInquiryHeader, GetReplyHeader,
    NotifyHeader, SetInquiryHeader, SubCommand, SubReplyHeader, SubscriptionHeader,
    MAX_HEADER_JSON_BYTES, MAX_HEADER_JSON_DEPTH, NOTIFY_STATUS_TERMINATE, NOTIFY_STATUS_TIMEOUT,
    NOTIFY_STATUS_TIMEOUT_WAIT,
};
pub use mcoded7::{decode as mcoded7_decode, encode as mcoded7_encode};
pub use reassembler::{
    MemoryStats, ReassembleEvent, Reassembler, INACTIVITY_TIMEOUT_MS, MAX_CONCURRENT_PER_PEER,
    MAX_TX_BYTES,
};
pub use registry::ResourceRegistry;
pub use resource::{DeviceInfoResource, Payload, PeQuery, PropertyResource, ResourceListResource};
pub use responder::ResponderEngine;
pub use status::{PeResult, PeStatus};
pub use subscriptions::{
    SubId, Subscription, SubscriptionTable, MAX_SUBSCRIPTIONS, MAX_SUBSCRIPTIONS_PER_PEER,
    SUB_ID_LEN,
};

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
