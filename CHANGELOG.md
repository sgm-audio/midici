# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is generated from the conventional-commit history with `git-cliff`
(`cliff.toml`) and then curated per phase.

## [Unreleased]

### Added
- Phase 4: PE Capabilities + Get pipeline with typed `PeStatus` (200/341/400/403/404/405/413/445)
- Phase 4: Resource registry with `DeviceInfo`, `ResourceList`
- Phase 4: JSON header parser with depth/size limits; malformed → 400 never panics
- Phase 4: Dual-engine loopback tests (max SysEx 128 + 4096) + NAK matrix + golden transcript
- Phase 5: ALSA UMP transport (`midici-transport-alsa`) with virtual endpoint
- Phase 5: `virtual-responder` daemon with CLI, structured logging, SIGINT shutdown
- Phase 6: CLAP transport core (`midici-transport-clap`): wait-free SPSC ring (miri-verified)
- Phase 7: PE Set + SetReply with per-resource write policy (default 405 NotAllowed)
- Phase 7: PE Subscriptions — start/end, responder-allocated `SubId`, `partial`/`full`/`notify`
  updates on `0x38`, auto-notify after Set on a subscribed resource (M2-103 §11)
- Phase 7: Subscriptions tied to peer MUID liveness — Invalidate/vanish reaps them
  (M2-103 §11.5 / ARD §7)
- Phase 7: `PeEvent` channel (`PropertySet`/`SubscribeStart`/`SubscribeEnd`) and
  `clap-autoprop` control-thread `flush_param_change` wiring with wire-level tests
- Phase 7: conformance coverage — `XTestResource`, Set status matrix
  (200/403/404/405), lifecycle tests, golden `05-pe-set-subscribe.transcript`
- Phase 9: Documentation — rustdoc on all public items (doc CI denies warnings),
  compiling doctests, per-crate READMEs, guide set under `docs/guide/`, arch diagram,
  spec-coverage table with honest gaps
- Documentation: crate READMEs, architecture guide, integration guide, RT contract, spec coverage

### Fixed
- Phase 6: `forbid(unsafe_code)` → `deny` at workspace level so FFI crates can allow
- Phase 6: `Ack` encode/decode for Message Format Version 1.1 (header-only parity with NAK)
- Phase 6: Discovery rejects `max_sysex_size < 128` per M2-101 §5.5.3
- Phase 6: Peer table full → status 341 (not busy/445)
- Phase 7: `ring.rs` stable layout (`[[u8; B]; N]`); SPSC race — `Consumer::pop` published
  read_idx before returning the slice; fixed via `peek`/`commit` (miri-clean)
- Phase 7: `wrap_around` ring test logic (dropped 96 items, then expected index 0)

## [0.1.0] - 2026-07-23

**First milestone of the midici stack.** Sans-io MIDI-CI Management core
(Discovery/Reply, MUID lifecycle incl. collision handling, ACK/NAK, Endpoint
inquiry, v1.1/v1.2 version masking), PE plumbing foundations (Mcoded7, chunker,
DoS-capped reassembler with fuzz coverage), engine determinism tests, and
constructed golden transcripts pending human review (G1). Responder-only scope
per ARD-001.

### Added
- Initial workspace scaffold per ARD-001
- Cargo workspace with 6 crates + 2 examples
- CI workflow (fmt, clippy, test, doc, cargo-deny)
- `docs/specs/` with M2-101, M2-103, M2-104 PDFs
- Phase 0: Bootstrap environment (Fedora 42 `midici-dev` container)
- Phase 1: Management framing (Discovery, Reply, Endpoint, ACK, NAK, Invalidate MUID)
- Phase 1: Management goldens (constructed, listed in VERIFY.md for human review)
- Phase 2: PE encodings + chunking (Mcoded7, zlib+Mcoded7, chunker, reassembler)
- Phase 2: Fuzz targets (`fuzz_mcoded7`, `fuzz_reassemble`) — 10⁶ execs clean
- Phase 3: `CiEngine` sans-io (feed_sysex, poll, next_outbound, next_event)
- Phase 3: MUID lifecycle, collision, invalidation, v1.1 feature masking
- Phase 3: Golden transcript replay (Discovery, collision)

### Changed
- Phase 3: Endpoint Inquiry Reply NAKs NotSupported (Product Instance ID deferred)

---

**Note:** This changelog is generated from conventional commits via `release-plz`/`git-cliff`. The 0.1.0 summary above is hand-edited; subsequent entries will be auto-generated.