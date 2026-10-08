# midici

Rust MIDI-CI / Property Exchange responder stack.

**Why MIDI 2.0 Matters Now**: MIDI 1.3 launched in 2020; four years later, zero open-source control-plane tooling exists. Every DAW, plugin, and hardware synth now speaks MIDI 2.0 natively, but the missing piece is the *property exchange layer* that lets software discover, negotiate, and configure capabilities at runtime. midici fills that gap — the only Rust-based PE responder stack that doesn't drag a C++/Python bridge. Zero-cost abstraction + compile-time capability negotiation.

**Getting Started**:
1. Clone the repo: `git clone https://github.com/sgm-audio/midici.git`
2. Ensure Rust toolchain 1.97.1 (pinned in `rust-toolchain.toml`; auto-installed by `rustup` on first `cargo` call)
3. Build the core crate: `cargo build -p midici-core`
4. Run the protocol test suites: `cargo test -p midici-core -p midici-pe -p midici-conformance`
5. Run the virtual responder daemon on a virtual ALSA UMP endpoint: `cargo run -p virtual-responder`
6. See `docs/guide/` for integration walkthroughs.

**API Reference** (layered per `docs/ARD-001.md` §3):
- `midici-core` — sans-io MIDI-CI state machines: `CiEngine` (Discovery, MUID lifecycle, ACK/NAK), `CiConfig`, `CiEvent`, `Muid`
- `midici-pe` — Property Exchange: `ResponderEngine` (combined CI + PE façade), `ResourceRegistry` + the `PropertyResource` trait (`DeviceInfo`, `ResourceList` provided), `SubscriptionTable`, chunker / Mcoded7 / JSON codecs
- `midici-responder` — ergonomic façade re-exporting the PE engine for hosts
- `midici-transport-alsa` / `midici-transport-clap` — transport adapters (ALSA UMP sequencer; CLAP RT-safe byte rings, `Ring<N, B>`)

See `AGENTS.md` for the binding build rules and `docs/ARD-001.md` for the full architecture.

## License

MIT
