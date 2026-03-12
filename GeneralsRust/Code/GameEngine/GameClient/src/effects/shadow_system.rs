//! Shadow Rendering System
//!
//! Provides dynamic shadow mapping with support for multiple shadow casters,
//! cascaded shadow maps, and soft shadows.

use nalgebra::{Matrix4, Point3, Vector3};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferUsages, Device, Extent3d, Queue, Sampler,
    SamplerBindingType, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor,
};

use super::EffectsError;

/// Shadow map resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowMapResolution {
    Low = 512,
    Medium = 1024,
    High = 2048,
    Ultra = 4096,
}

/// Shadow quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowQuality {
    /// No shadows
    None,
    /// Hard shadows, single shadow map
    Low,
    /// Soft shadows with PCF
    Medium,
    /// Cascaded shadow maps with PCF
    High,
    /// CSM with PCF and high resolution
    Ultra,
}

impl ShadowQuality {
    /// Get shadow map resolution for this quality
    pub fn resolution(self) -> ShadowMapResolution {
        match self {
            ShadowQuality::None => ShadowMapResolution::Low,
            ShadowQuality::Low => ShadowMapResolution::Low,
            ShadowQuality::Medium => ShadowMapResolution::Medium,
            ShadowQuality::High => ShadowMapResolution::High,
            ShadowQuality::Ultra => ShadowMapResolution::Ultra,
        }
    }

    /// Get number of PCF samples
    pub fn pcf_samples(self) -> u32 {
        match self {
            ShadowQuality::None | ShadowQuality::Low => 1,
            ShadowQuality::Medium => 4,
            ShadowQuality::High => 9,
            ShadowQuality::Ultra => 16,
        }
    }

    /// Check if cascaded shadow maps should be used
    pub fn use_cascades(self) -> bool {
        matches!(self, ShadowQuality::High | ShadowQuality::Ultra)
    }

    /// Get number of cascades
    pub fn cascade_count(self) -> u32 {
        match self {
            ShadowQuality::High => 3,
            ShadowQuality::Ultra => 4,
            _ => 1,
        }
    }
}

/// Shadow caster configuration
#[derive(Debug, Clone)]
pub struct ShadowCaster {
    /// Light position
    pub position: Point3<f32>,

    /// Light direction (for directional lights)
    pub direction: Vector3<f32>,

    /// Is directional light (vs point light)
    pub is_directional: bool,

    /// Shadow map index
    pub shadow_map_index: usize,

    /// Projection matrix for shadow mapping
    pub projection: Matrix4<f32>,

    /// View matrix for shadow mapping
    pub view: Matrix4<f32>,

    /// Shadow bias to prevent acne
    pub bias: f32,

    /// Shadow intensity (0.0 = no shadow, 1.0 = full shadow)
    pub intensity: f32,

    /// Maximum shadow distance
    pub max_distance: f32,
}

impl ShadowCaster {
    /// Create a directional shadow caster
    pub fn directional(
        direction: Vector3<f32>,
        shadow_map_index: usize,
        frustum_size: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let dir_normalized = direction.normalize();

        // Create orthographic projection for directional light
        let projection = Matrix4::new_orthographic(
            -frustum_size,
            frustum_size,
            -frustum_size,
            frustum_size,
            near,
            far,
        );

        // Create view matrix looking along light direction
        let position = Point3::origin() - dir_normalized * (far / 2.0);
        let target = Point3::origin();
        let up = if dir_normalized.y.abs() < 0.9 {
            Vector3::new(0.0, 1.0, 0.0)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };

        let view = Matrix4::look_at_rh(&position, &target, &up);

        Self {
            position,
            direction: dir_normalized,
            is_directional: true,
            shadow_map_index,
            projection,
            view,
            bias: 0.005,
            intensity: 0.7,
            max_distance: far,
        }
    }

    /// Create a point light shadow caster
    pub fn point(position: Point3<f32>, shadow_map_index: usize, range: f32) -> Self {
        // Use perspective projection for point light
        let projection = Matrix4::new_perspective(1.0, std::f32::consts::FRAC_PI_2, 0.1, range);

        // Default view matrix (will be updated per face for cubemap)
        let view = Matrix4::look_at_rh(
            &position,
            &(position + Vector3::new(0.0, 0.0, -1.0)),
            &Vector3::new(0.0, 1.0, 0.0),
        );

        Self {
            position,
            direction: Vector3::new(0.0, -1.0, 0.0),
            is_directional: false,
            shadow_map_index,
            projection,
            view,
            bias: 0.01,
            intensity: 0.6,
            max_distance: range,
        }
    }

    /// Get shadow matrix (projection * view)
    pub fn shadow_matrix(&self) -> Matrix4<f32> {
        self.projection * self.view
    }

    /// Update view matrix for a specific direction
    pub fn update_view(&mut self, target: Point3<f32>, up: Vector3<f32>) {
        self.view = Matrix4::look_at_rh(&self.position, &target, &up);
    }
}

/// Cascaded shadow map cascade
#[derive(Debug, Clone)]
pub struct ShadowCascade {
    /// Near plane distance
    pub near: f32,

    /// Far plane distance
    pub far: f32,

    /// Shadow matrix for this cascade
    pub shadow_matrix: Matrix4<f32>,

    /// Split distance in view space
    pub split_distance: f32,
}

/// Shadow map texture array
pub struct ShadowMapArray {
    /// Shadow map texture
    texture: Texture,

    /// Texture view
    view: TextureView,

    /// Depth sampler
    sampler: Sampler,

    /// Resolution
    resolution: ShadowMapResolution,

    /// Number of shadow maps in array
    count: u32,
}

impl ShadowMapArray {
    /// Create a new shadow map array
    pub fn new(device: &Device, resolution: ShadowMapResolution, count: u32) -> Self {
        let size = resolution as u32;

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Map Array"),
            size: Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("Shadow Map Array View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Map Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            resolution,
            count,
        }
    }

    /// Get texture view
    pub fn view(&self) -> &TextureView {
        &self.view
    }

    /// Get sampler
    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    /// Get resolution
    pub fn resolution(&self) -> ShadowMapResolution {
        self.resolution
    }

    /// Get individual layer view
    pub fn layer_view(&self, layer: u32) -> TextureView {
        self.texture.create_view(&TextureViewDescriptor {
            label: Some(&format!("Shadow Map Layer {}", layer)),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        })
    }
}

/// Shadow system manager
pub struct ShadowSystem {
    /// Shadow quality setting
    quality: ShadowQuality,

    /// Shadow casters
    casters: Vec<ShadowCaster>,

    /// Shadow map array
    shadow_maps: Option<ShadowMapArray>,

    /// Cascades for directional lights
    cascades: Vec<Vec<ShadowCascade>>,

    /// Shadow data buffer
    shadow_buffer: Option<Buffer>,

    /// Bind group layout
    bind_group_layout: Option<BindGroupLayout>,

    /// Bind group
    bind_group: Option<BindGroup>,

    /// Maximum number of shadow casters
    max_casters: u32,

    /// Global shadow intensity
    global_intensity: f32,
}

impl ShadowSystem {
    /// Create a new shadow system
    pub fn new(quality: ShadowQuality, max_casters: u32) -> Self {
        Self {
            quality,
            casters: Vec::new(),
            shadow_maps: None,
            cascades: Vec::new(),
            shadow_buffer: None,
            bind_group_layout: None,
            bind_group: None,
            max_casters,
            global_intensity: 1.0,
        }
    }

    /// Initialize GPU resources
    pub fn initialize(&mut self, device: &Device) -> Result<(), EffectsError> {
        if self.quality == ShadowQuality::None {
            return Ok(());
        }

        let resolution = self.quality.resolution();
        let total_maps = self.max_casters * self.quality.cascade_count();

        // Create shadow map array
        self.shadow_maps = Some(ShadowMapArray::new(device, resolution, total_maps));

        // Create shadow data buffer
        let buffer_size = (std::mem::size_of::<ShadowMatrixData>() * total_maps as usize) as u64;
        self.shadow_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Data Buffer"),
            size: buffer_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        // Create bind group layout
        self.bind_group_layout = Some(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Bind Group Layout"),
                entries: &[
                    // Shadow map array
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Shadow sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(SamplerBindingType::Comparison),
                        count: None,
                    },
                    // Shadow matrices buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            },
        ));

        // Create bind group
        if let (Some(shadow_maps), Some(shadow_buffer), Some(layout)) = (
            &self.shadow_maps,
            &self.shadow_buffer,
            &self.bind_group_layout,
        ) {
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Shadow Bind Group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(shadow_maps.view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(shadow_maps.sampler()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: shadow_buffer.as_entire_binding(),
                    },
                ],
            }));
        }

        Ok(())
    }

    /// Add a shadow caster
    pub fn add_caster(&mut self, caster: ShadowCaster) -> Result<usize, EffectsError> {
        if self.casters.len() >= self.max_casters as usize {
            return Err(EffectsError::ShadowError(
                "Maximum number of shadow casters reached".to_string(),
            ));
        }

        let index = self.casters.len();
        self.casters.push(caster);

        // Initialize cascades if needed
        if self.quality.use_cascades() {
            self.cascades.push(Vec::new());
        }

        Ok(index)
    }

    /// Update shadow caster
    pub fn update_caster(
        &mut self,
        index: usize,
        caster: ShadowCaster,
    ) -> Result<(), EffectsError> {
        if index >= self.casters.len() {
            return Err(EffectsError::ShadowError(
                "Invalid caster index".to_string(),
            ));
        }

        self.casters[index] = caster;
        Ok(())
    }

    /// Remove shadow caster
    pub fn remove_caster(&mut self, index: usize) -> Result<(), EffectsError> {
        if index >= self.casters.len() {
            return Err(EffectsError::ShadowError(
                "Invalid caster index".to_string(),
            ));
        }

        self.casters.remove(index);
        if self.quality.use_cascades() && index < self.cascades.len() {
            self.cascades.remove(index);
        }

        Ok(())
    }

    /// Update cascades for directional light
    pub fn update_cascades(
        &mut self,
        caster_index: usize,
        camera_view: &Matrix4<f32>,
        camera_projection: &Matrix4<f32>,
        near: f32,
        far: f32,
    ) -> Result<(), EffectsError> {
        if !self.quality.use_cascades() {
            return Ok(());
        }

        if caster_index >= self.casters.len() {
            return Err(EffectsError::ShadowError(
                "Invalid caster index".to_string(),
            ));
        }

        let caster = &self.casters[caster_index];
        if !caster.is_directional {
            return Ok(()); // Only directional lights use cascades
        }

        let cascade_count = self.quality.cascade_count() as usize;
        let mut cascades = Vec::with_capacity(cascade_count);

        // Calculate cascade split distances using practical split scheme
        let lambda = 0.75; // Blend between logarithmic and uniform
        let mut splits = Vec::with_capacity(cascade_count + 1);
        splits.push(near);

        for i in 1..cascade_count {
            let i_f = i as f32;
            let count_f = cascade_count as f32;

            // Logarithmic split
            let log_split = near * (far / near).powf(i_f / count_f);

            // Uniform split
            let uniform_split = near + (far - near) * (i_f / count_f);

            // Practical split (blend)
            let split = lambda * log_split + (1.0 - lambda) * uniform_split;
            splits.push(split);
        }
        splits.push(far);

        // Create cascades
        for i in 0..cascade_count {
            let cascade_near = splits[i];
            let cascade_far = splits[i + 1];

            // Calculate frustum corners in world space for this cascade
            let frustum_size = (cascade_far - cascade_near) * 0.5;

            let shadow_projection = Matrix4::new_orthographic(
                -frustum_size,
                frustum_size,
                -frustum_size,
                frustum_size,
                -frustum_size * 2.0,
                frustum_size * 2.0,
            );

            let shadow_view = caster.view;
            let shadow_matrix = shadow_projection * shadow_view;

            cascades.push(ShadowCascade {
                near: cascade_near,
                far: cascade_far,
                shadow_matrix,
                split_distance: cascade_far,
            });
        }

        // Store cascades
        while self.cascades.len() <= caster_index {
            self.cascades.push(Vec::new());
        }
        self.cascades[caster_index] = cascades;

        Ok(())
    }

    /// Update GPU buffers
    pub fn update_gpu_data(&self, queue: &Queue) -> Result<(), EffectsError> {
        if self.quality == ShadowQuality::None {
            return Ok(());
        }

        let shadow_buffer = self.shadow_buffer.as_ref().ok_or_else(|| {
            EffectsError::ShadowError("Shadow buffer not initialized".to_string())
        })?;

        let mut shadow_data = Vec::new();

        for (i, caster) in self.casters.iter().enumerate() {
            if self.quality.use_cascades() && i < self.cascades.len() {
                // Add cascade matrices
                for cascade in &self.cascades[i] {
                    shadow_data.push(ShadowMatrixData {
                        matrix: cascade.shadow_matrix.into(),
                        bias: caster.bias,
                        intensity: caster.intensity * self.global_intensity,
                        split_distance: cascade.split_distance,
                        _padding: 0.0,
                    });
                }
            } else {
                // Add single shadow matrix
                shadow_data.push(ShadowMatrixData {
                    matrix: caster.shadow_matrix().into(),
                    bias: caster.bias,
                    intensity: caster.intensity * self.global_intensity,
                    split_distance: caster.max_distance,
                    _padding: 0.0,
                });
            }
        }

        queue.write_buffer(shadow_buffer, 0, bytemuck::cast_slice(&shadow_data));

        Ok(())
    }

    /// Get shadow maps
    pub fn shadow_maps(&self) -> Option<&ShadowMapArray> {
        self.shadow_maps.as_ref()
    }

    /// Get bind group
    pub fn bind_group(&self) -> Option<&BindGroup> {
        self.bind_group.as_ref()
    }

    /// Get bind group layout
    pub fn bind_group_layout(&self) -> Option<&BindGroupLayout> {
        self.bind_group_layout.as_ref()
    }

    /// Get shadow casters
    pub fn casters(&self) -> &[ShadowCaster] {
        &self.casters
    }

    /// Get quality setting
    pub fn quality(&self) -> ShadowQuality {
        self.quality
    }

    /// Set quality (requires re-initialization)
    pub fn set_quality(&mut self, quality: ShadowQuality) {
        self.quality = quality;
    }

    /// Set global shadow intensity
    pub fn set_global_intensity(&mut self, intensity: f32) {
        self.global_intensity = intensity.clamp(0.0, 1.0);
    }

    /// Clear all shadow casters
    pub fn clear(&mut self) {
        self.casters.clear();
        self.cascades.clear();
    }
}

/// Shadow matrix data for GPU
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ShadowMatrixData {
    matrix: [[f32; 4]; 4],
    bias: f32,
    intensity: f32,
    split_distance: f32,
    _padding: f32,
}

unsafe impl bytemuck::Pod for ShadowMatrixData {}
unsafe impl bytemuck::Zeroable for ShadowMatrixData {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_quality() {
        assert_eq!(ShadowQuality::Low.resolution(), ShadowMapResolution::Low);
        assert_eq!(
            ShadowQuality::Ultra.resolution(),
            ShadowMapResolution::Ultra
        );

        assert_eq!(ShadowQuality::Low.pcf_samples(), 1);
        assert_eq!(ShadowQuality::Ultra.pcf_samples(), 16);

        assert!(!ShadowQuality::Low.use_cascades());
        assert!(ShadowQuality::High.use_cascades());

        assert_eq!(ShadowQuality::High.cascade_count(), 3);
        assert_eq!(ShadowQuality::Ultra.cascade_count(), 4);
    }

    #[test]
    fn test_shadow_caster_directional() {
        let caster = ShadowCaster::directional(Vector3::new(0.0, -1.0, 0.0), 0, 50.0, 0.1, 100.0);

        assert!(caster.is_directional);
        assert_eq!(caster.shadow_map_index, 0);
        assert_eq!(caster.max_distance, 100.0);
    }

    #[test]
    fn test_shadow_caster_point() {
        let caster = ShadowCaster::point(Point3::new(0.0, 10.0, 0.0), 1, 50.0);

        assert!(!caster.is_directional);
        assert_eq!(caster.shadow_map_index, 1);
        assert_eq!(caster.max_distance, 50.0);
    }

    #[test]
    fn test_shadow_system_creation() {
        let system = ShadowSystem::new(ShadowQuality::High, 4);

        assert_eq!(system.quality(), ShadowQuality::High);
        assert_eq!(system.casters().len(), 0);
    }
}
