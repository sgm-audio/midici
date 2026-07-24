//! PE session controller: Caps negotiation, Get pipeline, reassembly caps.
//! // ARD §4 / §5 / §7; M2-101 §8.5–§8.8; M2-103 §7.4

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use midici_core::spec::{
    is_pe_sub_id, DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2,
    PE_DEFAULT_SIMULTANEOUS_REQUESTS, PE_VERSION_MAJOR, PE_VERSION_MINOR, SUB_ID2_PE_CAPS_INQUIRY,
    SUB_ID2_PE_CAPS_REPLY, SUB_ID2_PE_GET_INQUIRY, SUB_ID2_PE_GET_REPLY,
};
use midici_core::{
    pe_caps_reply, CiError, CiHeader, Muid, OutboundSysex, PeCapabilities, PeGetMessage,
};

use crate::chunker::split;
use crate::frame::PeChunk;
use crate::json_header::{encode_reply_header, parse_get_inquiry_header, GetInquiryHeader};
use crate::reassembler::{ReassembleEvent, Reassembler, MAX_TX_BYTES};
use crate::registry::ResourceRegistry;
use crate::resource::PeQuery;
use crate::status::PeStatus;

const OUT_CAP: usize = 64;

#[derive(Clone, Debug)]
struct PeerPe {
    muid: Muid,
    simultaneous: u8,
    pe_major: u8,
    pe_minor: u8,
    /// Active inbound request ids awaiting completion (incomplete multi-chunk).
    active: Vec<u8>,
}

/// PE-side state owned by [`crate::responder::ResponderEngine`].
pub struct PeController {
    pub registry: ResourceRegistry,
    /// Our advertised simultaneous requests.
    pub our_simultaneous: u8,
    reassembler: Reassembler,
    peers: Vec<PeerPe>,
    outbound: VecDeque<OutboundSysex>,
    /// Scratch encode buffer for full SysEx bodies (CI + PE).
    encode_buf: Vec<u8>,
}

impl PeController {
    pub fn new(registry: ResourceRegistry, max_peers: usize) -> Self {
        Self {
            registry,
            our_simultaneous: PE_DEFAULT_SIMULTANEOUS_REQUESTS,
            reassembler: Reassembler::new(max_peers.max(1)),
            peers: Vec::with_capacity(max_peers.max(1)),
            outbound: VecDeque::with_capacity(OUT_CAP),
            encode_buf: alloc::vec![0u8; 8192],
        }
    }

    pub fn next_outbound(&mut self) -> Option<OutboundSysex> {
        self.outbound.pop_front()
    }

    /// True if this body is a PE category message we should handle.
    pub fn is_pe_body(body: &[u8]) -> bool {
        if let Ok((h, _)) = CiHeader::decode(body) {
            is_pe_sub_id(h.sub_id2)
        } else {
            false
        }
    }

    pub fn feed(
        &mut self,
        our_muid: Muid,
        peer_max_sysex: u32,
        group: u8,
        body: &[u8],
        now_ms: u64,
    ) -> Result<(), CiError> {
        let (header, _) = CiHeader::decode(body)?;
        match header.sub_id2 {
            SUB_ID2_PE_CAPS_INQUIRY => self.on_caps_inquiry(our_muid, group, body),
            SUB_ID2_PE_CAPS_REPLY => self.on_caps_reply(body),
            SUB_ID2_PE_GET_INQUIRY => {
                self.on_get_inquiry(our_muid, peer_max_sysex, group, body, now_ms)
            }
            SUB_ID2_PE_GET_REPLY => Ok(()), // initiator path handled in tests
            _ => Ok(()),
        }
    }

    pub fn poll(&mut self, our_muid: Muid, peer_max_sysex: u32, group: u8, now_ms: u64) {
        let events = self.reassembler.poll(now_ms);
        for ev in events {
            if let ReassembleEvent::Timeout { peer, request_id } = ev {
                self.clear_active(peer, request_id);
                // Stalled multi-chunk → PE status 341. // ARD §7 / M2-103 §7.4.1
                let _ = self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    PeStatus::Unavailable,
                    &[],
                );
            }
        }
    }

    fn on_caps_inquiry(&mut self, our_muid: Muid, group: u8, body: &[u8]) -> Result<(), CiError> {
        let inq = PeCapabilities::decode(body)?;
        let negotiated = inq.simultaneous_requests.min(self.our_simultaneous).max(1);
        self.upsert_peer(inq.header.source, negotiated, inq.pe_major, inq.pe_minor);
        let reply = pe_caps_reply(our_muid, inq.header.source, self.our_simultaneous);
        self.queue_msg(group, &reply)?;
        Ok(())
    }

    fn on_caps_reply(&mut self, body: &[u8]) -> Result<(), CiError> {
        let rep = PeCapabilities::decode(body)?;
        let negotiated = rep.simultaneous_requests.min(self.our_simultaneous).max(1);
        self.upsert_peer(rep.header.source, negotiated, rep.pe_major, rep.pe_minor);
        Ok(())
    }

    fn on_get_inquiry(
        &mut self,
        our_muid: Muid,
        peer_max_sysex: u32,
        group: u8,
        body: &[u8],
        now_ms: u64,
    ) -> Result<(), CiError> {
        let msg = PeGetMessage::decode(body)?;
        let peer = msg.header.source;
        let chunk = match PeChunk::decode(&msg.pe_payload) {
            Ok(c) => c,
            Err(_) => {
                return self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    0,
                    peer_max_sysex,
                    PeStatus::BadRequest,
                    &[],
                );
            }
        };

        // Ensure peer PE record exists (default simultaneous if Caps skipped).
        if self.peer_mut(peer).is_none() {
            self.upsert_peer(
                peer,
                self.our_simultaneous,
                PE_VERSION_MAJOR,
                PE_VERSION_MINOR,
            );
        }

        // Cap concurrent requests before accepting. // ARD §7 → PeStatus::Busy (445)
        let is_new = self
            .peer_mut(peer)
            .map(|p| !p.active.contains(&chunk.request_id))
            .unwrap_or(true);
        if is_new {
            if let Some(p) = self.peer_mut(peer) {
                if p.active.len() as u8 >= p.simultaneous {
                    return self.queue_get_reply(
                        our_muid,
                        peer,
                        group,
                        chunk.request_id,
                        peer_max_sysex,
                        PeStatus::Busy,
                        &[],
                    );
                }
                p.active.push(chunk.request_id);
            }
        }

        match self.reassembler.feed(peer, &msg.pe_payload, now_ms) {
            Ok(None) => Ok(()),
            Ok(Some(ReassembleEvent::Complete {
                request_id,
                header,
                body: prop,
                ..
            })) => {
                self.clear_active(peer, request_id);
                self.complete_get(
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    &header,
                    &prop,
                )
            }
            Ok(Some(ReassembleEvent::Timeout { request_id, .. })) => {
                self.clear_active(peer, request_id);
                self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    PeStatus::Unavailable,
                    &[],
                )
            }
            Err(crate::error::PeError::Oversize) => {
                self.clear_active(peer, chunk.request_id);
                self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    chunk.request_id,
                    peer_max_sysex,
                    PeStatus::PayloadTooLarge,
                    &[],
                )
            }
            Err(crate::error::PeError::TooManyConcurrent) => {
                self.clear_active(peer, chunk.request_id);
                self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    chunk.request_id,
                    peer_max_sysex,
                    PeStatus::Busy,
                    &[],
                )
            }
            Err(_) => {
                self.clear_active(peer, chunk.request_id);
                self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    chunk.request_id,
                    peer_max_sysex,
                    PeStatus::BadRequest,
                    &[],
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_get(
        &mut self,
        our_muid: Muid,
        peer: Muid,
        group: u8,
        request_id: u8,
        peer_max_sysex: u32,
        header_bytes: &[u8],
        _prop: &[u8],
    ) -> Result<(), CiError> {
        let inquiry = match parse_get_inquiry_header(header_bytes) {
            Ok(h) => h,
            Err(st) => {
                return self.queue_get_reply(
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    st,
                    &[],
                );
            }
        };
        self.serve_get(our_muid, peer, group, request_id, peer_max_sysex, &inquiry)
    }

    fn serve_get(
        &mut self,
        our_muid: Muid,
        peer: Muid,
        group: u8,
        request_id: u8,
        peer_max_sysex: u32,
        inquiry: &GetInquiryHeader,
    ) -> Result<(), CiError> {
        let query = PeQuery {
            res_id: inquiry.res_id.clone(),
        };
        match self.registry.get(&inquiry.resource, &query) {
            Ok(payload) => {
                if payload.body.len() > MAX_TX_BYTES {
                    self.queue_get_reply(
                        our_muid,
                        peer,
                        group,
                        request_id,
                        peer_max_sysex,
                        PeStatus::PayloadTooLarge,
                        &[],
                    )
                } else {
                    self.queue_get_reply(
                        our_muid,
                        peer,
                        group,
                        request_id,
                        peer_max_sysex,
                        PeStatus::Ok,
                        &payload.body,
                    )
                }
            }
            Err(st) => {
                self.queue_get_reply(our_muid, peer, group, request_id, peer_max_sysex, st, &[])
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn queue_get_reply(
        &mut self,
        our_muid: Muid,
        dest: Muid,
        group: u8,
        request_id: u8,
        max_sysex: u32,
        status: PeStatus,
        property: &[u8],
    ) -> Result<(), CiError> {
        let header = encode_reply_header(status, None).map_err(|_| CiError::BadField)?;
        let chunks =
            split(request_id, &header, property, max_sysex).map_err(|_| CiError::BadField)?;
        for pe_payload in chunks {
            let msg = PeGetMessage {
                header: CiHeader {
                    device_id: DEVICE_ID_FUNCTION_BLOCK,
                    sub_id2: SUB_ID2_PE_GET_REPLY,
                    version: MESSAGE_FORMAT_VERSION_1_2,
                    source: our_muid,
                    dest,
                },
                pe_payload,
            };
            self.queue_pe_get(group, &msg)?;
        }
        Ok(())
    }

    fn queue_msg(&mut self, group: u8, msg: &PeCapabilities) -> Result<(), CiError> {
        let n = msg.encode(&mut self.encode_buf)?;
        let body = self.encode_buf[..n].to_vec();
        self.push_out(group, &body);
        Ok(())
    }

    fn queue_pe_get(&mut self, group: u8, msg: &PeGetMessage) -> Result<(), CiError> {
        let need = midici_core::spec::CI_HEADER_LEN + msg.pe_payload.len();
        if self.encode_buf.len() < need {
            self.encode_buf.resize(need, 0);
        }
        let n = msg.encode(&mut self.encode_buf)?;
        let body = self.encode_buf[..n].to_vec();
        self.push_out(group, &body);
        Ok(())
    }

    fn push_out(&mut self, group: u8, body: &[u8]) {
        if self.outbound.len() >= OUT_CAP {
            let _ = self.outbound.pop_front();
        }
        self.outbound.push_back(OutboundSysex {
            group,
            body: body.to_vec(),
        });
    }

    fn upsert_peer(&mut self, muid: Muid, simultaneous: u8, pe_major: u8, pe_minor: u8) {
        if let Some(p) = self.peers.iter_mut().find(|p| p.muid == muid) {
            p.simultaneous = simultaneous.max(1);
            p.pe_major = pe_major;
            p.pe_minor = pe_minor;
            return;
        }
        self.peers.push(PeerPe {
            muid,
            simultaneous: simultaneous.max(1),
            pe_major,
            pe_minor,
            active: Vec::new(),
        });
    }

    fn peer_mut(&mut self, muid: Muid) -> Option<&mut PeerPe> {
        self.peers.iter_mut().find(|p| p.muid == muid)
    }

    fn clear_active(&mut self, muid: Muid, request_id: u8) {
        if let Some(p) = self.peer_mut(muid) {
            p.active.retain(|id| *id != request_id);
        }
    }

    /// Negotiated simultaneous requests for a peer (for tests).
    pub fn simultaneous_for(&self, muid: Muid) -> Option<u8> {
        self.peers
            .iter()
            .find(|p| p.muid == muid)
            .map(|p| p.simultaneous)
    }
}
