//! Ergonomic MIDI-CI responder façade (poll-driven timers, rings, event pump).
//!
//! See `docs/ARD-001.md` §2–§3. Application-facing APIs land in later phases;
//! this crate currently exposes identity metadata only.

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
