//! Uniform Buffer Management for WW3D Rendering
//!
//! This module defines the uniform buffer structures used in the rendering pipeline.
//! Uniforms are organized by update frequency:
//! - FrameUniforms: Updated once per frame (camera, lighting, fog)
//! - ObjectUniforms: Updated per object (model matrix, bone transforms)
//! - MaterialUniforms: Updated per material pass (colors, shininess)
//!
//! Reference: GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dx8wrapper.h (shader state)

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Per-frame uniforms - updated once per frame
/// Contains camera and global scene parameters
///
/// WGSL Layout:
/// ```wgsl
/// struct FrameUniforms {
///     view_proj: mat4x4<f32>,      // Offset: 0,   Size: 64
///     camera_pos: vec3<f32>,       // Offset: 64,  Size: 12
///     _pad0: f32,                  // Offset: 76,  Size: 4  (padding)
///     ambient_color: vec3<f32>,    // Offset: 80,  Size: 12
///     _pad1: f32,                  // Offset: 92,  Size: 4  (padding)
///     fog_color: vec3<f32>,        // Offset: 96,  Size: 12
///     fog_start: f32,              // Offset: 108, Size: 4
///     fog_end: f32,                // Offset: 112, Size: 4
///     time: f32,                   // Offset: 116, Size: 4
/// }
/// ```
/// Total size: 120 bytes (padded to 128 for alignment)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    /// Combined view-projection matrix
    pub view_proj_matrix: Mat4,

    /// Camera position in world space
    pub camera_pos: Vec3,
    pub _pad0: f32,

    /// Global ambient light color
    pub ambient_color: Vec3,
    pub _pad1: f32,

    /// Fog color
    pub fog_color: Vec3,

    /// Fog start distance
    pub fog_start: f32,

    /// Fog end distance
    pub fog_end: f32,

    /// Current frame time (for animated shaders)
    pub time: f32,

    /// Padding to 128 bytes for alignment
    pub _pad2: [f32; 2],
}

impl Default for FrameUniforms {
    fn default() -> Self {
        Self {
            view_proj_matrix: Mat4::IDENTITY,
            camera_pos: Vec3::ZERO,
            _pad0: 0.0,
            ambient_color: Vec3::new(0.2, 0.2, 0.2),
            _pad1: 0.0,
            fog_color: Vec3::new(0.5, 0.5, 0.5),
            fog_start: 100.0,
            fog_end: 1000.0,
            time: 0.0,
            _pad2: [0.0; 2],
        }
    }
}

impl FrameUniforms {
    /// Create new frame uniforms
    pub fn new(
        view_proj_matrix: Mat4,
        camera_pos: Vec3,
        ambient_color: Vec3,
        fog_color: Vec3,
        fog_start: f32,
        fog_end: f32,
        time: f32,
    ) -> Self {
        Self {
            view_proj_matrix,
            camera_pos,
            _pad0: 0.0,
            ambient_color,
            _pad1: 0.0,
            fog_color,
            fog_start,
            fog_end,
            time,
            _pad2: [0.0; 2],
        }
    }

    /// Create a uniform buffer on the GPU
    pub fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Frame Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[*self]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    /// Update an existing buffer
    pub fn update_buffer(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

/// Per-object uniforms - updated per object/mesh
/// Contains transformation and skinning data
///
/// WGSL Layout:
/// ```wgsl
/// struct ObjectUniforms {
///     model_matrix: mat4x4<f32>,           // Offset: 0,    Size: 64
///     bone_matrices: array<mat4x4<f32>, 64>,  // Offset: 64,   Size: 4096
///     object_color: vec4<f32>,             // Offset: 4160, Size: 16
/// }
/// ```
/// Total size: 4176 bytes (padded to 4192 for alignment)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ObjectUniforms {
    /// Model-to-world transformation matrix
    pub model_matrix: Mat4,

    /// Bone transformation matrices for skinning (up to 64 bones)
    /// C++ Reference: dx8wrapper.h MAX_VERTEX_SHADER_CONSTANTS
    pub bone_matrices: [Mat4; 64],

    /// Per-object color tint
    pub object_color: Vec4,
}

impl Default for ObjectUniforms {
    fn default() -> Self {
        Self {
            model_matrix: Mat4::IDENTITY,
            bone_matrices: [Mat4::IDENTITY; 64],
            object_color: Vec4::ONE,
        }
    }
}

impl ObjectUniforms {
    /// Create new object uniforms
    pub fn new(model_matrix: Mat4) -> Self {
        Self {
            model_matrix,
            bone_matrices: [Mat4::IDENTITY; 64],
            object_color: Vec4::ONE,
        }
    }

    /// Create new object uniforms with skinning
    pub fn with_bones(model_matrix: Mat4, bone_matrices: &[Mat4]) -> Self {
        let mut uniforms = Self::new(model_matrix);
        let bone_count = bone_matrices.len().min(64);
        uniforms.bone_matrices[..bone_count].copy_from_slice(&bone_matrices[..bone_count]);
        uniforms
    }

    /// Set object color tint
    pub fn with_color(mut self, color: Vec4) -> Self {
        self.object_color = color;
        self
    }

    /// Create a uniform buffer on the GPU
    pub fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Object Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[*self]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    /// Update an existing buffer
    pub fn update_buffer(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

/// Per-material uniforms - updated per material pass
/// Contains material properties for lighting calculations
///
/// WGSL Layout:
/// ```wgsl
/// struct MaterialUniforms {
///     diffuse: vec4<f32>,      // Offset: 0,  Size: 16
///     specular: vec4<f32>,     // Offset: 16, Size: 16
///     emissive: vec4<f32>,     // Offset: 32, Size: 16
///     shininess: f32,          // Offset: 48, Size: 4
///     opacity: f32,            // Offset: 52, Size: 4
///     translucency: f32,       // Offset: 56, Size: 4
/// }
/// ```
/// Total size: 60 bytes (padded to 64 for alignment)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    /// Diffuse color (base color)
    /// C++ Reference: vertmaterial.h W3dVertexMaterialStruct::diffuse
    pub diffuse: Vec4,

    /// Specular color and intensity
    /// C++ Reference: vertmaterial.h W3dVertexMaterialStruct::specular
    pub specular: Vec4,

    /// Emissive color (self-illumination)
    /// C++ Reference: vertmaterial.h W3dVertexMaterialStruct::emissive
    pub emissive: Vec4,

    /// Specular exponent (shininess)
    /// Higher values = smaller, sharper highlights
    /// C++ Reference: vertmaterial.h W3dVertexMaterialStruct::shininess
    pub shininess: f32,

    /// Material opacity (0 = transparent, 1 = opaque)
    /// C++ Reference: vertmaterial.h W3dVertexMaterialStruct::opacity
    pub opacity: f32,

    /// Translucency (subsurface scattering approximation)
    /// C++ Reference: vertmaterial.h W3dVertexMaterialStruct::translucency
    pub translucency: f32,

    /// Padding to 64 bytes
    pub _pad: f32,
}

impl Default for MaterialUniforms {
    fn default() -> Self {
        Self {
            diffuse: Vec4::ONE,
            specular: Vec4::new(0.5, 0.5, 0.5, 1.0),
            emissive: Vec4::ZERO,
            shininess: 32.0,
            opacity: 1.0,
            translucency: 0.0,
            _pad: 0.0,
        }
    }
}

impl MaterialUniforms {
    /// Create new material uniforms
    pub fn new(
        diffuse: Vec4,
        specular: Vec4,
        emissive: Vec4,
        shininess: f32,
        opacity: f32,
    ) -> Self {
        Self {
            diffuse,
            specular,
            emissive,
            shininess,
            opacity,
            translucency: 0.0,
            _pad: 0.0,
        }
    }

    /// Create material from vertex material colors
    /// C++ Reference: vertmaterial.h VertexMaterialClass
    pub fn from_vertex_material(
        ambient: Vec3,
        diffuse: Vec3,
        specular: Vec3,
        emissive: Vec3,
        shininess: f32,
        opacity: f32,
        translucency: f32,
    ) -> Self {
        // Combine ambient with diffuse (standard approach)
        let combined_diffuse = Vec4::new(
            diffuse.x + ambient.x * 0.2,
            diffuse.y + ambient.y * 0.2,
            diffuse.z + ambient.z * 0.2,
            opacity,
        );

        Self {
            diffuse: combined_diffuse,
            specular: Vec4::new(specular.x, specular.y, specular.z, 1.0),
            emissive: Vec4::new(emissive.x, emissive.y, emissive.z, 1.0),
            shininess,
            opacity,
            translucency,
            _pad: 0.0,
        }
    }

    /// Set translucency
    pub fn with_translucency(mut self, translucency: f32) -> Self {
        self.translucency = translucency;
        self
    }

    /// Check if material is transparent
    pub fn is_transparent(&self) -> bool {
        self.opacity < 1.0 || self.translucency > 0.0
    }

    /// Create a uniform buffer on the GPU
    pub fn create_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[*self]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    /// Update an existing buffer
    pub fn update_buffer(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*self]));
    }
}

/// Bind group layout builder for uniforms
pub struct UniformBindGroupLayouts {
    pub frame_layout: wgpu::BindGroupLayout,
    pub object_layout: wgpu::BindGroupLayout,
    pub material_layout: wgpu::BindGroupLayout,
}

impl UniformBindGroupLayouts {
    /// Create all uniform bind group layouts
    pub fn new(device: &wgpu::Device) -> Self {
        // Group 0: Frame uniforms
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Frame Uniforms Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Group 1: Object uniforms
        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Object Uniforms Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Group 2: Material uniforms (texture + sampler will be added separately)
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Uniforms Bind Group Layout"),
            entries: &[
                // Binding 0: Material uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Texture (if present)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 2: Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            frame_layout,
            object_layout,
            material_layout,
        }
    }
}

/// Uniform buffer manager - manages GPU buffers for uniforms
pub struct UniformBufferManager {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    /// Frame uniform buffer (updated once per frame)
    frame_buffer: Option<wgpu::Buffer>,

    /// Object uniform buffers (pool for reuse)
    object_buffer_pool: Vec<wgpu::Buffer>,
    object_buffer_in_use: Vec<bool>,

    /// Material uniform buffers (pool for reuse)
    material_buffer_pool: Vec<wgpu::Buffer>,
    material_buffer_in_use: Vec<bool>,
}

impl UniformBufferManager {
    /// Create a new uniform buffer manager
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device,
            queue,
            frame_buffer: None,
            object_buffer_pool: Vec::new(),
            object_buffer_in_use: Vec::new(),
            material_buffer_pool: Vec::new(),
            material_buffer_in_use: Vec::new(),
        }
    }

    /// Update frame uniforms
    pub fn update_frame_uniforms(&mut self, uniforms: &FrameUniforms) -> &wgpu::Buffer {
        if self.frame_buffer.is_none() {
            self.frame_buffer = Some(uniforms.create_buffer(&self.device));
        } else {
            uniforms.update_buffer(&self.queue, self.frame_buffer.as_ref().unwrap());
        }
        self.frame_buffer.as_ref().unwrap()
    }

    /// Allocate object uniform buffer from pool
    pub fn allocate_object_buffer(&mut self, uniforms: &ObjectUniforms) -> (usize, &wgpu::Buffer) {
        // Find free buffer in pool
        for (i, in_use) in self.object_buffer_in_use.iter_mut().enumerate() {
            if !*in_use {
                *in_use = true;
                uniforms.update_buffer(&self.queue, &self.object_buffer_pool[i]);
                return (i, &self.object_buffer_pool[i]);
            }
        }

        // Allocate new buffer
        let buffer = uniforms.create_buffer(&self.device);
        let index = self.object_buffer_pool.len();
        self.object_buffer_pool.push(buffer);
        self.object_buffer_in_use.push(true);
        (index, &self.object_buffer_pool[index])
    }

    /// Free object buffer back to pool
    pub fn free_object_buffer(&mut self, index: usize) {
        if index < self.object_buffer_in_use.len() {
            self.object_buffer_in_use[index] = false;
        }
    }

    /// Allocate material uniform buffer from pool
    pub fn allocate_material_buffer(
        &mut self,
        uniforms: &MaterialUniforms,
    ) -> (usize, &wgpu::Buffer) {
        // Find free buffer in pool
        for (i, in_use) in self.material_buffer_in_use.iter_mut().enumerate() {
            if !*in_use {
                *in_use = true;
                uniforms.update_buffer(&self.queue, &self.material_buffer_pool[i]);
                return (i, &self.material_buffer_pool[i]);
            }
        }

        // Allocate new buffer
        let buffer = uniforms.create_buffer(&self.device);
        let index = self.material_buffer_pool.len();
        self.material_buffer_pool.push(buffer);
        self.material_buffer_in_use.push(true);
        (index, &self.material_buffer_pool[index])
    }

    /// Free material buffer back to pool
    pub fn free_material_buffer(&mut self, index: usize) {
        if index < self.material_buffer_in_use.len() {
            self.material_buffer_in_use[index] = false;
        }
    }

    /// Reset all buffers for next frame
    pub fn reset_frame(&mut self) {
        // Mark all buffers as free
        for in_use in &mut self.object_buffer_in_use {
            *in_use = false;
        }
        for in_use in &mut self.material_buffer_in_use {
            *in_use = false;
        }
    }

    /// Get buffer pool statistics
    pub fn stats(&self) -> UniformBufferStats {
        UniformBufferStats {
            object_buffers: self.object_buffer_pool.len(),
            object_buffers_in_use: self.object_buffer_in_use.iter().filter(|&&x| x).count(),
            material_buffers: self.material_buffer_pool.len(),
            material_buffers_in_use: self.material_buffer_in_use.iter().filter(|&&x| x).count(),
        }
    }
}

/// Uniform buffer statistics
#[derive(Debug, Clone, Copy)]
pub struct UniformBufferStats {
    pub object_buffers: usize,
    pub object_buffers_in_use: usize,
    pub material_buffers: usize,
    pub material_buffers_in_use: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_uniforms_size() {
        // Ensure correct size for GPU alignment
        assert_eq!(std::mem::size_of::<FrameUniforms>(), 128);
    }

    #[test]
    fn test_object_uniforms_size() {
        // Bone matrices: 64 * 64 bytes = 4096
        // Model matrix: 64 bytes
        // Object color: 16 bytes
        // Total: 4176 bytes
        assert_eq!(std::mem::size_of::<ObjectUniforms>(), 4176);
    }

    #[test]
    fn test_material_uniforms_size() {
        // Should be 64 bytes for alignment
        assert_eq!(std::mem::size_of::<MaterialUniforms>(), 64);
    }

    #[test]
    fn test_frame_uniforms_default() {
        let uniforms = FrameUniforms::default();
        assert_eq!(uniforms.view_proj_matrix, Mat4::IDENTITY);
        assert_eq!(uniforms.camera_pos, Vec3::ZERO);
        assert_eq!(uniforms.fog_start, 100.0);
        assert_eq!(uniforms.fog_end, 1000.0);
    }

    #[test]
    fn test_object_uniforms_with_bones() {
        let bones = vec![Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0)); 32];
        let uniforms = ObjectUniforms::with_bones(Mat4::IDENTITY, &bones);

        assert_eq!(uniforms.bone_matrices[0], bones[0]);
        assert_eq!(uniforms.bone_matrices[31], bones[31]);
        assert_eq!(uniforms.bone_matrices[32], Mat4::IDENTITY); // Rest should be identity
    }

    #[test]
    fn test_material_uniforms() {
        let mat = MaterialUniforms::new(
            Vec4::ONE,
            Vec4::new(0.5, 0.5, 0.5, 1.0),
            Vec4::ZERO,
            32.0,
            0.5,
        );

        assert_eq!(mat.diffuse, Vec4::ONE);
        assert_eq!(mat.opacity, 0.5);
        assert!(mat.is_transparent());
    }

    #[test]
    fn test_material_from_vertex_material() {
        let mat = MaterialUniforms::from_vertex_material(
            Vec3::new(0.2, 0.2, 0.2),
            Vec3::new(0.8, 0.8, 0.8),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::ZERO,
            32.0,
            1.0,
            0.0,
        );

        // Diffuse should combine ambient contribution
        assert!(mat.diffuse.x > 0.8);
        assert_eq!(mat.shininess, 32.0);
    }

    #[test]
    fn test_bytemuck_pod() {
        // Ensure all uniform types are Pod
        let _: &[u8] = bytemuck::bytes_of(&FrameUniforms::default());
        let _: &[u8] = bytemuck::bytes_of(&ObjectUniforms::default());
        let _: &[u8] = bytemuck::bytes_of(&MaterialUniforms::default());
    }
}
