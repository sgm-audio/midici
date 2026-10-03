# midici

Rust MIDI-CI / Property Exchange responder stack.

**Why MIDI 2.0 Matters Now**: MIDI 1.3 launched 2020; 4 years later, zero open-source control-plane tooling exists. Every DAW, plugin, and synth now speaks MIDI 2.0 natively — the missing piece is the *property exchange layer* that lets software discover, negotiate, and configure capabilities at runtime. midici is the only Rust-based PE responder stack that doesn't drag a C++/Python bridge. Zero-cost abstraction + compile-time capability negotiation.

**Getting Started**:
1. `cargo build --workspace --features pe` (builds the PE-enabled surface)
2. `cargo run --example discover` — prints the property bag from a Discovery exchange
3. See `examples/clap-autoprop/` for the killer demo: `ChCtrlList` auto-mapped from `clap_plugin_params`

**API Reference** (sans-io contract, v1 ships all):

- `CiEngine::new(cfg, rng)` — owns MUID, peers, transactions
- `engine.feed_sysex(group, body)` — feed one complete inbound SysEx7 body (F0/F7 stripped), tagged with UMP group
- `engine.poll(now)` — drive timeouts. Call at >= 10 Hz. `now` is monotonic millis.
- `engine.next_outbound()` — drain outbound SysEx bodies (already chunk-split to negotiated max size)
- `engine.next_event()` — drain application-facing events

**CiEvent** variants:
- `PeerDiscovered { muid, info, caps }`
- `PeerInvalidated { muid }`
- `PropertyGet { peer, request, resource }`
- `PropertySet { peer, request, resource, body }`
- `SubscribeStart { peer, resource, sub }`
- `SubscribeEnd { peer, sub }`
- `Nak { peer, original, code }`

**Core Protocol Coverage** (from ARD-001 §4):

*Management (v1 ships all)*:
- `0x70` Discovery / `0x71` Reply — identity, capability bits (Profiles `0x04`, PE `0x08`, Process Inquiry `0x10`), max SysEx size (≥128), output path ID
- `0x7D` ACK · `0x7E` Invalidate MUID · `0x7F` NAK (with status codes)
- `0x72/0x73` Endpoint (Name/ProductInstanceId) — ship; cheap and hosts query it

*Property Exchange (v1 ships all)*:
- `0x30/0x31` PE Capabilities (negotiates `numSimultaneousRequests`, PE version)
- `0x34/0x35` Get / `0x36/0x37` Set (with `status`: 200/202/341/400/403/404/405/413/445 subset)
- `0x38/0x39` Subscription · `0x3F` Notify
- Encodings: ASCII, `Mcoded7`, `zlib+Mcoded7` (feature-gated)
- Chunking: `requestId` (7-bit) correlation, `numChunks`/`chunkNum` 14-bit fields, header-in-first-chunk rule

*Profiles (v1 = polite refusal)*: `0x20/0x21` Inquiry/Reply reporting zero profiles; everything else NAKs with "not supported." Full profile support is v2.

*Version handling*: advertise CI v1.2; accept v1.1 peers by masking v1.2-only features (ACK usage, Endpoint messages) per the `ci ver` byte. Never NAK on version alone.

**Property Resources** (from ARD-001 §5):
- `DeviceInfo` — manufacturer/family/model/version/serial
- `ResourceList` — auto-generated from the registry
- `ChCtrlList` — entries: `title`, `ctrlType` (`"cc" | "rpn" | "nrpn" | "pnac" | "pnp"`), `ctrlIndex`, `channel`, `minMax`, `default`, plus link metadata. v1 emits per-channel CC/RPN maps; per-note controller types included where the synth supports them.

**clap-autoprop flow (killer demo)**:
1. Main thread walks `clap_plugin_params` → builds `ChCtrlList` entries (param id → assigned controller, range mapped from `clap_param_info`)
2. Controller/host does Discovery → PE Get `ResourceList` → Get `ChCtrlList` → auto-maps knobs. Zero manual MIDI-learn.
3. Param changes flush through `clap_host_params` → subscription `Notify` to subscribed peers.

**Threading & RT Contract** (from ARD-001 §6):

- **Audio thread**: move bytes only. No parsing, no JSON, no allocation, no locks, no logging.
- **CLAP adapter, RT side**: `clap_process` copies inbound SysEx event bytes into a wait-free SPSC ring (`rtrb`, 64 slots × 512 B default, tunable). Pops the outbound ring and emits CLAP MIDI events. Overflow policy: drop-with-counter.
- **Control thread**: host timer (10 ms period) drives `engine.poll(now)`, drains rings, runs all state machines and JSON.
- **Reassembly buffers**: pre-reserved, capped at 64 KiB per (peer, requestId), max 4 concurrent per peer, LRU-evicted on timeout. DoS guard.
- **Verification**: `assert_no_alloc` wraps the RT path in debug; loom/miri pass on the ring wrapper.

**Build Order** (from ARD-001 §9):
1. **Slice 0** — plumbing: virtual ALSA UMP endpoint up; `aseqdump` shows traffic
2. **Slice 1** — `midici-core`: Discovery + ACK/NAK + MUID lifecycle against Workbench
3. **Slice 2** — PE Capabilities + Get for `DeviceInfo`/`ResourceList` (chunker + Mcoded7)
4. **Slice 3** — `clap-autoprop` — `ChCtrlList` from `clap_plugin_params`, CLAP RT bridge. ← announce here; this is the demo video.
5. **Slice 4** — Set + Subscriptions + Notify.
6. **v2** — initiator role, Profiles (DAW Ctrl), `State` resource, CoreMIDI/WMS transports.

**Test Strategy** (from ARD-001 §8):
- **Property tests** (proptest): Mcoded7 round-trip; chunk-split/reassemble round-trip at every negotiated size 128–4096; MUID lifecycle model test.
- **Fuzzing** (cargo-fuzz): SysEx framing, CI header parse, PE JSON header, reassembler. Gate: 10⁸ execs clean before 0.1.
- **Golden transcripts**: captured Discovery/PE exchanges checked into `midici-conformance` (sources: MIDI 2.0 Workbench, Wireshark UMP dissector pcaps from midi2.dev tooling). Replayed byte-exact in CI.
- **RT audit**: `assert_no_alloc` + miri on ring path; 24 h soak of discovery churn (peer appear/vanish loop) with zero growth (heaptrack).
- **Interop matrix** (definition of done for 0.1):
  - MIDI 2.0 Workbench: full Discovery + PE Get suite passes
  - Bitwig on Linux (only Linux DAW with usable UMP today): discovers responder, reads DeviceInfo
  - Windows MIDI Services console tooling: enumerates + Endpoint Name reads
  - Hardware (when available — Keystage-class PE controller): ChCtrlList auto-map demo

**Perf gates**: loopback Discovery < 100 ms; 4 KiB PE Get < 50 ms; RT path ≤ 2 memcpys/event, zero alloc.

---

See `AGENTS.md` for agent build rules and `docs/ARD-001.md` for the full architecture reference.