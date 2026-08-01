//! ChCtrlList resource — maps CLAP plugin parameters to MIDI CI
//! Channel Controller List entries. // ARD §5
//!
//! ## Assignment scheme (see rustdoc on `ctrl_type_from_flags`)
//!
//! | Param range / flags                     | `ctrlType` |
//! |----------------------------------------|------------|
//! | `min == 0, max == 127` (7-bit)         | Cc         |
//! | `min == 0, max <= 16383` (14-bit)      | Cc         |
//! | Enum / stepped (`is_stepped`)           | Cc         |
//! | `CLAP_PARAM_IS_PER_NOTE`                | Pnac       |
//! | `CLAP_PARAM_IS_PER_CHANNEL`, wide       | Pnp        |
//! | All other (continuous, wide range)      | Pnac       |
//!
//! `ctrlIndex` = `base_ctrl_index + param_index`.
//!
//! ## clap-sys 0.4 compatibility
//!
//! `from_clap_params` takes `*const clap_plugin` + `*const clap_plugin_params`
//! because `get_info` in clap-sys 0.4 has signature
//! `fn(plugin: *const clap_plugin, param_index: u32, param_info: *mut clap_param_info)`.

#[cfg(feature = "clap-ffi")]
use core::ffi::CStr;

/// A single ChCtrlList entry. // M2-103 §6.3.2
#[derive(Clone, Debug, PartialEq)]
pub struct ChCtrlEntry {
    pub title: String,
    pub ctrl_type: CtrlType,
    pub ctrl_index: u16,
    pub channel: u8,
    pub min_max: [f64; 2],
    pub default: f64,
}

/// Controller type per MIDI CI ChCtrlList. // M2-103 §6.3.2 Table 29
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtrlType {
    Cc,
    Rpn,
    Nrpn,
    Pnac,
    Pnp,
}

impl CtrlType {
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

/// A complete `ChCtrlList` resource. // ARD §5
#[derive(Clone, Debug, PartialEq)]
pub struct ChCtrlList {
    pub resource: &'static str,
    pub entries: Vec<ChCtrlEntry>,
}

impl ChCtrlList {
    pub fn new() -> Self {
        Self {
            resource: "ChCtrlList",
            entries: Vec::new(),
        }
    }

    /// Build from CLAP plugin params.
    ///
    /// `base_ctrl_index` sets the starting CC number (default: 20 to
    /// avoid well-known CCs 0–19).
    ///
    /// Requires the `clap-ffi` feature.
    ///
    /// # Safety
    ///
    /// `plugin` and `params` must be valid pointers for the duration.
    #[cfg(feature = "clap-ffi")]
    pub unsafe fn from_clap_params(
        plugin: *const clap_sys::plugin::clap_plugin,
        params: *const clap_sys::ext::params::clap_plugin_params,
        param_count: u32,
        base_ctrl_index: u16,
    ) -> Self {
        let mut list = Self::new();
        if plugin.is_null() || params.is_null() {
            return list;
        }
        let get_info = unsafe { (*params).get_info };
        if get_info.is_none() {
            return list;
        }
        let get_info = get_info.unwrap();

        for i in 0..param_count {
            let mut info = core::mem::MaybeUninit::<
                clap_sys::ext::params::clap_param_info,
            >::uninit();
            // clap-sys 0.4: fn(plugin, param_index, param_info)
            let ok = unsafe { get_info(plugin, i, info.as_mut_ptr()) };
            if !ok {
                continue;
            }
            let info = unsafe { info.assume_init() };

            let title = unsafe { CStr::from_ptr(info.name.as_ptr()) }
                .to_string_lossy()
                .into_owned();

            let ctrl_type =
                ctrl_type_from_flags(info.flags, info.min_value, info.max_value);
            let ctrl_index = base_ctrl_index + i as u16;

            list.entries.push(ChCtrlEntry {
                title,
                ctrl_type,
                ctrl_index,
                channel: 1,
                min_max: [info.min_value, info.max_value],
                default: info.default_value,
            });
        }
        list
    }

    /// Serialize to JSON (non-RT).
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
/// Requires the `clap-ffi` feature.
///
/// ## Assignment rules
///
/// 1. `CLAP_PARAM_IS_PER_NOTE` → `Pnac`
/// 2. `CLAP_PARAM_IS_PER_CHANNEL` + wide range → `Pnp`
/// 3. `min==0, max≈127` (7-bit) → `Cc`
/// 4. `min==0, max≤16383` (14-bit) → `Cc`
/// 5. Stepped/enum → `Cc`
/// 6. Default → `Pnac`
#[cfg(feature = "clap-ffi")]
pub fn ctrl_type_from_flags(
    flags: clap_sys::ext::params::clap_param_info_flags,
    min: f64,
    max: f64,
) -> CtrlType {
    use clap_sys::ext::params::{
        CLAP_PARAM_IS_PER_CHANNEL, CLAP_PARAM_IS_PER_NOTE, CLAP_PARAM_IS_STEPPED,
    };

    if flags & CLAP_PARAM_IS_PER_NOTE != 0 {
        return CtrlType::Pnac;
    }
    if flags & CLAP_PARAM_IS_PER_CHANNEL != 0 && (max - min) > 127.0 {
        return CtrlType::Pnp;
    }
    if min == 0.0 && (max - 127.0).abs() < 0.001 {
        return CtrlType::Cc;
    }
    if min == 0.0 && max <= 16383.0 && max > 127.0 {
        return CtrlType::Cc;
    }
    if flags & CLAP_PARAM_IS_STEPPED != 0 {
        return CtrlType::Cc;
    }
    CtrlType::Pnac
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_list_is_empty() {
        let list = ChCtrlList::new();
        assert_eq!(list.resource, "ChCtrlList");
        assert!(list.entries.is_empty());
    }

    #[cfg(feature = "clap-ffi")]
    #[test]
    fn ctrl_type_7bit_is_cc() {
        assert_eq!(ctrl_type_from_flags(0, 0.0, 127.0), CtrlType::Cc);
    }

    #[cfg(feature = "clap-ffi")]
    #[test]
    fn ctrl_type_14bit_is_cc() {
        assert_eq!(ctrl_type_from_flags(0, 0.0, 16383.0), CtrlType::Cc);
    }

    #[cfg(feature = "clap-ffi")]
    #[test]
    fn ctrl_type_stepped_is_cc() {
        use clap_sys::ext::params::CLAP_PARAM_IS_STEPPED;
        assert_eq!(
            ctrl_type_from_flags(CLAP_PARAM_IS_STEPPED, 0.0, 10.0),
            CtrlType::Cc,
        );
    }

    #[cfg(feature = "clap-ffi")]
    #[test]
    fn ctrl_type_per_note_is_pnac() {
        use clap_sys::ext::params::CLAP_PARAM_IS_PER_NOTE;
        assert_eq!(
            ctrl_type_from_flags(CLAP_PARAM_IS_PER_NOTE, 0.0, 1.0),
            CtrlType::Pnac,
        );
    }

    #[cfg(feature = "clap-ffi")]
    #[test]
    fn ctrl_type_default_is_pnac() {
        assert_eq!(ctrl_type_from_flags(0, -24.0, 24.0), CtrlType::Pnac);
    }

    #[test]
    fn json_smoke() {
        let mut list = ChCtrlList::new();
        list.entries.push(ChCtrlEntry {
            title: "Gain".into(),
            ctrl_type: CtrlType::Pnac,
            ctrl_index: 20,
            channel: 1,
            min_max: [0.0, 1.0],
            default: 0.8,
        });
        let json = list.to_json();
        assert!(json.contains("ChCtrlList"));
        assert!(json.contains("Gain"));
        assert!(json.contains("pnac"));
    }
}
