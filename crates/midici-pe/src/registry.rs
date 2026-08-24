//! Resource registry. // ARD §5

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use midici_core::DeviceIdentity;

use crate::resource::{
    DeviceInfoResource, Payload, PeQuery, PropertyResource, ResourceListResource,
};
use crate::status::{PeResult, PeStatus};

/// Owns registered [`PropertyResource`] handlers.
pub struct ResourceRegistry {
    resources: Vec<Box<dyn PropertyResource>>,
}

impl ResourceRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
        }
    }

    /// Registry with DeviceInfo + ResourceList (ResourceList lists both). // ARD §5
    pub fn with_device_info(identity: DeviceIdentity) -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(DeviceInfoResource::new(identity)));
        // ResourceList is added after we know names — rebuild list entry.
        let names = reg.resource_names();
        // Include ResourceList itself in the list.
        let mut names = names;
        if !names.iter().any(|n| n == "ResourceList") {
            names.push(String::from("ResourceList"));
        }
        reg.register(Box::new(ResourceListResource::new(names)));
        reg
    }

    pub fn register(&mut self, resource: Box<dyn PropertyResource>) {
        // Replace existing same name.
        if let Some(i) = self
            .resources
            .iter()
            .position(|r| r.resource() == resource.resource())
        {
            self.resources[i] = resource;
        } else {
            self.resources.push(resource);
        }
    }

    pub fn resource_names(&self) -> Vec<String> {
        self.resources
            .iter()
            .map(|r| String::from(r.resource()))
            .collect()
    }

    /// Get by resource name.
    pub fn get(&self, name: &str, query: &PeQuery) -> PeResult<Payload> {
        let r = self
            .resources
            .iter()
            .find(|r| r.resource() == name)
            .ok_or(PeStatus::NotFound)?;
        r.get(query)
    }

    /// Replace or insert a resource by name.
    pub fn resources_mut(&mut self) -> &mut Vec<Box<dyn PropertyResource>> {
        &mut self.resources
    }
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
