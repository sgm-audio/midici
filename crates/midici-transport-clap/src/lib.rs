//! CLAP plugin transport adapter with RT-safe bridge. // ARD §6


pub mod ring;

pub use ring::{Consumer, DefaultRing, Producer, Ring};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_roundtrip() {
        let ring: Ring<64, 512> = Ring::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };
        let body = [0x7E, 0x7F, 0x0D, 0x70, 0x02];
        assert!(prod.push(&body));
        let data = cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], &body[..]);
    }

    #[test]
    fn overflow_drops() {
        let ring: Ring<8, 512> = Ring::new();
        let mut prod = unsafe { Producer::new(&ring) };
        for _ in 0..8 { assert!(prod.push(&[0xAA; 64])); }
        for _ in 0..50 { prod.push(&[0xFF; 64]); }
        assert_eq!(ring.drops(), 50);
    }
}
