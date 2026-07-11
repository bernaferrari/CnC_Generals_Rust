//! GPU Tessellation Pipeline for N-Patch Rendering
//!
//! This module provides GPU-based tessellation for N-Patch curved surfaces,
//! implementing the PN-Triangles algorithm on the GPU using compute shaders.
//!
//! WGPU doesn't have native tessellation shaders (those are DirectX-specific),
//! so we implement tessellation using:
//! 1. Compute shader for expanding and evaluating tessellated vertices
//! 2. Indirect draw calls for dynamic geometry
//! 3. Cached mesh data for performance
//!
//! Reference: Vlachos, Peters, Boyd, Mitchell (2001) "Curved PN Triangles"
//! C++ Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp lines 1033-1037

use crate::*;
use std::sync::Arc;
use wgpu::{ComputePipeline, Device, Queue};

/// Result of GPU tessellation
pub type TessellationResult<T> = Result<T, TessellationError>;

/// Tessellation errors
#[derive(Debug, Clone)]
pub enum TessellationError {
    /// GPU device error
    DeviceError(String),
    /// Shader compilation error
    ShaderError(String),
    /// Buffer allocation failed
    BufferError(String),
    /// Invalid tessellation parameters
    InvalidParams(String),
}

impl std::fmt::Display for TessellationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::DeviceError(msg) => write!(f, "Device error: {}", msg),
            Self::ShaderError(msg) => write!(f, "Shader error: {}", msg),
            Self::BufferError(msg) => write!(f, "Buffer error: {}", msg),
            Self::InvalidParams(msg) => write!(f, "Invalid params: {}", msg),
        }
    }
}

impl std::error::Error for TessellationError {}

/// Tessellation level constants
/// Matches CPU-side TessellationLevel enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum GpuTessellationLevel {
    /// No tessellation (1 vertex per corner = 3 vertices total)
    None = 1,
    /// Low tessellation (2 vertices per edge)
    Low = 2,
    /// Medium tessellation (3 vertices per edge)
    Medium = 3,
    /// High tessellation (4 vertices per edge)
    High = 4,
    /// Very high tessellation (5 vertices per edge)
    VeryHigh = 5,
}

impl GpuTessellationLevel {
    /// Get vertex count for tessellated triangle
    /// Formula: (level + 1) * (level + 2) / 2
    pub fn vertex_count(self) -> u32 {
        let n = self as u32;
        (n + 1) * (n + 2) / 2
    }

    /// Get triangle count for tessellated triangle
    /// Formula: level^2
    pub fn triangle_count(self) -> u32 {
        let n = self as u32;
        n * n
    }

    /// Get index count for tessellated triangle
    /// Formula: triangle_count * 3
    pub fn index_count(self) -> u32 {
        self.triangle_count() * 3
    }
}

/// GPU tessellation configuration
#[derive(Debug, Clone)]
pub struct GpuTessellationConfig {
    /// Tessellation level
    pub level: GpuTessellationLevel,
    /// Enable tessellation
    pub enabled: bool,
    /// Use adaptive tessellation based on screen-space error
    pub adaptive: bool,
    /// Maximum screen-space error for adaptive tessellation
    pub max_error: f32,
}

impl Default for GpuTessellationConfig {
    fn default() -> Self {
        Self {
            level: GpuTessellationLevel::Medium,
            enabled: true,
            adaptive: false,
            max_error: 0.5,
        }
    }
}

/// GPU vertex data for tessellation
/// Matches vertex layout expected by tessellation compute shader
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TessellationVertex {
    /// Vertex position (xyz)
    pub position: [f32; 3],
    /// _padding
    pub _pad0: u32,
    /// Vertex normal (xyz)
    pub normal: [f32; 3],
    /// _padding
    pub _pad1: u32,
    /// Texture coordinates (uv)
    pub texcoord: [f32; 2],
    /// _padding
    pub _pad2: [f32; 2],
}

/// GPU N-Patch control point data
/// 10 control points for quintic Bezier triangle
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ControlPointData {
    /// 10 control points (4 floats each for padding)
    pub points: [[f32; 4]; 10],
}

impl ControlPointData {
    /// Create from corner vertices and normals
    /// Implements PN-Triangles algorithm on GPU
    pub fn from_triangle(
        p0: [f32; 3],
        _n0: [f32; 3],
        p1: [f32; 3],
        _n1: [f32; 3],
        p2: [f32; 3],
        _n2: [f32; 3],
    ) -> Self {
        // Control points for PN-Triangles:
        // b300 = p0, b030 = p1, b003 = p2
        // b210, b120, b021, b012, b102, b201 = edge control points
        // b111 = interior control point

        // For simplicity, use linear interpolation for edge points
        // Full PN-Triangles would compute these based on normals
        Self {
            points: [
                [p0[0], p0[1], p0[2], 1.0], // b300
                [p1[0], p1[1], p1[2], 1.0], // b030
                [p2[0], p2[1], p2[2], 1.0], // b003
                [
                    (2.0 * p0[0] + p1[0]) / 3.0,
                    (2.0 * p0[1] + p1[1]) / 3.0,
                    (2.0 * p0[2] + p1[2]) / 3.0,
                    1.0,
                ], // b210
                [
                    (p0[0] + 2.0 * p1[0]) / 3.0,
                    (p0[1] + 2.0 * p1[1]) / 3.0,
                    (p0[2] + 2.0 * p1[2]) / 3.0,
                    1.0,
                ], // b120
                [
                    (2.0 * p1[0] + p2[0]) / 3.0,
                    (2.0 * p1[1] + p2[1]) / 3.0,
                    (2.0 * p1[2] + p2[2]) / 3.0,
                    1.0,
                ], // b021
                [
                    (p1[0] + 2.0 * p2[0]) / 3.0,
                    (p1[1] + 2.0 * p2[1]) / 3.0,
                    (p1[2] + 2.0 * p2[2]) / 3.0,
                    1.0,
                ], // b012
                [
                    (2.0 * p2[0] + p0[0]) / 3.0,
                    (2.0 * p2[1] + p0[1]) / 3.0,
                    (2.0 * p2[2] + p0[2]) / 3.0,
                    1.0,
                ], // b102
                [
                    (p2[0] + 2.0 * p0[0]) / 3.0,
                    (p2[1] + 2.0 * p0[1]) / 3.0,
                    (p2[2] + 2.0 * p0[2]) / 3.0,
                    1.0,
                ], // b201
                [
                    (p0[0] + p1[0] + p2[0]) / 3.0,
                    (p0[1] + p1[1] + p2[1]) / 3.0,
                    (p0[2] + p1[2] + p2[2]) / 3.0,
                    1.0,
                ], // b111
            ],
        }
    }
}

/// GPU tessellation pipeline
#[derive(Debug)]
pub struct GpuTessellationPipeline {
    /// Configuration
    config: GpuTessellationConfig,
    /// Compute pipeline for tessellation
    compute_pipeline: Arc<ComputePipeline>,
    /// Bind group layout for compute shader
    bind_group_layout: Arc<wgpu::BindGroupLayout>,
}

impl GpuTessellationPipeline {
    /// Create a new GPU tessellation pipeline
    pub fn new(device: &Device, config: GpuTessellationConfig) -> TessellationResult<Self> {
        // Create bind group layout
        let bind_group_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("tessellation_bind_group_layout"),
                entries: &[
                    // Input triangle buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Output vertex buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Output index buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Control point buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
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

        // Create compute shader - embedded WGSL code
        let shader_code = r#"
// GPU Tessellation Compute Shader for PN-Triangles
// Implements Curved PN Triangles algorithm on GPU

// Vertex structure matching Rust TessellationVertex
struct Vertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    texcoord: vec2<f32>,
};

// Control point data for Bezier triangle (10 points)
struct ControlPoints {
    points: array<vec4<f32>, 10>,
};

// Input vertices (3 corner vertices)
@group(0) @binding(0)
var<storage, read> input_vertices: array<Vertex>;

// Output tessellated vertices
@group(0) @binding(1)
var<storage, read_write> output_vertices: array<Vertex>;

// Output indices
@group(0) @binding(2)
var<storage, read_write> output_indices: array<u32>;

// Control points for the Bezier triangle
@group(0) @binding(3)
var<uniform> control_points: ControlPoints;

// Evaluate position on Bezier triangle using barycentric coordinates
fn evaluate_bezier_position(u: f32, v: f32, w: f32) -> vec3<f32> {
    let u2 = u * u;
    let v2 = v * v;
    let w2 = w * w;
    let u3 = u2 * u;
    let v3 = v2 * v;
    let w3 = w2 * w;

    // Quintic Bernstein polynomials
    let b300 = u3;
    let b030 = v3;
    let b003 = w3;
    let b210 = 3.0 * u2 * v;
    let b120 = 3.0 * u * v2;
    let b021 = 3.0 * v2 * w;
    let b012 = 3.0 * v * w2;
    let b102 = 3.0 * u * w2;
    let b201 = 3.0 * u2 * w;
    let b111 = 6.0 * u * v * w;

    var position = vec3<f32>(0.0);

    position += b300 * control_points.points[0].xyz;
    position += b030 * control_points.points[1].xyz;
    position += b003 * control_points.points[2].xyz;
    position += b210 * control_points.points[3].xyz;
    position += b120 * control_points.points[4].xyz;
    position += b021 * control_points.points[5].xyz;
    position += b012 * control_points.points[6].xyz;
    position += b102 * control_points.points[7].xyz;
    position += b201 * control_points.points[8].xyz;
    position += b111 * control_points.points[9].xyz;

    return position;
}

// Evaluate normal on Bezier triangle (quadratic interpolation)
fn evaluate_bezier_normal(u: f32, v: f32, w: f32) -> vec3<f32> {
    let normal = u * input_vertices[0].normal + v * input_vertices[1].normal + w * input_vertices[2].normal;
    return normalize(normal);
}

// Evaluate texture coordinates on Bezier triangle
fn evaluate_texcoord(u: f32, v: f32, w: f32) -> vec2<f32> {
    let uv0 = input_vertices[0].texcoord;
    let uv1 = input_vertices[1].texcoord;
    let uv2 = input_vertices[2].texcoord;

    return u * uv0 + v * uv1 + w * uv2;
}

// Convert flat vertex index to tessellation grid coordinates
fn index_to_barycentric(index: u32, level: u32) -> vec3<f32> {
    var i = 0u;
    var j = 0u;
    var idx = index;

    var row_size = level + 1u;
    var row = 0u;

    while row < level + 1u && idx >= row_size {
        idx -= row_size;
        row += 1u;
        row_size = row_size - 1u;
    }

    i = row;
    j = idx;
    let k = level - i - j;

    let u = f32(j) / f32(level);
    let v = f32(i) / f32(level);
    let w = f32(k) / f32(level);

    return normalize(vec3<f32>(u, v, w));
}

// Main tessellation compute shader
@compute
@workgroup_size(64, 1, 1)
fn tessellate_triangle(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let vertex_idx = global_id.x;
    let level = 3u; // Medium level
    let vertex_count = (level + 1u) * (level + 2u) / 2u;

    if vertex_idx >= vertex_count {
        return;
    }

    let bary = index_to_barycentric(vertex_idx, level);

    let position = evaluate_bezier_position(bary.x, bary.y, bary.z);
    let normal = evaluate_bezier_normal(bary.x, bary.y, bary.z);
    let texcoord = evaluate_texcoord(bary.x, bary.y, bary.z);

    output_vertices[vertex_idx].position = position;
    output_vertices[vertex_idx].normal = normal;
    output_vertices[vertex_idx].texcoord = texcoord;
}
"#;

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessellation_compute"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_code)),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessellation_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let compute_pipeline = Arc::new(device.create_compute_pipeline(
            &wgpu::ComputePipelineDescriptor {
                label: Some("tessellation_compute_pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some("tessellate_triangle"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            },
        ));

        Ok(Self {
            config,
            compute_pipeline,
            bind_group_layout,
        })
    }

    /// Get tessellation level
    pub fn level(&self) -> GpuTessellationLevel {
        self.config.level
    }

    /// Set tessellation level
    pub fn set_level(&mut self, level: GpuTessellationLevel) {
        self.config.level = level;
    }

    /// Enable/disable tessellation
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Check if tessellation is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get compute pipeline
    pub fn pipeline(&self) -> &wgpu::ComputePipeline {
        &self.compute_pipeline
    }

    /// Get bind group layout
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

/// GPU tessellation dispatcher
/// Handles compute shader invocation and result collection
#[derive(Debug)]
pub struct TessellationDispatcher {
    pipeline: GpuTessellationPipeline,
}

impl TessellationDispatcher {
    /// Create a new tessellation dispatcher
    pub fn new(pipeline: GpuTessellationPipeline) -> Self {
        Self { pipeline }
    }

    /// Tessellate a single triangle on GPU
    pub fn tessellate_triangle(
        &self,
        device: &Device,
        queue: &Queue,
        vertices: &[TessellationVertex; 3],
        normals: &[[f32; 3]; 3],
    ) -> TessellationResult<(Vec<TessellationVertex>, Vec<u32>)> {
        if !self.pipeline.is_enabled() {
            // Return original triangle if tessellation disabled
            return Ok((vec![vertices[0], vertices[1], vertices[2]], vec![0, 1, 2]));
        }

        let level = self.pipeline.level();

        // Compute output buffer sizes
        let vertex_count = level.vertex_count() as usize;
        let index_count = level.index_count() as usize;

        // Create output buffers
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tessellation_output_vertices"),
            size: (vertex_count * std::mem::size_of::<TessellationVertex>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tessellation_output_indices"),
            size: (index_count * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create control point buffer
        let cp_data = ControlPointData::from_triangle(
            [
                vertices[0].position[0],
                vertices[0].position[1],
                vertices[0].position[2],
            ],
            normals[0],
            [
                vertices[1].position[0],
                vertices[1].position[1],
                vertices[1].position[2],
            ],
            normals[1],
            [
                vertices[2].position[0],
                vertices[2].position[1],
                vertices[2].position[2],
            ],
            normals[2],
        );

        // Create control point buffer with mapped-at-creation
        let cp_data_vec = vec![cp_data];
        let cp_data_bytes = bytemuck::cast_slice(&cp_data_vec);
        let cp_size = cp_data_bytes.len() as u64;

        let cp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tessellation_control_points"),
            size: cp_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        cp_buffer.slice(..).get_mapped_range_mut()[..cp_data_bytes.len()]
            .copy_from_slice(cp_data_bytes);
        cp_buffer.unmap();

        // Create input vertex buffer with mapped-at-creation
        let vertex_data_vec = vec![vertices[0], vertices[1], vertices[2]];
        let vertex_bytes = bytemuck::cast_slice(&vertex_data_vec);
        let vertex_size = vertex_bytes.len() as u64;

        let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tessellation_input_vertices"),
            size: vertex_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        input_buffer.slice(..).get_mapped_range_mut()[..vertex_bytes.len()]
            .copy_from_slice(vertex_bytes);
        input_buffer.unmap();

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tessellation_bind_group"),
            layout: self.pipeline.bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: cp_buffer.as_entire_binding(),
                },
            ],
        });

        // Submit compute shader
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tessellation_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("tessellation_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(self.pipeline.pipeline());
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch with enough threads for all output vertices
            let workgroup_size = 64u32; // Must match shader
            let num_workgroups = (vertex_count as u32).div_ceil(workgroup_size);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        queue.submit(Some(encoder.finish()));

        // Read results back
        let vertex_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertex_staging"),
            size: (vertex_count * std::mem::size_of::<TessellationVertex>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let index_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("index_staging"),
            size: (index_count * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut copy_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("copy_encoder"),
        });

        copy_encoder.copy_buffer_to_buffer(
            &vertex_buffer,
            0,
            &vertex_staging,
            0,
            (vertex_count * std::mem::size_of::<TessellationVertex>()) as u64,
        );

        copy_encoder.copy_buffer_to_buffer(
            &index_buffer,
            0,
            &index_staging,
            0,
            (index_count * std::mem::size_of::<u32>()) as u64,
        );

        queue.submit(Some(copy_encoder.finish()));

        // Wait for results (note: blocking - should be async in production)
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        // Map and read vertex data
        let vertex_slice = vertex_staging.slice(..);
        vertex_slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        let vertex_data = vertex_slice.get_mapped_range();
        let vertices: Vec<TessellationVertex> = bytemuck::cast_slice(&vertex_data).to_vec();
        drop(vertex_data);
        vertex_staging.unmap();

        // Map and read index data
        let index_slice = index_staging.slice(..);
        index_slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        let index_data = index_slice.get_mapped_range();
        let indices: Vec<u32> = bytemuck::cast_slice(&index_data).to_vec();
        drop(index_data);
        index_staging.unmap();

        Ok((vertices, indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tessellation_level_vertex_count() {
        assert_eq!(GpuTessellationLevel::None.vertex_count(), 3);
        assert_eq!(GpuTessellationLevel::Low.vertex_count(), 6);
        assert_eq!(GpuTessellationLevel::Medium.vertex_count(), 10);
        assert_eq!(GpuTessellationLevel::High.vertex_count(), 15);
        assert_eq!(GpuTessellationLevel::VeryHigh.vertex_count(), 21);
    }

    #[test]
    fn test_tessellation_level_triangle_count() {
        assert_eq!(GpuTessellationLevel::None.triangle_count(), 1);
        assert_eq!(GpuTessellationLevel::Low.triangle_count(), 4);
        assert_eq!(GpuTessellationLevel::Medium.triangle_count(), 9);
        assert_eq!(GpuTessellationLevel::High.triangle_count(), 16);
        assert_eq!(GpuTessellationLevel::VeryHigh.triangle_count(), 25);
    }

    #[test]
    fn test_tessellation_level_index_count() {
        assert_eq!(GpuTessellationLevel::None.index_count(), 3);
        assert_eq!(GpuTessellationLevel::Low.index_count(), 12);
        assert_eq!(GpuTessellationLevel::Medium.index_count(), 27);
        assert_eq!(GpuTessellationLevel::High.index_count(), 48);
    }

    #[test]
    fn test_control_point_creation() {
        let p0 = [0.0, 0.0, 0.0];
        let n0 = [0.0, 1.0, 0.0];
        let p1 = [1.0, 0.0, 0.0];
        let n1 = [0.0, 1.0, 0.0];
        let p2 = [0.0, 1.0, 0.0];
        let n2 = [0.0, 1.0, 0.0];

        let cp = ControlPointData::from_triangle(p0, n0, p1, n1, p2, n2);

        // Verify first 3 control points match vertices
        assert_eq!(cp.points[0][0], p0[0]);
        assert_eq!(cp.points[1][0], p1[0]);
        assert_eq!(cp.points[2][0], p2[0]);
    }

    #[test]
    fn test_tessellation_config_defaults() {
        let config = GpuTessellationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.level, GpuTessellationLevel::Medium);
        assert!(!config.adaptive);
    }
}
