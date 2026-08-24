//! Errors for the ALSA UMP transport.

use std::fmt;

/// Transport / ALSA / UMP bridging errors.
#[derive(Debug)]
pub enum TransportError {
    /// ALSA library call failed (`errno`-style negative code).
    Alsa { op: &'static str, code: i32 },
    /// Invalid configuration (empty name, bad group, etc.).
    Config(&'static str),
    /// UMP / SysEx7 framing error.
    Ump(&'static str),
    /// Engine / CI error surfaced on the control path.
    Engine(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alsa { op, code } => write!(f, "ALSA {op} failed ({code})"),
            Self::Config(m) => write!(f, "config: {m}"),
            Self::Ump(m) => write!(f, "UMP: {m}"),
            Self::Engine(m) => write!(f, "engine: {m}"),
        }
    }
}

impl std::error::Error for TransportError {}

pub type Result<T> = std::result::Result<T, TransportError>;

pub(crate) fn alsa_check(op: &'static str, code: i32) -> Result<()> {
    if code < 0 {
        Err(TransportError::Alsa { op, code })
    } else {
        Ok(())
    }
}
