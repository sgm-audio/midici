//! Dual-engine loopback extended: Set round-trips (200/403/405/404) and the
//! subscription lifecycle (subscribe → notify → unsubscribe; vanish → reaped).
//! // ARD §5 / §7; M2-101 §8.9–§8.13; M2-103 §8.2 / §11

mod common;

use midici_conformance::test_resources::XTestResource;
use midici_core::{CiConfig, DeviceIdentity, Muid};
use midici_pe::{NotifyBody, PeEvent, PeQuery, PeStatus, ResourceRegistry, ResponderEngine};
use rand::rngs::StdRng;
use rand::SeedableRng;

use common::{
    drain_all, parse_sub_reply, parse_subscription_update, set_reply_status, TestInitiator,
};

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 3,
        model: 4,
        software_revision: [2, 0, 0, 0],
    }
}

fn make_responder(f: impl FnOnce(&mut ResourceRegistry)) -> ResponderEngine<StdRng> {
    let mut cfg = CiConfig::responder_default(identity());
    cfg.max_sysex_size = 512;
    let mut registry = ResourceRegistry::with_device_info(identity());
    f(&mut registry);
    ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry)
}

fn with_xtest(set_forbidden: bool) -> impl FnOnce(&mut ResourceRegistry) {
    move |reg| {
        reg.register(Box::new(XTestResource::with_set_forbidden(set_forbidden)));
    }
}

/// Discovery + PE Caps handshake; returns (responder muid, active request counter).
fn handshake(responder: &mut ResponderEngine<StdRng>, init: &mut TestInitiator) -> Muid {
    responder
        .feed_sysex(0, &init.discovery())
        .expect("discovery feed");
    assert_eq!(drain_all(responder).len(), 1, "expected Reply to Discovery");
    let dest = responder.muid();
    responder
        .feed_sysex(0, &init.pe_caps(dest))
        .expect("pe caps feed");
    assert_eq!(drain_all(responder).len(), 1, "expected PE Caps Reply");
    dest
}

fn feed_all(responder: &mut ResponderEngine<StdRng>, msgs: Vec<Vec<u8>>) {
    for m in msgs {
        responder.feed_sysex(0, &m).expect("feed");
    }
}

// ---------------------------------------------------------------------------
// Set round-trips
// ---------------------------------------------------------------------------

#[test]
fn set_roundtrip_ok_200() {
    let mut responder = make_responder(with_xtest(false));
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.set(dest, "X-Test", br#"{"value":7}"#));
    let outs = drain_all(&mut responder);
    assert_eq!(outs.len(), 1, "expected Set reply");
    assert_eq!(set_reply_status(&outs), PeStatus::Ok.as_u16());

    // Resource actually changed.
    let got = responder
        .pe()
        .registry
        .get("X-Test", &PeQuery::default())
        .unwrap();
    assert_eq!(got.body, br#"{"value":7}"#);

    // PropertySet event surfaced. // ARD §3
    match responder.next_pe_event() {
        Some(PeEvent::PropertySet { resource, .. }) => assert_eq!(resource, "X-Test"),
        other => panic!("expected PropertySet event, got {other:?}"),
    }
}

#[test]
fn set_forbidden_403() {
    let mut responder = make_responder(with_xtest(true));
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.set(dest, "X-Test", br#"{"value":7}"#));
    let outs = drain_all(&mut responder);
    assert_eq!(set_reply_status(&outs), PeStatus::Forbidden.as_u16());
}

#[test]
fn set_default_read_only_405() {
    // DeviceInfo uses the trait default `set` → NotAllowed. // ARD §5
    let mut responder = make_responder(|_| {});
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.set(dest, "DeviceInfo", br#"{"x":1}"#));
    let outs = drain_all(&mut responder);
    assert_eq!(set_reply_status(&outs), PeStatus::NotAllowed.as_u16());
}

#[test]
fn set_unknown_resource_404() {
    let mut responder = make_responder(|_| {});
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.set(dest, "Nope", br#"{"x":1}"#));
    let outs = drain_all(&mut responder);
    assert_eq!(set_reply_status(&outs), PeStatus::NotFound.as_u16());
}

#[test]
fn set_multi_chunk_roundtrip() {
    let mut responder = make_responder(with_xtest(false));
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 128);
    let dest = handshake(&mut responder, &mut init);

    // 200-byte body forces multi-chunk at max_sysex 128.
    let body = format!("{{\"value\":\"{}\"}}", "A".repeat(200));
    feed_all(&mut responder, init.set(dest, "X-Test", body.as_bytes()));
    let outs = drain_all(&mut responder);
    assert_eq!(set_reply_status(&outs), PeStatus::Ok.as_u16());
    let got = responder
        .pe()
        .registry
        .get("X-Test", &PeQuery::default())
        .unwrap();
    assert_eq!(got.body, body.as_bytes());
}

// ---------------------------------------------------------------------------
// Subscription lifecycle
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_subscribe_notify_unsubscribe() {
    let mut responder = make_responder(with_xtest(false));
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    // 1. Subscribe start.
    feed_all(&mut responder, init.subscribe_start(dest, "X-Test"));
    let outs = drain_all(&mut responder);
    let reply = parse_sub_reply(&outs);
    assert_eq!(reply.status, PeStatus::Ok.as_u16());
    let sub_id = reply.subscribe_id.clone().expect("subscribeId");
    match responder.next_pe_event() {
        Some(PeEvent::SubscribeStart { resource, sub, .. }) => {
            assert_eq!(resource, "X-Test");
            assert_eq!(sub.as_str(), sub_id);
        }
        other => panic!("expected SubscribeStart, got {other:?}"),
    }
    assert_eq!(responder.pe().subscriptions().len(), 1);

    // 2. Notify fan-out (app-triggered, control thread only). // M2-103 §11
    let n = responder.notify_resource_changed("X-Test", NotifyBody::Partial(br#"{"/value":9}"#));
    assert_eq!(n, 1, "one subscribed peer");
    let outs = drain_all(&mut responder);
    assert_eq!(outs.len(), 1, "one Subscription update");
    let (command, update_sub, body) = parse_subscription_update(&outs);
    assert_eq!(command, "partial");
    assert_eq!(update_sub, sub_id);
    assert_eq!(body, br#"{"/value":9}"#);

    // 3. Unsubscribe.
    feed_all(&mut responder, init.subscribe_end(dest, &sub_id));
    let outs = drain_all(&mut responder);
    let reply = parse_sub_reply(&outs);
    assert_eq!(reply.status, PeStatus::Ok.as_u16());
    match responder.next_pe_event() {
        Some(PeEvent::SubscribeEnd { sub, .. }) => assert_eq!(sub.as_str(), sub_id),
        other => panic!("expected SubscribeEnd, got {other:?}"),
    }
    assert!(responder.pe().subscriptions().is_empty());

    // No subscriber left → notify is a no-op.
    let n = responder.notify_resource_changed("X-Test", NotifyBody::Notify);
    assert_eq!(n, 0);
    assert!(drain_all(&mut responder).is_empty());
}

#[test]
fn subscribe_then_peer_vanishes_reaped() {
    let mut responder = make_responder(with_xtest(false));
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.subscribe_start(dest, "X-Test"));
    let outs = drain_all(&mut responder);
    assert_eq!(parse_sub_reply(&outs).status, PeStatus::Ok.as_u16());
    assert_eq!(responder.pe().subscriptions().len(), 1);
    let _ = responder.next_pe_event(); // SubscribeStart

    // Peer sends Invalidate targeting its own MUID → all subscriptions end
    // without End messages. // M2-103 §11.5; ARD §7 "subscription leak"
    let inv = init.invalidate(init.muid);
    responder.feed_sysex(0, &inv).expect("invalidate feed");
    assert_eq!(drain_all(&mut responder).len(), 0);
    responder.poll(1);

    assert!(responder.pe().subscriptions().is_empty(), "reaped");
    match responder.next_pe_event() {
        Some(PeEvent::SubscribeEnd { peer, .. }) => assert_eq!(peer, init.muid),
        other => panic!("expected SubscribeEnd for reaped peer, got {other:?}"),
    }

    let n = responder.notify_resource_changed("X-Test", NotifyBody::Notify);
    assert_eq!(n, 0);
    assert!(drain_all(&mut responder).is_empty());
}

#[test]
fn subscribe_not_subscribable_405() {
    let mut responder = make_responder(|_| {});
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.subscribe_start(dest, "DeviceInfo"));
    let outs = drain_all(&mut responder);
    assert_eq!(parse_sub_reply(&outs).status, PeStatus::NotAllowed.as_u16());
    assert!(responder.pe().subscriptions().is_empty());
}

#[test]
fn subscribe_unknown_resource_404() {
    let mut responder = make_responder(|_| {});
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.subscribe_start(dest, "Nope"));
    let outs = drain_all(&mut responder);
    assert_eq!(parse_sub_reply(&outs).status, PeStatus::NotFound.as_u16());
}

#[test]
fn set_to_subscribed_resource_sends_notify() {
    let mut responder = make_responder(with_xtest(false));
    let mut init = TestInitiator::new(Muid::ordinary(0x01020304).unwrap(), 512);
    let dest = handshake(&mut responder, &mut init);

    feed_all(&mut responder, init.subscribe_start(dest, "X-Test"));
    let outs = drain_all(&mut responder);
    let sub_id = parse_sub_reply(&outs).subscribe_id.unwrap();
    let _ = responder.next_pe_event();

    feed_all(&mut responder, init.set(dest, "X-Test", br#"{"value":7}"#));
    let outs = drain_all(&mut responder);
    assert_eq!(
        outs.len(),
        2,
        "Set reply + Subscription update after Set. // M2-103 §11"
    );
    // First outbound: Set reply 200.
    assert_eq!(set_reply_status(&outs[..1]), PeStatus::Ok.as_u16());
    // Second: Subscription "notify" (responder may not echo Set body).
    let (command, update_sub, body) = parse_subscription_update(&outs[1..]);
    assert_eq!(command, "notify");
    assert_eq!(update_sub, sub_id);
    assert!(body.is_empty(), "notify command carries no body");
}
