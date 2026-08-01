//! ResourceList resource — MIDI-CI Property Exchange standard resource.
//!
//! Per M2-103 §6.2.2, `ResourceList` enumerates all PE resources
//! available on the device. Controllers query this after Discovery to
//! know which resources to GET.
//!
//! Built on the main thread; serialized to JSON on the control thread.

/// The `ResourceList` resource — mandatory per M2-103 §6.2.2.
///
/// Lists all resource identifiers available on this device.
/// For the `clap-autoprop` demo, the list is:
///
/// ```text
/// ["DeviceInfo", "ResourceList", "ChCtrlList"]
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceList {
    /// Resource identifiers available on this device.
    pub resources: Vec<String>,
}

impl ResourceList {
    /// Resource identifier — always `"ResourceList"`.
    pub const RESOURCE_ID: &'static str = "ResourceList";

    /// Create a ResourceList with the standard trio:
    /// `DeviceInfo`, `ResourceList`, `ChCtrlList`.
    pub fn standard() -> Self {
        Self {
            resources: vec![
                "DeviceInfo".into(),
                "ResourceList".into(),
                "ChCtrlList".into(),
            ],
        }
    }

    /// Serialize to JSON per MIDI-CI PE spec. Called on the control thread.
    pub fn to_json(&self) -> String {
        let mut json = String::from(r#"{"resource":"ResourceList","resources":["#);
        for (i, res) in self.resources.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!("{:?}", res));
        }
        json.push_str("]}");
        json
    }
}

impl Default for ResourceList {
    fn default() -> Self {
        Self::standard()
    }
}
