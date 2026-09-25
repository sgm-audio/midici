//! PE Get status / NAK matrix. // M2-103 §7.4.1; ARD §7; Phase 4 DoD
//!
//! Covers typed [`PeStatus`] codes: 400/403/404/405/413/445/341 (plus 200 covered in loopback).

mod common;

use midici_core::spec::{
    DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2, SUB_ID2_PE_GET_INQUIRY,
};
use midici_core::{CiConfig, CiHeader, DeviceIdentity, Muid, PeGetMessage};
use midici_pe::{
    encode_inquiry_header, DeviceInfoResource, Payload, PeChunk, PeQuery, PeStatus,
    PropertyResource, ResourceRegistry, ResponderEngine, INACTIVITY_TIMEOUT_MS, MAX_TX_BYTES,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

use common::{collect_get_reply_payload, drain_all, parse_reply_to_discovery, TestInitiator};

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 3,
        model: 4,
        software_revision: [2, 0, 0, 0],
    }
}

fn setup(max_sysex: u32) -> (ResponderEngine<StdRng>, TestInitiator, Muid) {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_sysex_size = max_sysex;
    let registry = ResourceRegistry::with_device_info(identity());
    let mut responder = ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry);
    let initiator_muid = Muid::ordinary(0x01020304).unwrap();
    let init = TestInitiator::new(initiator_muid, max_sysex);
    responder.feed_sysex(0, &init.discovery()).unwrap();
    let outs = drain_all(&mut responder);
    let reply = parse_reply_to_discovery(&outs[0].body).unwrap();
    let dest = reply.header.source;
    responder.feed_sysex(0, &init.pe_caps(dest)).unwrap();
    let _ = drain_all(&mut responder);
    (responder, init, dest)
}

fn assert_status(outs: &[midici_core::OutboundSysex], want: PeStatus) {
    let (hdr, body) = collect_get_reply_payload(outs);
    assert_eq!(hdr.status, want.as_u16(), "body={:?}", body);
    assert!(body.is_empty() || want == PeStatus::Ok);
}

#[test]
fn nak_404_unknown_resource() {
    let (mut responder, mut init, dest) = setup(512);
    for c in init.get(dest, "NoSuchResource") {
        responder.feed_sysex(0, &c).unwrap();
    }
    assert_status(&drain_all(&mut responder), PeStatus::NotFound);
}

#[test]
fn nak_400_malformed_json_header() {
    let (mut responder, init, dest) = setup(512);
    // Hand-craft Get with broken header JSON.
    let pe = PeChunk {
        request_id: 9,
        header: b"{not-json".to_vec(),
        num_chunks: 1,
        chunk_num: 1,
        property: Vec::new(),
    }
    .to_vec()
    .unwrap();
    let body = PeGetMessage {
        header: CiHeader {
            device_id: DEVICE_ID_FUNCTION_BLOCK,
            sub_id2: SUB_ID2_PE_GET_INQUIRY,
            version: MESSAGE_FORMAT_VERSION_1_2,
            source: init.muid,
            dest,
        },
        pe_payload: pe,
    }
    .to_vec()
    .unwrap();
    responder.feed_sysex(0, &body).unwrap();
    assert_status(&drain_all(&mut responder), PeStatus::BadRequest);
}

#[test]
fn nak_403_forbidden() {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_sysex_size = 512;
    let mut registry = ResourceRegistry::with_device_info(identity());
    let mut di = DeviceInfoResource::new(identity());
    di.forbidden = true;
    registry.register(Box::new(di));
    let mut responder = ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry);
    let initiator_muid = Muid::ordinary(0x01020304).unwrap();
    let mut init = TestInitiator::new(initiator_muid, 512);
    responder.feed_sysex(0, &init.discovery()).unwrap();
    let outs = drain_all(&mut responder);
    let dest = parse_reply_to_discovery(&outs[0].body)
        .unwrap()
        .header
        .source;
    responder.feed_sysex(0, &init.pe_caps(dest)).unwrap();
    let _ = drain_all(&mut responder);
    for c in init.get(dest, "DeviceInfo") {
        responder.feed_sysex(0, &c).unwrap();
    }
    assert_status(&drain_all(&mut responder), PeStatus::Forbidden);
}

#[test]
fn nak_405_not_allowed() {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_sysex_size = 512;
    let mut registry = ResourceRegistry::with_device_info(identity());
    let mut di = DeviceInfoResource::new(identity());
    di.not_allowed = true;
    registry.register(Box::new(di));
    let mut responder = ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry);
    let initiator_muid = Muid::ordinary(0x01020304).unwrap();
    let mut init = TestInitiator::new(initiator_muid, 512);
    responder.feed_sysex(0, &init.discovery()).unwrap();
    let outs = drain_all(&mut responder);
    let dest = parse_reply_to_discovery(&outs[0].body)
        .unwrap()
        .header
        .source;
    responder.feed_sysex(0, &init.pe_caps(dest)).unwrap();
    let _ = drain_all(&mut responder);
    for c in init.get(dest, "DeviceInfo") {
        responder.feed_sysex(0, &c).unwrap();
    }
    assert_status(&drain_all(&mut responder), PeStatus::NotAllowed);
}

/// Resource that returns a body larger than [`MAX_TX_BYTES`].
struct HugeResource;

impl PropertyResource for HugeResource {
    fn resource(&self) -> &str {
        "HugeBlob"
    }
    fn get(&self, _req: &PeQuery) -> Result<Payload, PeStatus> {
        Ok(Payload {
            body: vec![0x20u8; MAX_TX_BYTES + 1],
        })
    }
}

#[test]
fn nak_413_oversize_payload() {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_sysex_size = 512;
    let mut registry = ResourceRegistry::with_device_info(identity());
    registry.register(Box::new(HugeResource));
    let mut responder = ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry);
    let initiator_muid = Muid::ordinary(0x01020304).unwrap();
    let mut init = TestInitiator::new(initiator_muid, 512);
    responder.feed_sysex(0, &init.discovery()).unwrap();
    let outs = drain_all(&mut responder);
    let dest = parse_reply_to_discovery(&outs[0].body)
        .unwrap()
        .header
        .source;
    responder.feed_sysex(0, &init.pe_caps(dest)).unwrap();
    let _ = drain_all(&mut responder);
    for c in init.get(dest, "HugeBlob") {
        responder.feed_sysex(0, &c).unwrap();
    }
    assert_status(&drain_all(&mut responder), PeStatus::PayloadTooLarge);
}

#[test]
fn nak_445_fifth_simultaneous() {
    let (mut responder, mut init, dest) = setup(128);
    // Open 4 incomplete multi-chunk Gets, then a 5th should Busy(445).
    let mut partials = Vec::new();
    for _ in 0..4 {
        let (_rid, body) = init.get_partial_multi(dest, "DeviceInfo");
        partials.push(body);
    }
    for p in &partials {
        responder.feed_sysex(0, p).unwrap();
        // Incomplete → no GetReply yet.
        assert!(
            drain_all(&mut responder).is_empty(),
            "partial multi-chunk must not complete"
        );
    }
    for c in init.get(dest, "DeviceInfo") {
        responder.feed_sysex(0, &c).unwrap();
    }
    assert_status(&drain_all(&mut responder), PeStatus::Busy);
}

#[test]
fn nak_341_stall_via_injected_time() {
    let (mut responder, mut init, dest) = setup(128);
    let (rid, body) = init.get_partial_multi(dest, "DeviceInfo");
    responder.feed_sysex(0, &body).unwrap();
    assert!(drain_all(&mut responder).is_empty());
    // Inject time past inactivity timeout. // M2-103 §12.2 / ARD §7
    responder.poll(1 + INACTIVITY_TIMEOUT_MS);
    let outs = drain_all(&mut responder);
    let (hdr, _) = collect_get_reply_payload(&outs);
    assert_eq!(hdr.status, PeStatus::Unavailable.as_u16());
    // Correlate request id on first reply chunk.
    let msg = PeGetMessage::decode(&outs[0].body).unwrap();
    let chunk = PeChunk::decode(&msg.pe_payload).unwrap();
    assert_eq!(chunk.request_id, rid);
}

#[test]
fn malformed_json_never_panics_through_engine() {
    let (mut responder, init, dest) = setup(512);
    let header = encode_inquiry_header("DeviceInfo").unwrap();
    // Corrupt: valid size but invalid JSON content already tested; also empty header via raw.
    let pe = PeChunk {
        request_id: 3,
        header: b"".to_vec(), // empty → missing resource → 400
        num_chunks: 1,
        chunk_num: 1,
        property: Vec::new(),
    }
    .to_vec()
    .unwrap();
    let body = PeGetMessage {
        header: CiHeader {
            device_id: DEVICE_ID_FUNCTION_BLOCK,
            sub_id2: SUB_ID2_PE_GET_INQUIRY,
            version: MESSAGE_FORMAT_VERSION_1_2,
            source: init.muid,
            dest,
        },
        pe_payload: pe,
    }
    .to_vec()
    .unwrap();
    let _ = header; // keep encode path exercised in this file
    responder.feed_sysex(0, &body).unwrap();
    assert_status(&drain_all(&mut responder), PeStatus::BadRequest);
}
