//! Minimal PE initiator shim for in-memory loopback tests (not a published API).
#![allow(dead_code)] // Shared helpers; each integration binary uses a subset.

use midici_core::spec::{
    CAP_PROPERTY_EXCHANGE, DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2,
    SUB_ID2_PE_GET_INQUIRY, SUB_ID2_PE_SET_INQUIRY, SUB_ID2_PE_SUBSCRIPTION,
    SUB_ID2_REPLY_TO_DISCOVERY,
};
use midici_core::{
    mgmt_header, pe_caps_inquiry, CiHeader, Discovery, InvalidateMuid, Muid, OutboundSysex,
    PeCapabilities, PeGetMessage, ReplyToDiscovery,
};
use midici_pe::{
    encode_inquiry_header, encode_set_inquiry_header, encode_subscription_header, split,
    GetReplyHeader, PeChunk, SubCommand, SubReplyHeader,
};
use rand_core::RngCore;

/// Test-only initiator that builds Discovery / PE Caps / Get inquiries.
pub struct TestInitiator {
    pub muid: Muid,
    pub max_sysex: u32,
    pub simultaneous: u8,
    next_request_id: u8,
}

impl TestInitiator {
    pub fn new(muid: Muid, max_sysex: u32) -> Self {
        Self {
            muid,
            max_sysex,
            simultaneous: 4,
            next_request_id: 1,
        }
    }

    pub fn discovery(&self) -> Vec<u8> {
        let msg = Discovery {
            header: mgmt_header(
                midici_core::spec::SUB_ID2_DISCOVERY,
                self.muid,
                Muid::BROADCAST,
            ),
            manufacturer: [0x7D, 0, 0],
            family: 1,
            model: 2,
            software_revision: [1, 0, 0, 0],
            category_supported: CAP_PROPERTY_EXCHANGE,
            max_sysex_size: self.max_sysex,
            output_path_id: 0,
        };
        let mut buf = vec![0u8; 64];
        let n = msg.encode(&mut buf).unwrap();
        buf.truncate(n);
        buf
    }

    pub fn pe_caps(&self, dest: Muid) -> Vec<u8> {
        let msg = pe_caps_inquiry(self.muid, dest, self.simultaneous);
        let mut buf = vec![0u8; 32];
        let n = msg.encode(&mut buf).unwrap();
        buf.truncate(n);
        buf
    }

    pub fn get(&mut self, dest: Muid, resource: &str) -> Vec<Vec<u8>> {
        let header = encode_inquiry_header(resource).unwrap();
        self.chunked(dest, SUB_ID2_PE_GET_INQUIRY, &header, &[])
    }

    /// Build an Inquiry: Set Property Data message (possibly multi-chunk).
    /// // M2-101 §8.9 / M2-103 §8.2
    pub fn set(&mut self, dest: Muid, resource: &str, body: &[u8]) -> Vec<Vec<u8>> {
        let header = encode_set_inquiry_header(resource, None, false).unwrap();
        self.chunked(dest, SUB_ID2_PE_SET_INQUIRY, &header, body)
    }

    /// Build a Subscription message ("start" with resource, or "end" with id).
    /// // M2-101 §8.11 / M2-103 §11.1
    pub fn subscribe_start(&mut self, dest: Muid, resource: &str) -> Vec<Vec<u8>> {
        let header =
            encode_subscription_header(SubCommand::Start, Some(resource), None, None).unwrap();
        self.chunked(dest, SUB_ID2_PE_SUBSCRIPTION, &header, &[])
    }

    pub fn subscribe_end(&mut self, dest: Muid, subscribe_id: &str) -> Vec<Vec<u8>> {
        let header = encode_subscription_header(
            SubCommand::End,
            None,
            Some(subscribe_id),
            Some("initiator"),
        )
        .unwrap();
        self.chunked(dest, SUB_ID2_PE_SUBSCRIPTION, &header, &[])
    }

    /// Build an Invalidate MUID message targeting `target`. // M2-101 §5.9
    pub fn invalidate(&self, target: Muid) -> Vec<u8> {
        let msg = InvalidateMuid {
            header: mgmt_header(
                midici_core::spec::SUB_ID2_INVALIDATE_MUID,
                self.muid,
                Muid::BROADCAST,
            ),
            target,
        };
        let mut buf = vec![0u8; 32];
        let n = msg.encode(&mut buf).unwrap();
        buf.truncate(n);
        buf
    }

    fn chunked(&mut self, dest: Muid, sub_id2: u8, header: &[u8], body: &[u8]) -> Vec<Vec<u8>> {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1) & 0x7F;
        if self.next_request_id == 0 {
            self.next_request_id = 1;
        }
        let chunks = split(request_id, header, body, self.max_sysex).unwrap();
        chunks
            .into_iter()
            .map(|pe_payload| {
                PeGetMessage {
                    header: CiHeader {
                        device_id: DEVICE_ID_FUNCTION_BLOCK,
                        sub_id2,
                        version: MESSAGE_FORMAT_VERSION_1_2,
                        source: self.muid,
                        dest,
                    },
                    pe_payload,
                }
                .to_vec()
                .unwrap()
            })
            .collect()
    }

    /// Build a partial multi-chunk Get (chunk 1 of 2) for stall tests.
    pub fn get_partial_multi(&mut self, dest: Muid, resource: &str) -> (u8, Vec<u8>) {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1) & 0x7F;
        if self.next_request_id == 0 {
            self.next_request_id = 1;
        }
        let header = encode_inquiry_header(resource).unwrap();
        // Force multi-chunk by claiming num_chunks=2 with empty property on chunk 1.
        let pe = PeChunk {
            request_id,
            header,
            num_chunks: 2,
            chunk_num: 1,
            property: Vec::new(),
        }
        .to_vec()
        .unwrap();
        let body = PeGetMessage {
            header: CiHeader {
                device_id: DEVICE_ID_FUNCTION_BLOCK,
                sub_id2: SUB_ID2_PE_GET_INQUIRY,
                version: MESSAGE_FORMAT_VERSION_1_2,
                source: self.muid,
                dest,
            },
            pe_payload: pe,
        }
        .to_vec()
        .unwrap();
        (request_id, body)
    }
}

pub fn parse_reply_to_discovery(body: &[u8]) -> Option<ReplyToDiscovery> {
    let (h, _) = midici_core::CiHeader::decode(body).ok()?;
    if h.sub_id2 != SUB_ID2_REPLY_TO_DISCOVERY {
        return None;
    }
    ReplyToDiscovery::decode(body).ok()
}

pub fn parse_pe_caps(body: &[u8]) -> Option<PeCapabilities> {
    PeCapabilities::decode(body).ok()
}

pub fn collect_get_reply_payload(chunks: &[OutboundSysex]) -> (GetReplyHeader, Vec<u8>) {
    let (header, body) = reassemble_reply_payload(chunks);
    let h: GetReplyHeader = serde_json::from_slice(&header).expect("reply header json");
    (h, body)
}

/// Reassemble a (possibly multi-chunk) PE reply/update into header+body bytes.
pub fn reassemble_reply_payload(chunks: &[OutboundSysex]) -> (Vec<u8>, Vec<u8>) {
    use midici_pe::Reassembler;
    let mut ra = Reassembler::new(1);
    let mut done = None;
    for (i, o) in chunks.iter().enumerate() {
        let msg = PeGetMessage::decode(&o.body).expect("pe reply");
        let peer = msg.header.source;
        if let Some(ev) = ra.feed(peer, &msg.pe_payload, i as u64 + 1).unwrap() {
            done = Some(ev);
        }
    }
    match done.expect("incomplete pe reply") {
        midici_pe::ReassembleEvent::Complete { header, body, .. } => (header, body),
        other => panic!("unexpected {other:?}"),
    }
}

/// Status of a Set reply (0x37). // M2-103 §7.4
pub fn set_reply_status(chunks: &[OutboundSysex]) -> u16 {
    let (header, _) = reassemble_reply_payload(chunks);
    let h: GetReplyHeader = serde_json::from_slice(&header).expect("set reply header");
    h.status
}

/// Parse a Reply to Subscription (0x39). // M2-103 §11.2
pub fn parse_sub_reply(chunks: &[OutboundSysex]) -> SubReplyHeader {
    let (header, body) = reassemble_reply_payload(chunks);
    assert!(body.is_empty(), "sub replies carry no property data");
    serde_json::from_slice(&header).expect("sub reply header")
}

/// Parse an outbound responder-originated Subscription update (0x38):
/// returns (command, subscribeId, property body). // M2-103 §11.1
pub fn parse_subscription_update(chunks: &[OutboundSysex]) -> (String, String, Vec<u8>) {
    for (i, o) in chunks.iter().enumerate() {
        let _ = i;
        let (h, _) = midici_core::CiHeader::decode(&o.body).expect("ci header");
        assert_eq!(
            h.sub_id2,
            midici_core::spec::SUB_ID2_PE_SUBSCRIPTION,
            "expected Subscription message (0x38)"
        );
    }
    let (header, body) = reassemble_reply_payload(chunks);
    let sh: midici_pe::SubscriptionHeader =
        serde_json::from_slice(&header).expect("subscription header");
    (
        serde_json::to_value(sh.command.expect("command"))
            .expect("command json")
            .as_str()
            .expect("command str")
            .to_string(),
        sh.subscribe_id.expect("subscribeId"),
        body,
    )
}

pub fn drain_all(eng: &mut midici_pe::ResponderEngine<impl RngCore>) -> Vec<OutboundSysex> {
    let mut v = Vec::new();
    while let Some(o) = eng.next_outbound() {
        v.push(o);
    }
    v
}
