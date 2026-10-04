# midici-transport-alsa

Linux ALSA UMP sequencer adapter. It creates a virtual UMP endpoint, converts SysEx7 packets to/from F0/F7-stripped bodies, and drives a 10 ms responder control loop.

> **Environment requirement:** build/link against ALSA library 1.2.13 or newer for the UMP sequencer endpoint APIs used here, plus `pkg-config` and the ALSA development package. The live smoke test also needs a usable `/dev/snd/seq` and device permissions.

## Implemented

- UMP sequencer client/endpoint/block setup through `alsa-sys`.
- SysEx7 UMP packetization and reassembly through `midi2`.
- Non-blocking sequencer input/output and a 10 ms control loop around `ResponderEngine`.
- Optional live smoke test (`alsa-live` feature); it is `#[ignore]` by default and checks local endpoint availability, not peer interoperability.

`LoopOptions::on_event` currently exposes management `CiEvent`s only; PE application events are not forwarded by this loop.

## Known safety limitation

`Sysex7Reassembler` currently appends continuation packet words to a `Vec` until it sees a Complete/End packet. It has no configured maximum message length or inactivity timeout. An unterminated or excessively large SysEx sequence can therefore grow memory without a bound. Do not expose this transport to untrusted MIDI input until a bounded reassembly policy and regression tests are added.

## Why `alsa-sys`

The Rust `alsa` crate version used by the project does not expose the sequencer virtual UMP endpoint APIs required here; `alsa-sys` provides the needed symbols. The unsafe FFI calls are isolated in `ump_seq.rs`; build/test coverage still needs to be run in an environment with the required ALSA library.

## Run the example

```sh
cargo run -p virtual-responder -- --help
cargo run -p virtual-responder -- --device-name midici --endpoint-name midici-responder
```

The first command displays options. The second opens an ALSA virtual endpoint and runs until interrupted.

## Dependencies

- `alsa-sys` 0.6 — UMP sequencer FFI.
- `libc` — `pollfd` and errno handling.
- `midi2` — SysEx7 UMP encode/decode.
- `midici-core`, `midici-pe` — responder engines.
- `rand` — MUID RNG.
