//! Conformance harness: golden transcripts, fuzz targets, and interop checks.
//!
//! See `docs/ARD-001.md` §8. Goldens under `goldens/` are append-only
//! (`AGENTS.md` rule 7). This crate currently exposes identity metadata only.

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Absolute path to the goldens directory in this crate.
pub const GOLDENS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/goldens");

#[cfg(test)]
mod tests {
    #[test]
    fn goldens_dir_exists() {
        assert!(std::path::Path::new(crate::GOLDENS_DIR).is_dir());
    }
}
