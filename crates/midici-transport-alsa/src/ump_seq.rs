//! Thin safe wrappers around `alsa-sys` UMP sequencer symbols.
//!
//! The `alsa` 0.12 crate exposes rawmidi [`alsa::Ump`] open/read/write but does
//! **not** wrap sequencer virtual-endpoint APIs (`snd_seq_set_client_midi_version`,
//! `snd_seq_set_ump_endpoint_info`, `snd_seq_ump_event_*`). Those live in
//! `alsa-sys` 0.6 (bound against alsa-lib ≥ 1.2.10). // ARD §2 / Phase 5 decision

use std::ffi::CString;
use std::mem::MaybeUninit;
use std::os::raw::{c_int, c_uint};
use std::ptr;

use alsa_sys as ffi;

use crate::error::{alsa_check, Result, TransportError};

/// Bidirectional ALSA sequencer client configured as a UMP MIDI 2.0 endpoint.
pub struct UmpSeqEndpoint {
    seq: *mut ffi::snd_seq_t,
    client: i32,
    port: i32,
    /// ALSA port `ump_group` (1..=16). UMP packet group nibble is `ump_group - 1`.
    alsa_ump_group: u8,
}

impl Drop for UmpSeqEndpoint {
    fn drop(&mut self) {
        if !self.seq.is_null() {
            unsafe {
                let _ = ffi::snd_seq_delete_port(self.seq, self.port);
                let _ = ffi::snd_seq_close(self.seq);
            }
            self.seq = ptr::null_mut();
        }
    }
}

/// Configuration for a virtual UMP endpoint.
#[derive(Clone, Debug)]
pub struct EndpointConfig {
    /// ALSA sequencer client name (visible in `aseqdump -l`).
    pub client_name: String,
    /// UMP Endpoint Name / primary port name.
    pub endpoint_name: String,
    /// UMP group nibble 0..=15 (mapped to ALSA port ump_group 1..=16).
    pub group: u8,
    pub manufacturer_id: u32,
    pub family_id: u16,
    pub model_id: u16,
    pub sw_revision: [u8; 4],
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            client_name: String::from("midici"),
            endpoint_name: String::from("midici-responder"),
            group: 0,
            manufacturer_id: 0x7D,
            family_id: 1,
            model_id: 1,
            sw_revision: [0, 1, 0, 0],
        }
    }
}

impl UmpSeqEndpoint {
    /// Open sequencer, advertise UMP MIDI 2.0 endpoint + block, create port.
    pub fn create(cfg: &EndpointConfig) -> Result<Self> {
        if cfg.client_name.is_empty() || cfg.endpoint_name.is_empty() {
            return Err(TransportError::Config("client/endpoint name must be non-empty"));
        }
        if cfg.group > 15 {
            return Err(TransportError::Config("UMP group must be 0..=15"));
        }
        let alsa_ump_group = cfg.group + 1; // ALSA: 1..=16

        let mut seq: *mut ffi::snd_seq_t = ptr::null_mut();
        // DUPLEX open. // alsa-lib seq.h
        alsa_check(
            "snd_seq_open",
            unsafe {
                ffi::snd_seq_open(
                    &mut seq,
                    c"default".as_ptr(),
                    ffi::SND_SEQ_OPEN_DUPLEX as c_int,
                    0,
                )
            },
        )?;
        alsa_check("snd_seq_nonblock", unsafe {
            ffi::snd_seq_nonblock(seq, 1)
        })?;

        let client_cstr = CString::new(cfg.client_name.as_str())
            .map_err(|_| TransportError::Config("client name contains NUL"))?;
        alsa_check("snd_seq_set_client_name", unsafe {
            ffi::snd_seq_set_client_name(seq, client_cstr.as_ptr())
        })?;

        // Advertise as UMP MIDI 2.0 client. // alsa-lib seq.h SND_SEQ_CLIENT_UMP_MIDI_2_0
        alsa_check("snd_seq_set_client_midi_version", unsafe {
            ffi::snd_seq_set_client_midi_version(seq, ffi::SND_SEQ_CLIENT_UMP_MIDI_2_0 as c_int)
        })?;
        // Prefer native UMP; disable legacy conversion when available.
        let _ = unsafe { ffi::snd_seq_set_client_ump_conversion(seq, 0) };

        let client = unsafe { ffi::snd_seq_client_id(seq) };
        if client < 0 {
            return Err(TransportError::Alsa {
                op: "snd_seq_client_id",
                code: client,
            });
        }

        set_endpoint_info(seq, cfg)?;
        set_block_info(seq, cfg, alsa_ump_group)?;

        let port = create_ump_port(seq, cfg, alsa_ump_group)?;

        Ok(Self {
            seq,
            client,
            port,
            alsa_ump_group,
        })
    }

    pub fn client_id(&self) -> i32 {
        self.client
    }

    pub fn port_id(&self) -> i32 {
        self.port
    }

    /// UMP group nibble (0..=15) for outbound packets.
    pub fn ump_group_nibble(&self) -> u8 {
        self.alsa_ump_group.saturating_sub(1)
    }

    /// Address string `client:port` for logging / aseqdump `-p`.
    pub fn address_string(&self) -> String {
        format!("{}:{}", self.client, self.port)
    }

    /// Non-blocking input of one UMP sequencer event (up to 4×u32 words).
    ///
    /// Returns `Ok(None)` when no event is pending (`-EAGAIN`).
    pub fn input_ump(&self) -> Result<Option<[u32; 4]>> {
        let mut ev: *mut ffi::snd_seq_ump_event_t = ptr::null_mut();
        let rc = unsafe { ffi::snd_seq_ump_event_input(self.seq, &mut ev) };
        if rc == -libc::EAGAIN {
            return Ok(None);
        }
        alsa_check("snd_seq_ump_event_input", rc)?;
        if ev.is_null() {
            return Ok(None);
        }
        let words = unsafe {
            let e = &*ev;
            // Only UMP-flagged events carry packet data.
            if (u32::from(e.flags) & ffi::SND_SEQ_EVENT_UMP) == 0 {
                return Ok(None);
            }
            e.__bindgen_anon_1.ump
        };
        Ok(Some(words))
    }

    /// Output one UMP packet (1..=4 words) to all subscribers.
    pub fn output_ump(&self, words: &[u32]) -> Result<()> {
        if words.is_empty() || words.len() > 4 {
            return Err(TransportError::Ump("UMP packet must be 1..=4 words"));
        }
        let mut ev = unsafe { MaybeUninit::<ffi::snd_seq_ump_event_t>::zeroed().assume_init() };
        unsafe {
            // Mirror snd_seq_ev_clear / set_ump / set_source / set_subs / set_direct.
            ptr::write_bytes(
                &mut ev as *mut _ as *mut u8,
                0,
                core::mem::size_of::<ffi::snd_seq_ump_event_t>(),
            );
            ev.flags |= ffi::SND_SEQ_EVENT_UMP as u8;
            ev.type_ = 0;
            ev.queue = ffi::SND_SEQ_QUEUE_DIRECT as u8;
            ev.source.client = self.client as u8;
            ev.source.port = self.port as u8;
            ev.dest.client = ffi::SND_SEQ_ADDRESS_SUBSCRIBERS as u8;
            ev.dest.port = ffi::SND_SEQ_ADDRESS_UNKNOWN as u8;
            for (i, w) in words.iter().enumerate() {
                ev.__bindgen_anon_1.ump[i] = *w;
            }
            alsa_check(
                "snd_seq_ump_event_output_direct",
                ffi::snd_seq_ump_event_output_direct(self.seq, &mut ev),
            )?;
        }
        Ok(())
    }

    /// Poll descriptors for the sequencer (for optional blocking wait).
    pub fn poll_fds(&self) -> Result<Vec<libc::pollfd>> {
        let n = unsafe { ffi::snd_seq_poll_descriptors_count(self.seq, libc::POLLIN as c_short) };
        if n < 0 {
            return Err(TransportError::Alsa {
                op: "snd_seq_poll_descriptors_count",
                code: n,
            });
        }
        let mut fds = vec![
            libc::pollfd {
                fd: -1,
                events: 0,
                revents: 0,
            };
            n as usize
        ];
        let filled = unsafe {
            ffi::snd_seq_poll_descriptors(self.seq, fds.as_mut_ptr(), n as c_uint, libc::POLLIN as c_short)
        };
        alsa_check("snd_seq_poll_descriptors", filled)?;
        fds.truncate(filled as usize);
        Ok(fds)
    }
}

use std::os::raw::c_short;

fn set_endpoint_info(seq: *mut ffi::snd_seq_t, cfg: &EndpointConfig) -> Result<()> {
    let mut info: *mut ffi::snd_ump_endpoint_info_t = ptr::null_mut();
    alsa_check("snd_ump_endpoint_info_malloc", unsafe {
        ffi::snd_ump_endpoint_info_malloc(&mut info)
    })?;
    unsafe {
        ffi::snd_ump_endpoint_info_clear(info);
        ffi::snd_ump_endpoint_info_set_name(
            info,
            CString::new(cfg.endpoint_name.as_str())
                .map_err(|_| TransportError::Config("endpoint name contains NUL"))?
                .as_ptr(),
        );
        ffi::snd_ump_endpoint_info_set_protocol_caps(
            info,
            ffi::SND_UMP_EP_INFO_PROTO_MIDI1 | ffi::SND_UMP_EP_INFO_PROTO_MIDI2,
        );
        ffi::snd_ump_endpoint_info_set_protocol(info, ffi::SND_UMP_EP_INFO_PROTO_MIDI2);
        ffi::snd_ump_endpoint_info_set_num_blocks(info, 1);
        ffi::snd_ump_endpoint_info_set_version(info, ffi::SND_UMP_EP_INFO_DEFAULT_VERSION);
        ffi::snd_ump_endpoint_info_set_manufacturer_id(info, cfg.manufacturer_id);
        ffi::snd_ump_endpoint_info_set_family_id(info, cfg.family_id as c_uint);
        ffi::snd_ump_endpoint_info_set_model_id(info, cfg.model_id as c_uint);
        ffi::snd_ump_endpoint_info_set_sw_revision(info, cfg.sw_revision.as_ptr());
        ffi::snd_ump_endpoint_info_set_flags(info, ffi::SND_UMP_EP_INFO_STATIC_BLOCKS);
        let rc = ffi::snd_seq_set_ump_endpoint_info(seq, info as *const _);
        ffi::snd_ump_endpoint_info_free(info);
        alsa_check("snd_seq_set_ump_endpoint_info", rc)?;
    }
    Ok(())
}

fn set_block_info(seq: *mut ffi::snd_seq_t, cfg: &EndpointConfig, alsa_ump_group: u8) -> Result<()> {
    let mut info: *mut ffi::snd_ump_block_info_t = ptr::null_mut();
    alsa_check("snd_ump_block_info_malloc", unsafe {
        ffi::snd_ump_block_info_malloc(&mut info)
    })?;
    unsafe {
        ffi::snd_ump_block_info_clear(info);
        ffi::snd_ump_block_info_set_block_id(info, 0);
        ffi::snd_ump_block_info_set_active(info, 1);
        ffi::snd_ump_block_info_set_direction(info, ffi::SND_UMP_DIR_BIDIRECTION);
        // first_group is 0-based UMP group index in block info.
        ffi::snd_ump_block_info_set_first_group(info, cfg.group as c_uint);
        ffi::snd_ump_block_info_set_num_groups(info, 1);
        ffi::snd_ump_block_info_set_midi_ci_version(
            info,
            ffi::SND_UMP_BLOCK_INFO_DEFAULT_MIDI_CI_VERSION,
        );
        ffi::snd_ump_block_info_set_ui_hint(info, ffi::SND_UMP_BLOCK_UI_HINT_BOTH);
        ffi::snd_ump_block_info_set_name(
            info,
            CString::new(cfg.endpoint_name.as_str())
                .map_err(|_| TransportError::Config("block name contains NUL"))?
                .as_ptr(),
        );
        let _ = alsa_ump_group; // port carries the group association
        let rc = ffi::snd_seq_set_ump_block_info(seq, 0, info as *const _);
        ffi::snd_ump_block_info_free(info);
        alsa_check("snd_seq_set_ump_block_info", rc)?;
    }
    Ok(())
}

fn create_ump_port(
    seq: *mut ffi::snd_seq_t,
    cfg: &EndpointConfig,
    alsa_ump_group: u8,
) -> Result<i32> {
    let mut info: *mut ffi::snd_seq_port_info_t = ptr::null_mut();
    alsa_check("snd_seq_port_info_malloc", unsafe {
        ffi::snd_seq_port_info_malloc(&mut info)
    })?;
    let name = CString::new(cfg.endpoint_name.as_str())
        .map_err(|_| TransportError::Config("port name contains NUL"))?;
    let caps = ffi::SND_SEQ_PORT_CAP_READ
        | ffi::SND_SEQ_PORT_CAP_WRITE
        | ffi::SND_SEQ_PORT_CAP_SUBS_READ
        | ffi::SND_SEQ_PORT_CAP_SUBS_WRITE
        | ffi::SND_SEQ_PORT_CAP_UMP_ENDPOINT;
    let ptype = ffi::SND_SEQ_PORT_TYPE_MIDI_UMP
        | ffi::SND_SEQ_PORT_TYPE_MIDI_GENERIC
        | ffi::SND_SEQ_PORT_TYPE_APPLICATION;
    unsafe {
        ffi::snd_seq_port_info_set_name(info, name.as_ptr());
        ffi::snd_seq_port_info_set_capability(info, caps);
        ffi::snd_seq_port_info_set_type(info, ptype);
        ffi::snd_seq_port_info_set_ump_group(info, alsa_ump_group as c_int);
        let rc = ffi::snd_seq_create_port(seq, info);
        let port = ffi::snd_seq_port_info_get_port(info);
        ffi::snd_seq_port_info_free(info);
        alsa_check("snd_seq_create_port", rc)?;
        Ok(port)
    }
}

/// True when `/dev/snd/seq` appears openable (best-effort probe).
pub fn sequencer_available() -> bool {
    let mut seq: *mut ffi::snd_seq_t = ptr::null_mut();
    let rc = unsafe {
        ffi::snd_seq_open(
            &mut seq,
            c"default".as_ptr(),
            ffi::SND_SEQ_OPEN_DUPLEX as c_int,
            ffi::SND_SEQ_NONBLOCK as c_int,
        )
    };
    if rc >= 0 && !seq.is_null() {
        unsafe {
            let _ = ffi::snd_seq_close(seq);
        }
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_rejects_bad_group() {
        let cfg = EndpointConfig {
            group: 16,
            ..EndpointConfig::default()
        };
        assert!(matches!(
            UmpSeqEndpoint::create(&cfg),
            Err(TransportError::Config(_))
        ));
    }
}
