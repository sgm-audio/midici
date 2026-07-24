//! State-machine model tests: (state, input) → (state', outputs).
//!
//! Citations for NAK vs silent-drop behavior are inline per `AGENTS.md` §5.

use midici_core::spec::{
    CI_HEADER_LEN, MESSAGE_FORMAT_VERSION_1_1, MESSAGE_FORMAT_VERSION_1_2,
    MESSAGE_FORMAT_VERSION_RESERVED_MASK, SUB_ID2_DISCOVERY, SUB_ID2_INVALIDATE_MUID, SUB_ID2_NAK,
    SUB_ID2_REPLY_TO_DISCOVERY, UNIVERSAL_NON_REALTIME,
};
use midici_core::{
    mgmt_header, AckNakBody, CapFlags, CiConfig, CiEngine, CiEvent, DeviceIdentity, Discovery,
    InvalidateMuid, Muid, Nak, NakCode, ReplyToDiscovery,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        manufacturer: [0x43, 0x00, 0x00],
        family: 3,
        model: 4,
        software_revision: [2, 0, 0, 0],
    }
}

fn cfg() -> CiConfig {
    let mut c = CiConfig::responder_default(identity());
    c.max_peers = 8;
    c.local_group = 0;
    c.max_sysex_size = 512;
    c
}

fn engine(seed: u64) -> CiEngine<StdRng> {
    CiEngine::new(cfg(), StdRng::seed_from_u64(seed))
}

fn drain_out(eng: &mut CiEngine<StdRng>) -> Vec<Vec<u8>> {
    let mut v = Vec::new();
    while let Some(o) = eng.next_outbound() {
        v.push(o.body);
    }
    v
}

fn drain_events(eng: &mut CiEngine<StdRng>) -> Vec<CiEvent> {
    let mut v = Vec::new();
    while let Some(e) = eng.next_event() {
        v.push(e);
    }
    v
}

fn discovery_from(source: Muid, dest: Muid, version: u8, max_sysex: u32, path: u8) -> Vec<u8> {
    let msg = Discovery {
        header: {
            let mut h = mgmt_header(SUB_ID2_DISCOVERY, source, dest);
            h.version = version;
            h
        },
        manufacturer: [0x7D, 0x00, 0x00],
        family: 1,
        model: 2,
        software_revision: [1, 0, 0, 0],
        category_supported: CapFlags(midici_core::spec::CAP_PROPERTY_EXCHANGE).bits(),
        max_sysex_size: max_sysex,
        output_path_id: path,
    };
    let mut buf = vec![0u8; 64];
    let n = msg.encode(&mut buf).unwrap();
    buf.truncate(n);
    buf
}

#[test]
fn broadcast_discovery_replies_and_discovers_peer() {
    let mut eng = engine(0xA11CE);
    let our = eng.muid();
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    assert_ne!(peer, our);

    let inbound = discovery_from(peer, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_2, 512, 0);
    eng.feed_sysex(0, &inbound).unwrap();

    let events = drain_events(&mut eng);
    assert!(matches!(
        &events[..],
        [CiEvent::PeerDiscovered { muid, .. }] if *muid == peer
    ));

    let outs = drain_out(&mut eng);
    assert_eq!(outs.len(), 1);
    let reply = ReplyToDiscovery::decode(&outs[0]).unwrap();
    assert_eq!(reply.header.source, our);
    assert_eq!(reply.header.dest, peer);
    assert_eq!(reply.output_path_id, 0);
    assert_eq!(eng.peers().len(), 1);
    assert_eq!(eng.peers()[0].max_sysex, 512);
}

#[test]
fn directed_discovery_same_as_broadcast_when_addressed_to_us() {
    let mut eng = engine(0xBEEF);
    let our = eng.muid();
    let peer = Muid::ordinary(0x0A0B_0C0D).unwrap();
    let inbound = discovery_from(peer, our, MESSAGE_FORMAT_VERSION_1_2, 1024, 3);
    eng.feed_sysex(1, &inbound).unwrap();
    let outs = drain_out(&mut eng);
    assert_eq!(outs.len(), 1);
    let reply = ReplyToDiscovery::decode(&outs[0]).unwrap();
    assert_eq!(reply.header.dest, peer);
    assert_eq!(reply.output_path_id, 3);
    assert_eq!(eng.peers()[0].max_sysex, 512); // min(1024, ours=512)
}

#[test]
fn unknown_muid_destination_silently_dropped() {
    // M2-101 §5.2.1 Destination MUID: intended receiver only.
    let mut eng = engine(1);
    let stranger = Muid::ordinary(0x0011_2233).unwrap();
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let inbound = discovery_from(peer, stranger, MESSAGE_FORMAT_VERSION_1_2, 512, 0);
    eng.feed_sysex(0, &inbound).unwrap();
    assert!(drain_out(&mut eng).is_empty());
    assert!(drain_events(&mut eng).is_empty());
    assert!(eng.peers().is_empty());
}

#[test]
fn non_ci_sysex_silently_dropped() {
    // M2-101 §5.2.1 Table 5 — only Universal Non-Realtime + Sub-ID#1 0x0D are MIDI-CI.
    let mut eng = engine(2);
    eng.feed_sysex(0, &[0x7F, 0x00, 0x01]).unwrap();
    eng.feed_sysex(0, &[UNIVERSAL_NON_REALTIME, 0x7F, 0x06])
        .unwrap(); // not MIDI-CI
    assert!(drain_out(&mut eng).is_empty());
}

#[test]
fn truncated_header_silently_dropped() {
    // M2-101 §5.2.1 — incomplete header cannot form an addressed NAK.
    let mut eng = engine(3);
    eng.feed_sysex(0, &[UNIVERSAL_NON_REALTIME, 0x7F, 0x0D, 0x70])
        .unwrap();
    assert!(drain_out(&mut eng).is_empty());
}

#[test]
fn reserved_version_bits_nak_02() {
    // M2-101 §5.3 / §5.4 — reserved version bits → NAK status 0x02.
    let mut eng = engine(4);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let mut inbound = discovery_from(peer, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_2, 512, 0);
    inbound[4] |= MESSAGE_FORMAT_VERSION_RESERVED_MASK;
    eng.feed_sysex(0, &inbound).unwrap();
    let outs = drain_out(&mut eng);
    assert_eq!(outs.len(), 1);
    let nak = Nak::decode(&outs[0]).unwrap();
    assert_eq!(nak.body.status_code, NakCode::VersionNotSupported.to_u8());
    assert_eq!(nak.header.dest, peer);
}

#[test]
fn malformed_discovery_payload_nak_41() {
    // M2-101 §5.11 Table 16 — 0x41 Message was malformed.
    let mut eng = engine(5);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let mut inbound = discovery_from(peer, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_2, 512, 0);
    inbound.truncate(CI_HEADER_LEN + 4); // truncated payload after valid header
    eng.feed_sysex(0, &inbound).unwrap();
    let outs = drain_out(&mut eng);
    assert_eq!(outs.len(), 1);
    let nak = Nak::decode(&outs[0]).unwrap();
    assert_eq!(nak.body.status_code, NakCode::Malformed.to_u8());
}

#[test]
fn muid_collision_on_discovery_invalidate_regenerate_reannounce() {
    // M2-101 §5.9.1 Option B / ARD §7.
    let mut eng = engine(0xC0111);
    let our = eng.muid();
    let inbound = discovery_from(our, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_2, 512, 0);
    eng.feed_sysex(0, &inbound).unwrap();
    let outs = drain_out(&mut eng);
    assert!(outs.len() >= 2);
    let inv = InvalidateMuid::decode(&outs[0]).unwrap();
    assert_eq!(inv.target, our);
    assert!(inv.header.dest.is_broadcast());
    let disc = Discovery::decode(&outs[1]).unwrap();
    assert_ne!(disc.header.source, our);
    assert_eq!(eng.muid(), disc.header.source);
}

#[test]
fn peer_invalidate_tears_down_and_emits_event() {
    // M2-101 §5.9 — discard cached info; no reply.
    let mut eng = engine(0xD00D);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    eng.feed_sysex(
        0,
        &discovery_from(peer, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_2, 512, 0),
    )
    .unwrap();
    let _ = drain_out(&mut eng);
    let _ = drain_events(&mut eng);

    let inv = InvalidateMuid {
        header: mgmt_header(SUB_ID2_INVALIDATE_MUID, peer, Muid::BROADCAST),
        target: peer,
    };
    let mut buf = vec![0u8; 32];
    let n = inv.encode(&mut buf).unwrap();
    eng.feed_sysex(0, &buf[..n]).unwrap();
    assert!(drain_out(&mut eng).is_empty()); // no reply // M2-101 §5.9
    let events = drain_events(&mut eng);
    assert_eq!(events, vec![CiEvent::PeerInvalidated { muid: peer }]);
    assert!(eng.peers().is_empty());
}

#[test]
fn self_invalidate_regenerates_and_reannounces() {
    // M2-101 §5.9 — target == our MUID → new MUID + Discovery.
    let mut eng = engine(0xE11);
    let old = eng.muid();
    let inv = InvalidateMuid {
        header: mgmt_header(
            SUB_ID2_INVALIDATE_MUID,
            Muid::ordinary(0x0102_0304).unwrap(),
            Muid::BROADCAST,
        ),
        target: old,
    };
    let mut buf = vec![0u8; 32];
    let n = inv.encode(&mut buf).unwrap();
    eng.feed_sysex(0, &buf[..n]).unwrap();
    assert_ne!(eng.muid(), old);
    let outs = drain_out(&mut eng);
    assert_eq!(outs.len(), 1);
    let disc = Discovery::decode(&outs[0]).unwrap();
    assert_eq!(disc.header.source, eng.muid());
}

#[test]
fn reply_to_discovery_records_peer() {
    let mut eng = engine(0xF00D);
    let our = eng.muid();
    let peer = Muid::ordinary(0x0506_0708).unwrap();
    let reply = ReplyToDiscovery {
        header: mgmt_header(SUB_ID2_REPLY_TO_DISCOVERY, peer, our),
        manufacturer: [0x43, 0, 0],
        family: 9,
        model: 8,
        software_revision: [1, 2, 3, 4],
        category_supported: 0x0C,
        max_sysex_size: 256,
        output_path_id: 0,
        function_block: 0,
    };
    let mut buf = vec![0u8; 64];
    let n = reply.encode(&mut buf).unwrap();
    eng.feed_sysex(0, &buf[..n]).unwrap();
    let events = drain_events(&mut eng);
    assert!(matches!(
        events.as_slice(),
        [CiEvent::PeerDiscovered { muid, caps, .. }] if *muid == peer && caps.bits() == 0x0C
    ));
    assert_eq!(eng.peers()[0].max_sysex, 256); // min(256, 512)
}

#[test]
fn v1_1_peer_never_receives_ack_or_endpoint() {
    // ARD §4 / §7 — feature mask; never version-NAK.
    let mut eng = engine(0x1111);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    eng.feed_sysex(
        0,
        &discovery_from(peer, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_1, 512, 0),
    )
    .unwrap();
    let _ = drain_out(&mut eng);
    let _ = drain_events(&mut eng);
    assert!(!eng.peers()[0].allow_ack());

    eng.send_ack(
        0,
        peer,
        AckNakBody {
            original_sub_id2: SUB_ID2_DISCOVERY,
            status_code: 0,
            status_data: 0,
            details: [0; 5],
            message: &[],
        },
    )
    .unwrap();
    eng.send_endpoint_inquiry(0, peer, 0).unwrap();
    assert!(drain_out(&mut eng).is_empty());
}

#[test]
fn inbound_nak_surfaces_typed_event() {
    let mut eng = engine(0x2222);
    let our = eng.muid();
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    let nak = Nak {
        header: mgmt_header(SUB_ID2_NAK, peer, our),
        body: AckNakBody {
            original_sub_id2: SUB_ID2_DISCOVERY,
            status_code: NakCode::NotSupported.to_u8(),
            status_data: 0,
            details: [0; 5],
            message: &[],
        },
    };
    let mut buf = vec![0u8; 64];
    let n = nak.encode(&mut buf).unwrap();
    eng.feed_sysex(0, &buf[..n]).unwrap();
    assert_eq!(
        drain_events(&mut eng),
        vec![CiEvent::Nak {
            peer,
            original: SUB_ID2_DISCOVERY,
            code: NakCode::NotSupported,
        }]
    );
}

#[test]
fn poll_accepts_monotonic_time() {
    let mut eng = engine(0x3333);
    eng.poll(0);
    eng.poll(10);
    eng.poll(10_000);
}

#[test]
fn sub_min_sysex_discovery_is_malformed() {
    let mut eng = engine(0x4444);
    let peer = Muid::ordinary(0x0102_0304).unwrap();
    // Encode a valid Discovery then patch max SysEx below §5.5.3 floor.
    let mut inbound = discovery_from(peer, Muid::BROADCAST, MESSAGE_FORMAT_VERSION_1_2, 128, 0);
    // max_sysex_size is bytes 12..16 of the payload after CI header.
    inbound[CI_HEADER_LEN + 12] = 64; // LSB of size 64
    inbound[CI_HEADER_LEN + 13] = 0;
    inbound[CI_HEADER_LEN + 14] = 0;
    inbound[CI_HEADER_LEN + 15] = 0;
    eng.feed_sysex(0, &inbound).unwrap();
    assert!(eng.peers().is_empty());
    let out = eng.next_outbound().expect("NAK for malformed Discovery");
    let nak = Nak::decode(&out.body).unwrap();
    assert_eq!(nak.body.status_code, NakCode::Malformed.to_u8());
}
