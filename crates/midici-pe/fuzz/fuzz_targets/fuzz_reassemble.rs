#![no_main]

use libfuzzer_sys::fuzz_target;
use midici_core::Muid;
use midici_pe::Reassembler;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let peer = match Muid::ordinary(u32::from(data[0])) {
        Ok(m) => m,
        Err(_) => return,
    };
    let mut ra = Reassembler::new(2);
    let mut now = 0u64;
    // Split input into pseudo-chunks of varying lengths.
    let mut rest = &data[1..];
    while !rest.is_empty() {
        let n = (rest[0] as usize % 64).max(1).min(rest.len());
        let (chunk, next) = rest.split_at(n);
        rest = next;
        now = now.saturating_add(1);
        let _ = ra.feed(peer, chunk, now);
        if now % 17 == 0 {
            let _ = ra.poll(now.saturating_add(3_000));
        }
    }
});
