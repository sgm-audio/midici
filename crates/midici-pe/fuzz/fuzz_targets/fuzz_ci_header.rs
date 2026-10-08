#![no_main]

use libfuzzer_sys::fuzz_target;
use rand_core::RngCore;

use midici_core::{CiConfig, CiEngine, CiHeader, DeviceIdentity};

/// Deterministic RngCore (matches the midici-core doctest pattern) so the
/// engine under test is fully reproducible.
#[derive(Clone, Copy)]
struct Seeded(u64);

impl RngCore for Seeded {
    fn next_u64(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let v = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&v[..chunk.len()]);
        }
    }
    fn try_fill_bytes(&mut self, buf: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(buf);
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    // Header parse must never panic on arbitrary bytes.
    let _ = CiHeader::decode(data);

    // Full engine dispatch must never panic, and — since malformed input is
    // NAK 0x41 or a silent drop — must never return an error either.
    let identity = DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 1,
        model: 2,
        software_revision: [1, 0, 0, 0],
    };
    let mut eng = CiEngine::new(CiConfig::responder_default(identity), Seeded(0xC0FFEE));
    assert!(eng.feed_sysex(0, data).is_ok());
    // Drain so repeated feeds on the same engine state also stay clean.
    while let Some(_out) = eng.next_outbound() {}
    while let Some(_ev) = eng.next_event() {}
});
