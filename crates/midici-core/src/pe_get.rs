//! PE Get Inquiry/Reply — backward-compatible alias for the generic chunked
//! PE message. // M2-101 §8.7–§8.8 Tables 33–34

use crate::pe_msg::PeMessage;

/// A PE Get Inquiry or Reply: CI header followed by PE chunk fields.
///
/// Since Phase 7 this is an alias of [`PeMessage`], which accepts any
/// chunked PE sub-ID (0x34–0x39, 0x3F); routing by `header.sub_id2` happens
/// in the PE controller. // M2-101 Tables 33–39
pub type PeGetMessage = PeMessage;
