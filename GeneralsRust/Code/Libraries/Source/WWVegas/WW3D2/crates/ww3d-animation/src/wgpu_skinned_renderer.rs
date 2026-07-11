//! WGPU Skinned Mesh Renderer
//!
//! Provides GPU-accelerated rendering for W3D skinned meshes with skeletal animation.
//! Implements vertex skinning on the GPU using bone matrices in uniform buffers.
//!
//! Reference: Modern GPU skinning techniques, wgpu best practices
use crate::skeletal_animation::{AnimatedModel, SkeletonState, MAX_BONES};
use crate::w3d_model_loader::{W3DMeshData, W3DModel};
use glam::Vec3;
use std::mem;
use wgpu::util::DeviceExt;

/// Vertex format for skinned meshes
/// Compatible with WGPU vertex buffers
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedVertex {
    /// Position in model space
    pub position: [f32; 3],
    /// Normal vector
    pub normal: [f32; 3],
    /// Texture coordinates
    pub tex_coord: [f32; 2],
    /// Bone index (single bone per vertex in W3D format)
    pub bone_index: u32,
    /// Bone weight (always 1.0 in W3D)
    pub bone_weight: f32,
    /// Padding to align to 16 bytes
    pub _padding: [f32; 2],
}

impl SkinnedVertex {
    /// Get vertex buffer layout for WGPU
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<SkinnedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // TexCoord
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Bone Index
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
                // Bone Weight
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 8]>() + mem::size_of::<u32>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Uniform buffer for bone matrices
/// Must match shader layout exactly
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoneMatricesUniform {
    /// Bone skinning matrices (up to MAX_BONES)
    pub matrices: [[f32; 16]; MAX_BONES],
}

impl Default for BoneMatricesUniform {
    fn default() -> Self {
        Self {
            matrices: [[0.0; 16]; MAX_BONES],
        }
    }
}

impl BoneMatricesUniform {
    /// Create from skeleton state
    pub fn from_skeleton(skeleton: &SkeletonState) -> Self {
        let mut uniform = Self::default();
        let matrices = skeleton.get_skinning_matrices();

        for (i, matrix) in matrices.iter().enumerate().take(MAX_BONES) {
            uniform.matrices[i] = matrix.to_cols_array();
        }

        uniform
    }

    /// Create from animated model
    pub fn from_model(model: &AnimatedModel) -> Self {
        Self::from_skeleton(&model.skeleton)
    }
}

/// GPU mesh buffer
pub struct SkinnedMeshBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    pub index_count: u32,
}

/// Skinned mesh renderer for WGPU
pub struct SkinnedMeshRenderer {
    /// Bone matrices uniform buffer
    bone_buffer: wgpu::Buffer,
    /// Bind group for bone matrices
    bone_bind_group: wgpu::BindGroup,
    /// Bind group layout
    bone_bind_group_layout: wgpu::BindGroupLayout,
}

impl SkinnedMeshRenderer {
    /// Create a new skinned mesh renderer
    pub fn new(device: &wgpu::Device) -> Self {
        // Create bind group layout for bone matrices
        let bone_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bone Matrices Bind Group Layout"),
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

        // Create bone matrices buffer
        let bone_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bone Matrices Buffer"),
            size: mem::size_of::<BoneMatricesUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bone Matrices Bind Group"),
            layout: &bone_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_buffer.as_entire_binding(),
            }],
        });

        Self {
            bone_buffer,
            bone_bind_group,
            bone_bind_group_layout,
        }
    }

    /// Update bone matrices from animated model
    pub fn update_bones(&self, queue: &wgpu::Queue, model: &AnimatedModel) {
        let uniform = BoneMatricesUniform::from_model(model);
        queue.write_buffer(&self.bone_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Update bone matrices from skeleton state
    pub fn update_bones_from_skeleton(&self, queue: &wgpu::Queue, skeleton: &SkeletonState) {
        let uniform = BoneMatricesUniform::from_skeleton(skeleton);
        queue.write_buffer(&self.bone_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Get bind group layout
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bone_bind_group_layout
    }

    /// Get bind group
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bone_bind_group
    }

    /// Create vertex and index buffers from W3D mesh data
    pub fn create_mesh_buffers(
        device: &wgpu::Device,
        mesh_data: &W3DMeshData,
    ) -> SkinnedMeshBuffer {
        // Convert to skinned vertex format
        let mut vertices = Vec::with_capacity(mesh_data.vertices.len());

        for i in 0..mesh_data.vertices.len() {
            let position = mesh_data.vertices.get(i).copied().unwrap_or(Vec3::ZERO);
            let normal = mesh_data.normals.get(i).copied().unwrap_or(Vec3::ZERO);
            let tex_coord = mesh_data.tex_coords.get(i).copied().unwrap_or([0.0, 0.0]);
            let bone_influence = mesh_data
                .bone_influences
                .get(i)
                .copied()
                .unwrap_or_default();

            vertices.push(SkinnedVertex {
                position: position.to_array(),
                normal: normal.to_array(),
                tex_coord,
                bone_index: bone_influence.bone_index as u32,
                bone_weight: bone_influence.weight,
                _padding: [0.0, 0.0],
            });
        }

        // Create vertex buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skinned Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create index buffer
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skinned Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&mesh_data.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        SkinnedMeshBuffer {
            vertex_buffer,
            index_buffer,
            vertex_count: vertices.len() as u32,
            index_count: mesh_data.indices.len() as u32,
        }
    }
}

/// Example WGSL shader for skinned mesh rendering
pub const SKINNED_MESH_SHADER: &str = r#"
// Vertex shader
struct BoneMatrices {
    matrices: array<mat4x4<f32>, 256>,
}

@group(0) @binding(0)
var<uniform> bones: BoneMatrices;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) bone_index: u32,
    @location(4) bone_weight: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
}

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

@group(1) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply bone transform (single bone per vertex in W3D)
    let bone_matrix = bones.matrices[in.bone_index];
    let skinned_position = bone_matrix * vec4<f32>(in.position, 1.0);
    let skinned_normal = (bone_matrix * vec4<f32>(in.normal, 0.0)).xyz;

    // Transform to world space (would apply model matrix here if needed)
    out.world_position = skinned_position.xyz;
    out.world_normal = normalize(skinned_normal);
    out.tex_coord = in.tex_coord;

    // Transform to clip space
    out.clip_position = camera.view_proj * skinned_position;

    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple diffuse lighting
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let diffuse = max(dot(in.world_normal, light_dir), 0.0);
    let ambient = 0.3;
    let color = vec3<f32>(0.8, 0.8, 0.8);

    return vec4<f32>(color * (ambient + diffuse), 1.0);
}
"#;

/// Helper to convert W3D model to GPU buffers
pub fn prepare_model_for_rendering(
    device: &wgpu::Device,
    model: &W3DModel,
) -> Vec<SkinnedMeshBuffer> {
    model
        .meshes
        .iter()
        .map(|mesh| SkinnedMeshRenderer::create_mesh_buffers(device, mesh))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skinned_vertex_size() {
        // Ensure vertex struct is properly aligned
        assert_eq!(mem::size_of::<SkinnedVertex>(), 48);
        assert_eq!(mem::align_of::<SkinnedVertex>(), 4);
    }

    #[test]
    fn test_bone_uniform_size() {
        // Verify uniform buffer size
        let expected_size = MAX_BONES * 16 * 4; // 256 matrices * 16 floats * 4 bytes
        assert_eq!(mem::size_of::<BoneMatricesUniform>(), expected_size);
    }

    #[test]
    fn test_bone_uniform_creation() {
        let uniform = BoneMatricesUniform::default();
        // All matrices should be zero-initialized
        assert_eq!(uniform.matrices[0][0], 0.0);
        assert_eq!(uniform.matrices[MAX_BONES - 1][15], 0.0);
    }
}
