# midici-core

Sans-I/O MIDI-CI wire codecs and a management responder state machine. The crate supports `no_std` + `alloc`; it has no transport I/O.

## Current implementation

- **Discovery / Reply to Discovery** — identity, category bits, max SysEx size, and output path fields.
- **MUID lifecycle** — injected RNG, peer records, collision handling, and Invalidate MUID.
- **ACK / NAK** — codecs, outbound ACK helper for known v1.2 peers, inbound NAK events. Inbound ACK is decoded and discarded; a general ACK/flow-control transaction system is not implemented.
- **Endpoint messages** — codecs and an outbound Endpoint Inquiry helper exist, but the responder answers an incoming Endpoint Inquiry with `NotSupported` and ignores incoming Endpoint Reply.
- **PE wire types** — PE Capabilities and generic PE-message framing; PE handling is in `midici-pe`.
- **Engine API** — `feed_sysex`, `poll`, `next_outbound`, and `next_event`.

The default `CiConfig` advertises Property Exchange only. Applications can set other category bits, but `CiEngine` does not validate those bits against implemented handlers. Profile Configuration and Process Inquiry are not implemented.

## Minimal use

```rust,ignore
use midici_core::{CiConfig, CiEngine, DeviceIdentity};

let identity = DeviceIdentity {
    manufacturer: [0x7D, 0, 0],
    family: 1,
    model: 2,
    software_revision: [1, 0, 0, 0],
};
let cfg = CiConfig::responder_default(identity);
let mut engine = CiEngine::new(cfg, rng);

// Feed a complete SysEx7 body with F0/F7 stripped.
engine.feed_sysex(group, body)?;
engine.poll(now_ms);
while let Some(out) = engine.next_outbound() {
    transport.send(out.group, &out.body);
}
while let Some(event) = engine.next_event() {
    handle(event);
}
```

`CiEngine::poll` accepts a timestamp but currently does not store or use it, and has no management timeout/retry behavior. PE reassembly timers are driven by `midici_pe::ResponderEngine::poll`. Management messages are not PE-chunked; PE response chunking is implemented in `midici-pe`.

## Dependencies

- `rand_core` — deterministic MUID generation with an injected RNG.
- No transport, async-runtime, logging, or OS-I/O dependency.
