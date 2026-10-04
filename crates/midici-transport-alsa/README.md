# midici-transport-alsa

ALSA UMP sequencer transport adapter (Linux). Creates a virtual UMP endpoint, bridges SysEx7 ↔ F0/F7-stripped bodies, runs 10 ms control loop.

## What it does

- **Virtual UMP endpoint** — `snd_seq_set_client_midi_version`, `snd_seq_set_ump_endpoint_info`, `snd_seq_set_ump_block_info`, port `MIDI_UMP` / `UMP_ENDPOINT`
- **SysEx7 bridge** — `Sysex7Reassembler` + `encode_sysex7_packets()` via `midi2` (sysex7 feature)
- **Control loop** — 10 ms tick (`CONTROL_TICK`), non-blocking `input_ump()` / `output_ump()`, drives `ResponderEngine`
- **Live test** — feature `alsa-live` + `#[ignore]` (requires `/dev/snd/seq`)

## Why alsa-sys not alsa

`alsa` 0.12 exposes rawmidi `Ump` open/read/write but **no** sequencer virtual-endpoint APIs. `alsa-sys` 0.6 exposes the full UMP sequencer symbol set. This crate wraps `alsa-sys` in a thin safe module (`ump_seq`). No hand-rolled ioctls.

## Key types

```rust
use midici_transport_alsa::{run_responder_loop, EndpointConfig, LoopOptions};
use midici_core::DeviceIdentity;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

let running = Arc::new(AtomicBool::new(true));
let opts = LoopOptions {
    endpoint: EndpointConfig {
        client_name: "midici".into(),
        endpoint_name: "midici-responder".into(),
        group: 0,
        manufacturer_id: 0x7D,
        family_id: 1,
        model_id: 1,
        sw_revision: [0, 1, 0, 0],
    },
    identity: DeviceIdentity { ... },
    max_sysex: 512,
    seed: 0xC0FFEE,
    on_event: Box::new(|ev| log_event(ev)),
    running,
};

run_responder_loop(opts)?;
```

## Binary

`cargo run -p virtual-responder` — standalone daemon with CLI flags.

## Dependencies

- `alsa-sys` 0.6 — UMP sequencer FFI
- `libc` — pollfd, errno
- `midi2` — SysEx7 UMP encode/decode (feature `sysex7`)
- `midici-core`, `midici-pe` (drives `midici_pe::ResponderEngine`)
- `rand` — RNG for engine construction