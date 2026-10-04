//! Dual-engine PE loopback: Discovery → PE Caps → Get ResourceList → Get DeviceInfo.
//! Test initiator shim is test-only (not a published API). // ARD §4 / §5 / Phase 4 DoD

mod common;

use midici_core::{CiConfig, DeviceIdentity, Muid, Nak, NakCode};
use midici_pe::{
    encode_reply_header, split, DeviceInfoResource, Payload, PeQuery, PeResult, PeStatus,
    PropertyResource, ResourceRegistry, ResponderEngine, MAX_TX_BYTES,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

use common::{
    collect_get_reply_payload, drain_all, parse_pe_caps, parse_reply_to_discovery, TestInitiator,
};

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 3,
        model: 4,
        software_revision: [2, 0, 0, 0],
    }
}

struct LargeResource {
    body: Vec<u8>,
}

impl PropertyResource for LargeResource {
    fn resource(&self) -> &str {
        "Large"
    }

    fn get(&self, _req: &PeQuery) -> PeResult<Payload> {
        Ok(Payload {
            body: self.body.clone(),
        })
    }
}

fn make_responder(max_sysex: u32) -> ResponderEngine<StdRng> {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_sysex_size = max_sysex;
    let registry = ResourceRegistry::with_device_info(identity());
    ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry)
}

fn expected_device_info() -> Vec<u8> {
    DeviceInfoResource::new(identity())
        .get(&PeQuery::default())
        .unwrap()
        .body
}

fn expected_resource_list() -> Vec<u8> {
    let reg = ResourceRegistry::with_device_info(identity());
    reg.get("ResourceList", &PeQuery::default()).unwrap().body
}

fn run_loopback(max_sysex: u32) {
    let mut responder = make_responder(max_sysex);
    let initiator_muid = Muid::ordinary(0x01020304).unwrap();
    let mut init = TestInitiator::new(initiator_muid, max_sysex);

    // Discovery
    responder
        .feed_sysex(0, &init.discovery())
        .expect("discovery feed");
    let outs = drain_all(&mut responder);
    assert_eq!(outs.len(), 1, "expected Reply to Discovery");
    let reply = parse_reply_to_discovery(&outs[0].body).expect("reply to discovery");
    let dest = reply.header.source;
    assert_eq!(dest, responder.muid());

    // PE Caps
    responder
        .feed_sysex(0, &init.pe_caps(dest))
        .expect("pe caps feed");
    let outs = drain_all(&mut responder);
    assert_eq!(outs.len(), 1, "expected PE Caps Reply");
    let caps = parse_pe_caps(&outs[0].body).expect("pe caps reply");
    assert_eq!(caps.simultaneous_requests, 4);
    assert_eq!(
        responder.pe_mut().simultaneous_for(initiator_muid),
        Some(4.min(init.simultaneous))
    );

    // Get ResourceList
    let chunks = init.get(dest, "ResourceList");
    for c in &chunks {
        responder.feed_sysex(0, c).expect("get ResourceList");
    }
    let outs = drain_all(&mut responder);
    let (hdr, body) = collect_get_reply_payload(&outs);
    assert_eq!(hdr.status, PeStatus::Ok.as_u16());
    assert_eq!(body, expected_resource_list());
    let reply_hdr = encode_reply_header(PeStatus::Ok, None).unwrap();
    let expected_chunks = split(1, &reply_hdr, &body, max_sysex).unwrap();
    assert_eq!(
        outs.len(),
        expected_chunks.len(),
        "ResourceList chunk count at max_sysex={max_sysex}"
    );

    // Get DeviceInfo
    let chunks = init.get(dest, "DeviceInfo");
    for c in &chunks {
        responder.feed_sysex(0, c).expect("get DeviceInfo");
    }
    let outs = drain_all(&mut responder);
    let (hdr, body) = collect_get_reply_payload(&outs);
    assert_eq!(hdr.status, PeStatus::Ok.as_u16());
    assert_eq!(body, expected_device_info());
    let reply_hdr = encode_reply_header(PeStatus::Ok, None).unwrap();
    let expected_chunks = split(2, &reply_hdr, &body, max_sysex).unwrap();
    assert_eq!(
        outs.len(),
        expected_chunks.len(),
        "DeviceInfo chunk count at max_sysex={max_sysex}"
    );
    // Payload equality already asserted; also verify each PE payload matches chunker.
    for (i, (got, want)) in outs.iter().zip(expected_chunks.iter()).enumerate() {
        let msg = midici_core::PeGetMessage::decode(&got.body).unwrap();
        assert_eq!(
            &msg.pe_payload, want,
            "DeviceInfo PE chunk {i} payload mismatch at max_sysex={max_sysex}"
        );
    }
}

#[test]
fn pe_loopback_max_sysex_128() {
    run_loopback(128);
}

#[test]
fn pe_loopback_max_sysex_4096() {
    run_loopback(4096);
}

#[test]
fn pe_get_for_another_destination_is_silently_dropped() {
    let mut responder = make_responder(512);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let other = Muid::ordinary(0x0506_0708).unwrap();
    let mut init = TestInitiator::new(peer, 512);

    for body in init.get(other, "DeviceInfo") {
        responder.feed_sysex(0, &body).unwrap();
    }

    assert!(drain_all(&mut responder).is_empty());
}

#[test]
fn unknown_pe_sub_id_gets_not_supported_nak() {
    let mut responder = make_responder(512);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let mut init = TestInitiator::new(peer, 512);
    let mut request = init
        .get(responder.muid(), "DeviceInfo")
        .into_iter()
        .next()
        .expect("single-chunk Get");
    request[3] = 0x3A; // reserved/unknown PE-category Sub-ID#2

    responder.feed_sysex(0, &request).unwrap();
    let outs = drain_all(&mut responder);
    assert_eq!(outs.len(), 1);
    let nak = Nak::decode(&outs[0].body).unwrap();
    assert_eq!(nak.body.status_code, NakCode::NotSupported.to_u8());
}

#[test]
fn pe_message_with_reserved_ci_version_bits_gets_version_nak() {
    let mut responder = make_responder(512);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let mut init = TestInitiator::new(peer, 512);
    let mut request = init
        .get(responder.muid(), "DeviceInfo")
        .into_iter()
        .next()
        .expect("single-chunk Get");
    request[4] |= midici_core::spec::MESSAGE_FORMAT_VERSION_RESERVED_MASK;

    responder.feed_sysex(0, &request).unwrap();
    let outs = drain_all(&mut responder);
    assert_eq!(outs.len(), 1);
    let nak = Nak::decode(&outs[0].body).unwrap();
    assert_eq!(nak.body.status_code, NakCode::VersionNotSupported.to_u8());
}

#[test]
fn maximum_sized_get_reply_is_not_truncated_by_output_queue() {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_peers = 1;
    cfg.max_sysex_size = 128;
    let expected = format!("\"{}\"", "x".repeat(MAX_TX_BYTES - 2)).into_bytes();
    assert_eq!(expected.len(), MAX_TX_BYTES);

    let mut registry = ResourceRegistry::with_device_info(identity());
    registry.register(Box::new(LargeResource {
        body: expected.clone(),
    }));
    let mut responder = ResponderEngine::new(
        cfg,
        StdRng::seed_from_u64(0xC0FFEE),
        registry,
    );
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let mut init = TestInitiator::new(peer, 128);

    responder.feed_sysex(0, &init.discovery()).unwrap();
    let discovery_out = drain_all(&mut responder);
    let dest = parse_reply_to_discovery(&discovery_out[0].body)
        .expect("Reply to Discovery")
        .header
        .source;
    responder.feed_sysex(0, &init.pe_caps(dest)).unwrap();
    let _ = drain_all(&mut responder);

    for body in init.get(dest, "Large") {
        responder.feed_sysex(0, &body).unwrap();
    }
    let outs = drain_all(&mut responder);
    assert!(outs.len() > 64, "test payload must exceed the old queue cap");
    let (header, body) = collect_get_reply_payload(&outs);
    assert_eq!(header.status, PeStatus::Ok.as_u16());
    assert_eq!(body, expected);
}
