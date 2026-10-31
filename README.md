# midici

A Rust implementation of the MIDI 2.0 **MIDI Capability Inquiry (MIDI-CI)** and
**Property Exchange (PE)** responder stack.

MIDI-CI is the part of MIDI 2.0 that lets two devices discover each other and
negotiate what they support; Property Exchange lets one query and set a
device's properties (device info, resource lists, controller lists) as JSON
over SysEx. midici implements the responder side as sans-io state machines,
so it can sit behind any transport: an ALSA UMP endpoint, a CLAP plugin, or a
test harness.

- Sans-io core: no threads, sockets or allocation on the real-time path
  (see `docs/guide/rt-contract.md`).
- Discovery, MUID lifecycle and collision handling, ACK/NAK, Endpoint Inquiry.
- PE Capabilities, Get, Set, Subscribe and Notify, with chunking, Mcoded7 and
  optional zlib.
- Conformance harness with byte-exact golden transcripts, proptests and fuzz
  targets.
- Message-by-message status: `docs/guide/spec-coverage.md`.

## Getting started

Requires the Rust toolchain pinned in `rust-toolchain.toml` (installed
automatically by `rustup`). The ALSA transport and the `virtual-responder`
example are Linux-only and need `libasound2-dev` (ALSA ≥ 1.2.13) and
`pkg-config`.

```sh
git clone https://github.com/sgm-audio/midici.git
cd midici

# Build and test the portable crates (any OS)
cargo test -p midici-core -p midici-pe -p midici-conformance -p midici-responder -p midici-transport-clap

# Linux: run a MIDI-CI responder on a virtual ALSA UMP endpoint
cargo run -p virtual-responder -- --help
cargo run -p virtual-responder
```

Integration walkthroughs are in `docs/guide/`.

## Crates

Layered per `docs/ARD-001.md` §3:

- `midici-core` — sans-io MIDI-CI state machines: `CiEngine` (Discovery, MUID
  lifecycle, ACK/NAK), `CiConfig`, `CiEvent`, `Muid`.
- `midici-pe` — Property Exchange: `ResponderEngine` (combined CI + PE
  façade), `ResourceRegistry` and the `PropertyResource` trait (`DeviceInfo`,
  `ResourceList` provided), `SubscriptionTable`, chunker / Mcoded7 / JSON
  codecs.
- `midici-responder` — ergonomic façade re-exporting the PE engine for hosts.
- `midici-transport-alsa` / `midici-transport-clap` — transport adapters (ALSA
  UMP sequencer; RT-safe byte rings for CLAP plugins, `Ring<N, B>`).
- `midici-conformance` — golden transcripts and interop checks.

## Status

Pre-1.0. Interop testing against third-party hosts and hardware is tracked in
`docs/INTEROP.md`; the management goldens are constructed from the spec tables
(the spec has no worked byte examples) and their derivation is recorded in
`docs/VERIFY.md`.

## Contributing

See `CONTRIBUTING.md` for the project rules and the local checks that mirror CI.
Architecture: `docs/ARD-001.md`.

## License

MIT — see `LICENSE`.
