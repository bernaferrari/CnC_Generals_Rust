use crate::core::error::{Error, Result};
use crate::material_system::VertexMaterialClass;
use crate::math::Vector2;
use crate::rendering::camera_system::ViewportClass as Viewport;
use crate::rendering::shader_system::shader::{ShaderApplyResources, ShaderClass};
use crate::rendering::texture_system::texture_base::TextureBaseClass;
use crate::RenderTargets;
use glam::{Mat4, Vec3, Vec4};
use pollster::block_on;
use std::sync::{Arc, Mutex, OnceLock};
use wgpu::{
    Adapter, Device, Instance, Queue, Surface, SurfaceConfiguration, SurfaceError, SurfaceTexture,
};

use super::wgpu_buffer::{EngineRef, WgpuIndexBuffer, WgpuVertexBuffer};
use super::wgpu_render_state::{ChangedStates, RenderStateStruct};
use super::wgpu_surface::WgpuSurfaceManager;
use super::wgpu_texture::BasicTextureManager;

/// Maximum number of texture stages supported by the compatibility wrapper
pub const MAX_TEXTURE_STAGES: usize = 8;
/// Maximum number of vertex streams supported
pub const MAX_VERTEX_STREAMS: usize = 2;
/// Maximum number of vertex shader constants
pub const MAX_VERTEX_SHADER_CONSTANTS: usize = 96;
/// Maximum number of pixel shader constants
pub const MAX_PIXEL_SHADER_CONSTANTS: usize = 8;
/// Maximum number of shadow maps supported by legacy paths
pub const MAX_SHADOW_MAPS: usize = 1;

/// Buffer type enumeration kept for compatibility with DX8 wrapper API
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferType {
    Dx8,
    Sorting,
    DynamicDx8,
    DynamicSorting,
    Invalid,
}

/// Transform state enumeration matching D3DTRANSFORMSTATETYPE
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformState {
    World = 0,
    View = 1,
    Projection = 2,
    Texture0 = 16,
    Texture1 = 17,
    Texture2 = 18,
    Texture3 = 19,
    Texture4 = 20,
    Texture5 = 21,
    Texture6 = 22,
    Texture7 = 23,
}

/// Render state enumeration matching D3DRENDERSTATETYPE compatibility IDs.
#[allow(dead_code)] // C++ parity
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderState {
    ZEnable = 7,
    ZWriteEnable = 14,
    SrcBlend = 19,
    DestBlend = 20,
    CullMode = 22,
    FogEnable = 28,
    FogColor = 34,
    ColorWriteEnable = 168,
}

/// Texture stage state enumeration matching D3DTEXTURESTAGESTATETYPE compatibility IDs.
#[allow(dead_code)] // C++ parity
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureStageState {
    ColorOp = 1,
    ColorArg1 = 2,
    ColorArg2 = 3,
    AlphaOp = 4,
    AlphaArg1 = 5,
    AlphaArg2 = 6,
    Constant = 32,
}

/// Arguments cached for a clear request. Applied lazily when the first render pass is executed.
#[derive(Debug, Clone)]
struct ClearArgs {
    color: Option<wgpu::Color>,
    depth: Option<f32>,
    stencil: Option<u32>,
}

/// Per-frame state containing the acquired frame resources and command encoder.
struct ActiveFrame {
    encoder: wgpu::CommandEncoder,
    surface_output: Option<SurfaceTexture>,
    color_view: wgpu::TextureView,
    had_pass: bool,
}

/// Depth resources associated with the currently configured surface size.
struct DepthResources {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// WGPU wrapper that mimics the original DX8 wrapper contract while driving a modern WGPU backend.
pub struct WgpuWrapper {
    instance: Option<Arc<Instance>>,
    adapter: Option<Arc<Adapter>>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    surface: Option<Arc<Surface<'static>>>,
    surface_config: SurfaceConfiguration,
    surface_manager: WgpuSurfaceManager,
    texture_manager: BasicTextureManager,

    render_state: RenderStateStruct,
    render_state_changed: ChangedStates,
    transforms: [Mat4; 24],

    // Statistics
    matrix_changes: u32,
    material_changes: u32,
    vertex_buffer_changes: u32,
    index_buffer_changes: u32,
    light_changes: u32,
    texture_changes: u32,
    render_state_changes: u32,
    texture_stage_state_changes: u32,
    draw_calls: u32,

    frame_count: u64,

    fog_enable: bool,
    fog_color: Vec3,
    fog_start: f32,
    fog_end: f32,
    ambient_color: Vec3,
    z_bias: i32,
    z_near: f32,
    z_far: f32,

    is_initted: bool,
    is_device_lost: bool,
    enable_triangle_draw: bool,

    pending_clear: Option<ClearArgs>,
    active_frame: Option<ActiveFrame>,
    depth: Option<DepthResources>,
    headless_target: Option<wgpu::Texture>,

    cached_shader_resources: Option<ShaderApplyResources>,
}

impl WgpuWrapper {
    /// Create a wrapper from fully constructed WGPU parts. This is primarily used by higher level
    /// initialization helpers and keeps lifetime management in one place.
    pub fn from_parts(
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface: Option<Arc<Surface<'static>>>,
        mut surface_config: SurfaceConfiguration,
        instance: Option<Arc<Instance>>,
        adapter: Option<Arc<Adapter>>,
    ) -> Result<Self> {
        if surface_config.width == 0 || surface_config.height == 0 {
            surface_config.width = surface_config.width.max(1);
            surface_config.height = surface_config.height.max(1);
        }

        let mut wrapper = Self {
            instance,
            adapter,
            device: device.clone(),
            queue: queue.clone(),
            surface: surface.clone(),
            surface_config,
            surface_manager: WgpuSurfaceManager::new(),
            texture_manager: BasicTextureManager::new(),
            render_state: RenderStateStruct::new(),
            render_state_changed: ChangedStates::empty(),
            transforms: [Mat4::IDENTITY; 24],
            matrix_changes: 0,
            material_changes: 0,
            vertex_buffer_changes: 0,
            index_buffer_changes: 0,
            light_changes: 0,
            texture_changes: 0,
            render_state_changes: 0,
            texture_stage_state_changes: 0,
            draw_calls: 0,
            frame_count: 0,
            fog_enable: false,
            fog_color: Vec3::ZERO,
            fog_start: 0.0,
            fog_end: 1.0,
            ambient_color: Vec3::ZERO,
            z_bias: 0,
            z_near: 0.0,
            z_far: 1.0,
            is_initted: false,
            is_device_lost: false,
            enable_triangle_draw: true,
            pending_clear: None,
            active_frame: None,
            depth: None,
            headless_target: None,
            cached_shader_resources: None,
        };

        wrapper.recreate_depth_resources()?;

        if let Some(surface) = surface {
            surface.configure(&device, &wrapper.surface_config);
            wrapper.surface_manager.set_surface(surface);
        } else {
            wrapper.create_headless_target()?;
        }

        Ok(wrapper)
    }

    /// Create a wrapper by constructing a fresh WGPU instance and surface from the provided window.
    ///
    /// # Safety and Lifetime Management
    ///
    /// This function creates a surface that is stored with 'static lifetime. This is safe because:
    /// 1. The window type W is constrained to 'static lifetime
    /// 2. We use wgpu's SurfaceTarget to safely manage the window handle ownership
    /// 3. The surface is owned by WgpuWrapper and dropped when the wrapper is dropped
    ///
    /// The caller must ensure the window remains valid for the lifetime of the WgpuWrapper.
    pub fn new_with_surface<W>(
        window: W,
        size: (u32, u32),
        present_mode: wgpu::PresentMode,
    ) -> Result<Self>
    where
        W: Into<wgpu::SurfaceTarget<'static>>,
    {
        let instance = Arc::new(Instance::new(&wgpu::InstanceDescriptor::default()));

        // Create surface using SurfaceTarget which safely handles window ownership
        // This avoids the need for unsafe transmute by letting wgpu manage the lifetime
        let surface = instance
            .create_surface(window)
            .map_err(|e| Error::Generic(format!("Failed to create surface: {e}")))?;

        let surface = Arc::new(surface);
        let adapter = Arc::new(
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .map_err(|e| Error::AdapterNotFound(format!("No compatible adapter: {e}")))?,
        );

        let mut required_features = wgpu::Features::empty();
        if adapter
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
        }

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("WW3D Renderer Device"),
            required_features,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| Error::Generic(format!("Failed to request device: {e}")))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .into_iter()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm
                        | wgpu::TextureFormat::Bgra8UnormSrgb
                        | wgpu::TextureFormat::Rgba8Unorm
                        | wgpu::TextureFormat::Rgba8UnormSrgb
                )
            })
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0.max(1),
            height: size.1.max(1),
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self::from_parts(
            device,
            queue,
            Some(surface),
            surface_config,
            Some(instance),
            Some(adapter),
        )
    }

    /// Create a wrapper using an off-screen texture as the render target.
    pub fn new_headless(size: (u32, u32), format: wgpu::TextureFormat) -> Result<Self> {
        let instance = Arc::new(Instance::new(&wgpu::InstanceDescriptor::default()));
        let adapter = Arc::new(
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
            .map_err(|e| Error::AdapterNotFound(format!("No adapter available: {e}")))?,
        );

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("WW3D Headless Device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| Error::Generic(format!("Failed to request device: {e}")))?;

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: size.0.max(1),
            height: size.1.max(1),
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        Self::from_parts(
            Arc::new(device),
            Arc::new(queue),
            None,
            surface_config,
            Some(instance),
            Some(adapter),
        )
    }

    /// Accessor for the underlying device.
    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    /// Accessor for the underlying queue.
    pub fn queue(&self) -> Arc<Queue> {
        self.queue.clone()
    }

    /// Accessor for the surface configuration currently in use.
    pub fn surface_config(&self) -> &SurfaceConfiguration {
        &self.surface_config
    }

    /// Adjust the MSAA sample count used when configuring the surface.
    pub fn set_msaa_samples(&mut self, _samples: u32) {
        // MSAA support is not yet threaded through the wrapper; accept the call for parity.
    }

    /// Clone the underlying surface handle when available.
    pub fn surface(&self) -> Option<Arc<Surface<'static>>> {
        self.surface.clone()
    }

    /// Execute a closure with the active render targets.
    pub fn with_render_targets<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(RenderTargets<'_>) -> Result<T>,
    {
        self.ensure_frame()?;
        let frame = self.active_frame.as_mut().expect("frame prepared");
        let depth_view = self.depth.as_ref().map(|depth| &depth.view);
        let result = f(RenderTargets {
            encoder: &mut frame.encoder,
            color_view: &frame.color_view,
            depth_view,
        })?;
        frame.had_pass = true;
        self.pending_clear = None;
        Ok(result)
    }

    /// Resize the surface or headless target.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        self.surface_config.width = width;
        self.surface_config.height = height;

        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.surface_config);
        } else {
            self.create_headless_target()?;
        }

        self.recreate_depth_resources()?;
        Ok(())
    }

    /// Begin rendering for the current frame. Equivalent to DX8 BeginScene.
    pub fn begin_scene(&mut self) -> Result<()> {
        self.frame_count += 1;
        self.reset_statistics();
        self.is_device_lost = false;
        self.ensure_frame()?;
        self.is_initted = true;
        Ok(())
    }

    /// End rendering for the current frame. Equivalent to DX8 EndScene.
    pub fn end_scene(&mut self, flip_frame: bool) -> Result<()> {
        if let Some(mut frame) = self.active_frame.take() {
            let command_buffer = frame.encoder.finish();
            self.queue.submit(std::iter::once(command_buffer));

            if flip_frame {
                if let Some(output) = frame.surface_output.take() {
                    output.present();
                }
            }
        }

        Ok(())
    }

    /// Clear the framebuffer and optional depth/stencil targets on the next draw call.
    pub fn clear(
        &mut self,
        clear_color: bool,
        clear_depth_stencil: bool,
        color: Vec3,
        alpha: f32,
        depth: f32,
        stencil: u32,
    ) {
        let color = if clear_color {
            Some(wgpu::Color {
                r: color.x as f64,
                g: color.y as f64,
                b: color.z as f64,
                a: alpha as f64,
            })
        } else {
            None
        };

        let depth = if clear_depth_stencil {
            Some(depth)
        } else {
            None
        };
        let stencil = if clear_depth_stencil {
            Some(stencil)
        } else {
            None
        };

        self.pending_clear = Some(ClearArgs {
            color,
            depth,
            stencil,
        });
    }

    /// Set the active viewport used for subsequent draw calls.
    pub fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        _min_z: f32,
        _max_z: f32,
    ) {
        self.render_state.viewport = Some(Viewport::from_position_size(
            Vector2::new(x, y),
            Vector2::new(width, height),
        ));
    }

    /// Bind a vertex buffer to the specified stream index.
    pub fn set_vertex_buffer(&mut self, vb: &WgpuVertexBuffer, stream: usize) {
        if stream >= MAX_VERTEX_STREAMS {
            return;
        }

        self.render_state.vertex_buffers[stream] = Some(Arc::new(vb.clone()));
        self.render_state_changed
            .insert(ChangedStates::VERTEX_BUFFER_CHANGED);
        self.vertex_buffer_changes += 1;
    }

    /// Bind an index buffer for upcoming draw calls.
    pub fn set_index_buffer(&mut self, ib: &WgpuIndexBuffer, index_base_offset: u16) {
        self.render_state.index_buffer = Some(Arc::new(ib.clone()));
        self.render_state.index_base_offset = index_base_offset;
        self.render_state_changed
            .insert(ChangedStates::INDEX_BUFFER_CHANGED);
        self.index_buffer_changes += 1;
    }

    /// Bind a texture to the specified texture stage.
    pub fn set_texture(&mut self, stage: usize, texture: Option<&TextureBaseClass>) {
        if stage >= MAX_TEXTURE_STAGES {
            return;
        }

        self.render_state.textures[stage] = texture.map(|t| Arc::new(t.clone()));
        self.render_state_changed
            .insert(ChangedStates::from_bits_truncate(
                ChangedStates::TEXTURE0_CHANGED.bits() << stage,
            ));
        self.texture_changes += 1;
    }

    /// Bind a vertex material.
    pub fn set_material(&mut self, material: Option<&VertexMaterialClass>) {
        self.render_state.material = material.map(|m| Arc::new(m.clone()));
        self.render_state_changed
            .insert(ChangedStates::MATERIAL_CHANGED);
        self.material_changes += 1;
    }

    /// Bind a shader for upcoming draw calls.
    pub fn set_shader(&mut self, shader: &ShaderClass) {
        self.render_state.shader = Some(Arc::new(*shader));
        self.render_state_changed
            .insert(ChangedStates::SHADER_CHANGED);
    }

    /// Update one of the standard transform matrices.
    pub fn set_transform(&mut self, transform_type: TransformState, matrix: &Mat4) {
        let index = transform_type as usize;
        if index >= self.transforms.len() {
            return;
        }

        match transform_type {
            TransformState::World => {
                self.render_state.world = *matrix;
                self.render_state_changed
                    .insert(ChangedStates::WORLD_CHANGED);
                self.render_state_changed
                    .remove(ChangedStates::WORLD_IDENTITY);
            }
            TransformState::View => {
                self.render_state.view = *matrix;
                self.render_state_changed
                    .insert(ChangedStates::VIEW_CHANGED);
                self.render_state_changed
                    .remove(ChangedStates::VIEW_IDENTITY);
            }
            TransformState::Projection => {
                self.render_state.projection_matrix = *matrix;
            }
            _ => {}
        }

        self.transforms[index] = *matrix;
        self.matrix_changes += 1;
    }

    /// Reset world transform to identity.
    pub fn set_world_identity(&mut self) {
        self.render_state.world = Mat4::IDENTITY;
        self.render_state_changed
            .insert(ChangedStates::WORLD_CHANGED | ChangedStates::WORLD_IDENTITY);
    }

    /// Reset view transform to identity.
    pub fn set_view_identity(&mut self) {
        self.render_state.view = Mat4::IDENTITY;
        self.render_state_changed
            .insert(ChangedStates::VIEW_CHANGED | ChangedStates::VIEW_IDENTITY);
    }

    /// Are we currently using an identity world transform?
    pub fn is_world_identity(&self) -> bool {
        self.render_state_changed
            .contains(ChangedStates::WORLD_IDENTITY)
    }

    /// Are we currently using an identity view transform?
    pub fn is_view_identity(&self) -> bool {
        self.render_state_changed
            .contains(ChangedStates::VIEW_IDENTITY)
    }

    /// Access the render state for inspection.
    pub fn get_render_state(&self) -> &RenderStateStruct {
        &self.render_state
    }

    /// Replace the current render state wholesale.
    pub fn set_render_state(&mut self, state: RenderStateStruct) {
        for vb in &mut self.render_state.vertex_buffers {
            if let Some(buffer) = vb.take() {
                buffer.release_engine_ref();
            }
        }

        if let Some(index) = self.render_state.index_buffer.take() {
            index.release_engine_ref();
        }

        self.render_state = state;
        self.render_state_changed = ChangedStates::all();
    }

    /// Release all resources referenced by the render state.
    pub fn release_render_state(&mut self) {
        for vb in &mut self.render_state.vertex_buffers {
            if let Some(buffer) = vb.take() {
                buffer.release_engine_ref();
            }
        }

        if let Some(index) = self.render_state.index_buffer.take() {
            index.release_engine_ref();
        }

        self.render_state.material = None;
        for tex in &mut self.render_state.textures {
            *tex = None;
        }
    }

    /// Apply deferred state changes before drawing.
    fn apply_render_state_changes(&mut self) {
        if self.render_state_changed.is_empty() {
            return;
        }

        // Real implementation would translate state changes into GPU commands.
        self.render_state_changed = ChangedStates::empty();
    }

    /// Draw indexed triangles.
    pub fn draw_triangles(
        &mut self,
        _buffer_type: BufferType,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) -> Result<()> {
        if !self.enable_triangle_draw {
            return Ok(());
        }

        self.ensure_frame()?;
        self.apply_render_state_changes();

        let shader = self
            .render_state
            .shader
            .clone()
            .ok_or_else(|| Error::NotInitialized("Shader not bound".into()))?;

        let frame = self.active_frame.as_mut().expect("frame prepared");
        let color_ops = if !frame.had_pass {
            let clear = self.pending_clear.take();
            frame.had_pass = true;
            clear
        } else {
            None
        };

        {
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &frame.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: color_ops
                        .as_ref()
                        .and_then(|c| c.color)
                        .map(wgpu::LoadOp::Clear)
                        .unwrap_or(wgpu::LoadOp::Load),
                    store: wgpu::StoreOp::Store,
                },
            };

            let depth_attachment = self.depth.as_ref().map(|depth| {
                let depth_load = color_ops
                    .as_ref()
                    .and_then(|c| c.depth)
                    .map(wgpu::LoadOp::Clear)
                    .unwrap_or(wgpu::LoadOp::Load);
                let depth_ops = Some(wgpu::Operations {
                    load: depth_load,
                    store: wgpu::StoreOp::Store,
                });
                let stencil_ops =
                    color_ops
                        .as_ref()
                        .and_then(|c| c.stencil)
                        .map(|value| wgpu::Operations {
                            load: wgpu::LoadOp::Clear(value),
                            store: wgpu::StoreOp::Store,
                        });
                wgpu::RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops,
                    stencil_ops,
                }
            });

            let resources = shader.apply(
                &self.device,
                &self.surface_config,
                &self.render_state.view,
                &self.render_state.world,
                None,
                None,
                None,
            );
            let vertex_buffer = self
                .render_state
                .vertex_buffers.first()
                .and_then(|vb| vb.clone());
            let index_buffer = self.render_state.index_buffer.clone();
            let vertex_buffer_handle = vertex_buffer.as_ref().map(|vb| vb.buffer().clone());
            let index_buffer_handle = index_buffer.as_ref().map(|ib| ib.buffer().clone());

            {
                let mut render_pass =
                    frame
                        .encoder
                        .begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("WW3D Triangle Pass"),
                            color_attachments: &[Some(color_attachment)],
                            depth_stencil_attachment: depth_attachment,
                            occlusion_query_set: None,
                            timestamp_writes: None,
                        });

                resources.apply_to_render_pass(&mut render_pass);

                if let Some(ref buffer) = vertex_buffer_handle {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                }

                if let Some(viewport) = &self.render_state.viewport {
                    let min = viewport.min;
                    let size = viewport.size();
                    render_pass.set_viewport(
                        min.x,
                        min.y,
                        size.x.max(1.0),
                        size.y.max(1.0),
                        0.0,
                        1.0,
                    );
                }

                if let Some(ref buffer) = index_buffer_handle {
                    render_pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint16);
                    let index_count = polygon_count as u32 * 3;
                    render_pass.draw_indexed(
                        start_index as u32..start_index as u32 + index_count,
                        min_vertex_index as i32,
                        0..1,
                    );
                } else {
                    let vertex_count = vertex_count.max(polygon_count * 3);
                    render_pass.draw(
                        min_vertex_index as u32..min_vertex_index as u32 + vertex_count as u32,
                        0..1,
                    );
                }
            }
        }

        self.cached_shader_resources = None;
        self.draw_calls += 1;
        Ok(())
    }

    /// Convenience helper mirroring DX8 DrawPrimitiveUP semantics.
    pub fn draw_triangles_simple(
        &mut self,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) -> Result<()> {
        self.draw_triangles(
            BufferType::Dx8,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
        )
    }

    /// Draw a triangle strip. Currently this reuses the triangle list path for simplicity.
    pub fn draw_strip(
        &mut self,
        start_index: u16,
        index_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) -> Result<()> {
        let polygon_count = index_count.saturating_sub(2);
        self.draw_triangles(
            BufferType::Dx8,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
        )
    }

    /// Draw points using a point-list pipeline while preserving DX8 wrapper semantics.
    pub fn draw_points(&mut self, vertex_start: u32, vertex_count: u32) -> Result<()> {
        self.ensure_frame()?;
        self.apply_render_state_changes();

        if vertex_count == 0 {
            return Ok(());
        }

        let shader = self
            .render_state
            .shader
            .clone()
            .ok_or_else(|| Error::NotInitialized("Shader not bound".into()))?;

        let frame = self.active_frame.as_mut().expect("frame prepared");
        let color_ops = if !frame.had_pass {
            let clear = self.pending_clear.take();
            frame.had_pass = true;
            clear
        } else {
            None
        };

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &frame.color_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: color_ops
                    .as_ref()
                    .and_then(|c| c.color)
                    .map(wgpu::LoadOp::Clear)
                    .unwrap_or(wgpu::LoadOp::Load),
                store: wgpu::StoreOp::Store,
            },
        };
        let depth_attachment = self.depth.as_ref().map(|depth| {
            let depth_load = color_ops
                .as_ref()
                .and_then(|c| c.depth)
                .map(wgpu::LoadOp::Clear)
                .unwrap_or(wgpu::LoadOp::Load);
            let depth_ops = Some(wgpu::Operations {
                load: depth_load,
                store: wgpu::StoreOp::Store,
            });
            let stencil_ops =
                color_ops
                    .as_ref()
                    .and_then(|c| c.stencil)
                    .map(|value| wgpu::Operations {
                        load: wgpu::LoadOp::Clear(value),
                        store: wgpu::StoreOp::Store,
                    });
            wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops,
                stencil_ops,
            }
        });

        let vertex_buffer_handle = self
            .render_state
            .vertex_buffers.first()
            .and_then(|vb| vb.clone())
            .map(|vb| vb.buffer().clone());

        let resources = shader.apply_with_topology(
            &self.device,
            &self.surface_config,
            &self.render_state.view,
            &self.render_state.world,
            None,
            None,
            None,
            wgpu::PrimitiveTopology::PointList,
        );

        {
            let mut render_pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("WW3D Point Pass"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: depth_attachment,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
            resources.apply_to_render_pass(&mut render_pass);

            if let Some(ref buffer) = vertex_buffer_handle {
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                if let Some(viewport) = &self.render_state.viewport {
                    let min = viewport.min;
                    let size = viewport.size();
                    render_pass.set_viewport(
                        min.x,
                        min.y,
                        size.x.max(1.0),
                        size.y.max(1.0),
                        0.0,
                        1.0,
                    );
                }
                render_pass.draw(vertex_start..vertex_start + vertex_count, 0..1);
            }
        }
        self.cached_shader_resources = None;
        self.draw_calls += 1;
        Ok(())
    }

    /// Present the current frame.
    pub fn present(&mut self) -> Result<()> {
        self.end_scene(true)
    }

    /// Clear the screen immediately.
    pub fn clear_screen(&mut self, clear_color: Vec4) -> Result<()> {
        self.clear(true, true, clear_color.truncate(), clear_color.w, 1.0, 0);
        self.ensure_frame()?;
        let frame = self.active_frame.as_mut().expect("frame prepared");

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &frame.color_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: clear_color.x as f64,
                    g: clear_color.y as f64,
                    b: clear_color.z as f64,
                    a: clear_color.w as f64,
                }),
                store: wgpu::StoreOp::Store,
            },
        };

        drop(
            frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("WW3D Immediate Clear"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                }),
        );

        frame.had_pass = true;
        Ok(())
    }

    /// Convert a floating point color to packed ARGB format.
    pub fn convert_color(color: Vec3, alpha: f32) -> u32 {
        let r = (color.x.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (color.y.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (color.z.clamp(0.0, 1.0) * 255.0) as u32;
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }

    /// Convert packed ARGB color back to floating representation.
    pub fn convert_color_u32(color: u32) -> Vec4 {
        let a = ((color >> 24) & 0xFF) as f32 / 255.0;
        let r = ((color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((color >> 8) & 0xFF) as f32 / 255.0;
        let b = (color & 0xFF) as f32 / 255.0;
        Vec4::new(r, g, b, a)
    }

    /// Query adapter support for BC compression formats.
    pub fn adapter_supports_bc() -> bool {
        WGPU_WRAPPER_INSTANCE
            .get()
            .and_then(|wrapper| wrapper.lock().ok())
            .map(|instance| {
                instance
                    .device
                    .features()
                    .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
            })
            .unwrap_or(false)
    }

    /// Query whether we should prefer 16-bit textures.
    pub fn prefer_16bit_textures() -> bool {
        crate::config::get().prefer_16bit_textures
    }

    /// Set the ambient color for the scene.
    pub fn set_ambient_color(&mut self, color: Vec3) {
        self.ambient_color = color;
    }

    /// Get the ambient color.
    pub fn ambient_color(&self) -> Vec3 {
        self.ambient_color
    }

    /// Set the near and far z-planes for depth rendering.
    pub fn set_z_planes(&mut self, near: f32, far: f32) {
        self.z_near = near;
        self.z_far = far;
    }

    /// Get the near and far z-planes.
    pub fn z_planes(&self) -> (f32, f32) {
        (self.z_near, self.z_far)
    }

    /// Set the z-bias value for depth testing.
    pub fn set_z_bias(&mut self, bias: i32) {
        self.z_bias = bias;
    }

    /// Get the z-bias value.
    pub fn z_bias(&self) -> i32 {
        self.z_bias
    }

    /// Set fog parameters.
    pub fn set_fog(&mut self, enable: bool, color: Vec3, start: f32, end: f32) {
        self.fog_enable = enable;
        self.fog_color = color;
        self.fog_start = start;
        self.fog_end = end;
    }

    /// Get fog settings if fog is enabled.
    pub fn fog_settings(&self) -> Option<(Vec3, f32, f32)> {
        if self.fog_enable {
            Some((self.fog_color, self.fog_start, self.fog_end))
        } else {
            None
        }
    }

    /// Get a reference to the texture manager.
    pub fn texture_manager(&self) -> &BasicTextureManager {
        &self.texture_manager
    }

    /// Get a mutable reference to the texture manager.
    pub fn texture_manager_mut(&mut self) -> &mut BasicTextureManager {
        &mut self.texture_manager
    }

    /// Begin a frame for rendering. Returns a Frame handle that manages the frame lifecycle.
    /// This is a higher-level API compared to begin_scene/end_scene.
    pub fn begin_frame(&mut self) -> Result<Frame<'_>> {
        self.begin_scene()?;
        Ok(Frame { wrapper: self })
    }

    /// Reset per-frame statistics counters.

    /// Retrieve counters accumulated during the last frame. Matches the tuple returned by the
    /// legacy DX8 renderer for compatibility with tooling.
    pub fn get_last_frame_stats(&self) -> (u32, u32, u32, u32, u32, u32, u32, u32, u32) {
        (
            self.matrix_changes,
            self.material_changes,
            self.vertex_buffer_changes,
            self.index_buffer_changes,
            self.light_changes,
            self.texture_changes,
            self.render_state_changes,
            self.texture_stage_state_changes,
            self.draw_calls,
        )
    }
    fn reset_statistics(&mut self) {
        self.matrix_changes = 0;
        self.material_changes = 0;
        self.vertex_buffer_changes = 0;
        self.index_buffer_changes = 0;
        self.light_changes = 0;
        self.texture_changes = 0;
        self.render_state_changes = 0;
        self.texture_stage_state_changes = 0;
        self.draw_calls = 0;
    }

    /// Ensure a frame has been acquired for rendering.
    fn ensure_frame(&mut self) -> Result<()> {
        if self.active_frame.is_some() {
            return Ok(());
        }

        let (surface_output, color_view) = match &self.surface {
            Some(surface) => match surface.get_current_texture() {
                Ok(output) => {
                    let view = output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    (Some(output), view)
                }
                Err(SurfaceError::Lost) => {
                    if let Some(surface) = &self.surface {
                        surface.configure(&self.device, &self.surface_config);
                    }
                    return Err(Error::RenderError("Surface lost".into()));
                }
                Err(SurfaceError::OutOfMemory) => {
                    self.is_device_lost = true;
                    return Err(Error::RenderError("Out of memory".into()));
                }
                Err(e) => {
                    return Err(Error::RenderError(format!(
                        "Failed to acquire surface texture: {e}"
                    )))
                }
            },
            None => {
                let texture = self
                    .headless_target
                    .as_ref()
                    .ok_or_else(|| Error::NotInitialized("Headless target missing".into()))?;
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                (None, view)
            }
        };

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("WW3D Command Encoder"),
            });

        self.active_frame = Some(ActiveFrame {
            encoder,
            surface_output,
            color_view,
            had_pass: false,
        });

        Ok(())
    }

    /// Recreate the depth buffer to match the current surface dimensions.
    fn recreate_depth_resources(&mut self) -> Result<()> {
        let size = wgpu::Extent3d {
            width: self.surface_config.width.max(1),
            height: self.surface_config.height.max(1),
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("WW3D Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth = Some(DepthResources { texture, view });
        Ok(())
    }

    /// Create or recreate the headless render target texture.
    fn create_headless_target(&mut self) -> Result<()> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("WW3D Headless Target"),
            size: wgpu::Extent3d {
                width: self.surface_config.width.max(1),
                height: self.surface_config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.headless_target = Some(texture);
        Ok(())
    }
}

/// Frame handle that manages the lifecycle of a rendering frame.
/// Automatically calls end_scene when dropped.
pub struct Frame<'a> {
    wrapper: &'a mut WgpuWrapper,
}

impl<'a> Frame<'a> {
    /// Clear the frame with specified parameters.
    pub fn clear(
        &mut self,
        clear_color: bool,
        clear_depth_stencil: bool,
        color: Vec3,
        alpha: f32,
        depth: f32,
        stencil: u32,
    ) {
        self.wrapper.clear(
            clear_color,
            clear_depth_stencil,
            color,
            alpha,
            depth,
            stencil,
        );
    }

    /// Finish the frame and present it.
    pub fn finish(self) -> Result<()> {
        // The Drop impl will handle calling end_scene
        Ok(())
    }
}

impl<'a> Drop for Frame<'a> {
    fn drop(&mut self) {
        // Always call end_scene when the frame is dropped
        let _ = self.wrapper.end_scene(true);
    }
}

impl Drop for WgpuWrapper {
    fn drop(&mut self) {
        self.release_render_state();
    }
}

/// Global wrapper instance mirroring the DX8 global singleton.
static WGPU_WRAPPER_INSTANCE: OnceLock<Mutex<WgpuWrapper>> = OnceLock::new();

/// Retrieve the global wrapper instance. Panic if it has not been initialised.
pub fn get_wgpu_wrapper() -> std::sync::MutexGuard<'static, WgpuWrapper> {
    WGPU_WRAPPER_INSTANCE
        .get()
        .expect("WgpuWrapper not initialised")
        .lock()
        .expect("WgpuWrapper mutex poisoned")
}

fn set_global_wrapper(wrapper: WgpuWrapper) -> Result<()> {
    if let Some(slot) = WGPU_WRAPPER_INSTANCE.get() {
        *slot.lock().expect("WgpuWrapper mutex poisoned") = wrapper;
        Ok(())
    } else {
        WGPU_WRAPPER_INSTANCE
            .set(Mutex::new(wrapper))
            .map_err(|_| Error::InvalidOperation("WgpuWrapper already initialised".into()))
    }
}

/// Initialise the global wrapper from pre-constructed device parts. This is primarily useful for
/// integration tests or embedding scenarios.
pub fn init_wgpu_wrapper(
    device: Arc<Device>,
    queue: Arc<Queue>,
    surface: Option<Arc<Surface<'static>>>,
    config: SurfaceConfiguration,
) -> Result<()> {
    let wrapper = WgpuWrapper::from_parts(device, queue, surface, config, None, None)?;
    set_global_wrapper(wrapper)
}

/// Initialise the global wrapper in headless mode.
pub fn init_wgpu_wrapper_headless(size: (u32, u32), format: wgpu::TextureFormat) -> Result<()> {
    let wrapper = WgpuWrapper::new_headless(size, format)?;
    set_global_wrapper(wrapper)
}

/// Initialise the global wrapper from a window and surface configuration.
///
/// # Lifetime Safety
///
/// The window is moved into the surface, ensuring safe lifetime management.
/// The SurfaceTarget owns the window handle, preventing use-after-free.
pub fn init_wgpu_wrapper_with_surface<W>(
    window: W,
    size: (u32, u32),
    present_mode: wgpu::PresentMode,
) -> Result<()>
where
    W: Into<wgpu::SurfaceTarget<'static>>,
{
    let wrapper = WgpuWrapper::new_with_surface(window, size, present_mode)?;
    set_global_wrapper(wrapper)
}
