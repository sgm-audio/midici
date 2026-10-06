# midici

`midici` is a Rust MIDI-CI responder stack. The 0.1.0 workspace contains a sans-I/O management engine, a Property Exchange responder, a Linux ALSA UMP adapter, and a small SPSC ring primitive intended for a future CLAP adapter.

> **Scope:** this is a partial responder implementation, not a complete MIDI-CI/Property Exchange implementation or a claim of hardware interoperability. The current limitations and unverified protocol decisions are listed below and in [`docs/guide/spec-coverage.md`](docs/guide/spec-coverage.md).

## What is implemented

- **`midici-core`** — MIDI-CI wire codecs and a responder state machine for Discovery, peer/MUID lifecycle, Invalidate, ACK/NAK, and version handling. Endpoint Inquiry/Reply codecs exist, but an incoming Endpoint Inquiry currently receives `NotSupported`; Profile Configuration and Process Inquiry are not implemented.
- **`midici-pe`** — PE Capabilities, chunked Get/Set, subscription start/end, outbound subscription updates, resource handlers, JSON headers, and bounded chunk reassembly. The default registry provides `DeviceInfo` and `ResourceList`; applications can register their own `PropertyResource` implementations.
- **`midici-transport-alsa`** — Linux ALSA sequencer UMP endpoint setup, SysEx7 packet conversion, and a 10 ms responder control loop. It needs ALSA development files and an ALSA library providing the UMP sequencer APIs (1.2.13 or newer).
- **`midici-transport-clap`** — a fixed-slot, SPSC ring with two-phase consumption. It does **not** provide CLAP ABI/event bindings, a plugin, or a complete SysEx bridge.
- **Examples** — `virtual-responder` is an ALSA UMP responder daemon. `clap-autoprop` is a control-thread helper/test harness; it is not a loadable CLAP plugin and does not construct a `ChCtrlList` resource from plugin parameters.

The core and PE crates are `no_std` + `alloc` capable. The transport and example crates use `std`.

## Known limitations and open decisions

- **No full protocol coverage.** Profile messages and Process Inquiry are not implemented. Endpoint Inquiry is not positively answered. The `CiEngine`'s `poll` currently has no management timers; the PE responder uses polling for PE reassembly timeouts and peer-state cleanup.
- **PE encodings are not negotiated by the responder.** ASCII/7-bit JSON is the active path. Mcoded7 and zlib codec functions are available separately (`zlib` feature), but the engine does not apply them to Get/Set payloads or negotiate `mutualEncoding`.
- **No built-in `ChCtrlList`.** PE can serve an application-provided resource, but automatic CLAP parameter mapping is not implemented.
- **PE capability state has no configured peer-count bound.** `PeerPe` rows grow for unique PE capability sources; choose an admission/eviction policy before relying on this responder with untrusted peers.
- **CLAP ring is not yet a SysEx transport.** Slots are zero-padded to a fixed size and the ring does not preserve each input's actual length; do not use it for variable-length SysEx without a length/framing API.
- **ALSA inbound SysEx reassembly is not bounded.** An unterminated or excessively large UMP SysEx sequence can grow memory without a maximum length or timeout; do not expose this path to untrusted input until bounded reassembly is added.
- **Queues have overload policies.** The PE output queue now has room for one maximum-sized response at the minimum SysEx size, but aggregate overload evicts the oldest queued output. `CiEngine`, `ResponderEngine` management events, and PE application-event queues are bounded and discard oldest entries on overflow without a drop counter/event. The responder retains at most 32 management events; applications should drain regularly if every event matters.
- **Status 445 needs a human protocol decision.** The ARD maps the simultaneous-request limit to 445, while the checked-in M2-103 table and implementation comments record a conflicting meaning. See [`docs/PROGRESS.md`](docs/PROGRESS.md) and the review report before changing this mapping.
- **Interoperability/performance claims are unverified.** [`docs/INTEROP.md`](docs/INTEROP.md) is an evidence ledger; blank cells mean “not tested,” not success. Checked-in golden messages/transcripts are constructed deterministic fixtures, not packet captures from external tools or hardware. No 24-hour soak or enforced no-allocation gate is present in the repository.

## Build and test

The workspace toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml) (Rust 1.97.1). On Linux, install `pkg-config`, a C toolchain, and ALSA development headers/library with the UMP sequencer APIs (ALSA 1.2.13+).

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo deny check
```

These commands are the local verification entry points; see [CI](.github/workflows/ci.yml) for the configured jobs. Fuzz targets are separate and require nightly Rust plus `cargo-fuzz`; the only checked-in targets are `fuzz_mcoded7` and `fuzz_reassemble` under `crates/midici-pe/fuzz/`.

## Run the ALSA responder

```sh
cargo run -p virtual-responder -- --help
cargo run -p virtual-responder -- --device-name midici --endpoint-name midici-responder
```

The first command prints CLI help. The second opens the ALSA sequencer endpoint and runs until interrupted; it requires a usable `/dev/snd/seq` and appropriate device permissions. The daemon currently logs management `CiEvent`s; it does not expose PE application events through its CLI.

## Repository map

- `crates/midici-core` — management state machine and wire codecs
- `crates/midici-pe` — PE controller, resource registry, chunker/reassembler, codecs
- `crates/midici-responder` — re-export façade
- `crates/midici-transport-alsa` — Linux UMP sequencer adapter
- `crates/midici-transport-clap` — SPSC ring primitive only
- `crates/midici-conformance` — constructed goldens, transcript replay, loopback tests
- `docs/specs/` — checked-in MIDI 2.0 specification PDFs
- `docs/guide/spec-coverage.md` — implementation status by area
- `docs/PROGRESS.md` — append-only engineering and verification record
