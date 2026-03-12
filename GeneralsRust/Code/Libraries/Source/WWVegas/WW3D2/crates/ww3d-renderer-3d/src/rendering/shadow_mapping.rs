//! Shadow Mapping System
//!
//! Port of shadow.cpp implementing depth-based shadow mapping.
//! Key algorithms:
//! - Shadow map rendering (lines 100-180)
//! - Shadow sampling with PCF (lines 200-230)
//! - Light space transformation

use glam::{Mat4, Vec3, Vec4};

/// Shadow map size presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowMapSize {
    Small = 512,
    Medium = 1024,
    Large = 2048,
    Ultra = 4096,
}

impl ShadowMapSize {
    /// Get size as u32
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Get size from u32
    pub fn from_u32(size: u32) -> Self {
        match size {
            0..=512 => Self::Small,
            513..=1024 => Self::Medium,
            1025..=2048 => Self::Large,
            _ => Self::Ultra,
        }
    }
}

/// Shadow filtering quality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowFilterQuality {
    /// No filtering (hard shadows)
    None,
    /// 2x2 PCF
    Low,
    /// 3x3 PCF
    Medium,
    /// 5x5 PCF
    High,
}

/// Directional light for shadow casting
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    /// Light direction (world space)
    pub direction: Vec3,
    /// Light color
    pub color: Vec3,
    /// Light intensity
    pub intensity: f32,
    /// Shadow casting enabled
    pub cast_shadows: bool,
}

impl DirectionalLight {
    /// Create new directional light
    pub fn new(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            direction: direction.normalize(),
            color,
            intensity,
            cast_shadows: true,
        }
    }

    /// Get light view matrix
    /// Port of shadow.cpp ComputeLightViewMatrix (lines 100-120)
    pub fn get_view_matrix(&self, target: Vec3) -> Mat4 {
        // Position light far along negative direction
        let light_pos = target - self.direction * 100.0;

        // Look at target from light position
        Mat4::look_at_rh(light_pos, target, Vec3::Y)
    }

    /// Get orthographic projection for shadow map
    /// Port of shadow.cpp ComputeLightProjection (lines 122-140)
    pub fn get_projection_matrix(&self, scene_bounds: &ShadowSceneBounds) -> Mat4 {
        let half_width = scene_bounds.width * 0.5;
        let half_height = scene_bounds.height * 0.5;

        Mat4::orthographic_rh(
            -half_width,
            half_width,
            -half_height,
            half_height,
            scene_bounds.near,
            scene_bounds.far,
        )
    }

    /// Get combined view-projection matrix for shadow mapping
    pub fn get_view_proj_matrix(&self, target: Vec3, bounds: &ShadowSceneBounds) -> Mat4 {
        let view = self.get_view_matrix(target);
        let proj = self.get_projection_matrix(bounds);
        proj * view
    }
}

/// Shadow scene bounds for orthographic projection
#[derive(Debug, Clone, Copy)]
pub struct ShadowSceneBounds {
    /// Orthographic width
    pub width: f32,
    /// Orthographic height
    pub height: f32,
    /// Near plane
    pub near: f32,
    /// Far plane
    pub far: f32,
}

impl ShadowSceneBounds {
    /// Create new shadow scene bounds
    pub fn new(width: f32, height: f32, near: f32, far: f32) -> Self {
        Self {
            width,
            height,
            near,
            far,
        }
    }

    /// Create from scene extents
    pub fn from_scene_extents(min: Vec3, max: Vec3) -> Self {
        let size = max - min;
        Self {
            width: size.x.max(size.z),
            height: size.y,
            near: 0.1,
            far: size.length() * 2.0,
        }
    }
}

impl Default for ShadowSceneBounds {
    fn default() -> Self {
        Self::new(100.0, 100.0, 0.1, 200.0)
    }
}

/// Shadow map configuration
#[derive(Debug, Clone)]
pub struct ShadowMapConfig {
    /// Shadow map resolution
    pub size: ShadowMapSize,
    /// Shadow filtering quality
    pub filter_quality: ShadowFilterQuality,
    /// Shadow bias to prevent acne
    pub depth_bias: f32,
    /// Normal offset bias
    pub normal_bias: f32,
    /// Enable Percentage Closer Filtering
    pub enable_pcf: bool,
}

impl ShadowMapConfig {
    /// Create new shadow map configuration
    pub fn new(size: ShadowMapSize) -> Self {
        Self {
            size,
            filter_quality: ShadowFilterQuality::Medium,
            depth_bias: 0.005,
            normal_bias: 0.01,
            enable_pcf: true,
        }
    }

    /// Create high quality configuration
    pub fn high_quality() -> Self {
        Self {
            size: ShadowMapSize::Large,
            filter_quality: ShadowFilterQuality::High,
            depth_bias: 0.003,
            normal_bias: 0.015,
            enable_pcf: true,
        }
    }

    /// Create low quality configuration
    pub fn low_quality() -> Self {
        Self {
            size: ShadowMapSize::Small,
            filter_quality: ShadowFilterQuality::Low,
            depth_bias: 0.01,
            normal_bias: 0.005,
            enable_pcf: false,
        }
    }
}

impl Default for ShadowMapConfig {
    fn default() -> Self {
        Self::new(ShadowMapSize::Medium)
    }
}

/// Shadow map renderer
/// Port of C++ ShadowMapRenderer (shadow.cpp lines 100-230)
pub struct ShadowMapRenderer {
    /// Shadow map configuration
    config: ShadowMapConfig,
    /// Light view-projection matrix
    light_view_proj: Mat4,
    /// Scene bounds for shadow frustum
    scene_bounds: ShadowSceneBounds,
    /// Shadow map texture ID (managed externally)
    shadow_texture_id: Option<u64>,
}

impl ShadowMapRenderer {
    /// Create new shadow map renderer
    pub fn new(config: ShadowMapConfig) -> Self {
        Self {
            config,
            light_view_proj: Mat4::IDENTITY,
            scene_bounds: ShadowSceneBounds::default(),
            shadow_texture_id: None,
        }
    }

    /// Set light and scene bounds
    /// Port of shadow.cpp SetupShadowPass (lines 100-140)
    pub fn setup(
        &mut self,
        light: &DirectionalLight,
        target: Vec3,
        bounds: ShadowSceneBounds,
    ) {
        self.scene_bounds = bounds;
        self.light_view_proj = light.get_view_proj_matrix(target, &bounds);
    }

    /// Get light view-projection matrix
    pub fn get_light_view_proj(&self) -> Mat4 {
        self.light_view_proj
    }

    /// Get shadow map size
    pub fn get_shadow_map_size(&self) -> u32 {
        self.config.size.as_u32()
    }

    /// Set shadow texture ID
    pub fn set_shadow_texture(&mut self, texture_id: u64) {
        self.shadow_texture_id = Some(texture_id);
    }

    /// Get shadow texture ID
    pub fn get_shadow_texture(&self) -> Option<u64> {
        self.shadow_texture_id
    }

    /// Generate WGSL shader code for shadow sampling
    /// Port of shadow.cpp GenerateShadowSamplingCode (lines 200-230)
    pub fn generate_shadow_sampling_code(&self) -> String {
        let pcf_code = if self.config.enable_pcf {
            match self.config.filter_quality {
                ShadowFilterQuality::None => self.generate_basic_sampling(),
                ShadowFilterQuality::Low => self.generate_pcf_2x2(),
                ShadowFilterQuality::Medium => self.generate_pcf_3x3(),
                ShadowFilterQuality::High => self.generate_pcf_5x5(),
            }
        } else {
            self.generate_basic_sampling()
        };

        format!(
            r#"
// Shadow mapping shader code
// Generated with bias: {}, PCF: {}

struct ShadowUniforms {{
    light_view_proj: mat4x4<f32>,
    shadow_bias: f32,
    shadow_map_size: f32,
}}

@group(3) @binding(0)
var<uniform> shadow_uniforms: ShadowUniforms;

@group(3) @binding(1)
var shadow_map: texture_depth_2d;

@group(3) @binding(2)
var shadow_sampler: sampler_comparison;

{}
"#,
            self.config.depth_bias,
            self.config.enable_pcf,
            pcf_code
        )
    }

    /// Generate basic shadow sampling (no PCF)
    fn generate_basic_sampling(&self) -> String {
        format!(
            r#"
fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {{
    // Transform to light space
    let light_space_pos = shadow_uniforms.light_view_proj * vec4<f32>(world_pos, 1.0);

    // Perspective divide
    var shadow_coord = light_space_pos.xyz / light_space_pos.w;

    // Transform to [0, 1] texture coordinates
    shadow_coord.x = shadow_coord.x * 0.5 + 0.5;
    shadow_coord.y = -shadow_coord.y * 0.5 + 0.5;

    // Check if in shadow map bounds
    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0 ||
        shadow_coord.z < 0.0 || shadow_coord.z > 1.0) {{
        return 1.0; // Outside shadow map, fully lit
    }}

    // Apply depth bias
    let biased_depth = shadow_coord.z - {};

    // Sample shadow map
    return textureSampleCompare(shadow_map, shadow_sampler, shadow_coord.xy, biased_depth);
}}
"#,
            self.config.depth_bias
        )
    }

    /// Generate 2x2 PCF sampling
    fn generate_pcf_2x2(&self) -> String {
        format!(
            r#"
fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {{
    let light_space_pos = shadow_uniforms.light_view_proj * vec4<f32>(world_pos, 1.0);
    var shadow_coord = light_space_pos.xyz / light_space_pos.w;

    shadow_coord.x = shadow_coord.x * 0.5 + 0.5;
    shadow_coord.y = -shadow_coord.y * 0.5 + 0.5;

    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0 ||
        shadow_coord.z < 0.0 || shadow_coord.z > 1.0) {{
        return 1.0;
    }}

    let biased_depth = shadow_coord.z - {};
    let texel_size = 1.0 / shadow_uniforms.shadow_map_size;

    var shadow = 0.0;
    for (var x = -1; x <= 0; x++) {{
        for (var y = -1; y <= 0; y++) {{
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(shadow_map, shadow_sampler,
                                          shadow_coord.xy + offset, biased_depth);
        }}
    }}

    return shadow / 4.0;
}}
"#,
            self.config.depth_bias
        )
    }

    /// Generate 3x3 PCF sampling
    fn generate_pcf_3x3(&self) -> String {
        format!(
            r#"
fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {{
    let light_space_pos = shadow_uniforms.light_view_proj * vec4<f32>(world_pos, 1.0);
    var shadow_coord = light_space_pos.xyz / light_space_pos.w;

    shadow_coord.x = shadow_coord.x * 0.5 + 0.5;
    shadow_coord.y = -shadow_coord.y * 0.5 + 0.5;

    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0 ||
        shadow_coord.z < 0.0 || shadow_coord.z > 1.0) {{
        return 1.0;
    }}

    let biased_depth = shadow_coord.z - {};
    let texel_size = 1.0 / shadow_uniforms.shadow_map_size;

    var shadow = 0.0;
    for (var x = -1; x <= 1; x++) {{
        for (var y = -1; y <= 1; y++) {{
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(shadow_map, shadow_sampler,
                                          shadow_coord.xy + offset, biased_depth);
        }}
    }}

    return shadow / 9.0;
}}
"#,
            self.config.depth_bias
        )
    }

    /// Generate 5x5 PCF sampling
    fn generate_pcf_5x5(&self) -> String {
        format!(
            r#"
fn sample_shadow_map(world_pos: vec3<f32>) -> f32 {{
    let light_space_pos = shadow_uniforms.light_view_proj * vec4<f32>(world_pos, 1.0);
    var shadow_coord = light_space_pos.xyz / light_space_pos.w;

    shadow_coord.x = shadow_coord.x * 0.5 + 0.5;
    shadow_coord.y = -shadow_coord.y * 0.5 + 0.5;

    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0 ||
        shadow_coord.z < 0.0 || shadow_coord.z > 1.0) {{
        return 1.0;
    }}

    let biased_depth = shadow_coord.z - {};
    let texel_size = 1.0 / shadow_uniforms.shadow_map_size;

    var shadow = 0.0;
    for (var x = -2; x <= 2; x++) {{
        for (var y = -2; y <= 2; y++) {{
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(shadow_map, shadow_sampler,
                                          shadow_coord.xy + offset, biased_depth);
        }}
    }}

    return shadow / 25.0;
}}
"#,
            self.config.depth_bias
        )
    }

    /// Calculate shadow factor for a world position (CPU version)
    pub fn calculate_shadow_factor(&self, world_pos: Vec3, shadow_depth: f32) -> f32 {
        // Transform to light space
        let light_space = self.light_view_proj * Vec4::from((world_pos, 1.0));
        let ndc = light_space.xyz() / light_space.w;

        // Transform to texture coordinates
        let shadow_coord = Vec3::new(
            ndc.x * 0.5 + 0.5,
            -ndc.y * 0.5 + 0.5,
            ndc.z,
        );

        // Check bounds
        if shadow_coord.x < 0.0
            || shadow_coord.x > 1.0
            || shadow_coord.y < 0.0
            || shadow_coord.y > 1.0
        {
            return 1.0; // Outside shadow map
        }

        // Compare depths with bias
        let biased_depth = shadow_coord.z - self.config.depth_bias;
        if biased_depth < shadow_depth {
            1.0 // Lit
        } else {
            0.0 // Shadowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_map_size() {
        assert_eq!(ShadowMapSize::Small.as_u32(), 512);
        assert_eq!(ShadowMapSize::Medium.as_u32(), 1024);
        assert_eq!(ShadowMapSize::Large.as_u32(), 2048);
        assert_eq!(ShadowMapSize::Ultra.as_u32(), 4096);
    }

    #[test]
    fn test_shadow_map_size_from_u32() {
        assert_eq!(ShadowMapSize::from_u32(256), ShadowMapSize::Small);
        assert_eq!(ShadowMapSize::from_u32(1024), ShadowMapSize::Medium);
        assert_eq!(ShadowMapSize::from_u32(2048), ShadowMapSize::Large);
        assert_eq!(ShadowMapSize::from_u32(8192), ShadowMapSize::Ultra);
    }

    #[test]
    fn test_directional_light_creation() {
        let light = DirectionalLight::new(Vec3::new(1.0, -1.0, 0.0), Vec3::ONE, 1.0);
        assert!(light.direction.is_normalized());
        assert!(light.cast_shadows);
    }

    #[test]
    fn test_shadow_scene_bounds() {
        let bounds = ShadowSceneBounds::new(100.0, 50.0, 0.1, 200.0);
        assert_eq!(bounds.width, 100.0);
        assert_eq!(bounds.height, 50.0);
        assert_eq!(bounds.near, 0.1);
        assert_eq!(bounds.far, 200.0);
    }

    #[test]
    fn test_shadow_scene_bounds_from_extents() {
        let min = Vec3::new(-10.0, 0.0, -10.0);
        let max = Vec3::new(10.0, 20.0, 10.0);
        let bounds = ShadowSceneBounds::from_scene_extents(min, max);

        assert!(bounds.width >= 20.0);
        assert_eq!(bounds.height, 20.0);
    }

    #[test]
    fn test_shadow_map_config() {
        let config = ShadowMapConfig::new(ShadowMapSize::Medium);
        assert_eq!(config.size, ShadowMapSize::Medium);
        assert!(config.enable_pcf);
    }

    #[test]
    fn test_shadow_map_config_presets() {
        let high = ShadowMapConfig::high_quality();
        assert_eq!(high.size, ShadowMapSize::Large);
        assert_eq!(high.filter_quality, ShadowFilterQuality::High);

        let low = ShadowMapConfig::low_quality();
        assert_eq!(low.size, ShadowMapSize::Small);
        assert!(!low.enable_pcf);
    }

    #[test]
    fn test_shadow_map_renderer_creation() {
        let config = ShadowMapConfig::default();
        let renderer = ShadowMapRenderer::new(config);

        assert_eq!(renderer.get_shadow_map_size(), 1024);
        assert!(renderer.get_shadow_texture().is_none());
    }

    #[test]
    fn test_shadow_map_renderer_setup() {
        let config = ShadowMapConfig::default();
        let mut renderer = ShadowMapRenderer::new(config);

        let light = DirectionalLight::new(Vec3::new(0.0, -1.0, 0.0), Vec3::ONE, 1.0);
        let bounds = ShadowSceneBounds::default();

        renderer.setup(&light, Vec3::ZERO, bounds);

        let view_proj = renderer.get_light_view_proj();
        assert_ne!(view_proj, Mat4::IDENTITY);
    }

    #[test]
    fn test_shader_code_generation() {
        let config = ShadowMapConfig::default();
        let renderer = ShadowMapRenderer::new(config);

        let shader = renderer.generate_shadow_sampling_code();
        assert!(shader.contains("sample_shadow_map"));
        assert!(shader.contains("shadow_map"));
        assert!(shader.contains("light_view_proj"));
    }

    #[test]
    fn test_shader_code_generation_quality_levels() {
        let configs = [
            ShadowMapConfig::low_quality(),
            ShadowMapConfig::default(),
            ShadowMapConfig::high_quality(),
        ];

        for config in configs {
            let renderer = ShadowMapRenderer::new(config);
            let shader = renderer.generate_shadow_sampling_code();
            assert!(shader.contains("sample_shadow_map"));
        }
    }

    #[test]
    fn test_light_view_matrix() {
        let light = DirectionalLight::new(Vec3::new(0.0, -1.0, 0.0), Vec3::ONE, 1.0);
        let view = light.get_view_matrix(Vec3::ZERO);

        // View matrix should not be identity
        assert_ne!(view, Mat4::IDENTITY);
    }

    #[test]
    fn test_light_projection_matrix() {
        let light = DirectionalLight::new(Vec3::new(0.0, -1.0, 0.0), Vec3::ONE, 1.0);
        let bounds = ShadowSceneBounds::default();
        let proj = light.get_projection_matrix(&bounds);

        // Projection matrix should not be identity
        assert_ne!(proj, Mat4::IDENTITY);
    }
}
