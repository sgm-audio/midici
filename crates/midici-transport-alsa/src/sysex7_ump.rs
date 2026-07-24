//! SysEx7 UMP ↔ F0/F7-stripped body bridging via `midi2`.
//!
//! Packet layout cites M2-104-UM (Data messages / SysEx7). Payload encode/decode
//! is delegated to the `midi2` crate (ARD §2).

use midi2::prelude::*;
use midi2::sysex7::Sysex7;

use crate::error::{Result, TransportError};

/// Incremental reassembler for inbound SysEx7 UMP packets (2 words each).
#[derive(Default)]
pub struct Sysex7Reassembler {
    words: Vec<u32>,
    group: Option<u8>,
}

/// Completed SysEx7 body (F0/F7 stripped) with UMP group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompleteSysex {
    pub group: u8,
    pub body: Vec<u8>,
}

impl Sysex7Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one ALSA UMP event's words. Returns a complete body when the SysEx ends.
    pub fn feed(&mut self, words: &[u32]) -> Result<Option<CompleteSysex>> {
        if words.len() < 2 {
            return Ok(None);
        }
        let w0 = words[0];
        let msg_type = (w0 >> 28) & 0xF;
        if msg_type != 0x3 {
            // Non-SysEx7 UMP — ignore for CI path.
            return Ok(None);
        }
        let group = ((w0 >> 24) & 0xF) as u8;
        let status = ((w0 >> 20) & 0xF) as u8; // Complete/Start/Continue/End // M2-104

        if status == 0x1 || self.words.is_empty() {
            // Start (or Complete single-packet) begins a new message.
            self.words.clear();
            self.group = Some(group);
        } else if self.group != Some(group) {
            // Group change mid-message — reset.
            self.words.clear();
            self.group = Some(group);
        }

        self.words.push(words[0]);
        self.words.push(words[1]);

        // Complete (0) or End (3) finish the message. // M2-104 SysEx7 status
        if status == 0x0 || status == 0x3 {
            let done = self.take_complete()?;
            return Ok(Some(done));
        }
        Ok(None)
    }

    fn take_complete(&mut self) -> Result<CompleteSysex> {
        let group = self.group.unwrap_or(0);
        let words = std::mem::take(&mut self.words);
        self.group = None;
        let borrowed = Sysex7::<&[u32]>::try_from(words.as_slice()).map_err(|_| {
            TransportError::Ump("invalid SysEx7 UMP message")
        })?;
        let body: Vec<u8> = borrowed.payload().map(u8::from).collect();
        Ok(CompleteSysex { group, body })
    }
}

/// Encode an F0/F7-stripped SysEx body into SysEx7 UMP packet words (pairs of u32).
pub fn encode_sysex7_packets(group: u8, body: &[u8]) -> Result<Vec<[u32; 2]>> {
    if group > 15 {
        return Err(TransportError::Config("UMP group must be 0..=15"));
    }
    if body.iter().any(|b| *b > 0x7F) {
        return Err(TransportError::Ump("SysEx7 body must be 7-bit"));
    }
    let mut msg = Sysex7::<Vec<u32>>::new();
    msg.set_group(u4::new(group));
    msg.set_payload(body.iter().copied().map(u7::new));
    let data = msg.data();
    if data.len() % 2 != 0 {
        return Err(TransportError::Ump("SysEx7 UMP length not multiple of 2 words"));
    }
    let mut out = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        out.push([chunk[0], chunk[1]]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_discovery_sized_body() {
        // Universal Non-Realtime + CI header-ish payload (7-bit).
        let body: Vec<u8> = {
            let mut v = vec![0x7E, 0x7F, 0x0D, 0x70, 0x02];
            v.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
            v.extend_from_slice(&[0x7F, 0x7F, 0x7F, 0x7F]);
            v.extend(std::iter::repeat_n(0x00, 20));
            v
        };
        let packets = encode_sysex7_packets(0, &body).unwrap();
        assert!(!packets.is_empty());
        let mut ra = Sysex7Reassembler::new();
        let mut done = None;
        for p in &packets {
            if let Some(c) = ra.feed(p).unwrap() {
                done = Some(c);
            }
        }
        let c = done.expect("complete");
        assert_eq!(c.group, 0);
        assert_eq!(c.body, body);
    }
}
