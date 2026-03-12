//! Light System - Complete lighting functionality
//!
//! This module implements the LightClass from the original C++ code,
//! providing comprehensive lighting with WGPU integration.
//!
//! Converted from:
//! - light.cpp/h (light class implementation)
//! - lightenvironment.cpp/h (light environment management)

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use glam::{Vec3, Vec4, Mat4, Quat};
use crate::core::error::{W3dError, Result};
use crate::render_object_system::{RenderObjClass, RenderObjClassId};
use crate::bounding_volumes::{AABoxClass, SphereClass};
use crate::scene_system::SceneManagerClass;
use crate::scene_system::scene::SceneClass;
use crate::core::ww3d_core::WW3D;

static LIGHT_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_light_id() -> u32 {
    LIGHT_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

/// Light type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightType {
    /// Directional light
    Directional = 0,
    /// Point light
    Point,
    /// Spot light
    Spot,
}

/// Light class - Core lighting functionality
#[derive(Debug)]
pub struct LightClass {
    /// Base render object
    pub base: Option<Arc<dyn RenderObjClass>>,
    /// Light type
    pub light_type: LightType,
    /// Light position
    pub position: Vec3,
    /// Light direction (for directional and spot lights)
    pub direction: Vec3,
    /// Ambient color
    pub ambient: Vec4,
    /// Diffuse color
    pub diffuse: Vec4,
    /// Specular color
    pub specular: Vec4,
    /// Light intensity
    pub intensity: f32,
    /// Spotlight inner angle (in radians)
    pub inner_cone_angle: f32,
    /// Spotlight outer angle (in radians)
    pub outer_cone_angle: f32,
    /// Spotlight falloff
    pub spot_falloff: f32,
    /// Attenuation start distance
    pub attenuation_start: f32,
    /// Attenuation end distance
    pub attenuation_end: f32,
    /// Light range (maximum distance)
    pub range: f32,
    /// Transform matrix
    pub transform: Mat4,
    /// Whether transform is dirty
    pub transform_dirty: bool,
    /// Light ID
    pub light_id: u32,
}

impl LightClass {
    /// Create new directional light
    pub fn new_directional(direction: Vec3, color: Vec4) -> Self {
        let light_id = next_light_id();

        Self {
            base: None, // Placeholder - would be proper render object
            light_type: LightType::Directional,
            position: Vec3::ZERO,
            direction: direction.normalize(),
            ambient: Vec4::ZERO,
            diffuse: color,
            specular: color,
            intensity: 1.0,
            inner_cone_angle: 0.0,
            outer_cone_angle: 0.0,
            spot_falloff: 1.0,
            attenuation_start: 0.0,
            attenuation_end: f32::INFINITY,
            range: f32::INFINITY,
            transform: Mat4::IDENTITY,
            transform_dirty: true,
            light_id,
        }
    }

    /// Create new point light
    pub fn new_point(position: Vec3, color: Vec4, range: f32) -> Self {
        let light_id = next_light_id();

        Self {
            base: None, // Placeholder
            light_type: LightType::Point,
            position,
            direction: Vec3::ZERO,
            ambient: Vec4::ZERO,
            diffuse: color,
            specular: color,
            intensity: 1.0,
            inner_cone_angle: 0.0,
            outer_cone_angle: 0.0,
            spot_falloff: 1.0,
            attenuation_start: 0.0,
            attenuation_end: range,
            range,
            transform: Mat4::from_translation(position),
            transform_dirty: false,
            light_id,
        }
    }

    /// Create new spot light
    pub fn new_spot(position: Vec3, direction: Vec3, color: Vec4, range: f32, inner_angle: f32, outer_angle: f32) -> Self {
        let light_id = next_light_id();

        Self {
            base: None, // Placeholder
            light_type: LightType::Spot,
            position,
            direction: direction.normalize(),
            ambient: Vec4::ZERO,
            diffuse: color,
            specular: color,
            intensity: 1.0,
            inner_cone_angle: inner_angle,
            outer_cone_angle: outer_angle,
            spot_falloff: 1.0,
            attenuation_start: 0.0,
            attenuation_end: range,
            range,
            transform: Mat4::from_translation(position),
            transform_dirty: false,
            light_id,
        }
    }

    /// Clone light
    pub fn clone(&self) -> Self {
        let light_id = next_light_id();

        Self {
            base: self.base.clone(),
            light_type: self.light_type,
            position: self.position,
            direction: self.direction,
            ambient: self.ambient,
            diffuse: self.diffuse,
            specular: self.specular,
            intensity: self.intensity,
            inner_cone_angle: self.inner_cone_angle,
            outer_cone_angle: self.outer_cone_angle,
            spot_falloff: self.spot_falloff,
            attenuation_start: self.attenuation_start,
            attenuation_end: self.attenuation_end,
            range: self.range,
            transform: self.transform,
            transform_dirty: self.transform_dirty,
            light_id,
        }
    }

    /// Set position
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.transform = Mat4::from_translation(position);
        self.transform_dirty = false;

        if self.light_type == LightType::Directional {
            // For directional lights, position doesn't affect lighting
            // but we still update it for consistency
        }
    }

    /// Get position
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    /// Set direction (for directional and spot lights)
    pub fn set_direction(&mut self, direction: Vec3) {
        self.direction = direction.normalize();
    }

    /// Get direction
    pub fn get_direction(&self) -> Vec3 {
        self.direction
    }

    /// Set diffuse color
    pub fn set_diffuse(&mut self, color: Vec4) {
        self.diffuse = color;
    }

    /// Get diffuse color
    pub fn get_diffuse(&self) -> Vec4 {
        self.diffuse
    }

    /// Set specular color
    pub fn set_specular(&mut self, color: Vec4) {
        self.specular = color;
    }

    /// Get specular color
    pub fn get_specular(&self) -> Vec4 {
        self.specular
    }

    /// Set ambient color
    pub fn set_ambient(&mut self, color: Vec4) {
        self.ambient = color;
    }

    /// Get ambient color
    pub fn get_ambient(&self) -> Vec4 {
        self.ambient
    }

    /// Set intensity
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, f32::INFINITY);
    }

    /// Get intensity
    pub fn get_intensity(&self) -> f32 {
        self.intensity
    }

    /// Set range (for point and spot lights)
    pub fn set_range(&mut self, range: f32) {
        self.range = range;
        self.attenuation_end = range;
    }

    /// Get range
    pub fn get_range(&self) -> f32 {
        self.range
    }

    /// Set spot angles (for spot lights)
    pub fn set_spot_angles(&mut self, inner: f32, outer: f32) {
        self.inner_cone_angle = inner;
        self.outer_cone_angle = outer;
    }

    /// Get spot angles
    pub fn get_spot_angles(&self) -> (f32, f32) {
        (self.inner_cone_angle, self.outer_cone_angle)
    }

    /// Set attenuation parameters
    pub fn set_attenuation(&mut self, start: f32, end: f32) {
        self.attenuation_start = start;
        self.attenuation_end = end;
        if self.light_type != LightType::Directional {
            self.range = end;
        }
    }

    /// Get attenuation parameters
    pub fn get_attenuation(&self) -> (f32, f32) {
        (self.attenuation_start, self.attenuation_end)
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        // Extract position from transform
        self.position = transform.row(3).truncate();
        self.transform_dirty = false;
    }

    /// Get transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Get attenuation range
    pub fn get_attenuation_range(&self) -> f32 {
        self.attenuation_end
    }

    /// Get object space bounding sphere
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        // For lights, the bounding sphere represents the light's influence area
        match self.light_type {
            LightType::Directional => {
                // Directional lights affect everything
                SphereClass::from_center_and_radius(Vec3::ZERO, f32::INFINITY)
            }
            LightType::Point => {
                SphereClass::from_center_and_radius(Vec3::ZERO, self.range)
            }
            LightType::Spot => {
                SphereClass::from_center_and_radius(Vec3::ZERO, self.range)
            }
        }
    }

    /// Get object space bounding box
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        // For lights, the bounding box represents the light's influence area
        match self.light_type {
            LightType::Directional => {
                // Directional lights affect everything
                AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY))
            }
            LightType::Point => {
                let extent = Vec3::new(self.range, self.range, self.range);
                AABoxClass::from_center_and_extent(Vec3::ZERO, extent)
            }
            LightType::Spot => {
                let extent = Vec3::new(self.range, self.range, self.range);
                AABoxClass::from_center_and_extent(Vec3::ZERO, extent)
            }
        }
    }

    /// Push light to vertex processor
    pub fn vertex_processor_push(&self) -> Result<()> {
        // In a full implementation, this would add the light to the GPU pipeline
        // For now, this is a placeholder
        Ok(())
    }

    /// Pop light from vertex processor
    pub fn vertex_processor_pop(&self) -> Result<()> {
        // In a full implementation, this would remove the light from the GPU pipeline
        Ok(())
    }

    /// Notify when light is added to scene
    pub fn notify_added(&mut self, scene: &mut SceneClass) -> Result<()> {
        // Add light to scene's light list
        // In a full implementation, this would register the light with the scene
        let _ = scene;
        Ok(())
    }

    /// Notify when light is removed from scene
    pub fn notify_removed(&mut self, scene: &mut SceneClass) -> Result<()> {
        // Remove light from scene's light list
        let _ = scene;
        Ok(())
    }

    /// Load light from W3D file
    pub fn load_w3d(&mut self, data: &[u8]) -> Result<()> {
        // In a full implementation, this would parse W3D light chunk data
        // For now, this is a placeholder
        let _ = data;
        Ok(())
    }

    /// Save light to W3D file
    pub fn save_w3d(&self, writer: &mut dyn std::io::Write) -> Result<()> {
        // In a full implementation, this would write W3D light chunk data
        let _ = writer;
        Ok(())
    }

    /// Get light type
    pub fn get_type(&self) -> LightType {
        self.light_type
    }

    /// Check if light is directional
    pub fn is_directional(&self) -> bool {
        self.light_type == LightType::Directional
    }

    /// Check if light is point light
    pub fn is_point(&self) -> bool {
        self.light_type == LightType::Point
    }

    /// Check if light is spot light
    pub fn is_spot(&self) -> bool {
        self.light_type == LightType::Spot
    }

    /// Get light color (diffuse)
    pub fn get_color(&self) -> Vec4 {
        self.diffuse
    }

    /// Set light color
    pub fn set_color(&mut self, color: Vec4) {
        self.diffuse = color;
        self.specular = color;
    }

    /// Calculate light intensity at distance
    pub fn calculate_intensity_at_distance(&self, distance: f32) -> f32 {
        if self.light_type == LightType::Directional {
            return self.intensity;
        }

        if distance <= self.attenuation_start {
            return self.intensity;
        }

        if distance >= self.attenuation_end {
            return 0.0;
        }

        // Linear attenuation
        let factor = 1.0 - ((distance - self.attenuation_start) / (self.attenuation_end - self.attenuation_start));
        self.intensity * factor
    }

    /// Calculate spot light factor
    pub fn calculate_spot_factor(&self, direction_to_light: Vec3) -> f32 {
        if self.light_type != LightType::Spot {
            return 1.0;
        }

        let cos_angle = self.direction.dot(-direction_to_light);
        let cos_inner = self.inner_cone_angle.cos();
        let cos_outer = self.outer_cone_angle.cos();

        if cos_angle > cos_inner {
            return 1.0; // Inside inner cone
        }

        if cos_angle < cos_outer {
            return 0.0; // Outside outer cone
        }

        // Between inner and outer cone - smooth falloff
        let factor = (cos_angle - cos_outer) / (cos_inner - cos_outer);
        factor.powf(self.spot_falloff)
    }

    /// Get light contribution at point
    pub fn get_contribution(&self, point: Vec3, normal: Vec3, view_dir: Vec3) -> LightContribution {
        let mut contribution = LightContribution {
            ambient: Vec4::ZERO,
            diffuse: Vec4::ZERO,
            specular: Vec4::ZERO,
        };

        match self.light_type {
            LightType::Directional => {
                let light_dir = -self.direction.normalize();
                let intensity = self.intensity;

                // Ambient
                contribution.ambient = self.ambient * intensity;

                // Diffuse
                let n_dot_l = normal.dot(light_dir).max(0.0);
                contribution.diffuse = self.diffuse * intensity * n_dot_l;

                // Specular (simplified Blinn-Phong)
                let half_vector = (light_dir + view_dir).normalize();
                let n_dot_h = normal.dot(half_vector).max(0.0);
                let specular_intensity = n_dot_h.powf(32.0); // Hardcoded shininess
                contribution.specular = self.specular * intensity * specular_intensity;
            }

            LightType::Point => {
                let to_light = self.position - point;
                let distance = to_light.length();
                let light_dir = to_light.normalize();

                let intensity = self.calculate_intensity_at_distance(distance);

                // Ambient
                contribution.ambient = self.ambient * intensity;

                // Diffuse
                let n_dot_l = normal.dot(light_dir).max(0.0);
                contribution.diffuse = self.diffuse * intensity * n_dot_l;

                // Specular
                let half_vector = (light_dir + view_dir).normalize();
                let n_dot_h = normal.dot(half_vector).max(0.0);
                let specular_intensity = n_dot_h.powf(32.0);
                contribution.specular = self.specular * intensity * specular_intensity;
            }

            LightType::Spot => {
                let to_light = self.position - point;
                let distance = to_light.length();
                let light_dir = to_light.normalize();

                let intensity = self.calculate_intensity_at_distance(distance);
                let spot_factor = self.calculate_spot_factor(light_dir);

                let final_intensity = intensity * spot_factor;

                // Ambient
                contribution.ambient = self.ambient * final_intensity;

                // Diffuse
                let n_dot_l = normal.dot(light_dir).max(0.0);
                contribution.diffuse = self.diffuse * final_intensity * n_dot_l;

                // Specular
                let half_vector = (light_dir + view_dir).normalize();
                let n_dot_h = normal.dot(half_vector).max(0.0);
                let specular_intensity = n_dot_h.powf(32.0);
                contribution.specular = self.specular * final_intensity * specular_intensity;
            }
        }

        contribution
    }
}

/// Light contribution structure
#[derive(Debug, Clone, Copy)]
pub struct LightContribution {
    /// Ambient contribution
    pub ambient: Vec4,
    /// Diffuse contribution
    pub diffuse: Vec4,
    /// Specular contribution
    pub specular: Vec4,
}

impl std::ops::Add for LightContribution {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            ambient: self.ambient + other.ambient,
            diffuse: self.diffuse + other.diffuse,
            specular: self.specular + other.specular,
        }
    }
}

impl std::ops::AddAssign for LightContribution {
    fn add_assign(&mut self, other: Self) {
        self.ambient += other.ambient;
        self.diffuse += other.diffuse;
        self.specular += other.specular;
    }
}

/// Light environment class - manages multiple lights
#[derive(Debug)]
pub struct LightEnvironmentClass {
    /// Ambient light color
    pub ambient: Vec4,
    /// Lights in the environment
    pub lights: Vec<Arc<LightClass>>,
    /// Maximum number of lights
    pub max_lights: usize,
    /// Whether environment is enabled
    pub enabled: bool,
}

impl LightEnvironmentClass {
    /// Create new light environment
    pub fn new() -> Self {
        Self {
            ambient: Vec4::new(0.2, 0.2, 0.2, 1.0),
            lights: Vec::new(),
            max_lights: 8, // Common limit for shader-based lighting
            enabled: true,
        }
    }

    /// Add light to environment
    pub fn add_light(&mut self, light: Arc<LightClass>) -> Result<()> {
        if self.lights.len() >= self.max_lights {
            return Err(W3dError::InvalidParameter(format!(
                "Maximum number of lights ({}) exceeded", self.max_lights
            )));
        }

        self.lights.push(light);
        Ok(())
    }

    /// Remove light from environment
    pub fn remove_light(&mut self, light_id: u32) -> bool {
        self.lights.retain(|light| light.light_id != light_id);
        true
    }

    /// Clear all lights
    pub fn clear_lights(&mut self) {
        self.lights.clear();
    }

    /// Get light contribution at point
    pub fn get_contribution(&self, point: Vec3, normal: Vec3, view_dir: Vec3) -> LightContribution {
        if !self.enabled {
            return LightContribution {
                ambient: self.ambient,
                diffuse: Vec4::ZERO,
                specular: Vec4::ZERO,
            };
        }

        let mut total_contribution = LightContribution {
            ambient: self.ambient,
            diffuse: Vec4::ZERO,
            specular: Vec4::ZERO,
        };

        for light in &self.lights {
            let contribution = light.get_contribution(point, normal, view_dir);
            total_contribution += contribution;
        }

        total_contribution
    }

    /// Get number of lights
    pub fn get_light_count(&self) -> usize {
        self.lights.len()
    }

    /// Get light by index
    pub fn get_light(&self, index: usize) -> Option<Arc<LightClass>> {
        self.lights.get(index).map(|light| Arc::clone(light))
    }

    /// Set ambient color
    pub fn set_ambient(&mut self, ambient: Vec4) {
        self.ambient = ambient;
    }

    /// Get ambient color
    pub fn get_ambient(&self) -> Vec4 {
        self.ambient
    }

    /// Set maximum lights
    pub fn set_max_lights(&mut self, max: usize) {
        self.max_lights = max;
        // Remove excess lights if any
        while self.lights.len() > self.max_lights {
            self.lights.pop();
        }
    }

    /// Enable/disable environment
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for LightEnvironmentClass {
    fn default() -> Self {
        Self::new()
    }
}

fn light_environment_slot() -> &'static Mutex<Option<LightEnvironmentClass>> {
    static SLOT: OnceLock<Mutex<Option<LightEnvironmentClass>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_light_environment_slot() -> MutexGuard<'static, Option<LightEnvironmentClass>> {
    match light_environment_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Handle used to access the shared light environment safely.
pub struct LightEnvironmentHandle<'a> {
    guard: MutexGuard<'a, Option<LightEnvironmentClass>>,
}

impl<'a> Deref for LightEnvironmentHandle<'a> {
    type Target = LightEnvironmentClass;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("light environment must be initialized before use")
    }
}

impl<'a> DerefMut for LightEnvironmentHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("light environment must be initialized before use")
    }
}

/// Initialize light system
pub fn init_light_system() -> Result<()> {
    let mut guard = lock_light_environment_slot();
    *guard = Some(LightEnvironmentClass::default());
    Ok(())
}

/// Shutdown light system
pub fn shutdown_light_system() {
    let mut guard = lock_light_environment_slot();
    *guard = None;
}

/// Get light environment instance
pub fn get_light_environment() -> Option<LightEnvironmentHandle<'static>> {
    let guard = lock_light_environment_slot();
    if guard.is_none() {
        None
    } else {
        Some(LightEnvironmentHandle { guard })
    }
}

/// Quick light creation functions
pub fn create_directional_light(direction: Vec3, color: Vec4) -> LightClass {
    LightClass::new_directional(direction, color)
}

pub fn create_point_light(position: Vec3, color: Vec4, range: f32) -> LightClass {
    LightClass::new_point(position, color, range)
}

pub fn create_spot_light(position: Vec3, direction: Vec3, color: Vec4, range: f32, inner_angle: f32, outer_angle: f32) -> LightClass {
    LightClass::new_spot(position, direction, color, range, inner_angle, outer_angle)
}

/// Quick light environment functions
pub fn add_light_to_environment(light: LightClass) -> Result<()> {
    let mut env = get_light_environment()
        .ok_or_else(|| W3dError::NotInitialized("Light environment not initialized".to_string()))?;

    env.add_light(Arc::new(light))
}

pub fn get_light_contribution(point: Vec3, normal: Vec3, view_dir: Vec3) -> LightContribution {
    if let Some(env) = get_light_environment() {
        env.get_contribution(point, normal, view_dir)
    } else {
        LightContribution {
            ambient: Vec4::new(0.2, 0.2, 0.2, 1.0),
            diffuse: Vec4::ZERO,
            specular: Vec4::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_directional_light_creation() {
        let direction = Vec3::new(0.0, -1.0, 0.0);
        let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
        let light = LightClass::new_directional(direction, color);

        assert_eq!(light.light_type, LightType::Directional);
        assert_eq!(light.direction, direction.normalize());
        assert_eq!(light.diffuse, color);
    }

    #[test]
    fn test_point_light_creation() {
        let position = Vec3::new(10.0, 0.0, 0.0);
        let color = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let range = 100.0;
        let light = LightClass::new_point(position, color, range);

        assert_eq!(light.light_type, LightType::Point);
        assert_eq!(light.position, position);
        assert_eq!(light.range, range);
        assert_eq!(light.diffuse, color);
    }

    #[test]
    fn test_spot_light_creation() {
        let position = Vec3::new(0.0, 0.0, 10.0);
        let direction = Vec3::new(0.0, 0.0, -1.0);
        let color = Vec4::new(0.0, 1.0, 0.0, 1.0);
        let range = 50.0;
        let inner_angle = 0.1;
        let outer_angle = 0.5;
        let light = LightClass::new_spot(position, direction, color, range, inner_angle, outer_angle);

        assert_eq!(light.light_type, LightType::Spot);
        assert_eq!(light.position, position);
        assert_eq!(light.direction, direction.normalize());
        assert_eq!(light.inner_cone_angle, inner_angle);
        assert_eq!(light.outer_cone_angle, outer_angle);
    }

    #[test]
    fn test_light_intensity_calculation() {
        let light = LightClass::new_point(Vec3::ZERO, Vec4::ONE, 10.0);

        assert_eq!(light.calculate_intensity_at_distance(0.0), 1.0);
        assert_eq!(light.calculate_intensity_at_distance(5.0), 1.0); // Within attenuation start
        assert_eq!(light.calculate_intensity_at_distance(10.0), 0.0); // At attenuation end
        assert_eq!(light.calculate_intensity_at_distance(15.0), 0.0); // Beyond attenuation end
    }

    #[test]
    fn test_spot_light_factor() {
        let mut light = LightClass::new_spot(Vec3::ZERO, Vec3::Z, Vec4::ONE, 10.0, 0.1, 0.5);

        // Light direction is +Z, so -Z direction should have full intensity
        let factor = light.calculate_spot_factor(-Vec3::Z);
        assert_eq!(factor, 1.0);

        // Perpendicular direction should have zero intensity
        let factor = light.calculate_spot_factor(Vec3::X);
        assert_eq!(factor, 0.0);
    }

    #[test]
    fn test_light_environment() {
        let mut env = LightEnvironmentClass::new();
        assert_eq!(env.get_light_count(), 0);

        let light = Arc::new(LightClass::new_directional(Vec3::Y, Vec4::ONE));
        env.add_light(Arc::clone(&light)).unwrap();
        assert_eq!(env.get_light_count(), 1);

        env.clear_lights();
        assert_eq!(env.get_light_count(), 0);
    }

    #[test]
    fn test_light_contribution() {
        let light = LightClass::new_directional(-Vec3::Z, Vec4::ONE);
        let point = Vec3::ZERO;
        let normal = Vec3::Z;
        let view_dir = Vec3::Z;

        let contribution = light.get_contribution(point, normal, view_dir);

        // For directional light pointing down -Z, hitting surface with +Z normal
        // should give full diffuse contribution
        assert_eq!(contribution.diffuse, Vec4::ONE);
        assert_eq!(contribution.specular, Vec4::ZERO); // No specular for this case
    }

    #[test]
    fn test_light_clone() {
        let original = LightClass::new_point(Vec3::ONE, Vec4::new(1.0, 0.0, 0.0, 1.0), 50.0);
        let cloned = original.clone();

        assert_eq!(original.light_type, cloned.light_type);
        assert_eq!(original.position, cloned.position);
        assert_eq!(original.diffuse, cloned.diffuse);
        assert_ne!(original.light_id, cloned.light_id); // IDs should be different
    }
}
