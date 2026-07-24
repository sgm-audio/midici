//! Incremental PE chunk reassembler with DoS caps. // ARD §6 / §7; M2-101 §8.3
//!
//! # Memory model
//!
//! [`Reassembler`] pre-reserves all heap storage at construction time. For
//! `max_peers` peers it owns:
//!
//! - `max_peers` peer records
//! - each peer: [`MAX_CONCURRENT_PER_PEER`] (4) transaction slots
//! - each slot:
//!   - `header` [`Vec`] with capacity [`MAX_HEADER_BYTES`]
//!   - `arena` [`Vec`] with capacity [`MAX_TX_BYTES`] (64 KiB) holding packed
//!     property fragment bytes for out-of-order arrival
//!   - `index` [`Vec`] of fragment descriptors with capacity [`MAX_FRAGMENT_RECORDS`]
//!
//! The feed path never grows these capacities. It only writes into reserved
//! space (`Vec` length changes, capacity stays). Oversize property totals are
//! rejected with [`PeError::Oversize`] before writing. A fifth concurrent
//! transaction for a peer is rejected with [`PeError::TooManyConcurrent`]
//! (no eviction to make room — LRU applies to inactivity timeout reclaim).
//!
//! On [`ReassembleEvent::Timeout`] or successful completion the slot is cleared
//! (`clear` sets lengths to 0) and returned to the free list; capacities remain
//! reserved so the next transaction reuses the same buffers. [`MemoryStats`]
//! exposes reserved vs active byte counts for the allocator-counting harness.
//!
//! Inactivity: if no chunk arrives for [`INACTIVITY_TIMEOUT_MS`] (3 s) the
//! slot surfaces [`ReassembleEvent::Timeout`] (engine maps to NAK 341 in Phase 4).
//! // M2-103 §12.2

use alloc::vec::Vec;

use midici_core::Muid;

use crate::error::PeError;
use crate::frame::PeChunk;

/// Cap on reassembled property bytes per `(peer, requestId)`. // ARD §6
pub const MAX_TX_BYTES: usize = 64 * 1024;
/// Max concurrent incomplete transactions per peer. // ARD §6
pub const MAX_CONCURRENT_PER_PEER: usize = 4;
/// Inactivity timeout between chunks. // M2-103 §12.2 / ARD §7
pub const INACTIVITY_TIMEOUT_MS: u64 = 3_000;
/// Header capacity reserved per slot (largest first-chunk header at max SysEx 4096).
pub const MAX_HEADER_BYTES: usize = 4 * 1024;
/// Max stored fragment records per transaction (bounds OOO metadata).
pub const MAX_FRAGMENT_RECORDS: usize = 2_048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FragmentMeta {
    chunk_num: u16,
    offset: u32,
    len: u16,
}

struct TxSlot {
    active: bool,
    request_id: u8,
    last_activity_ms: u64,
    /// LRU stamp; larger means more recently used.
    lru_tick: u64,
    num_chunks: u16,
    header: Vec<u8>,
    header_set: bool,
    arena: Vec<u8>,
    index: Vec<FragmentMeta>,
    property_bytes: usize,
}

impl TxSlot {
    fn reserved(header_cap: usize, arena_cap: usize, index_cap: usize) -> Self {
        Self {
            active: false,
            request_id: 0,
            last_activity_ms: 0,
            lru_tick: 0,
            num_chunks: 0,
            header: Vec::with_capacity(header_cap),
            header_set: false,
            arena: Vec::with_capacity(arena_cap),
            index: Vec::with_capacity(index_cap),
            property_bytes: 0,
        }
    }

    fn clear(&mut self) {
        self.active = false;
        self.request_id = 0;
        self.last_activity_ms = 0;
        self.num_chunks = 0;
        self.header.clear();
        self.header_set = false;
        self.arena.clear();
        self.index.clear();
        self.property_bytes = 0;
    }

    fn reserved_bytes(&self) -> usize {
        self.header.capacity()
            + self.arena.capacity()
            + self.index.capacity() * core::mem::size_of::<FragmentMeta>()
    }

    fn active_bytes(&self) -> usize {
        if !self.active {
            return 0;
        }
        self.header.len()
            + self.arena.len()
            + self.index.len() * core::mem::size_of::<FragmentMeta>()
    }
}

struct PeerState {
    bound: bool,
    muid: Muid,
    slots: [TxSlot; MAX_CONCURRENT_PER_PEER],
}

/// Events produced by the reassembler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReassembleEvent {
    /// All chunks present; header + concatenated property body in chunk order.
    Complete {
        peer: Muid,
        request_id: u8,
        header: Vec<u8>,
        body: Vec<u8>,
    },
    /// Inactivity timeout; slot reclaimed. // ARD §7 "stalled tx"
    Timeout { peer: Muid, request_id: u8 },
}

/// Snapshot of pre-reserved vs in-use buffer accounting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryStats {
    pub reserved_bytes: usize,
    pub active_bytes: usize,
    pub active_slots: usize,
}

/// Incremental reassembler keyed by `(peer MUID, requestId)`.
pub struct Reassembler {
    peers: Vec<PeerState>,
    lru_clock: u64,
    reserved_bytes: usize,
}

impl Reassembler {
    /// Construct a reassembler that can track up to `max_peers` distinct peer MUIDs.
    ///
    /// All slot buffers are allocated here; see the module-level memory model.
    pub fn new(max_peers: usize) -> Self {
        let mut peers = Vec::with_capacity(max_peers);
        let mut reserved_bytes = 0usize;
        for _ in 0..max_peers {
            let slots = [
                TxSlot::reserved(MAX_HEADER_BYTES, MAX_TX_BYTES, MAX_FRAGMENT_RECORDS),
                TxSlot::reserved(MAX_HEADER_BYTES, MAX_TX_BYTES, MAX_FRAGMENT_RECORDS),
                TxSlot::reserved(MAX_HEADER_BYTES, MAX_TX_BYTES, MAX_FRAGMENT_RECORDS),
                TxSlot::reserved(MAX_HEADER_BYTES, MAX_TX_BYTES, MAX_FRAGMENT_RECORDS),
            ];
            for s in &slots {
                reserved_bytes = reserved_bytes.saturating_add(s.reserved_bytes());
            }
            peers.push(PeerState {
                bound: false,
                muid: Muid::BROADCAST,
                slots,
            });
        }
        Self {
            peers,
            lru_clock: 1,
            reserved_bytes,
        }
    }

    /// Pre-reserved buffer bytes (stable after construction).
    #[inline]
    pub fn reserved_bytes(&self) -> usize {
        self.reserved_bytes
    }

    /// Accounting snapshot for the allocator-counting harness.
    pub fn memory_stats(&self) -> MemoryStats {
        let mut active_bytes = 0usize;
        let mut active_slots = 0usize;
        for peer in &self.peers {
            for slot in &peer.slots {
                if slot.active {
                    active_slots += 1;
                    active_bytes = active_bytes.saturating_add(slot.active_bytes());
                }
            }
        }
        MemoryStats {
            reserved_bytes: self.reserved_bytes,
            active_bytes,
            active_slots,
        }
    }

    /// Feed one encoded PE chunk payload from `peer` at monotonic time `now_ms`.
    ///
    /// Returns `Ok(Some(Complete))` when the transaction finishes, `Ok(None)` if
    /// more chunks are needed, or `Err` on cap / framing violations.
    pub fn feed(
        &mut self,
        peer: Muid,
        raw_chunk: &[u8],
        now_ms: u64,
    ) -> Result<Option<ReassembleEvent>, PeError> {
        let chunk = PeChunk::decode(raw_chunk)?;
        let peer_idx = self.bind_peer(peer)?;
        let slot_idx = self.find_or_alloc_slot(peer_idx, chunk.request_id, now_ms)?;

        let tick = self.lru_clock.saturating_add(1);
        self.lru_clock = tick;
        let peer_muid = self.peers[peer_idx].muid;
        let slot = &mut self.peers[peer_idx].slots[slot_idx];
        slot.lru_tick = tick;
        slot.last_activity_ms = now_ms;

        if chunk.chunk_num == 1 {
            if chunk.header.len() > slot.header.capacity() {
                return Err(PeError::HeaderTooLarge);
            }
            if slot.header_set && slot.header.as_slice() != chunk.header.as_slice() {
                return Err(PeError::InconsistentChunking);
            }
            slot.header.clear();
            slot.header.extend_from_slice(&chunk.header);
            slot.header_set = true;
        } else if !chunk.header.is_empty() {
            return Err(PeError::InconsistentChunking);
        }

        if chunk.num_chunks != 0 {
            if slot.num_chunks == 0 {
                slot.num_chunks = chunk.num_chunks;
            } else if slot.num_chunks != chunk.num_chunks {
                return Err(PeError::InconsistentChunking);
            }
        }

        // Early-termination sentinel: treat as incomplete end → reject framing for now.
        // // M2-101 §8.3
        if chunk.chunk_num == 0 {
            slot.clear();
            return Err(PeError::InconsistentChunking);
        }

        if slot.index.iter().any(|f| f.chunk_num == chunk.chunk_num) {
            return Err(PeError::InconsistentChunking);
        }
        if slot.index.len() == slot.index.capacity() {
            return Err(PeError::Oversize);
        }

        let prop_len = chunk.property.len();
        if slot.property_bytes.saturating_add(prop_len) > MAX_TX_BYTES {
            slot.clear();
            return Err(PeError::Oversize);
        }
        if slot.arena.len().saturating_add(prop_len) > slot.arena.capacity() {
            slot.clear();
            return Err(PeError::Oversize);
        }

        let offset = slot.arena.len() as u32;
        slot.arena.extend_from_slice(&chunk.property);
        slot.index.push(FragmentMeta {
            chunk_num: chunk.chunk_num,
            offset,
            len: prop_len as u16,
        });
        slot.property_bytes += prop_len;

        // Unknown total: wait until a chunk declares num_chunks.
        if slot.num_chunks == 0 {
            return Ok(None);
        }

        if slot.index.len() < slot.num_chunks as usize {
            return Ok(None);
        }

        // Require every chunk_num in 1..=num_chunks exactly once.
        if !slot.header_set {
            return Ok(None);
        }
        for n in 1..=slot.num_chunks {
            if !slot.index.iter().any(|f| f.chunk_num == n) {
                return Ok(None);
            }
        }

        let body = assemble_body(slot)?;
        let header = slot.header.clone();
        let request_id = slot.request_id;
        slot.clear();

        Ok(Some(ReassembleEvent::Complete {
            peer: peer_muid,
            request_id,
            header,
            body,
        }))
    }

    /// Drive inactivity timeouts. Call from the engine poll loop.
    pub fn poll(&mut self, now_ms: u64) -> Vec<ReassembleEvent> {
        let mut events = Vec::new();
        // Collect timeouts in LRU order (oldest activity first).
        let mut timed_out: Vec<(usize, usize, u64, u8)> = Vec::new();
        for (pi, peer) in self.peers.iter().enumerate() {
            for (si, slot) in peer.slots.iter().enumerate() {
                if slot.active
                    && now_ms.saturating_sub(slot.last_activity_ms) >= INACTIVITY_TIMEOUT_MS
                {
                    timed_out.push((pi, si, slot.lru_tick, slot.request_id));
                }
            }
        }
        timed_out.sort_by_key(|t| t.2);
        for (pi, si, _, request_id) in timed_out {
            let peer = self.peers[pi].muid;
            self.peers[pi].slots[si].clear();
            events.push(ReassembleEvent::Timeout { peer, request_id });
        }
        events
    }

    fn bind_peer(&mut self, peer: Muid) -> Result<usize, PeError> {
        if let Some(i) = self.peers.iter().position(|p| p.bound && p.muid == peer) {
            return Ok(i);
        }
        if let Some(i) = self.peers.iter().position(|p| !p.bound) {
            self.peers[i].bound = true;
            self.peers[i].muid = peer;
            return Ok(i);
        }
        // Rebind an idle peer record when the table is full.
        if let Some(i) = self
            .peers
            .iter()
            .position(|p| p.slots.iter().all(|s| !s.active))
        {
            self.peers[i].muid = peer;
            self.peers[i].bound = true;
            return Ok(i);
        }
        Err(PeError::TooManyConcurrent)
    }

    fn find_or_alloc_slot(
        &mut self,
        peer_idx: usize,
        request_id: u8,
        now_ms: u64,
    ) -> Result<usize, PeError> {
        let peer = &mut self.peers[peer_idx];
        for (i, slot) in peer.slots.iter().enumerate() {
            if slot.active && slot.request_id == request_id {
                return Ok(i);
            }
        }
        for (i, slot) in peer.slots.iter_mut().enumerate() {
            if !slot.active {
                slot.active = true;
                slot.request_id = request_id;
                slot.last_activity_ms = now_ms;
                return Ok(i);
            }
        }
        Err(PeError::TooManyConcurrent)
    }
}

fn assemble_body(slot: &TxSlot) -> Result<Vec<u8>, PeError> {
    let mut metas = slot.index.clone();
    metas.sort_by_key(|m| m.chunk_num);
    let mut body = Vec::with_capacity(slot.property_bytes);
    for meta in metas {
        let start = meta.offset as usize;
        let end = start + meta.len as usize;
        if end > slot.arena.len() {
            return Err(PeError::BadField);
        }
        body.extend_from_slice(&slot.arena[start..end]);
    }
    Ok(body)
}
