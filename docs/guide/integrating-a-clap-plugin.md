# Integrating a CLAP Plugin

**Goal (ARD §5):** a CLAP plugin exposes `ChCtrlList` over Property Exchange so
a CI-capable controller/host auto-maps its parameters. No MIDI-learn.

> **Current status:** PE Get/Set/subscription and update code exists with in-repository tests, but this does not establish full specification conformance. `examples/clap-autoprop` contains a control-thread helper and a test-only `ChCtrlList` stand-in; its binary is not a CLAP plugin. There is no CLAP ABI/event binding, parameter-derived resource, or integrated RT bridge. The walkthrough below separates code that exists from design sketches.

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
`NotifyBody::Partial(body)`. The engine looks up subscribers of the resource,
assigns a request id, chunks to the negotiated/clamped max SysEx per peer, and
queues the wire bytes. This helper and its PE loopback test exist; there is no
CLAP audio callback or host timer invoking them in this repository.

The example's unit tests exercise the control-thread PE path
(`subscribed_peer_receives_partial_notify_on_flush`), including the no-op case
when nobody subscribed. They do not load a plugin or test CLAP host behavior.

### 3. Notify semantics (M2-103 §11)

- `partial` — small changes, body is a partial-Set map `{"​/path": value}`
- `full` — complete replacement body (same as a Get reply payload)
- `notify` — no body; initiator should re-Get (used automatically after a Set
  lands on a subscribed resource)

If the update does not fit one chunk, prefer `notify` (M2-103 §11.1.1).

## Future work — not present in this release

- `clap-autoprop` `libclap_autoprop.so` + `clap-validator` run
- `clap_plugin_params` walk → `ChCtrlList` entries (`title`, `ctrlType`,
  `ctrlIndex`, `channel`, `minMax`, `default`)
- RT ring bridge (`midici-transport-clap` `Ring`) wired to CLAP MIDI events
- Host timer (`clap_host_timer`, 10 ms) driving `engine.poll` + ring drain

The repository currently has no plugin binary, `ChCtrlList` builder, or demo recording. Do not describe this flow as an available demo until those artifacts and host-validation evidence exist.
