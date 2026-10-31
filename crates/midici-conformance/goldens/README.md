# Goldens

The Phase 1 management goldens (`mgmt/`) are constructed vectors: the spec has
no worked byte examples for these messages. See `docs/VERIFY.md` for how each
one was derived and its review status.

Append-only. Modifying or deleting a golden requires a commit message line:

`HUMAN-APPROVED-GOLDEN-CHANGE: <reason>`

approved by a maintainer (see `CONTRIBUTING.md`).

## Exchange transcripts (`goldens/exchanges/`)

Byte-exact engine replay (`midici_conformance::transcript`). Phase 3 uses
`ENGINE ci` (default). Phase 4 PE Get uses `ENGINE responder`.

| Transcript | Spec anchors |
|---|---|
| `01-broadcast-discovery.transcript` | M2-101 §5.5 / §5.6 |
| `02-directed-discovery.transcript` | M2-101 §5.5 / §5.6 |
| `03-collision.transcript` | M2-101 §5.9.1 Option B |
| `04-pe-get-deviceinfo.transcript` | M2-101 §8.5 / §8.7–§8.8; M2-103 DeviceInfo |
