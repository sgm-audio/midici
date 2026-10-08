# Verification ledger

Phases append verification entries (command, expected, observed, result).

## Constructed vectors — needs human check

M2-101-UM v1.2.1 contains **no worked byte-sequence examples** for Management messages
(Discovery / Reply / Endpoint / Invalidate / ACK / NAK). The following goldens were
constructed field-by-field from the cited tables; each `.hex` file includes per-byte
section citations. Scott must diff-review against the PDFs (HUMAN GATE G1) before Phase 2.

| Golden | Spec anchor | Notes |
|---|---|---|
| `goldens/mgmt/01-discovery.hex` | M2-101 §5.5 Table 6 (p.27) | v2 Discovery; mfr `7D 00 00`; caps PE; max SysEx 512 |
| `goldens/mgmt/02-reply-to-discovery.hex` | M2-101 §5.6 Table 8 (p.29) | v2 Reply; caps Profiles\|PE; FB=0 |
| `goldens/mgmt/03-endpoint-inquiry.hex` | M2-101 §5.7 Table 9–10 (p.30) | Status `0x00` Product Instance ID |
| `goldens/mgmt/04-endpoint-reply.hex` | M2-101 §5.8 Table 11 (p.31) / §5.8.3.1 | Info `"ABC"` ASCII 32–126 |
| `goldens/mgmt/05-invalidate-muid.hex` | M2-101 §5.9 Table 12 (p.32) | Dest broadcast; target `0x0A0B0C0D` |
| `goldens/mgmt/06-ack.hex` | M2-101 §5.10 Table 13–14 (p.33–34) | Status ACK `0x00`; empty message text |
| `goldens/mgmt/07-nak.hex` | M2-101 §5.11 Table 15–16 (p.35–36) | Status `0x01` not supported; empty text |

### Constructed Phase-7 vectors (same caveat — no worked byte examples in the specs)

| Golden | Spec anchor | Notes |
|---|---|---|
| `goldens/exchanges/05-pe-set-subscribe.transcript` | M2-101 §8.9–§8.12 Tables 35–38 (pp.55–58); M2-103 §7.2/§8.2/§11.1–11.2 (pp.40–44) | Caps → subscribe `X-Test` (subscribeId `00000001`) → Set `{"value":7}` → reply 200 + auto `{"command":"notify"}` → end. Deterministic (SEED 0xC0FFEE, engine-generated). Header first-property rule: request=`command`/`resource`, reply=`status`. |

### Encoding conventions used in constructed vectors

- Multibyte numeric fields use 7-bit LSB-first packing (same scheme as MUID). // M2-101 §5.2.1 note / §3.3.3
- Manufacturer 1-byte IDs padded `ID 00 00`. // M2-101 §5.5.1
- Message Format Version `0x02` (MIDI-CI 1.2). // M2-101 §5.2.1
