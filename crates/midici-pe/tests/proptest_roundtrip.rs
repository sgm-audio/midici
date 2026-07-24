//! Property tests: split ∘ reassemble = id across chunk sizes and orderings.

use midici_core::Muid;
use midici_pe::{split, PeChunk, ReassembleEvent, Reassembler};
use proptest::prelude::*;
use rand::seq::SliceRandom;
use rand::{rngs::StdRng, SeedableRng};

const CHUNK_SIZES: [u32; 5] = [128, 256, 512, 1024, 4096];

fn ordinary_muid(v: u32) -> Muid {
    Muid::ordinary(v % Muid::ORDINARY_END).unwrap()
}

fn reassemble_shuffled(peer: Muid, chunks: &[Vec<u8>], seed: u64) -> (Vec<u8>, Vec<u8>) {
    let mut order: Vec<usize> = (0..chunks.len()).collect();
    let mut rng = StdRng::seed_from_u64(seed);
    order.shuffle(&mut rng);

    let mut ra = Reassembler::new(1);
    let mut done = None;
    let mut now = 0u64;
    for &i in &order {
        now += 1;
        match ra.feed(peer, &chunks[i], now).unwrap() {
            Some(ReassembleEvent::Complete { header, body, .. }) => {
                done = Some((header, body));
                break;
            }
            Some(ReassembleEvent::Timeout { .. }) => panic!("unexpected timeout"),
            None => {}
        }
    }
    done.expect("reassembly did not complete")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn split_reassemble_identity(
        body_len in 0usize..=64 * 1024,
        header_len in 0usize..64usize,
        req in 0u8..128u8,
        size_idx in 0usize..CHUNK_SIZES.len(),
        seed in any::<u64>(),
    ) {
        let max_sysex = CHUNK_SIZES[size_idx];
        // 7-bit-safe header/body (ASCII path); chunker does not Mcoded7-encode.
        let header = vec![0x41u8; header_len];
        let body: Vec<u8> = (0..body_len).map(|i| (i % 95 + 0x20) as u8).collect();

        // Header must fit in first chunk at this size.
        prop_assume!(midici_pe::first_chunk_property_capacity(max_sysex, header.len()).is_ok());

        let chunks = split(req, &header, &body, max_sysex).expect("split");
        for raw in &chunks {
            prop_assert!(raw.len() <= midici_pe::pe_payload_budget(max_sysex));
            let _ = PeChunk::decode(raw).expect("decode chunk");
        }

        let peer = ordinary_muid(0x11);
        let (got_header, got_body) = reassemble_shuffled(peer, &chunks, seed);
        prop_assert_eq!(got_header, header);
        prop_assert_eq!(got_body, body);
    }

    #[test]
    fn mcoded7_roundtrip(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        let enc = midici_pe::mcoded7_encode(&data);
        prop_assert!(enc.iter().all(|b| *b <= 0x7F));
        let dec = midici_pe::mcoded7_decode(&enc).expect("decode");
        prop_assert_eq!(dec, data);
    }
}

#[test]
fn edges_empty_and_max_body() {
    let peer = ordinary_muid(7);
    for &size in &CHUNK_SIZES {
        for &blen in &[0usize, 1, 64 * 1024] {
            let header = b"{}".to_vec();
            if midici_pe::first_chunk_property_capacity(size, header.len()).is_err() {
                continue;
            }
            let body = vec![0x30u8; blen];
            let chunks = split(9, &header, &body, size).unwrap();
            let (h, b) = reassemble_shuffled(peer, &chunks, 0xC0FFEE);
            assert_eq!(h, header);
            assert_eq!(b, body);
        }
    }
}

#[test]
fn reverse_order_multi_chunk() {
    let peer = ordinary_muid(3);
    let header = b"{\"status\":200}";
    let body = vec![0x22u8; 2500];
    let mut chunks = split(2, header, &body, 128).unwrap();
    assert!(chunks.len() > 2);
    chunks.reverse();
    let mut ra = Reassembler::new(1);
    let mut done = None;
    for (i, c) in chunks.iter().enumerate() {
        if let Some(ev) = ra.feed(peer, c, i as u64).unwrap() {
            done = Some(ev);
        }
    }
    match done.unwrap() {
        ReassembleEvent::Complete {
            header: h, body: b, ..
        } => {
            assert_eq!(h, header);
            assert_eq!(b, body);
        }
        other => panic!("unexpected {other:?}"),
    }
}
