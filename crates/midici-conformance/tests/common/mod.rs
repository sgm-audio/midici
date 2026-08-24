//! Minimal PE initiator shim for in-memory loopback tests (not a published API).
#![allow(dead_code)] // Shared helpers; each integration binary uses a subset.

use midici_core::spec::{
    CAP_PROPERTY_EXCHANGE, DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2,
    SUB_ID2_PE_GET_INQUIRY, SUB_ID2_REPLY_TO_DISCOVERY,
};
use midici_core::{
    mgmt_header, pe_caps_inquiry, CiHeader, Discovery, Muid, OutboundSysex, PeCapabilities,
    PeGetMessage, ReplyToDiscovery,
};
use midici_pe::{encode_inquiry_header, split, GetReplyHeader, PeChunk};
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
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1) & 0x7F;
        if self.next_request_id == 0 {
            self.next_request_id = 1;
        }
        let header = encode_inquiry_header(resource).unwrap();
        let chunks = split(request_id, &header, &[], self.max_sysex).unwrap();
        chunks
            .into_iter()
            .map(|pe_payload| {
                PeGetMessage {
                    header: CiHeader {
                        device_id: DEVICE_ID_FUNCTION_BLOCK,
                        sub_id2: SUB_ID2_PE_GET_INQUIRY,
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
    use midici_pe::Reassembler;
    let mut ra = Reassembler::new(1);
    let mut done = None;
    for (i, o) in chunks.iter().enumerate() {
        let msg = PeGetMessage::decode(&o.body).expect("get reply");
        let peer = msg.header.source;
        if let Some(ev) = ra.feed(peer, &msg.pe_payload, i as u64 + 1).unwrap() {
            done = Some(ev);
        }
    }
    match done.expect("incomplete get reply") {
        midici_pe::ReassembleEvent::Complete { header, body, .. } => {
            let h: GetReplyHeader = serde_json::from_slice(&header).expect("reply header json");
            (h, body)
        }
        other => panic!("unexpected {other:?}"),
    }
}

pub fn drain_all(eng: &mut midici_pe::ResponderEngine<impl RngCore>) -> Vec<OutboundSysex> {
    let mut v = Vec::new();
    while let Some(o) = eng.next_outbound() {
        v.push(o);
    }
    v
}
