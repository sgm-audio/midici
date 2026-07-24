//! Determinism: same RNG seed + same input schedule ⇒ identical outbound bytes.

use midici_core::spec::SUB_ID2_DISCOVERY;
use midici_core::{mgmt_header, CapFlags, CiConfig, CiEngine, DeviceIdentity, Discovery, Muid};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn run_schedule(seed: u64) -> (Muid, Vec<Vec<u8>>) {
    let identity = DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 1,
        model: 2,
        software_revision: [1, 0, 0, 0],
    };
    let mut eng = CiEngine::new(
        CiConfig::responder_default(identity),
        StdRng::seed_from_u64(seed),
    );
    let our = eng.muid();

    let peers = [
        Muid::ordinary(0x0102_0304).unwrap(),
        Muid::ordinary(0x0A0B_0C0D).unwrap(),
        our, // collision mid-schedule
        Muid::ordinary(0x0011_1314).unwrap(),
    ];

    for (i, &peer) in peers.iter().enumerate() {
        let msg = Discovery {
            header: mgmt_header(SUB_ID2_DISCOVERY, peer, Muid::BROADCAST),
            manufacturer: [0x7D, 0, 0],
            family: 1,
            model: 2,
            software_revision: [1, 0, 0, 0],
            category_supported: CapFlags(0x08).bits(),
            max_sysex_size: 512,
            output_path_id: i as u8,
        };
        let mut buf = vec![0u8; 64];
        let n = msg.encode(&mut buf).unwrap();
        eng.feed_sysex(0, &buf[..n]).unwrap();
        eng.poll((i as u64 + 1) * 10);
    }

    let mut out = Vec::new();
    while let Some(o) = eng.next_outbound() {
        out.push(o.body);
    }
    (eng.muid(), out)
}

#[test]
fn identical_seed_and_schedule_identical_outbound() {
    let a = run_schedule(0xDE70);
    let b = run_schedule(0xDE70);
    assert_eq!(a, b);
}

#[test]
fn different_seed_diverges_after_collision() {
    let a = run_schedule(0xDE70);
    let b = run_schedule(0xDE71);
    // Initial MUID differs with seed, so schedules diverge.
    assert_ne!(a.0, b.0);
}
