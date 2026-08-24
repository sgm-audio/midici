//! Cap enforcement: oversize, 5th concurrent, timeout reclaim (accounting harness).

use midici_core::Muid;
use midici_pe::{
    split, PeError, ReassembleEvent, Reassembler, INACTIVITY_TIMEOUT_MS, MAX_CONCURRENT_PER_PEER,
    MAX_TX_BYTES,
};

fn peer(v: u32) -> Muid {
    Muid::ordinary(v).unwrap()
}

/// Minimal valid multi-chunk first fragment for an incomplete transaction.
fn open_tx(req: u8, body_hint: usize) -> Vec<u8> {
    // One-byte header keeps first-chunk property capacity high at size 128.
    let header = b"{}";
    let body = vec![0x31u8; body_hint.max(2000)];
    let chunks = split(req, header, &body, 128).unwrap();
    assert!(chunks.len() >= 2);
    chunks[0].clone()
}

#[test]
fn oversize_transaction_rejected() {
    let mut ra = Reassembler::new(1);
    let p = peer(1);
    // Craft a single chunk claiming a huge property payload via raw framing that
    // exceeds MAX_TX_BYTES when accumulated. Use legitimate split pieces that
    // sum over the cap by feeding many max-sized property chunks under one requestId.
    let header = b"{}";
    // Build synthetic chunks with known num_chunks that would exceed 64 KiB.
    // Chunker won't emit >64KiB bodies in our helpers; assemble manually.
    use midici_pe::PeChunk;
    let piece = vec![0x20u8; 4096];
    let num = (MAX_TX_BYTES / piece.len()) as u16 + 2;
    let mut now = 0u64;
    let mut rejected = false;
    for n in 1..=num {
        let chunk = PeChunk {
            request_id: 1,
            header: if n == 1 { header.to_vec() } else { Vec::new() },
            num_chunks: num,
            chunk_num: n,
            property: piece.clone(),
        };
        now += 1;
        match ra.feed(p, &chunk.to_vec().unwrap(), now) {
            Err(PeError::Oversize) => {
                rejected = true;
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("unexpected error {e:?}"),
        }
    }
    assert!(
        rejected,
        "expected Oversize once property exceeds {MAX_TX_BYTES}"
    );
}

#[test]
fn fifth_concurrent_rejected() {
    let mut ra = Reassembler::new(1);
    let p = peer(2);
    let reserved_before = ra.memory_stats().reserved_bytes;
    for req in 0..MAX_CONCURRENT_PER_PEER as u8 {
        let raw = open_tx(req, 2000);
        assert!(ra.feed(p, &raw, req as u64).unwrap().is_none());
    }
    let stats = ra.memory_stats();
    assert_eq!(stats.active_slots, MAX_CONCURRENT_PER_PEER);
    assert_eq!(stats.reserved_bytes, reserved_before);

    let fifth = open_tx(MAX_CONCURRENT_PER_PEER as u8, 2000);
    assert_eq!(
        ra.feed(p, &fifth, 100).unwrap_err(),
        PeError::TooManyConcurrent
    );
}

#[test]
fn timeout_eviction_reclaims_slot_and_active_bytes() {
    let mut ra = Reassembler::new(2);
    let p = peer(3);
    let reserved0 = ra.reserved_bytes();
    assert!(reserved0 > 0);

    let raw = open_tx(5, 4000);
    assert!(ra.feed(p, &raw, 1_000).unwrap().is_none());
    let mid = ra.memory_stats();
    assert_eq!(mid.active_slots, 1);
    assert!(mid.active_bytes > 0);
    assert_eq!(mid.reserved_bytes, reserved0);

    let events = ra.poll(1_000 + INACTIVITY_TIMEOUT_MS);
    assert_eq!(
        events,
        vec![ReassembleEvent::Timeout {
            peer: p,
            request_id: 5
        }]
    );

    let after = ra.memory_stats();
    assert_eq!(after.active_slots, 0);
    assert_eq!(after.active_bytes, 0);
    // Pre-reserved capacities are retained (no heap churn); logical reclaim only.
    assert_eq!(after.reserved_bytes, reserved0);

    // Slot is reusable after timeout reclaim.
    let raw2 = open_tx(6, 4000);
    assert!(ra.feed(p, &raw2, 10_000).unwrap().is_none());
    assert_eq!(ra.memory_stats().active_slots, 1);
}

#[test]
fn peer_table_full_is_not_too_many_concurrent() {
    // One peer slot in the table; fill it with an incomplete tx, then a different
    // MUID must surface PeerTableFull — not the per-peer TooManyConcurrent busy path.
    let mut ra = Reassembler::new(1);
    let p0 = peer(1);
    let p1 = peer(2);
    let raw0 = open_tx(1, 2000);
    assert!(ra.feed(p0, &raw0, 1).unwrap().is_none());
    let raw1 = open_tx(1, 2000);
    assert_eq!(
        ra.feed(p1, &raw1, 2).unwrap_err(),
        PeError::PeerTableFull
    );
}

/// Allocator-counting harness: reserved bytes are fixed at construction and do
/// not grow across feed / timeout / reuse cycles (capacities pre-reserved).
#[test]
fn allocator_counting_harness_stable_reservation() {
    let mut ra = Reassembler::new(1);
    let baseline = ra.memory_stats();
    let p = peer(9);

    for cycle in 0..8u64 {
        let req = (cycle % 4) as u8;
        let raw = open_tx(req, 3000);
        let _ = ra.feed(p, &raw, cycle * 10_000).unwrap();
        let _ = ra.poll(cycle * 10_000 + INACTIVITY_TIMEOUT_MS);
        let s = ra.memory_stats();
        assert_eq!(
            s.reserved_bytes, baseline.reserved_bytes,
            "reservation grew on cycle {cycle}"
        );
        assert_eq!(s.active_slots, 0);
        assert_eq!(s.active_bytes, 0);
    }
}
