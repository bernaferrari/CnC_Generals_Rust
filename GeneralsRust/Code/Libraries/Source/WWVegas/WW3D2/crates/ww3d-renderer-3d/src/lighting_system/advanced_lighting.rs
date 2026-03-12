//! Advanced Lighting System
//!
//! This module provides a comprehensive lighting system with:
//! - Multiple light types (point, directional, spot, area)
//! - Shadow mapping with cascaded shadow maps
//! - Screen space ambient occlusion (SSAO)
//! - Global illumination approximations
//! - Volumetric lighting effects
//! - Light probes for environment lighting

use std::collections::HashMap;
use std::sync::Arc;
use glam::{Vec3, Mat4 as Matrix4, Vec3 as Vec3Dup, Mat4};
use wgpu::util::DeviceExt;

/// Advanced lighting manager
pub struct AdvancedLightingManager {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    // Light sources
    directional_lights: Vec<DirectionalLight>,
    point_lights: Vec<PointLight>,
    spot_lights: Vec<SpotLight>,
    area_lights: Vec<AreaLight>,

    // Shadow mapping
    shadow_maps: HashMap<String, ShadowMap>,
    cascaded_shadow_maps: Vec<CascadedShadowMap>,

    // SSAO
    ssao_pipeline: SSAOPipeline,

    // Light probes
    light_probes: Vec<LightProbe>,

    // Volumetric lighting
    volumetric_pipeline: Option<VolumetricLightingPipeline>,

    // Lighting statistics
    stats: LightingStats,
}

impl AdvancedLightingManager {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            directional_lights: Vec::new(),
            point_lights: Vec::new(),
            spot_lights: Vec::new(),
            area_lights: Vec::new(),
            shadow_maps: HashMap::new(),
            cascaded_shadow_maps: Vec::new(),
            ssao_pipeline: SSAOPipeline::new(device.as_ref()),
            light_probes: Vec::new(),
            volumetric_pipeline: None,
            stats: LightingStats::default(),
        }
    }

    /// Add a directional light
    pub fn add_directional_light(&mut self, light: DirectionalLight) {
        self.directional_lights.push(light);
        self.stats.total_lights += 1;
        self.stats.directional_lights += 1;
    }

    /// Add a point light
    pub fn add_point_light(&mut self, light: PointLight) {
        self.point_lights.push(light);
        self.stats.total_lights += 1;
        self.stats.point_lights += 1;
    }

    /// Add a spot light
    pub fn add_spot_light(&mut self, light: SpotLight) {
        self.spot_lights.push(light);
        self.stats.total_lights += 1;
        self.stats.spot_lights += 1;
    }

    /// Add an area light
    pub fn add_area_light(&mut self, light: AreaLight) {
        self.area_lights.push(light);
        self.stats.total_lights += 1;
        self.stats.area_lights += 1;
    }

    /// Create a shadow map for a light
    pub fn create_shadow_map(&mut self, name: String, size: u32) -> Result<(), LightingError> {
        let shadow_map = ShadowMap::new(self.device.as_ref(), size)?;
        self.shadow_maps.insert(name, shadow_map);
        self.stats.shadow_maps += 1;
        Ok(())
    }

    /// Create cascaded shadow maps for directional light
    pub fn create_cascaded_shadow_maps(&mut self, cascades: u32, size: u32) -> Result<(), LightingError> {
        let cascaded = CascadedShadowMap::new(self.device.as_ref(), cascades, size)?;
        self.cascaded_shadow_maps.push(cascaded);
        self.stats.cascaded_shadow_maps += 1;
        Ok(())
    }

    /// Add a light probe
    pub fn add_light_probe(&mut self, probe: LightProbe) {
        self.light_probes.push(probe);
        self.stats.light_probes += 1;
    }

    /// Update all lighting data
    pub fn update(&mut self, camera: &CameraData) {
        // Update shadow maps
        for shadow_map in self.shadow_maps.values_mut() {
            shadow_map.update(self.queue.as_ref());
        }

        for cascaded in &mut self.cascaded_shadow_maps {
            cascaded.update(camera, self.queue.as_ref());
        }

        // Update SSAO
        self.ssao_pipeline.update(self.queue.as_ref());

        // Update volumetric lighting
        if let Some(ref mut volumetric) = self.volumetric_pipeline {
            volumetric.update(self.queue.as_ref());
        }
    }

    /// Render shadow maps
    pub fn render_shadows(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        for shadow_map in self.shadow_maps.values() {
            shadow_map.render(encoder, view);
        }

        for cascaded in &self.cascaded_shadow_maps {
            cascaded.render(encoder, view);
        }
    }

    /// Render SSAO
    pub fn render_ssao(&self, encoder: &mut wgpu::CommandEncoder) {
        self.ssao_pipeline.render(encoder);
    }

    /// Render volumetric lighting
    pub fn render_volumetric(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(ref volumetric) = self.volumetric_pipeline {
            volumetric.render(encoder);
        }
    }

    /// Get lighting statistics
    pub fn get_stats(&self) -> &LightingStats {
        &self.stats
    }

    /// Get the most influential lights for a position
    pub fn get_influential_lights(&self, position: Vec3, max_lights: usize) -> Vec<LightInfluence> {
        let mut influences = Vec::new();

        // Collect influences from all light types
        for light in &self.directional_lights {
            if let Some(influence) = light.get_influence(position) {
                influences.push(influence);
            }
        }

        for light in &self.point_lights {
            if let Some(influence) = light.get_influence(position) {
                influences.push(influence);
            }
        }

        for light in &self.spot_lights {
            if let Some(influence) = light.get_influence(position) {
                influences.push(influence);
            }
        }

        // Sort by influence strength and take top N
        influences.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());
        influences.truncate(max_lights);

        influences
    }

    /// Sample environment lighting from light probes
    pub fn sample_environment(&self, position: Vec3, normal: Vec3) -> Vec3 {
        if self.light_probes.is_empty() {
            return Vec3::ZERO;
        }

        // Find closest light probe
        let mut closest_probe = &self.light_probes[0];
        let mut closest_distance = (position - closest_probe.position).length_squared();

        for probe in &self.light_probes {
            let distance = (position - probe.position).length_squared();
            if distance < closest_distance {
                closest_distance = distance;
                closest_probe = probe;
            }
        }

        // Sample irradiance from probe
        closest_probe.sample_irradiance(normal)
    }
}

/// Light influence data
#[derive(Debug, Clone)]
pub struct LightInfluence {
    pub light_type: LightType,
    pub position: Option<Vec3>,
    pub direction: Option<Vec3>,
    pub color: Vec3,
    pub intensity: f32,
    pub range: Option<f32>,
}

/// Light types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    Directional,
    Point,
    Spot,
    Area,
}

/// Directional light
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub cast_shadows: bool,
}

impl DirectionalLight {
    pub fn new(direction: Vec3, color: Vec3, intensity: f32) -> Self {
        Self {
            direction: direction.normalize(),
            color,
            intensity,
            cast_shadows: true,
        }
    }

    pub fn get_influence(&self, _position: Vec3) -> Option<LightInfluence> {
        Some(LightInfluence {
            light_type: LightType::Directional,
            position: None,
            direction: Some(self.direction),
            color: self.color,
            intensity: self.intensity,
            range: None,
        })
    }
}

/// Point light
#[derive(Debug, Clone)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub cast_shadows: bool,
}

impl PointLight {
    pub fn new(position: Vec3, color: Vec3, intensity: f32, range: f32) -> Self {
        Self {
            position,
            color,
            intensity,
            range,
            cast_shadows: true,
        }
    }

    pub fn get_influence(&self, position: Vec3) -> Option<LightInfluence> {
        let distance = (position - self.position).length();
        if distance > self.range {
            return None;
        }

        let attenuation = 1.0 / (distance * distance + 0.1); // Prevent division by zero
        let intensity = self.intensity * attenuation;

        Some(LightInfluence {
            light_type: LightType::Point,
            position: Some(self.position),
            direction: Some((position - self.position).normalize()),
            color: self.color,
            intensity,
            range: Some(self.range),
        })
    }
}

/// Spot light
#[derive(Debug, Clone)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32, // radians
    pub outer_angle: f32, // radians
    pub cast_shadows: bool,
}

impl SpotLight {
    pub fn new(position: Vec3, direction: Vec3, color: Vec3, intensity: f32, range: f32, inner_angle: f32, outer_angle: f32) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            color,
            intensity,
            range,
            inner_angle,
            outer_angle,
            cast_shadows: true,
        }
    }

    pub fn get_influence(&self, position: Vec3) -> Option<LightInfluence> {
        let to_light = self.position - position;
        let distance = to_light.length();

        if distance > self.range {
            return None;
        }

        let light_dir = to_light.normalize();
        let cos_angle = light_dir.dot(self.direction);

        if cos_angle < self.outer_angle.cos() {
            return None;
        }

        let attenuation = 1.0 / (distance * distance + 0.1);
        let angle_attenuation = ((cos_angle - self.outer_angle.cos()) /
                                (self.inner_angle.cos() - self.outer_angle.cos())).clamp(0.0, 1.0);
        let intensity = self.intensity * attenuation * angle_attenuation;

        Some(LightInfluence {
            light_type: LightType::Spot,
            position: Some(self.position),
            direction: Some(self.direction),
            color: self.color,
            intensity,
            range: Some(self.range),
        })
    }
}

/// Area light (rectangular)
#[derive(Debug, Clone)]
pub struct AreaLight {
    pub position: Vec3,
    pub normal: Vec3,
    pub right: Vec3,
    pub width: f32,
    pub height: f32,
    pub color: Vec3,
    pub intensity: f32,
}

impl AreaLight {
    pub fn new(position: Vec3, normal: Vec3, right: Vec3, width: f32, height: f32, color: Vec3, intensity: f32) -> Self {
        Self {
            position,
            normal: normal.normalize(),
            right: right.normalize(),
            width,
            height,
            color,
            intensity,
        }
    }
}

/// Shadow map
pub struct ShadowMap {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    size: u32,
}

impl ShadowMap {
    pub fn new(device: &wgpu::Device, size: u32) -> Result<Self, LightingError> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
            size,
        })
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        // Update shadow map data if needed
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        // Render shadow map
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        // Render scene from light's perspective
        // (Implementation would render shadow-casting objects here)

        drop(render_pass);
    }
}

/// Cascaded shadow maps for directional lights
pub struct CascadedShadowMap {
    cascades: Vec<ShadowMap>,
    cascade_splits: Vec<f32>,
}

impl CascadedShadowMap {
    pub fn new(device: &wgpu::Device, num_cascades: u32, size: u32) -> Result<Self, LightingError> {
        let mut cascades = Vec::new();
        for _ in 0..num_cascades {
            cascades.push(ShadowMap::new(device, size)?);
        }

        // Calculate cascade splits (using practical split scheme)
        let mut cascade_splits = Vec::new();
        let lambda = 0.95; // Blend factor between uniform and logarithmic splits
        let near = 0.1;
        let far = 1000.0;

        for i in 0..num_cascades {
            let uniform = near + (far - near) * (i as f32 + 1.0) / num_cascades as f32;
            let logarithmic = near * (far / near).powf((i as f32 + 1.0) / num_cascades as f32);
            let split = lambda * logarithmic + (1.0 - lambda) * uniform;
            cascade_splits.push(split);
        }

        Ok(Self {
            cascades,
            cascade_splits,
        })
    }

    pub fn update(&mut self, camera: &CameraData, queue: &wgpu::Queue) {
        // Update cascade view-projection matrices
        for (i, shadow_map) in self.cascades.iter_mut().enumerate() {
            // Calculate frustum for this cascade
            let near = if i == 0 { camera.near } else { self.cascade_splits[i - 1] };
            let far = if i == self.cascades.len() - 1 { camera.far } else { self.cascade_splits[i] };

            // Update shadow map
            shadow_map.update(queue);
        }
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        for shadow_map in &self.cascades {
            shadow_map.render(encoder, view);
        }
    }
}

/// SSAO (Screen Space Ambient Occlusion) pipeline
pub struct SSAOPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl SSAOPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        // Create SSAO texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Texture"),
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create pipeline and bind group (simplified)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SSAO Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("SSAO Vertex Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.vert.wgsl")),
                    
                }),
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("SSAO Fragment Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.frag.wgsl")),
                    
                }),
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Bind Group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[],
        });

        Self {
            pipeline,
            bind_group,
            texture,
            view,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        // Update SSAO parameters
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SSAO Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Draw fullscreen triangle

        drop(render_pass);
    }
}

/// Light probe for environment lighting
#[derive(Debug, Clone)]
pub struct LightProbe {
    pub position: Vec3,
    pub irradiance_map: Vec<Vec3>, // Spherical harmonics or irradiance map
    pub range: f32,
}

impl LightProbe {
    pub fn new(position: Vec3, range: f32) -> Self {
        Self {
            position,
            irradiance_map: Vec::new(),
            range,
        }
    }

    pub fn sample_irradiance(&self, normal: Vec3) -> Vec3 {
        // Simple diffuse irradiance approximation
        // In practice, this would sample from a precomputed irradiance map
        let intensity = normal.dot(Vec3::new(0.0, 1.0, 0.0)).max(0.0);
        Vec3::new(intensity * 0.5, intensity * 0.7, intensity * 0.9)
    }
}

/// Camera data for lighting calculations
#[derive(Debug, Clone)]
pub struct CameraData {
    pub position: Vec3,
    pub near: f32,
    pub far: f32,
    pub fov: f32,
    pub aspect_ratio: f32,
}

/// Lighting statistics
#[derive(Debug, Clone, Default)]
pub struct LightingStats {
    pub total_lights: usize,
    pub directional_lights: usize,
    pub point_lights: usize,
    pub spot_lights: usize,
    pub area_lights: usize,
    pub shadow_maps: usize,
    pub cascaded_shadow_maps: usize,
    pub light_probes: usize,
}

/// Lighting system errors
#[derive(Debug, thiserror::Error)]
pub enum LightingError {
    #[error("Failed to create shadow map texture")]
    ShadowMapCreationFailed,

    #[error("Invalid cascade count: {0}")]
    InvalidCascadeCount(u32),

    #[error("Device error: {0}")]
    DeviceError(String),
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_directional_light() {
        let light = DirectionalLight::new(
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
            1.0
        );

        let influence = light.get_influence(Vec3::ZERO).unwrap();
        assert_eq!(influence.light_type, LightType::Directional);
        assert_eq!(influence.intensity, 1.0);
    }

    #[test]
    fn test_point_light() {
        let light = PointLight::new(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
            10.0,
            20.0
        );

        // Test within range
        let influence = light.get_influence(Vec3::ZERO).unwrap();
        assert_eq!(influence.light_type, LightType::Point);
        assert!(influence.intensity > 0.0);

        // Test outside range
        assert!(light.get_influence(Vec3::new(0.0, 30.0, 0.0)).is_none());
    }

    #[test]
    fn test_spot_light() {
        let light = SpotLight::new(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
            10.0,
            20.0,
            30.0f32.to_radians(),
            45.0f32.to_radians()
        );

        // Test within cone
        let influence = light.get_influence(Vec3::ZERO).unwrap();
        assert_eq!(influence.light_type, LightType::Spot);
        assert!(influence.intensity > 0.0);
    }

    #[test]
    fn test_light_probe() {
        let probe = LightProbe::new(Vec3::ZERO, 10.0);

        let irradiance = probe.sample_irradiance(Vec3::new(0.0, 1.0, 0.0));
        assert!(irradiance.x >= 0.0 && irradiance.y >= 0.0 && irradiance.z >= 0.0);
    }
}
