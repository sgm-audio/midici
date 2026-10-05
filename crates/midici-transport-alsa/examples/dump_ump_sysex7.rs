//! Dump a MIDI-CI message's SysEx7 body as UMP words (one 32-bit word per
//! line, hex) — used to drive real endpoints from `midi.exe` for captured
//! conformance evidence (G5-Windows).
//!
//! Usage: `cargo run -p midici-transport-alsa --example dump_ump_sysex7 -- discovery`

use midici_core::spec::{CAP_PROPERTY_EXCHANGE, SUB_ID2_DISCOVERY};
use midici_core::{mgmt_header, Discovery, Muid};
use midici_transport_alsa::sysex7_ump::encode_sysex7_packets;

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "discovery".into());
    let body: Vec<u8> = match which.as_str() {
        "discovery" => {
            // Our MUID under test; broadcast dest. Identity mirrors the
            // conformance identity so replies can be diffed against goldens.
            let src = Muid::ordinary(0x01020304).unwrap();
            let d = Discovery {
                header: mgmt_header(SUB_ID2_DISCOVERY, src, Muid::BROADCAST),
                manufacturer: [0x7D, 0x00, 0x00],
                family: 1,
                model: 2,
                software_revision: [1, 0, 0, 0],
                category_supported: CAP_PROPERTY_EXCHANGE,
                max_sysex_size: 512,
                output_path_id: 0,
            };
            let mut buf = vec![0u8; 64];
            let n = d.encode(&mut buf).unwrap();
            buf.truncate(n);
            buf
        }
        other => {
            eprintln!("unknown message: {other}");
            std::process::exit(2);
        }
    };
    let packets = encode_sysex7_packets(0, &body).unwrap();
    let mut lines = String::new();
    for p in &packets {
        for w in p {
            lines.push_str(&format!("{w:08X}\n"));
        }
    }
    print!("{lines}");
}
