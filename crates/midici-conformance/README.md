# midici-conformance

Deterministic constructed golden fixtures, replay/loopback tests, and two PE fuzz targets. These fixtures are not external packet captures; see `docs/VERIFY.md` for human-review status and `docs/INTEROP.md` for external evidence. Goldens are append-only.

## What it does

- **Constructed exchange fixtures** — deterministic replay of Discovery/PE exchanges (`goldens/exchanges/*.transcript`, including `05-pe-set-subscribe`); no external capture provenance is recorded
- **Constructed management vectors** — individual message round-trips (`goldens/mgmt/*.hex`); human review is still pending in `docs/VERIFY.md`
- **Fuzz targets** — `fuzz_mcoded7`, `fuzz_reassemble` (cargo-fuzz, nightly); CI is configured for 60 seconds per target, but this review has no current run result
- **Test initiator shim** — `TestInitiator` (test-only, not published) drives Discovery → PE Caps → Get / Set / Subscribe loopback
- **Set/subscription tests** — status matrix (200/403/404/405), multi-chunk Set, lifecycle (subscribe → notify → unsubscribe; vanish → reaped)
- **`XTestResource`** — canned writable + subscribable `X-Test` resource used by tests and goldens

## Goldens (append-only)

```
goldens/
├── mgmt/
│   ├── 01-discovery.hex … 07-nak.hex
└── exchanges/
    ├── 01-broadcast-discovery.transcript
    ├── 02-directed-discovery.transcript
    ├── 03-collision.transcript
    ├── 04-pe-get-deviceinfo.transcript
    └── 05-pe-set-subscribe.transcript
```

**Rule:** Modifying/deleting a golden requires commit message `HUMAN-APPROVED-GOLDEN-CHANGE: <reason>` authored by Scott.

## Transcript format

```
ENGINE responder|ci
SEED <u64>
IDENTITY <m0> <m1> <m2> <family> <model> <r0> <r1> <r2> <r3>
MAX_SYSEX <u32>
CAPS <hex>
FB <hex>
PATH <hex>
GROUP <u8>       # default UMP group for subsequent inbound/outbound steps
XTEST             # register the canned writable+subscribable X-Test resource
> <hex...>         # inbound on the current group
< <hex...>         # expected outbound on the current group
POLL <now_ms>
```

## Running

```bash
# Unit tests (golden replay, NAK matrix, loopback)
cargo test -p midici-conformance --all-features

# Fuzz (requires nightly)
cargo +nightly fuzz run fuzz_mcoded7 --fuzz-dir crates/midici-pe/fuzz -- -runs=1000000
cargo +nightly fuzz run fuzz_reassemble --fuzz-dir crates/midici-pe/fuzz -- -runs=1000000
```

## Dependencies

- `midici-core`, `midici-pe`
- `serde_json`, `rand`, `rand_core` — harness only