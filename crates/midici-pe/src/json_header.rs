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
    if bytes.len() > MAX_HEADER_JSON_BYTES {
        return Err(PeStatus::BadRequest);
    }
    if bytes.iter().any(|b| *b > 0x7F) {
        return Err(PeStatus::BadRequest);
    }
    let depth = json_depth(bytes).unwrap_or(usize::MAX);
    if depth > MAX_HEADER_JSON_DEPTH {
        return Err(PeStatus::BadRequest);
    }
    match serde_json::from_slice::<GetInquiryHeader>(bytes) {
        Ok(h) if !h.resource.is_empty() => Ok(h),
        _ => Err(PeStatus::BadRequest),
    }
}

/// Serialize a reply header without whitespace. // M2-103 §7.1.1
pub fn encode_reply_header(status: PeStatus, message: Option<&str>) -> PeResult<Vec<u8>> {
    let h = GetReplyHeader {
        status: status.as_u16(),
        message: message.map(String::from),
    };
    // Compact encoding (no whitespace); reject non-7-bit for SysEx-safe wire. // M2-103 §7.1
    let bytes = serde_json::to_vec(&h).map_err(|_| PeStatus::BadRequest)?;
    if bytes.iter().any(|b| *b > 0x7F) {
        return Err(PeStatus::BadRequest);
    }
    Ok(bytes)
}

/// Serialize an inquiry header without whitespace.
pub fn encode_inquiry_header(resource: &str) -> PeResult<Vec<u8>> {
    let h = GetInquiryHeader {
        resource: String::from(resource),
        res_id: None,
    };
    let bytes = serde_json::to_vec(&h).map_err(|_| PeStatus::BadRequest)?;
    if bytes.iter().any(|b| *b > 0x7F) {
        return Err(PeStatus::BadRequest);
    }
    Ok(bytes)
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
        assert_eq!(
            encode_inquiry_header("Devicé"),
            Err(PeStatus::BadRequest)
        );
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
