# midici-responder

Ergonomic MIDI-CI responder façade: the application-facing entry point.

## For 0.1.0

This crate re-exports the complete responder engine and its resource API
(`ResponderEngine`, `ResourceRegistry`, `PropertyResource`, `NotifyBody`,
`PeEvent`, …) from `midici-pe`, so applications depend on one crate.

```rust
use midici_responder::{ResponderEngine, ResourceRegistry};
use midici_core::{CiConfig, DeviceIdentity};

let cfg = CiConfig::responder_default(identity);
let registry = ResourceRegistry::with_device_info(identity);
let mut engine = ResponderEngine::new(cfg, rng, registry);

loop {
    // control thread at ~10 ms
    engine.poll(now_ms);
    while let Some(out) = engine.next_outbound() { /* send */ }
    while let Some(ev) = engine.next_event() { /* management */ }
    while let Some(ev) = engine.next_pe_event() { /* PE set/subscribe */ }
}
```

Transport-agnostic event-pump helpers (ring pairings, logging hooks) land next.

## Dependencies

- `midici-core`, `midici-pe`
