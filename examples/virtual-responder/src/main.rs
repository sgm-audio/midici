//! Standalone virtual ALSA UMP MIDI-CI responder daemon.
//!
//! Slice 0/1 live wiring (`docs/ARD-001.md` §9). Control loop is a plain thread
//! with a 10 ms tick (`midici_transport_alsa::CONTROL_TICK`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use midici_core::{CiEvent, DeviceIdentity};
use midici_transport_alsa::{run_responder_loop, EndpointConfig, LoopOptions};
use rand::Rng;

#[derive(Parser, Debug)]
#[command(name = "virtual-responder", about = "MIDI-CI responder on a virtual ALSA UMP endpoint")]
struct Cli {
    /// ALSA sequencer client name (`aseqdump -l`).
    #[arg(long, default_value = "midici")]
    device_name: String,

    /// UMP endpoint / port name.
    #[arg(long, default_value = "midici-responder")]
    endpoint_name: String,

    /// UMP group nibble 0..=15.
    #[arg(long, default_value_t = 0)]
    group: u8,

    /// Receivable max SysEx size.
    #[arg(long, default_value_t = 512)]
    max_sysex: u32,

    /// Verbosity: 0=quiet events summary, 1=every CiEvent, 2=debug.
    #[arg(short, long, default_value_t = 1)]
    verbosity: u8,

    /// Fixed RNG seed for MUID (default: OS entropy).
    #[arg(long)]
    seed: Option<u64>,
}

fn main() {
    let cli = Cli::parse();
    if cli.group > 15 {
        eprintln!("error: --group must be 0..=15");
        std::process::exit(2);
    }

    let running = Arc::new(AtomicBool::new(true));
    {
        let r = Arc::clone(&running);
        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .expect("install SIGINT handler");
    }

    let identity = DeviceIdentity {
        manufacturer: [0x7D, 0, 0],
        family: 1,
        model: 1,
        software_revision: [0, 1, 0, 0],
    };
    let seed = cli.seed.unwrap_or_else(|| rand::thread_rng().gen());
    let verbosity = cli.verbosity;

    let opts = LoopOptions {
        endpoint: EndpointConfig {
            client_name: cli.device_name,
            endpoint_name: cli.endpoint_name,
            group: cli.group,
            manufacturer_id: 0x7D,
            family_id: identity.family,
            model_id: identity.model,
            sw_revision: identity.software_revision,
        },
        identity,
        max_sysex: cli.max_sysex,
        seed,
        on_event: Box::new(move |ev| log_event(verbosity, &ev)),
        running,
    };

    eprintln!(
        "virtual-responder {} (midici-responder {}) seed={seed:#x}",
        env!("CARGO_PKG_VERSION"),
        midici_responder::VERSION
    );

    if let Err(e) = run_responder_loop(opts) {
        eprintln!("fatal: {e}");
        std::process::exit(1);
    }
}

fn log_event(verbosity: u8, ev: &CiEvent) {
    if verbosity == 0 {
        return;
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    match ev {
        CiEvent::PeerDiscovered { muid, info, caps } => {
            eprintln!(
                "ts={ts} event=PeerDiscovered muid={muid:?} mfr={:?} family={} model={} caps={:#04x}",
                info.manufacturer, info.family, info.model, caps.bits()
            );
        }
        CiEvent::PeerInvalidated { muid } => {
            eprintln!("ts={ts} event=PeerInvalidated muid={muid:?}");
        }
        CiEvent::Nak {
            peer,
            original,
            code,
        } => {
            eprintln!("ts={ts} event=Nak peer={peer:?} orig_subid2={original:#04x} code={code:?}");
        }
    }
}
