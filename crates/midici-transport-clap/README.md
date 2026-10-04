# midici-transport-clap

**Status: ring primitive only — not a CLAP transport or plugin.** This crate contains a fixed-slot SPSC ring intended as one future component of a CLAP audio/control bridge. It has no CLAP ABI/event bindings, host timer, `clap_process` callback, or `ChCtrlList` builder.

## What the ring provides

- `Ring<const N, const B>`, `Producer::push`, and `Consumer::peek` / `commit` / `pop_into`.
- Fixed `N × B` storage; no heap allocation in ring operations.
- One producer and one consumer only; the unsafe constructors require callers to uphold SPSC ownership.
- Full-ring and oversize pushes are dropped and counted.
- Consumer slots remain reserved until `commit`; commit without an uncommitted successful `peek` is ignored.

## Blocking limitation before variable-length SysEx use

A successful `push(data)` copies `data` into a `B`-byte slot and zero-pads the rest. The ring does **not** store `data.len()`, and `peek()` returns the entire `B`-byte slot. MIDI SysEx data can contain zero bytes, so a consumer cannot reliably infer the original length from padding. Treat this crate as a low-level prototype; do not use it as a variable-length SysEx bridge until an explicit length/framing design is added.

## Historical verification

Ring unit tests and Miri tests are present. Earlier runs are recorded in `docs/PROGRESS.md`; the current review added a guard against empty/double consumer commits but could not rerun Cargo or Miri because Rust tooling is unavailable in this environment. A ring-only Miri run does not verify a CLAP plugin RT path.

## Example (fixed-width slot only)

```rust,ignore
use midici_transport_clap::{Consumer, Producer, Ring};

let ring: Ring<64, 512> = Ring::new();
// SAFETY: exactly one producer and one consumer may access this ring.
let (mut producer, mut consumer) = unsafe {
    (Producer::new(&ring), Consumer::new(&ring))
};
producer.push(b"example");
if let Some(slot) = consumer.peek() {
    // `slot.len()` is 512; the original input length is not available.
    consume_fixed_slot(slot);
    consumer.commit();
}
```

The ARD's desired CLAP callback/control-thread design is documented in [`docs/guide/rt-contract.md`](../../docs/guide/rt-contract.md), but it is not implemented here.
