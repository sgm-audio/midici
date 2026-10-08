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
    ///
    /// `ResourceList` stays in sync automatically: every later
    /// [`Self::register`] refreshes it, so resources added after this call
    /// (e.g. `ChCtrlList`) appear in the list.
    pub fn with_device_info(identity: DeviceIdentity) -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(DeviceInfoResource::new(identity)));
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
        self.refresh_resource_list();
    }

    /// Rebuild the `ResourceList` entry from the current names (including
    /// itself). Called on every registration so the list is never stale.
    /// // M2-103 §14 / ARD §5
    fn refresh_resource_list(&mut self) {
        let mut names = self
            .resources
            .iter()
            .map(|r| String::from(r.resource()))
            .filter(|n| n != "ResourceList")
            .collect::<Vec<String>>();
        names.push(String::from("ResourceList"));
        if let Some(i) = self
            .resources
            .iter()
            .position(|r| r.resource() == "ResourceList")
        {
            self.resources[i] = Box::new(ResourceListResource::new(names));
        } else {
            self.resources.push(Box::new(ResourceListResource::new(names)));
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

    /// Set by resource name. Default trait policy is read-only → 405.
    /// // ARD §5; M2-103 §7.4.1
    pub fn set(&mut self, name: &str, query: &PeQuery, body: &[u8]) -> PeResult<()> {
        let r = self
            .resources
            .iter_mut()
            .find(|r| r.resource() == name)
            .ok_or(PeStatus::NotFound)?;
        r.set(query, body)
    }

    /// True when `name` exists and declares itself subscribable. // M2-103 §11
    pub fn subscribable(&self, name: &str) -> bool {
        self.resources
            .iter()
            .find(|r| r.resource() == name)
            .map(|r| r.subscribable())
            .unwrap_or(false)
    }

    /// True when `name` exists (any writability).
    pub fn contains(&self, name: &str) -> bool {
        self.resources.iter().any(|r| r.resource() == name)
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
