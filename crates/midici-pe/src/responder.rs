//! Combined Management + PE responder façade. // ARD §3 / §4
//!
//! ## Example: complete Discovery → PE Caps → Get exchange (loopback)
//!
//! ```
//! use midici_core::spec::{CAP_PROPERTY_EXCHANGE, SUB_ID2_DISCOVERY};
//! use midici_core::{mgmt_header, CiConfig, DeviceIdentity, Discovery, Muid, PeMessage};
//! use midici_pe::{encode_inquiry_header, split, ResponderEngine, ResourceRegistry};
//!
//! struct Seeded(u32);
//! impl rand_core::RngCore for Seeded {
//!     fn next_u32(&mut self) -> u32 {
//!         self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
//!         self.0
//!     }
//!     fn next_u64(&mut self) -> u64 {
//!         (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
//!     }
//!     fn fill_bytes(&mut self, buf: &mut [u8]) {
//!         let mut n = 0;
//!         while n < buf.len() {
//!             let b = self.next_u32().to_le_bytes();
//!             let take = b.len().min(buf.len() - n);
//!             buf[n..n + take].copy_from_slice(&b[..take]);
//!             n += take;
//!         }
//!     }
//!     fn try_fill_bytes(&mut self, b: &mut [u8]) -> Result<(), rand_core::Error> {
//!         self.fill_bytes(b);
//!         Ok(())
//!     }
//! }
//!
//! let identity = DeviceIdentity {
//!     manufacturer: [0x7D, 0, 0],
//!     family: 1,
//!     model: 2,
//!     software_revision: [1, 0, 0, 0],
//! };
//! let registry = ResourceRegistry::with_device_info(identity);
//! let mut eng = ResponderEngine::new(CiConfig::responder_default(identity), Seeded(7), registry);
//!
//! // Peer broadcasts Discovery.
//! let peer = Muid::ordinary(0x01020304).unwrap();
//! let disc = Discovery {
//!     header: mgmt_header(SUB_ID2_DISCOVERY, peer, Muid::BROADCAST),
//!     manufacturer: [0x7D, 0, 0],
//!     family: 5,
//!     model: 6,
//!     software_revision: [1, 0, 0, 0],
//!     category_supported: CAP_PROPERTY_EXCHANGE,
//!     max_sysex_size: 512,
//!     output_path_id: 0,
//! };
//! let mut buf = [0u8; 64];
//! let n = disc.encode(&mut buf).unwrap();
//! eng.feed_sysex(0, &buf[..n]).unwrap();
//! assert!(eng.next_outbound().is_some()); // Reply to Discovery
//!
//! // Peer asks for DeviceInfo (single chunk fits easily).
//! let header = encode_inquiry_header("DeviceInfo").unwrap();
//! let chunks = split(1, &header, &[], 512).unwrap();
//! let msg = PeMessage {
//!     header: midici_core::CiHeader {
//!         device_id: 0x7F,
//!         sub_id2: midici_core::spec::SUB_ID2_PE_GET_INQUIRY,
//!         version: 0x02,
//!         source: peer,
//!         dest: eng.muid(),
//!     },
//!     pe_payload: chunks.into_iter().next().unwrap(),
//! }
//! .to_vec()
//! .unwrap();
//! eng.feed_sysex(0, &msg).unwrap();
//! let reply = eng.next_outbound().unwrap(); // Reply to Get, status 200
//! assert_eq!(reply.body[3], 0x35); // sub-ID#2: PE Get Reply
//! ```

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use rand_core::RngCore;

use midici_core::{CiConfig, CiEngine, CiError, CiEvent, Muid, OutboundSysex};

use crate::controller::{NotifyBody, PeController, PeEvent};
use crate::registry::ResourceRegistry;

/// Sans-io responder: [`CiEngine`] + PE Capabilities/Get/Set/Subscriptions.
pub struct ResponderEngine<R: RngCore> {
    pub ci: CiEngine<R>,
    pe: PeController,
    now_ms: u64,
    /// Peers seen on the last `poll`; disappearance ⇒ reap PE state (ARD §7).
    known_peers: Vec<Muid>,
    /// Management events already drained from `ci`, kept for the app.
    pending_events: VecDeque<CiEvent>,
}

impl<R: RngCore> ResponderEngine<R> {
    pub fn new(cfg: CiConfig, rng: R, registry: ResourceRegistry) -> Self {
        let max_peers = cfg.max_peers;
        let ci = CiEngine::new(cfg, rng);
        let pe = PeController::new(registry, max_peers);
        Self {
            ci,
            pe,
            now_ms: 0,
            known_peers: Vec::new(),
            pending_events: VecDeque::new(),
        }
    }

    pub fn muid(&self) -> Muid {
        self.ci.muid()
    }

    pub fn pe_mut(&mut self) -> &mut PeController {
        &mut self.pe
    }

    pub fn pe(&self) -> &PeController {
        &self.pe
    }

    pub fn announce_discovery(&mut self) {
        self.ci.announce_discovery();
    }

    pub fn feed_sysex(&mut self, group: u8, body: &[u8]) -> Result<(), CiError> {
        if PeController::is_pe_body(body) {
            let peer_max = midici_core::CiHeader::decode(body)
                .ok()
                .and_then(|(h, _)| {
                    self.ci
                        .peers()
                        .iter()
                        .find(|p| p.muid == h.source)
                        .map(|p| p.max_sysex)
                })
                .unwrap_or(self.ci.config().max_sysex_size);
            return self
                .pe
                .feed(self.ci.muid(), peer_max, group, body, self.now_ms);
        }
        self.ci.feed_sysex(group, body)
    }

    pub fn poll(&mut self, now: u64) {
        self.now_ms = now;
        self.ci.poll(now);
        let peer_max = self.ci.config().max_sysex_size;
        self.pe
            .poll(self.ci.muid(), peer_max, self.ci.config().local_group, now);
        // Intercept PeerInvalidated so PE state (subscriptions, in-flight
        // reassembly) is reaped at poll time, even if the app drains events
        // lazily. Events stay available via `next_event`. // M2-103 §11.5
        while let Some(ev) = self.ci.next_event() {
            if let CiEvent::PeerInvalidated { muid } = ev {
                self.pe.reap_peer(muid);
            }
            self.pending_events.push_back(ev);
        }
        self.reap_vanished_peers();
    }

    pub fn next_outbound(&mut self) -> Option<OutboundSysex> {
        self.ci.next_outbound().or_else(|| self.pe.next_outbound())
    }

    pub fn next_event(&mut self) -> Option<CiEvent> {
        self.pending_events.pop_front()
    }

    /// Drain PE-level events (PropertySet / SubscribeStart / SubscribeEnd).
    pub fn next_pe_event(&mut self) -> Option<PeEvent> {
        self.pe.next_pe_event()
    }

    /// Fan out a resource update to every subscribed peer (`Notify` fan-out).
    ///
    /// Control-thread only — call from the timer/flush path, never from an
    /// audio callback. // M2-103 §11; ARD §6
    pub fn notify_resource_changed(&mut self, resource: &str, update: NotifyBody<'_>) -> usize {
        self.pe
            .notify_resource_changed(self.ci.muid(), resource, update)
    }

    /// Subscriptions are bound to peer MUID liveness: any peer that
    /// disappeared from the CI table (Invalidate, eviction, discovery
    /// timeout) has its subscriptions reaped, without End messages.
    /// // M2-103 §11.5 / ARD §7 "subscription leak"
    fn reap_vanished_peers(&mut self) {
        for known in core::mem::take(&mut self.known_peers) {
            if !self.ci.peers().iter().any(|p| p.muid == known) {
                self.pe.reap_peer(known);
            }
        }
        self.known_peers
            .extend(self.ci.peers().iter().map(|p| p.muid));
    }
}
