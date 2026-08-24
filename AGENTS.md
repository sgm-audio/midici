# AGENTS.md — midici build rules (binding for all agent sessions)

## Session protocol
1. START: read this file, docs/ARD-001.md, docs/PROGRESS.md. Work ONLY the current phase.
2. END: append to docs/PROGRESS.md: phase, what was done, deviations from ARD (with reason),
   DoD command outputs (verbatim), open items, next phase. This file is the single state object.
3. On any tool/test failure: compact the failure to <10 lines in PROGRESS.md (root cause, not logs).
4. Never start the next phase in the same session.

## Source of truth — anti-fabrication (HIGHEST PRIORITY)
5. Protocol facts come ONLY from docs/specs/* and docs/ARD-001.md. If a required byte layout,
   constant, status code, or field is not derivable from those, STOP the phase and report the
   exact gap in PROGRESS.md. NEVER guess protocol bytes. NEVER fill gaps from training memory.
6. Every spec constant carries a doc reference comment: // M2-101 §<section>.
7. crates/midici-conformance/goldens/ is APPEND-ONLY. Modifying or deleting a golden requires
   the commit message line "HUMAN-APPROVED-GOLDEN-CHANGE: <reason>" authored by Scott.
8. Tests, assertions, and lint levels may never be weakened, #[ignore]d, or deleted to make a
   phase pass. If a test is genuinely wrong, STOP and report.

## Engineering rules
9. RT contract = ARD §6, verbatim. Any allocation, lock, syscall, or logging on the RT path is a
   defect, not a style issue.
10. Dependency policy = ARD §2. Any new dependency requires a justification entry in PROGRESS.md.
11. No stubs, no todo!(), no placeholder implementations in committed code. Scope not yet built
    simply does not exist in the API surface.
12. Conventional Commits (feat/fix/chore/docs/test/refactor). PR-sized branches; merge to main
    only with CI green.
13. All commands run inside distrobox `midici-dev`. Never modify the host OS.
14. rustfmt + clippy -D warnings are gates, not suggestions.

## Definition-of-done protocol
15. A phase's DoD is a list of shell commands. Done = all exit 0 AND outputs pasted in
 PROGRESS.md. Screenshots of green CI are not a substitute for local command output.

## Cursor Cloud specific instructions
- Environment: the Cursor Cloud VM is **Ubuntu 24.04**, not the Fedora `distrobox midici-dev`
  described in rule 13. That distrobox does **not** exist here — run `cargo`/`rustfmt`/`clippy`
  directly on the host. The pinned toolchain (`rust-toolchain.toml` → stable `1.97.1`, with
  `rustfmt`+`clippy`) is managed by `rustup` and auto-installs on the first `cargo` call.
- System deps: `libasound2-dev` + `alsa-utils` + `pkg-config` are pre-installed for the planned
  ALSA transport (ARD §2/§9). No committed crate links ALSA yet, so builds do not require them today.
- Cargo dependencies download from crates.io at build time; the startup update script runs
  `cargo fetch` to pre-warm them. No other refresh step is needed.
- **Known pre-existing breakage (not an env problem):** whole-workspace commands
  (`cargo build/test/clippy --workspace`, `cargo fmt --all -- --check`) currently FAIL, isolated to
  `crates/midici-transport-clap`. `src/ring.rs` uses `[u8; N * B]` (a const-generic expression that
  needs nightly `generic_const_exprs`) and is also unformatted. This is the open item logged under
  "Phase 6 CI debugging" in `docs/PROGRESS.md`; fix it there, do not treat it as a setup failure.
- To build/test the crates that DO compile (the real protocol surface), scope commands, e.g.:
  `cargo test -p midici-core -p midici-pe -p midici-conformance -p midici-responder -p midici-transport-alsa`
  and `cargo clippy` / `cargo run` on the same set. `examples/virtual-responder` and
  `examples/clap-autoprop` are placeholder binaries that just print `name version` today.
- Optional extras: `midici-pe` has a `zlib` feature (`--all-features`); the `crates/midici-pe/fuzz`
  targets need `cargo +nightly` + `cargo-fuzz` (not installed by default).
