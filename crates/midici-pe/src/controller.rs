//! PE session controller: Caps negotiation, Get/Set/Subscription pipelines,
//! Notify fan-out, reassembly caps.
//! // ARD §4 / §5 / §7; M2-101 §8.5–§8.13; M2-103 §7.4, §11

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use midici_core::spec::{
    is_pe_sub_id, DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2,
    PE_DEFAULT_SIMULTANEOUS_REQUESTS, PE_VERSION_MAJOR, PE_VERSION_MINOR, SUB_ID2_PE_CAPS_INQUIRY,
    SUB_ID2_PE_CAPS_REPLY, SUB_ID2_PE_GET_INQUIRY, SUB_ID2_PE_GET_REPLY, SUB_ID2_PE_NOTIFY,
    SUB_ID2_PE_SET_INQUIRY, SUB_ID2_PE_SET_REPLY, SUB_ID2_PE_SUBSCRIPTION,
    SUB_ID2_PE_SUBSCRIPTION_REPLY,
};
use midici_core::{
    pe_caps_reply, CiError, CiHeader, Muid, OutboundSysex, PeCapabilities, PeMessage,
};

use crate::chunker::split;
use crate::frame::PeChunk;
use crate::json_header::{
    encode_reply_header, encode_sub_reply_header, encode_subscription_header,
    parse_get_inquiry_header, parse_notify_header, parse_set_inquiry_header,
    parse_subscription_header, GetInquiryHeader, SetInquiryHeader, SubCommand,
    NOTIFY_STATUS_TERMINATE,
};
use crate::reassembler::{ReassembleEvent, Reassembler, MAX_TX_BYTES};
use crate::registry::ResourceRegistry;
use crate::resource::PeQuery;
use crate::status::PeStatus;
use crate::subscriptions::{SubId, SubscriptionTable};

const OUT_CAP: usize = 64;
const EVENT_CAP: usize = 64;

/// Chunked PE inquiry kind (the three request/response pairs). // M2-101 §8.7–§8.12
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InquiryKind {
    Get,
    Set,
    Subscription,
}

impl InquiryKind {
    /// Reply Sub-ID#2 for this inquiry kind. // M2-101 §8.8/§8.10/§8.12
    pub const fn reply_sub_id2(self) -> u8 {
        match self {
            Self::Get => SUB_ID2_PE_GET_REPLY,
            Self::Set => SUB_ID2_PE_SET_REPLY,
            Self::Subscription => SUB_ID2_PE_SUBSCRIPTION_REPLY,
        }
    }
}

/// Body shape of an outbound subscription update. // M2-103 §11.1
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyBody<'a> {
    /// `command:"notify"`, no body — Initiator must re-Get. // M2-103 §11.1
    Notify,
    /// `command:"partial"` with a Partial-Set-shaped body. // M2-103 §11.1
    Partial(&'a [u8]),
    /// `command:"full"` with a full Get-Reply-shaped body. // M2-103 §11.1
    Full(&'a [u8]),
}

/// Application-facing PE events (drained via [`PeController::next_pe_event`]).
/// // ARD §3 (PropertySet / SubscribeStart / SubscribeEnd)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeEvent {
    /// A Set completed successfully (status 200); `body` is the accepted property data.
    PropertySet {
        peer: Muid,
        request_id: u8,
        resource: String,
        res_id: Option<String>,
        set_partial: bool,
        body: Vec<u8>,
    },
    /// An Initiator established a subscription.
    SubscribeStart {
        peer: Muid,
        resource: String,
        res_id: Option<String>,
        sub: SubId,
    },
    /// A subscription ended (Initiator "end", Responder end, or peer reaped).
    SubscribeEnd {
        peer: Muid,
        sub: SubId,
        resource: String,
    },
}

#[derive(Clone, Copy, Debug)]
struct ActiveTx {
    request_id: u8,
    kind: InquiryKind,
}

#[derive(Clone, Debug)]
struct PeerPe {
    muid: Muid,
    simultaneous: u8,
    pe_major: u8,
    pe_minor: u8,
    /// Active inbound transactions awaiting completion (incomplete multi-chunk).
    active: Vec<ActiveTx>,
}

/// PE-side state owned by [`crate::responder::ResponderEngine`].
pub struct PeController {
    pub registry: ResourceRegistry,
    /// Our advertised simultaneous requests.
    pub our_simultaneous: u8,
    reassembler: Reassembler,
    subs: SubscriptionTable,
    peers: Vec<PeerPe>,
    outbound: VecDeque<OutboundSysex>,
    events: VecDeque<PeEvent>,
    next_out_request_id: u8,
    /// Scratch encode buffer for full SysEx bodies (CI + PE).
    encode_buf: Vec<u8>,
}

impl PeController {
    pub fn new(registry: ResourceRegistry, max_peers: usize) -> Self {
        Self {
            registry,
            our_simultaneous: PE_DEFAULT_SIMULTANEOUS_REQUESTS,
            reassembler: Reassembler::new(max_peers.max(1)),
            subs: SubscriptionTable::new(),
            peers: Vec::with_capacity(max_peers.max(1)),
            outbound: VecDeque::with_capacity(OUT_CAP),
            events: VecDeque::with_capacity(EVENT_CAP),
            next_out_request_id: 1,
            encode_buf: alloc::vec![0u8; 8192],
        }
    }

    pub fn next_outbound(&mut self) -> Option<OutboundSysex> {
        self.outbound.pop_front()
    }

    /// Drain one PE application event (Set applied, subscription start/end).
    pub fn next_pe_event(&mut self) -> Option<PeEvent> {
        self.events.pop_front()
    }

    /// Read-only view of active subscriptions.
    pub fn subscriptions(&self) -> &SubscriptionTable {
        &self.subs
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
            SUB_ID2_PE_GET_INQUIRY => self.on_chunked_inquiry(
                InquiryKind::Get,
                our_muid,
                peer_max_sysex,
                group,
                body,
                now_ms,
            ),
            SUB_ID2_PE_SET_INQUIRY => self.on_chunked_inquiry(
                InquiryKind::Set,
                our_muid,
                peer_max_sysex,
                group,
                body,
                now_ms,
            ),
            SUB_ID2_PE_SUBSCRIPTION => self.on_chunked_inquiry(
                InquiryKind::Subscription,
                our_muid,
                peer_max_sysex,
                group,
                body,
                now_ms,
            ),
            // Replies carry the initiator side (v1 ships responder only; consumed in tests).
            SUB_ID2_PE_GET_REPLY | SUB_ID2_PE_SET_REPLY | SUB_ID2_PE_SUBSCRIPTION_REPLY => Ok(()),
            SUB_ID2_PE_NOTIFY => self.on_notify(body),
            _ => Ok(()),
        }
    }

    pub fn poll(&mut self, our_muid: Muid, peer_max_sysex: u32, group: u8, now_ms: u64) {
        let events = self.reassembler.poll(now_ms);
        for ev in events {
            if let ReassembleEvent::Timeout { peer, request_id } = ev {
                let kind = self
                    .take_active(peer, request_id)
                    .map(|tx| tx.kind)
                    .unwrap_or(InquiryKind::Get);
                // Stalled multi-chunk → PE status 341. // ARD §7 / M2-103 §7.4.1
                let _ = self.queue_reply_simple(
                    kind,
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    PeStatus::Unavailable,
                    None,
                    &[],
                );
            }
        }
    }

    /// Fan out a resource update to every subscribed peer via Subscription
    /// messages (`command` = notify/partial/full). Returns the number of
    /// peers messaged. Control-thread only. // M2-103 §11 / ARD §6
    pub fn notify_resource_changed(
        &mut self,
        our_muid: Muid,
        resource: &str,
        update: NotifyBody<'_>,
    ) -> usize {
        let (command, body): (SubCommand, &[u8]) = match update {
            NotifyBody::Notify => (SubCommand::Notify, &[]),
            NotifyBody::Partial(b) => (SubCommand::Partial, b),
            NotifyBody::Full(b) => (SubCommand::Full, b),
        };
        let targets: Vec<(Muid, u8, u32, SubId)> = self
            .subs
            .for_resource(resource)
            .map(|s| (s.peer, s.group, s.peer_max_sysex, s.id))
            .collect();
        let mut sent = 0;
        for (peer, group, max_sysex, sub) in targets {
            let header = match encode_subscription_header(command, None, Some(sub.as_str()), None) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let request_id = self.alloc_out_request_id();
            let chunks = match split(request_id, &header, body, max_sysex) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for pe_payload in chunks {
                // Re-allocation possible (multiple peers); keep simple: per-peer encode.
                let _ = self.queue_pe(group, SUB_ID2_PE_SUBSCRIPTION, our_muid, peer, &pe_payload);
            }
            sent += 1;
        }
        sent
    }

    /// Reap all PE state of a vanished peer: subscriptions (→ SubscribeEnd
    /// events), in-flight reassembly, negotiated caps row.
    /// // M2-103 §11.5 / ARD §7 "subscription leak"
    pub fn reap_peer(&mut self, peer: Muid) {
        for sub in self.subs.reap_peer(peer) {
            self.push_event(PeEvent::SubscribeEnd {
                peer,
                sub: sub.id,
                resource: sub.resource,
            });
        }
        let _ = self.reassembler.drop_peer(peer);
        if let Some(idx) = self.peers.iter().position(|p| p.muid == peer) {
            self.peers.remove(idx);
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

    fn on_chunked_inquiry(
        &mut self,
        kind: InquiryKind,
        our_muid: Muid,
        peer_max_sysex: u32,
        group: u8,
        body: &[u8],
        now_ms: u64,
    ) -> Result<(), CiError> {
        let msg = PeMessage::decode(body)?;
        let peer = msg.header.source;
        let chunk = match PeChunk::decode(&msg.pe_payload) {
            Ok(c) => c,
            Err(_) => {
                return self.queue_reply_simple(
                    kind,
                    our_muid,
                    peer,
                    group,
                    0,
                    peer_max_sysex,
                    PeStatus::BadRequest,
                    None,
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
            .map(|p| !p.active.iter().any(|t| t.request_id == chunk.request_id))
            .unwrap_or(true);
        if is_new {
            if let Some(p) = self.peer_mut(peer) {
                if p.active.len() as u8 >= p.simultaneous {
                    return self.queue_reply_simple(
                        kind,
                        our_muid,
                        peer,
                        group,
                        chunk.request_id,
                        peer_max_sysex,
                        PeStatus::Busy,
                        None,
                        &[],
                    );
                }
                p.active.push(ActiveTx {
                    request_id: chunk.request_id,
                    kind,
                });
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
                self.take_active(peer, request_id);
                self.complete_inquiry(
                    kind,
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
                self.take_active(peer, request_id);
                self.queue_reply_simple(
                    kind,
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    PeStatus::Unavailable,
                    None,
                    &[],
                )
            }
            Err(e) => {
                self.take_active(peer, chunk.request_id);
                // Peer table pressure ≠ per-peer busy (445). // M2-103 §7.4.1 status 341
                let st = match e {
                    crate::error::PeError::Oversize => PeStatus::PayloadTooLarge,
                    crate::error::PeError::TooManyConcurrent => PeStatus::Busy,
                    crate::error::PeError::PeerTableFull => PeStatus::Unavailable,
                    _ => PeStatus::BadRequest,
                };
                self.queue_reply_simple(
                    kind,
                    our_muid,
                    peer,
                    group,
                    chunk.request_id,
                    peer_max_sysex,
                    st,
                    None,
                    &[],
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_inquiry(
        &mut self,
        kind: InquiryKind,
        our_muid: Muid,
        peer: Muid,
        group: u8,
        request_id: u8,
        peer_max_sysex: u32,
        header_bytes: &[u8],
        prop: &[u8],
    ) -> Result<(), CiError> {
        match kind {
            InquiryKind::Get => {
                let inquiry = match parse_get_inquiry_header(header_bytes) {
                    Ok(h) => h,
                    Err(st) => {
                        return self.queue_reply_simple(
                            kind,
                            our_muid,
                            peer,
                            group,
                            request_id,
                            peer_max_sysex,
                            st,
                            None,
                            &[],
                        );
                    }
                };
                self.serve_get(our_muid, peer, group, request_id, peer_max_sysex, &inquiry)
            }
            InquiryKind::Set => self.complete_set(
                our_muid,
                peer,
                group,
                request_id,
                peer_max_sysex,
                header_bytes,
                prop,
            ),
            InquiryKind::Subscription => self.complete_subscription(
                our_muid,
                peer,
                group,
                request_id,
                peer_max_sysex,
                header_bytes,
            ),
        }
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
                    self.queue_reply_simple(
                        InquiryKind::Get,
                        our_muid,
                        peer,
                        group,
                        request_id,
                        peer_max_sysex,
                        PeStatus::PayloadTooLarge,
                        None,
                        &[],
                    )
                } else {
                    self.queue_reply_simple(
                        InquiryKind::Get,
                        our_muid,
                        peer,
                        group,
                        request_id,
                        peer_max_sysex,
                        PeStatus::Ok,
                        None,
                        &payload.body,
                    )
                }
            }
            Err(st) => self.queue_reply_simple(
                InquiryKind::Get,
                our_muid,
                peer,
                group,
                request_id,
                peer_max_sysex,
                st,
                None,
                &[],
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_set(
        &mut self,
        our_muid: Muid,
        peer: Muid,
        group: u8,
        request_id: u8,
        peer_max_sysex: u32,
        header_bytes: &[u8],
        prop: &[u8],
    ) -> Result<(), CiError> {
        let inquiry: SetInquiryHeader = match parse_set_inquiry_header(header_bytes) {
            Ok(h) => h,
            Err(st) => {
                return self.queue_reply_simple(
                    InquiryKind::Set,
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    st,
                    None,
                    &[],
                );
            }
        };
        let query = PeQuery {
            res_id: inquiry.res_id.clone(),
        };
        let status = self.registry.set(&inquiry.resource, &query, prop);
        let reply_status = match status {
            Ok(()) => PeStatus::Ok,
            Err(st) => st,
        };
        let r = self.queue_reply_simple(
            InquiryKind::Set,
            our_muid,
            peer,
            group,
            request_id,
            peer_max_sysex,
            reply_status,
            None,
            &[],
        );
        // Set succeeded → event + Notify fan-out (responder-originated "notify";
        // initiators re-Get). // M2-103 §11 (updates after Set) / §11.1
        if reply_status == PeStatus::Ok {
            self.push_event(PeEvent::PropertySet {
                peer,
                request_id,
                resource: inquiry.resource.clone(),
                res_id: inquiry.res_id.clone(),
                set_partial: inquiry.set_partial,
                body: prop.to_vec(),
            });
            self.notify_resource_changed(our_muid, &inquiry.resource, NotifyBody::Notify);
        }
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_subscription(
        &mut self,
        our_muid: Muid,
        peer: Muid,
        group: u8,
        request_id: u8,
        peer_max_sysex: u32,
        header_bytes: &[u8],
    ) -> Result<(), CiError> {
        let header = match parse_subscription_header(header_bytes) {
            Ok(h) => h,
            Err(st) => {
                return self.queue_reply_simple(
                    InquiryKind::Subscription,
                    our_muid,
                    peer,
                    group,
                    request_id,
                    peer_max_sysex,
                    st,
                    None,
                    &[],
                );
            }
        };
        let reply = |me: &mut Self, status: PeStatus, sub: Option<SubId>| {
            me.queue_reply_simple(
                InquiryKind::Subscription,
                our_muid,
                peer,
                group,
                request_id,
                peer_max_sysex,
                status,
                sub,
                &[],
            )
        };
        match header.command {
            Some(SubCommand::Start) => {
                let resource = match header.resource.as_deref() {
                    Some(r) if !r.is_empty() => r.to_string(),
                    _ => return reply(self, PeStatus::BadRequest, None),
                };
                if !self.registry.contains(&resource) {
                    return reply(self, PeStatus::NotFound, None);
                }
                if !self.registry.subscribable(&resource) {
                    return reply(self, PeStatus::NotAllowed, None);
                }
                match self.subs.start(
                    peer,
                    group,
                    peer_max_sysex,
                    &resource,
                    header.res_id.as_deref(),
                ) {
                    Ok(sub) => {
                        self.push_event(PeEvent::SubscribeStart {
                            peer,
                            resource: resource.clone(),
                            res_id: header.res_id.clone(),
                            sub,
                        });
                        reply(self, PeStatus::Ok, Some(sub))
                    }
                    Err(st) => reply(self, st, None),
                }
            }
            Some(SubCommand::End) => {
                let id = match header.subscribe_id.as_deref() {
                    Some(s) if !s.is_empty() => s,
                    _ => return reply(self, PeStatus::BadRequest, None),
                };
                match self.subs.end(peer, id) {
                    Some(sub) => {
                        self.push_event(PeEvent::SubscribeEnd {
                            peer,
                            sub: sub.id,
                            resource: sub.resource,
                        });
                        reply(self, PeStatus::Ok, Some(sub.id))
                    }
                    None => reply(self, PeStatus::NotFound, None),
                }
            }
            // Initiator shall not send updates via Subscription; use Set. // M2-103 §11.1
            Some(SubCommand::Partial) | Some(SubCommand::Full) | Some(SubCommand::Notify) => {
                reply(self, PeStatus::BadRequest, None)
            }
            None => reply(self, PeStatus::BadRequest, None),
        }
    }

    /// Legacy Notify (0x3F) — receive-only. // M2-103 §12; M2-101 §8.13
    fn on_notify(&mut self, body: &[u8]) -> Result<(), CiError> {
        let msg = match PeMessage::decode(body) {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };
        let chunk = match PeChunk::decode(&msg.pe_payload) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let header = match parse_notify_header(&chunk.header) {
            Ok(h) => h,
            Err(_) => return Ok(()),
        };
        if header.status == NOTIFY_STATUS_TERMINATE {
            // Terminate the inquiry for this Request ID immediately. // M2-103 §12.1.3
            let _ = self.reassembler.cancel(msg.header.source, chunk.request_id);
            self.take_active(msg.header.source, chunk.request_id);
        }
        // 100 (timeout wait) / 408 (timeout) are initiator-side niceties; the
        // responder side has no pending initiator transactions in v1.
        Ok(())
    }

    /// Queue a reply: header first (`{"status":…}`, optionally subscribeId).
    /// // M2-103 §7.1 first-property rule / §11.2
    #[allow(clippy::too_many_arguments)]
    fn queue_reply_simple(
        &mut self,
        kind: InquiryKind,
        our_muid: Muid,
        dest: Muid,
        group: u8,
        request_id: u8,
        max_sysex: u32,
        status: PeStatus,
        sub: Option<SubId>,
        property: &[u8],
    ) -> Result<(), CiError> {
        let header = match kind {
            InquiryKind::Subscription => {
                encode_sub_reply_header(status, sub.as_ref().map(SubId::as_str), None)
            }
            _ => encode_reply_header(status, None),
        }
        .map_err(|_| CiError::BadField)?;
        let chunks =
            split(request_id, &header, property, max_sysex).map_err(|_| CiError::BadField)?;
        for pe_payload in chunks {
            self.queue_pe(group, kind.reply_sub_id2(), our_muid, dest, &pe_payload)?;
        }
        Ok(())
    }

    fn queue_pe(
        &mut self,
        group: u8,
        sub_id2: u8,
        source: Muid,
        dest: Muid,
        pe_payload: &[u8],
    ) -> Result<(), CiError> {
        let msg = PeMessage {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source,
                dest,
            },
            pe_payload: pe_payload.to_vec(),
        };
        let need = midici_core::spec::CI_HEADER_LEN + msg.pe_payload.len();
        if self.encode_buf.len() < need {
            self.encode_buf.resize(need, 0);
        }
        let n = msg.encode(&mut self.encode_buf)?;
        let body = self.encode_buf[..n].to_vec();
        self.push_out(group, &body);
        Ok(())
    }

    fn queue_msg(&mut self, group: u8, msg: &PeCapabilities) -> Result<(), CiError> {
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

    fn push_event(&mut self, ev: PeEvent) {
        if self.events.len() >= EVENT_CAP {
            let _ = self.events.pop_front();
        }
        self.events.push_back(ev);
    }

    fn alloc_out_request_id(&mut self) -> u8 {
        let id = self.next_out_request_id;
        self.next_out_request_id = self.next_out_request_id.wrapping_add(1) & 0x7F;
        if self.next_out_request_id == 0 {
            self.next_out_request_id = 1;
        }
        id
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

    fn take_active(&mut self, muid: Muid, request_id: u8) -> Option<ActiveTx> {
        self.peer_mut(muid).and_then(|p| {
            p.active
                .iter()
                .position(|t| t.request_id == request_id)
                .map(|i| p.active.remove(i))
        })
    }

    /// Negotiated simultaneous requests for a peer (for tests).
    pub fn simultaneous_for(&self, muid: Muid) -> Option<u8> {
        self.peers
            .iter()
            .find(|p| p.muid == muid)
            .map(|p| p.simultaneous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inquiry_kind_reply_sub_ids() {
        // M2-101 Appendix E
        assert_eq!(InquiryKind::Get.reply_sub_id2(), 0x35);
        assert_eq!(InquiryKind::Set.reply_sub_id2(), 0x37);
        assert_eq!(InquiryKind::Subscription.reply_sub_id2(), 0x39);
    }
}
