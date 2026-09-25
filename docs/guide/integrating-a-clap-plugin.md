# Integrating a CLAP Plugin

**Goal (ARD §5):** a CLAP plugin exposes `ChCtrlList` over Property Exchange so
a CI-capable controller/host auto-maps its parameters. No MIDI-learn.

> Status note (per AGENTS honesty rules): the **PE machinery is complete**
> (Set, subscriptions, Notify fan-out, tested at the wire level) and
> `examples/clap-autoprop` ships the control-thread entry point
> `flush_param_change`. The CLAP plugin binary itself (entry point vtable,
> `clap_plugin_params`, `ChCtrlList` builder, RT event bridge) is the Phase-6/8
> follow-up. This guide describes the finished wiring as it is designed and
> partially implemented; the walkthrough below marks what exists vs. what lands
> with the plugin binding.

## The Flow

```
Host/Controller          plugin (CLAP + midici)
     │                          │
     ├── Discovery ────────────►│
     │◄─── Reply to Discovery ───┤
     ├── PE Caps Inquiry ──────►│
     │◄─── PE Caps Reply ───────┤
     ├── Get ResourceList ─────►│
     │◄─── Get Reply (names) ───┤
     ├── Get ChCtrlList ───────►│ ← auto-maps knobs
     │◄─── Get Reply (ctrls) ───┤
     ├── Subscription "start" ─►│
     │◄─── Sub Reply (id) ──────┤
     │◄── Subscription "partial"/"notify" ┤ ← param changes, control thread only
```

## What exists today (and is covered by tests)

### 1. The resource layer (`midici-pe`)

Any struct implementing `PropertyResource` can be served over PE. For
`ChCtrlList` you want `subscribable() == true`:

```rust
use midici_pe::{Payload, PeQuery, PeResult, PropertyResource};

struct ChCtrlList { /* built from clap_plugin_params — Phase 6/8 */ }

impl PropertyResource for ChCtrlList {
    fn resource(&self) -> &str { "ChCtrlList" }
    fn get(&self, _: &PeQuery) -> PeResult<Payload> {
        Ok(Payload { body: self.to_json7bit() })  // control thread only
    }
    fn subscribable(&self) -> bool { true }
}
```

Register it: `registry.register(Box::new(chctrllist))`, then construct
`ResponderEngine::new(cfg, rng, registry)` — subscriptions, fan-out, and peer
reaping are all handled by the engine.

### 2. The flush-path wiring (`examples/clap-autoprop`)

This is the function the plugin's param flush path calls — **from the control
thread only** (see [rt-contract.md](rt-contract.md)):

```rust
use midici_pe::ResponderEngine;
// `flush_param_change` — tiny wrapper shipped by the clap-autoprop example:
//   engine.notify_resource_changed(resource, NotifyBody::Partial(body))

// Host flushed params (clap_plugin_params::flush ran on main thread, or the
// host timer callback notices a change):
let n = flush_param_change(&mut engine, "ChCtrlList", br#"{"/gain":0.5}"#);
// n = number of subscribed peers that just got a 0x38 "partial" update
```

`flush_param_change` calls `ResponderEngine::notify_resource_changed` with
`NotifyBody::Partial(body)`. The engine looks up every subscriber of the
resource, assigns a request id, chunks to the negotiated max SysEx per peer,
and queues the wire bytes. All PE/JSON work is in `midici-pe`; the audio
thread only moves ring bytes.

The autoprop crate's unit tests exercise this end to end
(`subscribed_peer_receives_partial_notify_on_flush`), including the no-op case
when nobody subscribed.

### 3. Notify semantics (M2-103 §11)

- `partial` — small changes, body is a partial-Set map `{"​/path": value}`
- `full` — complete replacement body (same as a Get reply payload)
- `notify` — no body; initiator should re-Get (used automatically after a Set
  lands on a subscribed resource)

If the update does not fit one chunk, prefer `notify` (M2-103 §11.1.1).

## Landing with the plugin binding (Phase 6/8)

- `clap-autoprop` `libclap_autoprop.so` + `clap-validator` run
- `clap_plugin_params` walk → `ChCtrlList` entries (`title`, `ctrlType`,
  `ctrlIndex`, `channel`, `minMax`, `default`)
- RT ring bridge (`midici-transport-clap` `Ring`) wired to CLAP MIDI events
- Host timer (`clap_host_timer`, 10 ms) driving `engine.poll` + ring drain

Once the binary lands, the demo (`docs/media/autoprop.cast`, G9a) records:
build → load in a CLAP host → watch Discovery → Get ChCtrlList → Subscribe →
updates on param changes.
