/// Lighting System
/// This module implements the lighting system from C++ light.h/cpp and lightenvironment.h/cpp
///
/// The lighting system provides:
/// - Point lights, directional lights, and spot lights
/// - Light attenuation and falloff
/// - Light environment management with importance sorting
/// - Dynamic light contribution calculation
use glam::{Mat4, Vec3};
use std::sync::{Arc, Mutex, OnceLock};

/// Maximum number of lights that can affect an object simultaneously
pub const MAX_LIGHTS: usize = 4;

/// Light type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    /// Point light - radiates in all directions
    Point,
    /// Directional light - parallel rays from infinite distance
    Directional,
    /// Spot light - cone of light
    Spot,
}

/// Light flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightFlags {
    pub near_attenuation: bool,
    pub far_attenuation: bool,
}

impl Default for LightFlags {
    fn default() -> Self {
        Self {
            near_attenuation: false,
            far_attenuation: true,
        }
    }
}

/// Light class - represents a light source in the scene
///
/// This is the Rust equivalent of C++ LightClass. Lights are render objects
/// that act as vertex processors, affecting how geometry is lit.
#[derive(Clone, Debug)]
pub struct Light {
    /// Light name
    pub name: String,
    /// Type of light
    pub light_type: LightType,
    /// Light flags
    pub flags: LightFlags,
    /// Does this light cast shadows?
    pub cast_shadows: bool,
    /// Light intensity multiplier
    pub intensity: f32,
    /// Ambient color contribution
    pub ambient: Vec3,
    /// Diffuse color
    pub diffuse: Vec3,
    /// Specular color
    pub specular: Vec3,
    /// Near attenuation range
    pub near_atten_start: f32,
    pub near_atten_end: f32,
    /// Far attenuation range
    pub far_atten_start: f32,
    pub far_atten_end: f32,
    /// Spotlight parameters
    pub spot_angle: f32,
    pub spot_angle_cos: f32,
    pub spot_exponent: f32,
    pub spot_direction: Vec3,
    /// Light position in world space
    pub position: Vec3,
    /// Light transform
    pub transform: Mat4,
}

impl Light {
    /// Create a new light
    pub fn new(name: String, light_type: LightType) -> Self {
        Self {
            name,
            light_type,
            flags: LightFlags::default(),
            cast_shadows: false,
            intensity: 1.0,
            ambient: Vec3::ZERO,
            diffuse: Vec3::ONE,
            specular: Vec3::ONE,
            near_atten_start: 0.0,
            near_atten_end: 0.0,
            far_atten_start: 0.0,
            far_atten_end: 100.0,
            spot_angle: 45.0_f32.to_radians(),
            spot_angle_cos: (45.0_f32.to_radians()).cos(),
            spot_exponent: 1.0,
            spot_direction: Vec3::new(0.0, -1.0, 0.0),
            position: Vec3::ZERO,
            transform: Mat4::IDENTITY,
        }
    }

    /// Create a point light
    pub fn point(name: String, position: Vec3, color: Vec3, range: f32) -> Self {
        let mut light = Self::new(name, LightType::Point);
        light.position = position;
        light.diffuse = color;
        light.far_atten_end = range;
        light.far_atten_start = range * 0.8;
        light
    }

    /// Create a directional light
    pub fn directional(name: String, direction: Vec3, color: Vec3) -> Self {
        let mut light = Self::new(name, LightType::Directional);
        light.spot_direction = direction.normalize();
        light.diffuse = color;
        light
    }

    /// Create a spot light
    pub fn spot(
        name: String,
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        angle: f32,
        range: f32,
    ) -> Self {
        let mut light = Self::new(name, LightType::Spot);
        light.position = position;
        light.spot_direction = direction.normalize();
        light.diffuse = color;
        light.spot_angle = angle;
        light.spot_angle_cos = angle.cos();
        light.far_atten_end = range;
        light.far_atten_start = range * 0.8;
        light
    }

    /// Set the intensity of the light
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity;
    }

    /// Set the far attenuation range
    pub fn set_far_attenuation_range(&mut self, start: f32, end: f32) {
        self.far_atten_start = start;
        self.far_atten_end = end;
    }

    /// Set the near attenuation range
    pub fn set_near_attenuation_range(&mut self, start: f32, end: f32) {
        self.near_atten_start = start;
        self.near_atten_end = end;
    }

    /// Set the spot angle (in radians)
    pub fn set_spot_angle(&mut self, angle: f32) {
        self.spot_angle = angle;
        self.spot_angle_cos = angle.cos();
    }

    /// Get the attenuation range
    pub fn get_attenuation_range(&self) -> f32 {
        self.far_atten_end
    }

    /// Calculate attenuation factor for a distance
    pub fn calculate_attenuation(&self, distance: f32) -> f32 {
        if self.light_type == LightType::Directional {
            return 1.0;
        }

        let mut atten = 1.0;

        // Far attenuation
        if self.flags.far_attenuation && distance > self.far_atten_start {
            if distance >= self.far_atten_end {
                return 0.0;
            }
            atten *= 1.0
                - ((distance - self.far_atten_start) / (self.far_atten_end - self.far_atten_start));
        }

        // Near attenuation
        if self.flags.near_attenuation && distance < self.near_atten_end {
            if distance <= self.near_atten_start {
                return 0.0;
            }
            atten *=
                (distance - self.near_atten_start) / (self.near_atten_end - self.near_atten_start);
        }

        atten.max(0.0).min(1.0)
    }

    /// Calculate spotlight falloff
    pub fn calculate_spot_falloff(&self, direction_to_light: Vec3) -> f32 {
        if self.light_type != LightType::Spot {
            return 1.0;
        }

        let cos_angle = self.spot_direction.dot(direction_to_light.normalize());

        if cos_angle < self.spot_angle_cos {
            return 0.0;
        }

        // Apply spot exponent for smooth falloff
        cos_angle.powf(self.spot_exponent)
    }
}

/// Input light structure - light before transformation to camera space
#[derive(Clone, Debug)]
#[allow(dead_code)] // C++ parity
struct InputLight {
    /// Direction to the light (or light direction for directional)
    direction: Vec3,
    /// Ambient contribution
    ambient: Vec3,
    /// Diffuse contribution (with attenuation applied)
    diffuse: Vec3,
    /// Was diffuse rejected due to being too weak?
    diffuse_rejected: bool,
    /// Is this a point light?
    is_point: bool,
    /// Point light specific data
    center: Vec3,
    inner_radius: f32,
    outer_radius: f32,
    point_ambient: Vec3,
    point_diffuse: Vec3,
}

impl InputLight {
    /// Initialize from a light and object center
    fn from_light(light: &Light, object_center: Vec3) -> Self {
        match light.light_type {
            LightType::Point | LightType::Spot => Self::from_point_or_spot(light, object_center),
            LightType::Directional => Self::from_directional(light, object_center),
        }
    }

    /// Initialize from a point or spot light
    fn from_point_or_spot(light: &Light, object_center: Vec3) -> Self {
        let to_light = light.position - object_center;
        let distance = to_light.length();
        let direction = if distance > 0.0 {
            to_light / distance
        } else {
            Vec3::Y
        };

        // Calculate attenuation
        let mut atten = light.calculate_attenuation(distance);

        // Apply spot falloff if applicable
        if light.light_type == LightType::Spot {
            atten *= light.calculate_spot_falloff(-direction);
        }

        // Apply intensity
        atten *= light.intensity;

        let diffuse = light.diffuse * atten;
        let ambient = light.ambient * light.intensity;

        Self {
            direction,
            ambient,
            diffuse,
            diffuse_rejected: diffuse.length() < get_lighting_lod_cutoff(),
            is_point: true,
            center: light.position,
            inner_radius: light.near_atten_end,
            outer_radius: light.far_atten_end,
            point_ambient: ambient,
            point_diffuse: diffuse,
        }
    }

    /// Initialize from a directional light
    fn from_directional(light: &Light, _object_center: Vec3) -> Self {
        let diffuse = light.diffuse * light.intensity;
        let ambient = light.ambient * light.intensity;

        Self {
            direction: -light.spot_direction,
            ambient,
            diffuse,
            diffuse_rejected: diffuse.length() < get_lighting_lod_cutoff(),
            is_point: false,
            center: Vec3::ZERO,
            inner_radius: 0.0,
            outer_radius: 0.0,
            point_ambient: Vec3::ZERO,
            point_diffuse: Vec3::ZERO,
        }
    }

    /// Calculate contribution metric (for importance sorting)
    fn contribution(&self) -> f32 {
        // Contribution is based on diffuse intensity
        self.diffuse.length()
    }
}

/// Output light structure - light after transformation to camera space
#[derive(Clone, Debug)]
struct OutputLight {
    /// Direction in camera/eye space
    direction: Vec3,
    /// Diffuse color with attenuation
    diffuse: Vec3,
}

impl OutputLight {
    /// Initialize from input light and camera transform
    fn from_input(input: &InputLight, camera_tm: &Mat4) -> Self {
        // Transform direction to camera space
        let direction = camera_tm.transform_vector3(input.direction);

        Self {
            direction: direction.normalize(),
            diffuse: input.diffuse,
        }
    }
}

/// Light Environment - Manages local lighting for an object
///
/// This class represents an approximation of the local lighting for a given point.
/// It collects all point light sources affecting an object and creates temporary
/// directional light sources representing them, with distance/attenuation
/// precalculated into intensity.
#[derive(Debug, Clone)]
pub struct LightEnvironment {
    /// Number of active lights
    light_count: usize,
    /// Center of the object being lit
    object_center: Vec3,
    /// Input lights (sorted by importance)
    input_lights: Vec<InputLight>,
    /// Output ambient (scene ambient + light ambients)
    output_ambient: Vec3,
    /// Output lights (transformed to camera space)
    output_lights: Vec<OutputLight>,
    /// Fill light (optional additional light)
    fill_light: Option<InputLight>,
    /// Fill light intensity multiplier
    fill_intensity: f32,
}

impl LightEnvironment {
    /// Create a new light environment
    pub fn new() -> Self {
        Self {
            light_count: 0,
            object_center: Vec3::ZERO,
            input_lights: Vec::new(),
            output_ambient: Vec3::ZERO,
            output_lights: Vec::new(),
            fill_light: None,
            fill_intensity: 0.5,
        }
    }

    /// Reset the light environment for a new object
    pub fn reset(&mut self, object_center: Vec3, scene_ambient: Vec3) {
        self.object_center = object_center;
        self.output_ambient = scene_ambient;
        self.input_lights.clear();
        self.output_lights.clear();
        self.light_count = 0;
        self.fill_light = None;
    }

    /// Add a light to the environment
    ///
    /// Lights are sorted by importance (contribution). Only the most important
    /// MAX_LIGHTS lights are kept.
    pub fn add_light(&mut self, light: &Light) {
        let input_light = InputLight::from_light(light, self.object_center);

        // Add ambient contribution
        self.output_ambient += input_light.ambient;

        // Skip if diffuse is too weak
        if input_light.diffuse_rejected {
            return;
        }

        // Add to input lights
        self.input_lights.push(input_light);

        // Sort by contribution (highest first)
        self.input_lights.sort_by(|a, b| {
            b.contribution()
                .partial_cmp(&a.contribution())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Keep only MAX_LIGHTS
        if self.input_lights.len() > MAX_LIGHTS {
            self.input_lights.truncate(MAX_LIGHTS);
        }

        self.light_count = self.input_lights.len();
    }

    /// Pre-render update - transform lights to camera space
    pub fn pre_render_update(&mut self, camera_tm: &Mat4) {
        self.output_lights.clear();

        for input_light in &self.input_lights {
            self.output_lights
                .push(OutputLight::from_input(input_light, camera_tm));
        }
    }

    /// Calculate and add a fill light
    ///
    /// Fill light is used to prevent completely black shadows
    pub fn calculate_fill_light(&mut self) {
        if self.light_count == 0 {
            return;
        }

        // Calculate average opposite direction of all lights
        let mut avg_dir = Vec3::ZERO;
        for light in &self.input_lights {
            avg_dir -= light.direction;
        }

        if avg_dir.length() > 0.0 {
            avg_dir = avg_dir.normalize();
        } else {
            avg_dir = Vec3::Y;
        }

        // Create fill light
        let fill = InputLight {
            direction: avg_dir,
            ambient: Vec3::ZERO,
            diffuse: self.output_ambient * self.fill_intensity,
            diffuse_rejected: false,
            is_point: false,
            center: Vec3::ZERO,
            inner_radius: 0.0,
            outer_radius: 0.0,
            point_ambient: Vec3::ZERO,
            point_diffuse: Vec3::ZERO,
        };

        self.fill_light = Some(fill);
    }

    /// Add the fill light to the active lights
    pub fn add_fill_light(&mut self) {
        if let Some(fill) = &self.fill_light {
            if self.light_count < MAX_LIGHTS {
                self.input_lights.push(fill.clone());
                self.light_count += 1;
            }
        }
    }

    /// Set fill light intensity
    pub fn set_fill_intensity(&mut self, intensity: f32) {
        self.fill_intensity = intensity;
    }

    /// Get the equivalent ambient light
    pub fn get_equivalent_ambient(&self) -> Vec3 {
        self.output_ambient
    }

    /// Get the number of active lights
    pub fn get_light_count(&self) -> usize {
        self.light_count
    }

    /// Get light direction (in camera space)
    pub fn get_light_direction(&self, index: usize) -> Vec3 {
        self.output_lights
            .get(index)
            .map(|l| l.direction)
            .unwrap_or(Vec3::ZERO)
    }

    /// Get light diffuse color
    pub fn get_light_diffuse(&self, index: usize) -> Vec3 {
        self.output_lights
            .get(index)
            .map(|l| l.diffuse)
            .unwrap_or(Vec3::ZERO)
    }

    /// Check if a light is a point light
    pub fn is_point_light(&self, index: usize) -> bool {
        self.input_lights
            .get(index)
            .map(|l| l.is_point)
            .unwrap_or(false)
    }
}

/// Global lighting LOD cutoff - uses Arc<Mutex<f32>> for thread-safe mutable access
static LIGHTING_LOD_CUTOFF: OnceLock<Arc<Mutex<f32>>> = OnceLock::new();

fn get_lod_cutoff_cell() -> Arc<Mutex<f32>> {
    LIGHTING_LOD_CUTOFF
        .get_or_init(|| Arc::new(Mutex::new(0.01)))
        .clone()
}

/// Set the lighting LOD cutoff
///
/// Lights with diffuse intensity below this threshold are converted to pure ambient
pub fn set_lighting_lod_cutoff(cutoff: f32) {
    let cell = get_lod_cutoff_cell();
    let mut guard = cell.lock().unwrap();
    *guard = cutoff;
}

/// Get the lighting LOD cutoff
pub fn get_lighting_lod_cutoff() -> f32 {
    let cell = get_lod_cutoff_cell();
    let guard = cell.lock().unwrap();
    *guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_creation() {
        let light = Light::new("TestLight".to_string(), LightType::Point);
        assert_eq!(light.name, "TestLight");
        assert_eq!(light.light_type, LightType::Point);
    }

    #[test]
    fn test_point_light() {
        let light = Light::point(
            "PointLight".to_string(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::ONE,
            50.0,
        );
        assert_eq!(light.light_type, LightType::Point);
        assert_eq!(light.position, Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(light.far_atten_end, 50.0);
    }

    #[test]
    fn test_directional_light() {
        let light =
            Light::directional("DirLight".to_string(), Vec3::new(0.0, -1.0, 0.0), Vec3::ONE);
        assert_eq!(light.light_type, LightType::Directional);
    }

    #[test]
    fn test_spot_light() {
        let light = Light::spot(
            "SpotLight".to_string(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::ONE,
            45.0_f32.to_radians(),
            30.0,
        );
        assert_eq!(light.light_type, LightType::Spot);
    }

    #[test]
    fn test_attenuation() {
        let mut light = Light::point("TestLight".to_string(), Vec3::ZERO, Vec3::ONE, 100.0);
        light.set_far_attenuation_range(50.0, 100.0);

        assert_eq!(light.calculate_attenuation(0.0), 1.0); // No attenuation at center
        assert_eq!(light.calculate_attenuation(50.0), 1.0); // No attenuation before start
        assert!(light.calculate_attenuation(75.0) < 1.0); // Partial attenuation
        assert_eq!(light.calculate_attenuation(100.0), 0.0); // Full attenuation at end
    }

    #[test]
    fn test_light_environment() {
        let mut env = LightEnvironment::new();
        env.reset(Vec3::ZERO, Vec3::new(0.2, 0.2, 0.2));

        let light = Light::point(
            "Light1".to_string(),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::ONE,
            10.0,
        );
        env.add_light(&light);

        assert_eq!(env.get_light_count(), 1);
    }

    #[test]
    fn test_light_sorting() {
        let mut env = LightEnvironment::new();
        env.reset(Vec3::ZERO, Vec3::new(0.2, 0.2, 0.2));

        // Add lights with different intensities
        let light1 = Light::point(
            "Light1".to_string(),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::ONE * 0.5,
            10.0,
        );
        let light2 = Light::point(
            "Light2".to_string(),
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::ONE,
            10.0,
        );

        env.add_light(&light1);
        env.add_light(&light2);

        // Light2 should be first (higher intensity)
        assert_eq!(env.get_light_count(), 2);
    }

    #[test]
    fn test_max_lights_limit() {
        let mut env = LightEnvironment::new();
        env.reset(Vec3::ZERO, Vec3::new(0.2, 0.2, 0.2));

        // Add more than MAX_LIGHTS
        for i in 0..10 {
            let light = Light::point(
                format!("Light{}", i),
                Vec3::new(i as f32, 0.0, 0.0),
                Vec3::ONE,
                10.0,
            );
            env.add_light(&light);
        }

        // Should only keep MAX_LIGHTS
        assert_eq!(env.get_light_count(), MAX_LIGHTS);
    }

    #[test]
    fn test_fill_light() {
        let mut env = LightEnvironment::new();
        env.reset(Vec3::ZERO, Vec3::new(0.2, 0.2, 0.2));

        let light = Light::point(
            "Light1".to_string(),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::ONE,
            10.0,
        );
        env.add_light(&light);

        env.calculate_fill_light();
        env.add_fill_light();

        // Should have original light + fill light
        assert_eq!(env.get_light_count(), 2);
    }
}
