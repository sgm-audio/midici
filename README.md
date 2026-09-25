# midici

[![CI](https://github.com/sgm-audio/midici/actions/workflows/ci.yml/badge.svg)](https://github.com/sgm-audio/midici/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

Rust MIDI-CI / Property Exchange responder stack — the control plane for MIDI 2.0.

## Quickstart (4 commands)

```bash
# 1. Build the virtual responder
cargo build -p virtual-responder --release

# 2. Run it (creates a virtual ALSA UMP endpoint)
cargo run -p virtual-responder --release -- --device-name midici --endpoint-name midici-responder -v 1

# 3. In another terminal, list endpoints
aseqdump -l

# 4. Monitor traffic (replace <client:port> from step 3)
aseqdump -u 2 -p <client:port>
```

The responder implements Discovery, PE Capabilities, Get, Set, Subscribe, Notify
fan-out, ACK/NAK, MUID lifecycle, and Invalidate MUID — MIDI-CI v1.2 Management
plus Property Exchange per M2-101/M2-103.

## The autoprop story

`clap-autoprop` demonstrates why this stack exists: a CLAP plugin whose
parameters **auto-appear as MIDI-CI Property Exchange data** — zero manual
MIDI-learn:

1. Controller/host does Discovery → PE Get `ResourceList` → Get `ChCtrlList`
   (the controller list resource) → auto-maps knobs
2. Controller subscribes to the resource (`0x38` `command: "start"`)
3. When the host flushes a param change, the plugin's **control thread** calls
   `clap_autoprop::flush_param_change(engine, "ChCtrlList", partial_json)`,
   which fans out a `command: "partial"` update to every subscribed peer —
   the audio thread never touches the engine

The CLAP event bridge (RT ring + host-timer glue) and the full plugin binary
land with the Phase-6/8 transport work; the PE subscription plumbing they will
drive is complete and tested at the wire level.

See [`docs/guide/integrating-a-clap-plugin.md`](docs/guide/integrating-a-clap-plugin.md) for the full walkthrough.

## Demo

*Asciinema recording of the autoprop demo — pending (HUMAN GATE G9a). Recording
goes to `docs/media/autoprop.cast`; this link will be added once the file exists.*

## Architecture

```
┌────────────────────────────────────────────────────────┐
│  Application (plugin params, device model, resources)  │
├────────────────────────────────────────────────────────┤
│  midici-responder      poll(now) / feed() / drain()    │
├──────────────────────┬─────────────────────────────────┤
│  midici-core         │  midici-pe                      │
│  Discovery, MUID,    │  Chunker/Reassembler, Mcoded7,  │
│  ACK/NAK, tx mgr     │  Resource registry, Subs        │
├──────────────────────┴─────────────────────────────────┤
│  midi2 crate (SysEx7 / UMP message types)              │
├────────────────────────────────────────────────────────┤
│  Transport adapters: ALSA UMP · CLAP events · (CoreMIDI│
│  / Windows MIDI Services later)                        │
└────────────────────────────────────────────────────────┘
```

**Sans-io core** — zero I/O, deterministic, `no_std` + alloc capable. Transport adapters are thin.

## RT Contract (non-negotiable)

Audio thread does exactly one thing: move bytes.

- **CLAP RT side**: copies inbound SysEx bytes into wait-free SPSC ring, pops outbound ring, emits CLAP MIDI events. **No parsing, no JSON, no allocation, no locks, no logging.**
- **Control thread**: 10 ms timer drives `poll()`, drains rings, runs all state machines + JSON.
- **Reassembly buffers**: pre-reserved, capped at 64 KiB per (peer, requestId), max 4 concurrent per peer, LRU-evicted on 3 s timeout.
- **Verified**: `assert_no_alloc` on RT path in debug; loom/miri pass on ring wrapper.

## Crates

| Crate | Role |
|-------|------|
| `midici-core` | Sans-io CI state machines (Discovery, MUID, ACK/NAK, version masking) + PE wire message types |
| `midici-pe` | PE Capabilities/Get/Set/Subscriptions/Notify, chunker, reassembler, Mcoded7, resource registry, `ResponderEngine` façade |
| `midici-responder` | Reserved for the ergonomic event-pump façade (use `midici_pe::ResponderEngine` today) |
| `midici-transport-alsa` | ALSA UMP sequencer adapter (Linux) |
| `midici-transport-clap` | CLAP adapter core: miri-verified wait-free SPSC ring (`Ring`/`Producer`/`Consumer`) |
| `midici-conformance` | Golden transcripts, fuzz targets, interop harness |

## Spec Coverage

| Area | Status | Spec |
|------|--------|------|
| Discovery / Reply | ✅ full | M2-101 §5.5–5.6 |
| Endpoint Inquiry/Reply | ✅ inquiry only | M2-101 §5.7–5.8 |
| ACK / NAK | ✅ full | M2-101 §5.10–5.11 |
| Invalidate MUID | ✅ full | M2-101 §5.9 |
| PE Capabilities | ✅ full | M2-101 §8.5–8.6 |
| PE Get / GetReply | ✅ full | M2-101 §8.7–8.8 |
| PE Set / SetReply | ✅ full (per-resource write policy) | M2-101 §8.9–8.10 |
| PE Subscribe / updates | ✅ full (0x38/0x39; `partial`/`full`/`notify` fan-out) | M2-101 §8.11–8.12 · M2-103 §11 |
| Notify (0x3F) | ✅ receive-only (deprecated in v1.2; status 144 honored) | M2-101 §8.13 · M2-103 §12 |
| Profiles | 🔄 inquiry-only | M2-101 §6–7 |
| Encodings: ASCII, Mcoded7, zlib+Mcoded7 | ✅ / ✅ / feature `zlib` | M2-103 §6 |

## Documentation

- [Architecture](docs/guide/architecture.md)
- [Integrating a CLAP plugin](docs/guide/integrating-a-clap-plugin.md)
- [Writing a transport](docs/guide/writing-a-transport.md)
- [RT Contract](docs/guide/rt-contract.md)
- [Spec Coverage](docs/guide/spec-coverage.md)

## MSRV

Rust 1.97.1 (pinned in `rust-toolchain.toml`).

## License

MIT. See [`LICENSE`](LICENSE).