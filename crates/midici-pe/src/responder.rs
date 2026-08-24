//! Combined Management + PE responder façade. // ARD §3 / §4

use rand_core::RngCore;

use midici_core::{CiConfig, CiEngine, CiError, CiEvent, Muid, OutboundSysex};

use crate::controller::PeController;
use crate::registry::ResourceRegistry;

/// Sans-io responder: [`CiEngine`] + PE Capabilities/Get pipeline.
pub struct ResponderEngine<R: RngCore> {
    pub ci: CiEngine<R>,
    pe: PeController,
    now_ms: u64,
}

impl<R: RngCore> ResponderEngine<R> {
    pub fn new(cfg: CiConfig, rng: R, registry: ResourceRegistry) -> Self {
        let max_peers = cfg.max_peers;
        let ci = CiEngine::new(cfg, rng);
        let pe = PeController::new(registry, max_peers);
        Self { ci, pe, now_ms: 0 }
    }

    pub fn muid(&self) -> Muid {
        self.ci.muid()
    }

    pub fn pe_mut(&mut self) -> &mut PeController {
        &mut self.pe
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
    }

    pub fn next_outbound(&mut self) -> Option<OutboundSysex> {
        self.ci.next_outbound().or_else(|| self.pe.next_outbound())
    }

    pub fn next_event(&mut self) -> Option<CiEvent> {
        self.ci.next_event()
    }
}
