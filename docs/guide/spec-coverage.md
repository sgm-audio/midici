# Implementation status and specification coverage

Static source audit dated 2026-10-04. Status means that code exists in this checkout; it is **not** a current build, conformance, hardware, or interoperability pass. The workspace test suite could not be run in this environment because Cargo is unavailable; see [`PROGRESS.md`](../PROGRESS.md).

## Status legend

- **Implemented in source** — code path exists; current verification status is stated separately.
- **Partial** — codecs or a subset exist, but the responder behavior is incomplete.
- **Not implemented** — no implementation was found in the checked-in source.
- **Not verified** — no external, hardware, or performance evidence is recorded.
- **Human decision** — protocol/design conflict must be resolved before changing behavior.

## MIDI-CI management

| Message / area | Status | Evidence and limits |
|---|---|---|
| Discovery `0x70` / Reply `0x71` | Implemented in source | `midici-core/src/engine.rs` handles Discovery, peer recording, and replies; model and transcript tests exist. |
| MUID collision / Invalidate `0x7E` | Implemented in source | Collision path invalidates, regenerates, and re-announces; incoming invalidation removes a peer or regenerates our MUID. |
| ACK `0x7D` | Partial | Outbound ACK is feature-masked for known v1.2 peers; inbound ACK is decoded and discarded without an application event. This is not a general ACK transaction/flow-control implementation. |
| NAK `0x7F` | Implemented in source | Typed NAK encoding/decoding and an inbound `CiEvent::Nak` path exist. The complete status mapping still needs clause-level review. |
| Endpoint Inquiry `0x72` / Reply `0x73` | Partial | Wire codecs and an outgoing inquiry helper exist. The responder currently answers an incoming inquiry with `NotSupported`; incoming Endpoint Reply is ignored. |
| Profile Configuration `0x20`–`0x29` | Not implemented | Messages fall through to the unsupported-message path; there is no zero-profile inquiry responder. |
| Process Inquiry | Not implemented | No handler found. |
| Capability advertisement | Partial / caller-controlled | `CiConfig::responder_default` advertises PE only. Callers can set other category bits, but the engine does not verify that matching handlers exist. |

## Property Exchange

| Area | Status | Evidence and limits |
|---|---|---|
| PE Capabilities `0x30`/`0x31` | Implemented in source, with a capacity risk | The responder advertises its local PE version/limit and stores peer capability rows, but the `PeerPe` vector has no configured count bound. Unique peer capability inquiries can grow this state without a configured bound. Choose a bounded admission/eviction policy (which may affect active transactions/subscriptions) before changing behavior. No external interoperability evidence is recorded. |
| Get `0x34`/`0x35` | Implemented in source | Resource lookup, reply status/header, and chunking exist. Response payloads are capped at 64 KiB. |
| Set `0x36`/`0x37` | Implemented in source | Custom resource `set` methods are called; the default trait policy is read-only. Successful writes surface a `PeEvent::PropertySet`. |
| Subscription `0x38`/`0x39` | Implemented in source | Start/end lifecycle, subscription IDs, outbound updates, and peer cleanup exist. |
| Legacy Notify `0x3F` | Partial | A receive-only path parses termination status and cancels reassembly by request ID. It is not a general initiator-side Notify transaction implementation. |
| Resource registry | Partial | Built-in `DeviceInfo` and `ResourceList` exist; applications can register custom resources. There is no built-in `ChCtrlList`, `State`, `StateList`, or `ProfileList`. |
| Mcoded7 / zlib | Codec-only | Mcoded7 helpers and optional zlib helpers exist. The responder path does not negotiate `mutualEncoding` or apply these codecs to PE payloads. `zlib_codec::decode` has no explicit decompressed-output limit; do not use it on untrusted compressed input until bounded decoding is added. |
| Chunking / reassembly | Implemented with limits | Chunk splitting clamps SysEx size to 128–4096 bytes; reassembly reserves per-peer/per-transaction storage, caps a property transaction at 64 KiB, allows four concurrent transactions per peer, and times out inactive fragments after 3 seconds. |
| Busy status 445 | **Human decision** | `midici-pe/src/status.rs` follows the ARD's 445 busy mapping, while its own comment records M2-103 Table 15 as assigning 445 a different meaning and 343 to “Too Many Requests.” Do not change this public wire behavior until the spec owner resolves the conflict. See [`PROGRESS.md`](../PROGRESS.md). |

## Resources and integrations

| Item | Status | Evidence and limits |
|---|---|---|
| `DeviceInfo` | Implemented in source | Shipped `PropertyResource` based on `DeviceIdentity`. |
| `ResourceList` | Implemented in source | Shipped with `with_device_info`; registered resources are included when using `register`. Direct edits through `resources_mut` bypass refresh. |
| `ChCtrlList` / automatic plugin mapping | Not implemented | Only a test stand-in in `examples/clap-autoprop`; no CLAP parameter enumeration or production resource builder. |
| ALSA UMP transport | Implemented in source; live status not verified here | Sequencer endpoint setup, UMP/SysEx7 conversion, and responder loop exist. The live test is ignored by default and requires ALSA UMP support/device access. `Sysex7Reassembler` has no message-size or inactivity bound, and the control loop drains input until empty without a per-tick budget or shutdown check inside the drain; untrusted sustained input can grow memory or delay polling/shutdown. |
| CLAP transport/plugin | Partial | `midici-transport-clap` contains an SPSC fixed-slot ring only. There are no CLAP ABI/event bindings or loadable plugin. The ring also does not preserve each pushed byte slice's actual length, so it is not yet a variable-length SysEx bridge. |
| CoreMIDI / Windows MIDI Services | Not implemented | No transport crates were found. |

## Conformance and verification evidence

| Area | Repository evidence | What it does not establish |
|---|---|---|
| Unit/property/model tests | Tests are present across core, PE, transport, and conformance crates. | They were not run in this review environment. No current local pass is claimed. |
| Golden messages/transcripts | Checked-in fixtures are deterministic, constructed examples; replay tests compare output bytes. | They are not captures from MIDI 2.0 Workbench, Wireshark, a DAW, or hardware. The historical human review gates in `docs/VERIFY.md` remain unresolved. |
| Fuzzing | Two targets exist: `fuzz_mcoded7` and `fuzz_reassemble`; CI is configured for a 60-second smoke run per target. | No fuzz execution result is available in this review. The advertised `10^8`-execution gate is not configured. |
| RT checks | SPSC ring source and Miri tests exist; historical Miri results are in `PROGRESS.md`. | No CLAP callback integration exists, and no `assert_no_alloc` gate or 24-hour soak is present. A ring-only Miri result is not proof of a complete plugin RT contract. |
| Interoperability | See [`INTEROP.md`](../INTEROP.md). | All empty cells mean not tested; they are not pass results. |
| Build/lint/docs/advisories | CI workflows define Cargo checks. | Local build/test/fmt/clippy/doc/deny commands could not start because Cargo/Rust tools are absent. Recent GitHub Actions runs are marked failed; the available job API returned no steps and log retrieval failed with `EOF`, so the cause is unknown. An account billing lock is mentioned in earlier progress notes but was not reverified as the cause of these runs. |

## Deferred scope

- MIDI-CI initiator/client role.
- Full Profile Configuration and Process Inquiry.
- Endpoint Information responder behavior.
- PE encoding negotiation/integration beyond 7-bit JSON.
- A complete CLAP plugin and `ChCtrlList` builder.
- CoreMIDI and Windows MIDI Services transports.
- External interoperability, long-duration soak, and performance gates.
