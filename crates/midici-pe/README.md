# midici-pe

Property Exchange (PE) implementation: chunking, Mcoded7, zlib+Mcoded7, resource registry, JSON codecs, and the `ResponderEngine` façade that composes `CiEngine` + `PeController`.

## What it does

- **PE chunking** — splits header + body across SysEx-sized chunks (128..=4096), header-in-first-chunk rule, 14-bit `numChunks`/`chunkNum` (M2-101 §8.3)
- **Get / Set** — typed headers, per-resource write policy (default read-only → 405), `PeEvent::PropertySet`
- **Subscriptions** — `start`/`end` with responder-allocated `SubId`, `partial`/`full`/`notify` updates on `0x38`, subscriptions reaped when the peer vanishes (M2-103 §11)
- **Notify (0x3F)** — receive-only (deprecated in MIDI-CI v1.2); status 144 terminates the named transaction
- **Mcoded7** — 7-bit encoding/decoding (M2-103 §6.1.7); **zlib+Mcoded7** via feature `zlib` (`miniz_oxide`)
- **Reassembler** — incremental, keyed by `(peer MUID, requestId)`, 64 KiB/tx cap, 4 concurrent/peer, 3 s inactivity timeout; pre-reserved buffers (DoS guard)
- **Resource registry** — `PropertyResource` trait (`get`/`set`/`subscribable`) with shipped `DeviceInfo` + `ResourceList`
- **JSON headers** — depth (32) and size (4096) limits; malformed → `PeStatus::BadRequest`, never panics
- **Typed status codes** — `PeStatus` (200/201/341/400/403/404/405/413/445) citing M2-103 §7.4.1 Table 15
- **`ResponderEngine`** — complete responder in one struct: `feed_sysex`, `poll`, `next_outbound`, `next_event`, `next_pe_event`, `notify_resource_changed`

## Who should use this

- `midici-transport-*` adapters (they drive `ResponderEngine` from the control thread)
- Anyone implementing PE on top of a custom transport or device model

## Key types

```rust
use midici_pe::{NotifyBody, PeController, PeQuery, ResourceRegistry, ResponderEngine};

// Registry with DeviceInfo + auto-generated ResourceList.
let registry = ResourceRegistry::with_device_info(identity);
// registry.register(Box::new(MyResource)); // custom resources

// Complete responder (needs a `rand_core::RngCore`):
let mut engine = ResponderEngine::new(cfg, rng, registry);
engine.feed_sysex(group, &body)?;
engine.poll(now_ms);
while let Some(out) = engine.next_outbound() { /* send */ }
while let Some(ev) = engine.next_event() { /* management events */ }
while let Some(ev) = engine.next_pe_event() { /* Set/Subscribe events */ }

// App-originated updates (control thread only):
engine.notify_resource_changed("ChCtrlList", NotifyBody::Partial(br#"{"/gain":0.5}"#));
```

## Feature flags

- `zlib` — enables `zlib+Mcoded7` mutualEncoding (adds `miniz_oxide`)

## Dependencies

- `serde` + `serde_json` — JSON headers/resources (control path only)
- `miniz_oxide` — zlib codec (feature `zlib`)
- `midici-core` — CI headers, MUID, PE message types
- `rand_core` — RNG bound for `ResponderEngine` (shared with `CiEngine`)
