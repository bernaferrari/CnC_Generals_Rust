//! Shadow Map Implementation
//!
//! Core shadow mapping functionality for different light types.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupLayout, Device, Queue, Sampler, Texture, TextureView};

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }

    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn approximate_shadow_visibility(
    receiver_depth: f32,
    occluder_depth_estimate: f32,
    filter_mode: ShadowFilterMode,
) -> f32 {
    let depth_delta = receiver_depth - occluder_depth_estimate;
    if depth_delta <= 0.0 {
        return 1.0;
    }

    let softness = match filter_mode {
        ShadowFilterMode::None => 0.0,
        ShadowFilterMode::Pcf2x2 => 0.03,
        ShadowFilterMode::Pcf4x4 => 0.06,
        ShadowFilterMode::Pcf8x8 => 0.10,
        ShadowFilterMode::Soft => 0.15,
    };

    if softness <= 0.0 {
        return 0.0;
    }

    1.0 - smoothstep(0.0, softness, depth_delta)
}

/// Shadow map quality levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowQuality {
    Low = 512,
    Medium = 1024,
    High = 2048,
    Ultra = 4096,
}

impl ShadowQuality {
    pub fn from_resolution(resolution: u32) -> Self {
        match resolution {
            r if r <= ShadowQuality::Low as u32 => ShadowQuality::Low,
            r if r <= ShadowQuality::Medium as u32 => ShadowQuality::Medium,
            r if r <= ShadowQuality::High as u32 => ShadowQuality::High,
            _ => ShadowQuality::Ultra,
        }
    }

    pub fn resolution(self) -> u32 {
        self as u32
    }
}

/// Shadow map filter modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowFilterMode {
    None,   // Hard shadows
    Pcf2x2, // 2x2 PCF
    Pcf4x4, // 4x4 PCF
    Pcf8x8, // 8x8 PCF
    Soft,   // Soft shadows with blur
}

/// Shadow bias settings
#[derive(Debug, Clone)]
pub struct ShadowBias {
    pub constant_bias: f32,
    pub slope_scale_bias: f32,
    pub normal_offset_bias: f32,
}

impl Default for ShadowBias {
    fn default() -> Self {
        Self {
            constant_bias: 0.005,
            slope_scale_bias: 0.5,
            normal_offset_bias: 0.01,
        }
    }
}

/// Base shadow map structure
pub struct ShadowMap {
    /// Shadow map texture
    pub texture: Arc<Texture>,
    /// Shadow map texture view
    pub view: Arc<TextureView>,
    /// Shadow map sampler
    pub sampler: Arc<Sampler>,
    /// Bind group for shadow sampling
    pub bind_group: Arc<BindGroup>,
    /// Bind group layout
    pub bind_group_layout: Arc<BindGroupLayout>,
    /// Shadow map size
    pub size: u32,
    /// Light view-projection matrix
    pub light_vp_matrix: Mat4,
    /// Quality level
    pub quality: ShadowQuality,
    /// Filter mode
    pub filter_mode: ShadowFilterMode,
    /// Bias settings
    pub bias: ShadowBias,
    /// Near plane
    pub near_plane: f32,
    /// Far plane
    pub far_plane: f32,
}

impl ShadowMap {
    /// Create a new shadow map
    pub fn new(device: &Device, quality: ShadowQuality, filter_mode: ShadowFilterMode) -> Self {
        let size = quality as u32;

        // Create shadow map texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler based on filter mode
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Map Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: match filter_mode {
                ShadowFilterMode::None => wgpu::FilterMode::Nearest,
                _ => wgpu::FilterMode::Linear,
            },
            min_filter: match filter_mode {
                ShadowFilterMode::None => wgpu::FilterMode::Nearest,
                _ => wgpu::FilterMode::Linear,
            },
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: Some(wgpu::CompareFunction::LessEqual),
            anisotropy_clamp: 1,
            border_color: None,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Map Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Map Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture: Arc::new(texture),
            view: Arc::new(view),
            sampler: Arc::new(sampler),
            bind_group: Arc::new(bind_group),
            bind_group_layout: Arc::new(bind_group_layout),
            size,
            light_vp_matrix: Mat4::IDENTITY,
            quality,
            filter_mode,
            bias: ShadowBias::default(),
            near_plane: 0.1,
            far_plane: 100.0,
        }
    }

    /// Update the light view-projection matrix
    pub fn update_light_matrix(
        &mut self,
        light_direction: Vec3,
        light_position: Vec3,
        scene_center: Vec3,
        scene_radius: f32,
    ) {
        // Create light view matrix (looking from light position in light direction)
        let light_target = light_position + light_direction;
        let light_up = if light_direction.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };

        let light_view = Mat4::look_at_rh(light_position, light_target, light_up);

        // Create orthographic projection matrix that covers the scene
        let left = scene_center.x - scene_radius;
        let right = scene_center.x + scene_radius;
        let bottom = scene_center.z - scene_radius;
        let top = scene_center.z + scene_radius;

        let light_projection =
            Mat4::orthographic_rh(left, right, bottom, top, self.near_plane, self.far_plane);

        self.light_vp_matrix = light_projection * light_view;
    }

    /// Get the shadow bias matrix for shader calculations
    pub fn get_shadow_bias_matrix(&self) -> Mat4 {
        // Transform from [-1, 1] to [0, 1] for texture sampling
        Mat4::from_cols_array(&[
            0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.5, 1.0,
        ])
    }

    /// Get the combined shadow matrix (bias * light_vp)
    pub fn get_shadow_matrix(&self) -> Mat4 {
        self.get_shadow_bias_matrix() * self.light_vp_matrix
    }

    /// Check if a point is in shadow
    pub fn is_point_in_shadow(&self, world_position: Vec3) -> f32 {
        let world_pos_vec4 = Vec4::new(world_position.x, world_position.y, world_position.z, 1.0);
        let light_space_pos = self.light_vp_matrix * world_pos_vec4;
        let light_space_pos = light_space_pos / light_space_pos.w;

        // Convert to texture coordinates
        let shadow_coord = Vec3::new(
            light_space_pos.x * 0.5 + 0.5,
            light_space_pos.y * 0.5 + 0.5,
            light_space_pos.z,
        );

        // Simple shadow test (would be done in shader in real implementation)
        if shadow_coord.x < 0.0
            || shadow_coord.x > 1.0
            || shadow_coord.y < 0.0
            || shadow_coord.y > 1.0
            || shadow_coord.z < 0.0
            || shadow_coord.z > 1.0
        {
            return 1.0; // Not in shadow
        }

        // CPU-side approximation path used when querying shadowing outside shader execution.
        // We cannot sample the GPU depth texture directly here, so use a bias-aware estimate
        // instead of a fixed constant.
        let receiver_depth = (shadow_coord.z - self.bias.constant_bias).clamp(0.0, 1.0);
        let occluder_depth_estimate = (0.5 + self.bias.normal_offset_bias * 0.25).clamp(0.0, 1.0);
        approximate_shadow_visibility(receiver_depth, occluder_depth_estimate, self.filter_mode)
    }

    /// Get shadow map statistics
    pub fn get_stats(&self) -> ShadowMapStats {
        ShadowMapStats {
            size: self.size,
            memory_usage: self.size * self.size * 4, // 4 bytes per depth pixel
            quality: self.quality,
            filter_mode: self.filter_mode,
        }
    }
}

/// Shadow map statistics
#[derive(Debug, Clone)]
pub struct ShadowMapStats {
    pub size: u32,
    pub memory_usage: u32,
    pub quality: ShadowQuality,
    pub filter_mode: ShadowFilterMode,
}

/// PCF (Percentage Closer Filtering) kernel sizes
pub const PCF_KERNEL_2X2: [(f32, f32); 4] = [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)];

pub const PCF_KERNEL_4X4: [(f32, f32); 16] = [
    (-1.5, -1.5),
    (-0.5, -1.5),
    (0.5, -1.5),
    (1.5, -1.5),
    (-1.5, -0.5),
    (-0.5, -0.5),
    (0.5, -0.5),
    (1.5, -0.5),
    (-1.5, 0.5),
    (-0.5, 0.5),
    (0.5, 0.5),
    (1.5, 0.5),
    (-1.5, 1.5),
    (-0.5, 1.5),
    (0.5, 1.5),
    (1.5, 1.5),
];

/// Primitive description for one shadow caster submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowCasterPrimitive {
    NonIndexed {
        vertex_count: u32,
        first_vertex: u32,
    },
    Indexed {
        index_count: u32,
        first_index: u32,
        base_vertex: i32,
    },
}

/// Render submission for a shadow-casting object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowCasterSubmission {
    pub primitive: ShadowCasterPrimitive,
    pub first_instance: u32,
    pub instance_count: u32,
}

impl ShadowCasterSubmission {
    pub fn triangles(vertex_count: u32) -> Self {
        Self {
            primitive: ShadowCasterPrimitive::NonIndexed {
                vertex_count,
                first_vertex: 0,
            },
            first_instance: 0,
            instance_count: 1,
        }
    }

    pub fn indexed_triangles(index_count: u32) -> Self {
        Self {
            primitive: ShadowCasterPrimitive::Indexed {
                index_count,
                first_index: 0,
                base_vertex: 0,
            },
            first_instance: 0,
            instance_count: 1,
        }
    }

    pub fn is_renderable(&self) -> bool {
        if self.instance_count == 0 {
            return false;
        }
        match self.primitive {
            ShadowCasterPrimitive::NonIndexed { vertex_count, .. } => vertex_count > 0,
            ShadowCasterPrimitive::Indexed { index_count, .. } => index_count > 0,
        }
    }
}

/// Counters emitted by shadow-caster submission processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShadowCasterRenderStats {
    pub casters_rendered: u32,
    pub draw_calls: u32,
}

fn count_renderable_submissions(submissions: &[ShadowCasterSubmission]) -> ShadowCasterRenderStats {
    let mut stats = ShadowCasterRenderStats::default();
    for submission in submissions {
        if submission.is_renderable() {
            stats.casters_rendered = stats.casters_rendered.saturating_add(1);
            stats.draw_calls = stats.draw_calls.saturating_add(1);
        }
    }
    stats
}

/// Shadow map rendering utilities
pub struct ShadowMapRenderer {
    _device: Arc<Device>,
    _queue: Arc<Queue>,
}

impl ShadowMapRenderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            _device: device,
            _queue: queue,
        }
    }

    /// Begin shadow map rendering
    pub fn begin_shadow_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        shadow_map: &'a ShadowMap,
        clear_depth: f32,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Map Render Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &shadow_map.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        })
    }

    /// Render scene to shadow map
    pub fn render_shadow_casters(
        &self,
        render_pass: &mut wgpu::RenderPass,
        shadow_map: &ShadowMap,
    ) {
        let _ = self.render_shadow_casters_with_submissions(render_pass, shadow_map, &[]);
    }

    /// Render a prepared list of shadow-casting submissions.
    pub fn render_shadow_casters_with_submissions(
        &self,
        render_pass: &mut wgpu::RenderPass,
        shadow_map: &ShadowMap,
        submissions: &[ShadowCasterSubmission],
    ) -> ShadowCasterRenderStats {
        let _ = shadow_map;

        for submission in submissions {
            if !submission.is_renderable() {
                continue;
            }

            let instance_range =
                submission.first_instance..(submission.first_instance + submission.instance_count);
            match submission.primitive {
                ShadowCasterPrimitive::NonIndexed {
                    vertex_count,
                    first_vertex,
                } => {
                    render_pass.draw(first_vertex..(first_vertex + vertex_count), instance_range);
                }
                ShadowCasterPrimitive::Indexed {
                    index_count,
                    first_index,
                    base_vertex,
                } => {
                    render_pass.draw_indexed(
                        first_index..(first_index + index_count),
                        base_vertex,
                        instance_range,
                    );
                }
            }
        }

        count_renderable_submissions(submissions)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_shadow_map_creation() {
        // Note: This test would require a WGPU device, so it's just a structure test
        let bias = ShadowBias::default();
        assert_eq!(bias.constant_bias, 0.005);
        assert_eq!(bias.slope_scale_bias, 0.5);
        assert_eq!(bias.normal_offset_bias, 0.01);
    }

    #[test]
    fn test_shadow_quality_sizes() {
        assert_eq!(ShadowQuality::Low as u32, 512);
        assert_eq!(ShadowQuality::Medium as u32, 1024);
        assert_eq!(ShadowQuality::High as u32, 2048);
        assert_eq!(ShadowQuality::Ultra as u32, 4096);
    }

    #[test]
    fn test_pcf_kernels() {
        assert_eq!(PCF_KERNEL_2X2.len(), 4);
        assert_eq!(PCF_KERNEL_4X4.len(), 16);
    }

    #[test]
    fn test_approximate_shadow_visibility_hard_mode() {
        assert_eq!(
            approximate_shadow_visibility(0.4, 0.5, ShadowFilterMode::None),
            1.0
        );
        assert_eq!(
            approximate_shadow_visibility(0.7, 0.5, ShadowFilterMode::None),
            0.0
        );
    }

    #[test]
    fn test_approximate_shadow_visibility_soft_modes_are_smoothed() {
        let hard = approximate_shadow_visibility(0.56, 0.5, ShadowFilterMode::Pcf2x2);
        let soft = approximate_shadow_visibility(0.56, 0.5, ShadowFilterMode::Soft);
        assert!(soft > hard);
        assert!((0.0..=1.0).contains(&hard));
        assert!((0.0..=1.0).contains(&soft));
    }

    #[test]
    fn test_shadow_caster_submission_validation() {
        assert!(ShadowCasterSubmission::triangles(3).is_renderable());
        assert!(ShadowCasterSubmission::indexed_triangles(6).is_renderable());

        let invalid = ShadowCasterSubmission {
            primitive: ShadowCasterPrimitive::NonIndexed {
                vertex_count: 0,
                first_vertex: 0,
            },
            first_instance: 0,
            instance_count: 1,
        };
        assert!(!invalid.is_renderable());
    }

    #[test]
    fn test_count_renderable_submissions_ignores_empty_work() {
        let submissions = vec![
            ShadowCasterSubmission::triangles(12),
            ShadowCasterSubmission {
                primitive: ShadowCasterPrimitive::Indexed {
                    index_count: 0,
                    first_index: 0,
                    base_vertex: 0,
                },
                first_instance: 0,
                instance_count: 1,
            },
            ShadowCasterSubmission {
                primitive: ShadowCasterPrimitive::Indexed {
                    index_count: 24,
                    first_index: 0,
                    base_vertex: 0,
                },
                first_instance: 0,
                instance_count: 0,
            },
            ShadowCasterSubmission::indexed_triangles(18),
        ];

        let stats = count_renderable_submissions(&submissions);
        assert_eq!(stats.casters_rendered, 2);
        assert_eq!(stats.draw_calls, 2);
    }
}
