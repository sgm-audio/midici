//! PE JSON header parse/serialize with depth and size limits. // M2-103 §7.1 / ARD §7

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::status::{PeResult, PeStatus};

/// Maximum PE header JSON bytes accepted on parse. // ARD §7 depth/size limits
pub const MAX_HEADER_JSON_BYTES: usize = 4096;
/// Maximum nesting depth for PE JSON headers.
pub const MAX_HEADER_JSON_DEPTH: usize = 32;

/// Inquiry Get header (`resource` required). // M2-103 §8.2 / §7.1
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetInquiryHeader {
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_id: Option<String>,
}

/// Reply Get header (`status` required). // M2-103 §7.4
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetReplyHeader {
    pub status: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Inquiry Set header (`resource` required). // M2-103 §8.2 / §7.2 Table 13
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetInquiryHeader {
    pub resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_id: Option<String>,
    /// True for a Partial Set (property data is a map of "/path": value).
    /// Omitted on the wire when false. // M2-103 §8.2 (Table 35 example) / §8.5
    #[serde(default, rename = "setPartial", skip_serializing_if = "is_false")]
    pub set_partial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutual_encoding: Option<String>,
}

/// Subscription command values. // M2-103 §11.1 Table 39
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubCommand {
    /// Start Subscription — Initiator only.
    #[serde(rename = "start")]
    Start,
    /// Partial Property Data update — Responder only.
    #[serde(rename = "partial")]
    Partial,
    /// Full Property Data update — Responder only.
    #[serde(rename = "full")]
    Full,
    /// Ask Initiator to re-Get the full Property Data — Responder only.
    #[serde(rename = "notify")]
    Notify,
    /// End Subscription — either device.
    #[serde(rename = "end")]
    End,
}

/// Subscription message header. // M2-103 §11.1 Table 39
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionHeader {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<SubCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "subscribeId"
    )]
    pub subscribe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "endedBy")]
    pub ended_by: Option<String>,
}

/// Reply to Subscription header (`status` first — reply rule).
/// // M2-103 §7.1 first-property rule / §11.1 (subscribeId)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubReplyHeader {
    pub status: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "subscribeId"
    )]
    pub subscribe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Legacy Notify header (`status` only). Receive-only; deprecated.
/// // M2-103 §12 Tables 60/62/63; M2-101 §8.13
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyHeader {
    pub status: u16,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// Notify status: Terminate Inquiry. // M2-103 §12.1.3
pub const NOTIFY_STATUS_TERMINATE: u16 = 144;
/// Notify status: Timeout Wait (deprecated). // M2-103 §12.2.2
pub const NOTIFY_STATUS_TIMEOUT_WAIT: u16 = 100;
/// Notify status: Timeout Has Occurred (deprecated). // M2-103 §12.2.4
pub const NOTIFY_STATUS_TIMEOUT: u16 = 408;

/// Count JSON structural nesting depth; rejects non-ASCII / controls loosely.
fn json_depth(bytes: &[u8]) -> Option<usize> {
    let mut depth = 0usize;
    let mut max = 0usize;
    let mut in_string = false;
    let mut escape = false;
    for &b in bytes {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                max = max.max(depth);
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Some(max)
}

/// Parse PE header JSON with size/depth limits. Malformed → [`PeStatus::BadRequest`].
/// // M2-103 §7.4.1 status 400 / ARD §7
pub fn parse_get_inquiry_header(bytes: &[u8]) -> PeResult<GetInquiryHeader> {
    let h: GetInquiryHeader = parse_header(bytes)?;
    if h.resource.is_empty() {
        return Err(PeStatus::BadRequest);
    }
    Ok(h)
}

/// Parse an Inquiry: Set Property Data header. // M2-103 §8.2 / §7.2
pub fn parse_set_inquiry_header(bytes: &[u8]) -> PeResult<SetInquiryHeader> {
    let h: SetInquiryHeader = parse_header(bytes)?;
    if h.resource.is_empty() {
        return Err(PeStatus::BadRequest);
    }
    Ok(h)
}

/// Parse a Subscription message header. // M2-103 §11.1 Table 39
pub fn parse_subscription_header(bytes: &[u8]) -> PeResult<SubscriptionHeader> {
    parse_header(bytes)
}

/// Parse a legacy Notify header. // M2-103 §12
pub fn parse_notify_header(bytes: &[u8]) -> PeResult<NotifyHeader> {
    parse_header(bytes)
}

/// Shared size/depth/7-bit guards + serde parse. // M2-103 §7.1 / ARD §7
fn parse_header<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> PeResult<T> {
    if bytes.len() > MAX_HEADER_JSON_BYTES || bytes.is_empty() {
        return Err(PeStatus::BadRequest);
    }
    if bytes.iter().any(|b| *b > 0x7F) {
        return Err(PeStatus::BadRequest);
    }
    let depth = json_depth(bytes).unwrap_or(usize::MAX);
    if depth > MAX_HEADER_JSON_DEPTH {
        return Err(PeStatus::BadRequest);
    }
    serde_json::from_slice::<T>(bytes).map_err(|_| PeStatus::BadRequest)
}

/// Serialize JSON header bytes without whitespace; reject non-7-bit. // M2-103 §7.1
fn encode_header<T: Serialize>(v: &T) -> PeResult<Vec<u8>> {
    let bytes = serde_json::to_vec(v).map_err(|_| PeStatus::BadRequest)?;
    if bytes.iter().any(|b| *b > 0x7F) {
        return Err(PeStatus::BadRequest);
    }
    Ok(bytes)
}

/// Serialize a reply header without whitespace. // M2-103 §7.1.1
pub fn encode_reply_header(status: PeStatus, message: Option<&str>) -> PeResult<Vec<u8>> {
    let h = GetReplyHeader {
        status: status.as_u16(),
        message: message.map(String::from),
    };
    encode_header(&h)
}

/// Serialize an inquiry header without whitespace.
pub fn encode_inquiry_header(resource: &str) -> PeResult<Vec<u8>> {
    let h = GetInquiryHeader {
        resource: String::from(resource),
        res_id: None,
    };
    encode_header(&h)
}

/// Serialize an Inquiry: Set header. // M2-103 §8.2 / §7.2
pub fn encode_set_inquiry_header(
    resource: &str,
    res_id: Option<&str>,
    set_partial: bool,
) -> PeResult<Vec<u8>> {
    let h = SetInquiryHeader {
        resource: String::from(resource),
        res_id: res_id.map(String::from),
        set_partial,
        mutual_encoding: None,
    };
    encode_header(&h)
}

/// Serialize a Subscription inquiry/update header. // M2-103 §11.1
pub fn encode_subscription_header(
    command: SubCommand,
    resource: Option<&str>,
    subscribe_id: Option<&str>,
    ended_by: Option<&str>,
) -> PeResult<Vec<u8>> {
    let h = SubscriptionHeader {
        command: Some(command),
        resource: resource.map(String::from),
        res_id: None,
        subscribe_id: subscribe_id.map(String::from),
        ended_by: ended_by.map(String::from),
    };
    encode_header(&h)
}

/// Serialize a Reply to Subscription header (status first). // M2-103 §7.1 / §11.2
pub fn encode_sub_reply_header(
    status: PeStatus,
    subscribe_id: Option<&str>,
    message: Option<&str>,
) -> PeResult<Vec<u8>> {
    let h = SubReplyHeader {
        status: status.as_u16(),
        subscribe_id: subscribe_id.map(String::from),
        message: message.map(String::from),
    };
    encode_header(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_inquiry() {
        let raw = br#"{"resource":"DeviceInfo"}"#;
        let h = parse_get_inquiry_header(raw).unwrap();
        assert_eq!(h.resource, "DeviceInfo");
    }

    #[test]
    fn malformed_never_panics() {
        assert_eq!(
            parse_get_inquiry_header(b"{not json"),
            Err(PeStatus::BadRequest)
        );
        assert_eq!(parse_get_inquiry_header(b""), Err(PeStatus::BadRequest));
        assert_eq!(
            parse_get_inquiry_header(b"{\"resource\":}"),
            Err(PeStatus::BadRequest)
        );
    }

    #[test]
    fn depth_limit_rejects() {
        let mut s = alloc::vec![b'['; 40];
        s.extend_from_slice(b"1");
        s.extend(core::iter::repeat_n(b']', 40));
        assert_eq!(parse_get_inquiry_header(&s), Err(PeStatus::BadRequest));
    }

    #[test]
    fn size_limit_rejects() {
        let mut s = b"{\"resource\":\"".to_vec();
        s.extend(core::iter::repeat_n(b'A', MAX_HEADER_JSON_BYTES));
        s.extend_from_slice(b"\"}");
        assert_eq!(parse_get_inquiry_header(&s), Err(PeStatus::BadRequest));
    }

    #[test]
    fn encode_rejects_non_7bit() {
        assert_eq!(encode_inquiry_header("Devicé"), Err(PeStatus::BadRequest));
        assert_eq!(
            encode_reply_header(PeStatus::Ok, Some("oké")),
            Err(PeStatus::BadRequest)
        );
        let inquiry = encode_inquiry_header("DeviceInfo").unwrap();
        assert!(inquiry.iter().all(|b| *b <= 0x7F));
        let reply = encode_reply_header(PeStatus::Ok, Some("ok")).unwrap();
        assert!(reply.iter().all(|b| *b <= 0x7F));
    }
}
