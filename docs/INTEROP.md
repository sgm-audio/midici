# Interoperability evidence

This is an evidence ledger, not a planned-pass matrix. As of 2026-10-04, no external tool, DAW, Windows service, or hardware test result has been recorded below. **Blank cells mean “not tested,” never “pass.”** Internal loopback/golden tests do not count as external interoperability evidence.

| Target | Discovery | PE Get DeviceInfo | PE Get ResourceList | PE Get ChCtrlList | Endpoint Info | Evidence / notes |
|---|---|---|---|---|---|---|
| MIDI 2.0 Workbench | Not tested | Not tested | Not tested | Not available (`ChCtrlList` not built in) | Not tested | No capture or test log recorded. |
| Bitwig on Linux (UMP) | Not tested | Not tested | Not tested | Not available (`ChCtrlList` not built in) | Not tested | No capture or test log recorded. |
| Windows MIDI Services console tooling | Not tested | Not tested | Not tested | Not available (`ChCtrlList` not built in) | Not tested | No Windows transport exists in this workspace. |
| Hardware PE controller | Not tested | Not tested | Not tested | Not available (`ChCtrlList` not built in) | Not tested | No hardware evidence recorded. |

The ALSA live smoke test is separately feature-gated and ignored by default; it checks local endpoint availability, not interoperability with a peer. See `crates/midici-transport-alsa/tests/live.rs` and [`PROGRESS.md`](PROGRESS.md).
