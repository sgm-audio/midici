//! PE subscription table: SubId allocation, fan-out lookup, peer reaping.
//! // M2-103 §11 / §11.5; ARD §7 "subscription leak"
//!
//! ## Example
//!
//! ```
//! use midici_core::Muid;
//! use midici_pe::SubscriptionTable;
//!
//! let mut subs = SubscriptionTable::new();
//! let peer = Muid::ordinary(0x01020304).unwrap();
//! let id = subs.start(peer, 0, 512, "ChCtrlList", None).unwrap();
//! assert_eq!(id.as_str(), "00000001"); // deterministic, reusable after end
//! assert_eq!(subs.for_resource("ChCtrlList").count(), 1);
//! assert_eq!(subs.reap_peer(peer).len(), 1); // peer vanished → subscriptions end
//! assert!(subs.is_empty());
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use midici_core::Muid;

use crate::status::{PeResult, PeStatus};

/// Maximum concurrent subscriptions, all peers. // ARD §6 bounded tables
pub const MAX_SUBSCRIPTIONS: usize = 32;
/// Maximum concurrent subscriptions per peer MUID. // ARD §6 bounded tables
pub const MAX_SUBSCRIPTIONS_PER_PEER: usize = 8;
/// SubId wire length: exactly 8 chars of `0-9`/`a-z`/`_`. // M2-103 §11.1 Table 39
pub const SUB_ID_LEN: usize = 8;

/// Responder-assigned Subscription Id: 8 lowercase hex chars, allocated from a
/// monotonic counter and reusable after end. // M2-103 §11.1 Table 39
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubId([u8; SUB_ID_LEN]);

impl SubId {
    /// The 8 ASCII bytes of this id.
    pub fn as_bytes(&self) -> &[u8; SUB_ID_LEN] {
        &self.0
    }

    /// The id as `&str` (always valid ASCII).
    pub fn as_str(&self) -> &str {
        // Invariant: only hex chars are ever written by `alloc_id`.
        core::str::from_utf8(&self.0).unwrap_or("00000000")
    }
}

/// An active subscription to a subscribable resource. // M2-103 §11
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription {
    pub peer: Muid,
    /// UMP group the subscription arrived on (updates go back on it).
    pub group: u8,
    /// Negotiated max SysEx for chunking updates to this peer.
    pub peer_max_sysex: u32,
    pub resource: String,
    pub res_id: Option<String>,
    pub id: SubId,
}

/// Bounded subscription table tied to peer MUID liveness. // ARD §7
#[derive(Clone, Debug)]
pub struct SubscriptionTable {
    subs: Vec<Subscription>,
    next: u32,
}

impl SubscriptionTable {
    pub fn new() -> Self {
        Self {
            subs: Vec::with_capacity(MAX_SUBSCRIPTIONS),
            next: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.subs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    /// All subscriptions of one peer.
    pub fn of_peer(&self, peer: Muid) -> impl Iterator<Item = &Subscription> {
        self.subs.iter().filter(move |s| s.peer == peer)
    }

    /// All subscriptions watching `resource` — the Notify fan-out set.
    /// // M2-103 §11
    pub fn for_resource<'a>(&'a self, resource: &'a str) -> impl Iterator<Item = &'a Subscription> {
        self.subs.iter().filter(move |s| s.resource == resource)
    }

    /// Start a subscription. Caps are hard bounds; excess → [`PeStatus::Busy`].
    /// // ARD §7; M2-103 §11.1 (Responder assigns the Id)
    pub fn start(
        &mut self,
        peer: Muid,
        group: u8,
        peer_max_sysex: u32,
        resource: &str,
        res_id: Option<&str>,
    ) -> PeResult<SubId> {
        if self.subs.len() >= MAX_SUBSCRIPTIONS
            || self.of_peer(peer).count() >= MAX_SUBSCRIPTIONS_PER_PEER
        {
            return Err(PeStatus::Busy);
        }
        let id = self.alloc_id();
        self.subs.push(Subscription {
            peer,
            group,
            peer_max_sysex,
            resource: String::from(resource),
            res_id: res_id.map(String::from),
            id,
        });
        Ok(id)
    }

    /// End one subscription of `peer`. False when the id is unknown.
    pub fn end(&mut self, peer: Muid, id: &str) -> Option<Subscription> {
        let idx = self
            .subs
            .iter()
            .position(|s| s.peer == peer && s.id.as_str() == id)?;
        Some(self.subs.swap_remove(idx))
    }

    /// Reap every subscription of a vanished peer (Invalidate MUID or
    /// discovery/table eviction). // M2-103 §11.5; ARD §7
    pub fn reap_peer(&mut self, peer: Muid) -> Vec<Subscription> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.subs.len() {
            if self.subs[i].peer == peer {
                out.push(self.subs.swap_remove(i));
            } else {
                i += 1;
            }
        }
        out
    }

    /// Deterministic 8-char hex ids: "00000001", "00000002", …
    /// // M2-103 §11.1 (id charset `0-9`/`a-z`/`_`, max 8 chars)
    fn alloc_id(&mut self) -> SubId {
        let mut id = [b'0'; SUB_ID_LEN];
        let mut n = self.next;
        self.next = self.next.wrapping_add(1);
        let mut i = SUB_ID_LEN;
        while i > 0 {
            i -= 1;
            let d = (n & 0xF) as u8;
            id[i] = if d < 10 { b'0' + d } else { b'a' + (d - 10) };
            n >>= 4;
        }
        SubId(id)
    }
}

impl Default for SubscriptionTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(v: u8) -> Muid {
        Muid::ordinary(u32::from(v)).unwrap()
    }

    #[test]
    fn alloc_ids_are_deterministic_hex() {
        let mut t = SubscriptionTable::new();
        let a = t.start(peer(1), 0, 128, "X", None).unwrap();
        let b = t.start(peer(1), 0, 128, "X", None).unwrap();
        assert_eq!(a.as_str(), "00000001");
        assert_eq!(b.as_str(), "00000002");
    }

    #[test]
    fn global_and_per_peer_caps() {
        let mut t = SubscriptionTable::new();
        let p = peer(7);
        for _ in 0..MAX_SUBSCRIPTIONS_PER_PEER {
            t.start(p, 0, 128, "X", None).unwrap();
        }
        assert_eq!(t.start(p, 0, 128, "X", None), Err(PeStatus::Busy));
        // Another peer still fits globally.
        assert!(t.start(peer(8), 0, 128, "X", None).is_ok());
    }

    #[test]
    fn end_removes_only_matching_id() {
        let mut t = SubscriptionTable::new();
        let a = t.start(peer(1), 0, 128, "X", None).unwrap();
        assert!(t.end(peer(1), "ffffffff").is_none());
        assert!(t.end(peer(2), a.as_str()).is_none());
        assert!(t.end(peer(1), a.as_str()).is_some());
        assert!(t.is_empty());
    }

    #[test]
    fn reap_peer_drops_everything() {
        let mut t = SubscriptionTable::new();
        t.start(peer(1), 0, 128, "A", None).unwrap();
        t.start(peer(1), 0, 128, "B", None).unwrap();
        t.start(peer(2), 0, 128, "A", None).unwrap();
        let reaped = t.reap_peer(peer(1));
        assert_eq!(reaped.len(), 2);
        assert_eq!(t.len(), 1);
        assert_eq!(t.of_peer(peer(2)).count(), 1);
    }
}
