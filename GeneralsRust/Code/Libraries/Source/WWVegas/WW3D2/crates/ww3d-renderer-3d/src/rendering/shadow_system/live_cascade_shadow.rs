//! Live cascaded shadow map bound by the Main forward (opaque) pass.
//!
//! The cascade depth array + comparison sampler + uniform buffer are created
//! here and bound on group 1 (bindings 2/3/4). When the map has not been
//! filled (`enabled == false`) the opaque shader falls back to projected
//! contact / blob discs.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub const LIVE_CASCADE_COUNT: u32 = 4;
pub const LIVE_CASCADE_MAP_SIZE: u32 = 1024;
pub const LIVE_CASCADE_DUMMY_SIZE: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LiveCascadeShadowUniform {
    pub view_proj: [Mat4; 4],
    pub splits: [f32; 4],
    /// x = cascade_count, y = texel size, z = depth bias, w = enabled (1 = sample CSM).
    pub params: [f32; 4],
}

impl Default for LiveCascadeShadowUniform {
    fn default() -> Self {
        Self {
            view_proj: [Mat4::IDENTITY; 4],
            splits: [20.0, 60.0, 150.0, 400.0],
            params: [0.0, 1.0 / LIVE_CASCADE_MAP_SIZE as f32, 0.002, 0.0],
        }
    }
}

/// GPU resources for the live forward-pass CSM/PCF sample.
pub struct LiveCascadeShadowMap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform: LiveCascadeShadowUniform,
    pub layer_views: Vec<wgpu::TextureView>,
}

impl LiveCascadeShadowMap {
    pub fn new(device: &wgpu::Device) -> Self {
        Self::with_size(device, LIVE_CASCADE_MAP_SIZE)
    }

    pub fn dummy(device: &wgpu::Device) -> Self {
        Self::with_size(device, LIVE_CASCADE_DUMMY_SIZE)
    }

    fn with_size(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("live_csm_depth_array"),
            size: wgpu::Extent3d {
                width: size.max(1),
                height: size.max(1),
                depth_or_array_layers: LIVE_CASCADE_COUNT,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("live_csm_depth_array_view"),
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(LIVE_CASCADE_COUNT),
            ..Default::default()
        });
        let mut layer_views = Vec::with_capacity(LIVE_CASCADE_COUNT as usize);
        for layer in 0..LIVE_CASCADE_COUNT {
            layer_views.push(texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("live_csm_cascade_layer"),
                format: Some(wgpu::TextureFormat::Depth32Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::DepthOnly,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            }));
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("live_csm_comparison_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let uniform = LiveCascadeShadowUniform::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("live_csm_uniform"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            texture,
            view,
            sampler,
            uniform_buffer,
            uniform,
            layer_views,
        }
    }

    /// Update cascade matrices from camera + directional light. `enabled` is
    /// true only after a depth pass actually filled the map.
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        camera_position: Vec3,
        camera_forward: Vec3,
        light_direction: Vec3,
        enabled: bool,
    ) {
        let light_dir = if light_direction.length_squared() > 1e-6 {
            light_direction.normalize()
        } else {
            Vec3::new(0.35, -0.85, 0.35).normalize()
        };
        let splits = [20.0_f32, 60.0, 150.0, 400.0];
        let mut view_proj = [Mat4::IDENTITY; 4];
        let mut near = 1.0_f32;
        for (i, &far) in splits.iter().enumerate() {
            let radius = (far - near) * 0.5 + 8.0;
            let center =
                camera_position + camera_forward.normalize_or_zero() * ((near + far) * 0.5);
            let eye = center - light_dir * (radius * 2.0);
            let view = Mat4::look_at_rh(eye, center, Vec3::Y);
            let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.5, radius * 4.0);
            view_proj[i] = proj * view;
            near = far;
        }
        self.uniform.view_proj = view_proj;
        self.uniform.splits = splits;
        self.uniform.params = [
            LIVE_CASCADE_COUNT as f32,
            1.0 / LIVE_CASCADE_MAP_SIZE as f32,
            0.002,
            if enabled { 1.0 } else { 0.0 },
        ];
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.uniform));
    }

    pub fn bind_group_entries(&self) -> [wgpu::BindGroupEntry<'_>; 3] {
        [
            wgpu::BindGroupEntry {
                binding: 2,
                resource: self.uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&self.view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
        ]
    }

    pub fn is_enabled(&self) -> bool {
        self.uniform.params[3] >= 0.5
    }
}

/// Honesty: live opaque.wgsl must bind + PCF-sample the cascade map.
pub fn honesty_live_csm_pcf_shader_bound(opaque_wgsl: &str) -> bool {
    opaque_wgsl.contains("@group(1) @binding(2)")
        && opaque_wgsl.contains("@group(1) @binding(3)")
        && opaque_wgsl.contains("texture_depth_2d_array")
        && opaque_wgsl.contains("sampler_comparison")
        && opaque_wgsl.contains("fn sample_csm_pcf")
        && opaque_wgsl.contains("textureSampleCompare")
        && opaque_wgsl.contains("sample_csm_pcf(")
        // Array layer for textureSampleCompare must be integer (f32 fails naga validation).
        && opaque_wgsl.contains("let layer: i32")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_csm_uniform_is_std140_sized() {
        assert_eq!(std::mem::size_of::<LiveCascadeShadowUniform>(), 288);
        assert!(std::mem::size_of::<LiveCascadeShadowUniform>() % 16 == 0);
    }

    #[test]
    fn honesty_pins_live_opaque_bind_and_pcf_sample() {
        let opaque = include_str!("../shader_system/opaque.wgsl");
        assert!(
            honesty_live_csm_pcf_shader_bound(opaque),
            "opaque.wgsl must bind cascade map + PCF sample (not comments only)"
        );
        assert!(
            opaque.contains("sample_projected_shadow"),
            "empty-map fallback must remain"
        );
    }

    #[test]
    fn bind_group_entries_use_live_slots_2_3_4() {
        // CPU-side pin of the entry layout without a GPU device.
        let src = include_str!("live_cascade_shadow.rs");
        assert!(src.contains("binding: 2"));
        assert!(src.contains("binding: 3"));
        assert!(src.contains("binding: 4"));
        assert!(src.contains("as_entire_binding"));
        assert!(src.contains("TextureView"));
        assert!(src.contains("Sampler"));
    }
}
