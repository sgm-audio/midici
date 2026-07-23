//! Sans-io MIDI Capability Inquiry (MIDI-CI) state machines.
//!
//! Designed for `no_std` + `alloc` targets. Protocol machines are introduced in
//! later phases; this crate currently exposes only crate identity metadata.
//!
//! See `docs/ARD-001.md` §2–§3 for workspace role and layering.

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
