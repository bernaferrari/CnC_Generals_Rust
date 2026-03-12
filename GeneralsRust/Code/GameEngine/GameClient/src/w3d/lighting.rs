//! W3D Advanced Lighting System

use super::renderer::W3DLightData;
use std::sync::Arc;
use ultraviolet::{Vec3, Vec4};
use wgpu::{Buffer, Device, Queue};

/// Light types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DLightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
}

/// Light configuration
#[derive(Debug, Clone)]
pub struct W3DLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub light_type: W3DLightType,
    pub spot_inner_angle: f32,
    pub spot_outer_angle: f32,
    pub cast_shadows: bool,
    pub shadow_map_index: i32,
}

impl W3DLight {
    /// Create a directional light (like sun)
    pub fn directional(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 0.0),
            direction: direction.normalized(),
            color,
            intensity,
            range: f32::INFINITY,
            light_type: W3DLightType::Directional,
            spot_inner_angle: 0.0,
            spot_outer_angle: 0.0,
            cast_shadows: true,
            shadow_map_index: -1,
        }
    }

    /// Create a point light
    pub fn point(position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            position,
            direction: Vec3::new(0.0, 0.0, 0.0),
            color,
            intensity,
            range,
            light_type: W3DLightType::Point,
            spot_inner_angle: 0.0,
            spot_outer_angle: 0.0,
            cast_shadows: false,
            shadow_map_index: -1,
        }
    }

    /// Create a spot light
    pub fn spot(
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        Self {
            position,
            direction: direction.normalized(),
            color,
            intensity,
            range,
            light_type: W3DLightType::Spot,
            spot_inner_angle: inner_angle,
            spot_outer_angle: outer_angle,
            cast_shadows: true,
            shadow_map_index: -1,
        }
    }

    /// Convert to GPU data format
    pub fn to_gpu_data(&self) -> W3DLightData {
        W3DLightData {
            position: [self.position.x, self.position.y, self.position.z],
            light_type: self.light_type as u32,
            color_intensity: [self.color.x, self.color.y, self.color.z, self.intensity],
            direction: [self.direction.x, self.direction.y, self.direction.z],
            range: self.range,
            spot_angles: [self.spot_inner_angle, self.spot_outer_angle],
            shadow_index: self.shadow_map_index,
            _padding: 0,
        }
    }
}

/// Advanced lighting manager with shadow mapping
pub struct W3DLightManager {
    lights: Vec<W3DLight>,
    max_lights: u32,
    dirty: bool,
    shadow_casters: Vec<usize>, // Indices of lights that cast shadows
    ambient_color: Vec3,
    ambient_intensity: f32,
}

impl W3DLightManager {
    pub fn new(max_lights: u32) -> Self {
        Self {
            lights: Vec::new(),
            max_lights,
            dirty: false,
            shadow_casters: Vec::new(),
            ambient_color: Vec3::new(0.2, 0.2, 0.3),
            ambient_intensity: 0.1,
        }
    }

    /// Add a light to the scene
    pub fn add_light(&mut self, light: W3DLight) -> Result<usize, String> {
        if self.lights.len() >= self.max_lights as usize {
            return Err("Maximum number of lights exceeded".to_string());
        }

        let index = self.lights.len();

        // Assign shadow map index if this light casts shadows
        let mut new_light = light;
        if new_light.cast_shadows && self.shadow_casters.len() < 16 {
            new_light.shadow_map_index = self.shadow_casters.len() as i32;
            self.shadow_casters.push(index);
        }

        self.lights.push(new_light);
        self.dirty = true;
        Ok(index)
    }

    /// Remove a light
    pub fn remove_light(&mut self, index: usize) -> Result<(), String> {
        if index >= self.lights.len() {
            return Err("Light index out of bounds".to_string());
        }

        // Remove from shadow casters if needed
        if let Some(shadow_index) = self.shadow_casters.iter().position(|&x| x == index) {
            self.shadow_casters.remove(shadow_index);
            // Reassign shadow map indices
            for (i, &light_index) in self.shadow_casters.iter().enumerate() {
                self.lights[light_index].shadow_map_index = i as i32;
            }
        }

        self.lights.remove(index);
        self.dirty = true;
        Ok(())
    }

    /// Update light properties
    pub fn update_light(&mut self, index: usize, light: W3DLight) -> Result<(), String> {
        if index >= self.lights.len() {
            return Err("Light index out of bounds".to_string());
        }

        self.lights[index] = light;
        self.dirty = true;
        Ok(())
    }

    /// Get light at index
    pub fn get_light(&self, index: usize) -> Option<&W3DLight> {
        self.lights.get(index)
    }

    /// Get all shadow casting lights
    pub fn get_shadow_casters(&self) -> Vec<&W3DLight> {
        self.shadow_casters
            .iter()
            .filter_map(|&index| self.lights.get(index))
            .collect()
    }

    /// Set ambient lighting
    pub fn set_ambient(&mut self, color: Vec3, intensity: f32) {
        self.ambient_color = color;
        self.ambient_intensity = intensity;
        self.dirty = true;
    }

    /// Update GPU buffer with current light data
    pub fn update(&mut self, queue: &Queue, buffer: &Buffer) {
        if self.dirty {
            let gpu_lights: Vec<W3DLightData> = self
                .lights
                .iter()
                .map(|light| light.to_gpu_data())
                .collect();

            // Pad to required size
            let mut padded_lights = gpu_lights;
            padded_lights.resize(
                self.max_lights as usize,
                W3DLightData {
                    position: [0.0; 3],
                    light_type: 0,
                    color_intensity: [0.0; 4], // Zero intensity = disabled
                    direction: [0.0; 3],
                    range: 0.0,
                    spot_angles: [0.0; 2],
                    shadow_index: -1,
                    _padding: 0,
                },
            );

            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&padded_lights));
            self.dirty = false;
        }
    }

    /// Get number of active lights
    pub fn active_lights(&self) -> u32 {
        self.lights.len() as u32
    }

    /// Get ambient lighting data
    pub fn ambient_data(&self) -> (Vec3, f32) {
        (self.ambient_color, self.ambient_intensity)
    }

    /// Clear all lights
    pub fn clear(&mut self) {
        self.lights.clear();
        self.shadow_casters.clear();
        self.dirty = true;
    }
}
