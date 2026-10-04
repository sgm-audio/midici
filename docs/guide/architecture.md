# Architecture and implementation status

This describes the code in the current workspace, not the full target architecture. See [`spec-coverage.md`](spec-coverage.md) for protocol gaps and [`rt-contract.md`](rt-contract.md) for the CLAP/real-time status.

## Current layers

```text
Applications / custom PropertyResource handlers
                    │
midici-responder     ├── re-exports PE responder types
                    │
midici-pe::ResponderEngine ── composes CiEngine + PeController
       ┌────────────┴─────────────┐
midici-core                    midici-pe
MIDI-CI codecs,                PE caps/Get/Set/subscriptions,
Discovery/peer state           chunking, reassembly, resources
       └────────────┬─────────────┘
                    │
midici-transport-alsa: ALSA UMP endpoint and control loop
midici-transport-clap: fixed-slot SPSC ring only (no CLAP bindings/plugin)
```

CoreMIDI and Windows MIDI Services transports are not present. The `midi2` dependency is used by the ALSA transport for SysEx7/UMP conversion.

## Ownership and data flow

- `midici-core::CiEngine` owns the local MUID, the bounded management peer table, Discovery/MUID lifecycle, management codecs, and management events.
- `midici-pe::ResponderEngine` composes `CiEngine` with `PeController`; the PE controller owns capability rows, chunked inquiries, resource access, subscriptions, PE events, and PE outbound messages.
- `ResourceRegistry` owns registered `PropertyResource` handlers. The built-in `DeviceInfo` and `ResourceList` are not a `ChCtrlList` implementation.
- The transport frames/reassembles UMP SysEx7 and feeds complete F0/F7-stripped bodies to the engine. In the ALSA adapter this loop exists; the CLAP side is not wired to an engine.

```text
ALSA UMP input → SysEx7 reassembly → ResponderEngine::feed_sysex(group, body)
                                           │
                             CiEngine management routing
                             PeController PE routing/reassembly
                                           │
                                 registry Get/Set handler
                                           │
ALSA UMP output ← SysEx7 packetization ← ResponderEngine::next_outbound()
```

PE messages are addressed only when the destination MUID is ours or broadcast. Unrelated destinations are silently dropped, consistent with the management engine's destination policy.

## Polling and events

- `CiEngine::poll(now)` accepts a timestamp but currently does not store or use it; it does not drive management retry/ACK timers.
- `ResponderEngine::poll(now)` drives PE reassembly inactivity timeouts, moves management events to the responder's pending queue, and reaps PE state for vanished peers.
- `ResponderEngine::next_event()` drains management `CiEvent`s; `next_pe_event()` drains PE `PeEvent`s.
- `next_outbound()` drains management output before PE output. PE replies are chunked to the negotiated/clamped SysEx size; `CiEngine` management messages are not PE-chunked.

The current event enums are smaller than the original architecture proposal:

```rust
// Management events
PeerDiscovered { muid, info, caps }
PeerInvalidated { muid }
Nak { peer, original, code }

// PE events
PropertySet { peer, request_id, resource, res_id, set_partial, body }
SubscribeStart { peer, resource, res_id, sub }
SubscribeEnd { peer, sub, resource }
```

## Allocation and threading

Both engines are control-path code and may allocate while decoding messages, building chunks/replies, and handling resources. PE reassembly buffers are pre-reserved and bounded, but the entire `feed_sysex` path is **not** an allocation-free real-time API.

The intended future audio/control split is: audio callback moves bounded, framed events through an RT-safe transport primitive; control thread runs all parsing, `poll`, resource, and PE work. At present there is no integrated CLAP callback/host timer, and the existing ring does not preserve each push's actual length. See [`rt-contract.md`](rt-contract.md).

## Version and capability behavior

The default `CiConfig` advertises the PE category bit and a 512-byte maximum SysEx size. The engine advertises MIDI-CI version 1.2 and masks selected outbound v1.2-only messages for known v1.1 peers. Callers can set other category bits; the engine does not validate those claims against implemented handlers.

## Transport adapter contract

An adapter should:

1. Receive complete SysEx7 bodies (F0/F7 stripped) and the correct UMP group.
2. Call `ResponderEngine::feed_sysex(group, body)` from a non-RT control context.
3. Call `poll` regularly with monotonic milliseconds.
4. Drain `next_outbound()` and frame each body for the platform transport.
5. Forward both management and PE events if the application needs them.

For the implemented ALSA path, see [`crates/midici-transport-alsa/README.md`](../../crates/midici-transport-alsa/README.md). For future adapter guidance, see [`writing-a-transport.md`](writing-a-transport.md).
