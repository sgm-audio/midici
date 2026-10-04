# Real-time contract and current status

The architectural target in `docs/ARD-001.md` §6 is strict: an audio callback moves bounded data only; it does not parse MIDI-CI, allocate, lock, log, perform I/O, or run PE/JSON logic.

> **Implementation status:** there is no loadable CLAP plugin or CLAP callback bridge in this checkout. The pseudocode below describes the intended architecture, not a production callback. `midici-transport-clap` currently provides only a fixed-slot SPSC ring. Its payload-length limitation (below) must be resolved before using it for variable-length SysEx.

## Intended split

| Work | Intended owner | Present in this checkout? |
|---|---|---|
| Copy bounded event bytes between callback and control thread | CLAP audio callback + SPSC rings | Ring primitive only; no CLAP event bindings or callback integration |
| MIDI-CI parsing, peer state, PE reassembly, JSON, resource handlers | Control thread | Sans-I/O engines and PE code exist; not wired to a CLAP host |
| UMP/SysEx7 conversion and event loop | Platform transport | Linux ALSA adapter exists; no CoreMIDI/Windows adapter |
| Parameter-to-`ChCtrlList` mapping | Application/plugin | Not implemented; no built-in `ChCtrlList` resource |

## Intended CLAP callback shape (illustrative pseudocode)

```rust
// Audio callback: move already-bounded byte slices only.
for event in input_events {
    if let Some(sysex) = event.as_sysex() {
        let accepted = producer.push(sysex.body());
        // Record/drop telemetry only through a callback-safe mechanism.
        let _ = accepted;
    }
}

while let Some(slot) = consumer.peek() {
    // Forward only when the ring API provides an explicit payload length.
    // Current Ring does not preserve that length; do not treat this as usable
    // variable-length SysEx code yet.
    emit_event(slot);
    consumer.commit();
}
```

No code in this repository currently implements `clap_process`, CLAP event conversion, host timers, or the control-thread drain described above.

## Current ring contract

`crates/midici-transport-clap/src/ring.rs` provides `Ring<N, B>`, `Producer::push`, and `Consumer::peek`/`commit`/`pop_into`.

- One producer and one consumer per ring; the unsafe constructors require callers to uphold this SPSC discipline.
- Each push writes a fixed `B`-byte slot and zero-pads shorter input. `peek()` returns the full slot; the original byte-slice length is not recorded. SysEx data can contain zero bytes, so consumers cannot safely recover arbitrary input length from padding.
- A successful `peek()` keeps its slot reserved until `commit()`. `commit()` now ignores calls with no uncommitted successful `peek`, avoiding index corruption on accidental extra commits.
- A full ring or oversize push is dropped and counted. The ring has fixed storage and performs no heap allocation on push/peek.
- The intended overflow telemetry must be read/reset off the audio thread.

The missing length metadata is a prerequisite issue for a future CLAP bridge. Fixing it changes the public ring behavior/API and is deliberately left for a design decision rather than guessed in this review.

## PE reassembly is not an audio-thread path

The PE engine is control-thread code, not RT-safe callback work. Its reassembler pre-reserves bounded per-peer buffers (64 KiB property arena, 4 KiB header, up to 2,048 fragment records) and caps active transactions, but message decoding, chunk construction, JSON handling, and response queuing can allocate. Do not call `ResponderEngine::feed_sysex`, `poll`, `next_outbound`, registry methods, or notification fan-out from an audio callback.

## Verification status

- Unit and Miri tests exist for the ring; historical runs are recorded in `docs/PROGRESS.md`.
- This repository review changed the consumer commit guard and added regression tests, but the current environment has no `cargo`, `rustc`, `rustfmt`, or `clippy-driver`, so the changed tests and Miri checks could not be run here.
- There is no `assert_no_alloc` gate, no integrated CLAP callback to audit, and no 24-hour soak test in the current workspace.

The historical latency/allocation goals in ARD §6 are targets, not measured results. See [`spec-coverage.md`](spec-coverage.md) and [`INTEROP.md`](../INTEROP.md) for implementation and external-evidence status.
