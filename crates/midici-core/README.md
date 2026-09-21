# midici-core

Sans-io MIDI-CI state machines. Zero I/O, deterministic, `no_std` + alloc capable.

## What it does

- **Discovery / Reply-to-Discovery** — identity, capability bits, max SysEx size, output path ID
- **MUID lifecycle** — 28-bit random MUID generation, collision detection, Invalidate MUID
- **ACK / NAK** — typed status codes, ACK flow control
- **Endpoint Inquiry/Reply** — Product Instance ID (inquiry-only)
- **PE wire types** — `PeCapabilities` codec + generic chunked `PeMessage` framing (payload stays opaque; PE logic lives in `midici-pe`)
- **Engine API** — `feed_sysex()`, `poll(now)`, `next_outbound()`, `next_event()`
- **Version handling** — v1.2 advertised, v1.1 peers feature-masked (`allow_ack` / `allow_endpoint`); never NAK on version alone

## Who should use this

- Transport adapter authors (ALSA, CLAP, CoreMIDI, Windows MIDI Services)
- Anyone building a MIDI-CI initiator or responder from scratch
- Test harnesses needing deterministic protocol simulation

## Key types

```rust
use midici_core::{CiConfig, CiEngine, CiEvent, Muid, OutboundSysex};

// Configure
let cfg = CiConfig::responder_default(identity);
let mut engine = CiEngine::new(cfg, rng);

// Feed inbound SysEx (F0/F7 stripped)
engine.feed_sysex(group, body)?;

// Drive timeouts (call at >= 10 Hz)
engine.poll(now_ms);

// Drain outbound SysEx (already chunk-split)
while let Some(out) = engine.next_outbound() {
    transport.send(out.group, &out.body);
}

// Drain application events
while let Some(ev) = engine.next_event() {
    handle(ev);
}
```

## Dependencies

- `rand_core` — MUID generation with injected RNG

(`midi2` is used by the transports, not by core.)

## No dependencies on

- async runtimes
- std (works with `no_std` + alloc)
- logging, locks, or syscalls on hot paths