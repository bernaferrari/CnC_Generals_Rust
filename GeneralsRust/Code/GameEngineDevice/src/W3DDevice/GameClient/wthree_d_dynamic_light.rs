//! W3DDynamicLight Module - Complete Dynamic Lighting System
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/W3DDynamicLight.cpp
//!
//! This module provides dynamic point and directional lighting for the W3D engine,
//! supporting light decay, color fading, and range attenuation.

use cgmath::{Vector3, Zero};
use std::fmt;

/// Maximum number of lights in the light environment
pub const MAX_LIGHTS: usize = 8;

/// Light type enumeration matching C++ LightClass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    Point = 0,
    Directional = 1,
    Spot = 2,
    Ambient = 3,
}

/// Dynamic light implementation matching C++ W3DDynamicLight
#[derive(Debug, Clone)]
pub struct W3DDynamicLight {
    /// Type of light
    pub light_type: LightType,

    /// Whether the light is enabled
    pub enabled: bool,
    /// Previous enabled state (for transitions)
    pub prior_enable: bool,

    // Position for point/spot lights
    pub position: Vector3<f32>,

    // Direction for directional/spot lights
    pub direction: Vector3<f32>,

    // Light colors
    pub ambient: Vector3<f32>,
    pub diffuse: Vector3<f32>,
    pub specular: Vector3<f32>,

    // Target colors for fading
    pub target_ambient: Vector3<f32>,
    pub target_diffuse: Vector3<f32>,

    // Attenuation parameters
    pub far_atten_start: f32,
    pub far_atten_end: f32,
    pub target_range: f32,

    // Decay parameters
    pub decay_frame_count: u32,
    pub cur_decay_frame_count: u32,
    pub increase_frame_count: u32,
    pub cur_increase_frame_count: u32,

    // Decay flags
    pub decay_range: bool,
    pub decay_color: bool,

    // Spot light parameters
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,

    // Intensity multiplier
    pub intensity: f32,
}

impl Default for W3DDynamicLight {
    fn default() -> Self {
        Self::new(LightType::Point)
    }
}

impl W3DDynamicLight {
    /// Create a new dynamic light of the specified type
    pub fn new(light_type: LightType) -> Self {
        Self {
            light_type,
            enabled: true,
            prior_enable: false,
            position: Vector3::zero(),
            direction: Vector3::new(0.0, -1.0, 0.0),
            ambient: Vector3::new(0.0, 0.0, 0.0),
            diffuse: Vector3::new(1.0, 1.0, 1.0),
            specular: Vector3::new(1.0, 1.0, 1.0),
            target_ambient: Vector3::zero(),
            target_diffuse: Vector3::new(1.0, 1.0, 1.0),
            far_atten_start: 1.0,
            far_atten_end: 100.0,
            target_range: 100.0,
            decay_frame_count: 0,
            cur_decay_frame_count: 0,
            increase_frame_count: 0,
            cur_increase_frame_count: 0,
            decay_range: false,
            decay_color: false,
            inner_cone_angle: 0.0,
            outer_cone_angle: 45.0,
            intensity: 1.0,
        }
    }

    /// Create a new point light
    pub fn point() -> Self {
        Self::new(LightType::Point)
    }

    /// Create a new directional light
    pub fn directional() -> Self {
        Self::new(LightType::Directional)
    }

    /// Create a new spot light
    pub fn spot() -> Self {
        Self::new(LightType::Spot)
    }

    /// Check if light is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable the light
    pub fn set_enabled(&mut self, enabled: bool) {
        self.prior_enable = self.enabled;
        self.enabled = enabled;
    }

    /// Set light position
    pub fn set_position(&mut self, pos: Vector3<f32>) {
        self.position = pos;
    }

    /// Set light direction (for directional/spot lights)
    pub fn set_direction(&mut self, dir: Vector3<f32>) {
        self.direction = dir;
    }

    /// Set ambient color
    pub fn set_ambient(&mut self, color: Vector3<f32>) {
        self.ambient = color;
    }

    /// Set diffuse color
    pub fn set_diffuse(&mut self, color: Vector3<f32>) {
        self.diffuse = color;
    }

    /// Set specular color
    pub fn set_specular(&mut self, color: Vector3<f32>) {
        self.specular = color;
    }

    /// Set attenuation range
    pub fn set_range(&mut self, start: f32, end: f32) {
        self.far_atten_start = start;
        self.far_atten_end = end;
        self.target_range = end;
    }

    /// Set frame fade parameters (matching C++ setFrameFade)
    pub fn set_frame_fade(&mut self, frame_increase_time: u32, decay_frame_time: u32) {
        self.decay_frame_count = decay_frame_time;
        self.cur_decay_frame_count = decay_frame_time;
        self.increase_frame_count = frame_increase_time;
        self.cur_increase_frame_count = frame_increase_time;
        self.target_ambient = self.ambient;
        self.target_diffuse = self.diffuse;
        self.target_range = self.far_atten_end;
    }

    /// Enable range decay
    pub fn set_decay_range(&mut self, decay: bool) {
        self.decay_range = decay;
    }

    /// Enable color decay
    pub fn set_decay_color(&mut self, decay: bool) {
        self.decay_color = decay;
    }

    /// Calculate attenuation factor at a given distance
    pub fn calculate_attenuation(&self, distance: f32) -> f32 {
        if distance <= self.far_atten_start {
            return 1.0;
        }
        if distance >= self.far_atten_end {
            return 0.0;
        }
        let range = self.far_atten_end - self.far_atten_start;
        if range > 0.0 {
            1.0 - (distance - self.far_atten_start) / range
        } else {
            1.0
        }
    }

    /// Get bounding sphere radius (for culling)
    pub fn get_bounding_radius(&self) -> f32 {
        self.far_atten_end
    }

    /// Frame update - handles decay and fading (matching C++ On_Frame_Update)
    pub fn on_frame_update(&mut self) {
        if !self.enabled {
            return;
        }

        let mut factor = 1.0f32;

        if self.cur_increase_frame_count > 0 && self.increase_frame_count > 0 {
            // Increasing (fade in)
            self.cur_increase_frame_count -= 1;
            factor = (self.increase_frame_count - self.cur_increase_frame_count) as f32
                / self.increase_frame_count as f32;
        } else if self.decay_frame_count == 0 {
            // Never decays
            factor = 1.0;
        } else {
            // Decaying (fade out)
            if self.cur_decay_frame_count > 0 {
                self.cur_decay_frame_count -= 1;
            }
            if self.cur_decay_frame_count == 0 {
                self.enabled = false;
                return;
            }
            factor = self.cur_decay_frame_count as f32 / self.decay_frame_count as f32;
        }

        // Apply decay to range
        if self.decay_range {
            self.far_atten_end = factor * self.target_range;
            if self.far_atten_end < self.far_atten_start {
                self.far_atten_end = self.far_atten_start;
            }
        }

        // Apply decay to color
        if self.decay_color {
            self.ambient = self.target_ambient * factor;
            self.diffuse = self.target_diffuse * factor;
        }
    }

    /// Calculate light contribution at a point
    pub fn calculate_contribution(
        &self,
        point: Vector3<f32>,
        normal: Vector3<f32>,
    ) -> LightContribution {
        let mut contribution = LightContribution::default();

        if !self.enabled {
            return contribution;
        }

        match self.light_type {
            LightType::Directional => {
                // Directional light - no distance attenuation
                let n_dot_l = normal.dot(-self.direction).max(0.0);
                contribution.diffuse = self.diffuse * n_dot_l * self.intensity;
                contribution.ambient = self.ambient * self.intensity;
                contribution.specular = self.specular * self.intensity;
            }
            LightType::Point => {
                // Point light - with distance attenuation
                let to_light = self.position - point;
                let distance = to_light.magnitude();
                let atten = self.calculate_attenuation(distance);

                if atten > 0.0 && distance > 0.0 {
                    let light_dir = to_light / distance;
                    let n_dot_l = normal.dot(light_dir).max(0.0);
                    contribution.diffuse = self.diffuse * n_dot_l * atten * self.intensity;
                    contribution.ambient = self.ambient * atten * self.intensity;
                    contribution.specular = self.specular * atten * self.intensity;
                }
            }
            LightType::Spot => {
                // Spot light - with cone attenuation
                let to_light = self.position - point;
                let distance = to_light.magnitude();
                let atten = self.calculate_attenuation(distance);

                if atten > 0.0 && distance > 0.0 {
                    let light_dir = to_light / distance;
                    let spot_factor = (-self.direction).dot(light_dir);
                    let outer_cos = (self.outer_cone_angle * std::f32::consts::PI / 180.0).cos();
                    let inner_cos = (self.inner_cone_angle * std::f32::consts::PI / 180.0).cos();

                    let cone_atten = if spot_factor >= inner_cos {
                        1.0
                    } else if spot_factor <= outer_cos {
                        0.0
                    } else {
                        (spot_factor - outer_cos) / (inner_cos - outer_cos)
                    };

                    let n_dot_l = normal.dot(light_dir).max(0.0);
                    let total_atten = atten * cone_atten;
                    contribution.diffuse = self.diffuse * n_dot_l * total_atten * self.intensity;
                    contribution.ambient = self.ambient * total_atten * self.intensity;
                    contribution.specular = self.specular * total_atten * self.intensity;
                }
            }
            LightType::Ambient => {
                contribution.ambient = self.ambient * self.intensity;
            }
        }

        contribution
    }
}

/// Light contribution result
#[derive(Debug, Clone, Copy, Default)]
pub struct LightContribution {
    pub ambient: Vector3<f32>,
    pub diffuse: Vector3<f32>,
    pub specular: Vector3<f32>,
}

impl LightContribution {
    pub fn zero() -> Self {
        Self {
            ambient: Vector3::zero(),
            diffuse: Vector3::zero(),
            specular: Vector3::zero(),
        }
    }

    pub fn add(&mut self, other: &LightContribution) {
        self.ambient += other.ambient;
        self.diffuse += other.diffuse;
        self.specular += other.specular;
    }

    pub fn combined(&self) -> Vector3<f32> {
        self.ambient + self.diffuse + self.specular
    }
}

/// Light environment for managing multiple lights (matching C++ LightEnvironmentClass)
#[derive(Debug, Clone)]
pub struct LightEnvironment {
    pub center: Vector3<f32>,
    pub ambient: Vector3<f32>,
    pub output_ambient: Vector3<f32>,
    pub lights: Vec<W3DDynamicLight>,
    pub max_lights: usize,
}

impl Default for LightEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl LightEnvironment {
    pub fn new() -> Self {
        Self {
            center: Vector3::zero(),
            ambient: Vector3::new(0.2, 0.2, 0.2),
            output_ambient: Vector3::new(0.2, 0.2, 0.2),
            lights: Vec::with_capacity(MAX_LIGHTS),
            max_lights: MAX_LIGHTS,
        }
    }

    /// Reset the light environment with a new center and ambient color
    pub fn reset(&mut self, center: Vector3<f32>, ambient: Vector3<f32>) {
        self.center = center;
        self.ambient = ambient;
        self.output_ambient = ambient;
        self.lights.clear();
    }

    /// Add a light to the environment
    pub fn add_light(&mut self, light: &W3DDynamicLight) {
        if self.lights.len() < self.max_lights && light.enabled {
            self.lights.push(light.clone());
        }
    }

    /// Set output ambient color
    pub fn set_output_ambient(&mut self, ambient: Vector3<f32>) {
        self.output_ambient = ambient;
    }

    /// Get equivalent ambient (matching C++)
    pub fn get_equivalent_ambient(&self) -> Vector3<f32> {
        self.output_ambient
    }

    /// Pre-render update (transform lights to camera space)
    pub fn pre_render_update(&mut self, _camera_transform: &cgmath::Matrix4<f32>) {
        // Update all lights for the new frame
        for light in &mut self.lights {
            light.on_frame_update();
        }
    }

    /// Calculate combined lighting at a point
    pub fn calculate_lighting(
        &self,
        point: Vector3<f32>,
        normal: Vector3<f32>,
    ) -> LightContribution {
        let mut result = LightContribution {
            ambient: self.output_ambient,
            diffuse: Vector3::zero(),
            specular: Vector3::zero(),
        };

        for light in &self.lights {
            if light.enabled {
                let contrib = light.calculate_contribution(point, normal);
                result.add(&contrib);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgmath::Vector3;

    #[test]
    fn test_dynamic_light_creation() {
        let light = W3DDynamicLight::point();
        assert!(light.enabled);
        assert_eq!(light.light_type, LightType::Point);
    }

    #[test]
    fn test_light_attenuation() {
        let mut light = W3DDynamicLight::point();
        light.set_range(10.0, 100.0);

        assert_eq!(light.calculate_attenuation(5.0), 1.0);
        assert_eq!(light.calculate_attenuation(100.0), 0.0);
        assert!(light.calculate_attenuation(55.0) > 0.0 && light.calculate_attenuation(55.0) < 1.0);
    }

    #[test]
    fn test_light_decay() {
        let mut light = W3DDynamicLight::point();
        light.set_frame_fade(0, 10);
        light.set_decay_range(true);
        light.set_decay_color(true);

        let initial_range = light.far_atten_end;
        for _ in 0..5 {
            light.on_frame_update();
        }
        assert!(light.far_atten_end < initial_range);
    }

    #[test]
    fn test_light_environment() {
        let mut env = LightEnvironment::new();
        let light = W3DDynamicLight::directional();

        env.reset(Vector3::zero(), Vector3::new(0.2, 0.2, 0.2));
        env.add_light(&light);

        assert_eq!(env.lights.len(), 1);
    }
}
