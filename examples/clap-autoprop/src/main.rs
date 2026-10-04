//! CLAP auto-property demo plugin: control-thread Notify wiring.
//!
//! The full CLAP plugin (RT bridge + `ChCtrlList` from `clap_plugin_params`)
//! is Phase 6 scope and still behind its own DoD gate. What ships here is the
//! Phase 7 wiring contract the plugin binding calls into: when the host
//! reports param value changes (`clap_plugin_params.flush` / timer path), the
//! control thread calls [`flush_param_change`], which fans out PE
//! `command: "partial"` updates to every subscribed peer.
//!
//! RT contract: [`flush_param_change`] is **control-thread only**. The audio
//! thread never touches the engine; it only moves bytes through the
//! midici-transport-clap rings. // ARD §5 / §6; M2-103 §11

use midici_pe::{NotifyBody, ResponderEngine};
use rand_core::RngCore;

/// Fan out a changed param's partial update to all PE subscribers of
/// `resource` (e.g. `"ChCtrlList"`). Returns the number of peers messaged.
///
/// `partial_body` is a Partial-Set-shaped JSON object: `{"/path": value}`.
/// // M2-103 §11.1 (partial) / §8.5 (partial set body)
///
/// Call only from the control thread (host `clap_plugin_timer.on_timer`, i.e.
/// after the param flush path has run) — never from `process()`. // ARD §6
pub fn flush_param_change<R: RngCore>(
    engine: &mut ResponderEngine<R>,
    resource: &str,
    partial_body: &[u8],
) -> usize {
    engine.notify_resource_changed(resource, NotifyBody::Partial(partial_body))
}

fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), midici_responder::VERSION);
}

#[cfg(test)]
mod tests {
    use super::*;
    use midici_core::spec::{
        DEVICE_ID_FUNCTION_BLOCK, MESSAGE_FORMAT_VERSION_1_2, SUB_ID2_PE_SUBSCRIPTION,
    };
    use midici_core::{pe_caps_inquiry, CiConfig, CiHeader, DeviceIdentity, Muid, PeMessage};
    use midici_pe::{
        encode_subscription_header, split, Payload, PeQuery, PeResult, PropertyResource,
        ResourceRegistry, SubCommand,
    };
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    /// Subscribable stand-in for the future `ChCtrlList` resource.
    struct ChCtrlLike;

    impl PropertyResource for ChCtrlLike {
        fn resource(&self) -> &str {
            "ChCtrlList"
        }
        fn get(&self, _req: &PeQuery) -> PeResult<Payload> {
            Ok(Payload {
                body: b"[]".to_vec(),
            })
        }
        fn subscribable(&self) -> bool {
            true
        }
    }

    fn engine_with_chctrllist() -> ResponderEngine<StdRng> {
        let identity = DeviceIdentity {
            manufacturer: [0x43, 0, 0],
            family: 3,
            model: 4,
            software_revision: [2, 0, 0, 0],
        };
        let cfg = CiConfig::responder_default(identity);
        let mut registry = ResourceRegistry::with_device_info(identity);
        registry.register(Box::new(ChCtrlLike));
        ResponderEngine::new(cfg, StdRng::seed_from_u64(0xC0FFEE), registry)
    }

    #[test]
    fn flush_without_subscribers_is_noop() {
        let mut engine = engine_with_chctrllist();
        assert_eq!(
            flush_param_change(&mut engine, "ChCtrlList", br#"{"/gain":1}"#),
            0
        );
        assert!(engine.next_outbound().is_none());
    }

    #[test]
    fn subscribed_peer_receives_partial_notify_on_flush() {
        let mut engine = engine_with_chctrllist();
        let peer = Muid::ordinary(0x01020304).unwrap();
        let dest = engine.muid();

        // Peer negotiates PE caps (so its subscription path is fully exercised).
        let caps = pe_caps_inquiry(peer, dest, 4);
        let mut buf = [0u8; 32];
        let n = caps.encode(&mut buf).unwrap();
        engine.feed_sysex(0, &buf[..n]).unwrap();
        assert!(engine.next_outbound().is_some()); // caps reply

        // Peer subscribes to ChCtrlList.
        let header =
            encode_subscription_header(SubCommand::Start, Some("ChCtrlList"), None, None).unwrap();
        let chunks = split(1, &header, &[], 512).unwrap();
        for pe_payload in chunks {
            let msg = PeMessage {
                header: CiHeader {
                    device_id: DEVICE_ID_FUNCTION_BLOCK,
                    sub_id2: SUB_ID2_PE_SUBSCRIPTION,
                    version: MESSAGE_FORMAT_VERSION_1_2,
                    source: peer,
                    dest,
                },
                pe_payload,
            }
            .to_vec()
            .unwrap();
            engine.feed_sysex(0, &msg).unwrap();
        }
        assert!(engine.next_outbound().is_some()); // 0x39 reply w/ subscribeId

        // Control thread: host flush reported a param change → partial notify.
        let notified = flush_param_change(&mut engine, "ChCtrlList", br#"{"/gain":0.5}"#);
        assert_eq!(notified, 1);

        let out = engine.next_outbound().expect("subscription update");
        let (h, _) = CiHeader::decode(&out.body).unwrap();
        assert_eq!(h.sub_id2, SUB_ID2_PE_SUBSCRIPTION);
        let chunk =
            midici_pe::PeChunk::decode(&PeMessage::decode(&out.body).unwrap().pe_payload).unwrap();
        let hdr: midici_pe::SubscriptionHeader = serde_json::from_slice(&chunk.header).unwrap();
        assert_eq!(hdr.command, Some(SubCommand::Partial));
        assert_eq!(chunk.property, br#"{"/gain":0.5}"#);
    }
}
