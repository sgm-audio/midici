# ANNOUNCE.md — 0.1.0 release checklist + drafts

Everything here is a **draft for Scott** (HUMAN GATE G10). The agent never holds
or uses a crates.io token; `CARGO_REGISTRY_TOKEN` lives in repo secrets and the
`release` workflow's `publish-crates` job runs only on a final `vX.Y.Z` tag.

## Release checklist

### Pre-flight
- [ ] G1: mgmt goldens reviewed against M2-101 PDFs (VERIFY.md)
- [ ] VERIFY.md: phase-7 constructed golden `05-pe-set-subscribe` reviewed
- [ ] G5: live ALSA evidence pasted into PROGRESS.md (`aseqdump` + Workbench)
- [ ] CI green on main (fmt / clippy / test / doc / deny / fuzz-smoke / link-check)
- [ ] `git-cliff` CHANGELOG.md refreshed for the v0.1.0 section
- [ ] `cargo publish --dry-run` outputs (Phase 10, PROGRESS.md): leaf crates
      clean; midici-pe/responder/transport-alsa verified by release-plz in
      dependency order once midici-core 0.1.0 hits the index

### Ship it (Scott)
- [ ] `git tag v0.1.0-rc1 && git push origin v0.1.0-rc1` → check release
      workflow (draft release + SHA256SUMS attached)
- [ ] Confirm glibc compat of `virtual-responder` binary (built on GH
      ubuntu-latest; static-musl build can be added later if needed)
- [ ] `git tag v0.1.0 && git push origin v0.1.0` → release-plz publishes in
      order: `midici-core` → `midici-pe` → `midici-responder` →
      `midici-transport-alsa` → `midici-transport-clap`
- [ ] Verify on crates.io + docs.rs (links below)
- [ ] Publish the draft GitHub release

### crates.io / docs.rs live links (check after publish)
- <https://crates.io/crates/midici-core> · <https://docs.rs/midici-core>
- <https://crates.io/crates/midici-pe> · <https://docs.rs/midici-pe>
- <https://crates.io/crates/midici-responder> · <https://docs.rs/midici-responder>
- <https://crates.io/crates/midici-transport-alsa> · <https://docs.rs/midici-transport-alsa>
- <https://crates.io/crates/midici-transport-clap> · <https://docs.rs/midici-transport-clap>

### midi2.dev community submission
- [ ] Submit the project to the midi2.dev community/links page (repo URL,
      one-paragraph summary). Note: submission channel/format is on Scott —
      paste the actual submission URL here once filed.

## Post drafts (Scott posts; agent does not)

> Shared blurb (use everywhere): **midici** is a MIT-licensed Rust stack for the
> MIDI 2.0 control plane — a sans-io MIDI-CI v1.2 responder with Property
> Exchange (Get/Set/subscriptions with notify fan-out), DoS-capped chunk
> reassembly, golden-transcript conformance, and a miri-verified RT ring for
> CLAP plugins. Responder role for 0.1.0; initiator + profiles come next.

### r/rust
**Title:** `midici 0.1.0 — MIDI-CI / Property Exchange responder stack for MIDI
2.0 in Rust (sans-io, no_std-friendly)`

Body: blurb + quickstart (`cargo run -p virtual-responder`) + what it does
(discovery, PE get/set/sub with automatic notify, per-peer concurrency caps) +
what it doesn't do yet (initiator role, full Profiles, State resources) +
link to repo and the autoprop demo cast (`docs/media/autoprop.cast`, G9a).

### KVR developers forum (DSP and Plugin Development)
Blurb + the autoprop story: a CLAP plugin whose parameters show up on a
MIDI-CI controller without mapping work; params flush → 0x38 partial updates
only from the control thread; the RT thread does two ring memcpys per event.

### LinkedIn
Shorter: blurb + architecture diagram screenshot + repo + cast link.

## Rollback procedure

If a published 0.1.0 crate is broken:

1. **Yank, don't delete**: `cargo yank --vers 0.1.0 midici-<crate>` for each
   affected crate (dependents stay buildable via lockfiles; new resolves stop).
   Crates with reverse deps must be yanked in reverse order is *not* required —
   yank is per-crate.
2. Revert the release commit: `git revert <rel>` on a `fix/` branch.
3. `git tag -d v0.1.0 && git push origin :v0.1.0` if the tag itself must move;
   then publish the fixed version as `v0.1.1` (never re-tag a published
   crates.io version — the registry is immutable).
4. Mark the GitHub release as pre-release/broken in the notes; add the link to
   the fix issue.
