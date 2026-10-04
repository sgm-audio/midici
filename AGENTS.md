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
13. Run commands inside the project's designated isolated development environment where available (historically, distrobox `midici-dev`). If a managed checkout does not provide that environment or required tooling, do not install packages or modify the host OS without authorization; record the missing tools and attempted checks in `docs/PROGRESS.md`.
14. rustfmt + clippy -D warnings are gates, not suggestions.

## Definition-of-done protocol
15. A phase's DoD is a list of shell commands. Done = all exit 0 AND outputs pasted in
 PROGRESS.md. Screenshots of green CI are not a substitute for local command output.

## Historical environment and phase notes

The following notes describe earlier Cursor Cloud/WSL runs, not guaranteed tooling in every checkout. `docs/PROGRESS.md` is the current verification record; re-check the actual environment before relying on old results.

- Earlier planned Cursor Cloud environment: Ubuntu 24.04, with the pinned stable 1.97.1 toolchain managed by rustup. The Fedora `midici-dev` distrobox described in rule 13 was unavailable in that environment.
- The workspace now includes `midici-transport-alsa`, which links `alsa-sys` and requires ALSA development files/library with UMP sequencer APIs (the CI image uses ALSA 1.2.13+), plus `pkg-config` and a C toolchain.
- Cargo dependencies are fetched from crates.io during builds; network access may be needed if they are not cached.
- **Phase 7 historical result:** the CLAP ring's flat storage and wrap-around logic were fixed, and its consumer API changed to `peek()`/`commit()` to avoid publishing a slot before reading it. Historical workspace/Miri results are in `docs/PROGRESS.md`; the current ring still has the documented missing payload-length metadata, and this review's new commit-guard test was not run locally.
- A previous Phase 7 WSL run used vendored ALSA packages and a local `pkg-config` shim. Those host-specific paths and flags are not current setup instructions.
- Suggested focused test scope when Rust/ALSA tooling is available: `cargo test -p midici-core -p midici-pe -p midici-conformance -p midici-responder -p midici-transport-alsa`, followed by the corresponding clippy commands.
- `examples/virtual-responder` is an ALSA responder daemon. `examples/clap-autoprop` is a PE helper/test harness, not a loadable CLAP plugin.
- Optional extras: `midici-pe` has a `zlib` feature; the two fuzz targets under `crates/midici-pe/fuzz` need nightly Rust and `cargo-fuzz`.
