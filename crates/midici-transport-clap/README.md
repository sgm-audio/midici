# midici-transport-clap

CLAP plugin transport adapter core: the miri-verified, wait-free SPSC byte ring that bridges the audio thread and the control thread.

## What it does (today)

- **`Ring<const N, const B>`** — flat slot ring (default 64 × 512 B); zero alloc, zero lock, zero logging on either path
- **`Producer::push`** — RT-safe; on overflow: drop + relaxed `AtomicUsize` counter (never stalls audio)
- **`Consumer::peek` / `commit`** — two-phase consumption: the slot is published as consumed only by `commit`, so the producer can never overwrite bytes the reader is still using (this was a real race caught by miri)
- Geometry is const-generic and tunable; storage is `[[u8; B]; N]` for stable Rust

Layers landing next (CLAP `sys` event wrappers, host-timer control loop,
`ChCtrlList` built from `clap_plugin_params`, the `clap-autoprop` plugin
binary) are Phase-6/8 follow-ups; the PE side they will drive
(`midici_pe::ResponderEngine` + subscriptions) is already complete — see
`examples/clap-autoprop` for the control-thread `flush_param_change` wiring.

## RT Contract

Audio thread (`clap_process`):
- Copy inbound SysEx event bytes → `ring_in.push(...)`
- `while let Some(slot) = ring_out.peek() { emit_midi_event(slot); ring_out.commit(); }`
- **No parsing, no JSON, no allocation, no locks, no logging**

Control thread (host timer, 10 ms via `clap_host_timer`):
- Drains rings, drives `ResponderEngine::poll(now)`, runs all JSON/PE work, pushes replies into `ring_out`

## Why clap-sys, not clack (Phase 6 decision)

`clap-sys` gives byte-level control of MIDI events, direct timer registration, and an auditable FFI surface. `clack`'s ergonomic layer introduces allocation patterns infeasible to audit exhaustively. ARD §6 is non-negotiable.

## Example

```rust
use midici_transport_clap::{Consumer, Producer, Ring};

let ring: Ring<64, 512> = Ring::new(); // 32 KiB, fixed geometry
// SAFETY: SPSC — exactly one producer thread and one consumer thread.
let (mut prod, mut cons) = unsafe { (Producer::new(&ring), Consumer::new(&ring)) };
prod.push(&sysex_body);
if let Some(slot) = cons.peek() {
    handle_sysex(slot); // slot valid until commit
    cons.commit();
}
```

## Dependencies

None beyond std/core. (No `rtrb`, no `heapless` — the ring is ~200 lines of audited atomic code.)
