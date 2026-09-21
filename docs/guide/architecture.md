# Architecture

## Layering (sans-io)

```
┌────────────────────────────────────────────────────────┐
│  Application (plugin params, device model, resources)  │
├────────────────────────────────────────────────────────┤
│  midici-responder      poll(now) / feed() / drain()    │
├──────────────────────┬─────────────────────────────────┤
│  midici-core         │  midici-pe                      │
│  Discovery, MUID,    │  Caps/Get/Set/Sub, Chunker      │
│  ACK/NAK, tx mgr     │  Reassembler, Mcoded7, Subs     │
├──────────────────────┴─────────────────────────────────┤
│  midi2 crate (SysEx7 / UMP message types)              │
├────────────────────────────────────────────────────────┤
│  Transport adapters: ALSA UMP · CLAP events · (CoreMIDI│
│  / Windows MIDI Services later)                        │
└────────────────────────────────────────────────────────┘
```

## Core Principles

### Sans-io

Protocol state machines with zero I/O, à la `quinn-proto`. Transports are thin adapters.
- Deterministic: same input → same output given same RNG seed + monotonic time
- Testable: in-memory loopback, golden transcript replay, property tests
- Portable: `no_std` + alloc capable (embedded angle), no async runtime

### Ownership

- `midici-core` owns MUID, peer table, transaction manager, ACK/NAK
- `midici-pe` owns chunker, reassembler, Mcoded7, resource registry, subscriptions
- `midici_pe::ResponderEngine` composes both into a single façade
- Transport adapters own I/O, framing, and platform specifics

## Data Flow

### Inbound

```
Transport (ALSA/CLAP) → feed_sysex(group, body)
         ↓
midici-core: CiHeader decode → route by sub-ID#2
         ↓
Management (0x70-0x7F): CiEngine peer table, MUID, ACK/NAK
         ↓
Property Exchange (0x30-0x39, 0x3F): PeController
         ↓
Reassembler (peer, requestId) → chunks → Complete
         ↓
ResourceRegistry.get(resource, query) → Payload
         ↓
Chunker → outbound chunks → next_outbound()
```

### Outbound

```
next_outbound() → Transport.send(group, body)
         ↓
Transport frames SysEx7 UMP packets (ALSA) or CLAP MIDI events
```

## Engine API (sans-io contract)

```rust
pub struct CiEngine<R: Rng> { ... }

impl<R: Rng> CiEngine<R> {
    pub fn new(cfg: CiConfig, rng: R) -> Self;

    /// Feed one complete inbound SysEx7 body (F0/F7 stripped), tagged with UMP group.
    pub fn feed_sysex(&mut self, group: u8, body: &[u8]) -> Result<(), CiError>;

    /// Drive timeouts. Call at >= 10 Hz. `now` is monotonic millis.
    pub fn poll(&mut self, now: u64);

    /// Drain outbound SysEx bodies (already chunk-split to negotiated max size).
    pub fn next_outbound(&mut self) -> Option<OutboundSysex>;

    /// Drain application-facing events.
    pub fn next_event(&mut self) -> Option<CiEvent>;
}
```

### Events

Two channels, both drained from the control thread:

```rust
// midici-core — management events (CiEngine::next_event)
pub enum CiEvent {
    PeerDiscovered { muid: Muid, info: DeviceIdentity, caps: CapFlags },
    PeerInvalidated { muid: Muid },
    Nak           { peer: Muid, original: u8, code: NakCode },
}

// midici-pe — PE events (PeController::next_pe_event)
pub enum PeEvent {
    PropertySet    { peer: Muid, request_id: u8, resource: String, .. },
    SubscribeStart { peer: Muid, resource: String, sub: SubId, .. },
    SubscribeEnd   { peer: Muid, sub: SubId, resource: String },
}
```

`midici_pe::ResponderEngine` exposes `next_event()` (management) and
`next_pe_event()` (PE); it also reaps subscriptions automatically when a peer
vanishes (`PeerInvalidated`), per M2-103 §11.5.

## Threading Model

| Thread | Responsibility | Allocation | Locks | Syscalls | Logging |
|--------|----------------|------------|-------|----------|---------|
| Audio (RT) | Ring push/pop only | ❌ | ❌ | ❌ | ❌ |
| Control | All state machines, JSON, timers | ✅ | ✅ | ✅ | ✅ |

The engine never allocates on `feed_sysex` hot paths beyond bounded, pre-reserved reassembly buffers. All JSON work happens in `midici-pe` on the control thread.

## Version Handling

- Advertises CI v1.2 (`MESSAGE_FORMAT_VERSION_1_2 = 0x02`)
- Accepts v1.1 peers by masking v1.2-only features (ACK usage, Endpoint messages) per the `ci ver` byte
- Never NAKs on version alone

## Transport Adapter Contract

A transport must:
1. Open/create endpoint, tag inbound SysEx with UMP group
2. Call `feed_sysex(group, body)` for each complete inbound SysEx
3. Run `poll(now_ms)` at ≥10 Hz (monotonic millis)
4. Drain `next_outbound()` and send
5. Drain `next_event()` and forward to application
6. On shutdown, drop endpoint cleanly

See [Writing a Transport](writing-a-transport.md) for details.