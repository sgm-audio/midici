//! CLAP plugin transport adapter (RT-safe bridge).
//!
//! See `docs/ARD-001.md` §2 / §6. RT bridge implementation lands in later phases;
//! this crate currently exposes identity metadata only.

#![allow(unsafe_code)]

pub mod ring;

pub use ring::{Consumer, DefaultRing, Producer, Ring};

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
    fn ring_roundtrip() {
        let ring: crate::ring::Ring<64, 512> = crate::ring::Ring::new();
        let mut prod = unsafe { crate::ring::Producer::new(&ring) };
        let mut cons = unsafe { crate::ring::Consumer::new(&ring) };
        let body = [0x7E, 0x7F, 0x0D, 0x70, 0x02];
        assert!(prod.push(&body));
        let data = cons.peek().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], &body[..]);
        cons.commit();
    }
}
