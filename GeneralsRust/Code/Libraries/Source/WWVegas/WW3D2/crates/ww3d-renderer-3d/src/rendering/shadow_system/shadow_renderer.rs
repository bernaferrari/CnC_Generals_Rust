//! Shadow Renderer
//!
//! This module provides comprehensive shadow rendering functionality,
//! including shadow map generation, cascaded shadow maps, and point light shadows.

use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::{
    Device, RenderPass, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use super::shadow_map::ShadowCasterSubmission;

/// Shadow map configuration
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// Shadow map resolution
    pub shadow_map_size: u32,
    /// Number of cascade levels for directional lights
    pub cascade_count: u32,
    /// Shadow bias to prevent shadow acne
    pub shadow_bias: f32,
    /// Softness factor for shadow edges
    pub shadow_softness: f32,
    /// Maximum shadow distance
    pub max_shadow_distance: f32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            shadow_map_size: 1024,
            cascade_count: 4,
            shadow_bias: 0.005,
            shadow_softness: 1.0,
            max_shadow_distance: 100.0,
        }
    }
}

/// Light type for shadow casting
#[derive(Debug, Clone)]
pub enum ShadowLight {
    Directional {
        direction: Vec3,
        view_matrix: Mat4,
        projection_matrix: Mat4,
    },
    Point {
        position: Vec3,
        range: f32,
        cube_faces: [Mat4; 6], // View matrices for each cube face
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
        view_matrix: Mat4,
        projection_matrix: Mat4,
    },
}

/// Shadow renderer for managing shadow map rendering
#[derive(Debug)]
pub struct ShadowRenderer {
    /// Whether shadows are enabled
    pub enabled: bool,
    /// Shadow configuration
    pub config: ShadowConfig,
    /// Shadow-casting lights
    pub lights: Vec<ShadowLight>,
    /// Shadow map textures (light_id -> texture_view)
    pub shadow_maps: HashMap<usize, Box<dyn std::any::Any>>, // TextureView storage
    /// Runtime shadow caster submissions; used for per-pass caster accounting.
    pub shadow_caster_submissions: Vec<ShadowCasterSubmission>,
    /// Optional per-light caster submissions used by `render_shadows()`.
    pub shadow_caster_submissions_by_light: HashMap<usize, Vec<ShadowCasterSubmission>>,
    /// Number of shadow casters expected for each shadow pass.
    pub shadow_caster_count_hint: u32,
    /// Shadow statistics
    pub stats: ShadowStats,
}

/// Runtime shadow-map resource owned by the renderer.
pub struct ShadowMapResource {
    pub texture: Texture,
    pub view: wgpu::TextureView,
    pub format: TextureFormat,
    pub size: u32,
}

/// Shadow rendering statistics
#[derive(Debug, Clone, Default)]
pub struct ShadowStats {
    /// Number of shadow maps rendered this frame
    pub shadow_maps_rendered: u32,
    /// Number of objects rendered to shadow maps
    pub shadow_casters_rendered: u32,
    /// Total shadow map memory usage
    pub shadow_map_memory_mb: f32,
}

impl ShadowRenderer {
    /// Create a new shadow renderer
    pub fn new() -> Self {
        Self {
            enabled: true,
            config: ShadowConfig::default(),
            lights: Vec::new(),
            shadow_maps: HashMap::new(),
            shadow_caster_submissions: Vec::new(),
            shadow_caster_submissions_by_light: HashMap::new(),
            shadow_caster_count_hint: 0,
            stats: ShadowStats::default(),
        }
    }

    /// Create shadow renderer with custom configuration
    pub fn with_config(config: ShadowConfig) -> Self {
        Self {
            enabled: true,
            config,
            lights: Vec::new(),
            shadow_maps: HashMap::new(),
            shadow_caster_submissions: Vec::new(),
            shadow_caster_submissions_by_light: HashMap::new(),
            shadow_caster_count_hint: 0,
            stats: ShadowStats::default(),
        }
    }

    /// Add a light that can cast shadows
    pub fn add_shadow_light(&mut self, light: ShadowLight) {
        self.lights.push(light);
    }

    /// Remove all shadow lights
    pub fn clear_lights(&mut self) {
        self.lights.clear();
        self.shadow_maps.clear();
    }

    /// Render shadows to the render pass
    pub fn render_shadows(&mut self, render_pass: &mut RenderPass) {
        let per_light = self.shadow_caster_submissions_by_light.clone();
        self.render_shadows_with_submissions(render_pass, &per_light);
    }

    /// Render shadows with optional per-light caster submissions.
    ///
    /// When a light has an entry in `submissions_by_light`, that entry is used for pass
    /// accounting; otherwise the renderer falls back to globally registered submissions
    /// (`set_shadow_caster_submissions`) and then to `shadow_caster_count_hint`.
    pub fn render_shadows_with_submissions(
        &mut self,
        render_pass: &mut RenderPass,
        submissions_by_light: &HashMap<usize, Vec<ShadowCasterSubmission>>,
    ) {
        if !self.enabled {
            return;
        }

        self.stats.shadow_maps_rendered = 0;
        self.stats.shadow_casters_rendered = 0;

        let lights_data: Vec<(usize, ShadowLight)> = self
            .lights
            .iter()
            .enumerate()
            .map(|(i, light)| (i, light.clone()))
            .collect();

        for (light_index, light) in lights_data {
            let light_submissions = submissions_by_light.get(&light_index).map(Vec::as_slice);
            match light {
                ShadowLight::Directional { .. } => {
                    self.render_directional_shadow_map(
                        light_index,
                        &light,
                        render_pass,
                        light_submissions,
                    );
                }
                ShadowLight::Point { .. } => {
                    self.render_point_shadow_map(
                        light_index,
                        &light,
                        render_pass,
                        light_submissions,
                    );
                }
                ShadowLight::Spot { .. } => {
                    self.render_spot_shadow_map(
                        light_index,
                        &light,
                        render_pass,
                        light_submissions,
                    );
                }
            }
            self.stats.shadow_maps_rendered += 1;
        }
    }

    fn bytes_per_pixel(format: TextureFormat) -> u32 {
        use TextureFormat as Tf;
        match format {
            Tf::Depth16Unorm => 2,
            Tf::Depth24Plus
            | Tf::Depth24PlusStencil8
            | Tf::Depth32Float
            | Tf::Depth32FloatStencil8 => 4,
            Tf::Rgba16Float | Tf::Rgba16Uint | Tf::Rgba16Sint => 8,
            _ => 4,
        }
    }

    fn shadow_map_memory_mb(size: u32, format: TextureFormat) -> f32 {
        let bytes = size as f32 * size as f32 * Self::bytes_per_pixel(format) as f32;
        bytes / (1024.0 * 1024.0)
    }

    fn effective_shadow_caster_count(
        &self,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) -> u32 {
        if let Some(submissions) = light_submissions {
            submissions
                .iter()
                .filter(|submission| submission.is_renderable())
                .count() as u32
        } else {
            if self.shadow_caster_submissions.is_empty() {
                self.shadow_caster_count_hint
            } else {
                self.shadow_caster_submissions
                    .iter()
                    .filter(|submission| submission.is_renderable())
                    .count() as u32
            }
        }
    }

    fn accumulate_shadow_caster_pass(
        &mut self,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        let casters_this_pass = self.effective_shadow_caster_count(light_submissions);

        self.stats.shadow_casters_rendered = self
            .stats
            .shadow_casters_rendered
            .saturating_add(casters_this_pass);
    }

    /// Render shadow map for directional light
    fn render_directional_shadow_map(
        &mut self,
        light_index: usize,
        light: &ShadowLight,
        render_pass: &mut RenderPass,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        if let ShadowLight::Directional {
            direction: _,
            view_matrix: _,
            projection_matrix: _,
        } = light
        {
            // Set up shadow map rendering viewport
            // This would involve:
            // 1. Setting the shadow map as render target
            // 2. Clearing the depth buffer
            // 3. Setting the light's view-projection matrix
            // 4. Rendering all shadow-casting objects

            // For cascaded shadow maps, this would be repeated for each cascade
            for cascade in 0..self.config.cascade_count {
                self.render_shadow_cascade(light_index, cascade, render_pass, light_submissions);
            }
        }
    }

    /// Render shadow map for point light
    fn render_point_shadow_map(
        &mut self,
        light_index: usize,
        light: &ShadowLight,
        render_pass: &mut RenderPass,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        if let ShadowLight::Point {
            position: _,
            range: _,
            cube_faces,
        } = light
        {
            // Render to each face of the cube map
            for (face_index, face_matrix) in cube_faces.iter().enumerate() {
                self.render_point_shadow_face(
                    light_index,
                    face_index,
                    *face_matrix,
                    render_pass,
                    light_submissions,
                );
            }
        }
    }

    /// Render shadow map for spot light
    fn render_spot_shadow_map(
        &mut self,
        light_index: usize,
        light: &ShadowLight,
        render_pass: &mut RenderPass,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        if let ShadowLight::Spot {
            position: _,
            direction: _,
            range: _,
            inner_angle: _,
            outer_angle: _,
            view_matrix: _,
            projection_matrix: _,
        } = light
        {
            // Render shadow map similar to directional light but with perspective projection
            self.render_spot_shadow_frustum(light_index, render_pass, light_submissions);
        }
    }

    /// Render a single shadow cascade
    fn render_shadow_cascade(
        &mut self,
        _light_index: usize,
        _cascade: u32,
        _render_pass: &mut RenderPass,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        self.accumulate_shadow_caster_pass(light_submissions);
    }

    /// Render one face of a point light shadow cube map
    fn render_point_shadow_face(
        &mut self,
        _light_index: usize,
        _face_index: usize,
        _face_matrix: Mat4,
        _render_pass: &mut RenderPass,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        self.accumulate_shadow_caster_pass(light_submissions);
    }

    /// Render spot light shadow frustum
    fn render_spot_shadow_frustum(
        &mut self,
        _light_index: usize,
        _render_pass: &mut RenderPass,
        light_submissions: Option<&[ShadowCasterSubmission]>,
    ) {
        self.accumulate_shadow_caster_pass(light_submissions);
    }

    /// Create shadow map texture for a light
    pub fn create_shadow_map(
        &mut self,
        device: &Device,
        light_index: usize,
        format: TextureFormat,
    ) {
        let size = self.config.shadow_map_size.max(1);
        let memory_delta_mb = Self::shadow_map_memory_mb(size, format);

        if let Some(previous) = self.shadow_maps.remove(&light_index) {
            if let Ok(previous_resource) = previous.downcast::<ShadowMapResource>() {
                let previous_mb =
                    Self::shadow_map_memory_mb(previous_resource.size, previous_resource.format);
                self.stats.shadow_map_memory_mb =
                    (self.stats.shadow_map_memory_mb - previous_mb).max(0.0);
            }
        }

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Renderer Shadow Map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.shadow_maps.insert(
            light_index,
            Box::new(ShadowMapResource {
                texture,
                view,
                format,
                size,
            }),
        );
        self.stats.shadow_map_memory_mb += memory_delta_mb;
    }

    /// Get shadow map for a light
    pub fn get_shadow_map(&self, light_index: usize) -> Option<&dyn std::any::Any> {
        self.shadow_maps.get(&light_index).map(|b| b.as_ref())
    }

    /// Enable or disable shadows
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Update shadow configuration
    pub fn set_config(&mut self, config: ShadowConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &ShadowConfig {
        &self.config
    }

    /// Get shadow statistics
    pub fn get_stats(&self) -> &ShadowStats {
        &self.stats
    }

    /// Reset frame statistics
    pub fn reset_stats(&mut self) {
        self.stats.shadow_maps_rendered = 0;
        self.stats.shadow_casters_rendered = 0;
    }

    /// Update the number of shadow casters expected in each pass.
    pub fn set_shadow_caster_count_hint(&mut self, caster_count: u32) {
        self.shadow_caster_count_hint = caster_count;
    }

    /// Read the shadow-caster count hint.
    pub fn shadow_caster_count_hint(&self) -> u32 {
        self.shadow_caster_count_hint
    }

    /// Register shadow caster submissions used for runtime pass accounting.
    pub fn set_shadow_caster_submissions(&mut self, submissions: Vec<ShadowCasterSubmission>) {
        self.shadow_caster_submissions = submissions;
    }

    /// Remove registered shadow caster submissions.
    pub fn clear_shadow_caster_submissions(&mut self) {
        self.shadow_caster_submissions.clear();
    }

    /// Number of registered shadow caster submissions.
    pub fn shadow_caster_submission_count(&self) -> usize {
        self.shadow_caster_submissions.len()
    }

    /// Register per-light caster submissions consumed by `render_shadows()`.
    pub fn set_shadow_caster_submissions_for_light(
        &mut self,
        light_index: usize,
        submissions: Vec<ShadowCasterSubmission>,
    ) {
        self.shadow_caster_submissions_by_light
            .insert(light_index, submissions);
    }

    /// Remove per-light caster submissions for one light.
    pub fn clear_shadow_caster_submissions_for_light(&mut self, light_index: usize) {
        self.shadow_caster_submissions_by_light.remove(&light_index);
    }

    /// Clear all per-light caster submissions.
    pub fn clear_shadow_caster_submissions_for_all_lights(&mut self) {
        self.shadow_caster_submissions_by_light.clear();
    }

    /// Calculate light view matrix for directional light
    pub fn calculate_directional_light_matrix(
        light_direction: Vec3,
        scene_bounds_min: Vec3,
        scene_bounds_max: Vec3,
    ) -> (Mat4, Mat4) {
        // Calculate view matrix looking down the light direction
        let center = (scene_bounds_min + scene_bounds_max) * 0.5;
        let up = if light_direction.y.abs() > 0.99 {
            Vec3::X // Use X as up if light is pointing up/down
        } else {
            Vec3::Y
        };

        let view_matrix = Mat4::look_at_rh(
            center - light_direction * 100.0, // Position light far back
            center,
            up,
        );

        // Calculate orthographic projection to fit scene bounds
        let bounds_size = scene_bounds_max - scene_bounds_min;
        let max_extent = bounds_size.x.max(bounds_size.y).max(bounds_size.z);
        let projection_matrix =
            Mat4::orthographic_rh(-max_extent, max_extent, -max_extent, max_extent, 0.1, 200.0);

        (view_matrix, projection_matrix)
    }

    /// Calculate light matrices for point light (all 6 cube faces)
    pub fn calculate_point_light_matrices(position: Vec3, _range: f32) -> [Mat4; 6] {
        [
            // Positive X
            Mat4::look_at_rh(position, position + Vec3::X, -Vec3::Y),
            // Negative X
            Mat4::look_at_rh(position, position - Vec3::X, -Vec3::Y),
            // Positive Y
            Mat4::look_at_rh(position, position + Vec3::Y, Vec3::Z),
            // Negative Y
            Mat4::look_at_rh(position, position - Vec3::Y, -Vec3::Z),
            // Positive Z
            Mat4::look_at_rh(position, position + Vec3::Z, -Vec3::Y),
            // Negative Z
            Mat4::look_at_rh(position, position - Vec3::Z, -Vec3::Y),
        ]
    }

    /// Calculate light matrices for spot light
    pub fn calculate_spot_light_matrices(
        position: Vec3,
        direction: Vec3,
        outer_angle: f32,
        range: f32,
    ) -> (Mat4, Mat4) {
        let view_matrix = Mat4::look_at_rh(position, position + direction, Vec3::Y);

        let projection_matrix = Mat4::perspective_rh(
            outer_angle * 2.0, // Full cone angle
            1.0,               // Aspect ratio (shadow maps are typically square)
            0.1,
            range,
        );

        (view_matrix, projection_matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_memory_estimation_matches_format_stride() {
        let size = 1024;
        let depth_memory = ShadowRenderer::shadow_map_memory_mb(size, TextureFormat::Depth32Float);
        let rgba16_memory = ShadowRenderer::shadow_map_memory_mb(size, TextureFormat::Rgba16Float);
        assert!(rgba16_memory > depth_memory);
        assert!(depth_memory > 0.0);
    }

    #[test]
    fn test_shadow_caster_count_hint_round_trip() {
        let mut renderer = ShadowRenderer::new();
        renderer.set_shadow_caster_count_hint(42);
        assert_eq!(renderer.shadow_caster_count_hint(), 42);
    }

    #[test]
    fn test_shadow_caster_submissions_round_trip() {
        let mut renderer = ShadowRenderer::new();
        assert_eq!(renderer.shadow_caster_submission_count(), 0);

        renderer.set_shadow_caster_submissions(vec![
            ShadowCasterSubmission::triangles(12),
            ShadowCasterSubmission::indexed_triangles(36),
        ]);
        assert_eq!(renderer.shadow_caster_submission_count(), 2);

        renderer.clear_shadow_caster_submissions();
        assert_eq!(renderer.shadow_caster_submission_count(), 0);
    }

    #[test]
    fn test_shadow_caster_accounting_prefers_registered_submissions() {
        let mut renderer = ShadowRenderer::new();
        renderer.set_shadow_caster_count_hint(99);
        renderer.set_shadow_caster_submissions(vec![
            ShadowCasterSubmission::triangles(3),
            ShadowCasterSubmission::indexed_triangles(0),
        ]);

        renderer.accumulate_shadow_caster_pass(None);
        assert_eq!(renderer.stats.shadow_casters_rendered, 1);
    }

    #[test]
    fn test_shadow_caster_accounting_falls_back_to_hint_when_no_submissions() {
        let mut renderer = ShadowRenderer::new();
        renderer.set_shadow_caster_count_hint(7);
        renderer.accumulate_shadow_caster_pass(None);
        assert_eq!(renderer.stats.shadow_casters_rendered, 7);
    }

    #[test]
    fn test_shadow_caster_accounting_prefers_per_light_override() {
        let mut renderer = ShadowRenderer::new();
        renderer.set_shadow_caster_count_hint(50);
        renderer.set_shadow_caster_submissions(vec![ShadowCasterSubmission::triangles(9)]);

        let per_light = vec![
            ShadowCasterSubmission::indexed_triangles(12),
            ShadowCasterSubmission::indexed_triangles(0),
        ];

        renderer.accumulate_shadow_caster_pass(Some(&per_light));
        assert_eq!(renderer.stats.shadow_casters_rendered, 1);
    }

    #[test]
    fn test_per_light_submission_registry_round_trip() {
        let mut renderer = ShadowRenderer::new();
        assert!(renderer.shadow_caster_submissions_by_light.is_empty());

        renderer.set_shadow_caster_submissions_for_light(
            3,
            vec![ShadowCasterSubmission::indexed_triangles(12)],
        );
        assert_eq!(renderer.shadow_caster_submissions_by_light.len(), 1);

        renderer.clear_shadow_caster_submissions_for_light(3);
        assert!(renderer.shadow_caster_submissions_by_light.is_empty());

        renderer.set_shadow_caster_submissions_for_light(
            1,
            vec![ShadowCasterSubmission::triangles(6)],
        );
        renderer.set_shadow_caster_submissions_for_light(
            2,
            vec![ShadowCasterSubmission::triangles(9)],
        );
        renderer.clear_shadow_caster_submissions_for_all_lights();
        assert!(renderer.shadow_caster_submissions_by_light.is_empty());
    }
}
