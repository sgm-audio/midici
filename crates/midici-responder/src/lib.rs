//! Ergonomic MIDI-CI responder façade (poll-driven timers, rings, event pump).
//!
//! See `docs/ARD-001.md` §2–§3, §5.
//!
//! This crate exposes the resource types `DeviceInfo`, `ResourceList`, and
//! (via `midici-transport-clap`) `ChCtrlList` — the three standard MIDI-CI
//! Property Exchange resources for the responder role.

pub mod device_info;
pub mod resource_list;

pub use device_info::DeviceInfo;
pub use resource_list::ResourceList;

/// Package version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name from `Cargo.toml`.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_nonempty() {
        assert!(!crate::VERSION.is_empty());
    }

    #[test]
    fn device_info_serializes() {
        let info = crate::DeviceInfo {
            manufacturer: "SGM".into(),
            family: "midici".into(),
            model: "AutoProp".into(),
            version: "0.1.0".into(),
            serial_number: "0001".into(),
        };
        let json = info.to_json();
        assert!(json.contains("DeviceInfo"));
        assert!(json.contains("SGM"));
    }

    #[test]
    fn resource_list_serializes() {
        let list = crate::ResourceList {
            resources: vec!["DeviceInfo".into(), "ResourceList".into(), "ChCtrlList".into()],
        };
        let json = list.to_json();
        assert!(json.contains("ResourceList"));
        assert!(json.contains("ChCtrlList"));
    }
}
