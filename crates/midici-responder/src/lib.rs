//! Ergonomic MIDI-CI responder façade (poll-driven timers, rings, event pump).
//!
//! For 0.1.0 this crate re-exports the complete responder engine from
//! `midici-pe`; transport-agnostic event-pump helpers land next.
//! See `docs/ARD-001.md` §2–§3.

pub use midici_pe::{
    InquiryKind, NotifyBody, Payload, PeController, PeEvent, PeQuery, PeStatus, PropertyResource,
    ResourceRegistry, ResponderEngine, SubId, Subscription, SubscriptionTable,
};

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

    #[test]
    fn responder_engine_is_constructible_through_facade() {
        use midici_core::{CiConfig, DeviceIdentity};

        struct Seeded(u32);
        impl rand_core::RngCore for Seeded {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
                self.0
            }
            fn next_u64(&mut self) -> u64 {
                (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
            }
            fn fill_bytes(&mut self, buf: &mut [u8]) {
                let mut n = 0;
                while n < buf.len() {
                    let b = self.next_u32().to_le_bytes();
                    let take = b.len().min(buf.len() - n);
                    buf[n..n + take].copy_from_slice(&b[..take]);
                    n += take;
                }
            }
            fn try_fill_bytes(&mut self, b: &mut [u8]) -> Result<(), rand_core::Error> {
                self.fill_bytes(b);
                Ok(())
            }
        }

        let identity = DeviceIdentity {
            manufacturer: [0x7D, 0, 0],
            family: 1,
            model: 2,
            software_revision: [1, 0, 0, 0],
        };
        let mut engine = ResponderEngine::new(
            CiConfig::responder_default(identity),
            Seeded(0xC0FFEE),
            ResourceRegistry::new(),
        );
        assert!(engine.next_outbound().is_none());
    }
}
