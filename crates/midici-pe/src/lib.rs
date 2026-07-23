//! Property Exchange (PE) resource model, chunking, Mcoded7, and JSON codecs.
//!
//! Implements the PE layer described in `docs/ARD-001.md` §2 / §5. Protocol
//! codecs arrive in later phases; this crate currently exposes identity metadata.

#![no_std]

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
