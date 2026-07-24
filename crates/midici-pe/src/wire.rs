//! 7-bit LSB-first integer packing used by PE length / chunk fields.
//! // M2-101 Tables 33–35 (LSB first); same packing as M2-101 §5.5.1 family fields.

use crate::error::PeError;

/// Maximum value of a 14-bit PE field (`nh`, `nd`, `numChunks`, `chunkNum`).
pub const U14_MAX: u16 = 0x3FFF;

/// Decode a 14-bit value from two 7-bit LSB-first bytes. // M2-101 Tables 33–35
#[inline]
pub fn read_u14_le7(bytes: &[u8]) -> Result<u16, PeError> {
    if bytes.len() < 2 {
        return Err(PeError::Truncated);
    }
    if bytes[0] > 0x7F || bytes[1] > 0x7F {
        return Err(PeError::BadField);
    }
    Ok(u16::from(bytes[0]) | (u16::from(bytes[1]) << 7))
}

/// Encode a 14-bit value as two 7-bit LSB-first bytes. // M2-101 Tables 33–35
#[inline]
pub fn write_u14_le7(v: u16, out: &mut [u8]) -> Result<(), PeError> {
    if out.len() < 2 {
        return Err(PeError::BufferTooSmall);
    }
    if v > U14_MAX {
        return Err(PeError::BadField);
    }
    out[0] = (v & 0x7F) as u8;
    out[1] = ((v >> 7) & 0x7F) as u8;
    Ok(())
}
