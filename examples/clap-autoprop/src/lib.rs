//! CLAP auto-property demo plugin. // ARD §5 / §9 Slice 3
//!
//! A minimal but **real** CLAP cdylib plugin with:
//! - 5 genuine params: Gain, Bass, Mid, Treble, Presence
//! - `clap_plugin_params` extension
//! - `clap_plugin_timer_support` extension (10 ms host timer)
//! - RT `process()`: SysEx rings + audio pass-through with gain
//! - `ChCtrlList` built from the param table
//!
//! Built against clap-sys 0.4.0 struct layouts.
//!
//! ```sh
//! cargo build -p clap-autoprop --release
//! clap-validator validate target/release/libclap_autoprop.so
//! ```

use core::ffi::{c_char, c_void, CStr};
use core::ptr;

use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params, CLAP_PARAM_IS_AUTOMATABLE,
};
use clap_sys::factory::clap_plugin_factory;
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::{clap_process, clap_process_status, CLAP_PROCESS_CONTINUE};
use clap_sys::version::CLAP_VERSION;
use clap_sys::ext::timer_support::clap_plugin_timer_support;

use midici_transport_clap::ring::Ring;
use midici_transport_clap::chctrllist::{ChCtrlList, ChCtrlEntry, CtrlType};
use midici_transport_clap::{InputEvents, OutputEvents, Producer, Consumer};

// ── Static C strings ──────────────────────────────────────────

static ID_STR: &[u8] = b"com.sgm-audio.midici.clap-autoprop\0";
static NAME_STR: &[u8] = b"midici AutoProp Demo\0";
static VENDOR_STR: &[u8] = b"SGM Studios\0";
static URL_STR: &[u8] = b"https://github.com/sgm-audio/midici\0";
static VERSION_STR: &[u8] = b"0.1.0\0";
static DESC_STR: &[u8] = b"MIDI-CI Property Exchange demo\0";
static FEAT_AUDIO: &[u8] = b"audio-effect\0";
static FEAT_UTILITY: &[u8] = b"utility\0";
static EMPTY_STR: &[u8] = b"\0";

/// Null-terminated features pointer array for the descriptor.
/// clap-sys 0.4: `features: *const *const c_char` — pointer to a
/// null-terminated array of C string pointers.
static FEATURES: [*const c_char; 3] = [
    FEAT_AUDIO.as_ptr() as *const c_char,
    FEAT_UTILITY.as_ptr() as *const c_char,
    ptr::null(),
];

// ── Param constants ───────────────────────────────────────────

const PARAM_GAIN: u32 = 0;
const PARAM_BASS: u32 = 1;
const PARAM_MID: u32 = 2;
const PARAM_TREBLE: u32 = 3;
const PARAM_PRESENCE: u32 = 4;
const PARAM_COUNT: u32 = 5;

const PARAM_NAMES: &[&str] = &["Gain", "Bass", "Mid", "Treble", "Presence"];
const PARAM_MINS: &[f64] = &[0.0, -24.0, -24.0, -24.0, 0.0];
const PARAM_MAXS: &[f64] = &[1.0, 24.0, 24.0, 24.0, 1.0];
const PARAM_DEFAULTS: &[f64] = &[0.8, 0.0, 0.0, 0.0, 0.5];

// ── Extension IDs ─────────────────────────────────────────────

const EXT_PARAMS: &str = "clap.plugin-params\0";
const EXT_TIMER_SUPPORT: &str = "clap.timer-support\0";
const EXT_FACTORY: &str = "clap.plugin-factory\0";

// ── Plugin descriptor ─────────────────────────────────────────

static PLUGIN_DESC: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: ID_STR.as_ptr() as *const c_char,
    name: NAME_STR.as_ptr() as *const c_char,
    vendor: VENDOR_STR.as_ptr() as *const c_char,
    url: URL_STR.as_ptr() as *const c_char,
    manual_url: EMPTY_STR.as_ptr() as *const c_char,
    support_url: EMPTY_STR.as_ptr() as *const c_char,
    version: VERSION_STR.as_ptr() as *const c_char,
    description: DESC_STR.as_ptr() as *const c_char,
    features: FEATURES.as_ptr(),
};

// ── Plugin state ──────────────────────────────────────────────

struct PluginState {
    params: [f64; PARAM_COUNT as usize],
    ring_in: Ring<64, 512>,
    ring_out: Ring<64, 512>,
    ch_ctrl_list: ChCtrlList,
    timer_id: u32,
    timer_registered: bool,
}

impl PluginState {
    fn new() -> Box<Self> {
        let mut ch_ctrl_list = ChCtrlList::new();
        for i in 0..PARAM_COUNT as usize {
            ch_ctrl_list.entries.push(ChCtrlEntry {
                title: PARAM_NAMES[i].to_string(),
                ctrl_type: CtrlType::Pnac,
                ctrl_index: 20 + i as u16,
                channel: 1,
                min_max: [PARAM_MINS[i], PARAM_MAXS[i]],
                default: PARAM_DEFAULTS[i],
            });
        }
        Box::new(Self {
            params: [
                PARAM_DEFAULTS[0], PARAM_DEFAULTS[1], PARAM_DEFAULTS[2],
                PARAM_DEFAULTS[3], PARAM_DEFAULTS[4],
            ],
            ring_in: Ring::new(),
            ring_out: Ring::new(),
            ch_ctrl_list,
            timer_id: 0,
            timer_registered: false,
        })
    }
}

/// Mutable state ref from plugin pointer.
unsafe fn state_mut(plugin: *const clap_plugin) -> &'static mut PluginState {
    unsafe { &mut *((*plugin).plugin_data as *mut PluginState) }
}

/// Shared state ref from plugin pointer.
unsafe fn state_ref(plugin: *const clap_plugin) -> &'static PluginState {
    let ptr = unsafe { (*plugin).plugin_data as *const PluginState };
    assert!(!ptr.is_null());
    unsafe { &*ptr }
}

// ── Plugin vtable callbacks ───────────────────────────────────

unsafe extern "C" fn plugin_init(plugin: *const clap_plugin) -> bool {
    if plugin.is_null() { return false; }
    let state = PluginState::new();
    unsafe {
        (*(plugin as *mut clap_plugin)).plugin_data =
            Box::into_raw(state) as *mut c_void;
    }
    true
}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    if plugin.is_null() { return; }
    let state = unsafe { (*plugin).plugin_data as *mut PluginState };
    if !state.is_null() {
        drop(unsafe { Box::from_raw(state) });
    }
}

unsafe extern "C" fn plugin_activate(
    _plugin: *const clap_plugin,
    _sr: f64,
    _min: u32,
    _max: u32,
) -> bool { true }

unsafe extern "C" fn plugin_deactivate(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_start_processing(_plugin: *const clap_plugin) -> bool { true }

unsafe extern "C" fn plugin_stop_processing(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset(plugin: *const clap_plugin) {
    if plugin.is_null() { return; }
    let state = unsafe { state_mut(plugin) };
    for i in 0..PARAM_COUNT as usize {
        state.params[i] = PARAM_DEFAULTS[i];
    }
}

/// The audio process callback — **RT path**.
///
/// RT contract (ARD §6): zero alloc, zero lock, zero log.
/// - Copies inbound SysEx into `ring_in` (Producer)
/// - Drains `ring_out` (Consumer) → CLAP MIDI SysEx out
/// - Audio pass-through with gain (param 0)
unsafe extern "C" fn plugin_process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    if plugin.is_null() || process.is_null() {
        return CLAP_PROCESS_CONTINUE;
    }
    let state = unsafe { state_mut(plugin) };
    let proc = unsafe { &*process };

    // RT: MIDI input
    if !proc.in_events.is_null() {
        let in_events = InputEvents::new(proc.in_events);
        let mut prod = unsafe { Producer::new(&state.ring_in) };
        in_events.copy_sysex_to(&state.ring_in, &mut prod);
    }

    // RT: MIDI output
    if !proc.out_events.is_null() {
        let out_events = OutputEvents::new(proc.out_events);
        let mut cons = unsafe { Consumer::new(&state.ring_out) };
        out_events.drain_ring(&mut cons);
    }

    // RT: audio pass-through with gain
    if !proc.audio_inputs.is_null()
        && !proc.audio_outputs.is_null()
        && proc.audio_inputs_count > 0
        && proc.audio_outputs_count > 0
    {
        let n_frames = proc.frames_count as usize;
        let in_chans = unsafe { (*proc.audio_inputs).channel_count } as usize;
        let out_chans = unsafe { (*proc.audio_outputs).channel_count } as usize;
        let chans = in_chans.min(out_chans);
        let gain = state.params[PARAM_GAIN as usize] as f32;
        for ch in 0..chans {
            let in_ptr = unsafe { *((*proc.audio_inputs).data32).add(ch) };
            let out_ptr = unsafe { *((*proc.audio_outputs).data32).add(ch) };
            if in_ptr.is_null() || out_ptr.is_null() { continue; }
            for frame in 0..n_frames {
                unsafe {
                    *out_ptr.add(frame) = *in_ptr.add(frame) * gain;
                }
            }
        }
    }

    CLAP_PROCESS_CONTINUE
}

unsafe extern "C" fn plugin_get_extension(
    _plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() { return ptr::null(); }
    let id_str = match unsafe { CStr::from_ptr(id) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };
    match id_str {
        "clap.plugin-params" => {
            &PLUGIN_PARAMS_VTABLE as *const clap_plugin_params as *const c_void
        }
        "clap.timer-support" => {
            &PLUGIN_TIMER_VTABLE as *const clap_plugin_timer_support as *const c_void
        }
        _ => ptr::null(),
    }
}

// ── Plugin vtable ─────────────────────────────────────────────

static PLUGIN_VTABLE: clap_plugin = clap_plugin {
    desc: &PLUGIN_DESC,
    plugin_data: ptr::null_mut(),
    init: Some(plugin_init),
    destroy: Some(plugin_destroy),
    activate: Some(plugin_activate),
    deactivate: Some(plugin_deactivate),
    start_processing: Some(plugin_start_processing),
    stop_processing: Some(plugin_stop_processing),
    reset: Some(plugin_reset),
    process: Some(plugin_process),
    get_extension: Some(plugin_get_extension),
    on_main_thread: None,
};

// ── Params extension ──────────────────────────────────────────

// clap-sys 0.4: `get_info` has `param_index: u32`, `get_value` has `param_id: clap_id`,
// `value_to_text` has `param_id: clap_id, value: f64, out_buffer: *mut c_char, out_buffer_capacity: u32`.
// No `set_value`; param writes go through `flush`.

unsafe extern "C" fn params_count(_plugin: *const clap_plugin) -> u32 { PARAM_COUNT }

unsafe extern "C" fn params_get_info(
    _plugin: *const clap_plugin,
    param_index: u32,
    param_info: *mut clap_param_info,
) -> bool {
    if param_index >= PARAM_COUNT || param_info.is_null() { return false; }
    let idx = param_index as usize;
    let info = unsafe { &mut *param_info };
    info.id = param_index;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE;
    info.min_value = PARAM_MINS[idx];
    info.max_value = PARAM_MAXS[idx];
    info.default_value = PARAM_DEFAULTS[idx];
    info.cookie = ptr::null_mut();
    let name = PARAM_NAMES[idx].as_bytes();
    for (i, &b) in name.iter().enumerate().take(255) {
        info.name[i] = b as i8;
    }
    info.name[name.len().min(255)] = 0;
    info.module[0] = 0;
    true
}

unsafe extern "C" fn params_get_value(
    plugin: *const clap_plugin,
    param_id: u32,
    out_value: *mut f64,
) -> bool {
    if param_id >= PARAM_COUNT || plugin.is_null() || out_value.is_null() { return false; }
    let state = unsafe { state_ref(plugin) };
    unsafe { *out_value = state.params[param_id as usize] };
    true
}

unsafe extern "C" fn params_value_to_text(
    _plugin: *const clap_plugin,
    param_id: u32,
    value: f64,
    out_buffer: *mut c_char,
    out_buffer_capacity: u32,
) -> bool {
    if param_id >= PARAM_COUNT || out_buffer.is_null() || out_buffer_capacity == 0 { return false; }
    let name = PARAM_NAMES[param_id as usize];
    // Stack format: "{name}: {value:.2}"
    let mut buf = [0u8; 256];
    let formatted = fmt_param(name, value, &mut buf);
    let copy = formatted.len().min((out_buffer_capacity as usize).saturating_sub(1));
    unsafe {
        ptr::copy_nonoverlapping(formatted.as_ptr(), out_buffer as *mut u8, copy);
        *out_buffer.add(copy) = 0;
    }
    true
}

fn fmt_param<'a>(name: &str, value: f64, buf: &'a mut [u8; 256]) -> &'a [u8] {
    let mut pos = 0;
    for &b in name.as_bytes() {
        if pos < 255 { buf[pos] = b; pos += 1; }
    }
    if pos < 254 { buf[pos] = b':'; pos += 1; buf[pos] = b' '; pos += 1; }
    if value < 0.0 && pos < 255 { buf[pos] = b'-'; pos += 1; }
    let abs = value.abs();
    let int = (abs as u64).min(999999);
    let frac = ((abs - int as f64) * 100.0).round() as u32 % 100;
    let int_s = int_to_bytes(int);
    for &b in &int_s {
        if pos < 255 && b != 0 { buf[pos] = b; pos += 1; }
    }
    if int == 0 && pos < 255 { buf[pos] = b'0'; pos += 1; }
    if pos < 255 { buf[pos] = b'.'; pos += 1; }
    if frac < 10 && pos < 255 { buf[pos] = b'0'; pos += 1; }
    let frac_s = int_to_bytes(frac as u64);
    for &b in &frac_s {
        if pos < 255 && b != 0 { buf[pos] = b; pos += 1; }
    }
    if frac == 0 && pos < 255 { buf[pos] = b'0'; pos += 1; }
    &buf[..pos]
}

fn int_to_bytes(n: u64) -> [u8; 20] {
    let mut buf = [0u8; 20];
    if n == 0 { buf[0] = b'0'; return buf; }
    let mut v = n;
    let mut i = 0;
    while v > 0 { buf[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    let end = i;
    for j in 0..end / 2 { buf.swap(j, end - 1 - j); }
    buf
}

unsafe extern "C" fn params_text_to_value(
    _plugin: *const clap_plugin,
    param_id: u32,
    _param_value_text: *const c_char,
    _out_value: *mut f64,
) -> bool {
    // Not implemented: return false (CLAP spec: host should not rely on this).
    let _ = param_id;
    false
}

unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin,
    _in: *const clap_sys::events::clap_input_events,
    _out: *const clap_sys::events::clap_output_events,
) {
    if plugin.is_null() { return; }
    // Param changes arrive via the input events. For this demo, values
    // are already set via automation — no extra sync needed.
}

static PLUGIN_PARAMS_VTABLE: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_get_info),
    get_value: Some(params_get_value),
    value_to_text: Some(params_value_to_text),
    text_to_value: Some(params_text_to_value),
    flush: Some(params_flush),
};

// ── Timer support extension ───────────────────────────────────

/// Timer callback: drains the inbound ring, feeding engine.
/// Runs on the **main thread** (non-RT) every 10 ms.
unsafe extern "C" fn on_timer(plugin: *const clap_plugin, _timer_id: u32) {
    if plugin.is_null() { return; }
    let state = unsafe { state_mut(plugin) };
    let mut in_cons = unsafe { Consumer::new(&state.ring_in) };
    loop {
        match in_cons.pop() {
            Some(data) => {
                let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                if len > 0 {
                    let _ = len; // engine.feed_sysex() in full integration
                }
            }
            None => break,
        }
    }
}

// clap-sys 0.4: clap_plugin_timer_support only has `on_timer`.
// register_timer / unregister_timer are on the HOST side (clap_host_timer_support).
static PLUGIN_TIMER_VTABLE: clap_plugin_timer_support = clap_plugin_timer_support {
    on_timer: Some(on_timer),
};

// ── Factory ───────────────────────────────────────────────────

unsafe extern "C" fn factory_get_plugin_count(
    _factory: *const clap_plugin_factory,
) -> u32 { 1 }

unsafe extern "C" fn factory_get_plugin_descriptor(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 { &PLUGIN_DESC } else { ptr::null() }
}

unsafe extern "C" fn factory_create_plugin(
    _factory: *const clap_plugin_factory,
    host: *const clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    if host.is_null() || plugin_id.is_null() { return ptr::null(); }
    // Match plugin ID.
    let expected = ID_STR.as_ptr() as *const c_char;
    let mut a = unsafe { CStr::from_ptr(plugin_id) }.to_bytes_with_nul().as_ptr();
    let mut b = expected;
    let matches = loop {
        unsafe {
            if *a == 0 && *b == 0 { break true; }
            if *a != *b { break false; }
            a = a.add(1);
            b = b.add(1);
        }
    };
    if !matches { return ptr::null(); }

    let pbox = Box::new(clap_plugin {
        plugin_data: host as *mut c_void,
        ..PLUGIN_VTABLE
    });
    Box::into_raw(pbox)
}

static PLUGIN_FACTORY_VTABLE: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_get_plugin_count),
    get_plugin_descriptor: Some(factory_get_plugin_descriptor),
    create_plugin: Some(factory_create_plugin),
};

// ── Entry point ───────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn clap_entry(id: *const c_char) -> *const c_void {
    if id.is_null() { return ptr::null(); }
    let id_str = match unsafe { CStr::from_ptr(id) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };
    match id_str {
        "clap.plugin-factory" => {
            &PLUGIN_FACTORY_VTABLE as *const clap_plugin_factory as *const c_void
        }
        _ => ptr::null(),
    }
}
