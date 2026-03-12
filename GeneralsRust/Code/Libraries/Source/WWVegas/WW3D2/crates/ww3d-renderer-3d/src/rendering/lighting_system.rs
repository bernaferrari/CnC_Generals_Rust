//! Lighting system - equivalent to C++ LightEnvironmentClass

use crate::texture_system::TextureClass;
use glam::{Mat4, Vec3};
use std::sync::{Arc, Mutex};

/// Light environment class - manages lighting for rendering
#[derive(Debug, Clone)]
pub struct LightEnvironmentClass {
    pub ambient: Vec3,
    pub lights: Vec<Arc<Mutex<LightClass>>>,
}

impl LightEnvironmentClass {
    /// Create a new light environment
    pub fn new() -> Self {
        Self {
            ambient: Vec3::new(0.1, 0.1, 0.1),
            lights: Vec::new(),
        }
    }

    /// Add a light to the environment
    pub fn add_light(&mut self, light: Arc<Mutex<LightClass>>) {
        self.lights.push(light);
    }

    /// Remove a light from the environment
    pub fn remove_light(&mut self, light_id: u32) {
        self.lights
            .retain(|light| light.lock().unwrap().id != light_id);
    }

    /// Get ambient light color
    pub fn get_ambient(&self) -> &Vec3 {
        &self.ambient
    }

    /// Set ambient light color
    pub fn set_ambient(&mut self, ambient: Vec3) {
        self.ambient = ambient;
    }
}

/// Light class - represents a light source
#[derive(Debug, Clone)]
pub struct LightClass {
    pub id: u32,
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub light_type: LightType,
    pub range: f32,
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,
    pub casts_shadows: bool,
    pub shadow_map: Option<ShadowMap>,
    pub attenuation: LightAttenuation,
    pub enabled: bool,
}

impl LightClass {
    /// Create a new directional light
    pub fn directional(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            id: 0, // Would be assigned by a manager
            position: Vec3::ZERO,
            direction: direction.normalize(),
            color,
            intensity,
            light_type: LightType::Directional,
            range: 1000.0,
            inner_cone_angle: 0.0,
            outer_cone_angle: 0.0,
            casts_shadows: false,
            shadow_map: None,
            attenuation: LightAttenuation::default(),
            enabled: true,
        }
    }

    /// Create a new point light
    pub fn point(position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            id: 0,
            position,
            direction: Vec3::ZERO,
            color,
            intensity,
            light_type: LightType::Point,
            range,
            inner_cone_angle: 0.0,
            outer_cone_angle: 0.0,
            casts_shadows: false,
            shadow_map: None,
            attenuation: LightAttenuation::default(),
            enabled: true,
        }
    }

    /// Create a new spot light
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
            id: 0,
            position,
            direction: direction.normalize(),
            color,
            intensity,
            light_type: LightType::Spot,
            range,
            inner_cone_angle: inner_angle,
            outer_cone_angle: outer_angle,
            casts_shadows: false,
            shadow_map: None,
            attenuation: LightAttenuation::default(),
            enabled: true,
        }
    }

    /// Calculate light contribution at a point
    pub fn calculate_contribution(&self, point: Vec3, normal: Vec3, view_dir: Vec3) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }

        let (light_dir, attenuation, spot_factor) = match self.light_type {
            LightType::Directional => (-self.direction, 1.0, 1.0),
            LightType::Point => {
                let dir = (self.position - point).normalize();
                let distance = (self.position - point).length();
                (dir, self.attenuation.calculate(distance, self.range), 1.0)
            }
            LightType::Spot => {
                let dir = (self.position - point).normalize();
                let distance = (self.position - point).length();
                let attenuation = self.attenuation.calculate(distance, self.range);

                // Calculate spot factor
                let cos_angle = dir.dot(-self.direction);
                let spot = if cos_angle < self.outer_cone_angle.cos() {
                    0.0
                } else if cos_angle > self.inner_cone_angle.cos() {
                    1.0
                } else {
                    let t = (cos_angle - self.outer_cone_angle.cos())
                        / (self.inner_cone_angle.cos() - self.outer_cone_angle.cos());
                    t * t
                };
                (dir, attenuation, spot)
            }
        };

        // Diffuse lighting
        let n_dot_l = normal.dot(light_dir).max(0.0);
        let diffuse = self.color * self.intensity * n_dot_l;

        // Specular lighting (simplified)
        let reflect_dir = reflect(-light_dir, normal);
        let spec = view_dir.dot(reflect_dir).max(0.0).powf(32.0);
        let specular = self.color * self.intensity * spec * 0.5;

        (diffuse + specular) * attenuation * spot_factor
    }

    /// Get light view-projection matrix for shadow mapping
    pub fn get_light_view_projection(&self, scene_center: Vec3, scene_radius: f32) -> Mat4 {
        match self.light_type {
            LightType::Directional => {
                // Orthographic projection for directional light shadows
                let projection = Mat4::orthographic_rh(
                    -scene_radius,
                    scene_radius,
                    -scene_radius,
                    scene_radius,
                    -scene_radius,
                    scene_radius,
                );

                let view = Mat4::look_at_rh(
                    scene_center - self.direction * scene_radius * 2.0,
                    scene_center,
                    Vec3::Y,
                );

                projection * view
            }
            LightType::Point => {
                // Perspective projection for point light shadows
                let projection = Mat4::perspective_rh(
                    90.0f32.to_radians(), // 90 degrees for cube map face
                    1.0,
                    0.1,
                    self.range,
                );

                // For simplicity, return identity (would need cube map face handling)
                projection
            }
            LightType::Spot => {
                // Perspective projection for spot light shadows
                let projection =
                    Mat4::perspective_rh(self.outer_cone_angle * 2.0, 1.0, 0.1, self.range);

                let view = Mat4::look_at_rh(self.position, self.position + self.direction, Vec3::Y);

                projection * view
            }
        }
    }
}

/// Light types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightType {
    Directional,
    Point,
    Spot,
}

/// Light attenuation parameters
#[derive(Debug, Clone, Copy)]
pub struct LightAttenuation {
    pub constant: f32,
    pub linear: f32,
    pub quadratic: f32,
}

impl LightAttenuation {
    /// Calculate attenuation factor
    pub fn calculate(&self, distance: f32, range: f32) -> f32 {
        if distance >= range {
            return 0.0;
        }

        let attenuation =
            1.0 / (self.constant + self.linear * distance + self.quadratic * distance * distance);
        attenuation.min(1.0).max(0.0)
    }
}

impl Default for LightAttenuation {
    fn default() -> Self {
        Self {
            constant: 1.0,
            linear: 0.09,
            quadratic: 0.032,
        }
    }
}

/// Shadow map for storing depth information
#[derive(Debug, Clone)]
pub struct ShadowMap {
    pub texture: Option<TextureClass>,
    pub size: u32,
    pub light_view_projection: Mat4,
}

impl ShadowMap {
    /// Create a new shadow map
    pub fn new(size: u32) -> Self {
        Self {
            texture: None,
            size,
            light_view_projection: Mat4::IDENTITY,
        }
    }
}

/// Environment map for reflections
#[derive(Debug, Clone)]
pub struct EnvironmentMap {
    pub texture: Option<TextureClass>,
    pub position: Vec3,
    pub intensity: f32,
}

impl EnvironmentMap {
    /// Create a new environment map
    pub fn new(position: Vec3, intensity: f32) -> Self {
        Self {
            texture: None,
            position,
            intensity,
        }
    }
}

/// Advanced lighting manager
#[derive(Debug)]
pub struct LightingManager {
    pub light_environment: LightEnvironmentClass,
    pub environment_map: Option<EnvironmentMap>,
    pub shadow_enabled: bool,
    pub ssao_enabled: bool,
    pub bloom_enabled: bool,
}

impl LightingManager {
    /// Create a new lighting manager
    pub fn new() -> Self {
        Self {
            light_environment: LightEnvironmentClass::new(),
            environment_map: None,
            shadow_enabled: true,
            ssao_enabled: true,
            bloom_enabled: true,
        }
    }

    /// Add a light to the scene
    pub fn add_light(&mut self, light: LightClass) {
        let light_arc = Arc::new(Mutex::new(light));
        self.light_environment.add_light(light_arc);
    }

    /// Calculate lighting contribution for a point
    pub fn calculate_lighting(
        &self,
        position: Vec3,
        normal: Vec3,
        view_dir: Vec3,
        albedo: Vec3,
    ) -> Vec3 {
        let mut total_light = self.light_environment.ambient * albedo;

        for light in &self.light_environment.lights {
            let light = light.lock().unwrap();
            if light.enabled {
                let light_contrib = light.calculate_contribution(position, normal, view_dir);
                total_light += light_contrib * albedo;
            }
        }

        // Add environment map contribution if available
        if let Some(ref env_map) = self.environment_map {
            if env_map.texture.is_some() {
                // Simplified environment mapping
                let env_color = Vec3::new(0.5, 0.5, 0.7); // Placeholder
                total_light += env_color * env_map.intensity;
            }
        }

        total_light
    }

    /// Update shadow maps for all lights that cast shadows
    pub fn update_shadow_maps(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) {
        for light in &self.light_environment.lights {
            let mut light = light.lock().unwrap();
            if light.casts_shadows {
                if light.shadow_map.is_none() {
                    light.shadow_map = Some(ShadowMap::new(1024));
                }

                // Update light view-projection matrix without conflicting borrows
                let light_view_projection = {
                    let light_ref = &*light;
                    light_ref.get_light_view_projection(Vec3::ZERO, 100.0)
                };
                if let Some(ref mut shadow_map) = light.shadow_map {
                    shadow_map.light_view_projection = light_view_projection;
                    // In a full implementation, render the scene from the light to populate texture
                }
            }
        }
    }

    /// Get shadow factor for a point
    pub fn get_shadow_factor(&self, _position: Vec3, light_index: usize) -> f32 {
        if !self.shadow_enabled {
            return 1.0;
        }

        if let Some(light) = self.light_environment.lights.get(light_index) {
            let light = light.lock().unwrap();
            if let Some(ref _shadow_map) = light.shadow_map {
                // Simplified shadow mapping - in practice this would sample the shadow map
                return 1.0; // No shadow
            }
        }

        1.0
    }
}

/// Helper function for vector reflection
fn reflect(incident: Vec3, normal: Vec3) -> Vec3 {
    incident - 2.0 * incident.dot(normal) * normal
}

/// Default implementation for LightEnvironmentClass
impl Default for LightEnvironmentClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Default implementation for LightingManager
impl Default for LightingManager {
    fn default() -> Self {
        Self::new()
    }
}
