//! WGPU Renderer - Drop-in replacement for the DirectX renderer
//!
//! This renderer implements the same interface as the existing renderer
//! but uses WGPU for cross-platform compatibility.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::*;
use ww3d_gpu::present_surface_texture;
use winit::{event_loop::EventLoop, window::Window};

use crate::{
    interface::RenderInfo,
    loader::MeshGeometry,
    mesh::ShdMesh,
    renderer::{RenderNode, ShaderRenderer},
    wgpu_backend::{create_wgpu_shader_interface, WgpuShaderInterface, WgpuVertex},
    ShdError, ShdInterface, ShdResult,
};

/// WGPU-based renderer that maintains the same API as the DirectX renderer
pub struct WgpuRenderer {
    // WGPU core objects
    instance: Instance,
    device: Arc<Device>,
    queue: Arc<Queue>,
    adapter: Adapter,

    // Surface for rendering (optional - for windowed rendering)
    surface: Option<Surface<'static>>,
    surface_config: Option<SurfaceConfiguration>,

    // Rendering state
    is_initialized: bool,
    mesh_containers: HashMap<u32, WgpuMeshContainer>,
    current_transform: glam::Mat4,
    current_light_environment: LightEnvironment,

    // Resource management
    vertex_buffers: HashMap<u64, Buffer>,
    index_buffers: HashMap<u64, Buffer>,

    // Frame resources
    depth_texture: Option<TextureView>,
    msaa_framebuffer: Option<TextureView>,
}

/// WGPU mesh container - equivalent to the DirectX mesh container
#[derive(Debug)]
struct WgpuMeshContainer {
    class_id: u32,
    render_nodes: Vec<Arc<Mutex<WgpuRenderNode>>>,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
}

/// WGPU render node - implements the same RenderNode trait
#[derive(Debug)]
pub struct WgpuRenderNode {
    mesh: Arc<MeshGeometry>,
    shader: Arc<Mutex<dyn ShdInterface>>,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    is_visible: bool,
}

#[derive(Clone, Debug)]
pub struct LightEnvironment {
    pub ambient: glam::Vec3,
    pub lights: Vec<Light>,
}

#[derive(Clone, Debug)]
pub struct Light {
    pub position: glam::Vec3,
    pub color: glam::Vec3,
    pub intensity: f32,
    pub direction: glam::Vec3,
}

impl WgpuRenderer {
    /// Create a new WGPU renderer (headless mode)
    pub async fn new() -> ShdResult<Self> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                ShdError::HardwareUnsupported("No suitable adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("WGPU Device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| {
                ShdError::HardwareUnsupported(format!("Failed to create device: {}", e))
            })?;

        Ok(Self {
            instance,
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter,
            surface: None,
            surface_config: None,
            is_initialized: false,
            mesh_containers: HashMap::new(),
            current_transform: glam::Mat4::IDENTITY,
            current_light_environment: LightEnvironment {
                ambient: glam::Vec3::new(0.1, 0.1, 0.1),
                lights: Vec::new(),
            },
            vertex_buffers: HashMap::new(),
            index_buffers: HashMap::new(),
            depth_texture: None,
            msaa_framebuffer: None,
        })
    }

    /// Create a new WGPU renderer with window surface
    pub async fn new_with_window(window: Arc<Window>) -> ShdResult<Self> {
        let mut renderer = Self::new().await?;
        renderer.setup_surface(window)?;
        Ok(renderer)
    }

    /// Setup rendering surface for windowed rendering
    pub fn setup_surface(&mut self, window: Arc<Window>) -> ShdResult<()> {
        let surface = self.instance.create_surface(window).map_err(|e| {
            ShdError::HardwareUnsupported(format!("Failed to create surface: {}", e))
        })?;

        let surface_caps = surface.get_capabilities(&self.adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);

        self.create_depth_texture(size.width, size.height)?;

        Ok(())
    }

    fn create_depth_texture(&mut self, width: u32, height: u32) -> ShdResult<()> {
        let depth_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("Depth Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        self.depth_texture = Some(depth_texture.create_view(&TextureViewDescriptor::default()));
        Ok(())
    }

    /// Render a frame (for windowed rendering)
    pub fn render_frame(&mut self) -> ShdResult<()> {
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| ShdError::InvalidConfig("No surface configured".to_string()))?;

        let output = surface
            .get_current_texture()
            .map_err(|e| ShdError::GraphicsApi(format!("Failed to get surface texture: {}", e)))?;

        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: self.depth_texture.as_ref().map(|depth| {
                    RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(Operations {
                            load: LoadOp::Clear(1.0),
                            store: StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render all mesh containers
            self.render_pass(&mut render_pass)?;
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        present_surface_texture(output);

        Ok(())
    }

    fn render_pass(&mut self, render_pass: &mut RenderPass) -> ShdResult<()> {
        // Sort containers by class ID for consistent rendering order
        let mut class_ids: Vec<u32> = self.mesh_containers.keys().copied().collect();
        class_ids.sort();

        for class_id in class_ids {
            if let Some(container) = self.mesh_containers.get_mut(&class_id) {
                self.render_container(render_pass, container)?;
            }
        }

        Ok(())
    }

    fn render_container(
        &self,
        render_pass: &mut RenderPass,
        container: &WgpuMeshContainer,
    ) -> ShdResult<()> {
        // Set vertex and index buffers if available
        if let Some(vertex_buffer) = &container.vertex_buffer {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        }

        if let Some(index_buffer) = &container.index_buffer {
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
        }

        // Render all nodes in this container
        for node_arc in &container.render_nodes {
            let node = node_arc.lock().unwrap();
            if node.is_visible {
                self.render_node(render_pass, &*node)?;
            }
        }

        Ok(())
    }

    fn render_node(&self, render_pass: &mut RenderPass, node: &WgpuRenderNode) -> ShdResult<()> {
        let shader = node.shader.lock().unwrap();

        // Apply shader pipeline if it's a WGPU shader
        if let Some(wgpu_shader) = shader.as_any().downcast_ref::<WgpuShaderInterface>() {
            if let Some(pipeline) = &wgpu_shader.render_pipeline {
                render_pass.set_pipeline(pipeline);

                if let Some(bind_group) = &wgpu_shader.bind_group {
                    render_pass.set_bind_group(0, bind_group, &[]);
                }
            }
        }

        // Draw the mesh
        let vertex_count = node.mesh.vertices.len() as u32;
        if !node.mesh.indices.is_empty() {
            let index_count = node.mesh.indices.len() as u32;
            render_pass.draw_indexed(0..index_count, 0, 0..1);
        } else {
            render_pass.draw(0..vertex_count, 0..1);
        }

        Ok(())
    }

    fn create_vertex_buffer(&self, vertices: &[WgpuVertex]) -> Buffer {
        self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: BufferUsages::VERTEX,
        })
    }

    fn create_index_buffer(&self, indices: &[u32]) -> Buffer {
        self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: BufferUsages::INDEX,
        })
    }
}

impl WgpuRenderNode {
    pub fn new(mesh: Arc<MeshGeometry>, shader: Arc<Mutex<dyn ShdInterface>>) -> ShdResult<Self> {
        Ok(Self {
            mesh,
            shader,
            vertex_buffer: None,
            index_buffer: None,
            is_visible: false,
        })
    }
}

impl RenderNode for WgpuRenderNode {
    fn get_shader_class_id(&self) -> u32 {
        self.shader.lock().unwrap().get_class_id()
    }

    fn render(&mut self, _pass: u32, render_info: &RenderInfo) -> ShdResult<()> {
        self.is_visible = true;

        // Apply shader settings
        let shader = self.shader.lock().unwrap();
        shader.apply_shared(_pass, render_info)?;
        shader.apply_instance(_pass, render_info)?;

        Ok(())
    }

    fn flush(&mut self, _pass: u32) -> ShdResult<()> {
        // In WGPU, flushing is handled by the renderer's render_frame method
        self.is_visible = false;
        Ok(())
    }

    fn apply_shared_shader_settings(
        &mut self,
        _previous_node: Option<&dyn RenderNode>,
        _pass: u32,
    ) -> ShdResult<()> {
        // Shader settings are applied in the render method for WGPU
        Ok(())
    }

    fn compare_for_sorting(&self, other: &dyn RenderNode, _pass: u32) -> std::cmp::Ordering {
        self.get_shader_class_id().cmp(&other.get_shader_class_id())
    }

    fn is_similar_enough(&self, other: &dyn RenderNode, _pass: u32) -> bool {
        self.get_shader_class_id() == other.get_shader_class_id()
    }

    fn connect_to_visible_list(&mut self) {
        self.is_visible = true;
    }
}

// Implement the same ShaderRenderer trait as the DirectX renderer
impl ShaderRenderer for WgpuRenderer {
    fn initialize(&mut self) -> ShdResult<()> {
        if self.is_initialized {
            return Err(ShdError::InvalidConfig(
                "Renderer already initialized".to_string(),
            ));
        }

        // WGPU initialization is mostly done in new()
        self.is_initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> ShdResult<()> {
        if !self.is_initialized {
            return Ok(());
        }

        // Clear all resources
        self.mesh_containers.clear();
        self.vertex_buffers.clear();
        self.index_buffers.clear();

        self.is_initialized = false;
        Ok(())
    }

    fn register_mesh(
        &mut self,
        mesh: Arc<MeshGeometry>,
        shader: Arc<Mutex<dyn ShdInterface>>,
    ) -> ShdResult<Arc<Mutex<dyn RenderNode>>> {
        let class_id = shader.lock().unwrap().get_class_id();

        // Create render node
        let render_node = Arc::new(Mutex::new(WgpuRenderNode::new(mesh.clone(), shader)?));

        // Get or create container for this shader class
        let container = self
            .mesh_containers
            .entry(class_id)
            .or_insert_with(|| WgpuMeshContainer {
                class_id,
                render_nodes: Vec::new(),
                vertex_buffer: None,
                index_buffer: None,
            });

        container.render_nodes.push(render_node.clone());

        // Create vertex/index buffers for the container if not already created
        if container.vertex_buffer.is_none() && !mesh.vertices.is_empty() {
            // Convert mesh vertices to WGPU format
            let wgpu_vertices: Vec<WgpuVertex> = mesh
                .vertices
                .iter()
                .map(|v| WgpuVertex {
                    position: [v.position.x, v.position.y, v.position.z],
                    normal: [v.normal.x, v.normal.y, v.normal.z],
                    uv: [v.uv.x, v.uv.y],
                    tangent: [v.tangent.x, v.tangent.y, v.tangent.z],
                    color: [v.color.x, v.color.y, v.color.z, v.color.w],
                })
                .collect();

            container.vertex_buffer = Some(self.create_vertex_buffer(&wgpu_vertices));
        }

        if container.index_buffer.is_none() && !mesh.indices.is_empty() {
            container.index_buffer = Some(self.create_index_buffer(&mesh.indices));
        }

        Ok(render_node)
    }

    fn flush(&mut self) -> ShdResult<()> {
        // For headless rendering, we don't need to present
        // For windowed rendering, use render_frame() instead
        Ok(())
    }

    fn apply_default_state(&mut self) -> ShdResult<()> {
        self.current_transform = glam::Mat4::IDENTITY;
        Ok(())
    }

    fn set_transform(&mut self, matrix: &glam::Mat4) -> ShdResult<()> {
        self.current_transform = *matrix;
        Ok(())
    }

    fn set_light_environment(&mut self, lights: &LightEnvironment) -> ShdResult<()> {
        self.current_light_environment = lights.clone();
        Ok(())
    }

    fn draw_triangles(
        &mut self,
        _start_index: u32,
        _triangle_count: u32,
        _start_vertex: u32,
        _vertex_count: u32,
    ) -> ShdResult<()> {
        // In WGPU, drawing is handled by the render pass
        Ok(())
    }
}

/// Convenience trait for downcasting shader interfaces
trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
}

impl AsAny for WgpuShaderInterface {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Implement AsAny for the ShdInterface trait
impl<T: ShdInterface + 'static> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
