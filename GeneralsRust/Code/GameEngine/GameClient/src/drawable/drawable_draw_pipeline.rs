//! Drawable Draw Pipeline — bridges DrawSubmissions from the RenderBridge into wgpu draw calls.
//!
//! Follows the same pattern as `effects/particle_renderer.rs`:
//! - Owns wgpu render pipelines (opaque + transparent)
//! - Owns camera and per-object uniform buffers + bind groups
//! - Maintains a mesh cache mapping model names to vertex/index buffers
//! - Provides a fallback unit cube for models without loaded geometry
//!
//! Usage: `DrawableDrawPipeline::record_draw()` is called from
//! `DrawableManager::render_pass_through()` after the drawable iteration loop
//! populates the RenderBridge with DrawSubmissions.

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use wgpu::util::DeviceExt;

use crate::render_bridge::{self, get_render_bridge, DrawableId};

// ---------------------------------------------------------------------------
// Vertex / Uniform layouts
// ---------------------------------------------------------------------------

/// Vertex format matching `W3DVertex` subset used by the forward pipeline.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl MeshVertex {
    fn new(pos: [f32; 3], nrm: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            position: pos,
            normal: nrm,
            uv: uv,
        }
    }
}

/// Camera uniform buffer (view-projection matrix).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
}

/// Per-object uniform buffer (world transform + tint + opacity).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ObjectUniforms {
    pub world: [[f32; 4]; 4],
    pub color_tint: [f32; 4],
    pub opacity: f32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

// ---------------------------------------------------------------------------
// Mesh buffer cache entry
// ---------------------------------------------------------------------------

struct MeshBuffers {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

// ---------------------------------------------------------------------------
// Pipeline singleton
// ---------------------------------------------------------------------------

static DRAWABLE_PIPELINE: OnceLock<Arc<Mutex<DrawableDrawPipeline>>> = OnceLock::new();

pub fn register_drawable_pipeline(pipeline: Arc<Mutex<DrawableDrawPipeline>>) {
    let _ = DRAWABLE_PIPELINE.set(pipeline);
}

pub fn with_drawable_pipeline<R>(
    f: impl FnOnce(&Arc<Mutex<DrawableDrawPipeline>>) -> R,
) -> Option<R> {
    DRAWABLE_PIPELINE.get().map(f)
}

/// Main pipeline struct for rendering drawable geometry via wgpu.
pub struct DrawableDrawPipeline {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    // Pipelines
    opaque_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,

    // Camera
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    // Per-object (updated before each draw call)
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,

    // Bind group layouts (kept for pipeline creation reference)
    _camera_bgl: wgpu::BindGroupLayout,
    _object_bgl: wgpu::BindGroupLayout,

    // Mesh cache
    mesh_cache: HashMap<String, MeshBuffers>,

    // Fallback unit cube
    fallback_mesh: MeshBuffers,
}

impl DrawableDrawPipeline {
    /// Create the pipeline. Call once at startup with the graphics device/queue.
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // --- Camera bind group ---
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Drawable Camera BGL"),
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

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Drawable Camera UB"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Drawable Camera BG"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // --- Per-object bind group ---
        let object_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Drawable Object BGL"),
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

        let object_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Drawable Object UB"),
            size: std::mem::size_of::<ObjectUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let object_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Drawable Object BG"),
            layout: &object_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: object_buffer.as_entire_binding(),
            }],
        });

        // --- Pipeline layout ---
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Drawable Pipeline Layout"),
            bind_group_layouts: &[&camera_bgl, &object_bgl],
            push_constant_ranges: &[],
        });

        // --- Shaders ---
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Drawable Mesh Vertex"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mesh_vertex.wgsl").into()),
        });

        let opaque_fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Drawable Mesh Opaque"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mesh_opaque.wgsl").into()),
        });

        let transparent_fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Drawable Mesh Transparent"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mesh_transparent.wgsl").into()),
        });

        // --- Vertex buffer layout ---
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
                // uv
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 2,
                },
            ],
        };

        let depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        // --- Opaque pipeline ---
        let opaque_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Drawable Opaque Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &opaque_fragment,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None, // opaque, no blending
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(depth_stencil.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- Transparent pipeline ---
        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Drawable Transparent Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &transparent_fragment,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // no cull for transparent
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                depth_write_enabled: false, // transparent objects don't write depth
                ..depth_stencil
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- Fallback unit cube ---
        let fallback_mesh = Self::create_unit_cube(&device);

        Ok(Self {
            device,
            queue,
            opaque_pipeline,
            transparent_pipeline,
            camera_buffer,
            camera_bind_group,
            object_buffer,
            object_bind_group,
            _camera_bgl: camera_bgl,
            _object_bgl: object_bgl,
            mesh_cache: HashMap::new(),
            fallback_mesh,
        })
    }

    /// Update the camera uniform (view-projection matrix).
    pub fn update_camera(&self, view: &glam::Mat4, proj: &glam::Mat4) {
        let view_proj = (*proj * *view).to_cols_array_2d();
        let uniforms = CameraUniforms { view_proj };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Flush the global RenderBridge, drain culled/sorted submissions, and record
    /// draw calls into the given wgpu RenderPass.
    ///
    /// This is the main entry point called from `DrawableManager::render_pass_through()`.
    pub fn record_draw(&mut self, pass: &mut wgpu::RenderPass) {
        // 1. Flush the bridge (cull + sort + partition)
        let submissions = {
            let mut guard = get_render_bridge()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match guard.as_mut() {
                Some(bridge) => {
                    bridge.flush();
                    bridge.drain_scene_submissions()
                }
                None => return,
            }
        };

        if submissions.is_empty() {
            return;
        }

        // 2. Set the camera bind group (shared for all draws)
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, &self.object_bind_group, &[]);

        let mut draw_calls = 0u32;

        // 3. Opaque pass (front-to-back, already sorted by RenderBridge)
        let opaque: Vec<_> = submissions
            .iter()
            .filter(|(_, is_transparent)| !is_transparent)
            .collect();

        if !opaque.is_empty() {
            pass.set_pipeline(&self.opaque_pipeline);
            for (submission, _) in &opaque {
                self.draw_submission(pass, submission);
                draw_calls += 1;
            }
        }

        // 4. Transparent pass (back-to-front, already sorted by RenderBridge)
        let transparent: Vec<_> = submissions
            .iter()
            .filter(|(_, is_transparent)| *is_transparent)
            .collect();

        if !transparent.is_empty() {
            pass.set_pipeline(&self.transparent_pipeline);
            for (submission, _) in &transparent {
                self.draw_submission(pass, submission);
                draw_calls += 1;
            }
        }

        let _ = draw_calls; // available for stats if needed
    }

    /// Draw a single DrawSubmission.
    fn draw_submission(
        &self,
        pass: &mut wgpu::RenderPass,
        submission: &render_bridge::DrawSubmission,
    ) {
        // Update per-object uniform
        let color_tint = submission.render_state.emissive_tint;
        let opacity = submission.render_state.opacity;

        let object_uniforms = ObjectUniforms {
            world: submission.world_transform.to_cols_array_2d(),
            color_tint: [color_tint[0], color_tint[1], color_tint[2], 1.0],
            opacity,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        self.queue.write_buffer(
            &self.object_buffer,
            0,
            bytemuck::cast_slice(&[object_uniforms]),
        );

        // TODO: We should wait for the uniform write to complete via a submission.
        // For now this works because the queue writes are ordered relative to submits.

        // Look up mesh buffers (use fallback if model not loaded)
        let mesh = self
            .mesh_cache
            .get(&submission.model_name.to_lowercase())
            .unwrap_or(&self.fallback_mesh);

        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    /// Insert a mesh into the cache. Call during asset loading.
    pub fn insert_mesh(&mut self, model_name: &str, vertices: Vec<MeshVertex>, indices: Vec<u32>) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        let key = model_name.to_lowercase();
        if self.mesh_cache.contains_key(&key) {
            return;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Drawable Mesh VB: {}", model_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Drawable Mesh IB: {}", model_name)),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        self.mesh_cache.insert(
            key,
            MeshBuffers {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
            },
        );
    }

    /// Check if a mesh is already cached.
    pub fn has_mesh(&self, model_name: &str) -> bool {
        self.mesh_cache.contains_key(&model_name.to_lowercase())
    }

    /// Create a unit cube (1x1x1 centered at origin) as the fallback mesh.
    fn create_unit_cube(device: &wgpu::Device) -> MeshBuffers {
        // Six faces, each with 4 vertices (24 total) and 6 indices (36 total)
        let vertices = vec![
            // Front face (+Z)
            MeshVertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0], [0.0, 1.0]),
            MeshVertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0], [1.0, 1.0]),
            MeshVertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], [1.0, 0.0]),
            MeshVertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0], [0.0, 0.0]),
            // Back face (-Z)
            MeshVertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 1.0]),
            MeshVertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 1.0]),
            MeshVertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 0.0]),
            MeshVertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 0.0]),
            // Top face (+Y)
            MeshVertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0], [0.0, 1.0]),
            MeshVertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], [1.0, 1.0]),
            MeshVertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0], [1.0, 0.0]),
            MeshVertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0], [0.0, 0.0]),
            // Bottom face (-Y)
            MeshVertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], [0.0, 1.0]),
            MeshVertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0], [1.0, 1.0]),
            MeshVertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0], [1.0, 0.0]),
            MeshVertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0], [0.0, 0.0]),
            // Right face (+X)
            MeshVertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0], [0.0, 1.0]),
            MeshVertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0], [1.0, 1.0]),
            MeshVertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0], [1.0, 0.0]),
            MeshVertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], [0.0, 0.0]),
            // Left face (-X)
            MeshVertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], [0.0, 1.0]),
            MeshVertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0], [1.0, 1.0]),
            MeshVertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0], [1.0, 0.0]),
            MeshVertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0], [0.0, 0.0]),
        ];

        let indices: Vec<u32> = vec![
            0, 1, 2, 0, 2, 3, // front
            4, 5, 6, 4, 6, 7, // back
            8, 9, 10, 8, 10, 11, // top
            12, 13, 14, 12, 14, 15, // bottom
            16, 17, 18, 16, 18, 19, // right
            20, 21, 22, 20, 22, 23, // left
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Drawable Fallback Cube VB"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Drawable Fallback Cube IB"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        MeshBuffers {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}
