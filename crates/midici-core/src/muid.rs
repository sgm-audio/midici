//! MIDI Unique Identifier (MUID). // M2-101 §3.3

use crate::error::CiError;
use rand_core::RngCore;

/// 28-bit MIDI Unique Identifier. // M2-101 §3.3
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Muid(u32);

impl Muid {
    /// Broadcast MUID value `0x0FFFFFFF`. // M2-101 §3.3.3
    pub const BROADCAST_VALUE: u32 = 0x0FFF_FFFF;

    /// Exclusive upper bound of ordinary (non-reserved) MUID values. // M2-101 §3.3
    ///
    /// Spec: values `0x0FFFFF00`..=`0x0FFFFFFE` are reserved; `0x0FFFFFFF` is broadcast.
    pub const ORDINARY_END: u32 = 0x0FFF_FF00;

    /// Mask for the 28-bit MUID field. // M2-101 §3.3
    pub const MASK_28: u32 = 0x0FFF_FFFF;

    /// Broadcast MUID. // M2-101 §3.3.3
    pub const BROADCAST: Self = Self(Self::BROADCAST_VALUE);

    /// Create from a raw 28-bit value. Rejects values above 28 bits. // M2-101 §3.3
    #[inline]
    pub const fn from_u32(value: u32) -> Result<Self, CiError> {
        if value & !Self::MASK_28 != 0 {
            return Err(CiError::InvalidMuid);
        }
        Ok(Self(value))
    }

    /// Create an ordinary (non-reserved, non-broadcast) MUID. // M2-101 §3.3
    #[inline]
    pub const fn ordinary(value: u32) -> Result<Self, CiError> {
        if value >= Self::ORDINARY_END {
            return Err(CiError::InvalidMuid);
        }
        Self::from_u32(value)
    }

    /// Raw 28-bit value.
    #[inline]
    pub const fn to_u32(self) -> u32 {
        self.0
    }

    /// True if this is the Broadcast MUID. // M2-101 §3.3.3
    #[inline]
    pub const fn is_broadcast(self) -> bool {
        self.0 == Self::BROADCAST_VALUE
    }

    /// True if this value is in the reserved range `0x0FFFFF00`..=`0x0FFFFFFE`. // M2-101 §3.3
    #[inline]
    pub const fn is_reserved(self) -> bool {
        self.0 >= Self::ORDINARY_END && self.0 < Self::BROADCAST_VALUE
    }

    /// Generate a new ordinary MUID from an injected RNG. // M2-101 §3.3.1
    ///
    /// Retries until the 28-bit candidate is outside the reserved/broadcast range.
    pub fn generate<R: RngCore>(rng: &mut R) -> Self {
        loop {
            let candidate = rng.next_u32() & Self::MASK_28;
            if candidate < Self::ORDINARY_END {
                return Self(candidate);
            }
        }
    }

    /// Encode as 4×7-bit LSB-first bytes. // M2-101 §5.2.1 Table 5 / §3.3.3
    ///
    /// Broadcast `0x0FFFFFFF` encodes as `7F 7F 7F 7F`. // M2-101 §3.3.3
    #[inline]
    pub fn encode_7bit_le(self) -> [u8; 4] {
        let v = self.0;
        [
            (v & 0x7F) as u8,
            ((v >> 7) & 0x7F) as u8,
            ((v >> 14) & 0x7F) as u8,
            ((v >> 21) & 0x7F) as u8,
        ]
    }

    /// Decode 4×7-bit LSB-first bytes into a MUID. // M2-101 §5.2.1 Table 5
    #[inline]
    pub fn decode_7bit_le(bytes: &[u8; 4]) -> Result<Self, CiError> {
        for b in bytes {
            if *b > 0x7F {
                return Err(CiError::BadField);
            }
        }
        let v = u32::from(bytes[0])
            | (u32::from(bytes[1]) << 7)
            | (u32::from(bytes[2]) << 14)
            | (u32::from(bytes[3]) << 21);
        Self::from_u32(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn broadcast_encodes_as_four_7f() {
        // M2-101 §3.3.3
        assert_eq!(Muid::BROADCAST.encode_7bit_le(), [0x7F, 0x7F, 0x7F, 0x7F]);
        assert_eq!(
            Muid::decode_7bit_le(&[0x7F, 0x7F, 0x7F, 0x7F]).unwrap(),
            Muid::BROADCAST
        );
    }

    #[test]
    fn generate_is_deterministic_for_seed() {
        let mut a = StdRng::seed_from_u64(0xC1C1_C1C1);
        let mut b = StdRng::seed_from_u64(0xC1C1_C1C1);
        assert_eq!(Muid::generate(&mut a), Muid::generate(&mut b));
    }

    #[test]
    fn generate_stays_ordinary() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..256 {
            let m = Muid::generate(&mut rng);
            assert!(!m.is_broadcast());
            assert!(!m.is_reserved());
            assert!(m.to_u32() < Muid::ORDINARY_END);
        }
    }

    #[test]
    fn roundtrip_7bit() {
        let m = Muid::ordinary(0x0123_4567).unwrap();
        assert_eq!(Muid::decode_7bit_le(&m.encode_7bit_le()).unwrap(), m);
    }
}
