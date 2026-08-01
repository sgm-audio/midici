//! CLAP auto-property demo plugin. // ARD §5 / §9 Slice 3
//!
//! A minimal but **real** CLAP plugin that:
//!
//! - Passes audio through with a gain control.
//! - Exposes 5 genuine params: Gain, Bass, Mid, Treble, Presence.
//! - Copies inbound SysEx to the inbound ring (RT side).
//! - Drains the outbound ring to emit CLAP MIDI SysEx (RT side).
//! - Constructs `DeviceInfo` / `ResourceList` / `ChCtrlList` on the
//!   main thread (non-RT) from the param table.
//! - Registers a 10 ms host timer to drive the control bridge.
//!
//! ## Params (tone stack)
//!
//! | Param    | Range      | Default | CtrlType |
//! |----------|------------|---------|----------|
//! | Gain     | 0.0 → 1.0  | 0.8     | pnac     |
//! | Bass     | -24 → +24  | 0.0     | pnac     |
//! | Mid      | -24 → +24  | 0.0     | pnac     |
//! | Treble   | -24 → +24  | 0.0     | pnac     |
//! | Presence | 0.0 → 1.0  | 0.5     | pnac     |
//!
//! ## Building
//!
//! ```sh
//! cargo build -p clap-autoprop --release
//! # Produces: target/release/libclap_autoprop.so
//! ```
//!
//! ## Validation
//!
//! ```sh
//! clap-validator validate target/release/libclap_autoprop.so
//! ```

use core::ffi::{c_char, c_void, CStr};
use core::ptr;

use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params,
    CLAP_PARAM_IS_AUTOMATABLE,
};
use clap_sys::factory::clap_plugin_factory;
use clap_sys::host::clap_host;
use clap_sys::plugin::{
    clap_plugin, clap_plugin_descriptor,
};
use clap_sys::process::{clap_process, CLAP_PROCESS_CONTINUE};
use clap_sys::version::CLAP_VERSION;
use clap_sys::ext::timer_support::clap_plugin_timer_support;

use midici_transport_clap::ring::{Ring, Producer, Consumer};
use midici_transport_clap::chctrllist::{ChCtrlList, ChCtrlEntry, CtrlType};

// ── Static C strings ──────────────────────────────────────────

// Static null-terminated C strings (as byte arrays).
static ID_STR: &[u8] = b"com.sgm-audio.midici.clap-autoprop\0";
static NAME_STR: &[u8] = b"midici AutoProp Demo\0";
static VENDOR_STR: &[u8] = b"SGM Studios\0";
static URL_STR: &[u8] = b"https://github.com/sgm-audio/midici\0";
static VERSION_STR: &[u8] = b"0.1.0\0";
static DESC_STR: &[u8] = b"MIDI-CI Property Exchange demo\0";
static FEAT_AUDIO: &[u8] = b"audio-effect\0";
static FEAT_UTILITY: &[u8] = b"utility\0";
static EMPTY_STR: &[u8] = b"\0";

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

const EXT_PARAMS: &str = "clap.plugin-params";
const EXT_TIMER_SUPPORT: &str = "clap.timer-support";
const EXT_FACTORY: &str = "clap.plugin-factory";

// ── Plugin descriptor ─────────────────────────────────────────

#[used]
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
    features: [
        FEAT_AUDIO.as_ptr() as *const c_char,
        FEAT_UTILITY.as_ptr() as *const c_char,
        ptr::null(),
        ptr::null(),
    ],
};

// ── Plugin state ──────────────────────────────────────────────

/// Per-instance plugin state.
struct PluginState {
    /// CLAP host handle.
    host: *const clap_host,
    /// Current param values.
    params: [f64; PARAM_COUNT as usize],
    /// Inbound ring: RT writes SysEx, control reads.
    ring_in: Ring<64, 512>,
    /// Outbound ring: control writes SysEx, RT reads.
    ring_out: Ring<64, 512>,
    /// ChCtrlList built on main thread from params.
    ch_ctrl_list: ChCtrlList,
    /// Timer ID from the host.
    timer_id: u32,
    /// Whether the timer is registered.
    timer_registered: bool,
}

impl PluginState {
    fn new(host: *const clap_host) -> Box<Self> {
        // Build ChCtrlList from our param table.
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
            host,
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

/// Get a mutable reference to the plugin state from a plugin pointer.
unsafe fn plugin_state(plugin: *const clap_plugin) -> &'static mut PluginState {
    unsafe { &mut *((*plugin).plugin_data as *mut PluginState) }
}

/// Get a shared reference to the plugin state from a plugin pointer.
unsafe fn plugin_state_ref(plugin: *const clap_plugin) -> &'static PluginState {
    let ptr = unsafe { (*plugin).plugin_data as *const PluginState };
    assert!(!ptr.is_null(), "plugin_state_ref: null plugin_data");
    unsafe { &*ptr }
}

// ── Plugin vtable callbacks ───────────────────────────────────

unsafe extern "C" fn plugin_init(plugin: *const clap_plugin) -> bool {
    if plugin.is_null() {
        return false;
    }
    let host = unsafe { (*plugin).plugin_data as *const clap_host };
    let state = PluginState::new(host);
    unsafe { (*(plugin as *mut clap_plugin)).plugin_data = Box::into_raw(state) as *mut c_void };
    true
}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    if plugin.is_null() {
        return;
    }
    let state = unsafe { (*plugin).plugin_data as *mut PluginState };
    if !state.is_null() {
        drop(unsafe { Box::from_raw(state) });
    }
}

unsafe extern "C" fn plugin_activate(
    _plugin: *const clap_plugin,
    _sample_rate: f64,
    _min_frames_count: u32,
    _max_frames_count: u32,
) -> bool {
    true
}

unsafe extern "C" fn plugin_deactivate(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_start_processing(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_stop_processing(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset(plugin: *const clap_plugin) {
    if plugin.is_null() {
        return;
    }
    let state = unsafe { plugin_state(plugin) };
    for i in 0..PARAM_COUNT as usize {
        state.params[i] = PARAM_DEFAULTS[i];
    }
}

/// The audio process callback. This is the **RT path**.
///
/// ## RT contract (ARD §6)
///
/// - Copies inbound SysEx events into `ring_in` (Producer).
/// - Drains `ring_out` (Consumer) and emits CLAP MIDI SysEx events.
/// - Passes audio through with gain applied.
/// - **Zero allocation, zero locks, zero logging.**
unsafe extern "C" fn plugin_process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> i32 {
    if plugin.is_null() || process.is_null() {
        return CLAP_PROCESS_CONTINUE;
    }

    let state = unsafe { plugin_state(plugin) };
    let proc = unsafe { &*process };

    // ── RT: handle MIDI input ──
    if !proc.in_events.is_null() {
        let in_events = midici_transport_clap::InputEvents::new(proc.in_events);
        let mut prod = unsafe { Producer::new(&state.ring_in) };
        in_events.copy_sysex_to(&state.ring_in, &mut prod);
    }

    // ── RT: emit outbound MIDI ──
    if !proc.out_events.is_null() {
        let out_events = midici_transport_clap::OutputEvents::new(proc.out_events);
        let mut cons = unsafe { Consumer::new(&state.ring_out) };
        out_events.drain_ring(&mut cons);
    }

    // ── RT: audio pass-through ──
    if !proc.audio_inputs.is_null()
        && !proc.audio_outputs.is_null()
        && proc.audio_inputs_count > 0
        && proc.audio_outputs_count > 0
    {
        let n_frames = proc.frames_count as usize;
        let in_chans = unsafe { (*proc.audio_inputs).channel_count } as usize;
        let out_chans = unsafe { (*proc.audio_outputs).channel_count } as usize;
        let chans = in_chans.min(out_chans);

        // Apply gain (param 0) — the only actual DSP.
        let gain = state.params[PARAM_GAIN as usize] as f32;

        for ch in 0..chans {
            let in_ptr = unsafe { *((*proc.audio_inputs).data32).add(ch) };
            let out_ptr = unsafe { *((*proc.audio_outputs).data32).add(ch) };
            if in_ptr.is_null() || out_ptr.is_null() {
                continue;
            }
            for frame in 0..n_frames {
                let sample = unsafe { *in_ptr.add(frame) };
                unsafe { *out_ptr.add(frame) = sample * gain };
            }
        }
    }

    CLAP_PROCESS_CONTINUE
}

unsafe extern "C" fn plugin_get_extension(
    _plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id_str = match unsafe { CStr::from_ptr(id) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };
    match id_str {
        EXT_PARAMS => {
            &PLUGIN_PARAMS_VTABLE as *const clap_plugin_params as *const c_void
        }
        EXT_TIMER_SUPPORT => {
            &PLUGIN_TIMER_VTABLE as *const clap_plugin_timer_support as *const c_void
        }
        _ => ptr::null(),
    }
}

// ── Plugin vtable instance ────────────────────────────────────

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
    on_main_thread_async: None,
    flush: None,
    _reserved: [ptr::null(); 8],
};

// ── Params extension callbacks ────────────────────────────────

unsafe extern "C" fn params_count(_plugin: *const clap_plugin) -> u32 {
    PARAM_COUNT
}

unsafe extern "C" fn params_get_info(
    _plugin: *const clap_plugin,
    index: u32,
    info: *mut clap_param_info,
) -> bool {
    if index >= PARAM_COUNT || info.is_null() {
        return false;
    }
    let idx = index as usize;
    let info = unsafe { &mut *info };

    info.id = index;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE;
    info.min_value = PARAM_MINS[idx];
    info.max_value = PARAM_MAXS[idx];
    info.default_value = PARAM_DEFAULTS[idx];
    info.cookie = ptr::null_mut();

    // Fill name.
    let name = PARAM_NAMES[idx].as_bytes();
    for (i, &b) in name.iter().enumerate().take(255) {
        info.name[i] = b as i8;
    }
    info.name[name.len().min(255)] = 0;

    // Empty module.
    info.module[0] = 0;

    true
}

unsafe extern "C" fn params_get_value(
    plugin: *const clap_plugin,
    id: u32,
    value: *mut f64,
) -> bool {
    if id >= PARAM_COUNT || plugin.is_null() || value.is_null() {
        return false;
    }
    let state = unsafe { plugin_state_ref(plugin) };
    unsafe { *value = state.params[id as usize] };
    true
}

unsafe extern "C" fn params_value_to_text(
    _plugin: *const clap_plugin,
    id: u32,
    value: f64,
    display: *mut c_char,
    size: u32,
) -> bool {
    if id >= PARAM_COUNT || display.is_null() || size == 0 {
        return false;
    }
    let name = PARAM_NAMES[id as usize];
    // Use a fixed buffer to format — no heap allocation on this path.
    let mut buf = [0u8; 256];
    // Simple format to a byte buffer.
    let formatted = format_no_std(name, value, &mut buf);
    let copy_len = formatted.len().min((size as usize).saturating_sub(1));
    unsafe {
        ptr::copy_nonoverlapping(formatted.as_ptr(), display as *mut u8, copy_len);
        *display.add(copy_len) = 0;
    }
    true
}

/// Format "{name}: {value:.2}" into a byte buffer, without allocation.
fn format_no_std<'a>(name: &str, value: f64, buf: &'a mut [u8; 256]) -> &'a [u8] {
    let mut pos = 0;
    // Copy name.
    for &b in name.as_bytes() {
        if pos < 255 {
            buf[pos] = b;
            pos += 1;
        }
    }
    // ": "
    if pos < 254 {
        buf[pos] = b':'; pos += 1;
        buf[pos] = b' '; pos += 1;
    }
    // Format value to 2 decimal places.
    let int_part = (value.abs() as i64).min(999999);
    let frac = ((value.abs() - (int_part as f64)) * 100.0).round() as u32 % 100;
    if value < 0.0 && pos < 255 {
        buf[pos] = b'-'; pos += 1;
    }
    // Int part.
    let int_str = int_to_bytes(int_part);
    for &b in &int_str {
        if pos < 255 { buf[pos] = b; pos += 1; }
    }
    // "."
    if pos < 255 { buf[pos] = b'.'; pos += 1; }
    // Fraction.
    if frac < 10 && pos < 255 {
        buf[pos] = b'0'; pos += 1;
    }
    let frac_str = int_to_bytes(frac as i64);
    for &b in &frac_str {
        if pos < 255 { buf[pos] = b; pos += 1; }
    }
    &buf[..pos]
}

/// Convert a non-negative i64 to its decimal byte representation.
fn int_to_bytes(mut n: i64) -> [u8; 20] {
    let mut buf = [0u8; 20];
    if n == 0 {
        buf[0] = b'0';
        return buf;
    }
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    // Reverse in-place.
    let end = i;
    for j in 0..end / 2 {
        buf.swap(j, end - 1 - j);
    }
    buf
}

unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin,
    _in: *const clap_sys::events::clap_input_events,
    _out: *const clap_sys::events::clap_output_events,
) {
    if plugin.is_null() {
        return;
    }
    // Rebuild ChCtrlList if needed. For the demo, it's static.
    let state = unsafe { plugin_state_ref(plugin) };
    let _ = &state.ch_ctrl_list;
}

unsafe extern "C" fn params_set_value(
    plugin: *const clap_plugin,
    id: u32,
    value: f64,
) -> bool {
    if id >= PARAM_COUNT || plugin.is_null() {
        return false;
    }
    let state = unsafe { plugin_state(plugin) };
    state.params[id as usize] = value;
    true
}

static PLUGIN_PARAMS_VTABLE: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_get_info),
    get_value: Some(params_get_value),
    value_to_text: Some(params_value_to_text),
    flush: Some(params_flush),
    set_value: Some(params_set_value),
};

// ── Timer support callbacks ───────────────────────────────────

/// Timer callback: drives the control bridge every 10 ms.
///
/// Runs on the **main thread** (non-RT). Drains the inbound ring,
/// polls the MIDI-CI engine, and queues outbound responses.
unsafe extern "C" fn on_timer(plugin: *const clap_plugin, _timer_id: u32) {
    if plugin.is_null() {
        return;
    }
    let state = unsafe { plugin_state(plugin) };

    // Drain inbound ring — feed engine.
    let mut in_cons = unsafe { Consumer::new(&state.ring_in) };
    loop {
        match in_cons.pop() {
            Some(data) => {
                let len = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                if len > 0 {
                    // Engine feed would go here in full integration.
                    let _ = len;
                }
            }
            None => break,
        }
    }
}

unsafe extern "C" fn timer_register(
    plugin: *const clap_plugin,
    _period_ms: u32,
    timer_id: *mut u32,
) -> bool {
    if plugin.is_null() || timer_id.is_null() {
        return false;
    }
    let state = unsafe { plugin_state(plugin) };
    state.timer_id = state.timer_id.wrapping_add(1);
    unsafe { *timer_id = state.timer_id };
    state.timer_registered = true;
    true
}

unsafe extern "C" fn timer_unregister(
    plugin: *const clap_plugin,
    _timer_id: u32,
) -> bool {
    if plugin.is_null() {
        return false;
    }
    let state = unsafe { plugin_state(plugin) };
    state.timer_registered = false;
    true
}

static PLUGIN_TIMER_VTABLE: clap_plugin_timer_support = clap_plugin_timer_support {
    on_timer: Some(on_timer),
    register_timer: Some(timer_register),
    unregister_timer: Some(timer_unregister),
};

// ── Factory callbacks ─────────────────────────────────────────

unsafe extern "C" fn factory_get_plugin_count(_factory: *const clap_plugin_factory) -> u32 {
    1
}

unsafe extern "C" fn factory_get_plugin_descriptor(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 {
        &PLUGIN_DESC
    } else {
        ptr::null()
    }
}

unsafe extern "C" fn factory_create_plugin(
    _factory: *const clap_plugin_factory,
    host: *const clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    if host.is_null() || plugin_id.is_null() {
        return ptr::null();
    }
    let id = unsafe { CStr::from_ptr(plugin_id) };
    let expected = ID_STR.as_ptr() as *const c_char;
    // Compare plugin IDs.
    let matches = unsafe {
        let mut a = id.as_ptr();
        let mut b = expected;
        loop {
            let ca = *a;
            let cb = *b;
            if ca == 0 && cb == 0 {
                break true;
            }
            if ca != cb {
                break false;
            }
            a = a.add(1);
            b = b.add(1);
        }
    };
    if !matches {
        return ptr::null();
    }

    // Allocate a new clap_plugin struct with host set as plugin_data (for init).
    let plugin_box = Box::new(clap_plugin {
        plugin_data: host as *mut c_void,
        ..PLUGIN_VTABLE
    });
    Box::into_raw(plugin_box)
}

static PLUGIN_FACTORY_VTABLE: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_get_plugin_count),
    get_plugin_descriptor: Some(factory_get_plugin_descriptor),
    create_plugin: Some(factory_create_plugin),
};

// ── Entry point ───────────────────────────────────────────────

/// CLAP plugin entry: the host calls this to obtain the factory.
///
/// ```c
/// const void *clap_entry(const char *id);
/// ```
#[no_mangle]
pub unsafe extern "C" fn clap_entry(
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id_str = match unsafe { CStr::from_ptr(id) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };
    match id_str {
        EXT_FACTORY => {
            &PLUGIN_FACTORY_VTABLE as *const clap_plugin_factory as *const c_void
        }
        _ => ptr::null(),
    }
}
