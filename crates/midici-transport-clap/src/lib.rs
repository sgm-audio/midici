//! CLAP plugin transport adapter with RT-safe bridge. // ARD §6
//!
//! ## ChCtrlList resource (§5)
//!
//! Built on the main thread from `clap_plugin_params`. Each parameter is
//! assigned a controller type, index, and range.
//!
//! ## Features
//!
//! - `clap-ffi` (off by default): enables `InputEvents` / `OutputEvents`
//!   wrappers and `ControlBridge`. Requires `clap-sys`.
//!
//! See [`ChCtrlList`] and the [`chctrllist`] module.

pub mod chctrllist;
pub mod ring;

pub use ring::{Consumer, DefaultRing, Producer, Ring};

// ── CLAP FFI wrappers (feature-gated) ────────────────────────

#[cfg(feature = "clap-ffi")]
mod clap_ffi;

#[cfg(feature = "clap-ffi")]
pub use clap_ffi::{ControlBridge, InputEvents, OutputEvents};

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_roundtrip_sysex_body() {
        let ring_in: Ring<64, 512> = Ring::new();
        let ring_out: Ring<64, 512> = Ring::new();
        let mut in_prod = unsafe { Producer::new(&ring_in) };
        let mut in_cons = unsafe { Consumer::new(&ring_in) };
        let mut out_prod = unsafe { Producer::new(&ring_out) };
        let mut out_cons = unsafe { Consumer::new(&ring_out) };
        let body = [0x7E, 0x7F, 0x0D, 0x70, 0x02];
        assert!(in_prod.push(&body));
        let data = in_cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], &body[..]);
        assert!(out_prod.push(b"hello from control"));
        let data = out_cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], b"hello from control");
    }

    #[test]
    fn assert_no_alloc_sysex_flood_drop_counter() {
        let ring: Ring<8, 512> = Ring::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };
        for i in 0..8 {
            let mut data = [0u8; 512];
            data[0] = i as u8;
            assert!(prod.push(&data));
        }
        assert_eq!(ring.drops(), 0);
        for _ in 0..500 {
            prod.push(&[0xFF; 512]);
        }
        assert_eq!(ring.drops(), 500);
        for i in 0..8 {
            let data = cons.pop().unwrap();
            assert_eq!(data[0], i as u8);
        }
        assert!(cons.pop().is_none());
        assert_eq!(ring.drops(), 500);
        ring.reset_drops();
        assert_eq!(ring.drops(), 0);
    }

    #[cfg(feature = "clap-ffi")]
    #[test]
    fn control_bridge_construction() {
        use midici_core::{CiConfig, DeviceIdentity};
        use rand_core::SeedableRng;
        use rand_xorshift::XorShiftRng;
        let identity = DeviceIdentity {
            manufacturer: [0x00, 0x11, 0x22],
            family: 0x0102,
            model: 0x0304,
            software_revision: [0x01, 0x00, 0x00, 0x00],
        };
        let config = CiConfig::responder_default(identity);
        let rng = XorShiftRng::seed_from_u64(42);
        let engine = midici_core::CiEngine::new(config, rng);
        let bridge = ControlBridge::new(engine);
        assert_eq!(bridge.pending_len, 0);
        let _muid = bridge.engine.muid();
    }
}
