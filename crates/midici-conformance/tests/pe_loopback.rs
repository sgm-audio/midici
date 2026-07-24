//! Dual-engine PE loopback: Discovery → PE Caps → Get ResourceList → Get DeviceInfo.
//! Test initiator shim is test-only (not a published API). // ARD §4 / §5 / Phase 4 DoD

mod common;

use midici_core::{CiConfig, DeviceIdentity, Muid};
use midici_pe::{
    encode_reply_header, split, DeviceInfoResource, PeQuery, PeStatus, PropertyResource,
    ResourceRegistry, ResponderEngine,
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
