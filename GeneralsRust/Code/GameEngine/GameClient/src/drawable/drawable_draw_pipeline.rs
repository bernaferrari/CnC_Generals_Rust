//! Drawable Draw Pipeline — bridges DrawSubmissions from the RenderBridge into wgpu draw calls.
//!
//! Follows the same pattern as `effects/particle_renderer.rs`:
//! - Owns wgpu render pipelines (opaque + transparent)
//! - Owns camera and per-object uniform buffers + bind groups
//! - Maintains a mesh cache mapping model names to vertex/index buffers
//!
//! Usage: `DrawableDrawPipeline::record_draw()` is called from
//! `DrawableManager::render_pass_through()` after the drawable iteration loop
//! populates the RenderBridge with DrawSubmissions.

use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use wgpu::util::DeviceExt;

use crate::render_bridge::{self, get_render_bridge, DrainedDrawSubmission, DrawableId};

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
    pub color: [f32; 4],
}

impl MeshVertex {
    fn new(pos: [f32; 3], nrm: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            position: pos,
            normal: nrm,
            uv,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_render_state_keeps_drawable_texture_visible() {
        assert_eq!(
            object_color_tint(&render_bridge::RenderStateOverrides::default()),
            [1.0, 1.0, 1.0, 1.0]
        );
    }

    #[test]
    fn render_state_tint_combines_construction_damage_and_emissive() {
        let mut state = render_bridge::RenderStateOverrides::default();
        state.construction_tint = Some([0.5, 0.6, 0.7]);
        state.damage_overlay = 0.5;
        state.emissive_tint = [0.1, 0.0, 0.2];

        let tint = object_color_tint(&state);
        assert!((tint[0] - 0.5125).abs() < 0.0001);
        assert!((tint[1] - 0.495).abs() < 0.0001);
        assert!((tint[2] - 0.7775).abs() < 0.0001);
        assert_eq!(tint[3], 1.0);
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
    texture_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Pipeline singleton
// ---------------------------------------------------------------------------

static DRAWABLE_PIPELINE: OnceLock<Arc<Mutex<DrawableDrawPipeline>>> = OnceLock::new();

fn object_color_tint(state: &render_bridge::RenderStateOverrides) -> [f32; 4] {
    let mut rgb = state.construction_tint.unwrap_or([1.0, 1.0, 1.0]);

    if state.apply_night_map {
        rgb = [rgb[0] * 0.82, rgb[1] * 0.88, rgb[2]];
    }
    if state.apply_snow_map {
        rgb = [
            (rgb[0] * 1.08).min(1.0),
            (rgb[1] * 1.08).min(1.0),
            (rgb[2] * 1.12).min(1.0),
        ];
    }
    if state.damage_overlay > 0.0 {
        let damage_scale = (1.0 - state.damage_overlay.clamp(0.0, 1.0) * 0.35).max(0.0);
        rgb = [
            rgb[0] * damage_scale,
            rgb[1] * damage_scale,
            rgb[2] * damage_scale,
        ];
    }

    rgb = [
        (rgb[0] + state.emissive_tint[0]).clamp(0.0, 1.0),
        (rgb[1] + state.emissive_tint[1]).clamp(0.0, 1.0),
        (rgb[2] + state.emissive_tint[2]).clamp(0.0, 1.0),
    ];

    [rgb[0], rgb[1], rgb[2], 1.0]
}

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
    texture_bgl: wgpu::BindGroupLayout,

    // Mesh cache
    mesh_cache: HashMap<String, MeshBuffers>,
    texture_bind_groups: HashMap<String, wgpu::BindGroup>,
    default_texture_bind_group: wgpu::BindGroup,
    texture_sampler: wgpu::Sampler,
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

        // --- Texture bind group ---
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Drawable Texture BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Drawable Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let default_texture = Self::create_solid_texture(&device, &queue, [255, 255, 255, 255]);
        let default_texture_bind_group =
            Self::create_texture_bind_group(&device, &texture_bgl, &sampler, &default_texture);

        // --- Pipeline layout ---
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Drawable Pipeline Layout"),
            bind_group_layouts: &[&camera_bgl, &object_bgl, &texture_bgl],
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
                // color
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 3,
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
            texture_bgl,
            mesh_cache: HashMap::new(),
            texture_bind_groups: HashMap::new(),
            default_texture_bind_group,
            texture_sampler: sampler,
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
            .filter(|drained| !drained.is_transparent)
            .collect();

        if !opaque.is_empty() {
            pass.set_pipeline(&self.opaque_pipeline);
            for drained in &opaque {
                self.draw_submission(pass, drained);
                draw_calls += 1;
            }
        }

        // 4. Transparent pass (back-to-front, already sorted by RenderBridge)
        let transparent: Vec<_> = submissions
            .iter()
            .filter(|drained| drained.is_transparent)
            .collect();

        if !transparent.is_empty() {
            pass.set_pipeline(&self.transparent_pipeline);
            for drained in &transparent {
                self.draw_submission(pass, drained);
                draw_calls += 1;
            }
        }

        let _ = draw_calls; // available for stats if needed
    }

    /// Draw a single DrawSubmission.
    fn draw_submission(&self, pass: &mut wgpu::RenderPass, drained: &DrainedDrawSubmission) {
        let submission = &drained.submission;
        // Update per-object uniform
        let color_tint = object_color_tint(&submission.render_state);

        let object_uniforms = ObjectUniforms {
            world: submission.world_transform.to_cols_array_2d(),
            color_tint,
            opacity: submission.render_state.opacity.clamp(0.0, 1.0),
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

        let key = submission.model_name.to_lowercase();
        let mesh = if let Some(mesh) = self.mesh_cache.get(&key) {
            mesh
        } else {
            return;
        };
        let texture_bind_group = mesh
            .texture_name
            .as_ref()
            .and_then(|name| self.texture_bind_groups.get(&name.to_lowercase()))
            .unwrap_or(&self.default_texture_bind_group);

        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_bind_group(2, texture_bind_group, &[]);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    /// Insert a mesh into the cache. Call during asset loading.
    pub fn insert_mesh(&mut self, model_name: &str, vertices: Vec<MeshVertex>, indices: Vec<u32>) {
        self.insert_mesh_with_texture(model_name, vertices, indices, None);
    }

    pub fn insert_mesh_with_texture(
        &mut self,
        model_name: &str,
        vertices: Vec<MeshVertex>,
        indices: Vec<u32>,
        texture_name: Option<String>,
    ) {
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
                texture_name,
            },
        );
    }

    /// Check if a mesh is already cached.
    pub fn has_mesh(&self, model_name: &str) -> bool {
        self.mesh_cache.contains_key(&model_name.to_lowercase())
    }

    pub fn load_texture(
        &mut self,
        name: &str,
        texture_data: &[u8],
    ) -> Result<(), image::ImageError> {
        if texture_data.is_empty() {
            return Ok(());
        }

        let image = image::load_from_memory(texture_data)?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Drawable Texture: {name}")),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = Self::create_texture_bind_group(
            &self.device,
            &self.texture_bgl,
            &self.texture_sampler,
            &texture,
        );
        self.texture_bind_groups
            .insert(name.to_lowercase(), bind_group);
        Ok(())
    }

    fn create_solid_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: [u8; 4],
    ) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Drawable Default White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &color,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        texture
    }

    fn create_texture_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        texture: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Drawable Texture BG"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}
