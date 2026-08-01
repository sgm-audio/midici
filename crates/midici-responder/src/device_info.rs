//! DeviceInfo resource — MIDI-CI Property Exchange standard resource.
//!
//! Per M2-103 §6.2.1, `DeviceInfo` provides the mandatory identification
//! fields for a MIDI-CI device: manufacturer, family, model, version,
//! and serial number.
//!
//! This is the **first** resource any CI controller reads after Discovery.
//! Built on the main thread; serialized to JSON on the control thread.

/// The `DeviceInfo` resource — mandatory per M2-103 §6.2.1.
///
/// Fields correspond to the MIDI-CI DeviceInfo JSON schema:
///
/// | Field          | JSON key         | Example         |
/// |----------------|------------------|-----------------|
/// | `manufacturer` | `"manufacturer"` | `"SGM Studios"` |
/// | `family`       | `"family"`       | `"midici"`      |
/// | `model`        | `"model"`        | `"AutoProp"`    |
/// | `version`      | `"version"`      | `"0.1.0"`       |
/// | `serial_number`| `"serialNumber"` | `"0001"`        |
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Manufacturer name (matches SysEx ID in Discovery).
    pub manufacturer: String,
    /// Device family / product line.
    pub family: String,
    /// Specific model name.
    pub model: String,
    /// Software/firmware version string.
    pub version: String,
    /// Unique serial number (or UUID).
    pub serial_number: String,
}

impl DeviceInfo {
    /// Resource identifier — always `"DeviceInfo"`.
    pub const RESOURCE_ID: &'static str = "DeviceInfo";

    /// Serialize to JSON per MIDI-CI PE spec. Called on the control thread.
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"resource":"DeviceInfo","manufacturer":{:?},"family":{:?},"model":{:?},"version":{:?},"serialNumber":{:?}}}"#,
            self.manufacturer, self.family, self.model, self.version, self.serial_number,
        )
    }
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            manufacturer: String::new(),
            family: String::new(),
            model: String::new(),
            version: String::new(),
            serial_number: String::new(),
        }
    }
}
