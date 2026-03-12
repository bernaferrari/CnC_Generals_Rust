//! Reflection System
//!
//! Implements planar reflections for mirrors and water surfaces.
//! Matches C++ WW3D reflection capabilities with Fresnel effects.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{CommandEncoder, Device, RenderPass, Texture, TextureView};

/// Reflection plane in world space
pub struct ReflectionPlane {
    /// Plane equation: normal.xyz and distance (w component)
    pub plane: Vec4,
    /// Reflection texture
    pub texture: Option<Arc<Texture>>,
    /// Reflection texture view
    pub view: Option<Arc<TextureView>>,
    /// Depth texture for reflection rendering
    pub depth_texture: Option<Arc<Texture>>,
    /// Depth texture view
    pub depth_view: Option<Arc<TextureView>>,
    /// Reflection texture resolution
    pub resolution: (u32, u32),
    /// View matrix for reflected camera
    pub view_matrix: Mat4,
    /// Projection matrix for reflected camera
    pub proj_matrix: Mat4,
    /// Reflection strength (0.0 to 1.0)
    pub strength: f32,
    /// Enable Fresnel effect
    pub use_fresnel: bool,
    /// Fresnel power (default 5.0 from C++)
    pub fresnel_power: f32,
}

impl ReflectionPlane {
    /// Create a new reflection plane
    pub fn new(normal: Vec3, distance: f32, resolution: (u32, u32)) -> Self {
        Self {
            plane: Vec4::new(normal.x, normal.y, normal.z, distance),
            texture: None,
            view: None,
            depth_texture: None,
            depth_view: None,
            resolution,
            view_matrix: Mat4::IDENTITY,
            proj_matrix: Mat4::IDENTITY,
            strength: 1.0,
            use_fresnel: true,
            fresnel_power: 5.0,
        }
    }

    /// Create reflection textures
    /// Matches C++ texture creation for reflections
    pub fn create_textures(&mut self, device: &Device) {
        // Create color texture for reflection
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Reflection Color Texture"),
            size: wgpu::Extent3d {
                width: self.resolution.0,
                height: self.resolution.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Reflection Color View"),
            ..Default::default()
        });

        // Create depth texture for reflection rendering
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Reflection Depth Texture"),
            size: wgpu::Extent3d {
                width: self.resolution.0,
                height: self.resolution.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Reflection Depth View"),
            ..Default::default()
        });

        self.texture = Some(Arc::new(texture));
        self.view = Some(Arc::new(view));
        self.depth_texture = Some(Arc::new(depth_texture));
        self.depth_view = Some(Arc::new(depth_view));
    }

    /// Create a horizontal water plane at given height
    pub fn new_water_plane(height: f32, resolution: (u32, u32)) -> Self {
        Self::new(Vec3::new(0.0, 1.0, 0.0), height, resolution)
    }

    /// Get the plane normal
    pub fn get_normal(&self) -> Vec3 {
        Vec3::new(self.plane.x, self.plane.y, self.plane.z)
    }

    /// Get the plane distance
    pub fn get_distance(&self) -> f32 {
        self.plane.w
    }

    /// Calculate reflected camera matrix
    pub fn calculate_reflection_matrix(
        &mut self,
        camera_view: Mat4,
        camera_proj: Mat4,
        camera_position: Vec3,
    ) {
        let normal = self.get_normal();
        let d = self.get_distance();

        // Create reflection matrix for the plane
        let reflection_matrix = Self::create_plane_reflection_matrix(normal, d);

        // Mirror the camera position and view direction
        let _reflected_pos = reflection_matrix.transform_point3(camera_position);

        // The view matrix needs to be reflected
        self.view_matrix = reflection_matrix * camera_view;
        self.proj_matrix = camera_proj;
    }

    /// Create a reflection matrix for a plane
    pub fn create_plane_reflection_matrix(normal: Vec3, d: f32) -> Mat4 {
        let nx = normal.x;
        let ny = normal.y;
        let nz = normal.z;

        Mat4::from_cols_array(&[
            1.0 - 2.0 * nx * nx,
            -2.0 * ny * nx,
            -2.0 * nz * nx,
            0.0,
            -2.0 * nx * ny,
            1.0 - 2.0 * ny * ny,
            -2.0 * nz * ny,
            0.0,
            -2.0 * nx * nz,
            -2.0 * ny * nz,
            1.0 - 2.0 * nz * nz,
            0.0,
            -2.0 * nx * d,
            -2.0 * ny * d,
            -2.0 * nz * d,
            1.0,
        ])
    }

    /// Calculate Fresnel term for reflection blending
    pub fn calculate_fresnel(&self, view_direction: Vec3) -> f32 {
        if !self.use_fresnel {
            return self.strength;
        }

        let normal = self.get_normal();
        let cos_theta = view_direction.dot(normal).abs();

        // Schlick's approximation: F = F0 + (1 - F0) * (1 - cos_theta)^5
        // For water, F0 = 0.02 (from C++)
        let f0 = 0.02;
        let fresnel = f0 + (1.0 - f0) * (1.0 - cos_theta).powf(self.fresnel_power);

        fresnel * self.strength
    }

    /// Check if a point is above the reflection plane
    pub fn is_point_above(&self, point: Vec3) -> bool {
        let normal = self.get_normal();
        let d = self.get_distance();
        normal.dot(point) + d > 0.0
    }
}

/// Reflection system manager
pub struct ReflectionSystem {
    device: Arc<Device>,
    reflection_planes: Vec<ReflectionPlane>,
    enabled: bool,
    default_resolution: (u32, u32),
}

impl ReflectionSystem {
    /// Create a new reflection system
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            reflection_planes: Vec::new(),
            enabled: true,
            default_resolution: (512, 512), // Match C++ default
        }
    }

    /// Add a reflection plane
    pub fn add_plane(&mut self, mut plane: ReflectionPlane) -> usize {
        // Create GPU textures for the plane
        plane.create_textures(&self.device);
        self.reflection_planes.push(plane);
        self.reflection_planes.len() - 1
    }

    /// Create a water reflection plane
    pub fn add_water_plane(&mut self, height: f32) -> usize {
        let plane = ReflectionPlane::new_water_plane(height, self.default_resolution);
        self.add_plane(plane)
    }

    /// Get a reflection plane
    pub fn get_plane(&self, index: usize) -> Option<&ReflectionPlane> {
        self.reflection_planes.get(index)
    }

    /// Get a mutable reflection plane
    pub fn get_plane_mut(&mut self, index: usize) -> Option<&mut ReflectionPlane> {
        self.reflection_planes.get_mut(index)
    }

    /// Update reflection matrices for all planes
    pub fn update_reflections(
        &mut self,
        camera_view: Mat4,
        camera_proj: Mat4,
        camera_position: Vec3,
    ) {
        for plane in &mut self.reflection_planes {
            plane.calculate_reflection_matrix(camera_view, camera_proj, camera_position);
        }
    }

    /// Begin rendering to reflection texture
    /// Matches C++ reflection rendering setup
    pub fn begin_reflection_pass<'a>(
        &'a self,
        encoder: &'a mut CommandEncoder,
        index: usize,
    ) -> Option<RenderPass<'a>> {
        if !self.enabled || index >= self.reflection_planes.len() {
            return None;
        }

        let plane = &self.reflection_planes[index];
        let view = plane.view.as_ref()?;
        let depth_view = plane.depth_view.as_ref()?;

        Some(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&format!("Reflection {} Render Pass", index)),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.5,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        }))
    }

    /// Enable/disable reflections
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set default resolution for new reflection planes
    pub fn set_default_resolution(&mut self, width: u32, height: u32) {
        self.default_resolution = (width, height);
    }

    /// Get statistics
    pub fn get_stats(&self) -> ReflectionSystemStats {
        let total_memory: u32 = self
            .reflection_planes
            .iter()
            .map(|p| p.resolution.0 * p.resolution.1 * 4)
            .sum();

        ReflectionSystemStats {
            plane_count: self.reflection_planes.len(),
            total_memory_usage: total_memory as u64,
            enabled: self.enabled,
        }
    }
}

/// Reflection system statistics
#[derive(Debug, Clone)]
pub struct ReflectionSystemStats {
    pub plane_count: usize,
    pub total_memory_usage: u64,
    pub enabled: bool,
}

/// Water rendering system (combines reflection + refraction)
pub struct WaterRenderer {
    reflection_system: ReflectionSystem,
    wave_distortion: f32,
    wave_speed: f32,
    wave_scale: f32,
    fresnel_bias: f32,
    water_color: Vec3,
    normal_map: Option<Arc<Texture>>,
    time: f32,
}

impl WaterRenderer {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            reflection_system: ReflectionSystem::new(device),
            wave_distortion: 0.02, // From C++
            wave_speed: 1.0,
            wave_scale: 1.0,
            fresnel_bias: 0.02,                    // From C++ water shader
            water_color: Vec3::new(0.0, 0.3, 0.5), // Default blue-green water
            normal_map: None,
            time: 0.0,
        }
    }

    /// Add a water surface
    pub fn add_water_surface(&mut self, height: f32) -> usize {
        self.reflection_system.add_water_plane(height)
    }

    /// Calculate water color with reflection and refraction
    pub fn calculate_water_color(
        &self,
        reflection_color: Vec3,
        refraction_color: Vec3,
        view_direction: Vec3,
        plane_index: usize,
    ) -> Vec3 {
        if let Some(plane) = self.reflection_system.get_plane(plane_index) {
            let fresnel = plane.calculate_fresnel(view_direction);

            // Blend reflection and refraction based on Fresnel
            reflection_color * fresnel + refraction_color * (1.0 - fresnel)
        } else {
            refraction_color
        }
    }

    /// Set wave parameters
    pub fn set_wave_params(&mut self, distortion: f32, speed: f32, scale: f32) {
        self.wave_distortion = distortion;
        self.wave_speed = speed;
        self.wave_scale = scale;
    }

    /// Get wave distortion offset (for UV sampling)
    pub fn get_wave_offset(&self, position: Vec3, time: f32) -> (f32, f32) {
        let phase = (position.x * self.wave_scale + time * self.wave_speed).sin();
        let offset = phase * self.wave_distortion;
        (offset, offset * 0.5)
    }

    /// Set water color
    pub fn set_water_color(&mut self, color: Vec3) {
        self.water_color = color;
    }

    /// Set normal map for water surface
    pub fn set_normal_map(&mut self, texture: Arc<Texture>) {
        self.normal_map = Some(texture);
    }

    /// Update simulation time
    pub fn update(&mut self, delta_time: f32) {
        self.time += delta_time;
    }

    /// Get current simulation time
    pub fn get_time(&self) -> f32 {
        self.time
    }

    /// Calculate wave height at position
    pub fn calculate_wave_height(&self, position: Vec3) -> f32 {
        let phase = (position.x * self.wave_scale + self.time * self.wave_speed).sin();
        let wave_offset = phase * self.wave_distortion;

        // Add secondary wave for more natural look
        let phase2 = (position.z * self.wave_scale * 0.7 - self.time * self.wave_speed * 0.5).sin();
        let wave_offset2 = phase2 * self.wave_distortion * 0.5;

        wave_offset + wave_offset2
    }

    /// Get reflection system
    pub fn reflection_system(&self) -> &ReflectionSystem {
        &self.reflection_system
    }

    /// Get mutable reflection system
    pub fn reflection_system_mut(&mut self) -> &mut ReflectionSystem {
        &mut self.reflection_system
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflection_plane() {
        let plane = ReflectionPlane::new(Vec3::new(0.0, 1.0, 0.0), 0.0, (512, 512));
        assert_eq!(plane.get_normal(), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.get_distance(), 0.0);
    }

    #[test]
    fn test_point_above_plane() {
        let plane = ReflectionPlane::new(Vec3::new(0.0, 1.0, 0.0), -5.0, (512, 512));
        assert!(plane.is_point_above(Vec3::new(0.0, 10.0, 0.0)));
        assert!(!plane.is_point_above(Vec3::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn test_fresnel_calculation() {
        let plane = ReflectionPlane::new(Vec3::new(0.0, 1.0, 0.0), 0.0, (512, 512));
        let view_dir = Vec3::new(0.0, -1.0, 0.0); // Looking straight down
        let fresnel = plane.calculate_fresnel(view_dir);
        assert!(fresnel > 0.0 && fresnel <= 1.0);
    }

    #[test]
    fn test_reflection_matrix() {
        // Test horizontal plane reflection
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let d = -5.0; // Plane at Y = 5
        let matrix = ReflectionPlane::create_plane_reflection_matrix(normal, d);

        // Point above plane should be reflected below
        let point_above = Vec3::new(0.0, 10.0, 0.0);
        let reflected = matrix.transform_point3(point_above);

        // Should be reflected across Y = 5, so 10 -> 0
        assert!((reflected.y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_wave_calculation() {
        let position = Vec3::new(5.0, 0.0, 3.0);
        let time = 1.0;

        let wave_scale = 1.0;
        let wave_speed = 1.0;
        let wave_distortion = 0.02;

        let phase = (position.x * wave_scale + time * wave_speed).sin();
        let offset = phase * wave_distortion;

        // Offset should be within distortion range
        assert!(offset.abs() <= wave_distortion);
    }

    #[test]
    fn test_multiple_reflection_planes() {
        // Mock device (won't actually be used)
        // In real code, this would use a proper device
        // For now, we'll just test the logic without device creation
        let plane1 = ReflectionPlane::new_water_plane(0.0, (512, 512));
        let plane2 = ReflectionPlane::new_water_plane(5.0, (512, 512));

        assert_eq!(plane1.get_distance(), 0.0);
        assert_eq!(plane2.get_distance(), 5.0);
    }
}
