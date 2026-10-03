# midici-conformance

Golden transcripts, fuzz targets, interop harness. Append-only goldens.

## What it does

- **Golden transcripts** — byte-exact replay of Discovery/PE exchanges (`goldens/exchanges/*.transcript`, incl. `05-pe-set-subscribe`)
- **Management goldens** — individual message round-trips (`goldens/mgmt/*.hex`)
- **Fuzz targets** — `fuzz_mcoded7`, `fuzz_reassemble` (cargo-fuzz, nightly)
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
GROUP <u8>
XTEST             # register the canned writable+subscribable X-Test resource
> [group] <hex...>   # inbound
< [group] <hex...>   # expected outbound
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