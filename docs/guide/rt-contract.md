# RT Contract

**Audio thread does exactly one thing: move bytes.**

This is non-negotiable. Any allocation, lock, syscall, or logging on the RT path is a defect, not a style issue.

## What the Audio Thread Does

| Operation | Allowed? |
|-----------|----------|
| `ring.push(bytes)` / `ring.peek()` + `commit()` | ✅ wait-free SPSC |
| `memcpy` / pointer arithmetic | ✅ |
| Integer math | ✅ |
| `Vec::push`, `Box::new`, `String::from` | ❌ |
| `Mutex::lock`, `RwLock::read` | ❌ |
| `println!`, `eprintln!`, `log::info!` | ❌ |
| `std::thread::spawn`, `tokio::spawn` | ❌ |
| File I/O, socket I/O | ❌ |
| JSON (de)serialization | ❌ |
| `CiEngine::feed_sysex`, `poll`, `next_outbound` | ❌ |

## CLAP RT Path (`midici-transport-clap`)

```rust
// clap_process() — runs on audio thread
fn process(&mut self, audio: &mut AudioBuffers, events: &Events) {
    // 1. Inbound: copy SysEx bytes → ring_in
    for event in events.input() {
        if let Some(sysex) = event.as_sysex() {
            self.ring_in.push(sysex.body());  // wait-free, drop-on-overflow
        }
    }

    // 2. Outbound: ring_out → CLAP MIDI events (peek/commit: the producer
    //    cannot overwrite the slot until commit publishes it)
    while let Some(bytes) = self.ring_out.peek() {
        events.output().push_sysex(bytes);  // wait-free
        self.ring_out.commit();
    }

    // 3. Audio processing (gain, etc.)
    apply_gain(audio, self.gain.load(Ordering::Relaxed));
}
```

**Zero allocation, zero locks, zero logging.** Overflow = drop + relaxed `AtomicUsize` counter.

## Control Thread

Runs all protocol logic:
- `ResponderEngine::poll(now)` — timeouts, timers
- `ResponderEngine::feed_sysex()` — parsing, state machines
- `PeController` — chunking, reassembly, resource registry, subscriptions
- `ResourceRegistry::get` / `set` — JSON serialization
- `notify_resource_changed` — subscription fan-out (control thread only!)

Control thread may allocate, lock, log, syscall freely.

## Reassembly Buffers (DoS Guard)

Pre-reserved at construction, never grow on hot path:

```
Reassembler (max_peers)
  └─ Peer × max_peers
       └─ Slot × 4 (MAX_CONCURRENT_PER_PEER)
            ├─ header: Vec<u8> (cap = 4 KiB)
            ├─ arena: Vec<u8> (cap = 64 KiB) — packed property fragments
            └─ index: Vec<FragmentMeta> (cap = 2048)
```

- Feed path only writes into reserved capacity (`Vec::push` within cap)
- Oversize property totals rejected with `PeError::Oversize` before writing
- 5th concurrent transaction for a peer → `PeError::TooManyConcurrent`
- Full peer table with all slots active → `PeError::PeerTableFull`
- On timeout (3 s) or completion → `clear()` resets lengths, capacities stay

## Ring Buffer (`midici-transport-clap/src/ring.rs`)

```rust
struct Ring<const N: usize, const B: usize> {
    buf: UnsafeCell<MaybeUninit<[[u8; B]; N]>>, // N slots of B bytes
    write_idx: AtomicUsize,
    read_idx: AtomicUsize,
    drops: AtomicUsize,
}
```

- Two-phase consume: `peek()` returns the slot; `commit()` publishes it as
  read — the producer may overwrite a slot only after commit. (A `pop()`-style
  API that advanced `read_idx` before returning the slice was a real race;
  caught by miri in Phase 7.)
- Single `Acquire`/`Release` fence per push/peek
- Cached-index fast path for the uncontended case
- Overflow = drop + `AtomicUsize` counter (surfaced to UI/log from control thread)
- Verified: `cargo +nightly miri test -p midici-transport-clap` (13 tests pass)

## Verification

```bash
# Concurrency model checking (includes miri SPSC tests)
cargo +nightly miri test -p midici-transport-clap
```

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| `Vec::push` on RT path | Pre-reserve capacity; use ring buffer |
| `format!` / `to_string` | Move formatting to control thread |
| `Mutex` for shared state | Use atomics or wait-free structures |
| `log::info!` in `clap_process` | Remove; counter + control thread logging |
| `serde_json::to_vec` on RT | Do JSON on control thread only |
| Implicit allocation in `Drop` | Avoid `Box`/`Vec`/`String` in RT types |

## Latency Budget

| Operation | Budget |
|-----------|--------|
| Discovery reply | < 100 ms |
| PE round-trip (loopback) | < 50 ms |
| RT path per event | ≤ 2 memcpys, zero alloc |

Sample rate is irrelevant to CI except that it bounds how much SysEx a block can carry. The ring + incremental reassembler handles multi-block SysEx by construction; there is no per-block deadline.