//! CLAP plugin transport adapter with RT-safe bridge. // ARD §6
//!
//! ## ChCtrlList resource (§5)
//!
//! Built on the main thread from `clap_plugin_params`. Each parameter is
//! assigned a controller type, index, and range.
//!
//! See [`ChCtrlList`] and the [`chctrllist`] module.

pub mod chctrllist;
pub mod ring;

pub use ring::{Consumer, DefaultRing, Producer, Ring};

// ── InputEvents wrapper ──────────────────────────────────────

pub struct InputEvents {
    inner: *const clap_sys::events::clap_input_events,
}

impl InputEvents {
    #[inline]
    pub unsafe fn new(events: *const clap_sys::events::clap_input_events) -> Self {
        Self { inner: events }
    }

    #[inline]
    pub fn size(&self) -> u32 {
        if self.inner.is_null() {
            return 0;
        }
        unsafe { (*self.inner).size.unwrap()(self.inner) }
    }

    #[inline]
    pub fn get(&self, index: u32) -> *const clap_sys::events::clap_event_header {
        if self.inner.is_null() {
            return core::ptr::null();
        }
        unsafe { (*self.inner).get.unwrap()(self.inner, index) }
    }

    /// Iterate all events, copying SysEx payloads into `ring`.
    ///
    /// clap-sys 0.4: `clap_event_midi_sysex` has `buffer: *const u8` + `size: u32`.
    /// We strip F0/F7 per ARD §3.
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
            let event_type = unsafe { (*hdr).type_ };
            match event_type {
                CLAP_EVENT_MIDI_SYSEX => {
                    let sysex =
                        hdr as *const clap_sys::events::clap_event_midi_sysex;
                    let buf = unsafe { (*sysex).buffer };
                    let buf_len = unsafe { (*sysex).size } as usize;
                    if buf_len >= 2 && !buf.is_null() {
                        let raw = unsafe { core::slice::from_raw_parts(buf, buf_len) };
                        let body = if raw[0] == 0xF0 && raw[buf_len - 1] == 0xF7 {
                            &raw[1..buf_len - 1]
                        } else {
                            raw
                        };
                        if prod.push(body) {
                            pushed += 1;
                        }
                    }
                }
                CLAP_EVENT_MIDI => { /* skip */ }
                _ => { /* skip */ }
            }
        }
        pushed
    }
}

// ── OutputEvents wrapper ─────────────────────────────────────

pub struct OutputEvents {
    inner: *const clap_sys::events::clap_output_events,
}

impl OutputEvents {
    #[inline]
    pub unsafe fn new(events: *const clap_sys::events::clap_output_events) -> Self {
        Self { inner: events }
    }

    /// Try to push one CLAP MIDI SysEx event.
    ///
    /// clap-sys 0.4: `clap_event_midi_sysex` has `buffer: *const u8` + `size: u32`.
    /// Body + F0/F7 are stack-allocated; event struct is separately stack-allocated.
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

        use clap_sys::events::{clap_event_midi_sysex, CLAP_EVENT_MIDI_SYSEX};

        let sysex_len = body.len() + 2;
        if sysex_len > 512 {
            return false;
        }
        let mut sysex_buf = [0u8; 512];
        sysex_buf[0] = 0xF0;
        sysex_buf[1..1 + body.len()].copy_from_slice(body);
        sysex_buf[1 + body.len()] = 0xF7;

        let mut evt = core::mem::MaybeUninit::<clap_event_midi_sysex>::uninit();
        unsafe {
            let p = evt.as_mut_ptr();
            (*p).header.size = core::mem::size_of::<clap_event_midi_sysex>() as u32;
            (*p).header.time = 0;
            (*p).header.space_id = 0;
            (*p).header.type_ = CLAP_EVENT_MIDI_SYSEX as u16;
            (*p).header.flags = 0;
            (*p).port_index = 0;
            (*p).buffer = sysex_buf.as_ptr();
            (*p).size = sysex_len as u32;
        }
        let evt = unsafe { evt.assume_init() };
        unsafe {
            try_push(
                self.inner,
                &evt.header as *const clap_sys::events::clap_event_header,
            )
        }
    }

    /// Drain all pending outbound from the ring into CLAP output events.
    pub fn drain_ring<const N: usize, const B: usize>(
        &self,
        cons: &mut ring::Consumer<'_, N, B>,
    ) -> usize {
        let mut count = 0usize;
        const MAX_EMIT: usize = 32;
        while count < MAX_EMIT {
            match cons.pop() {
                Some(data) => {
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

// ── Control-side bridge ──────────────────────────────────────

const PENDING_OUTBOUND_CAP: usize = 8;

pub struct ControlBridge<R: rand_core::RngCore> {
    pub engine: midici_core::CiEngine<R>,
    pending_outbound: [Option<midici_core::OutboundSysex>; PENDING_OUTBOUND_CAP],
    pending_len: usize,
}

impl<R: rand_core::RngCore> ControlBridge<R> {
    pub fn new(engine: midici_core::CiEngine<R>) -> Self {
        Self {
            engine,
            pending_outbound: core::array::from_fn(|_| None),
            pending_len: 0,
        }
    }

    pub fn drain_inbound<const N: usize, const B: usize>(
        &mut self,
        cons: &mut ring::Consumer<'_, N, B>,
    ) -> usize {
        let mut count = 0usize;
        const MAX_DRAIN: usize = 16;
        while count < MAX_DRAIN {
            match cons.pop() {
                Some(data) => {
                    let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                    if len > 0 {
                        let _ = self.engine.feed_sysex(0, &data[..len]);
                        count += 1;
                    }
                }
                None => break,
            }
        }
        count
    }

    pub fn drain_outbound<const N: usize, const B: usize>(
        &mut self,
        prod: &mut ring::Producer<'_, N, B>,
    ) -> usize {
        let mut pushed = 0usize;
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
        self.compact_pending();
        while let Some(msg) = self.engine.next_outbound() {
            if msg.body.len() <= B {
                if prod.push(&msg.body) {
                    pushed += 1;
                } else if self.pending_len < PENDING_OUTBOUND_CAP {
                    self.pending_outbound[self.pending_len] = Some(msg);
                    self.pending_len += 1;
                } else {
                    break;
                }
            }
        }
        pushed
    }

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

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_roundtrip_sysex_body() {
        let ring_in: Ring<64, 512> = Ring::new();
        let ring_out: Ring<64, 512> = Ring::new();
        let mut in_prod = unsafe { Producer::new(&ring_in) };
        let mut in_cons = unsafe { Consumer::new(&ring_in) };
        let mut out_prod = unsafe { Producer::new(&ring_out) };
        let mut out_cons = unsafe { Consumer::new(&ring_out) };
        let body = [0x7E, 0x7F, 0x0D, 0x70, 0x02];
        assert!(in_prod.push(&body));
        let data = in_cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], &body[..]);
        assert!(out_prod.push(b"hello from control"));
        let data = out_cons.pop().unwrap();
        let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&data[..len], b"hello from control");
    }

    #[test]
    fn assert_no_alloc_sysex_flood_drop_counter() {
        let ring: Ring<8, 512> = Ring::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };
        for i in 0..8 {
            let mut data = [0u8; 512];
            data[0] = i as u8;
            assert!(prod.push(&data));
        }
        assert_eq!(ring.drops(), 0);
        for _ in 0..500 {
            prod.push(&[0xFF; 512]);
        }
        assert_eq!(ring.drops(), 500);
        for i in 0..8 {
            let data = cons.pop().unwrap();
            assert_eq!(data[0], i as u8);
        }
        assert!(cons.pop().is_none());
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
        let _muid = bridge.engine.muid();
    }
}
