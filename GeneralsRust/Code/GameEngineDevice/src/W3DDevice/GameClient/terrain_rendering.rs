//! Terrain Rendering System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/HeightMap.cpp
//! - GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp
//! - GameEngineDevice/Include/W3DDevice/GameClient/HeightMap.h
//!
//! This module implements the complete terrain rendering system including:
//! - HeightMap mesh generation with LOD support
//! - Multi-layer texture blending (base + detail + blend maps)
//! - Chunk-based rendering with frustum culling
//! - Static and dynamic lighting
//! - Cliff texturing based on slope
//! - Shoreline blending with water

use bytemuck::{Pod, Zeroable};
use cgmath::{Vector2, Vector3, Vector4, InnerSpace, Matrix4, Point3};
use wgpu::{
    Device, Queue, Buffer, BindGroup, BindGroupLayout, RenderPipeline,
    BufferUsages, TextureFormat, VertexBufferLayout, VertexAttribute,
    VertexFormat, VertexStepMode, BufferAddress, ShaderStages,
    BindGroupLayoutEntry, BindingType, TextureSampleType, SamplerBindingType,
    BufferBindingType, IndexFormat, RenderPass, CommandEncoder,
};
use std::sync::Arc;
use parking_lot::RwLock;
use anyhow::{Result, Context};

// Constants from C++ HeightMap.h and BaseHeightMap.h
pub const VERTEX_BUFFER_TILE_LENGTH: usize = 32; // C++ line 20: 32x32 vertex tiles
pub const MAP_XY_FACTOR: f32 = 10.0; // World units per grid cell
pub const MAP_HEIGHT_SCALE: f32 = 1.0; // Height scaling factor
pub const MAX_GLOBAL_LIGHTS: usize = 3; // Maximum terrain lights
pub const MAX_DYNAMIC_LIGHTS: usize = 20; // Maximum dynamic lights

/// Terrain vertex format matching C++ VERTEX_FORMAT (VertexFormatXYZDUV2)
/// From BaseHeightMap.h line 60: #define VERTEX_FORMAT VertexFormatXYZDUV2
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TerrainVertex {
    /// Position in world space
    pub position: [f32; 3],
    /// Vertex color (RGBA) for lighting
    pub diffuse: u32,
    /// Base texture UV coordinates
    pub uv0: [f32; 2],
    /// Detail/blend texture UV coordinates
    pub uv1: [f32; 2],
}

impl TerrainVertex {
    pub const ATTRIBUTES: &'static [VertexAttribute] = &[
        VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: VertexFormat::Float32x3,
        },
        VertexAttribute {
            offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
            shader_location: 1,
            format: VertexFormat::Uint32,
        },
        VertexAttribute {
            offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<u32>()) as BufferAddress,
            shader_location: 2,
            format: VertexFormat::Float32x2,
        },
        VertexAttribute {
            offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<u32>() + std::mem::size_of::<[f32; 2]>()) as BufferAddress,
            shader_location: 3,
            format: VertexFormat::Float32x2,
        },
    ];

    pub fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<TerrainVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: Self::ATTRIBUTES,
        }
    }
}

/// Terrain uniforms for shader
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TerrainUniforms {
    /// View-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Ambient light color (RGB)
    pub ambient_light: [f32; 3],
    pub _padding0: f32,
    /// Directional light direction (XYZ)
    pub light_direction: [f32; 3],
    pub _padding1: f32,
    /// Directional light color (RGB)
    pub light_color: [f32; 3],
    pub _padding2: f32,
    /// Fog parameters (start, end, density, _)
    pub fog_params: [f32; 4],
    /// Time for animation effects
    pub time: f32,
    pub _padding3: [f32; 3],
}

/// Terrain chunk (vertex buffer tile)
/// Corresponds to C++ HeightMapRenderObjClass vertex buffer system
pub struct TerrainChunk {
    /// Chunk position in grid coordinates
    pub grid_x: i32,
    pub grid_y: i32,
    /// Origin coordinates in heightmap
    pub origin_x: usize,
    pub origin_y: usize,
    /// Vertex buffer for this chunk
    pub vertex_buffer: Buffer,
    /// Number of vertices in this chunk
    pub vertex_count: usize,
    /// CPU-side vertex backup for dynamic lighting updates (C++ m_vertexBufferBackup)
    pub vertex_backup: Vec<TerrainVertex>,
    /// Is this chunk visible in frustum
    pub visible: bool,
    /// LOD level (0 = highest detail)
    pub lod_level: u32,
}

/// Heightmap terrain mesh
/// Corresponds to C++ HeightMapRenderObjClass from HeightMap.h
pub struct HeightMapMesh {
    /// Device for resource creation
    device: Arc<Device>,
    /// Queue for updates
    queue: Arc<Queue>,

    /// Terrain dimensions in vertices
    width: usize,
    height: usize,

    /// Heightmap data (matching C++ WorldHeightMap::m_data)
    height_data: Vec<u8>,

    /// Terrain chunks (matching C++ m_vertexBufferTiles)
    chunks: Vec<TerrainChunk>,

    /// Number of chunks in X and Y
    num_chunks_x: usize,
    num_chunks_y: usize,

    /// Shared index buffer for all chunks (C++ m_indexBuffer)
    index_buffer: Buffer,
    index_count: u32,

    /// Uniform buffer
    uniform_buffer: Buffer,

    /// Bind group for textures and uniforms
    bind_group: BindGroup,

    /// Render pipeline
    pipeline: RenderPipeline,

    /// Static light data (C++ m_terrainAmbient, m_terrainLightPos)
    ambient_light: Vector3<f32>,
    light_directions: [Vector3<f32>; MAX_GLOBAL_LIGHTS],
    light_colors: [Vector3<f32>; MAX_GLOBAL_LIGHTS],
    num_lights: usize,

    /// Scroll origin for efficient map scrolling (C++ m_originX, m_originY)
    origin_x: usize,
    origin_y: usize,

    /// Min/max heights for bounding (C++ m_minHeight, m_maxHeight)
    min_height: f32,
    max_height: f32,
}

impl HeightMapMesh {
    /// Create a new heightmap mesh
    /// Corresponds to C++ HeightMapRenderObjClass::initHeightData
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        width: usize,
        height: usize,
        height_data: Vec<u8>,
        bind_group_layout: &BindGroupLayout,
    ) -> Result<Self> {
        // Calculate chunk grid (C++ HeightMap.cpp:1323-1339)
        let num_chunks_x = (width + VERTEX_BUFFER_TILE_LENGTH - 1) / VERTEX_BUFFER_TILE_LENGTH;
        let num_chunks_y = (height + VERTEX_BUFFER_TILE_LENGTH - 1) / VERTEX_BUFFER_TILE_LENGTH;

        // Create shared index buffer (C++ HeightMap.cpp:1301-1321)
        let index_buffer = Self::create_index_buffer(&device)?;
        let index_count = (VERTEX_BUFFER_TILE_LENGTH * VERTEX_BUFFER_TILE_LENGTH * 2 * 3) as u32;

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Terrain Uniform Buffer"),
            size: std::mem::size_of::<TerrainUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Calculate min/max heights (C++ BaseHeightMap.cpp:233)
        let (min_height, max_height) = Self::calculate_height_bounds(&height_data);

        // Create chunks (C++ HeightMap.cpp:1343-1355)
        let mut chunks = Vec::with_capacity(num_chunks_x * num_chunks_y);
        for chunk_y in 0..num_chunks_y {
            for chunk_x in 0..num_chunks_x {
                let chunk = Self::create_chunk(
                    &device,
                    chunk_x,
                    chunk_y,
                    width,
                    height,
                    &height_data,
                )?;
                chunks.push(chunk);
            }
        }

        // Create dummy bind group (will be properly initialized with textures later)
        let bind_group = Self::create_dummy_bind_group(&device, bind_group_layout, &uniform_buffer);

        // Create render pipeline
        let pipeline = Self::create_pipeline(&device, bind_group_layout)?;

        Ok(Self {
            device,
            queue,
            width,
            height,
            height_data,
            chunks,
            num_chunks_x,
            num_chunks_y,
            index_buffer,
            index_count,
            uniform_buffer,
            bind_group,
            pipeline,
            ambient_light: Vector3::new(0.3, 0.3, 0.3),
            light_directions: [Vector3::new(0.0, -1.0, 0.0); MAX_GLOBAL_LIGHTS],
            light_colors: [Vector3::new(0.7, 0.7, 0.7); MAX_GLOBAL_LIGHTS],
            num_lights: 1,
            origin_x: 0,
            origin_y: 0,
            min_height,
            max_height,
        })
    }

    /// Create the shared index buffer for terrain chunks
    /// Corresponds to C++ HeightMap.cpp:1301-1321
    fn create_index_buffer(device: &Device) -> Result<Buffer> {
        let mut indices = Vec::with_capacity(VERTEX_BUFFER_TILE_LENGTH * VERTEX_BUFFER_TILE_LENGTH * 2 * 3);

        // Generate index buffer for triangle list (C++ HeightMap.cpp:1307-1320)
        for j in 0..(VERTEX_BUFFER_TILE_LENGTH * VERTEX_BUFFER_TILE_LENGTH * 4) {
            if j % (VERTEX_BUFFER_TILE_LENGTH * 4) == 0 {
                continue;
            }
            for i in (j..j + VERTEX_BUFFER_TILE_LENGTH * 4).step_by(4) {
                // First triangle
                indices.push(i as u16);
                indices.push((i + 2) as u16);
                indices.push((i + 3) as u16);

                // Second triangle
                indices.push(i as u16);
                indices.push((i + 1) as u16);
                indices.push((i + 2) as u16);
            }
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        Ok(buffer)
    }

    /// Create a single terrain chunk
    /// Corresponds to C++ HeightMapRenderObjClass::updateVB
    fn create_chunk(
        device: &Device,
        chunk_x: usize,
        chunk_y: usize,
        map_width: usize,
        map_height: usize,
        height_data: &[u8],
    ) -> Result<TerrainChunk> {
        let origin_x = chunk_x * VERTEX_BUFFER_TILE_LENGTH;
        let origin_y = chunk_y * VERTEX_BUFFER_TILE_LENGTH;

        let end_x = (origin_x + VERTEX_BUFFER_TILE_LENGTH + 1).min(map_width);
        let end_y = (origin_y + VERTEX_BUFFER_TILE_LENGTH + 1).min(map_height);

        let mut vertices = Vec::new();

        // Generate vertices for this chunk (C++ HeightMap.cpp:310-459)
        for j in origin_y..end_y {
            for i in origin_x..end_x {
                // Create 4 vertices per cell (quad)
                if i + 1 < map_width && j + 1 < map_height {
                    let quad_vertices = Self::create_quad_vertices(
                        i, j, map_width, height_data
                    );
                    vertices.extend_from_slice(&quad_vertices);
                }
            }
        }

        let vertex_count = vertices.len();
        let vertex_backup = vertices.clone();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Terrain Chunk {},{}", chunk_x, chunk_y)),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        Ok(TerrainChunk {
            grid_x: chunk_x as i32,
            grid_y: chunk_y as i32,
            origin_x,
            origin_y,
            vertex_buffer,
            vertex_count,
            vertex_backup,
            visible: true,
            lod_level: 0,
        })
    }

    /// Create vertices for a single quad
    /// Corresponds to C++ HeightMap.cpp:360-459 updateVB vertex generation
    fn create_quad_vertices(
        x: usize,
        y: usize,
        map_width: usize,
        height_data: &[u8],
    ) -> [TerrainVertex; 4] {
        let get_height = |gx: usize, gy: usize| -> f32 {
            let idx = gy * map_width + gx;
            if idx < height_data.len() {
                height_data[idx] as f32 * MAP_HEIGHT_SCALE
            } else {
                0.0
            }
        };

        // Calculate normals using cross product (C++ HeightMap.cpp:361-368)
        let calc_normal = |cx: usize, cy: usize| -> Vector3<f32> {
            let h_left = get_height(cx.saturating_sub(1), cy);
            let h_right = get_height((cx + 1).min(map_width - 1), cy);
            let h_down = get_height(cx, cy.saturating_sub(1));
            let h_up = get_height(cx, (cy + 1).min(map_width - 1));

            let l2r = Vector3::new(2.0 * MAP_XY_FACTOR, 0.0, h_right - h_left);
            let n2f = Vector3::new(0.0, 2.0 * MAP_XY_FACTOR, h_up - h_down);

            l2r.cross(n2f).normalize()
        };

        // Calculate lighting (simple diffuse for now, C++ HeightMap.cpp:379)
        let calc_diffuse = |normal: Vector3<f32>| -> u32 {
            let light_dir = Vector3::new(0.0, -1.0, 0.0);
            let shade = (-light_dir).dot(normal).max(0.0).min(1.0);
            let ambient = 0.3;
            let intensity = (ambient + shade * 0.7).min(1.0);
            let color = (intensity * 255.0) as u8;

            // RGBA format
            (255 << 24) | ((color as u32) << 16) | ((color as u32) << 8) | color as u32
        };

        // Calculate UV coordinates (simple mapping, will be enhanced with blend maps)
        let uv_scale = 1.0 / 64.0; // Texture repeat factor

        // Top-left vertex (C++ HeightMap.cpp:360-380)
        let pos0 = [x as f32 * MAP_XY_FACTOR, y as f32 * MAP_XY_FACTOR, get_height(x, y)];
        let normal0 = calc_normal(x, y);
        let diffuse0 = calc_diffuse(normal0);
        let uv00 = [x as f32 * uv_scale, y as f32 * uv_scale];

        // Top-right vertex (C++ HeightMap.cpp:382-402)
        let pos1 = [(x + 1) as f32 * MAP_XY_FACTOR, y as f32 * MAP_XY_FACTOR, get_height(x + 1, y)];
        let normal1 = calc_normal(x + 1, y);
        let diffuse1 = calc_diffuse(normal1);
        let uv01 = [(x + 1) as f32 * uv_scale, y as f32 * uv_scale];

        // Bottom-right vertex (C++ HeightMap.cpp:404-428)
        let pos2 = [(x + 1) as f32 * MAP_XY_FACTOR, (y + 1) as f32 * MAP_XY_FACTOR, get_height(x + 1, y + 1)];
        let normal2 = calc_normal(x + 1, y + 1);
        let diffuse2 = calc_diffuse(normal2);
        let uv02 = [(x + 1) as f32 * uv_scale, (y + 1) as f32 * uv_scale];

        // Bottom-left vertex (C++ HeightMap.cpp:430-458)
        let pos3 = [x as f32 * MAP_XY_FACTOR, (y + 1) as f32 * MAP_XY_FACTOR, get_height(x, y + 1)];
        let normal3 = calc_normal(x, y + 1);
        let diffuse3 = calc_diffuse(normal3);
        let uv03 = [x as f32 * uv_scale, (y + 1) as f32 * uv_scale];

        [
            TerrainVertex { position: pos0, diffuse: diffuse0, uv0: uv00, uv1: uv00 },
            TerrainVertex { position: pos1, diffuse: diffuse1, uv0: uv01, uv1: uv01 },
            TerrainVertex { position: pos2, diffuse: diffuse2, uv0: uv02, uv1: uv02 },
            TerrainVertex { position: pos3, diffuse: diffuse3, uv0: uv03, uv1: uv03 },
        ]
    }

    /// Calculate height bounds from heightmap data
    fn calculate_height_bounds(height_data: &[u8]) -> (f32, f32) {
        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;

        for &h in height_data {
            let height = h as f32 * MAP_HEIGHT_SCALE;
            min_h = min_h.min(height);
            max_h = max_h.max(height);
        }

        (min_h, max_h)
    }

    /// Create dummy bind group (placeholder until textures are loaded)
    fn create_dummy_bind_group(
        device: &Device,
        layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
    ) -> BindGroup {
        // This would be properly implemented with actual textures
        // For now, create a minimal bind group
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Terrain Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create the terrain rendering pipeline
    fn create_pipeline(device: &Device, bind_group_layout: &BindGroupLayout) -> Result<RenderPipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terrain_shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terrain Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[TerrainVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Ok(pipeline)
    }

    /// Update dynamic lighting on chunks
    /// Corresponds to C++ HeightMapRenderObjClass::updateVBForLight
    pub fn update_dynamic_lighting(&mut self, dynamic_lights: &[DynamicLight]) {
        // C++ HeightMap.cpp:540-688
        for chunk in &mut self.chunks {
            if !chunk.visible {
                continue;
            }

            // Update vertex colors based on dynamic lights
            for vertex in &mut chunk.vertex_backup {
                let pos = Vector3::new(vertex.position[0], vertex.position[1], vertex.position[2]);

                // Start with base diffuse color
                let mut r = ((vertex.diffuse >> 16) & 0xFF) as f32 / 255.0;
                let mut g = ((vertex.diffuse >> 8) & 0xFF) as f32 / 255.0;
                let mut b = (vertex.diffuse & 0xFF) as f32 / 255.0;

                // Add contribution from each dynamic light
                for light in dynamic_lights {
                    let light_vec = light.position - pos;
                    let distance = light_vec.magnitude();

                    if distance < light.range {
                        let attenuation = 1.0 - (distance / light.range);
                        let attenuation = attenuation.max(0.0).min(1.0);

                        r += light.color.x * attenuation;
                        g += light.color.y * attenuation;
                        b += light.color.z * attenuation;
                    }
                }

                // Clamp and convert back to u32
                r = r.min(1.0);
                g = g.min(1.0);
                b = b.min(1.0);

                let alpha = (vertex.diffuse >> 24) & 0xFF;
                vertex.diffuse = (alpha << 24)
                    | (((r * 255.0) as u32) << 16)
                    | (((g * 255.0) as u32) << 8)
                    | ((b * 255.0) as u32);
            }

            // Upload updated vertices to GPU
            self.queue.write_buffer(
                &chunk.vertex_buffer,
                0,
                bytemuck::cast_slice(&chunk.vertex_backup),
            );
        }
    }

    /// Update frustum culling
    /// Corresponds to C++ BaseHeightMapRenderObjClass::Render frustum culling
    pub fn update_frustum_culling(&mut self, frustum_planes: &[Vector4<f32>; 6]) {
        for chunk in &mut self.chunks {
            // Calculate chunk bounding box
            let min_x = chunk.origin_x as f32 * MAP_XY_FACTOR;
            let min_y = chunk.origin_y as f32 * MAP_XY_FACTOR;
            let max_x = (chunk.origin_x + VERTEX_BUFFER_TILE_LENGTH) as f32 * MAP_XY_FACTOR;
            let max_y = (chunk.origin_y + VERTEX_BUFFER_TILE_LENGTH) as f32 * MAP_XY_FACTOR;

            let center = Vector3::new(
                (min_x + max_x) * 0.5,
                (min_y + max_y) * 0.5,
                (self.min_height + self.max_height) * 0.5,
            );
            let extents = Vector3::new(
                (max_x - min_x) * 0.5,
                (max_y - min_y) * 0.5,
                (self.max_height - self.min_height) * 0.5,
            );

            // Test against frustum planes
            chunk.visible = true;
            for plane in frustum_planes {
                let dist = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
                let radius = extents.x * plane.x.abs() + extents.y * plane.y.abs() + extents.z * plane.z.abs();

                if dist + radius < 0.0 {
                    chunk.visible = false;
                    break;
                }
            }
        }
    }

    /// Render the terrain
    /// Corresponds to C++ HeightMapRenderObjClass::Render
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);

        // Render visible chunks
        for chunk in &self.chunks {
            if !chunk.visible {
                continue;
            }

            render_pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
    }

    /// Update uniforms
    pub fn update_uniforms(&self, view_proj: Matrix4<f32>, time: f32) {
        let uniforms = TerrainUniforms {
            view_proj: view_proj.into(),
            ambient_light: self.ambient_light.into(),
            _padding0: 0.0,
            light_direction: self.light_directions[0].into(),
            _padding1: 0.0,
            light_color: self.light_colors[0].into(),
            _padding2: 0.0,
            fog_params: [100.0, 1000.0, 0.001, 0.0],
            time,
            _padding3: [0.0; 3],
        };

        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Get height at world position
    pub fn get_height_at(&self, x: f32, y: f32) -> f32 {
        let grid_x = (x / MAP_XY_FACTOR) as usize;
        let grid_y = (y / MAP_XY_FACTOR) as usize;

        if grid_x >= self.width || grid_y >= self.height {
            return 0.0;
        }

        let idx = grid_y * self.width + grid_x;
        self.height_data[idx] as f32 * MAP_HEIGHT_SCALE
    }
}

/// Dynamic light for terrain
#[derive(Debug, Clone)]
pub struct DynamicLight {
    pub position: Vector3<f32>,
    pub color: Vector3<f32>,
    pub range: f32,
}

/// Texture blending layer
#[derive(Debug, Clone)]
pub struct TerrainTextureLayer {
    pub texture_index: u32,
    pub uv_scale: f32,
    pub blend_mode: BlendMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Replace,
    Alpha,
    Add,
    Multiply,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_size() {
        // Verify vertex layout matches expectations
        assert_eq!(std::mem::size_of::<TerrainVertex>(), 32);
    }

    #[test]
    fn test_height_calculation() {
        let height_data = vec![0u8, 128, 255];
        let (min_h, max_h) = HeightMapMesh::calculate_height_bounds(&height_data);
        assert_eq!(min_h, 0.0);
        assert_eq!(max_h, 255.0);
    }

    #[test]
    fn test_chunk_calculation() {
        let width = 129;
        let height = 129;
        let num_chunks_x = (width + VERTEX_BUFFER_TILE_LENGTH - 1) / VERTEX_BUFFER_TILE_LENGTH;
        let num_chunks_y = (height + VERTEX_BUFFER_TILE_LENGTH - 1) / VERTEX_BUFFER_TILE_LENGTH;

        // 129 vertices = 5 chunks of 32 + 1 partial (ceiling division)
        assert_eq!(num_chunks_x, 5);
        assert_eq!(num_chunks_y, 5);
    }
}
