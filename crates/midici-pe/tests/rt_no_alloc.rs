//! RT no-allocation invariant for the PE inbound hot path. // ARD §3
//!
//! Drives a real multi-chunk Set inquiry through [`ResponderEngine`] with a
//! counting global allocator installed (`midici-test-alloc`), and asserts:
//!
//! 1. **A partial feed is strictly allocation-free.** Chunk 1 of 2 is
//!    decoded as a borrowed [`midici_pe::PeChunkRef`] and its fragment is
//!    written into the reassembler's pre-reserved slot buffers; the live
//!    allocation count does not move at all.
//! 2. **Completing the transaction is allocation-net-zero.** The final chunk
//!    transiently allocates (reassembled body, serde parses, reply chunks) —
//!    all freed within the call, except the reply (outbound queue) and the
//!    PropertySet event (event queue); draining both returns the live count
//!    to the baseline.
//!
//! Setup (Discovery + PE Caps + one full warmup exchange) runs outside the
//! measured window so every buffer on the hot path (reassembler slots,
//! `encode_buf`, peer rows, in-flight `active` lists, queue rings) is
//! already sized. `poll` is deliberately never called inside the window.

#[global_allocator]
static GLOBAL: midici_test_alloc::Counting = midici_test_alloc::Counting;

use midici_core::spec::{CAP_PROPERTY_EXCHANGE, SUB_ID2_PE_SET_REPLY};
use midici_core::{
    mgmt_header, pe_caps_inquiry, CiConfig, CiHeader, Discovery, Muid, PeGetMessage,
};
use midici_pe::{encode_set_inquiry_header, split, ResponderEngine, ResourceRegistry};

/// Deterministic, allocation-free RNG for the engine's nonce paths.
struct Seeded(u32);

impl rand_core::RngCore for Seeded {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        self.0
    }

    fn next_u64(&mut self) -> u64 {
        (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
    }

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut n = 0;
        while n < buf.len() {
            let b = self.next_u32().to_le_bytes();
            let take = b.len().min(buf.len() - n);
            buf[n..n + take].copy_from_slice(&b[..take]);
            n += take;
        }
    }

    fn try_fill_bytes(&mut self, b: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(b);
        Ok(())
    }
}

/// Writable test resource. Validates the reassembled payload byte-for-byte
/// (rejecting corruption with 400) without allocating.
struct Writable {
    expect_len: usize,
}

impl midici_pe::PropertyResource for Writable {
    fn resource(&self) -> &str {
        "Writable"
    }

    fn get(&self, _req: &midici_pe::PeQuery) -> midici_pe::PeResult<midici_pe::Payload> {
        Err(midici_pe::PeStatus::NotFound)
    }

    fn set(&mut self, _req: &midici_pe::PeQuery, body: &[u8]) -> midici_pe::PeResult<()> {
        if body.len() != self.expect_len || body.iter().any(|&b| b != b'a') {
            return Err(midici_pe::PeStatus::BadRequest);
        }
        Ok(())
    }
}

/// Peer-broadcast Discovery. // M2-101 §5.4
fn discovery(peer: Muid) -> Vec<u8> {
    let msg = Discovery {
        header: mgmt_header(midici_core::spec::SUB_ID2_DISCOVERY, peer, Muid::BROADCAST),
        manufacturer: [0x7D, 0, 0],
        family: 1,
        model: 2,
        software_revision: [1, 0, 0, 0],
        category_supported: CAP_PROPERTY_EXCHANGE,
        max_sysex_size: 512,
        output_path_id: 0,
    };
    let mut buf = vec![0u8; 64];
    let n = msg.encode(&mut buf).unwrap();
    buf.truncate(n);
    buf
}

/// Wrap one PE chunk payload in a full SysEx body (F0/F7 stripped) carrying
/// an Inquiry: Set Property Data. // M2-101 §8.9
fn set_frame(peer: Muid, dest: Muid, pe_payload: &[u8]) -> Vec<u8> {
    let msg = PeGetMessage {
        header: CiHeader {
            device_id: midici_core::spec::DEVICE_ID_FUNCTION_BLOCK,
            sub_id2: midici_core::spec::SUB_ID2_PE_SET_INQUIRY,
            version: midici_core::spec::MESSAGE_FORMAT_VERSION_1_2,
            source: peer,
            dest,
        },
        pe_payload: pe_payload.to_vec(),
    };
    msg.to_vec().unwrap()
}

/// Drain the outbound queue (setup phase only), returning frames drained.
fn drain_outbound(eng: &mut ResponderEngine<Seeded>) -> usize {
    let mut n = 0;
    while let Some(o) = eng.next_outbound() {
        let _ = o;
        n += 1;
    }
    n
}

/// Drain all pending application events (setup phase only).
fn drain_events(eng: &mut ResponderEngine<Seeded>) -> usize {
    let mut n = 0;
    while let Some(ev) = eng.next_pe_event() {
        let _ = ev;
        n += 1;
    }
    while let Some(ev) = eng.next_event() {
        let _ = ev;
        n += 1;
    }
    n
}

#[test]
fn inbound_partial_feed_is_alloc_free_and_completion_is_net_zero() {
    // ---- Setup: outside the measured window --------------------------------
    let identity = midici_core::DeviceIdentity {
        manufacturer: [0x7D, 0, 0],
        family: 1,
        model: 2,
        software_revision: [1, 0, 0, 0],
    };
    // 150 property bytes force exactly two chunks at a 128-byte SysEx
    // (budget 113, first-chunk capacity 81 with the 23-byte Set header).
    let prop: Vec<u8> = vec![b'a'; 150];
    let mut registry = ResourceRegistry::with_device_info(identity);
    registry.register(Box::new(Writable {
        expect_len: prop.len(),
    }));
    let mut eng = ResponderEngine::new(
        CiConfig::responder_default(identity),
        Seeded(7),
        registry,
    );
    let peer = Muid::ordinary(0x0102_0304).unwrap();

    // Discovery + PE Caps: establish the peer (CI table row + PE caps row,
    // incl. the pre-reserved in-flight list) before measuring anything.
    eng.feed_sysex(0, &discovery(peer)).unwrap();
    drain_outbound(&mut eng);
    let caps = pe_caps_inquiry(peer, eng.muid(), 4);
    let mut caps_buf = vec![0u8; 64];
    let n = caps.encode(&mut caps_buf).unwrap();
    caps_buf.truncate(n);
    eng.feed_sysex(0, &caps_buf).unwrap();
    drain_outbound(&mut eng);
    drain_events(&mut eng);

    // Warmup exchange (request ID 5): one complete multi-chunk Set, so the
    // completion path (assemble, parse, encode, chunk, queue, event) has run
    // once and every scratch buffer is at steady size.
    let header = encode_set_inquiry_header("Writable", None, false).unwrap();
    let warm = split(5, &header, &prop, 128).unwrap();
    assert_eq!(warm.len(), 2, "harness must be a 2-chunk transaction");
    for c in &warm {
        eng.feed_sysex(0, &set_frame(peer, eng.muid(), c)).unwrap();
    }
    drain_outbound(&mut eng);
    drain_events(&mut eng);

    // Measured exchange (request ID 9), built before the baseline.
    let measured = split(9, &header, &prop, 128).unwrap();
    assert_eq!(measured.len(), 2);
    let frame1 = set_frame(peer, eng.muid(), &measured[0]);
    let frame2 = set_frame(peer, eng.muid(), &measured[1]);

    // ---- Invariant 1: partial feed is strictly allocation-free ------------
    let baseline = midici_test_alloc::live();
    eng.feed_sysex(0, &frame1).unwrap();
    assert_eq!(
        midici_test_alloc::live(),
        baseline,
        "chunk 1 of 2 must be handled without any heap allocation"
    );

    // ---- Invariant 2: completion is net-zero after draining ---------------
    eng.feed_sysex(0, &frame2).unwrap();
    // Drain the reply (outbound queue) and the PropertySet event (event
    // queue); each frame/event is dropped the moment it is inspected, so its
    // buffer is freed before the final count.
    let mut saw_200 = false;
    while let Some(o) = eng.next_outbound() {
        let (h, _) = CiHeader::decode(&o.body).expect("reply frame decodes");
        if h.sub_id2 == SUB_ID2_PE_SET_REPLY {
            assert!(
                o.body.windows(12).any(|w| w == b"\"status\":200"),
                "Set reply must be status 200, got {:?}",
                String::from_utf8_lossy(&o.body)
            );
            saw_200 = true;
        }
        // `o` drops at end of iteration, freeing the queued frame.
    }
    assert!(saw_200, "no Set reply was queued");
    drain_events(&mut eng);
    assert_eq!(
        midici_test_alloc::live(),
        baseline,
        "completed inbound exchange must be allocation-net-zero after drain"
    );
}
