#![allow(unsafe_code)]

//! CLAP auto-property demo — ChCtrlList construction + ring exercise.
//!
//! Demonstrates building a `ChCtrlList` from plugin parameters and
//! exercising the SPSC ring. The full CLAP FFI plugin surface
//! (`clap_entry`, `clap_plugin_factory`, audio processing) requires
//! `clap-sys` and is verified via `clap-validator` in the `midici-dev`
//! container. See `docs/PROGRESS.md` Phase 6.
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

use midici_transport_clap::chctrllist::{ChCtrlList, ChCtrlEntry, CtrlType};
use midici_transport_clap::ring::{Ring, Producer, Consumer};
use midici_responder::{DeviceInfo, ResourceList};

const PARAM_COUNT: usize = 5;
const PARAM_NAMES: &[&str] = &["Gain", "Bass", "Mid", "Treble", "Presence"];
const PARAM_MINS: &[f64] = &[0.0, -24.0, -24.0, -24.0, 0.0];
const PARAM_MAXS: &[f64] = &[1.0, 24.0, 24.0, 24.0, 1.0];
const PARAM_DEFAULTS: &[f64] = &[0.8, 0.0, 0.0, 0.0, 0.5];

/// Build the ChCtrlList from the plugin param table.
///
/// Assignment per ARD §5: wide continuous params → `Pnac`,
/// sequential `ctrlIndex` starting at 20.
pub fn build_ch_ctrl_list() -> ChCtrlList {
    let mut list = ChCtrlList::new();
    for i in 0..PARAM_COUNT {
        list.entries.push(ChCtrlEntry {
            title: PARAM_NAMES[i].to_string(),
            ctrl_type: CtrlType::Pnac,
            ctrl_index: 20 + i as u16,
            channel: 1,
            min_max: [PARAM_MINS[i], PARAM_MAXS[i]],
            default: PARAM_DEFAULTS[i],
        });
    }
    list
}

/// Build DeviceInfo for the autoprop plugin.
pub fn build_device_info() -> DeviceInfo {
    DeviceInfo {
        manufacturer: "SGM Studios".into(),
        family: "midici".into(),
        model: "AutoProp Demo".into(),
        version: "0.1.0".into(),
        serial_number: "0001".into(),
    }
}

/// Build ResourceList with the standard trio.
pub fn build_resource_list() -> ResourceList {
    ResourceList::standard()
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ch_ctrl_list_has_5_entries() {
        let list = build_ch_ctrl_list();
        assert_eq!(list.entries.len(), 5);
        assert_eq!(list.entries[0].title, "Gain");
        assert_eq!(list.entries[0].ctrl_type, CtrlType::Pnac);
        assert_eq!(list.entries[0].ctrl_index, 20);
        assert_eq!(list.entries[4].title, "Presence");
        assert_eq!(list.entries[4].ctrl_index, 24);
    }

    #[test]
    fn ch_ctrl_list_json() {
        let list = build_ch_ctrl_list();
        let json = list.to_json();
        assert!(json.contains("ChCtrlList"));
        assert!(json.contains("Gain"));
        assert!(json.contains("Presence"));
        assert!(json.contains("pnac"));
    }

    #[test]
    fn device_info_json() {
        let info = build_device_info();
        let json = info.to_json();
        assert!(json.contains("DeviceInfo"));
        assert!(json.contains("SGM Studios"));
        assert!(json.contains("AutoProp Demo"));
    }

    #[test]
    fn resource_list_json() {
        let list = build_resource_list();
        let json = list.to_json();
        assert!(json.contains("ResourceList"));
        assert!(json.contains("ChCtrlList"));
        assert!(json.contains("DeviceInfo"));
    }

    #[test]
    fn ring_exercise() {
        let ring: Ring<64, 512> = Ring::new();
        let mut prod = unsafe { Producer::new(&ring) };
        let mut cons = unsafe { Consumer::new(&ring) };

        // Simulate a SysEx message through the ring.
        let body = [0x7E, 0x7F, 0x0D, 0x70, 0x02];
        assert!(prod.push(&body));

        let popped = cons.pop().unwrap();
        let len = popped.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        assert_eq!(&popped[..len], &body[..]);
    }
}
