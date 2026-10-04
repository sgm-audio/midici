# midici-responder

A small façade crate that re-exports the current PE responder types from `midici-pe`. It is not a separate event loop, transport, or complete MIDI-CI implementation.

## For 0.1.0

The crate re-exports `ResponderEngine`, `ResourceRegistry`, `PropertyResource`, `NotifyBody`, PE events, and related PE types. Applications still use `midici-core` types such as `CiConfig` and `DeviceIdentity` to construct an engine, and a transport is needed to exchange messages.

```rust,ignore
use midici_responder::{ResponderEngine, ResourceRegistry};
use midici_core::{CiConfig, DeviceIdentity};

let cfg = CiConfig::responder_default(identity);
let registry = ResourceRegistry::with_device_info(identity);
let mut engine = ResponderEngine::new(cfg, rng, registry);

loop {
    // Run from a non-real-time control context.
    engine.poll(now_ms);
    while let Some(out) = engine.next_outbound() { /* frame and send */ }
    while let Some(event) = engine.next_event() { /* management events */ }
    while let Some(event) = engine.next_pe_event() { /* PE events */ }
}
```

The façade does not add transport-agnostic ring pairings or logging hooks. Current implementation limits are listed in the workspace [`README`](../../README.md).

## Dependencies

- `midici-core`, `midici-pe`
