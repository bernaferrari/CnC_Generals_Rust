//! Point Shadow Maps
//!
//! This module provides point light shadow mapping using cube maps.
//! Each point light renders to 6 faces of a cube map for full 360-degree shadows.

use glam::{Mat4, Vec3};
use std::convert::TryInto;
use std::sync::Arc;
use wgpu::{
    CommandEncoder, Device, RenderPass, Texture, TextureView, TextureViewDescriptor,
    TextureViewDimension,
};

/// Point shadow map for omni-directional shadows
#[derive(Debug)]
pub struct PointShadowMap {
    /// Size of the shadow map (cube map face size)
    pub size: u32,
    /// Light position
    pub light_position: Vec3,
    /// Light range
    pub light_range: f32,
    /// Cube map texture storing the six shadow faces
    pub cube_texture: Option<Arc<Texture>>,
    /// View for each cube face
    pub cube_face_views: Option<[TextureView; 6]>,
    /// View matrices for each cube face
    pub face_view_matrices: [Mat4; 6],
    /// Projection matrix for the cube map
    pub projection_matrix: Mat4,
}

/// Cube map face directions
#[derive(Debug, Clone, Copy)]
pub enum CubeFace {
    PositiveX = 0,
    NegativeX = 1,
    PositiveY = 2,
    NegativeY = 3,
    PositiveZ = 4,
    NegativeZ = 5,
}

impl CubeFace {
    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::PositiveX),
            1 => Some(Self::NegativeX),
            2 => Some(Self::PositiveY),
            3 => Some(Self::NegativeY),
            4 => Some(Self::PositiveZ),
            5 => Some(Self::NegativeZ),
            _ => None,
        }
    }
}

impl PointShadowMap {
    /// Create a new point shadow map
    pub fn new(size: u32, light_range: f32) -> Self {
        let projection_matrix = Mat4::perspective_rh(
            90.0_f32.to_radians(), // 90 degree FOV for cube map faces
            1.0,                   // Aspect ratio is 1:1 for cube map faces
            0.1,                   // Near plane
            light_range,           // Far plane
        );

        let mut shadow_map = Self {
            size,
            light_position: Vec3::ZERO,
            light_range,
            cube_texture: None,
            cube_face_views: None,
            face_view_matrices: [Mat4::IDENTITY; 6],
            projection_matrix,
        };
        shadow_map.update_view_matrices();
        shadow_map
    }

    /// Create a point shadow map with custom projection
    pub fn with_projection(size: u32, light_range: f32, near_plane: f32) -> Self {
        let projection_matrix =
            Mat4::perspective_rh(90.0_f32.to_radians(), 1.0, near_plane, light_range);

        let mut shadow_map = Self {
            size,
            light_position: Vec3::ZERO,
            light_range,
            cube_texture: None,
            cube_face_views: None,
            face_view_matrices: [Mat4::IDENTITY; 6],
            projection_matrix,
        };
        shadow_map.update_view_matrices();
        shadow_map
    }

    /// Update the shadow map with new light position
    pub fn update(&mut self, light_position: Vec3) {
        self.light_position = light_position;
        self.update_view_matrices();
    }

    /// Update view matrices for all cube faces
    fn update_view_matrices(&mut self) {
        let pos = self.light_position;

        // Calculate view matrices for each cube face
        self.face_view_matrices = [
            // Positive X (right)
            Mat4::look_at_rh(pos, pos + Vec3::X, -Vec3::Y),
            // Negative X (left)
            Mat4::look_at_rh(pos, pos - Vec3::X, -Vec3::Y),
            // Positive Y (top)
            Mat4::look_at_rh(pos, pos + Vec3::Y, Vec3::Z),
            // Negative Y (bottom)
            Mat4::look_at_rh(pos, pos - Vec3::Y, -Vec3::Z),
            // Positive Z (forward)
            Mat4::look_at_rh(pos, pos + Vec3::Z, -Vec3::Y),
            // Negative Z (backward)
            Mat4::look_at_rh(pos, pos - Vec3::Z, -Vec3::Y),
        ];
    }

    /// Initialize the cube map textures
    pub fn initialize_cube_map(&mut self, device: &Device) {
        let texture = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Point Light Shadow Cube"),
            size: wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }));

        let mut views: Vec<TextureView> = Vec::with_capacity(6);
        for layer in 0..6 {
            views.push(texture.create_view(&TextureViewDescriptor {
                label: Some("Point Shadow Face"),
                format: Some(wgpu::TextureFormat::Depth32Float),
                dimension: Some(TextureViewDimension::D2),
                usage: None,
                aspect: wgpu::TextureAspect::DepthOnly,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
            }));
        }

        self.cube_texture = Some(texture);
        self.cube_face_views = Some(views.try_into().expect("cube face view count"));
    }

    /// Render shadow map for a specific cube face
    pub fn render_face(&self, face: CubeFace, render_pass: &mut RenderPass) {
        let face_index = face as usize;

        // Set the view-projection matrix for this face
        let view_matrix = self.face_view_matrices[face_index];
        let view_proj_matrix = self.projection_matrix * view_matrix;

        self.render_objects_to_face(face, view_proj_matrix, render_pass);
    }

    /// Render all cube faces
    pub fn render_all_faces(&self, encoder: &mut CommandEncoder) {
        let Some(views) = self.cube_face_views.as_ref() else {
            return;
        };

        for (face_index, face_view) in views.iter().enumerate() {
            let face = CubeFace::from_index(face_index).expect("face index in range");
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Point Shadow Face Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: face_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.render_face(face, &mut render_pass);
        }
    }

    /// Render objects to a specific cube face
    fn render_objects_to_face(
        &self,
        _face: CubeFace,
        _view_proj_matrix: Mat4,
        render_pass: &mut RenderPass,
    ) {
        render_pass.set_viewport(0.0, 0.0, self.size as f32, self.size as f32, 0.0, 1.0);
        render_pass.set_scissor_rect(0, 0, self.size, self.size);
    }

    /// Get the view matrix for a specific cube face
    pub fn get_face_view_matrix(&self, face: CubeFace) -> Mat4 {
        self.face_view_matrices[face as usize]
    }

    /// Get the projection matrix
    pub fn get_projection_matrix(&self) -> Mat4 {
        self.projection_matrix
    }

    /// Get the view-projection matrix for a specific face
    pub fn get_face_view_proj_matrix(&self, face: CubeFace) -> Mat4 {
        self.projection_matrix * self.get_face_view_matrix(face)
    }

    /// Get light position
    pub fn get_light_position(&self) -> Vec3 {
        self.light_position
    }

    /// Get light range
    pub fn get_light_range(&self) -> f32 {
        self.light_range
    }

    /// Set light range and update projection matrix
    pub fn set_light_range(&mut self, range: f32) {
        self.light_range = range;
        self.projection_matrix = Mat4::perspective_rh(90.0_f32.to_radians(), 1.0, 0.1, range);
    }

    /// Check if a point is within the light's range
    pub fn is_point_in_range(&self, point: Vec3) -> bool {
        let distance = (point - self.light_position).length();
        distance <= self.light_range
    }

    /// Calculate the shadow factor for a world position
    pub fn calculate_shadow_factor(&self, world_pos: Vec3, shadow_bias: f32) -> f32 {
        if self.light_range <= 0.0 {
            return 0.0;
        }

        let distance_to_light = (world_pos - self.light_position).length();
        if distance_to_light >= self.light_range {
            return 0.0;
        }

        // Approximate shadow influence as distance attenuation with configurable bias.
        let normalized = 1.0 - (distance_to_light / self.light_range);
        let biased = (normalized + (shadow_bias / self.light_range)).clamp(0.0, 1.0);
        if biased <= 0.0 {
            0.0
        } else {
            biased * biased
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_cube_face_view_matrices() {
        let shadow = PointShadowMap::new(256, 50.0);
        let vp = shadow.get_face_view_proj_matrix(CubeFace::PositiveX);
        assert!(vp.is_finite());
        assert_ne!(vp, Mat4::IDENTITY);
    }

    #[test]
    fn shadow_factor_is_zero_outside_light_range() {
        let mut shadow = PointShadowMap::new(128, 10.0);
        shadow.update(Vec3::ZERO);
        let factor = shadow.calculate_shadow_factor(Vec3::new(20.0, 0.0, 0.0), 0.0);
        assert_eq!(factor, 0.0);
    }

    #[test]
    fn positive_bias_increases_shadow_factor_for_same_point() {
        let mut shadow = PointShadowMap::new(128, 10.0);
        shadow.update(Vec3::ZERO);
        let base = shadow.calculate_shadow_factor(Vec3::new(5.0, 0.0, 0.0), 0.0);
        let biased = shadow.calculate_shadow_factor(Vec3::new(5.0, 0.0, 0.0), 1.0);
        assert!(biased >= base);
    }
}
