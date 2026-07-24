//! One-shot helper: dump a PE Get exchange transcript to stdout (not a CI test).
//! Run: DUMP_PE_GOLDEN=1 cargo test -p midici-conformance dump_pe_get_golden -- --ignored --nocapture

mod common;

use midici_core::{CiConfig, DeviceIdentity, Muid};
use midici_pe::{ResourceRegistry, ResponderEngine};
use rand::rngs::StdRng;
use rand::SeedableRng;

use common::{drain_all, TestInitiator};

fn hex_line(tag: &str, body: &[u8]) -> String {
    let mut s = String::from(tag);
    for b in body {
        s.push_str(&format!(" {b:02X}"));
    }
    s
}

#[test]
#[ignore]
fn dump_pe_get_golden() {
    if std::env::var_os("DUMP_PE_GOLDEN").is_none() {
        return;
    }
    let identity = DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 3,
        model: 4,
        software_revision: [2, 0, 0, 0],
    };
    let mut cfg = CiConfig::responder_default(identity);
    cfg.max_sysex_size = 512;
    let registry = ResourceRegistry::with_device_info(identity);
    let mut responder = ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry);
    let initiator_muid = Muid::ordinary(0x01020304).unwrap();
    let mut init = TestInitiator::new(initiator_muid, 512);

    println!("# CONSTRUCTED exchange — Discovery → PE Caps → Get DeviceInfo");
    println!("# M2-101 §5.5 / §8.5 / §8.7–§8.8; M2-103 DeviceInfo; ENGINE responder");
    println!("ENGINE responder");
    println!("SEED 0xC0FFEE");
    println!("IDENTITY 43 00 00 3 4 02 00 00 00");
    println!("MAX_SYSEX 512");
    println!("CAPS 08");
    println!("FB 7F");
    println!("PATH 00");
    println!("GROUP 0");

    let disc = init.discovery();
    println!("# Peer Discovery (source 0x01020304) to Broadcast");
    println!("{}", hex_line(">", &disc));
    responder.feed_sysex(0, &disc).unwrap();
    for o in drain_all(&mut responder) {
        println!("# Reply to Discovery");
        println!("{}", hex_line("<", &o.body));
    }
    let dest = responder.muid();

    let caps = init.pe_caps(dest);
    println!("# PE Capabilities Inquiry");
    println!("{}", hex_line(">", &caps));
    responder.feed_sysex(0, &caps).unwrap();
    for o in drain_all(&mut responder) {
        println!("# PE Capabilities Reply");
        println!("{}", hex_line("<", &o.body));
    }

    let gets = init.get(dest, "DeviceInfo");
    for (i, g) in gets.iter().enumerate() {
        println!("# PE Get DeviceInfo inquiry chunk {}", i + 1);
        println!("{}", hex_line(">", g));
        responder.feed_sysex(0, g).unwrap();
    }
    for (i, o) in drain_all(&mut responder).into_iter().enumerate() {
        println!("# PE Get DeviceInfo reply chunk {}", i + 1);
        println!("{}", hex_line("<", &o.body));
    }
}
