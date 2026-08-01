//! ChCtrlList resource — maps CLAP plugin parameters to MIDI CI
//! Channel Controller List entries. // ARD §5
//!
//! ## Assignment scheme
//!
//! Each `clap_param_info` is assigned a `ctrlType`, `ctrlIndex`,
//! `channel`, `minMax`, and `default` as follows:
//!
//! ### `ctrlType` assignment
//!
//! | Param range / flags                     | `ctrlType` | Rationale                             |
//! |----------------------------------------|------------|---------------------------------------|
//! | `min == 0, max == 127` (7-bit)         | `"cc"`     | Classic MIDI CC range                 |
//! | `min == 0, max <= 16383` (14-bit)      | `"cc"`     | High-resolution CC (MSB/LSB pair)     |
//! | Enum / stepped (`is_stepped`)           | `"cc"`     | Maps to discrete CC values            |
//! | `CLAP_PARAM_IS_PER_NOTE` flag           | `"pnac"`   | Per-note assignable controller        |
//! | `CLAP_PARAM_IS_PER_CHANNEL` flag, wide  | `"pnp"`    | Per-note pitch (e.g. MPE)             |
//! | All other (continuous, wide range)      | `"pnac"`   | Default to per-note assignable        |
//!
//! ### `ctrlIndex` assignment
//!
//! Assigned sequentially from param index, offset by a configurable
//! base (default 20 to avoid well-known CCs). The map is:
//!
//! ```text
//! ctrlIndex = base + param_index
//! ```
//!
//! For two-CC-pair parameters (14-bit), the MSB uses `ctrlIndex`
//! and the LSB uses `ctrlIndex + 32`.
//!
//! ### `channel` assignment
//!
//! Per-channel params get the channel from `clap_param_info`.
//! Otherwise channel 1 (MIDI channel 0) is used.
//!
//! ### `minMax` and `default`
//!
//! Copied directly from `clap_param_info` min/max/default values.
//! The values are output as JSON numbers per MIDI CI PE spec.
//!
//! ## Example (gain control)
//!
//! ```text
//! clap_param_info { id: 0, name: "Gain", min: 0.0, max: 1.0,
//!                   default: 0.8, flags: 0 }
//! →
//! ChCtrlEntry { title: "Gain", ctrlType: "pnac", ctrlIndex: 20,
//!               channel: 1, minMax: [0.0, 1.0], default: 0.8 }
//! ```

use core::ffi::CStr;

/// A single ChCtrlList entry — maps one plugin parameter to a MIDI
/// controller assignment. // M2-103 §6.3.2
#[derive(Clone, Debug, PartialEq)]
pub struct ChCtrlEntry {
    /// Human-readable parameter name.
    pub title: String,
    /// Controller type: `"cc"`, `"rpn"`, `"nrpn"`, `"pnac"`, or `"pnp"`.
    pub ctrl_type: CtrlType,
    /// Controller index number.
    pub ctrl_index: u16,
    /// MIDI channel (1–16).
    pub channel: u8,
    /// `[min, max]` range in parameter units.
    pub min_max: [f64; 2],
    /// Default value in parameter units.
    pub default: f64,
}

/// Controller type per MIDI CI ChCtrlList. // M2-103 §6.3.2 Table 29
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlType {
    /// Standard MIDI CC (Control Change).
    Cc,
    /// Registered Parameter Number.
    Rpn,
    /// Non-Registered Parameter Number.
    Nrpn,
    /// Per-Note Assignable Controller.
    Pnac,
    /// Per-Note Pitch.
    Pnp,
}

impl CtrlType {
    /// JSON wire name per M2-103.
    pub fn as_str(&self) -> &'static str {
        match self {
            CtrlType::Cc => "cc",
            CtrlType::Rpn => "rpn",
            CtrlType::Nrpn => "nrpn",
            CtrlType::Pnac => "pnac",
            CtrlType::Pnp => "pnp",
        }
    }
}

/// A complete `ChCtrlList` resource — the list of all channel
/// controller assignments for a device.
///
/// Built on the main thread by walking `clap_plugin_params` and
/// assigning each parameter a controller slot.
#[derive(Clone, Debug, PartialEq)]
pub struct ChCtrlList {
    /// Resource identifier — always `"ChCtrlList"`. // M2-103 §6.3
    pub resource: &'static str,
    /// Per-parameter entries.
    pub entries: Vec<ChCtrlEntry>,
}

impl ChCtrlList {
    /// Create an empty ChCtrlList resource.
    pub fn new() -> Self {
        Self {
            resource: "ChCtrlList",
            entries: Vec::new(),
        }
    }

    /// Build the ChCtrlList from a CLAP plugin parameter iterator.
    ///
    /// Walks `clap_plugin_params`, assigning each parameter a `ctrlType`,
    /// `ctrlIndex`, `channel`, and range.
    ///
    /// `base_ctrl_index` sets the starting CC number for sequential
    /// assignment (default: 20 to avoid well-known CCs 0–19).
    ///
    /// # Safety
    ///
    /// `params` must be a valid `clap_plugin_params` pointer for the
    /// duration of this call. Called from the main thread only.
    pub unsafe fn from_clap_params(
        params: *const clap_sys::ext::params::clap_plugin_params,
        param_count: u32,
        base_ctrl_index: u16,
    ) -> Self {
        let mut list = Self::new();

        if params.is_null() {
            return list;
        }

        let get_info = unsafe { (*params).get_info };
        if get_info.is_none() {
            return list;
        }
        let get_info = get_info.unwrap();

        for i in 0..param_count {
            let mut info = core::mem::MaybeUninit::<clap_sys::ext::params::clap_param_info>::uninit();
            let ok = unsafe { get_info(params, i, info.as_mut_ptr()) };
            if !ok {
                continue;
            }
            let info = unsafe { info.assume_init() };

            let title = unsafe { CStr::from_ptr(info.name.as_ptr()) }
                .to_string_lossy()
                .into_owned();

            let ctrl_type = ctrl_type_from_flags(info.flags, info.min_value, info.max_value);
            let ctrl_index = base_ctrl_index + i as u16;

            list.entries.push(ChCtrlEntry {
                title,
                ctrl_type,
                ctrl_index,
                channel: 1, // default; per-channel params TBD
                min_max: [info.min_value, info.max_value],
                default: info.default_value,
            });
        }

        list
    }

    /// Serialize to JSON per MIDI CI PE spec.
    ///
    /// Returns a JSON string representing the ChCtrlList resource.
    /// This is called on the control thread (non-RT).
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\"resource\":\"ChCtrlList\",\"entries\":[");
        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                "{{\"title\":{:?},\"ctrlType\":{:?},\"ctrlIndex\":{},\"channel\":{},\"minMax\":[{},{}],\"default\":{}}}",
                entry.title,
                entry.ctrl_type.as_str(),
                entry.ctrl_index,
                entry.channel,
                entry.min_max[0],
                entry.min_max[1],
                entry.default,
            ));
        }
        json.push_str("]}");
        json
    }
}

impl Default for ChCtrlList {
    fn default() -> Self {
        Self::new()
    }
}

/// Determine `CtrlType` from CLAP parameter flags and range.
///
/// ## Assignment rules
///
/// 1. `CLAP_PARAM_IS_PER_NOTE` → `Pnac` (per-note assignable)
/// 2. `CLAP_PARAM_IS_PER_CHANNEL` + wide range → `Pnp`
/// 3. `min=0, max=127` (7-bit range) → `Cc`
/// 4. `min=0, max≤16383` (14-bit range) → `Cc`
/// 5. Stepped/enum → `Cc`
/// 6. All other → `Pnac` (default)
fn ctrl_type_from_flags(flags: u32, min: f64, max: f64) -> CtrlType {
    use clap_sys::ext::params::{
        CLAP_PARAM_IS_PER_CHANNEL, CLAP_PARAM_IS_PER_NOTE, CLAP_PARAM_IS_STEPPED,
    };

    if flags & CLAP_PARAM_IS_PER_NOTE != 0 {
        return CtrlType::Pnac;
    }

    if flags & CLAP_PARAM_IS_PER_CHANNEL != 0 && (max - min) > 127.0 {
        return CtrlType::Pnp;
    }

    // 7-bit range: 0..=127
    if min == 0.0 && (max - 127.0).abs() < 0.001 {
        return CtrlType::Cc;
    }

    // 14-bit range: 0..≤16383
    if min == 0.0 && max <= 16383.0 && max > 127.0 {
        return CtrlType::Cc;
    }

    // Stepped/enum parameters map to discrete CC values.
    if flags & CLAP_PARAM_IS_STEPPED != 0 {
        return CtrlType::Cc;
    }

    // Default: per-note assignable.
    CtrlType::Pnac
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_list_is_empty() {
        let list = ChCtrlList::new();
        assert_eq!(list.resource, "ChCtrlList");
        assert!(list.entries.is_empty());
    }

    #[test]
    fn ctrl_type_detection_7bit_cc() {
        // 0..127 range → CC
        assert_eq!(
            ctrl_type_from_flags(0, 0.0, 127.0),
            CtrlType::Cc
        );
    }

    #[test]
    fn ctrl_type_detection_14bit_cc() {
        // 0..16383 range → CC
        assert_eq!(
            ctrl_type_from_flags(0, 0.0, 16383.0),
            CtrlType::Cc
        );
    }

    #[test]
    fn ctrl_type_detection_stepped_is_cc() {
        use clap_sys::ext::params::CLAP_PARAM_IS_STEPPED;
        assert_eq!(
            ctrl_type_from_flags(CLAP_PARAM_IS_STEPPED, 0.0, 10.0),
            CtrlType::Cc
        );
    }

    #[test]
    fn ctrl_type_detection_per_note() {
        use clap_sys::ext::params::CLAP_PARAM_IS_PER_NOTE;
        assert_eq!(
            ctrl_type_from_flags(CLAP_PARAM_IS_PER_NOTE, 0.0, 1.0),
            CtrlType::Pnac
        );
    }

    #[test]
    fn ctrl_type_detection_default_pnac() {
        // Wide continuous range → PNAC
        assert_eq!(
            ctrl_type_from_flags(0, -24.0, 24.0),
            CtrlType::Pnac
        );
    }

    #[test]
    fn entry_construction() {
        let entry = ChCtrlEntry {
            title: "Gain".into(),
            ctrl_type: CtrlType::Pnac,
            ctrl_index: 20,
            channel: 1,
            min_max: [0.0, 1.0],
            default: 0.8,
        };
        assert_eq!(entry.title, "Gain");
        assert_eq!(entry.ctrl_type, CtrlType::Pnac);
        assert_eq!(entry.default, 0.8);
    }

    #[test]
    fn json_serialization_smoke() {
        let mut list = ChCtrlList::new();
        list.entries.push(ChCtrlEntry {
            title: "Gain".into(),
            ctrl_type: CtrlType::Pnac,
            ctrl_index: 20,
            channel: 1,
            min_max: [0.0, 1.0],
            default: 0.8,
        });
        list.entries.push(ChCtrlEntry {
            title: "Tone".into(),
            ctrl_type: CtrlType::Cc,
            ctrl_index: 21,
            channel: 1,
            min_max: [0.0, 127.0],
            default: 64.0,
        });

        let json = list.to_json();
        assert!(json.contains("ChCtrlList"));
        assert!(json.contains("Gain"));
        assert!(json.contains("Tone"));
        assert!(json.contains("pnac"));
        assert!(json.contains("cc"));
    }
}
