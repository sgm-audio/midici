# HUMAN GATE G1 still open for Phase 1 mgmt goldens.

## Exchange transcripts (`goldens/exchanges/`)

Byte-exact engine replay (`midici_conformance::transcript`). Phase 3 uses
`ENGINE ci` (default). Phase 4 PE Get uses `ENGINE responder`.

| Transcript | Spec anchors |
|---|---|
| `01-broadcast-discovery.transcript` | M2-101 §5.5 / §5.6 |
| `02-directed-discovery.transcript` | M2-101 §5.5 / §5.6 |
| `03-collision.transcript` | M2-101 §5.9.1 Option B |
| `04-pe-get-deviceinfo.transcript` | M2-101 §8.5 / §8.7–§8.8; M2-103 DeviceInfo |
