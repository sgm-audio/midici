//! Sans-io MIDI-CI engine: Discovery, peers, MUID collision, ACK/NAK. // ARD §3 / §4 / §7

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use rand_core::RngCore;

use crate::config::{CiConfig, PeerState};
use crate::error::CiError;
use crate::event::{CapFlags, CiEvent, DeviceIdentity, NakCode, OutboundSysex};
use crate::header::CiHeader;
use crate::mgmt::{mgmt_header, AckNakBody, Discovery, InvalidateMuid, Nak, ReplyToDiscovery};
use crate::muid::Muid;
use crate::spec::{
    message_format_version_reserved_set, CI_HEADER_LEN, DEVICE_ID_FUNCTION_BLOCK,
    MESSAGE_FORMAT_VERSION_1_2, MIN_RECEIVABLE_SYSEX_SIZE, SUB_ID1_MIDI_CI, SUB_ID2_ACK,
    SUB_ID2_DISCOVERY, SUB_ID2_ENDPOINT_INQUIRY, SUB_ID2_ENDPOINT_REPLY, SUB_ID2_INVALIDATE_MUID,
    SUB_ID2_NAK, SUB_ID2_REPLY_TO_DISCOVERY, UNIVERSAL_NON_REALTIME,
};
/// Pre-reserved outbound queue depth (control-plane).
const OUTBOUND_CAP: usize = 32;
/// Pre-reserved event queue depth.
const EVENT_CAP: usize = 32;
/// Encode scratch for management messages (Discovery/Reply/Invalidate/NAK fit well under 128).
const ENCODE_SCRATCH: usize = 256;

/// Sans-io MIDI-CI state machine. // ARD §3
///
/// No clocks, I/O, or threads. Monotonic time is injected via [`Self::poll`].
pub struct CiEngine<R: RngCore> {
    cfg: CiConfig,
    rng: R,
    our_muid: Muid,
    peers: Vec<PeerState>,
    outbound: VecDeque<OutboundSysex>,
    events: VecDeque<CiEvent>,
    /// Last `now` observed by [`Self::poll`] (monotonic millis).
    now_ms: u64,
    encode_buf: Vec<u8>,
}

impl<R: RngCore> CiEngine<R> {
    /// Construct an engine, generating an ordinary MUID from `rng`. // M2-101 §3.3.1
    pub fn new(cfg: CiConfig, mut rng: R) -> Self {
        let our_muid = Muid::generate(&mut rng);
        let max_peers = cfg.max_peers.max(1);
        let encode_buf = alloc::vec![0u8; ENCODE_SCRATCH];
        Self {
            cfg,
            rng,
            our_muid,
            peers: Vec::with_capacity(max_peers),
            outbound: VecDeque::with_capacity(OUTBOUND_CAP),
            events: VecDeque::with_capacity(EVENT_CAP),
            now_ms: 0,
            encode_buf,
        }
    }

    /// Our current MUID.
    #[inline]
    pub fn muid(&self) -> Muid {
        self.our_muid
    }

    /// Borrow the peer table.
    #[inline]
    pub fn peers(&self) -> &[PeerState] {
        &self.peers
    }

    /// Borrow configuration.
    #[inline]
    pub fn config(&self) -> &CiConfig {
        &self.cfg
    }

    /// Feed one complete inbound SysEx7 body (F0/F7 stripped). // ARD §3
    pub fn feed_sysex(&mut self, group: u8, body: &[u8]) -> Result<(), CiError> {
        // Non-CI SysEx: silent drop. // M2-101 §5.2.1 Table 5 (only 7E/0D are MIDI-CI)
        if body.is_empty() {
            return Ok(());
        }
        if body[0] != UNIVERSAL_NON_REALTIME {
            return Ok(());
        }
        if body.len() < 3 || body[2] != SUB_ID1_MIDI_CI {
            return Ok(());
        }
        if body.len() < CI_HEADER_LEN {
            // Cannot address a NAK without source/dest MUIDs.
            // Silent drop. // M2-101 §5.2.1 (header incomplete)
            return Ok(());
        }

        let (header, _) = match CiHeader::decode(body) {
            Ok(h) => h,
            Err(CiError::NotUniversalSysex) | Err(CiError::NotMidiCi) => return Ok(()),
            Err(CiError::Truncated) => return Ok(()),
            Err(e) => return Err(e),
        };

        // Reserved Message Format Version bits → NAK 0x02. // M2-101 §5.3 / §5.4
        if message_format_version_reserved_set(header.version) {
            if self.is_addressed_to_us(header.dest) {
                self.queue_nak(
                    group,
                    header.source,
                    header.sub_id2,
                    NakCode::VersionNotSupported,
                );
            }
            return Ok(());
        }

        // Not addressed to us (and not broadcast): silent drop.
        // // M2-101 §5.2.1 Destination MUID ("Device intended to receive this message")
        if !self.is_addressed_to_us(header.dest) {
            return Ok(());
        }

        match header.sub_id2 {
            SUB_ID2_DISCOVERY => self.on_discovery(group, body),
            SUB_ID2_REPLY_TO_DISCOVERY => self.on_reply_to_discovery(group, body),
            SUB_ID2_INVALIDATE_MUID => self.on_invalidate(group, body),
            SUB_ID2_NAK => self.on_nak(body),
            SUB_ID2_ACK => self.on_ack(body),
            SUB_ID2_ENDPOINT_INQUIRY => self.on_endpoint_inquiry(group, body),
            SUB_ID2_ENDPOINT_REPLY => Ok(()), // accept silently for now
            other => {
                // Unsupported MIDI-CI message → NAK 0x01. // M2-101 §5.11 / Table 16
                self.queue_nak(group, header.source, other, NakCode::NotSupported);
                Ok(())
            }
        }
    }

    /// Drive timeouts. `now` is monotonic millis. // ARD §3
    pub fn poll(&mut self, now: u64) {
        self.now_ms = now;
        // Management-only phase: no PE inactivity timers yet.
        let _ = self.now_ms;
    }

    /// Drain one outbound SysEx body. // ARD §3
    pub fn next_outbound(&mut self) -> Option<OutboundSysex> {
        self.outbound.pop_front()
    }

    /// Drain one application-facing event. // ARD §3
    pub fn next_event(&mut self) -> Option<CiEvent> {
        self.events.pop_front()
    }

    /// Initiate (or re-announce) Discovery to Broadcast. // M2-101 §5.5 / §5.9
    pub fn announce_discovery(&mut self) {
        let msg = Discovery {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2: SUB_ID2_DISCOVERY,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source: self.our_muid,
                dest: Muid::BROADCAST,
            },
            manufacturer: self.cfg.identity.manufacturer,
            family: self.cfg.identity.family,
            model: self.cfg.identity.model,
            software_revision: self.cfg.identity.software_revision,
            category_supported: self.cfg.category_supported.bits(),
            max_sysex_size: self.cfg.max_sysex_size.max(MIN_RECEIVABLE_SYSEX_SIZE),
            output_path_id: self.cfg.output_path_id,
        };
        self.queue_encoded(self.cfg.local_group, &msg);
    }

    fn is_addressed_to_us(&self, dest: Muid) -> bool {
        dest.is_broadcast() || dest == self.our_muid
    }

    fn on_discovery(&mut self, group: u8, body: &[u8]) -> Result<(), CiError> {
        let disc = match Discovery::decode(body) {
            Ok(d) => d,
            Err(_) => {
                // Malformed MIDI-CI message → NAK 0x41 when we can address the peer.
                // // M2-101 §5.11 / Table 16 (0x41 Message was malformed)
                if let Ok((h, _)) = CiHeader::decode(body) {
                    self.queue_nak(group, h.source, SUB_ID2_DISCOVERY, NakCode::Malformed);
                }
                return Ok(());
            }
        };

        // Collision: Discovery Source MUID equals ours. // M2-101 §5.9.1 Option B / ARD §7
        if disc.header.source == self.our_muid {
            self.resolve_collision(group);
            return Ok(());
        }

        let info = DeviceIdentity {
            manufacturer: disc.manufacturer,
            family: disc.family,
            model: disc.model,
            software_revision: disc.software_revision,
        };
        let caps = CapFlags(disc.category_supported);
        self.upsert_peer(PeerState::from_discovery(
            disc.header.source,
            info,
            caps,
            disc.header.version,
            disc.max_sysex_size,
            self.cfg.max_sysex_size,
            disc.output_path_id,
        ));
        self.push_event(CiEvent::PeerDiscovered {
            muid: disc.header.source,
            info,
            caps,
        });

        let reply = ReplyToDiscovery {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2: SUB_ID2_REPLY_TO_DISCOVERY,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source: self.our_muid,
                dest: disc.header.source,
            },
            manufacturer: self.cfg.identity.manufacturer,
            family: self.cfg.identity.family,
            model: self.cfg.identity.model,
            software_revision: self.cfg.identity.software_revision,
            category_supported: self.cfg.category_supported.bits(),
            max_sysex_size: self.cfg.max_sysex_size.max(MIN_RECEIVABLE_SYSEX_SIZE),
            output_path_id: disc.output_path_id, // echo // M2-101 §5.6.1
            function_block: self.cfg.function_block,
        };
        self.queue_encoded(group, &reply);
        Ok(())
    }

    fn on_reply_to_discovery(&mut self, group: u8, body: &[u8]) -> Result<(), CiError> {
        let reply = match ReplyToDiscovery::decode(body) {
            Ok(r) => r,
            Err(_) => {
                if let Ok((h, _)) = CiHeader::decode(body) {
                    self.queue_nak(
                        group,
                        h.source,
                        SUB_ID2_REPLY_TO_DISCOVERY,
                        NakCode::Malformed,
                    );
                }
                return Ok(());
            }
        };

        // Collision via Reply carrying our MUID as source. // ARD §7
        if reply.header.source == self.our_muid {
            self.resolve_collision(group);
            return Ok(());
        }

        let info = DeviceIdentity {
            manufacturer: reply.manufacturer,
            family: reply.family,
            model: reply.model,
            software_revision: reply.software_revision,
        };
        let caps = CapFlags(reply.category_supported);
        let mut peer = PeerState::from_discovery(
            reply.header.source,
            info,
            caps,
            reply.header.version,
            reply.max_sysex_size,
            self.cfg.max_sysex_size,
            reply.output_path_id,
        );
        peer.function_block = reply.function_block;
        self.upsert_peer(peer);
        self.push_event(CiEvent::PeerDiscovered {
            muid: reply.header.source,
            info,
            caps,
        });
        Ok(())
    }

    fn on_invalidate(&mut self, group: u8, body: &[u8]) -> Result<(), CiError> {
        let inv = match InvalidateMuid::decode(body) {
            Ok(i) => i,
            Err(_) => return Ok(()), // no reply to Invalidate // M2-101 §5.9
        };

        if inv.target == self.our_muid {
            // Our MUID invalidated: regenerate and re-announce. // M2-101 §5.9
            self.our_muid = Muid::generate(&mut self.rng);
            self.announce_discovery();
            let _ = group;
            return Ok(());
        }

        if let Some(idx) = self.peers.iter().position(|p| p.muid == inv.target) {
            let muid = self.peers[idx].muid;
            self.peers.swap_remove(idx);
            self.push_event(CiEvent::PeerInvalidated { muid });
        }
        Ok(())
    }

    fn on_nak(&mut self, body: &[u8]) -> Result<(), CiError> {
        let nak = match Nak::decode(body) {
            Ok(n) => n,
            Err(_) => return Ok(()),
        };
        self.push_event(CiEvent::Nak {
            peer: nak.header.source,
            original: nak.body.original_sub_id2,
            code: NakCode::from_u8(nak.body.status_code),
        });
        Ok(())
    }

    fn on_ack(&mut self, body: &[u8]) -> Result<(), CiError> {
        // Accept ACK bodies; application events for ACK deferred.
        let _ = crate::mgmt::Ack::decode(body);
        Ok(())
    }

    fn on_endpoint_inquiry(&mut self, group: u8, body: &[u8]) -> Result<(), CiError> {
        let inq = match crate::mgmt::EndpointInquiry::decode(body) {
            Ok(i) => i,
            Err(_) => {
                if let Ok((h, _)) = CiHeader::decode(body) {
                    self.queue_nak(
                        group,
                        h.source,
                        SUB_ID2_ENDPOINT_INQUIRY,
                        NakCode::Malformed,
                    );
                }
                return Ok(());
            }
        };
        // For Phase 3 management scope: NAK not supported for Endpoint
        // (Product Instance ID reply deferred).
        self.queue_nak(
            group,
            inq.header.source,
            SUB_ID2_ENDPOINT_INQUIRY,
            NakCode::NotSupported,
        );
        Ok(())
    }

    /// M2-101 §5.9.1 Option B + ARD §7: Invalidate → regenerate → re-announce.
    fn resolve_collision(&mut self, group: u8) {
        let old = self.our_muid;
        let inv = InvalidateMuid {
            header: mgmt_header(SUB_ID2_INVALIDATE_MUID, old, Muid::BROADCAST),
            target: old,
        };
        self.queue_encoded(group, &inv);
        self.our_muid = Muid::generate(&mut self.rng);
        // Peer table entries remain; they address us by old MUID until they see Invalidate.
        self.announce_discovery();
    }

    fn upsert_peer(&mut self, peer: PeerState) {
        if let Some(existing) = self.peers.iter_mut().find(|p| p.muid == peer.muid) {
            *existing = peer;
            return;
        }
        if self.peers.len() >= self.cfg.max_peers {
            // Evict oldest (index 0) to keep table bounded.
            let evicted = self.peers.remove(0);
            self.push_event(CiEvent::PeerInvalidated { muid: evicted.muid });
        }
        self.peers.push(peer);
    }

    fn queue_nak(&mut self, group: u8, dest: Muid, original_sub_id2: u8, code: NakCode) {
        // Never NAK on version alone for lower versions — only reserved bits. // ARD §4
        let nak = Nak {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2: SUB_ID2_NAK,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source: self.our_muid,
                dest,
            },
            body: AckNakBody {
                original_sub_id2,
                status_code: code.to_u8(),
                status_data: 0x00,
                details: [0; 5],
                message: &[],
            },
        };
        self.queue_encoded(group, &nak);
    }

    fn queue_encoded<E: EncodeTo>(&mut self, group: u8, msg: &E) {
        if self.outbound.len() >= OUTBOUND_CAP {
            let _ = self.outbound.pop_front();
        }
        let n = match msg.encode_to(&mut self.encode_buf) {
            Ok(n) => n,
            Err(_) => return,
        };
        let mut body = Vec::with_capacity(n);
        body.extend_from_slice(&self.encode_buf[..n]);
        self.outbound.push_back(OutboundSysex { group, body });
    }

    fn push_event(&mut self, ev: CiEvent) {
        if self.events.len() >= EVENT_CAP {
            let _ = self.events.pop_front();
        }
        self.events.push_back(ev);
    }

    /// Queue an ACK toward `dest` only if the peer's feature mask allows it.
    ///
    /// v1.1 peers never receive ACK (v1.2-only). // ARD §4 / §7 / M2-101 §5.10
    pub fn send_ack(&mut self, group: u8, dest: Muid, body: AckNakBody<'_>) -> Result<(), CiError> {
        if let Some(peer) = self.peers.iter().find(|p| p.muid == dest) {
            if !peer.allow_ack() {
                return Ok(());
            }
        } else {
            // Unknown peer: do not send v1.2-only ACK until version is known.
            return Ok(());
        }
        let ack = crate::mgmt::Ack {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2: SUB_ID2_ACK,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source: self.our_muid,
                dest,
            },
            body,
        };
        self.queue_encoded(group, &ack);
        Ok(())
    }

    /// Queue an Endpoint Inquiry toward `dest` only if the peer allows v1.2 messages.
    /// // ARD §4 / §7
    pub fn send_endpoint_inquiry(
        &mut self,
        group: u8,
        dest: Muid,
        status: u8,
    ) -> Result<(), CiError> {
        if let Some(peer) = self.peers.iter().find(|p| p.muid == dest) {
            if !peer.allow_endpoint() {
                return Ok(());
            }
        } else {
            return Ok(());
        }
        let msg = crate::mgmt::EndpointInquiry {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2: SUB_ID2_ENDPOINT_INQUIRY,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source: self.our_muid,
                dest,
            },
            status,
        };
        self.queue_encoded(group, &msg);
        Ok(())
    }
}

trait EncodeTo {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError>;
}

impl EncodeTo for Discovery {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError> {
        self.encode(out)
    }
}
impl EncodeTo for ReplyToDiscovery {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError> {
        self.encode(out)
    }
}
impl EncodeTo for InvalidateMuid {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError> {
        self.encode(out)
    }
}
impl EncodeTo for Nak<'_> {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError> {
        self.encode(out)
    }
}
impl EncodeTo for crate::mgmt::Ack<'_> {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError> {
        self.encode(out)
    }
}
impl EncodeTo for crate::mgmt::EndpointInquiry {
    fn encode_to(&self, out: &mut [u8]) -> Result<usize, CiError> {
        self.encode(out)
    }
}
