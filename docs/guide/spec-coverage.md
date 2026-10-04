# Spec Coverage

Message-by-message coverage vs M2-101-UM (MIDI-CI v1.2) and M2-103-UM (PE v1.1).

## Management (M2-101 §5 / Appendix E)

| Sub-ID#2 | Message | Status | Spec Ref | Notes |
|----------|---------|--------|----------|-------|
| 0x70 | Discovery | ✅ | §5.5 Table 6 | Broadcast + directed; caps bits, max SysEx, output path |
| 0x71 | Reply to Discovery | ✅ | §5.6 Table 8 | Identity, caps, max SysEx, negotiated features |
| 0x72 | Endpoint Inquiry | ✅ | §5.7 Table 9 | Product Instance ID only |
| 0x73 | Endpoint Reply | 🔄 inquiry-only | §5.8 Table 11 | Sends Inquiry; Reply NAKs NotSupported |
| 0x7D | ACK | ✅ | §5.10 Table 13 | Status 0x00/0x10/0x11; feature-masked for v1.1 |
| 0x7E | Invalidate MUID | ✅ | §5.9 Table 12 | Broadcast dest; collision → regen + re-announce |
| 0x7F | NAK | ✅ | §5.11 Table 15 | All status codes; typed `NakCode` |

### Capability Bits (M2-101 §5.5.2 Table 7)

| Bit | Category | Advertised | Handled |
|-----|----------|------------|---------|
| D1 | Protocol Negotiation | ❌ (deprecated) | ❌ |
| D2 | Profile Configuration | ✅ | 🔄 inquiry-only |
| D3 | Property Exchange | ✅ | ✅ full |
| D4 | Process Inquiry | ✅ | ❌ (v2) |

## Property Exchange (M2-101 §8 / M2-103)

| Sub-ID#2 | Message | Status | Spec Ref | Notes |
|----------|---------|--------|----------|-------|
| 0x30 | PE Capabilities Inquiry | ✅ | §8.5 Table 30 | Negotiates simultaneous + PE version |
| 0x31 | PE Capabilities Reply | ✅ | §8.6 Table 32 | |
| 0x34 | Get Inquiry | ✅ | §8.7 Table 33 | JSON header, chunked |
| 0x35 | Get Reply | ✅ | §8.8 Table 34 | JSON header + body, status codes |
| 0x36 | Set Inquiry | ✅ | §8.9 | Write policy per-resource (default NotAllowed/405); auto-Notifies subscribers |
| 0x37 | Set Reply | ✅ | §8.10 | `{"status":…}` header-only |
| 0x38 | Subscribe | ✅ | §8.11 | `start/end` inbound; `partial/full/notify` updates outbound |
| 0x39 | Subscribe Reply | ✅ | §8.12 | `{"status",subscribeId}`; SubId = 8-hex responder-allocated |
| 0x3F | Notify | ✅ receive-only | §8.13 | Deprecated v1.2; honored: status 144 terminates the named request |

### PE Status Codes (M2-103 §7.4.1 Table 15)

| Code | Name | Used | Mapping |
|------|------|------|---------|
| 200 | OK | ✅ | `PeStatus::Ok` |
| 341 | Unavailable | ✅ | `PeStatus::Unavailable` (stall timeout) |
| 400 | Bad Request | ✅ | `PeStatus::BadRequest` (malformed JSON, bad header) |
| 403 | Forbidden | ✅ | `PeStatus::Forbidden` (resource flag) |
| 404 | Not Found | ✅ | `PeStatus::NotFound` (unknown resource) |
| 405 | Not Allowed | ✅ | `PeStatus::NotAllowed` (resource flag, Set on read-only) |
| 413 | Payload Too Large | ✅ | `PeStatus::PayloadTooLarge` (chunk flood, >64 KiB) |
| 445 | Busy | ✅ | `PeStatus::Busy` (5th concurrent tx) — *ARD §7; M2-103 lists 343* |

**Note:** M2-103 Table 15 lists 343 as "Too Many Requests" and 445 as "Invalid Version of Data". This stack follows ARD §7 / Phase-4 DoD mapping excess concurrent → **445**. Documented on `PeStatus::Busy`.

### Encodings (M2-103 §6)

| Encoding | Status | Spec Ref |
|----------|--------|----------|
| ASCII (7-bit JSON) | ✅ | §6.1.6 |
| Mcoded7 | ✅ | §6.1.7 |
| zlib + Mcoded7 | feature `zlib` | §6.1.8 |

### Chunking (M2-101 §8.3 / M2-103 §5.2)

| Rule | Status | Spec Ref |
|------|--------|----------|
| requestId (7-bit) correlation | ✅ | §5.3 |
| numChunks/chunkNum (14-bit) | ✅ | §5.2 |
| Header only in chunk 1 | ✅ | §8.3.1 |
| Header-only = single chunk | ✅ | §8.3.2 |
| Clamp 128..=4096 | ✅ | §5.5.3 / ARD §4 |

## Resources (ARD §5 / M2-103 §6.2)

| Resource | Status | Spec Ref | Notes |
|----------|--------|----------|-------|
| DeviceInfo | ✅ | §6.2.1 | Manufacturer/Family/Model/Version arrays |
| ResourceList | ✅ | §6.2.2 | Auto-generated from registry |
| ChCtrlList | ✅ | §8.2 / ARD §5 | Full per-channel + per-note mapping |
| State | 🔄 v2 | §6.2.3 | Needs plugin state extension design |
| StateList | 🔄 v2 | §6.2.4 | |
| ProfileList | 🔄 v2 | §7 | |

## Profiles (M2-101 §6–7)

| Message | Status | Notes |
|---------|--------|-------|
| 0x20 Profile Inquiry | 🔄 inquiry-only | Replies zero profiles |
| 0x21 Profile Inquiry Reply | 🔄 inquiry-only | |
| 0x22 Profile Set | 🔄 v2 | NAK NotSupported |
| 0x23 Profile Set Reply | 🔄 v2 | |
| 0x24 Profile Get | 🔄 v2 | |
| 0x25 Profile Get Reply | 🔄 v2 | |
| 0x26 Profile Enabled | 🔄 v2 | |
| 0x27 Profile Disabled | 🔄 v2 | |
| 0x28 Profile Specific Data | 🔄 v2 | |
| 0x29 Profile Specific Data Reply | 🔄 v2 | |

**v1 ships polite refusal:** all Profile messages beyond Inquiry/Reply NAK with NotSupported. Full profile support (DAW Ctrl, Orchestral Articulation) is v2.

## Version Handling

| Scenario | Behavior |
|----------|----------|
| We advertise v1.2 | `ci ver = 0x02` |
| Peer is v1.1 | Mask v1.2-only features (ACK, Endpoint); never NAK on version |
| Peer sends reserved version bits | NAK 0x02 (Version Not Supported) |
| Peer sends unknown sub-ID#2 | NAK 0x01 (Not Supported) |

## Gaps & Deferred (Honest)

| Area | Gap | Target |
|------|-----|--------|
| PE Set | ✅ Phase 7 (write policy + fan-out) | — |
| PE Subscribe/Notify | ✅ Phase 7 (SubId alloc, lifecycle, peer-liveness reap) | — |
| Profile Configuration | Inquiry-only; rest NAK | v2 |
| Process Inquiry | Not implemented | v2 |
| State/StateList resources | Deferred | v2 |
| CoreMIDI transport | Not started | v2 |
| Windows MIDI Services transport | Not started | v2 |
| SMF2/Clip File | Separate crate | later |
| Initiator (client) role | Responder-only v1 | v2 |

## Test Coverage

| Test Type | Coverage |
|-----------|----------|
| Golden transcript replay | Discovery, PE Caps, Get DeviceInfo, Set+Subscribe+Notify exchange |
| NAK matrix | All 7 status codes + malformed |
| Loopback | max SysEx 128 + 4096; payload equality + chunk count; Set 200/403/404/405; subscribe→notify→unsubscribe; peer-vanish reap |
| Fuzz | SysEx framing, CI header, PE JSON, reassembler (10⁶ execs each) |
| Property tests | Mcoded7 round-trip; chunk split/reassemble identity |
| RT audit | `assert_no_alloc` + miri on ring; 24h soak |