# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is generated from the conventional-commit history with `git-cliff`
(`cliff.toml`) and then curated per phase.

## [Unreleased]

### Fixed
- Validate 7-bit requirements for fixed MIDI-CI header fields and PE property bytes.
- Normalize a zero configured management peer limit before peer insertion.
- Preserve destination/version handling for PE messages; route reserved CI versions and unknown PE Sub-ID#2 values through the management NAK path.
- Reject PE chunks outside the declared total or reserved fragment bound, and clear active slots on reassembly consistency/cap errors.
- Size the PE outbound queue to hold one maximum-size response at the minimum SysEx size.
- Refresh the generated ResourceList after normal custom-resource registration.
- Ignore consumer-ring commits without an uncommitted successful peek.
- Release ALSA sequencer handles on setup errors and validate endpoint/client names before allocating ALSA resources.

### Tests added
- Regression coverage for 7-bit header/property validation, zero peer limits, out-of-range PE chunks, ResourceList refresh, PE destination/version/unknown-sub-ID routing, large PE replies, ring commit misuse, and ALSA NUL-name validation.

### Documentation
- Rewrote implementation and conformance status claims to distinguish implemented code, design targets, and unverified interoperability.
- Corrected the published 0.1.0 release date and moved the already-released feature set into its release entry.

> The changes in this Unreleased section have not been built or tested in the current review environment: Cargo/Rust tooling is unavailable. See `docs/PROGRESS.md` for exact attempted commands and remaining human decisions.

## [0.1.0] - 2026-09-25

**First published release of the midici stack.** Responder-only scope per ARD-001. The checked-in golden vectors/transcripts are constructed deterministic fixtures, with human review still pending (G1); they are not external packet captures.

### Added
- Initial workspace with six crates and two examples; MIDI 2.0 specification PDFs checked in under `docs/specs/`.
- Sans-I/O MIDI-CI management core for Discovery/Reply, MUID lifecycle/collision handling, Invalidate, ACK/NAK, and version handling.
- Management codecs for Endpoint Inquiry/Reply; incoming Endpoint Inquiry returns `NotSupported` (positive Product Instance ID response deferred).
- PE Capability/Get pipeline, resource registry (`DeviceInfo`, `ResourceList`), typed response status subset, JSON header limits, chunker, Mcoded7 codec, and bounded reassembler.
- PE Set, subscriptions, subscription updates, peer-liveness cleanup, PE events, and the `clap-autoprop` control-thread helper with loopback tests. The helper is not a loadable CLAP plugin and there is no built-in `ChCtrlList`.
- Linux ALSA UMP sequencer adapter and `virtual-responder` daemon.
- Fixed-slot SPSC ring primitive in `midici-transport-clap`; this is not a complete CLAP adapter.
- Constructed management goldens and deterministic exchange transcripts, model/loopback/property tests, and fuzz targets `fuzz_mcoded7` / `fuzz_reassemble`.
- CI workflows for formatting, clippy, workspace tests, docs, cargo-deny, link checking, and 60-second-per-target fuzz smoke.
- Architecture, transport, RT-target, integration, verification, and implementation-status documentation.

### Changed
- `CiEngine::poll` is a no-op for management timers; the PE responder drives PE reassembly timeout and peer cleanup from `ResponderEngine::poll`.
- PE status 445 remains an unresolved human protocol decision: the ARD's busy mapping conflicts with the meaning recorded in M2-103 Table 15. See `docs/PROGRESS.md`.

### Verification recorded for 0.1.0
- Historical workspace, lint, documentation, dependency, fuzz-smoke, and Miri results are recorded in `docs/PROGRESS.md`.
- Live ALSA/external interoperability evidence and human golden review remain pending in `docs/INTEROP.md` and `docs/VERIFY.md`.

---

**Note:** release notes are hand-curated from the project history; they are not a claim that every deferred ARD item shipped.
