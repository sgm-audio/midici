#![no_main]

use libfuzzer_sys::fuzz_target;
use midici_pe::{mcoded7_decode, mcoded7_encode};

fuzz_target!(|data: &[u8]| {
    let enc = mcoded7_encode(data);
    assert!(enc.iter().all(|b| *b <= 0x7F));
    if let Ok(dec) = mcoded7_decode(&enc) {
        assert_eq!(dec, data);
    }
    // Decoder must not panic on arbitrary input.
    let _ = mcoded7_decode(data);
});
