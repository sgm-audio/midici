# midici

Rust MIDI-CI / Property Exchange responder stack.

**Why MIDI 2.0 Matters Now**: MIDI 1.3 launched in 2020; four years later, zero open-source control-plane tooling exists. Every DAW, plugin, and hardware synth now speaks MIDI 2.0 natively, but the missing piece is the *property exchange layer* that lets software discover, negotiate, and configure capabilities at runtime. midici fills that gap — the only Rust-based PE responder stack that doesn't drag a C++/Python bridge. Zero-cost abstraction + compile-time capability negotiation.

**Getting Started**:
1. Clone the repo: `git clone https://github.com/sgm-audio/midici.git`
2. Ensure Rust toolchain 1.97.1+: `rustup default stable`
3. Build the core crate: `cargo build -p midici-core`
4. Run the test suite: `cargo test -p midici-core -p midici-pe -p midici-conformance`
5. See `docs/guide/` for integration walkthroughs.

**Public API surface** (see `docs/ARD-001.md` for the full architecture):
- `CiEngine<R: Rng>` — sans-io MIDI-CI state machine (Discovery, MUID lifecycle, ACK/NAK)
- `ResponderEngine` — ergonomic façade composing `CiEngine` + `PeController` (in `midici-pe`, re-exported by `midici-responder`)
- `Ring<N, B>` — wait-free SPSC byte ring for RT-safe CLAP bridging (in `midici-transport-clap`)

See `AGENTS.md` for the binding build rules and `docs/ARD-001.md` for the full architecture.

## License

MIT
