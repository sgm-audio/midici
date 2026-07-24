//! Property resource trait and shipped DeviceInfo / ResourceList. // ARD §5

use alloc::string::String;
use alloc::vec::Vec;

use midici_core::DeviceIdentity;
use serde::Serialize;

use crate::status::{PeResult, PeStatus};

/// Query parameters for a Get (resource name already selected by registry).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeQuery {
    pub res_id: Option<String>,
}

/// Successful Get payload (JSON property body bytes, 7-bit ASCII).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    pub body: Vec<u8>,
}

/// Resource handler. // ARD §5
pub trait PropertyResource: Send {
    fn resource(&self) -> &str;
    fn get(&self, req: &PeQuery) -> PeResult<Payload>;
    fn set(&mut self, _req: &PeQuery, _body: &[u8]) -> PeResult<()> {
        Err(PeStatus::NotAllowed)
    }
    fn subscribable(&self) -> bool {
        false
    }
}

/// DeviceInfo resource built from [`DeviceIdentity`]. // M2-103 §8.2 examples / ARD §5
#[derive(Clone, Debug)]
pub struct DeviceInfoResource {
    pub identity: DeviceIdentity,
    /// When true, Get returns [`PeStatus::Forbidden`].
    pub forbidden: bool,
    /// When true, Get returns [`PeStatus::NotAllowed`].
    pub not_allowed: bool,
}

impl DeviceInfoResource {
    pub fn new(identity: DeviceIdentity) -> Self {
        Self {
            identity,
            forbidden: false,
            not_allowed: false,
        }
    }
}

#[derive(Serialize)]
struct DeviceInfoJson {
    #[serde(rename = "manufacturerId")]
    manufacturer_id: [u8; 3],
    #[serde(rename = "familyId")]
    family_id: [u8; 2],
    #[serde(rename = "modelId")]
    model_id: [u8; 2],
    #[serde(rename = "versionId")]
    version_id: [u8; 4],
}

impl PropertyResource for DeviceInfoResource {
    fn resource(&self) -> &str {
        "DeviceInfo"
    }

    fn get(&self, _req: &PeQuery) -> PeResult<Payload> {
        if self.forbidden {
            return Err(PeStatus::Forbidden);
        }
        if self.not_allowed {
            return Err(PeStatus::NotAllowed);
        }
        let id = &self.identity;
        let doc = DeviceInfoJson {
            manufacturer_id: id.manufacturer,
            family_id: [(id.family & 0xFF) as u8, ((id.family >> 8) & 0xFF) as u8],
            model_id: [(id.model & 0xFF) as u8, ((id.model >> 8) & 0xFF) as u8],
            version_id: id.software_revision,
        };
        let body = serde_json::to_vec(&doc).map_err(|_| PeStatus::BadRequest)?;
        if body.iter().any(|b| *b > 0x7F) {
            return Err(PeStatus::BadRequest);
        }
        Ok(Payload { body })
    }
}

/// Auto-generated ResourceList from registered resource names. // M2-103 §14 / ARD §5
#[derive(Clone, Debug)]
pub struct ResourceListResource {
    pub names: Vec<String>,
}

impl ResourceListResource {
    pub fn new(names: Vec<String>) -> Self {
        Self { names }
    }
}

#[derive(Serialize)]
struct ResourceListEntry<'a> {
    resource: &'a str,
}

impl PropertyResource for ResourceListResource {
    fn resource(&self) -> &str {
        "ResourceList"
    }

    fn get(&self, _req: &PeQuery) -> PeResult<Payload> {
        let entries: Vec<ResourceListEntry<'_>> = self
            .names
            .iter()
            .map(|n| ResourceListEntry {
                resource: n.as_str(),
            })
            .collect();
        let body = serde_json::to_vec(&entries).map_err(|_| PeStatus::BadRequest)?;
        Ok(Payload { body })
    }
}
