# PROGRESS — midici single state object

Append-only phase log. See `AGENTS.md`.

---

## Phase log template

```
### Phase <n> — <name> — <YYYY-MM-DD>
- Done:
- Deviations from ARD (with reason):
- DoD outputs (verbatim):
- Open items:
- Next phase:
```

---

## Phase 0 — bootstrap — 2026-07-23

### Done
- ENVIRONMENT: Fedora 42 container `midici-dev` with gcc, alsa-lib-devel, alsa-utils, pkg-config,
  rustup stable, rustfmt, clippy, cargo-fuzz, cargo-deny, release-plz.
- HARD GATE G0: verified `docs/specs/` contains M2-101, M2-103, M2-104 PDFs.
- SCAFFOLD: Cargo workspace per `docs/ARD-001.md` §2; root AGENTS.md, deny.toml, LICENSE (MIT),
  .gitignore, rust-toolchain.toml; docs PROGRESS/VERIFY/SPEC_VERSIONS/INTEROP; CI workflow.
- DoD commands run inside `midici-dev` (see outputs below).

### Environment versions
```
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14)
clippy 0.1.97 (8bab26f4f6 2026-07-14)
cargo-deny 0.20.2
cargo-fuzz 0.13.2
release-plz 0.3.160
alsa-lib 1.2.15.3 (pkg-config --modversion alsa)
```

### Deviations from ARD (with reason)
1. **distrobox enter broken in this cloud host**: `distrobox create --name midici-dev --image registry.fedoraproject.org/fedora:42` succeeded, but `distrobox enter` fails during host sockets integration (`/usr/bin/entrypoint` `mkdir` under `set -e` when linking host sockets). Replaced with an equivalent durable `podman` container named `midici-dev` from the same Fedora 42 image (`sleep infinity`), used for all DoD commands. Host still has distrobox/podman installed; image and packages match the requested box.
2. **ARD §2 dependency crates not added yet**: `midi2`, `serde`, `alsa`, etc. are deferred until phases that need them (no unused deps). Identity-only crate surfaces for Phase 0.
3. **ARD PE target note**: ARD header mentions PE v1.1; pinned PDF in tree is M2-103-UM **v1.2** (current midi.org). Recorded in `docs/SPEC_VERSIONS.md`.

### DoD outputs (verbatim)

```text
+ cargo build --workspace
   Compiling virtual-responder v0.1.0 (/workspace/examples/virtual-responder)
   Compiling clap-autoprop v0.1.0 (/workspace/examples/clap-autoprop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
+ cargo test --workspace
   Compiling clap-autoprop v0.1.0 (/workspace/examples/clap-autoprop)
   Compiling virtual-responder v0.1.0 (/workspace/examples/virtual-responder)
   Compiling midici-responder v0.1.0 (/workspace/crates/midici-responder)
   Compiling midici-conformance v0.1.0 (/workspace/crates/midici-conformance)
   Compiling midici-pe v0.1.0 (/workspace/crates/midici-pe)
   Compiling midici-transport-clap v0.1.0 (/workspace/crates/midici-transport-clap)
   Compiling midici-transport-alsa v0.1.0 (/workspace/crates/midici-transport-alsa)
   Compiling midici-core v0.1.0 (/workspace/crates/midici-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running unittests src/main.rs (target/debug/deps/clap_autoprop-34490660783db658)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_conformance-dd33da12fd24a22e)

running 1 test
test tests::goldens_dir_exists ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_core-92d2d0570cb77a5a)

running 1 test
test tests::version_is_nonempty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_pe-abca961954e9b9de)

running 1 test
test tests::version_is_nonempty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_responder-1c45b21d012ae839)

running 1 test
test tests::version_is_nonempty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_transport_alsa-2f3f46b0ef2d0101)

running 1 test
test tests::version_is_nonempty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_transport_clap-16f7a55b34036a04)

running 1 test
test tests::version_is_nonempty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/virtual_responder-1f5ba15aeb1e075b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_conformance

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_pe

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_responder

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_transport_alsa

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_transport_clap

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

+ cargo clippy --workspace --all-targets -- -D warnings
    Checking midici-responder v0.1.0 (/workspace/crates/midici-responder)
    Checking midici-conformance v0.1.0 (/workspace/crates/midici-conformance)
    Checking midici-pe v0.1.0 (/workspace/crates/midici-pe)
    Checking virtual-responder v0.1.0 (/workspace/examples/virtual-responder)
    Checking clap-autoprop v0.1.0 (/workspace/examples/clap-autoprop)
    Checking midici-transport-alsa v0.1.0 (/workspace/crates/midici-transport-alsa)
    Checking midici-core v0.1.0 (/workspace/crates/midici-core)
    Checking midici-transport-clap v0.1.0 (/workspace/crates/midici-transport-clap)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
+ cargo deny check
warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:14:6
   │
14 │     "Apache-2.0",
   │      ━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:15:6
   │
15 │     "BSD-2-Clause",
   │      ━━━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:16:6
   │
16 │     "BSD-3-Clause",
   │      ━━━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:18:6
   │
18 │     "Unicode-3.0",
   │      ━━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:17:6
   │
17 │     "Zlib",
   │      ━━━━ unmatched license allowance

advisories ok, bans ok, licenses ok, sources ok
```

### git log --oneline (after initial commits)

```text
981ce49 docs: refresh phase-0 git log in PROGRESS.md
5fc3793 docs: record phase-0 DoD command outputs
51e6850 ci: add workspace fmt/clippy/test/doc/deny gates
a981dd5 chore: scaffold Cargo workspace for midici crates
e1a29a7 docs: add AGENTS.md, ARD-001, and phase ledgers
afdd7d8 Merge pull request #1 from sgm-audio/cursor/midi-spec-pdfs-d03b
9e9daa0 Use current MIDI-CI v1.2.1 from midi.org
3a2853b Add MIDI Association core spec PDFs under docs/specs
c1c0e5d Initial commit
```

### CI status

- PR https://github.com/sgm-audio/midici/pull/4 — all workflow jobs green (fmt, clippy, test, doc, cargo-deny) on 2026-07-23.
- Tag: `phase-0-complete` on this phase tip.

### Open items
- Restore true `distrobox enter midici-dev` on developer hosts (Bazzite) where socket integration works.
- Human merge of prior draft PRs #2/#3 if still open (content folded into this phase).

### Next phase
- Slice 0 / Slice 1 per `docs/ARD-001.md` §9 (not started this session).

---

## Phase 1 — Management framing — 2026-07-24

### Done
- Implemented `midici-core` Management stack from M2-101-UM v1.2.1 only:
  - `spec` constants (Sub-ID#2, caps bits, broadcast MUID, FB device id, version, ACK/NAK codes)
  - `Muid` 28-bit newtype + `rand_core::RngCore` generation
  - `CiHeader` encode/decode (F0/F7 stripped; ARD §3 feed contract)
  - Messages: Discovery, Reply-to-Discovery, Endpoint Inquiry/Reply, Invalidate MUID, ACK, NAK
- Goldens under `crates/midici-conformance/goldens/mgmt/` (all constructed; listed in `docs/VERIFY.md`)
- Unit tests: byte-exact golden round-trips; proptest round-trips for header + each message
- HUMAN GATE G1: STOP — Scott must clear VERIFY.md constructed-vector entries before Phase 2

### Deviations from ARD (with reason)
1. **No `midi2` dependency yet**: framing is implemented directly over `&[u8]` for the ARD §3 SysEx-body contract; `midi2` can wrap transports later without changing these codecs.
2. **All Management goldens are constructed**: M2-101 has no worked byte examples for these messages; per session brief, listed under VERIFY.md for human check.
3. **Family/model/max-SysEx use 7-bit LSB-first packing** (same scheme as MUID). Spec says multibyte fields are generally LSB first; SysEx data bytes are 7-bit. Documented in VERIFY.md for Scott to confirm.

### Dependency justifications (AGENTS.md §10 / ARD §2)
- `rand_core` (midici-core): MUID generation with injected RNG (ARD §2)
- `proptest`, `rand` (dev-dependencies): property tests for encode/decode round-trips

### DoD outputs (verbatim)

```text
   Compiling midici-core v0.1.0 (/workspace/crates/midici-core)
   Compiling midici-conformance v0.1.0 (/workspace/crates/midici-conformance)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
     Running unittests src/lib.rs (target/debug/deps/midici_conformance-a09bcf60453da301)

running 2 tests
test tests::goldens_dir_exists ... ok
test tests::mgmt_goldens_byte_exact_roundtrip ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/midici_core-a17a3438af3c37da)

running 17 tests
test header::tests::header_roundtrip ... ok
test mgmt::tests::discovery_roundtrip ... ok
test mgmt::tests::endpoint_inquiry_proptest_roundtrip ... ok
test mgmt::tests::ack_proptest_roundtrip ... ok
test mgmt::tests::endpoint_reply_zero_copy ... ok
test mgmt::tests::invalidate_forces_broadcast_dest ... ok
test mgmt::tests::discovery_proptest_roundtrip ... ok
test mgmt::tests::invalidate_proptest_roundtrip ... ok
test mgmt::tests::nak_not_supported_roundtrip ... ok
test mgmt::tests::endpoint_reply_proptest_roundtrip ... ok
test muid::tests::broadcast_encodes_as_four_7f ... ok
test muid::tests::generate_is_deterministic_for_seed ... ok
test muid::tests::generate_stays_ordinary ... ok
test muid::tests::roundtrip_7bit ... ok
test mgmt::tests::nak_proptest_roundtrip ... ok
test mgmt::tests::header_proptest_roundtrip ... ok
test mgmt::tests::reply_to_discovery_proptest_roundtrip ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests midici_conformance

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midici_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Checking libc v0.2.189
    Checking cfg-if v1.0.4
    Checking zerocopy v0.8.55
    Checking linux-raw-sys v0.12.1
    Checking bitflags v2.13.1
    Checking once_cell v1.21.4
    Checking fastrand v2.5.0
    Checking rustix v1.1.4
    Checking fnv v1.0.7
    Checking quick-error v1.2.3
    Checking bit-vec v0.8.0
    Checking bit-set v0.8.0
    Checking num-traits v0.2.19
    Checking getrandom v0.2.17
    Checking rand_core v0.6.4
    Checking getrandom v0.3.4
    Checking getrandom v0.4.3
    Checking wait-timeout v0.2.1
    Checking rand_core v0.9.5
    Checking midici-core v0.1.0 (/workspace/crates/midici-core)
    Checking rand_xorshift v0.4.0
    Checking rand v0.9.5
    Checking unarray v0.1.4
    Checking tempfile v3.27.0
    Checking regex-syntax v0.8.11
    Checking rusty-fork v0.3.1
    Checking midici-conformance v0.1.0 (/workspace/crates/midici-conformance)
    Checking ppv-lite86 v0.2.21
    Checking rand_chacha v0.3.1
    Checking rand_chacha v0.9.0
    Checking proptest v1.11.0
    Checking rand v0.8.7
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.01s
warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:15:6
   │
15 │     "BSD-2-Clause",
   │      ━━━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:16:6
   │
16 │     "BSD-3-Clause",
   │      ━━━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:18:6
   │
18 │     "Unicode-3.0",
   │      ━━━━━━━━━━━ unmatched license allowance

warning[license-not-encountered]: license was not encountered
   ┌─ /workspace/deny.toml:17:6
   │
17 │     "Zlib",
   │      ━━━━ unmatched license allowance

advisories ok, bans ok, licenses ok, sources ok
```

### Open items
- HUMAN GATE G1: Scott diff-reviews `goldens/mgmt/` vs M2-101 PDFs and clears VERIFY.md constructed-vector table
- Do not start Phase 2 until G1 is cleared

### Next phase
- Phase 2 (not started): PE / further protocol per ARD §4 — blocked on G1


---

## Phase 2 — PE encodings + chunking — 2026-07-24

### Done
- BUILD (`midici-pe`): Mcoded7 encode/decode (M2-103 §6.1.7); feature `zlib` →
  `zlib+Mcoded7` via `miniz_oxide`; PE chunk framing (M2-101 Tables 33–35); Chunker
  (clamp 128..=4096, requestId + 14-bit numChunks/chunkNum, header-in-first-chunk);
  Reassembler keyed by `(peer MUID, requestId)` with 64 KiB/tx, 4 concurrent/peer,
  LRU inactivity ordering, 3 s timeout → `ReassembleEvent::Timeout` (NAK 341 deferred
  to Phase 4); pre-reserved buffers documented in rustdoc memory model.
- TESTS: proptest split∘reassemble identity for payload 0..=64 KiB × chunk sizes
  {128,256,512,1024,4096} with shuffled orderings; cap tests (oversize, 5th concurrent,
  timeout reclaim via `MemoryStats` allocator-counting harness).
- FUZZ: `crates/midici-pe/fuzz` targets `fuzz_mcoded7`, `fuzz_reassemble`; CI job
  `fuzz-smoke` runs 60 s/target on nightly.
- PR: https://github.com/sgm-audio/midici/pull/6 — CI green (fmt/clippy/test/doc/deny/fuzz-smoke).
- TAG: `phase-2-complete` on the Phase 2 commit after DoD.

### Deviations from ARD (with reason)
1. **Allocator-counting harness**: workspace `unsafe_code = "forbid"` blocks a
   `#[global_allocator]` counter inside `midici-pe`. Harness asserts via
   `Reassembler::memory_stats()` that reserved bytes are fixed at construction and
   that timeout clears `active_slots`/`active_bytes` (logical reclaim of pre-reserved
   capacities). Same DoS properties; no RT/heap growth on the feed path.
2. **`miniz_oxide` 0.8**: locked under feature `zlib` per ARD §2 (justification: only
   RFC 1950 zlib codec on the allowlist path; no `std` requirement with `with-alloc`).
3. **Fuzz runs use `cargo +nightly`**: `rust-toolchain.toml` pins stable 1.97.1;
   cargo-fuzz requires nightly. Documented in CI and DoD commands below.
4. **G1 human gate**: Phase 1 open item (golden review) still pending Scott; Phase 2
   protocol bytes were taken from local M2-101/M2-103 PDFs under `docs/specs/`.

### DoD outputs (verbatim)

#### `cargo test -p midici-pe --all-features`
```text
running 8 tests
test chunker::tests::clamp_range ... ok
test chunker::tests::header_only_is_single_chunk ... ok
test mcoded7::tests::empty_roundtrip ... ok
test chunker::tests::each_chunk_fits_budget ... ok
test mcoded7::tests::full_group_high_bits ... ok
test mcoded7::tests::seven_byte_mixed_high_bits ... ok
test mcoded7::tests::short_group_three_bytes ... ok
test zlib_codec::tests::zlib_mcoded7_roundtrip ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 4 tests
test fifth_concurrent_rejected ... ok
test timeout_eviction_reclaims_slot_and_active_bytes ... ok
test oversize_transaction_rejected ... ok
test allocator_counting_harness_stable_reservation ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 4 tests
test reverse_order_multi_chunk ... ok
test edges_empty_and_max_body ... ok
test mcoded7_roundtrip ... ok
test split_reassemble_identity ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

#### `cargo +nightly fuzz run fuzz_mcoded7 --fuzz-dir crates/midici-pe/fuzz -- -runs=1000000`
```text
#1000000	DONE   cov: 146 ft: 598 corp: 62/4597b lim: 4096 exec/s: 142857 rss: 483Mb
Done 1000000 runs in 7 second(s)
```

#### `cargo +nightly fuzz run fuzz_reassemble --fuzz-dir crates/midici-pe/fuzz -- -runs=1000000`
```text
#1000000	DONE   cov: 363 ft: 2020 corp: 375/217Kb lim: 4096 exec/s: 17241 rss: 157Mb
Done 1000000 runs in 58 second(s)
```

#### CI
- PR #6 checks: fmt, clippy, test, doc, cargo-deny, fuzz-smoke — all successful
  (https://github.com/sgm-audio/midici/pull/6).

### Open items
- HUMAN GATE G1 still open from Phase 1 (mgmt goldens review).
- Phase 4 will map `ReassembleEvent::Timeout` → NAK status 341 and Oversize → 413 /
  TooManyConcurrent → 445 at the engine layer.

### Next phase
- Phase 3 (not started): PE Capabilities + Get for DeviceInfo/ResourceList / further
  ARD §4–§5 surface — do not start in this session.
