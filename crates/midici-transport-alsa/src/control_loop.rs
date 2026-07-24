//! 10 ms control-plane loop: ALSA UMP ↔ [`ResponderEngine`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use midici_core::{CiConfig, CiEvent, DeviceIdentity};
use midici_pe::{ResourceRegistry, ResponderEngine};
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::error::{Result, TransportError};
use crate::sysex7_ump::{encode_sysex7_packets, Sysex7Reassembler};
use crate::ump_seq::{EndpointConfig, UmpSeqEndpoint};

/// Tick period for `poll(now)`. // Phase 5 DoD
pub const CONTROL_TICK: Duration = Duration::from_millis(10);

/// Options for [`run_responder_loop`].
pub struct LoopOptions {
    pub endpoint: EndpointConfig,
    pub identity: DeviceIdentity,
    pub max_sysex: u32,
    pub seed: u64,
    /// Called for every drained [`CiEvent`] (structured logging hook).
    pub on_event: Box<dyn FnMut(CiEvent) + Send>,
    pub running: Arc<AtomicBool>,
}

/// Open a virtual UMP endpoint and run the control loop until `running` is false.
pub fn run_responder_loop(mut opts: LoopOptions) -> Result<()> {
    let mut cfg = CiConfig::responder_default(opts.identity);
    cfg.max_sysex_size = opts.max_sysex.max(128);
    cfg.local_group = opts.endpoint.group;
    let registry = ResourceRegistry::with_device_info(opts.identity);
    let mut engine = ResponderEngine::new(cfg, StdRng::seed_from_u64(opts.seed), registry);

    let ep = UmpSeqEndpoint::create(&opts.endpoint)?;
    let group = ep.ump_group_nibble();
    eprintln!(
        "midici-transport-alsa: UMP endpoint up at {} (group={group})",
        ep.address_string()
    );

    engine.announce_discovery();
    flush_outbound(&ep, &mut engine, group)?;

    let mut reasm = Sysex7Reassembler::new();
    let start = Instant::now();

    while opts.running.load(Ordering::SeqCst) {
        // Drain all pending UMP events.
        loop {
            match ep.input_ump()? {
                None => break,
                Some(words) => {
                    if let Some(complete) = reasm.feed(&words)? {
                        engine
                            .feed_sysex(complete.group, &complete.body)
                            .map_err(|e| TransportError::Engine(format!("{e:?}")))?;
                        while let Some(ev) = engine.next_event() {
                            (opts.on_event)(ev);
                        }
                        flush_outbound(&ep, &mut engine, group)?;
                    }
                }
            }
        }

        let now_ms = start.elapsed().as_millis() as u64;
        engine.poll(now_ms);
        while let Some(ev) = engine.next_event() {
            (opts.on_event)(ev);
        }
        flush_outbound(&ep, &mut engine, group)?;

        // Short sleep; optionally wait on pollfds with remaining tick budget.
        let fds = ep.poll_fds().unwrap_or_default();
        if !fds.is_empty() {
            let mut fds = fds;
            unsafe {
                libc::poll(
                    fds.as_mut_ptr(),
                    fds.len() as libc::nfds_t,
                    CONTROL_TICK.as_millis() as i32,
                );
            }
        } else {
            thread::sleep(CONTROL_TICK);
        }
    }

    eprintln!("midici-transport-alsa: shutting down {}", ep.address_string());
    Ok(())
}

fn flush_outbound(
    ep: &UmpSeqEndpoint,
    engine: &mut ResponderEngine<StdRng>,
    group: u8,
) -> Result<()> {
    while let Some(out) = engine.next_outbound() {
        let g = if out.group <= 15 { out.group } else { group };
        let packets = encode_sysex7_packets(g, &out.body)?;
        for pkt in packets {
            ep.output_ump(&pkt)?;
        }
    }
    Ok(())
}
