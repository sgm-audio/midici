# Contributing to midici

## Ground rules

1. **Protocol facts come from the specs.** Every byte layout, constant, status
   code and field must be derivable from the pinned MIDI Association documents
   (see `docs/SPEC_VERSIONS.md`) or `docs/ARD-001.md`. If something is not
   derivable, open an issue describing the gap instead of guessing.
2. **Cite the spec.** Every protocol constant carries a reference comment,
   e.g. `// M2-101 §5.5 Table 6`.
3. **Goldens are append-only.** Files under
   `crates/midici-conformance/goldens/` may only be modified or deleted in a
   commit whose message contains a line
   `HUMAN-APPROVED-GOLDEN-CHANGE: <reason>`, approved by a maintainer.
4. **Never weaken tests to make them pass.** Tests, assertions and lint levels
   are not `#[ignore]`d, loosened or deleted to get CI green. If a test is
   wrong, fix it in its own change with a justification.

## Engineering rules

- The real-time contract is `docs/guide/rt-contract.md` / ARD §6: no
  allocation, locks, syscalls or logging on the RT path. A violation is a
  defect, not a style issue.
- New dependencies follow the policy in ARD §2 and need a justification in the
  PR description.
- No stubs, `todo!()` or placeholder implementations in committed code.
- Conventional Commits (`feat`, `fix`, `chore`, `docs`, `test`, `refactor`);
  PR-sized branches; merge to `main` only with CI green.

## Local checks

These mirror CI (`.github/workflows/ci.yml`). `midici-transport-alsa` and
`examples/virtual-responder` are Linux-only and need `libasound2-dev`
(ALSA ≥ 1.2.13) and `pkg-config`.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps --all-features
cargo deny check
```

On non-Linux hosts, scope to the portable crates:

```sh
cargo test -p midici-core -p midici-pe -p midici-conformance -p midici-responder -p midici-transport-clap
```

Fuzz targets live in `crates/midici-pe/fuzz` and need `cargo +nightly` and
`cargo-fuzz`.
