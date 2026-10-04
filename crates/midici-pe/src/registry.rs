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
    auto_resource_list: bool,
}

impl ResourceRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            auto_resource_list: false,
        }
    }

    /// Registry with DeviceInfo + a ResourceList refreshed by [`Self::register`]. // ARD §5
    pub fn with_device_info(identity: DeviceIdentity) -> Self {
        let mut reg = Self::new();
        reg.auto_resource_list = true;
        reg.register(Box::new(DeviceInfoResource::new(identity)));
        reg
    }

    /// Register or replace a resource by name.
    ///
    /// Registries created by [`Self::with_device_info`] also refresh their
    /// generated ResourceList after each registration.
    pub fn register(&mut self, resource: Box<dyn PropertyResource>) {
        let name = String::from(resource.resource());
        // Replace existing same name.
        if let Some(i) = self
            .resources
            .iter()
            .position(|r| r.resource() == name.as_str())
        {
            self.resources[i] = resource;
        } else {
            self.resources.push(resource);
        }
        if self.auto_resource_list && name != "ResourceList" {
            self.refresh_resource_list();
        }
    }

    fn refresh_resource_list(&mut self) {
        let index = self
            .resources
            .iter()
            .position(|r| r.resource() == "ResourceList");
        let mut names = self.resource_names();
        if index.is_none() {
            names.push(String::from("ResourceList"));
        }
        let list = Box::new(ResourceListResource::new(names));
        if let Some(index) = index {
            self.resources[index] = list;
        } else {
            self.resources.push(list);
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

    /// Mutable escape hatch for direct registry edits.
    ///
    /// Unlike [`Self::register`], direct edits do not refresh the generated
    /// ResourceList in registries created by [`Self::with_device_info`].
    pub fn resources_mut(&mut self) -> &mut Vec<Box<dyn PropertyResource>> {
        &mut self.resources
    }
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CustomResource;

    impl PropertyResource for CustomResource {
        fn resource(&self) -> &str {
            "Custom"
        }

        fn get(&self, _req: &PeQuery) -> PeResult<Payload> {
            Ok(Payload { body: b"null".to_vec() })
        }
    }

    fn identity() -> DeviceIdentity {
        DeviceIdentity {
            manufacturer: [0x43, 0, 0],
            family: 1,
            model: 2,
            software_revision: [1, 0, 0, 0],
        }
    }

    #[test]
    fn generated_resource_list_tracks_registered_resources() {
        let mut registry = ResourceRegistry::with_device_info(identity());
        registry.register(Box::new(CustomResource));

        let list = registry
            .get("ResourceList", &PeQuery::default())
            .unwrap();
        assert_eq!(
            list.body.as_slice(),
            br#"[{"resource":"DeviceInfo"},{"resource":"ResourceList"},{"resource":"Custom"}]"#
        );
    }
}
