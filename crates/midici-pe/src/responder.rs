//! Combined Management + PE responder façade. // ARD §3 / §4

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
