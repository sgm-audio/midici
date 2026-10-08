# internal/ENVIRONMENT.md — environment notes (not public-facing)

Moved out of AGENTS.md during the repo-hygiene pass (Oct 2026). Binding rules
live in AGENTS.md; this file only records *which machines/environments* phases
were executed against, for context when reading internal/PROGRESS.md.

## Canonical working copy
- `C:\Users\scott\midici` on the Windows host (Scott's machine). This is the
  live repo. `C:\Windows\System32\midici` is a stale read-only shadow and the
  old F: copies were archived to OneDrive `midici multi/`.

## Build environments used per phase
- **Phases 0–6**: Fedora 42 distrobox/`midici-dev` container (later replaced by
  an equivalent podman container when distrobox-enter broke on the Cursor Cloud
  host; see Phase 0 deviations).
- **Cursor Cloud VM notes (historical)**: Ubuntu 24.04, no distrobox — ran
  cargo/rustfmt/clippy directly on host; `libasound2-dev` + `pkg-config`
  pre-installed; `cargo fetch` pre-warmed by the startup script.
- **Phase 7+ on this host (no distrobox)**: WSL Ubuntu (`wsl -d Ubuntu`,
  later Ubuntu 26.04), rustup stable 1.97.1 auto-pinned. ALSA: initially
  vendored `libasound2t64` 1.2.15.3 (and 1.2.16.1 resolute debs in CI) to
  `~/alsa` + a `~/bin/pkg-config` shim without root; system-wide
  `libasound2-dev` + `alsa-utils` installed in WSL2 on Oct 2026.
- Output path convention for WSL builds: `CARGO_TARGET_DIR=/home/scott/midici-target`
  (keeps `target/` off the Windows filesystem for performance).

## Platform scoping
- `midici-transport-alsa` is Linux-only (links libasound ≥1.2.13 for the
  `snd_ump_*_set_*` symbols). On Windows, scope cargo commands:
  `cargo test -p midici-core -p midici-pe -p midici-conformance -p midici-responder -p midici-transport-clap`.
- `examples/virtual-responder` and `examples/clap-autoprop` are placeholder
  binaries today (they print name/version).

## Optional extras
- `midici-pe` feature `zlib` (`--all-features`).
- Fuzz targets in `crates/midici-pe/fuzz` need `cargo +nightly` + `cargo-fuzz`.
