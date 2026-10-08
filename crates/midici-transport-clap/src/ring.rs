//! Wait-free SPSC byte rings for the CLAP RT bridge. // ARD §6
//!
//! Two unidirectional rings (inbound, outbound) carry SysEx byte chunks
//! between the audio thread and the control thread.
//!
//! Consumption is two-phase — [`Consumer::peek`] returns a slice into the
//! slot, [`Consumer::commit`] publishes it as consumed — so the producer can
//! never overwrite bytes the reader is still using (miri-verified).
//!
//! ## Example
//!
//! ```
//! use midici_transport_clap::{Consumer, Producer, Ring};
//!
//! let ring: Ring<4, 64> = Ring::new();
//! // SAFETY: this thread owns both sides for the demo; in production each
//! // half is owned by exactly one thread (SPSC).
//! let mut prod = unsafe { Producer::new(&ring) };
//! let mut cons = unsafe { Consumer::new(&ring) };
//!
//! assert!(prod.push(b"hello")); // per-slot length tracks the payload
//! let slot = cons.peek().unwrap();
//! assert_eq!(slot.len(), 5);
//! assert_eq!(slot, b"hello");
//! cons.commit(); // publish as consumed — only now may the producer reuse it
//! assert!(cons.peek().is_none());
//! ```
//!
//! ## Geometry
//!
//! Const-generic: `N` slots of `B` bytes each. Default is 64 × 512 B
//! (32 KiB/ring), tunable via type parameters.
//!
//! ## RT contract (§6)
//!
//! The producer (`push`) and consumer (`pop`) are each single-owner,
//! lock-free, and allocation-free. Only [`core::sync::atomic`] operations
//! are used. No logging, no panics on the hot path.
//!
//! ## Overflow policy (§7)
//!
//! When the ring is full, incoming bytes are **dropped** and a relaxed
//! atomic counter is incremented. The control thread reads and resets
//! the counter for telemetry.
//!
//! ## Safety
//!
//! This is a SPSC ring: each end is owned by exactly one thread.
//! Concurrent `push`/`push` or `pop`/`pop` is UB. Miri tests enforce
//! this with `thread::scope`-isolated access.
//!
//! ## Implementation
//!
//! The ring uses a flat byte buffer of `N * B` bytes plus a per-slot
//! `u32` length array, each behind a [`core::cell::UnsafeCell`]. Producers
//! and consumers compute slot offsets from their respective indices. This
//! avoids nested `MaybeUninit` arrays and keeps the memory model simple for
//! miri.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

/// A wait-free SPSC byte-slot ring.
///
/// `N` slots, each `B` bytes. The ring stores raw byte chunks — no
/// serialization, no parsing.
#[derive(Debug)]
pub struct Ring<const N: usize, const B: usize> {
    /// Flat buffer: `N` slots of `B` bytes, stored as nested arrays.
    ///
    /// `[u8; N * B]` needs nightly `generic_const_exprs`; `[[u8; B]; N]`
    /// is layout-identical and stable.
    buf: UnsafeCell<MaybeUninit<[[u8; B]; N]>>,
    /// Per-slot payload length in bytes (`0..=B`). The producer writes it
    /// before publishing the slot; the consumer reads it after. This makes
    /// the length explicit — payloads may legitimately contain (or end with)
    /// `0x00` bytes, so "last non-zero byte" is NOT the length convention.
    lens: UnsafeCell<MaybeUninit<[u32; N]>>,
    /// Next write slot index (producer advances).
    write_idx: AtomicUsize,
    /// Next read slot index (consumer advances).
    read_idx: AtomicUsize,
    /// Count of dropped pushes since last reset.
    drops: AtomicUsize,
}

/// Default ring geometry: 64 slots × 512 bytes = 32 KiB. // ARD §6
pub type DefaultRing = Ring<64, 512>;

// Safety: Ring is Send + Sync when the two ends are owned by different
// threads and access is SPSC-disciplined. The caller must uphold this.
unsafe impl<const N: usize, const B: usize> Send for Ring<N, B> {}
unsafe impl<const N: usize, const B: usize> Sync for Ring<N, B> {}

impl<const N: usize, const B: usize> Ring<N, B> {
    /// Create a ring with uninitialised buffer.
    pub fn new() -> Self {
        Self {
            buf: UnsafeCell::new(MaybeUninit::uninit()),
            lens: UnsafeCell::new(MaybeUninit::uninit()),
            write_idx: AtomicUsize::new(0),
            read_idx: AtomicUsize::new(0),
            drops: AtomicUsize::new(0),
        }
    }

    /// Number of pending slots (producer view). Relaxed: approximate is fine.
    #[inline]
    pub fn len(&self) -> usize {
        let w = self.write_idx.load(Ordering::Relaxed);
        let r = self.read_idx.load(Ordering::Relaxed);
        w.wrapping_sub(r).min(N)
    }

    /// True when no slots are pending.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop counter since last reset. Relaxed: telemetry only.
    #[inline]
    pub fn drops(&self) -> usize {
        self.drops.load(Ordering::Relaxed)
    }

    /// Reset the drop counter to zero. Called from the control thread.
    #[inline]
    pub fn reset_drops(&self) {
        self.drops.store(0, Ordering::Relaxed);
    }
}

impl<const N: usize, const B: usize> Default for Ring<N, B> {
    fn default() -> Self {
        Self::new()
    }
}

/// Producer half: writes into the ring (RT audio thread for inbound,
/// control thread for outbound).
///
/// # Safety
///
/// Only one producer may exist per ring at a time.
#[derive(Debug)]
pub struct Producer<'a, const N: usize, const B: usize> {
    ring: &'a Ring<N, B>,
    /// Cached write index for the fast path.
    cached_write: usize,
    /// Cached read index (updated on full-ring check).
    cached_read: usize,
}

impl<'a, const N: usize, const B: usize> Producer<'a, N, B> {
    /// Borrow a ring for writing. The caller must ensure no other
    /// producer exists concurrently.
    ///
    /// # Safety
    ///
    /// SPSC discipline: only one `Producer` per ring.
    pub unsafe fn new(ring: &'a Ring<N, B>) -> Self {
        let cached_write = ring.write_idx.load(Ordering::Acquire);
        let cached_read = ring.read_idx.load(Ordering::Acquire);
        Self {
            ring,
            cached_write,
            cached_read,
        }
    }

    /// Try to push `data` into the ring. Returns `true` on success,
    /// `false` if the ring is full or `data` exceeds the slot size
    /// (data dropped, drop counter incremented).
    ///
    /// The payload is copied into the slot and its length recorded
    /// per-slot; `Consumer::peek` returns exactly the pushed bytes, so
    /// payloads may legitimately end in `0x00`.
    #[inline]
    pub fn push(&mut self, data: &[u8]) -> bool {
        if data.len() > B {
            // Chunk larger than a slot: drop + count.
            self.ring.drops.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // Full check.
        let pending = self.cached_write.wrapping_sub(self.cached_read);
        if pending >= N {
            // Refresh cached read index.
            self.cached_read = self.ring.read_idx.load(Ordering::Acquire);
            let pending = self.cached_write.wrapping_sub(self.cached_read);
            if pending >= N {
                self.ring.drops.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }

        let slot = self.cached_write % N;
        let offset = slot * B;
        let buf_ptr = self.ring.buf.get() as *mut u8;
        let len_ptr = self.ring.lens.get() as *mut u32;
        // SAFETY: producer has exclusive write access to this slot;
        // consumer won't read it until write_idx advances (published
        // below with Release). The length is written before publish, so
        // any consumer that observes the new write_idx (Acquire) also
        // observes the length.
        unsafe {
            let dst = buf_ptr.add(offset);
            let len = data.len();
            core::ptr::copy_nonoverlapping(data.as_ptr(), dst, len);
            core::ptr::write(len_ptr.add(slot), len as u32);
        }

        // Publish.
        self.cached_write = self.cached_write.wrapping_add(1);
        self.ring
            .write_idx
            .store(self.cached_write, Ordering::Release);
        true
    }

    /// Number of pending slots from the producer's cached view.
    #[inline]
    pub fn pending(&self) -> usize {
        self.cached_write.wrapping_sub(self.cached_read).min(N)
    }
}

/// Consumer half: reads from the ring (control thread for inbound,
/// RT audio thread for outbound).
///
/// # Safety
///
/// Only one consumer may exist per ring at a time.
#[derive(Debug)]
pub struct Consumer<'a, const N: usize, const B: usize> {
    ring: &'a Ring<N, B>,
    /// Cached read index for the fast path.
    cached_read: usize,
    /// Cached write index (updated on empty-ring check).
    cached_write: usize,
}

impl<'a, const N: usize, const B: usize> Consumer<'a, N, B> {
    /// Borrow a ring for reading. The caller must ensure no other
    /// consumer exists concurrently.
    ///
    /// # Safety
    ///
    /// SPSC discipline: only one `Consumer` per ring.
    pub unsafe fn new(ring: &'a Ring<N, B>) -> Self {
        let cached_read = ring.read_idx.load(Ordering::Acquire);
        let cached_write = ring.write_idx.load(Ordering::Acquire);
        Self {
            ring,
            cached_read,
            cached_write,
        }
    }

    /// Look at the next slot without consuming it. Returns `Some(&[u8])` on
    /// success — a slice of exactly the pushed payload length (the producer
    /// records it per slot; trailing `0x00` bytes are preserved). Returns
    /// `None` if empty.
    ///
    /// The returned slice stays valid until [`Self::commit`]: the slot is
    /// published as consumed only by `commit`, so the producer can never
    /// overwrite data the reader is still looking at.
    #[inline]
    pub fn peek(&mut self) -> Option<&[u8]> {
        if self.cached_read == self.cached_write {
            // Refresh cached write index.
            self.cached_write = self.ring.write_idx.load(Ordering::Acquire);
            if self.cached_read == self.cached_write {
                return None;
            }
        }

        let slot = self.cached_read % N;
        let buf_ptr = self.ring.buf.get() as *const u8;
        let len_ptr = self.ring.lens.get() as *const u32;
        // SAFETY: the producer cannot write this slot while read_idx still
        // points at it (it still counts as pending until commit). The length
        // was written before the producer published write_idx (Release) and
        // we refreshed cached_write with an Acquire load, so it is visible.
        let len = unsafe { (*len_ptr.add(slot)) as usize }.min(B);
        let data: &[u8] = unsafe { core::slice::from_raw_parts(buf_ptr.add(slot * B), len) };
        Some(data)
    }

    /// Consume the slot named by the last [`Self::peek`]. Publishes the read
    /// index so the producer may reuse the slot. Call only after the data
    /// from `peek` has been fully used (copied out / forwarded).
    #[inline]
    pub fn commit(&mut self) {
        self.cached_read = self.cached_read.wrapping_add(1);
        self.ring
            .read_idx
            .store(self.cached_read, Ordering::Release);
    }

    /// Convenience: peek + copy to `out` + commit. Copies `min(payload,
    /// out.len())` bytes (longer payloads are truncated) and returns the
    /// number of bytes copied, or `None` if the ring is empty.
    #[inline]
    pub fn pop_into(&mut self, out: &mut [u8]) -> Option<usize> {
        let data = self.peek()?;
        let n = data.len().min(out.len());
        out[..n].copy_from_slice(&data[..n]);
        self.commit();
        Some(n)
    }

    /// Number of pending slots from the consumer's cached view.
    #[inline]
    pub fn pending(&self) -> usize {
        self.cached_write.wrapping_sub(self.cached_read).min(N)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ring_is_empty() {
        let ring = Ring::<64, 512>::new();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.drops(), 0);
    }

    #[test]
    fn push_pop_single_slot() {
        let ring = Ring::<64, 512>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        let data = [0x41u8; 16];
        assert!(prod.push(&data));
        assert_eq!(prod.pending(), 1);

        let popped = cons.peek().unwrap();
        // Exactly the pushed bytes — no padding convention.
        assert_eq!(popped.len(), 16);
        assert_eq!(popped, &data[..]);
        cons.commit();
        assert!(cons.peek().is_none());
    }

    #[test]
    fn trailing_zero_bytes_are_preserved() {
        // Payloads may legitimately end in 0x00 (e.g. v2 Discovery with
        // output_path_id 0); the per-slot length — not "last non-zero byte" —
        // defines the payload.
        let ring = Ring::<4, 64>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        let data = [0xAA, 0x00, 0x00];
        assert!(prod.push(&data));
        let popped = cons.peek().unwrap();
        assert_eq!(popped, &data[..]);
        cons.commit();
    }

    #[test]
    fn pop_into_reports_copied_bytes() {
        let ring = Ring::<4, 64>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        assert_eq!(cons.pop_into(&mut [0u8; 8]), None); // empty
        assert!(prod.push(b"abcd"));
        let mut out = [0xFFu8; 2]; // smaller than the payload → truncation
        assert_eq!(cons.pop_into(&mut out), Some(2));
        assert_eq!(out, [b'a', b'b']);
    }

    #[test]
    fn push_pop_many_slots() {
        let ring = Ring::<64, 512>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        for i in 0u8..64 {
            let data = [i; 128];
            assert!(prod.push(&data));
        }

        for i in 0u8..64 {
            let popped = cons.peek().unwrap();
            assert_eq!(popped[0], i);
            assert_eq!(&popped[..128], &[i; 128]);
            cons.commit();
        }
        assert!(cons.peek().is_none());
    }

    #[test]
    fn overflow_increments_drop_counter() {
        let ring = Ring::<4, 512>::new();
        let mut prod = unsafe { Producer::new(&ring) };

        // Fill the ring (4 slots).
        for _ in 0..4 {
            assert!(prod.push(&[0xAA; 64]));
        }
        assert_eq!(ring.drops(), 0);

        // Overflow.
        assert!(!prod.push(&[0xBB; 64]));
        assert_eq!(ring.drops(), 1);

        assert!(!prod.push(&[0xCC; 64]));
        assert_eq!(ring.drops(), 2);
    }

    #[test]
    fn reset_drops_clears_counter() {
        let ring = Ring::<4, 512>::new();
        let mut prod = unsafe { Producer::new(&ring) };

        for _ in 0..4 {
            assert!(prod.push(&[0; 64]));
        }
        assert!(!prod.push(&[0; 64]));
        assert_eq!(ring.drops(), 1);

        ring.reset_drops();
        assert_eq!(ring.drops(), 0);
    }

    #[test]
    fn tulip_small_ring() {
        // 2-slot ring: push-pop-push-pop interleaved.
        let ring = Ring::<2, 64>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        assert!(prod.push(b"hello"));
        assert!(prod.push(b"world"));
        assert!(!prod.push(b"full"));

        let p1 = cons.peek().unwrap();
        assert_eq!(&p1[..5], b"hello");
        cons.commit();
        // Now one slot free.
        assert!(prod.push(b"again"));

        let p2 = cons.peek().unwrap();
        assert_eq!(&p2[..5], b"world");
        cons.commit();
        let p3 = cons.peek().unwrap();
        assert_eq!(&p3[..5], b"again");
        cons.commit();
        assert!(cons.peek().is_none());
    }

    #[test]
    fn tunable_geometry() {
        // Ring<16, 256> works.
        let ring = Ring::<16, 256>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        let data = [0x7F; 200];
        assert!(prod.push(&data));
        let popped = cons.peek().unwrap();
        assert_eq!(&popped[..200], &data[..]);
        cons.commit();
    }

    /// Overflow with every slot-size from 1..B, ensure drop counter correct.
    #[test]
    fn overflow_flood() {
        let ring = Ring::<8, 512>::new();
        let mut prod = unsafe { Producer::new(&ring) };

        // Fill.
        for _ in 0..8 {
            assert!(prod.push(&[0xFF; 512]));
        }
        // Flood 100 more.
        for _ in 0..100 {
            prod.push(&[0x00; 512]);
        }
        assert_eq!(ring.drops(), 100);
    }

    /// Test that the ring wraps correctly after many iterations.
    #[test]
    fn wrap_around() {
        let ring = Ring::<4, 64>::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        // Push 100 items through a 4-slot ring; every popped value must be
        // the next one in sequence (FIFO order preserved across wrap).
        let mut next = 0u8;
        for i in 0u8..100 {
            let data = [i; 32];
            while !prod.push(&data) {
                let popped = cons.peek().expect("full ring must pop a slot");
                assert_eq!(popped[0], next);
                cons.commit();
                next += 1;
            }
        }
        // Drain remaining.
        while let Some(data) = cons.peek() {
            assert_eq!(data[0], next);
            next += 1;
            cons.commit();
        }
        assert_eq!(next, 100);
    }
}

// ---------------------------------------------------------------------------
// Miri tests — run with: cargo +nightly miri test -p midici-transport-clap
// ---------------------------------------------------------------------------

#[cfg(all(test, miri))]
mod miri_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Two-thread SPSC: producer in one thread, consumer in another.
    /// Miri verifies no data races on the atomic indices.
    #[test]
    fn miri_spsc_two_threads() {
        let ring = Arc::new(Ring::<64, 512>::new());

        let r_prod = Arc::clone(&ring);
        let r_cons = Arc::clone(&ring);

        let producer = thread::spawn(move || {
            let mut prod = unsafe { Producer::new(&r_prod) };
            for i in 0u8..100 {
                let data = [i; 128];
                while !prod.push(&data) {
                    thread::yield_now();
                }
            }
        });

        let consumer = thread::spawn(move || {
            let mut cons = unsafe { Consumer::new(&r_cons) };
            let mut count = 0u8;
            while count < 100 {
                if let Some(data) = cons.peek() {
                    assert_eq!(data[0], count);
                    count += 1;
                    cons.commit();
                } else {
                    thread::yield_now();
                }
            }
        });

        producer.join().unwrap();
        consumer.join().unwrap();
    }

    /// Overflow path: producer floods full ring, drop counter advances.
    #[test]
    fn miri_overflow_counter() {
        let ring = Arc::new(Ring::<4, 512>::new());

        let r_prod = Arc::clone(&ring);
        let r_cons = Arc::clone(&ring);

        // Fill quickly.
        {
            let mut prod = unsafe { Producer::new(&r_prod) };
            for _ in 0..4 {
                assert!(prod.push(&[0; 64]));
            }
            // Now full — these drop.
            for _ in 0..10 {
                assert!(!prod.push(&[1; 64]));
            }
        }

        assert_eq!(r_cons.drops(), 10);

        // Drain and verify.
        {
            let mut cons = unsafe { Consumer::new(&r_cons) };
            for _ in 0..4 {
                assert!(cons.peek().is_some());
                cons.commit();
            }
            assert!(cons.peek().is_none());
        }
    }
}
