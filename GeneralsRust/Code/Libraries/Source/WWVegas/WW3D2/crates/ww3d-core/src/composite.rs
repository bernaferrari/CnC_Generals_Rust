//! Composite Objects System
//!
//! This module handles composite objects made up of multiple sub-objects.

use crate::core::error::{W3dError, W3DResult};
use glam::Mat4;

/// Composite object class
pub struct CompositeObject {
    name: String,
    sub_objects: Vec<SubObject>,
    transform: Mat4,
}

impl CompositeObject {
    /// Create a new composite object
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sub_objects: Vec::new(),
            transform: Mat4::IDENTITY,
        }
    }

    /// Add sub-object
    pub fn add_sub_object(&mut self, sub_object: SubObject) {
        self.sub_objects.push(sub_object);
    }

    /// Remove sub-object by name
    pub fn remove_sub_object(&mut self, name: &str) {
        self.sub_objects.retain(|obj| obj.name != name);
    }

    /// Get sub-object by name
    pub fn get_sub_object(&self, name: &str) -> Option<&SubObject> {
        self.sub_objects.iter().find(|obj| obj.name == name)
    }

    /// Get all sub-objects
    pub fn sub_objects(&self) -> &[SubObject] {
        &self.sub_objects
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    /// Get transform
    pub fn transform(&self) -> Mat4 {
        self.transform
    }

    /// Get sub-object count
    pub fn sub_object_count(&self) -> usize {
        self.sub_objects.len()
    }
}

/// Sub-object within a composite
#[derive(Clone, Debug)]
pub struct SubObject {
    pub name: String,
    pub local_transform: Mat4,
    pub world_transform: Mat4,
    pub visible: bool,
}

impl SubObject {
    /// Create a new sub-object
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            local_transform: Mat4::IDENTITY,
            world_transform: Mat4::IDENTITY,
            visible: true,
        }
    }

    /// Set local transform
    pub fn set_local_transform(&mut self, transform: Mat4) {
        self.local_transform = transform;
    }

    /// Update world transform
    pub fn update_world_transform(&mut self, parent_transform: Mat4) {
        self.world_transform = parent_transform * self.local_transform;
    }
}

/// Composite object manager
pub struct CompositeObjectManager {
    composites: Vec<CompositeObject>,
}

impl CompositeObjectManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            composites: Vec::new(),
        }
    }

    /// Add composite object
    pub fn add_composite(&mut self, composite: CompositeObject) {
        self.composites.push(composite);
    }

    /// Get composite by name
    pub fn get_composite(&self, name: &str) -> Option<&CompositeObject> {
        self.composites.iter().find(|comp| comp.name == name)
    }

    /// Get mutable composite by name
    pub fn get_composite_mut(&mut self, name: &str) -> Option<&mut CompositeObject> {
        self.composites.iter_mut().find(|comp| comp.name == name)
    }

    /// Remove composite by name
    pub fn remove_composite(&mut self, name: &str) {
        self.composites.retain(|comp| comp.name != name);
    }

    /// Update all world transforms
    pub fn update_world_transforms(&mut self) {
        for composite in &mut self.composites {
            for sub_object in &mut composite.sub_objects {
                sub_object.update_world_transform(composite.transform);
            }
        }
    }
}

