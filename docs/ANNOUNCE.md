# ANNOUNCE.md — release record and human-owned drafts

This file contains drafts, not approval to publish a new announcement. The 0.1.0 crates.io/GitHub release was completed on 2026-09-25 (see `docs/PROGRESS.md`); the human conformance/interoperability gates below remain open. The agent does not post announcements or handle registry credentials.

## Release and evidence status

| Item | Status | Evidence / next action |
|---|---|---|
| 0.1.0 crates.io publish | Complete | Phase 10 record in `docs/PROGRESS.md`; all five crates published. |
| 0.1.0 GitHub release | Complete | Phase 10 record; release contains the Linux `virtual-responder` binary and checksum file. |
| Management vector human review (G1) | **Pending** | Compare constructed vectors with the checked-in M2-101 PDF; record sign-off in `docs/VERIFY.md`. |
| Constructed PE vector review | **Pending** | Review `05-pe-set-subscribe.transcript` against M2-101/M2-103 and record sign-off. |
| Live ALSA / external peer evidence (G5) | **Pending** | Run the live smoke test and an external MIDI-CI peer test; record exact environment/results in `docs/PROGRESS.md` and `docs/INTEROP.md`. |
| Fresh CI results | **Unavailable in this review** | Prior release CI was green. Recent Actions runs are marked failed, but their steps/logs were unavailable during review, so the cause is unknown (an account billing lock was recorded earlier). Current local Cargo checks are also unavailable. |
| midi2.dev community submission | **Pending / human-owned** | Confirm submission channel and record a real URL after submission. |

Do not present blank interop cells as passes. See [`INTEROP.md`](INTEROP.md).

## Accurate short description (draft only)

> **midici 0.1.0** is a Rust MIDI-CI responder prototype with a sans-I/O management engine, a partial Property Exchange responder, and a Linux ALSA UMP adapter. It includes deterministic constructed fixtures and in-repository loopback tests. It is not a full MIDI-CI implementation or an externally validated interoperability claim. Profile Configuration, Process Inquiry, positive Endpoint Information responses, PE encoding negotiation, a CLAP plugin, and a built-in `ChCtrlList` are not implemented. The CLAP crate currently contains a fixed-slot SPSC ring whose payload-length limitation must be resolved before variable-length SysEx use.

Review [`README.md`](../README.md) and [`docs/guide/spec-coverage.md`](guide/spec-coverage.md) before reusing this wording. Do not claim hardware/DAW interoperability or a plugin auto-mapping demo without new evidence.

## Human-owned post drafts — not ready to post without revision

### r/rust

**Possible title:** `midici 0.1.0 — a Rust MIDI-CI / Property Exchange responder prototype`

Before posting, add an accurate quickstart and scope note. `cargo run -p virtual-responder -- --help` displays CLI options; running the daemon requires Linux ALSA UMP support (ALSA 1.2.13+) and a usable `/dev/snd/seq`. The checked-in goldens are constructed, not external captures. Do not link to a nonexistent demo recording.

### KVR developers forum

**HOLD:** the CLAP/`ChCtrlList` auto-mapping story is not implemented. Do not claim there is a loadable CLAP plugin, parameter-derived controller list, or tested CLAP host flow. Revisit only after those artifacts and external evidence exist.

### LinkedIn

**HOLD:** no architecture screenshot or demo recording is checked in. Use only the accurate short description above after a human review.

## Rollback guidance for a future release

1. If a published crate is broken, yank the affected version (do not delete it) and publish a fixed version after review. Coordinate dependent crates as needed.
2. Do not move, delete, or recreate the `v0.1.0` tag: it identifies an immutable published crate release. A correction should use a new version and tag (for example, `0.1.1` / `v0.1.1`).
3. Update GitHub release notes to describe the issue and point to the fixed release.
