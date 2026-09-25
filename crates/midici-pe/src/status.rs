//! PE reply `status` codes. // M2-103 §7.4.1 Table 15; ARD §4 / §7 subset

/// Typed Property Exchange reply status. // M2-103 §7.4.1 Table 15
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum PeStatus {
    /// 200 Success/Ok. // M2-103 §7.4.1 Table 15
    Ok = 200,
    /// 201 Accepted – Set accepted but results not guaranteed. // M2-103 §7.4.1
    ///
    /// (ARD §4 lists 202 for Set; M2-103 v1.2 Table 15 defines 201 and no 202 —
    /// the pinned spec wins. See PROGRESS.md Phase 7.)
    Accepted = 201,
    /// 341 Resource Currently Unavailable or an Error Occurred. // M2-103 §7.4.1
    ///
    /// ARD §7 maps stalled multi-chunk tx → 341.
    Unavailable = 341,
    /// 400 Bad Request – Data was received but it isn't correct. // M2-103 §7.4.1
    BadRequest = 400,
    /// 403 Request received but reply not available based on Authorization. // M2-103 §7.4.1
    Forbidden = 403,
    /// 404 Resource Not Supported/Found. // M2-103 §7.4.1
    NotFound = 404,
    /// 405 Resource Not Allowed – Resource is not applicable at this time. // M2-103 §7.4.1
    NotAllowed = 405,
    /// 413 Payload Too Large. // M2-103 §7.4.1 / ARD §7 chunk flood
    PayloadTooLarge = 413,
    /// Simultaneous-request / busy rejection used by this stack.
    ///
    /// ARD §7 maps excess concurrent txs → **445**. M2-103 §7.4.1 Table 15 lists
    /// **343** as “Too Many Requests” and **445** as “Invalid Version of Data”;
    /// this crate follows ARD §7 / Phase-4 DoD for the busy path and documents the
    /// table discrepancy in `docs/PROGRESS.md`.
    Busy = 445,
}

impl PeStatus {
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            200 => Some(Self::Ok),
            201 => Some(Self::Accepted),
            341 => Some(Self::Unavailable),
            400 => Some(Self::BadRequest),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            405 => Some(Self::NotAllowed),
            413 => Some(Self::PayloadTooLarge),
            445 => Some(Self::Busy),
            _ => None,
        }
    }
}

/// Result alias for resource get/set.
pub type PeResult<T> = Result<T, PeStatus>;
