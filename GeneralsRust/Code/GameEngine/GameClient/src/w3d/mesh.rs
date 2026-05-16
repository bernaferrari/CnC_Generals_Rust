//! W3D Mesh Management System

use super::{W3DConfig, W3DResult};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use thiserror::Error;
use ultraviolet::{Vec2, Vec3};
use wgpu::{util::DeviceExt, Buffer, BufferUsages, Device};

#[derive(Error, Debug)]
pub enum W3DMeshError {
    #[error("Mesh creation failed: {0}")]
    CreationFailed(String),
    #[error("Invalid mesh data: {0}")]
    InvalidData(String),
}

/// W3D Vertex data
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct W3DVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

/// W3D Mesh
pub struct W3DMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_count: u32,
    index_count: u32,
    material_id: Option<u32>,
}

pub struct W3DMeshBuilder {
    vertices: Vec<W3DVertex>,
    indices: Vec<u32>,
}

impl W3DMeshBuilder {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn add_vertex(&mut self, vertex: W3DVertex) {
        self.vertices.push(vertex);
    }

    pub fn add_triangle(&mut self, v0: u32, v1: u32, v2: u32) {
        self.indices.extend_from_slice(&[v0, v1, v2]);
    }

    fn validate(&self) -> W3DResult<()> {
        if self.vertices.is_empty() {
            return Err(W3DMeshError::InvalidData("mesh has no vertices".to_string()).into());
        }
        if self.indices.is_empty() {
            return Err(W3DMeshError::InvalidData("mesh has no indices".to_string()).into());
        }
        Ok(())
    }

    pub fn build(&self, device: &Device) -> W3DResult<W3DMesh> {
        self.validate()?;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("W3D Mesh Vertices"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("W3D Mesh Indices"),
            contents: bytemuck::cast_slice(&self.indices),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        Ok(W3DMesh {
            vertex_buffer,
            index_buffer,
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
            material_id: None,
        })
    }
}

pub struct W3DMeshManager {
    device: Arc<Device>,
    config: W3DConfig,
}

impl W3DMeshManager {
    pub fn new(device: &Device, config: &W3DConfig) -> Self {
        Self {
            device: Arc::new(device.clone()),
            config: config.clone(),
        }
    }

    pub fn begin_frame(&mut self, _frame_index: u64) {
        // Update mesh LOD, streaming, etc.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex() -> W3DVertex {
        W3DVertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            bone_indices: [0; 4],
            bone_weights: [0.0; 4],
        }
    }

    #[test]
    fn mesh_builder_rejects_empty_geometry_before_gpu_allocation() {
        let builder = W3DMeshBuilder::new();
        assert!(builder.validate().is_err());
    }

    #[test]
    fn mesh_builder_rejects_missing_indices_before_gpu_allocation() {
        let mut builder = W3DMeshBuilder::new();
        builder.add_vertex(vertex());
        assert!(builder.validate().is_err());
    }

    #[test]
    fn mesh_builder_accepts_triangle_geometry() {
        let mut builder = W3DMeshBuilder::new();
        builder.add_vertex(vertex());
        builder.add_vertex(W3DVertex {
            position: [1.0, 0.0, 0.0],
            ..vertex()
        });
        builder.add_vertex(W3DVertex {
            position: [0.0, 1.0, 0.0],
            ..vertex()
        });
        builder.add_triangle(0, 1, 2);
        assert!(builder.validate().is_ok());
    }
}
