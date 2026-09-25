//! Deterministic test/canary resource `X-Test` for Set + Subscription
//! conformance (loopback tests and golden transcripts). // M2-103 §8/§11
//!
//! `X-Test` is writable and subscribable: `set` stores the raw property body
//! (validated as 7-bit JSON), `get` returns the stored body, and the resource
//! appears in `ResourceList`-driven flows used by the harness.

use midici_pe::{Payload, PeQuery, PeResult, PeStatus, PropertyResource};

/// Canned initial property body for `X-Test` (deterministic goldens).
pub const XTEST_INITIAL_BODY: &[u8] = b"{\"value\":0}";

/// Writable + subscribable conformance resource.
#[derive(Clone, Debug)]
pub struct XTestResource {
    body: Vec<u8>,
    /// When true, `set` answers 403 (authorization-style read-only).
    pub set_forbidden: bool,
}

impl Default for XTestResource {
    fn default() -> Self {
        Self {
            body: XTEST_INITIAL_BODY.to_vec(),
            set_forbidden: false,
        }
    }
}

impl XTestResource {
    /// Constructor with the write policy pinned. // M2-103 §7.4.1 (403)
    pub fn with_set_forbidden(set_forbidden: bool) -> Self {
        Self {
            set_forbidden,
            ..Self::default()
        }
    }

    /// Last accepted Set body (or the initial body).
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl PropertyResource for XTestResource {
    fn resource(&self) -> &str {
        "X-Test"
    }

    fn get(&self, _req: &PeQuery) -> PeResult<Payload> {
        Ok(Payload {
            body: self.body.clone(),
        })
    }

    fn set(&mut self, _req: &PeQuery, body: &[u8]) -> PeResult<()> {
        if self.set_forbidden {
            return Err(PeStatus::Forbidden);
        }
        if body.is_empty()
            || body.iter().any(|b| *b > 0x7F)
            || serde_json::from_slice::<serde_json::Value>(body).is_err()
        {
            return Err(PeStatus::BadRequest);
        }
        self.body.clear();
        self.body.extend_from_slice(body);
        Ok(())
    }

    fn subscribable(&self) -> bool {
        true
    }
}
