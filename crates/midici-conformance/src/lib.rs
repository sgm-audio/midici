//! Conformance harness: golden transcripts, fuzz targets, and interop checks.
//!
//! See `docs/ARD-001.md` §8. Goldens under `goldens/` are append-only
//! (`AGENTS.md` rule 7).

use std::fs;
use std::path::{Path, PathBuf};

pub mod transcript;

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Absolute path to the goldens directory in this crate.
pub const GOLDENS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/goldens");

/// Parse a `.hex` golden: `#` comments, whitespace-separated hex bytes.
pub fn parse_hex_golden(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for tok in line.split_whitespace() {
            let b = u8::from_str_radix(tok, 16)
                .map_err(|e| format!("line {}: bad hex '{}': {}", lineno + 1, tok, e))?;
            out.push(b);
        }
    }
    if out.is_empty() {
        return Err("no bytes in golden".into());
    }
    Ok(out)
}

/// Load all `goldens/mgmt/*.hex` files, sorted by name.
pub fn load_mgmt_goldens() -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let dir = Path::new(GOLDENS_DIR).join("mgmt");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("hex"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let text = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        let bytes = parse_hex_golden(&text)?;
        out.push((p, bytes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midici_core::MgmtMessage;

    #[test]
    fn goldens_dir_exists() {
        assert!(Path::new(GOLDENS_DIR).is_dir());
    }

    #[test]
    fn mgmt_goldens_byte_exact_roundtrip() {
        let goldens = load_mgmt_goldens().expect("load goldens");
        assert!(
            goldens.len() >= 7,
            "expected ≥7 management goldens, got {}",
            goldens.len()
        );
        for (path, bytes) in goldens {
            let decoded = MgmtMessage::decode(&bytes)
                .unwrap_or_else(|e| panic!("{}: decode {:?}", path.display(), e));
            let mut buf = vec![0u8; bytes.len() + 64];
            let n = decoded
                .encode(&mut buf)
                .unwrap_or_else(|e| panic!("{}: encode {:?}", path.display(), e));
            assert_eq!(
                &buf[..n],
                bytes.as_slice(),
                "byte-exact mismatch for {}",
                path.display()
            );
            let again = MgmtMessage::decode(&buf[..n]).unwrap();
            let mut buf2 = vec![0u8; n];
            let n2 = again.encode(&mut buf2).unwrap();
            assert_eq!(&buf2[..n2], &buf[..n]);
        }
    }

    #[test]
    fn exchange_transcripts_byte_exact_replay() {
        let exchanges = transcript::load_exchange_transcripts().expect("load exchanges");
        assert!(
            exchanges.len() >= 3,
            "expected ≥3 exchange transcripts, got {}",
            exchanges.len()
        );
        for (path, t) in exchanges {
            transcript::replay(&t).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
    }
}
