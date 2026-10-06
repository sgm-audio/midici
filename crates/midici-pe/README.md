# midici-pe

Property Exchange responder code: PE capabilities, chunked Get/Set/subscription handling, resource interfaces, JSON headers, reassembly, and standalone encoding helpers. This is a partial implementation; see [`docs/guide/spec-coverage.md`](../../docs/guide/spec-coverage.md) for exact coverage and open decisions.

## Implemented in source

- **PE Capabilities** — replies with the local PE version and simultaneous-request limit, and stores the peer-reported version plus the minimum simultaneous-request limit. Peer capability rows currently have no configured count bound; see the capacity risk in `docs/guide/spec-coverage.md`. Cross-version behavior is not externally verified.
- **Get / Set** — resource lookup, typed reply headers, per-resource write policy (default read-only), and `PeEvent::PropertySet` after a successful Set.
- **Subscriptions** — start/end lifecycle, responder-allocated IDs, outbound partial/full/notify updates, and peer-state cleanup.
- **Legacy Notify (`0x3F`)** — receive-only; the supported termination path cancels reassembly by request ID.
- **Chunking / reassembly** — 7-bit framing, header only in chunk 1, 128–4096-byte clamp, 64 KiB property cap, four concurrent transactions per peer, bounded fragment metadata, and a 3-second inactivity timeout.
- **Resources** — `DeviceInfo`, a generated `ResourceList`, and custom `PropertyResource` handlers.
- **Management event retention** — `ResponderEngine` buffers at most 32 management events and discards the oldest on overflow. There is no drop counter/event; applications should drain `next_event()` regularly if every event matters.
- **JSON headers** — 7-bit, 4 KiB, and depth-32 checks.
- **Typed status subset** — see the status-445 conflict in `docs/PROGRESS.md`; do not assume the busy mapping is protocol-approved.

## Codec-only functionality

- Mcoded7 encode/decode helpers are available.
- Feature `zlib` enables standalone zlib helpers using `miniz_oxide`.
- The responder does not negotiate `mutualEncoding` or route Get/Set bodies through Mcoded7/zlib. The active path is 7-bit JSON/property bytes.

## Example API

```rust,ignore
use midici_pe::{NotifyBody, ResourceRegistry, ResponderEngine};

let registry = ResourceRegistry::with_device_info(identity);
// registry.register(Box::new(MyResource));
let mut engine = ResponderEngine::new(cfg, rng, registry);
engine.feed_sysex(group, &body)?;
engine.poll(now_ms);
while let Some(out) = engine.next_outbound() { /* frame and send */ }
while let Some(event) = engine.next_event() { /* management events */ }
while let Some(event) = engine.next_pe_event() { /* Set/Subscribe events */ }

// Control-thread update for a registered, subscribed custom resource:
engine.notify_resource_changed("MyResource", NotifyBody::Partial(br#"{"/gain":0.5}"#));
```

`ResourceRegistry::with_device_info` supplies `DeviceInfo` and a `ResourceList`; normal registrations through `register` refresh the generated list. There is no built-in `ChCtrlList`.

## Features and dependencies

- `zlib` — exposes standalone zlib codec helpers; it does not turn on wire-level encoding negotiation.
- `serde` + `serde_json` — JSON headers/resources (control path).
- `miniz_oxide` — optional standalone zlib codec.
- `midici-core` — MIDI-CI headers, MUID, and PE wire types.
- `rand_core` — RNG bound used by the combined responder engine.
