# Writing a Transport Adapter

A transport adapter connects `midici_pe::ResponderEngine` (or `CiEngine` directly) to a platform MIDI API. The ALSA adapter is implemented; the CLAP ring is not yet a transport, and CoreMIDI/Windows examples below are future-facing pseudocode.

## Minimal Contract

```rust
use midici_pe::{ResourceRegistry, ResponderEngine};
use midici_core::{CiConfig, DeviceIdentity, CiEvent};
use rand_core::RngCore;

// 1. Create engine
let cfg = CiConfig::responder_default(identity);
let registry = ResourceRegistry::with_device_info(identity);
let mut engine = ResponderEngine::new(cfg, rng, registry);

// 2. Announce presence
engine.announce_discovery();

// 3. Main loop (control thread)
loop {
    // A. Receive inbound MIDI → feed_sysex(group, body)
    //    body = F0/F7 stripped SysEx7 payload
    //    group = UMP group nibble (0-15)
    engine.feed_sysex(group, &body)?;

    // B. Drain outbound
    while let Some(out) = engine.next_outbound() {
        // out.body = F0/F7 stripped SysEx7 payload
        // out.group = UMP group
        transport_send(out.group, &out.body)?;
    }

    // C. Drain events (management + PE)
    while let Some(ev) = engine.next_event() { handle_ci(ev); }
    while let Some(ev) = engine.next_pe_event() { handle_pe(ev); }

    // D. Drive timeouts — subscription reaping against vanished peers also
    //    happens here (M2-103 §11.5 / ARD §7)
    engine.poll(monotonic_now_ms());

    // E. Sleep / wait on platform event
    sleep(CONTROL_TICK);
}
```

## Requirements

### Inbound

- Must deliver **complete SysEx7 messages** (F0...F7) with F0/F7 stripped
- Must tag with correct **UMP group** (0-15)
- Must not fragment a single SysEx across multiple `feed_sysex` calls

### Outbound

- `ResponderEngine` PE replies are chunk-split to the negotiated/clamped SysEx size; `CiEngine` management messages are not PE-chunked.
- The transport must frame each body as SysEx7 UMP packets and preserve the `group` from `OutboundSysex`.
- Do not assume that the CLAP ring can currently carry variable-length bodies: it zero-pads fixed slots and does not record the original length.

### Timing

- Pass monotonic milliseconds since an arbitrary epoch to `poll(now)`.
- The ALSA control loop polls every 10 ms. The PE responder uses polling for its 3-second reassembly inactivity timeout and peer-state cleanup.
- `CiEngine::poll` accepts the timestamp but currently does not store or use it, and does not drive management retry or ACK/NAK timers. Do not rely on those timers being implemented.

### Threading

| If your platform has... | Do this |
|-------------------------|---------|
| Separate audio/control threads | Audio thread: ring push/pop only. Control thread: all `feed_sysex`/`poll`/`next_*` |
| Single thread | Run loop above on that thread |
| Async runtime | Wrap loop in a task; use `tokio::time::interval` for poll |

## Platform Examples

### ALSA UMP (Linux) — `midici-transport-alsa`

```rust
// Opens /dev/snd/seq, creates virtual UMP endpoint
let ep = UmpSeqEndpoint::create(&config)?;

// Non-blocking input
while let Some(words) = ep.input_ump()? {
    if let Some(complete) = reasm.feed(&words)? {
        engine.feed_sysex(complete.group, &complete.body)?;
    }
}

// Output
for pkt in encode_sysex7_packets(group, &body)? {
    ep.output_ump(&pkt)?;
}
```

Key files: `ump_seq.rs`, `sysex7_ump.rs`, `control_loop.rs`

### CLAP — `midici-transport-clap` (not yet an adapter)

The crate currently exposes only `Ring`, `Producer`, and `Consumer`. It has no CLAP event wrappers or host-timer integration. Do not wire variable-length SysEx through it yet: a pushed slice is zero-padded to `B` bytes and its original length is lost.

The architectural sketch in [`rt-contract.md`](rt-contract.md) is pseudocode only. A CLAP bridge needs an explicit bounded length/framing design, event bindings, an audited callback, and tests before it can be called an implemented transport.

Key file today: `crates/midici-transport-clap/src/ring.rs`.

### CoreMIDI (macOS) — future

```rust
// MIDIReceivedBlock callback
MIDIReceivedBlock = { packets in
    for packet in packets {
        if packet.is_sysex7() {
            let body = strip_f0_f7(packet.data);
            engine.feed_sysex(packet.group, &body);
        }
    }
}

// Output via MIDISendSysex
```

### Windows MIDI Services — future

```rust
// IMidiInPort.MessageReceived
// IMidiOutPort.SendBuffer
```

## Testing Your Transport

1. **Loopback test** — pair two endpoints, verify Discovery round-trip
2. **Golden replay** — use `midici-conformance` transcript harness
3. **Interop** — test against MIDI 2.0 Workbench, Bitwig, hardware controllers

```rust
#[test]
fn transport_loopback() {
    let mut engine = make_engine();
    let mut transport = make_transport();

    // Simulate peer Discovery
    transport.inject_inbound(peer_discovery_bytes());
    transport.drain_outbound(); // should get Reply

    // Verify peer discovered
    assert!(matches!(engine.next_event(), Some(CiEvent::PeerDiscovered { .. })));
}
```

## Checklist

- [ ] Opens/creates MIDI endpoint
- [ ] Strips F0/F7 on inbound, adds on outbound
- [ ] Tags inbound with UMP group (0-15)
- [ ] Calls `feed_sysex()` for every complete inbound SysEx
- [ ] Calls `poll(now)` at ≥10 Hz with monotonic ms
- [ ] Drains `next_outbound()` and frames as SysEx7 UMP
- [ ] Drains `next_event()` and forwards to app
- [ ] Clean shutdown (closes endpoint, stops timer)
- [ ] No allocation/locks/logging on audio thread (if applicable)
- [ ] Handles endpoint disconnect gracefully