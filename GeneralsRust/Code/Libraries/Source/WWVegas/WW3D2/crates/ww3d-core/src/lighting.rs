/// Lighting system for WW3D
///
/// This module implements dynamic lights and light environments.
use crate::material::Color;
use crate::render_object::{RenderInfo, RenderObject};
use crate::w3d_format::*;
use crate::RenderObjClassId;
use glam::{Mat4, Vec3};
use std::any::Any;
use std::fmt::Debug;

/// Light types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    /// Directional light (like the sun)
    Directional,
    /// Point light (omnidirectional)
    Point,
    /// Spot light (cone-shaped)
    Spot,
}

/// Attenuation parameters for point and spot lights
#[derive(Debug, Clone, Copy)]
pub struct Attenuation {
    pub constant: f32,
    pub linear: f32,
    pub quadratic: f32,
}

impl Attenuation {
    pub fn new(constant: f32, linear: f32, quadratic: f32) -> Self {
        Self {
            constant,
            linear,
            quadratic,
        }
    }

    pub fn none() -> Self {
        Self {
            constant: 1.0,
            linear: 0.0,
            quadratic: 0.0,
        }
    }

    pub fn calculate(&self, distance: f32) -> f32 {
        1.0 / (self.constant + self.linear * distance + self.quadratic * distance * distance)
    }
}

impl Default for Attenuation {
    fn default() -> Self {
        Self::none()
    }
}

/// Light parameters
#[derive(Debug, Clone)]
pub struct Light {
    name: String,
    light_type: LightType,
    position: Vec3,
    direction: Vec3,
    color: Color,
    intensity: f32,
    range: f32,
    attenuation: Attenuation,
    spot_inner_angle: f32,
    spot_outer_angle: f32,
    cast_shadows: bool,
    enabled: bool,
}

impl Light {
    pub fn new(name: String, light_type: LightType) -> Self {
        Self {
            name,
            light_type,
            position: Vec3::ZERO,
            direction: Vec3::NEG_Y,
            color: Color::WHITE,
            intensity: 1.0,
            range: 100.0,
            attenuation: Attenuation::default(),
            spot_inner_angle: 30.0_f32.to_radians(),
            spot_outer_angle: 45.0_f32.to_radians(),
            cast_shadows: false,
            enabled: true,
        }
    }

    pub fn directional(name: String, direction: Vec3, color: Color, intensity: f32) -> Self {
        let mut light = Self::new(name, LightType::Directional);
        light.direction = direction.normalize();
        light.color = color;
        light.intensity = intensity;
        light
    }

    pub fn point(name: String, position: Vec3, color: Color, intensity: f32, range: f32) -> Self {
        let mut light = Self::new(name, LightType::Point);
        light.position = position;
        light.color = color;
        light.intensity = intensity;
        light.range = range;
        light.attenuation = Attenuation::new(1.0, 0.09, 0.032);
        light
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spot(
        name: String,
        position: Vec3,
        direction: Vec3,
        color: Color,
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        let mut light = Self::new(name, LightType::Spot);
        light.position = position;
        light.direction = direction.normalize();
        light.color = color;
        light.intensity = intensity;
        light.range = range;
        light.spot_inner_angle = inner_angle;
        light.spot_outer_angle = outer_angle;
        light.attenuation = Attenuation::new(1.0, 0.09, 0.032);
        light
    }

    pub fn from_w3d(w3d_light: &W3dLightStruct) -> Self {
        // Extract light type from attributes
        let light_type = match w3d_light.attributes & 0x3 {
            1 => LightType::Point,
            2 => LightType::Spot,
            _ => LightType::Directional,
        };

        let mut light = Self::new("Light".to_string(), light_type);

        // Convert diffuse color to Color (convert u8 to f32)
        let diffuse = &w3d_light.diffuse;
        light.color = Color::new(
            diffuse.r as f32 / 255.0,
            diffuse.g as f32 / 255.0,
            diffuse.b as f32 / 255.0,
            diffuse.a as f32 / 255.0,
        );
        light.intensity = w3d_light.intensity;

        light
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn light_type(&self) -> LightType {
        self.light_type
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    pub fn direction(&self) -> Vec3 {
        self.direction
    }

    pub fn set_direction(&mut self, direction: Vec3) {
        self.direction = direction.normalize();
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    pub fn intensity(&self) -> f32 {
        self.intensity
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity;
    }

    pub fn range(&self) -> f32 {
        self.range
    }

    pub fn set_range(&mut self, range: f32) {
        self.range = range;
    }

    pub fn attenuation(&self) -> &Attenuation {
        &self.attenuation
    }

    pub fn set_attenuation(&mut self, attenuation: Attenuation) {
        self.attenuation = attenuation;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn cast_shadows(&self) -> bool {
        self.cast_shadows
    }

    pub fn set_cast_shadows(&mut self, cast_shadows: bool) {
        self.cast_shadows = cast_shadows;
    }

    /// Calculate light contribution for a point
    pub fn calculate_contribution(&self, point: Vec3, normal: Vec3) -> Color {
        if !self.enabled {
            return Color::TRANSPARENT;
        }

        match self.light_type {
            LightType::Directional => {
                let n_dot_l = normal.dot(-self.direction).max(0.0);
                let mut result = self.color;
                result.r *= self.intensity * n_dot_l;
                result.g *= self.intensity * n_dot_l;
                result.b *= self.intensity * n_dot_l;
                result
            }
            LightType::Point => {
                let to_light = self.position - point;
                let distance = to_light.length();

                if distance > self.range {
                    return Color::TRANSPARENT;
                }

                let light_dir = to_light / distance;
                let n_dot_l = normal.dot(light_dir).max(0.0);
                let attenuation = self.attenuation.calculate(distance);

                let mut result = self.color;
                let factor = self.intensity * n_dot_l * attenuation;
                result.r *= factor;
                result.g *= factor;
                result.b *= factor;
                result
            }
            LightType::Spot => {
                let to_light = self.position - point;
                let distance = to_light.length();

                if distance > self.range {
                    return Color::TRANSPARENT;
                }

                let light_dir = to_light / distance;
                let n_dot_l = normal.dot(light_dir).max(0.0);

                // Spot light cone calculation
                let spot_factor = light_dir.dot(-self.direction);
                let inner_cos = self.spot_inner_angle.cos();
                let outer_cos = self.spot_outer_angle.cos();

                if spot_factor < outer_cos {
                    return Color::TRANSPARENT;
                }

                let spot_intensity = if spot_factor > inner_cos {
                    1.0
                } else {
                    (spot_factor - outer_cos) / (inner_cos - outer_cos)
                };

                let attenuation = self.attenuation.calculate(distance);

                let mut result = self.color;
                let factor = self.intensity * n_dot_l * attenuation * spot_intensity;
                result.r *= factor;
                result.g *= factor;
                result.b *= factor;
                result
            }
        }
    }
}

/// Light environment for managing multiple lights
#[derive(Debug, Clone)]
pub struct LightEnvironment {
    lights: Vec<Light>,
    ambient: Color,
}

impl LightEnvironment {
    pub fn new() -> Self {
        Self {
            lights: Vec::new(),
            ambient: Color::new(0.2, 0.2, 0.2, 1.0),
        }
    }

    pub fn add_light(&mut self, light: Light) {
        self.lights.push(light);
    }

    pub fn remove_light(&mut self, index: usize) -> Option<Light> {
        if index < self.lights.len() {
            Some(self.lights.remove(index))
        } else {
            None
        }
    }

    pub fn get_light(&self, index: usize) -> Option<&Light> {
        self.lights.get(index)
    }

    pub fn get_light_mut(&mut self, index: usize) -> Option<&mut Light> {
        self.lights.get_mut(index)
    }

    pub fn light_count(&self) -> usize {
        self.lights.len()
    }

    pub fn set_ambient(&mut self, color: Color) {
        self.ambient = color;
    }

    pub fn ambient(&self) -> Color {
        self.ambient
    }

    pub fn clear(&mut self) {
        self.lights.clear();
    }

    /// Calculate total lighting contribution for a point
    pub fn calculate_lighting(&self, point: Vec3, normal: Vec3) -> Color {
        let mut result = self.ambient;

        for light in &self.lights {
            let contribution = light.calculate_contribution(point, normal);
            result.r += contribution.r;
            result.g += contribution.g;
            result.b += contribution.b;
        }

        // Clamp to valid range
        result.r = result.r.min(1.0);
        result.g = result.g.min(1.0);
        result.b = result.b.min(1.0);

        result
    }
}

impl Default for LightEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Light render object for debug visualization
#[derive(Debug, Clone)]
pub struct LightRenderObject {
    light: Light,
    transform: Mat4,
}

impl LightRenderObject {
    pub fn new(light: Light) -> Self {
        Self {
            light,
            transform: Mat4::IDENTITY,
        }
    }

    pub fn light(&self) -> &Light {
        &self.light
    }

    pub fn light_mut(&mut self) -> &mut Light {
        &mut self.light
    }
}

impl RenderObject for LightRenderObject {
    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Light
    }

    fn name(&self) -> &str {
        self.light.name()
    }

    fn set_name(&mut self, name: String) {
        self.light.set_name(name);
    }

    fn clone_object(&self) -> Box<dyn RenderObject> {
        Box::new(self.clone())
    }

    fn render(&mut self, _info: &RenderInfo) -> crate::errors::W3DResult<()> {
        // Lights don't render themselves (unless debug visualization is enabled)
        Ok(())
    }

    fn get_transform(&self) -> Mat4 {
        self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        // Update light position from transform
        self.light.position = transform.transform_point3(Vec3::ZERO);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_creation() {
        let light = Light::directional("sun".to_string(), Vec3::NEG_Y, Color::WHITE, 1.0);

        assert_eq!(light.name(), "sun");
        assert_eq!(light.light_type(), LightType::Directional);
        assert!(light.is_enabled());
    }

    #[test]
    fn test_point_light() {
        let light = Light::point(
            "bulb".to_string(),
            Vec3::new(0.0, 5.0, 0.0),
            Color::WHITE,
            1.0,
            10.0,
        );

        assert_eq!(light.light_type(), LightType::Point);
        assert_eq!(light.range(), 10.0);
    }

    #[test]
    fn test_spot_light() {
        let light = Light::spot(
            "spot".to_string(),
            Vec3::ZERO,
            Vec3::NEG_Y,
            Color::WHITE,
            1.0,
            20.0,
            30.0_f32.to_radians(),
            45.0_f32.to_radians(),
        );

        assert_eq!(light.light_type(), LightType::Spot);
    }

    #[test]
    fn test_attenuation() {
        let attenuation = Attenuation::new(1.0, 0.09, 0.032);

        let factor_near = attenuation.calculate(1.0);
        let factor_far = attenuation.calculate(10.0);

        assert!(factor_near > factor_far);
    }

    #[test]
    fn test_directional_light_contribution() {
        let light = Light::directional("sun".to_string(), Vec3::NEG_Y, Color::WHITE, 1.0);

        let point = Vec3::ZERO;
        let normal = Vec3::Y;

        let contribution = light.calculate_contribution(point, normal);

        // Should receive full light as normal faces the light
        assert!(contribution.r > 0.9);
    }

    #[test]
    fn test_light_environment() {
        let mut env = LightEnvironment::new();

        env.add_light(Light::directional(
            "sun".to_string(),
            Vec3::NEG_Y,
            Color::WHITE,
            1.0,
        ));

        assert_eq!(env.light_count(), 1);

        let lighting = env.calculate_lighting(Vec3::ZERO, Vec3::Y);
        assert!(lighting.r > 0.2); // Should have ambient + directional
    }

    #[test]
    fn test_light_enable_disable() {
        let mut light = Light::directional("sun".to_string(), Vec3::NEG_Y, Color::WHITE, 1.0);

        assert!(light.is_enabled());

        light.set_enabled(false);
        assert!(!light.is_enabled());

        let contribution = light.calculate_contribution(Vec3::ZERO, Vec3::Y);
        assert_eq!(contribution.a, 0.0); // Disabled light contributes nothing
    }
}
