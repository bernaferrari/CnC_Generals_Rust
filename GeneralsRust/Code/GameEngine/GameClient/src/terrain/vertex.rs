//! Shared terrain vertex definition used by CPU terrain generation and GPU rendering.
//!
//! The live WGPU mesh carries C++ terrain's two independent tile coordinates
//! along with texture bindings, per-corner blend weights, and baked lighting.

use glam::Vec3;

/// Vertex layout used by both the terrain chunk generator and the wgpu renderer.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub blend_indices: [u16; 4],
    pub blend_weights: [f32; 4],
    pub color: [f32; 4],
    /// UV and weight index for the authored second terrain layer.
    pub blend_tex_coords: [f32; 2],
    pub blend_texture_slot: u32,
}

impl TerrainVertex {
    /// Create a vertex from strongly typed components.
    pub fn from_components(
        position: Vec3,
        normal: Vec3,
        tex_coords: (f32, f32),
        blend_indices: [u16; 4],
        blend_weights: [f32; 4],
        color: [f32; 4],
    ) -> Self {
        Self {
            position: [position.x, position.y, position.z],
            normal: [normal.x, normal.y, normal.z],
            tex_coords: [tex_coords.0, tex_coords.1],
            blend_indices,
            blend_weights,
            color,
            blend_tex_coords: [tex_coords.0, tex_coords.1],
            blend_texture_slot: u32::MAX,
        }
    }

    /// Convenience accessor for the position as a glam vector.
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.position[0], self.position[1], self.position[2])
    }

    /// Convenience accessor for the normal as a glam vector.
    pub fn normal(&self) -> Vec3 {
        Vec3::new(self.normal[0], self.normal[1], self.normal[2])
    }

    /// Update the position using a glam vector.
    pub fn set_position(&mut self, position: Vec3) {
        self.position = [position.x, position.y, position.z];
    }

    /// Update the normal using a glam vector.
    pub fn set_normal(&mut self, normal: Vec3) {
        self.normal = [normal.x, normal.y, normal.z];
    }

    /// Update the base UV coordinates.
    pub fn set_tex_coords(&mut self, u: f32, v: f32) {
        self.tex_coords = [u, v];
    }

    /// Update the blend indices for texture array sampling.
    pub fn set_blend_indices(&mut self, indices: [u16; 4]) {
        self.blend_indices = indices;
    }

    /// GPU vertex buffer layout descriptor for the live terrain shader.
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem::size_of;

        wgpu::VertexBufferLayout {
            array_stride: size_of::<TerrainVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint16x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 14]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 18]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 20]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}
