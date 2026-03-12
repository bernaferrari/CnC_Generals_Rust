//! LOD Manager - Integrated into 3D Renderer
//!
//! This module coordinates LOD calculations for the 3D rendering pipeline.

use super::lod_calculator::{LodCalculationParams, LodCalculator};
use super::lod_object::LodObject;
use super::prototype_loader;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use ww3d_assets::{assets::AssetManager, prototypes::LodModelPrototype};

/// LOD manager integrated with the 3D renderer
#[derive(Debug)]
pub struct LodManager {
    /// LOD calculator
    calculator: LodCalculator,

    /// Managed LOD objects
    objects: HashMap<u64, LodObject>,

    /// Index of objects by label
    name_index: HashMap<String, u64>,

    /// Camera position
    camera_position: Vec3,

    /// Camera projection matrix
    camera_projection: Mat4,

    /// Screen size
    screen_size: (u32, u32),

    /// Next object ID
    next_object_id: u64,
}

impl LodManager {
    /// Create a new LOD manager
    pub fn new() -> Self {
        Self {
            calculator: LodCalculator::new(),
            objects: HashMap::new(),
            name_index: HashMap::new(),
            camera_position: Vec3::ZERO,
            camera_projection: Mat4::IDENTITY,
            screen_size: (1920, 1080),
            next_object_id: 1,
        }
    }

    /// Add an LOD object
    pub fn add_object(&mut self, mut object: LodObject) -> u64 {
        let id = self.next_object_id;
        self.next_object_id += 1;
        object.id = id;
        if !object.label.is_empty() {
            self.name_index.insert(object.label.clone(), id);
        }
        self.objects.insert(id, object);
        id
    }

    /// Remove an LOD object
    pub fn remove_object(&mut self, id: u64) -> bool {
        if let Some(object) = self.objects.remove(&id) {
            if !object.label.is_empty() {
                self.name_index.remove(&object.label);
            }
            true
        } else {
            false
        }
    }

    /// Update camera parameters
    pub fn update_camera(&mut self, position: Vec3, projection: Mat4, screen_size: (u32, u32)) {
        self.camera_position = position;
        self.camera_projection = projection;
        self.screen_size = screen_size;
    }

    /// Update LOD calculations for all objects
    pub fn update(&mut self) {
        for object in self.objects.values_mut() {
            if !object.lod_enabled || object.lod_levels.is_empty() {
                continue;
            }

            let bounding_radius = object.lod_levels[0]
                .mesh
                .as_ref()
                .map(|mesh| mesh.bounding_radius())
                .unwrap_or(1.0);

            let new_lod_level = self.calculator.calculate_lod_level(
                object.position,
                self.camera_position,
                &self.camera_projection,
                self.screen_size,
                bounding_radius,
                object.current_lod_level,
            );

            if new_lod_level != object.current_lod_level {
                object.set_lod_level(new_lod_level);
            }
        }
    }

    /// Get an LOD object
    pub fn get_object(&self, id: u64) -> Option<&LodObject> {
        self.objects.get(&id)
    }

    /// Get visible objects
    pub fn get_visible_objects(&self) -> Vec<&LodObject> {
        self.objects
            .values()
            .filter(|obj| obj.should_render())
            .collect()
    }

    /// Look up an LOD object by its label.
    pub fn get_object_by_label(&self, label: &str) -> Option<&LodObject> {
        self.name_index
            .get(label)
            .and_then(|id| self.objects.get(id))
    }

    /// Register an LOD model prototype with the manager.
    pub fn add_model_from_prototype(
        &mut self,
        prototype: &LodModelPrototype,
        assets: &AssetManager,
    ) -> Option<u64> {
        if self.name_index.contains_key(&prototype.name) {
            return self.name_index.get(&prototype.name).copied();
        }

        let lod_object = prototype_loader::build_lod_object_from_prototype(prototype, assets)?;
        Some(self.add_object(lod_object))
    }

    /// Set LOD calculation parameters
    pub fn set_calculation_params(&mut self, params: LodCalculationParams) {
        self.calculator.update_params(params);
    }

    /// Get object count
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
}
