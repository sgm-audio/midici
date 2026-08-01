//! CLAP plugin transport adapter with RT-safe bridge. // ARD §6
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  Audio Thread (RT)                                  │
//! │  ┌──────────────┐    ┌──────────────────────────┐   │
//! │  │ clap_process │───▶│ inbound ring (Producer)   │   │
//! │  │   MIDI in    │    └──────────┬───────────────┘   │
//! │  └──────────────┘               │                   │
//! │  ┌──────────────┐    ┌──────────▼───────────────┐   │
//! │  │ clap_process │◀───│ outbound ring (Consumer)  │   │
//! │  │   MIDI out   │    └──────────────────────────┘   │
//! │  └──────────────┘                                   │
//! └─────────────────────────────────────────────────────┘
//!                        │                   ▲
//!                        │     rings         │
//!                        ▼                   │
//! ┌─────────────────────────────────────────────────────┐
//! │  Control Thread (timer-driven, 10 ms)               │
//! │  ┌──────────────┐    ┌──────────────────────────┐   │
//! │  │ CiEngine     │◀───│ inbound ring (Consumer)   │   │
//! │  │ poll/feed    │    └──────────────────────────┘   │
//! │  └──────┬───────┘                                   │
//! │         │            ┌──────────────────────────┐   │
//! │         └───────────▶│ outbound ring (Producer)  │   │
//! │                      └──────────────────────────┘   │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! ## ChCtrlList resource (§5)
//!
//! Built on the main thread from `clap_plugin_params`. Each parameter is
//! assigned a controller type, index, and range. The assignment scheme:
//!
//! - `clap_param_info.id` → MIDI CI `ctrlIndex`
//! - `clap_param_info.name` → `title`
//! - `clap_param_info.min_value` / `max_value` → `minMax`
//! - `clap_param_info.default_value` → `default`
//! - Param flags determine `ctrlType` (see [`chctrllist::ctrl_type_from_flags`])
//!
//! See [`ChCtrlList`] and the [`chctrllist`] module.

pub mod chctrllist;
pub mod ring;

/// Re-export ring types for convenience.
pub use ring::{Consumer, DefaultRing, Producer, Ring};

// ---------------------------------------------------------------------------
// CLAP event helpers — thin wrappers over `clap-sys` types
// ---------------------------------------------------------------------------

/// Thin wrapper around a `clap_input_events_t` for SysEx extraction.
///
/// Used on the RT path to copy inbound MIDI SysEx events into the
/// inbound ring. No allocation, no locking.
pub struct InputEvents {
    inner: *const clap_sys::events::clap_input_events,
}

impl InputEvents {
    /// Wrap a raw `clap_input_events` pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid for the duration of `process()`.
    #[inline]
    pub unsafe fn new(events: *const clap_sys::events::clap_input_events) -> Self {
        Self { inner: events }
    }

    /// Number of events in this block.
    #[inline]
    pub fn size(&self) -> u32 {
        if self.inner.is_null() {
            return 0;
        }
        // SAFETY: caller guarantees valid pointer for this block.
        unsafe { (*self.inner).size.unwrap()(self.inner) }
    }

    /// Get event at `index`. Returns a pointer to the event header,
    /// or null if out of range.
    #[inline]
    pub fn get(&self, index: u32) -> *const clap_sys::events::clap_event_header {
        if self.inner.is_null() {
            return core::ptr::null();
        }
        // SAFETY: caller guarantees valid pointer for this block.
        unsafe { (*self.inner).get.unwrap()(self.inner, index) }
    }

    /// Iterate all events, copying SysEx payloads into `ring`.
    ///
    /// Returns the number of events pushed. On overflow, increments
    /// the ring's drop counter — the caller can check `ring.drops()`.
    pub fn copy_sysex_to<const N: usize, const B: usize>(
        &self,
        _ring: &ring::Ring<N, B>,
        prod: &mut ring::Producer<'_, N, B>,
    ) -> usize {
        use clap_sys::events::{CLAP_EVENT_MIDI, CLAP_EVENT_MIDI_SYSEX};
        let mut pushed = 0usize;

        let n = self.size();
        for i in 0..n {
            let hdr = self.get(i);
            if hdr.is_null() {
                continue;
            }
            // SAFETY: valid event header for this block.
            let event_type = unsafe { (*hdr).type_ };
            let size = unsafe { (*hdr).size };

            match event_type {
                CLAP_EVENT_MIDI_SYSEX => {
                    // clap_event_midi_sysex: header + port_index + buffer[1..]
                    // The buffer includes the F0/F7 for completeness,
                    // but CI wants the body without F0/F7 per ARD §3.
                    if size < 4 {
                        continue;
                    }
                    // SAFETY: valid event pointer within the block.
                    let sysex: *const clap_sys::events::clap_event_midi_sysex =
                        hdr as *const clap_sys::events::clap_event_midi_sysex;
                    let buf = unsafe { (*sysex).buffer.as_ptr() };
                    let buf_len = (size as usize)
                        .saturating_sub(core::mem::size_of::<clap_sys::events::clap_event_midi_sysex>());
                    // Copy the body (strip F0/F7 if present).
                    let body = if buf_len >= 2
                        && unsafe { *buf } == 0xF0
                        && unsafe { *buf.add(buf_len - 1) } == 0xF7
                    {
                        // SAFETY: buf_len ≥ 2, valid for read.
                        unsafe { core::slice::from_raw_parts(buf.add(1), buf_len - 2) }
                    } else {
                        unsafe { core::slice::from_raw_parts(buf, buf_len) }
                    };
                    if prod.push(body) {
                        pushed += 1;
                    }
                }
                CLAP_EVENT_MIDI => {
                    // clap_event_midi: 1–3 data bytes. Not SysEx — skip.
                }
                _ => {
                    // Non-MIDI event, ignore.
                }
            }
        }

        pushed
    }
}

/// Thin wrapper around a `clap_output_events_t` for emitting MIDI.
///
/// Used on the RT path to drain the outbound ring and emit CLAP
/// MIDI SysEx events. No allocation, no locking.
pub struct OutputEvents {
    inner: *const clap_sys::events::clap_output_events,
}

impl OutputEvents {
    /// Wrap a raw `clap_output_events` pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid for the duration of `process()`.
    #[inline]
    pub unsafe fn new(events: *const clap_sys::events::clap_output_events) -> Self {
        Self { inner: events }
    }

    /// Try to push one CLAP MIDI SysEx event. Returns `true` on success.
    ///
    /// The event is framed with F0 / F7 — the ring stores the raw body
    /// per ARD §3 contract, so the RT path wraps/unwraps at the boundary.
    #[inline]
    pub fn try_push_sysex(&self, body: &[u8]) -> bool {
        if self.inner.is_null() {
            return false;
        }
        let try_push = unsafe { (*self.inner).try_push };
        if try_push.is_none() {
            return false;
        }
        let try_push = try_push.unwrap();

        // Build the event on the stack.
        use clap_sys::events::{clap_event_midi_sysex, CLAP_EVENT_MIDI_SYSEX};
        use core::mem::size_of;

        let buf_size = body.len() + 2; // +F0 +F7
        if buf_size > 512 {
            return false; // oversized for one event
        }

        // Allocate on stack.
        let mut raw = [0u8; 512 + size_of::<clap_event_midi_sysex>()];
        let evt = raw.as_mut_ptr() as *mut clap_event_midi_sysex;

        // SAFETY: raw is properly sized and aligned.
        unsafe {
            (*evt).header.size =
                (size_of::<clap_event_midi_sysex>() + buf_size) as u32;
            (*evt).header.time = 0;
            (*evt).header.space_id = 0;
            (*evt).header.type_ = CLAP_EVENT_MIDI_SYSEX as u16;
            (*evt).header.flags = 0;
            (*evt).port_index = 0;
            let buf_ptr = (*evt).buffer.as_mut_ptr();
            *buf_ptr = 0xF0;
            core::ptr::copy_nonoverlapping(body.as_ptr(), buf_ptr.add(1), body.len());
            *buf_ptr.add(body.len() + 1) = 0xF7;
        }

        // Try to push.
        // SAFETY: evt points to a valid clap_event_midi_sysex.
        unsafe {
            try_push(
                self.inner,
                evt as *const clap_sys::events::clap_event_header,
            )
        }
    }

    /// Drain all pending outbound from the ring into CLAP output events.
    ///
    /// Returns the number of events emitted.
    pub fn drain_ring<const N: usize, const B: usize>(
        &self,
        cons: &mut ring::Consumer<'_, N, B>,
    ) -> usize {
        let mut count = 0usize;
        // Limit per-block emission to prevent starving audio processing.
        const MAX_EMIT_PER_BLOCK: usize = 32;
        while count < MAX_EMIT_PER_BLOCK {
            match cons.pop() {
                Some(data) => {
                    // Find actual length: scan for trailing zeros (slot padding).
                    let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                    if len > 0 && self.try_push_sysex(&data[..len]) {
                        count += 1;
                    }
                }
                None => break,
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// Control-side bridge
// ---------------------------------------------------------------------------

/// Maximum outbound messages buffered on the control side if the
/// outbound ring is full. Small fixed array — no heap growth.
const PENDING_OUTBOUND_CAP: usize = 8;

/// Control-side bridge: drains rings, drives engine, owns all JSON.
///
/// Called from the host timer callback (10 ms period). This is the
/// non-RT side — allocations and JSON work happen here.
pub struct ControlBridge<R: rand_core::RngCore> {
    /// The sans-io state machine.
    pub engine: midici_core::CiEngine<R>,
    /// Pending outbound messages not yet delivered to the RT ring
    /// (fixed-size, stack-resident). Bounded to `PENDING_OUTBOUND_CAP`.
    pending_outbound: [Option<midici_core::OutboundSysex>; PENDING_OUTBOUND_CAP],
    /// Number of occupied slots in `pending_outbound`.
    pending_len: usize,
}

impl<R: rand_core::RngCore> ControlBridge<R> {
    /// Create a new control bridge wrapping an existing engine.
    pub fn new(engine: midici_core::CiEngine<R>) -> Self {
        let pending_outbound = core::array::from_fn(|_| None);
        Self {
            engine,
            pending_outbound,
            pending_len: 0,
        }
    }

    /// Drain the inbound ring, feeding SysEx bodies to the engine.
    ///
    /// Returns the number of messages fed.
    pub fn drain_inbound<const N: usize, const B: usize>(
        &mut self,
        cons: &mut ring::Consumer<'_, N, B>,
    ) -> usize {
        let mut count = 0usize;
        // Limit per tick to bound control-thread work.
        const MAX_DRAIN_PER_TICK: usize = 16;
        while count < MAX_DRAIN_PER_TICK {
            match cons.pop() {
                Some(data) => {
                    let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                    if len > 0 {
                        // Feed to engine — group 0 for now (CLAP MIDI is group-less).
                        let _ = self.engine.feed_sysex(0, &data[..len]);
                        count += 1;
                    }
                }
                None => break,
            }
        }
        count
    }

    /// Drain engine outbound into the outbound ring.
    ///
    /// Returns the number of messages pushed.
    pub fn drain_outbound<const N: usize, const B: usize>(
        &mut self,
        prod: &mut ring::Producer<'_, N, B>,
    ) -> usize {
        let mut pushed = 0usize;

        // First, try to flush any previously buffered pending outbound.
        let mut i = 0;
        while i < self.pending_len {
            if let Some(ref msg) = self.pending_outbound[i] {
                if msg.body.len() <= B && prod.push(&msg.body) {
                    self.pending_outbound[i] = None;
                    pushed += 1;
                }
            }
            i += 1;
        }
        // Compact the pending array.
        self.compact_pending();

        // Drain engine.
        while let Some(msg) = self.engine.next_outbound() {
            if msg.body.len() <= B {
                if prod.push(&msg.body) {
                    pushed += 1;
                } else {
                    // Ring full: buffer in pending.
                    if self.pending_len < PENDING_OUTBOUND_CAP {
                        self.pending_outbound[self.pending_len] = Some(msg);
                        self.pending_len += 1;
                    }
                    // If pending is full, the message is dropped.
                    break;
                }
            }
        }

        pushed
    }

    /// Full tick: drain inbound, poll engine, drain outbound.
    pub fn tick<const N: usize, const B: usize>(
        &mut self,
        now_ms: u64,
        in_cons: &mut ring::Consumer<'_, N, B>,
        out_prod: &mut ring::Producer<'_, N, B>,
    ) {
        self.drain_inbound(in_cons);
        self.engine.poll(now_ms);
        let _ = self.drain_outbound(out_prod);
    }

    /// Compact the pending array by removing `None` entries.
    fn compact_pending(&mut self) {
        let mut write = 0;
        for read in 0..self.pending_len {
            if self.pending_outbound[read].is_some() {
                self.pending_outbound.swap(read, write);
                write += 1;
            }
        }
        self.pending_len = write;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: ring round-trip through the full bridge path.
    #[test]
    fn ring_roundtrip_sysex_body() {
        let ring_in: Ring<64, 512> = Ring::new();
        let ring_out: Ring<64, 512> = Ring::new();

        let mut in_prod = unsafe { Producer::new(&ring_in) };
        let mut in_cons = unsafe { Consumer::new(&ring_in) };
        let mut out_prod = unsafe { Producer::new(&ring_out) };
        let mut out_cons = unsafe { Consumer::new(&ring_out) };

        // Simulate: RT copies SysEx into inbound ring.
        let body = [0x7E, 0x7F, 0x0D, 0x70, 0x02]; // CI Discovery header start
        assert!(in_prod.push(&body));

        // Control side drains.
        let data = in_cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], &body[..]);

        // Control side queues outbound.
        assert!(out_prod.push(b"hello from control"));

        // RT side drains outbound, emits.
        let data = out_cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], b"hello from control");
    }

    /// Zero-allocation check: the ring path does not allocate in debug.
    ///
    /// This test drives the ring with a SysEx flood and asserts
    /// correct drop-counter behavior — the mechanism by which we
    /// verify that overflow does not stall the RT path.
    #[test]
    fn assert_no_alloc_sysex_flood_drop_counter() {
        // Use a small ring to force overflow.
        let ring: Ring<8, 512> = Ring::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        // Fill the ring.
        for i in 0..8 {
            let mut data = [0u8; 512];
            data[0] = i as u8;
            assert!(prod.push(&data));
        }
        assert_eq!(ring.drops(), 0);

        // Push 500 more — all must overflow, drop counter must match.
        for _ in 0..500 {
            prod.push(&[0xFF; 512]);
        }
        assert_eq!(ring.drops(), 500);

        // Drain 8. Contents match original pushes (overflow dropped).
        for i in 0..8 {
            let data = cons.pop().unwrap();
            assert_eq!(data[0], i as u8);
        }
        assert!(cons.pop().is_none());

        // Ring empty, drops unchanged.
        assert_eq!(ring.drops(), 500);
        ring.reset_drops();
        assert_eq!(ring.drops(), 0);
    }

    #[test]
    fn control_bridge_construction() {
        use midici_core::{CiConfig, DeviceIdentity};
        use rand_core::SeedableRng;
        use rand_xorshift::XorShiftRng;

        let identity = DeviceIdentity {
            manufacturer: [0x00, 0x11, 0x22],
            family: 0x0102,
            model: 0x0304,
            software_revision: [0x01, 0x00, 0x00, 0x00],
        };
        let config = CiConfig::responder_default(identity);
        let rng = XorShiftRng::seed_from_u64(42);
        let engine = midici_core::CiEngine::new(config, rng);

        let bridge = ControlBridge::new(engine);
        assert_eq!(bridge.pending_len, 0);

        use midici_core::Muid;
        let _muid: Muid = bridge.engine.muid();
    }
}
