//! Live ALSA sequencer smoke (ignored unless `--features alsa-live`).

#![cfg(all(target_os = "linux", feature = "alsa-live"))]

use midici_transport_alsa::{sequencer_available, EndpointConfig, UmpSeqEndpoint};

#[test]
#[ignore = "requires /dev/snd/seq; run: cargo test -p midici-transport-alsa --features alsa-live -- --ignored"]
fn create_virtual_ump_endpoint_live() {
    if !sequencer_available() {
        eprintln!("skip: sequencer not available");
        return;
    }
    let cfg = EndpointConfig {
        client_name: "midici-live-test".into(),
        endpoint_name: "midici-live-ep".into(),
        group: 0,
        ..EndpointConfig::default()
    };
    let ep = UmpSeqEndpoint::create(&cfg).expect("create UMP endpoint");
    assert!(ep.client_id() >= 0);
    assert!(ep.port_id() >= 0);
    eprintln!("live endpoint at {}", ep.address_string());
}
