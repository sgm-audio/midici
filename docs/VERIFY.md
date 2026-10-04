# Verification ledger

This file records human review of constructed wire vectors. Running unit tests or transcript replay proves only that the checked-in bytes round-trip through this implementation; it does not prove that a vector matches the specification.

## Constructed management vectors — human review still open

The recorded review status is **pending**: no human sign-off is recorded in this ledger. The vector rows below were constructed field-by-field from the cited tables, not captured from an external implementation. A reviewer must compare them with the checked-in PDFs before they are treated as approved protocol evidence.

| Golden | Spec anchor | Notes |
|---|---|---|
| `goldens/mgmt/01-discovery.hex` | M2-101 §5.5 Table 6 (p.27) | v2 Discovery; mfr `7D 00 00`; caps PE; max SysEx 512 |
| `goldens/mgmt/02-reply-to-discovery.hex` | M2-101 §5.6 Table 8 (p.29) | v2 Reply; caps Profiles\|PE; FB=0 |
| `goldens/mgmt/03-endpoint-inquiry.hex` | M2-101 §5.7 Table 9–10 (p.30) | Status `0x00` Product Instance ID |
| `goldens/mgmt/04-endpoint-reply.hex` | M2-101 §5.8 Table 11 (p.31) / §5.8.3.1 | Info `"ABC"` ASCII 32–126 |
| `goldens/mgmt/05-invalidate-muid.hex` | M2-101 §5.9 Table 12 (p.32) | Dest broadcast; target `0x0A0B0C0D` |
| `goldens/mgmt/06-ack.hex` | M2-101 §5.10 Table 13–14 (p.33–34) | Status ACK `0x00`; empty message text |
| `goldens/mgmt/07-nak.hex` | M2-101 §5.11 Table 15–16 (p.35–36) | Status `0x01` not supported; empty text |

### Constructed PE vector — human review still open

| Golden | Spec anchor | Notes |
|---|---|---|
| `goldens/exchanges/05-pe-set-subscribe.transcript` | M2-101 §8.9–§8.12 Tables 35–38 (pp.55–58); M2-103 §7.2/§8.2/§11.1–11.2 (pp.40–44) | Caps → subscribe `X-Test` (subscribeId `00000001`) → Set `{"value":7}` → reply 200 + auto `{"command":"notify"}` → end. Deterministic (SEED 0xC0FFEE, engine-generated). Header first-property rule: request=`command`/`resource`, reply=`status`. |

### Encoding conventions in the constructed vectors

These are the conventions recorded when the fixtures were created; they remain subject to the human review above.

- Multibyte numeric fields use 7-bit LSB-first packing (M2-101 §5.2.1 note / §3.3.3).
- Manufacturer 1-byte IDs are padded `ID 00 00` (M2-101 §5.5.1).
- Message Format Version `0x02` is used for MIDI-CI 1.2 (M2-101 §5.2.1).

**Review procedure:** compare each fixture byte-for-byte with the cited PDF tables, record reviewer/date and any discrepancies here, and follow `AGENTS.md`'s append-only golden rule. Do not edit or replace golden files as part of routine review without the required human-approved change.
