//! WW3D Renderer - Complete WGPU-based 3D Graphics Engine
//!
//! This crate provides the complete rendering system for WW3D, implementing
//! all the features that were in the original C++ DirectX8 renderer.
#![allow(hidden_glob_reexports)]
#![allow(ambiguous_glob_reexports)]
#![allow(dead_code)]

pub mod animation_evaluator;
pub mod animation_synchronization;
pub mod asset_integration;
pub mod core;
pub mod effects_integration;
pub mod environment_mapping;
pub mod math;
pub mod mesh;
pub mod particle_bridge;
pub mod pointgr;
pub mod rendering;
pub mod seglinerenderer;
pub mod texturefilter;
pub mod textureloader;
pub mod utils;
pub mod w3d_animation_loader;
pub mod w3d_renderer;

// Alias for backward compatibility
pub mod math_utilities {
    // Standardize on glam for 2025
    pub use crate::texturefilter;
    pub use crate::textureloader;
    pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
    // Legacy-friendly aliases
    pub type Vector3 = Vec3;
    pub type Vector4 = Vec4;
    pub type Matrix4 = Mat4;
}
// DX8 re-exports removed; WGPU is the renderer path
pub mod config;
pub mod lod_system;
pub mod material_system;
pub mod render_object_system;
pub mod scene_system;
pub mod texture_system;

use crate::animation_synchronization::AnimationFrameInput;
use crate::render_object_system::RenderInfoClass as RendererRenderInfoClass;
use crate::rendering::frame_graph::{
    FrameGraph, FrameGraphPass, FrameGraphPassContext, FrameGraphQueue,
};
use crate::rendering::frame_uniform_arena::FrameUniformArena;
use crate::rendering::lighting_system::LightEnvironmentClass;
use crate::rendering::shadow_system::shadow_map::ShadowCasterSubmission;
use crate::rendering::swapchain_state::{
    make_surface_config, RendererSwapchainState, SwapchainFormatSet,
};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::core::error::Error;
pub use crate::core::error::RendererResult;
pub use ww3d_assets::AssetManager;
pub use ww3d_core::errors::{W3DError, W3DResult};
pub use ww3d_core::ww3d::WW3D;

const FRAME_UNIFORM_ARENA_SIZE: usize = 512 * 1024;
const FRAME_IN_FLIGHT: usize = 3;

// Re-export commonly used types
pub use rendering::camera_system::camera::{
    CameraClass, CameraUtils, ProjectionResType, ProjectionType,
};
pub use rendering::camera_system::frustum::{
    Frustum as CameraFrustum, FrustumClass as CameraFrustumClass, Plane3 as CameraPlane,
};
pub use rendering::camera_system::viewport::ViewportClass;
pub use rendering::camera_system::Camera;
pub use rendering::mesh_system::*;
pub use ww3d_core::*;
pub use ww3d_geometry::*;
// THE_DX8_MESH_RENDERER removed with DX8 path
pub use lod_system::*;
pub use render_object_system::*;
pub use scene_system::*;

// Re-export geometry types for convenience
pub use ww3d_collision::bounding_volumes::aabox::AABoxClass;
pub use ww3d_collision::bounding_volumes::obbox::OBBoxClass;
pub use ww3d_collision::bounding_volumes::sphere::SphereClass;

// Re-export high-level renderer API
pub use rendering::batching::batch_renderer::{
    BatchRenderer, BatchStats, BatchVertex, InstanceData, MaterialKey,
};
pub use w3d_renderer::W3DRenderer;

/// Attachment record emitted during rendering for gameplay hooks.
#[derive(Debug, Clone, Default)]
pub struct AttachmentRecord {
    pub name: String,
    pub parent_label: String,
}

/// Immutable bundle of render targets supplied to the renderer for a single frame.
pub struct RenderTargets<'a> {
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: Option<&'a wgpu::TextureView>,
}

// Alias for backward compatibility
pub mod bounding_volumes {
    pub use ww3d_collision::bounding_volumes::{
        aabox::AABoxClass as AABox, obbox::OBBoxClass as OBBox, sphere::SphereClass as Sphere,
    };

    // Submodule for AABox
    pub mod aabox {
        pub use ww3d_collision::bounding_volumes::aabox::AABoxClass;
    }

    // Submodule for Sphere
    pub mod sphere {
        pub use ww3d_collision::bounding_volumes::sphere::SphereClass;
    }

    // Stub for PlaneClass
    #[derive(Debug, Clone)]
    pub struct PlaneClass {
        pub normal: glam::Vec3,
        pub distance: f32,
    }

    impl PlaneClass {
        pub fn new(normal: glam::Vec3, distance: f32) -> Self {
            Self { normal, distance }
        }
    }
}

// Particle system integration through traits

// Placeholder vertex for simple demos
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
}

// Guard for managing render pass lifetime
pub struct RenderPassGuard<'a> {
    render_pass: ww3d_gpu::command::RenderPass<'a>,
}

impl<'a> RenderPassGuard<'a> {
    /// Borrow the underlying command pass wrapper
    pub fn command_pass(&mut self) -> &mut ww3d_gpu::command::RenderPass<'a> {
        &mut self.render_pass
    }

    /// Consume the guard and return the underlying command pass.
    pub fn into_command_pass(self) -> ww3d_gpu::command::RenderPass<'a> {
        self.render_pass
    }

    /// Borrow the underlying wgpu render pass for advanced operations
    pub fn wgpu_pass(&mut self) -> &mut wgpu::RenderPass<'a> {
        self.render_pass.inner()
    }

    /// Set the current render pipeline
    pub fn set_pipeline(&mut self, pipeline: &'a ww3d_gpu::pipeline::RenderPipeline) {
        self.render_pass.set_pipeline(pipeline);
    }

    /// Set vertex buffer
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &'a ww3d_gpu::Buffer, offset: u64) {
        self.render_pass.set_vertex_buffer(slot, buffer, offset);
    }

    /// Set index buffer
    pub fn set_index_buffer(
        &mut self,
        buffer: &'a ww3d_gpu::Buffer,
        offset: u64,
        format: wgpu::IndexFormat,
    ) {
        self.render_pass.set_index_buffer(buffer, offset, format);
    }

    /// Set bind group
    pub fn set_bind_group(&mut self, index: u32, bind_group: &'a wgpu::BindGroup) {
        self.render_pass.set_bind_group(index, bind_group, &[]);
    }

    /// Draw primitives
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.render_pass.draw(vertices, instances);
    }

    /// Draw indexed primitives
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.render_pass
            .draw_indexed(indices, base_vertex, instances);
    }
}

// Modern renderer integrated with GPU layer
pub struct Renderer {
    gpu_device: std::sync::Arc<ww3d_gpu::device::GpuDevice>,
    pipeline_manager: ww3d_gpu::pipeline::PipelineManager,
    texture_manager: ww3d_gpu::texture::TextureManager,
    buffer_manager: ww3d_gpu::buffer::BufferManager,
    frame_uniform_arenas: Vec<FrameUniformArena>,
    frame_uniform_index: usize,
    swapchain_state: Option<RendererSwapchainState>,

    mesh_render_manager: rendering::mesh_system::MeshRenderManager,
    registered_mesh_models: HashMap<usize, std::sync::Arc<rendering::mesh_system::MeshModelClass>>,
    frame_graph: FrameGraph,
    camera: Option<rendering::camera_system::CameraClass>,
    enable_lighting: bool,
    light_environment: Option<LightEnvironmentClass>,
    pending_attachments: Vec<AttachmentRecord>,
    pending_shadow_caster_submissions: Vec<ShadowCasterSubmission>,

    /// Animation frame coordinator - synchronizes all animation systems
    animation_coordinator: animation_synchronization::AnimationFrameCoordinator,
}

impl Renderer {
    /// Create a new renderer with GPU integration
    pub fn new(gpu_device: std::sync::Arc<ww3d_gpu::device::GpuDevice>) -> Self {
        let pipeline_manager = ww3d_gpu::pipeline::PipelineManager::new(gpu_device.clone());
        let texture_manager = ww3d_gpu::texture::TextureManager::new(gpu_device.clone());
        let buffer_manager = ww3d_gpu::buffer::BufferManager::new(gpu_device.clone());
        let mesh_render_manager =
            rendering::mesh_system::MeshRenderManager::new(gpu_device.clone());
        let mut frame_uniform_arenas = Vec::with_capacity(FRAME_IN_FLIGHT);
        for _ in 0..FRAME_IN_FLIGHT {
            frame_uniform_arenas.push(FrameUniformArena::new(
                &gpu_device,
                FRAME_UNIFORM_ARENA_SIZE,
            ));
        }

        Self {
            gpu_device,
            pipeline_manager,
            texture_manager,
            buffer_manager,
            frame_uniform_arenas,
            frame_uniform_index: 0,
            swapchain_state: None,
            mesh_render_manager,
            registered_mesh_models: HashMap::new(),
            frame_graph: FrameGraph::new(),
            camera: None,
            enable_lighting: true,
            light_environment: None,
            pending_attachments: Vec::new(),
            pending_shadow_caster_submissions: Vec::new(),
            animation_coordinator: animation_synchronization::AnimationFrameCoordinator::new(),
        }
    }

    /// Execute a closure with mutable access to the active renderer.
    pub fn with_active_mut<F, R>(f: F) -> RendererResult<R>
    where
        F: FnOnce(&mut Renderer) -> RendererResult<R>,
    {
        let renderer = WW3D::get_current_renderer()
            .ok_or_else(|| Error::NotInitialized("renderer not active".to_string()))?;
        let binding = renderer.handle();
        let mut backend_guard = binding
            .lock()
            .map_err(|_| Error::InvalidOperation("renderer backend poisoned".to_string()))?;

        let handle = backend_guard
            .as_any_mut()
            .downcast_mut::<rendering::wgpu_main_renderer::WgpuCoreBridge>()
            .map(|bridge| bridge.renderer_handle())
            .ok_or_else(|| {
                Error::InvalidOperation("renderer backend missing core bridge".to_string())
            })?;

        drop(backend_guard);

        let mut renderer_guard = handle
            .lock()
            .map_err(|_| Error::InvalidOperation("renderer handle poisoned".to_string()))?;
        f(&mut renderer_guard)
    }

    /// Backwards-compatible alias used by legacy call sites.
    pub fn with_global_mut<F, R>(f: F) -> RendererResult<R>
    where
        F: FnOnce(&mut Renderer) -> RendererResult<R>,
    {
        Self::with_active_mut(f)
    }

    /// Prepare renderer state for a new frame.
    pub fn begin_frame(&mut self) -> RendererResult<()> {
        self.mesh_render_manager.reset_stats();
        self.frame_graph.begin_frame();
        self.pending_shadow_caster_submissions.clear();
        if !self.frame_uniform_arenas.is_empty() {
            self.frame_uniform_index =
                (self.frame_uniform_index + 1) % self.frame_uniform_arenas.len();
            self.frame_uniform_arenas[self.frame_uniform_index].reset();
        }
        Ok(())
    }

    /// Pause animation playback
    pub fn pause_animation(&mut self) {
        self.animation_coordinator.clock_mut().pause();
    }

    /// Resume animation playback
    pub fn resume_animation(&mut self) {
        self.animation_coordinator.clock_mut().resume();
    }

    /// Set animation playback speed (1.0 = normal, 2.0 = 2x speed)
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.animation_coordinator.clock_mut().set_speed(speed);
    }

    /// Reset animation time to zero
    pub fn reset_animation(&mut self) {
        self.animation_coordinator.clock_mut().reset();
    }

    /// Get current animation elapsed time in seconds
    pub fn get_animation_time(&self) -> f32 {
        self.animation_coordinator.clock().elapsed_seconds()
    }

    /// Check if animation is playing
    pub fn is_animation_playing(&self) -> bool {
        self.animation_coordinator.clock().is_playing()
    }

    /// Load a shader from WGSL source code
    ///
    /// This creates a basic wgpu shader module from the provided WGSL source.
    /// For more advanced shader management with caching, use the pipeline_manager directly.
    pub fn load_shader(
        &mut self,
        source: &str,
        label: Option<&str>,
    ) -> Result<std::sync::Arc<ww3d_gpu::shader::Shader>, ww3d_gpu::GpuError> {
        // Create a wgpu shader module directly
        let _shader_module =
            self.gpu_device
                .device_arc()
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label,
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });

        // Return a default WW3D shader instance
        // Note: This returns the WW3D fixed-function shader representation
        // For WGSL shaders, users should use the pipeline_manager directly
        Ok(std::sync::Arc::new(ww3d_gpu::shader::Shader::new()))
    }

    /// Create a render pipeline
    pub fn create_pipeline(
        &mut self,
        compiled_shader: std::sync::Arc<ww3d_gpu::shader::CompiledShader>,
        label: Option<&str>,
    ) -> Result<std::sync::Arc<ww3d_gpu::pipeline::RenderPipeline>, ww3d_gpu::GpuError> {
        self.pipeline_manager
            .create_basic_render_pipeline(compiled_shader, label)
    }

    /// Create a texture
    pub fn create_texture(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<std::sync::Arc<ww3d_gpu::texture::GpuTexture>, ww3d_gpu::GpuError> {
        self.texture_manager
            .create_texture_2d(width, height, format, usage, label)
    }

    /// Create a vertex buffer
    pub fn create_vertex_buffer(
        &mut self,
        size: u64,
        label: Option<&str>,
    ) -> Result<std::sync::Arc<ww3d_gpu::buffer::GpuBuffer>, ww3d_gpu::GpuError> {
        self.buffer_manager.create_vertex_buffer(size, label)
    }

    /// Create an index buffer
    pub fn create_index_buffer(
        &mut self,
        size: u64,
        label: Option<&str>,
    ) -> Result<std::sync::Arc<ww3d_gpu::buffer::GpuBuffer>, ww3d_gpu::GpuError> {
        self.buffer_manager.create_index_buffer(size, label)
    }

    /// Get GPU device reference
    pub fn gpu_device(&self) -> &std::sync::Arc<ww3d_gpu::device::GpuDevice> {
        &self.gpu_device
    }

    /// Get texture manager
    pub fn texture_manager(&self) -> &ww3d_gpu::texture::TextureManager {
        &self.texture_manager
    }

    /// Get buffer manager
    pub fn buffer_manager(&self) -> &ww3d_gpu::buffer::BufferManager {
        &self.buffer_manager
    }

    /// Install an asset manager so render subsystems can stream resources on demand.
    pub fn set_asset_manager(
        &mut self,
        asset_manager: Arc<Mutex<AssetManager>>,
    ) -> RendererResult<()> {
        self.mesh_render_manager.set_asset_manager(asset_manager)
    }

    /// Access swapchain state if the renderer has been wired to a surface.
    pub fn swapchain_state(&self) -> Option<&RendererSwapchainState> {
        self.swapchain_state.as_ref()
    }

    /// Mutably access swapchain state if one has been initialised.
    pub fn swapchain_state_mut(&mut self) -> Option<&mut RendererSwapchainState> {
        self.swapchain_state.as_mut()
    }

    /// Configure render targets for a surface-backed frame flow.
    pub fn configure_render_targets(
        &mut self,
        surface_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        size: (u32, u32),
    ) -> RendererResult<()> {
        let surface_config = make_surface_config(size, surface_format, wgpu::PresentMode::Fifo);
        self.synchronize_swapchain(None, &surface_config, depth_format, 1, false)
    }

    /// Ensure swapchain state matches the latest surface configuration.
    pub fn synchronize_swapchain(
        &mut self,
        surface: Option<Arc<wgpu::Surface<'static>>>,
        surface_config: &wgpu::SurfaceConfiguration,
        depth_format: Option<wgpu::TextureFormat>,
        msaa_samples: u32,
        hdr_enabled: bool,
    ) -> RendererResult<()> {
        let msaa_samples = msaa_samples.max(1);

        if let Some(state) = self.swapchain_state.as_mut() {
            let current_format = state.surface_config().format;
            if current_format == surface_config.format {
                state.set_surface(surface.as_ref().map(Arc::clone))?;
                state.set_msaa_samples(msaa_samples)?;
                state.set_depth_format(depth_format)?;
                state.resize((surface_config.width, surface_config.height))?;
                if state.hdr_enabled() != hdr_enabled {
                    state.set_hdr_enabled(hdr_enabled)?;
                }
                return Ok(());
            }
        }

        let formats = SwapchainFormatSet::new(surface_config.format);
        let new_state = RendererSwapchainState::new(
            self.gpu_device.clone(),
            surface,
            surface_config.clone(),
            formats,
            depth_format,
            msaa_samples,
            hdr_enabled,
        )?;
        self.swapchain_state = Some(new_state);
        Ok(())
    }

    /// Record the queued meshes into the provided command encoder and render targets.
    pub fn render_with_targets(
        &mut self,
        targets: RenderTargets<'_>,
        clear_color: Option<wgpu::Color>,
        render_info: Option<&RendererRenderInfoClass>,
        frame_time: Option<AnimationFrameInput>,
    ) -> RendererResult<()> {
        self.mesh_render_manager.reset_stats();

        // Update animation frame and synchronize animation time
        let animation_input =
            frame_time.unwrap_or_else(|| AnimationFrameInput::from_delta(1.0 / 60.0));
        let animation_context = self.animation_coordinator.process_frame(animation_input);

        let mut info_owned;
        let info = if let Some(info) = render_info {
            // If render_info is provided externally, we use it as-is
            // (caller is responsible for animation time if needed)
            info
        } else {
            // Build render info and inject animation time
            info_owned = self.build_render_info()?;
            // Directly set animation time fields (they're public)
            info_owned.time = animation_context.elapsed_time;
            info_owned.frame_count = animation_context.frame_number as u64;
            &info_owned
        };

        let camera_pos = info.camera.get_position();
        let node = self.frame_graph.node_mut(FrameGraphPass::Main);
        let context = FrameGraphPassContext::from_render_info(FrameGraphPass::Main, info);
        node.set_context(context);
        let _static_flush_guard = rendering::mesh_system::StaticSortManager::begin_flush();
        let mut non_mesh_static = node.ingest_static_sort_entries();
        let prepared = node.prepare(camera_pos);
        self.pending_shadow_caster_submissions =
            Self::build_shadow_caster_submissions(&prepared.shadow_casters);
        let mut blended_and_decals = prepared.combined_translucent();
        blended_and_decals.extend(prepared.decals.iter().cloned());

        if self.frame_uniform_arenas.is_empty() {
            return Err(
                W3DError::NotInitialized("frame uniform arenas uninitialised".into()).into(),
            );
        }

        let frame_index = self.frame_uniform_index;
        assert!(
            frame_index < self.frame_uniform_arenas.len(),
            "frame uniform arena index out of range"
        );
        let arena = {
            let (_, tail) = self.frame_uniform_arenas.split_at_mut(frame_index);
            let (current, _) = tail
                .split_first_mut()
                .expect("frame uniform arena slice should not be empty");
            current
        };
        let mesh_manager = &mut self.mesh_render_manager;

        let load_op = clear_color
            .map(wgpu::LoadOp::Clear)
            .unwrap_or(wgpu::LoadOp::Load);

        let render_result = {
            let mut render_pass = targets
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("WW3D Main Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: targets.color_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: targets.depth_view.map(|view| {
                        wgpu::RenderPassDepthStencilAttachment {
                            view,
                            depth_ops: Some(wgpu::Operations {
                                load: if clear_color.is_some() {
                                    wgpu::LoadOp::Clear(1.0)
                                } else {
                                    wgpu::LoadOp::Load
                                },
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

            
            mesh_manager.render_pass(
                &mut render_pass,
                &prepared.opaque,
                &blended_and_decals,
                info,
                arena,
            )
        };
        render_result?;
        for render_obj in non_mesh_static.drain(..) {
            render_obj.render(info)?;
        }
        rendering::mesh_system::StaticSortManager::flush_static_sort_list();
        Ok(())
    }

    /// Render the queued meshes using the engine frame lifecycle.
    pub fn render_frame(
        &mut self,
        frame: &mut ww3d_engine::RenderFrame,
        clear_color: Option<wgpu::Color>,
        render_info: Option<&RendererRenderInfoClass>,
    ) -> RendererResult<()> {
        let color_view = frame.color_view_arc();
        let depth_view = frame.depth_view_arc();
        let depth_ref = depth_view.as_deref();
        let timing = frame.timing;
        let animation_input = AnimationFrameInput::new(
            timing.delta_seconds(),
            Some(timing.total_seconds()),
            Some(timing.frame_number),
        );
        self.render_with_targets(
            RenderTargets {
                encoder: frame.encoder(),
                color_view: color_view.as_ref(),
                depth_view: depth_ref,
            },
            clear_color,
            render_info,
            Some(animation_input),
        )
    }

    /// Handle post-render cleanup tasks
    /// Register a mesh model so its GPU resources are prepared
    pub fn register_mesh(
        &mut self,
        mesh_model: std::sync::Arc<rendering::mesh_system::MeshModelClass>,
    ) -> RendererResult<()> {
        let key = std::sync::Arc::as_ptr(&mesh_model) as usize;
        if self.registered_mesh_models.contains_key(&key) {
            return Ok(());
        }

        self.mesh_render_manager.ensure_model(&mesh_model)?;
        self.registered_mesh_models.insert(key, mesh_model);
        Ok(())
    }

    /// Queue a mesh instance to be rendered during the next flush call
    pub fn queue_mesh(
        &mut self,
        mesh: std::sync::Arc<rendering::mesh_system::MeshClass>,
    ) -> RendererResult<()> {
        if let Some(model) = &mesh.model {
            self.register_mesh(model.clone())?;
        }
        let queue = if mesh.is_alpha() {
            FrameGraphQueue::Alpha
        } else {
            FrameGraphQueue::Opaque
        };
        let shadows_enabled_for_frame = self
            .light_environment
            .as_ref()
            .map(|env| {
                env.lights.iter().any(|light| {
                    if let Ok(light) = light.lock() {
                        light.enabled && light.casts_shadows
                    } else {
                        false
                    }
                })
            })
            .unwrap_or(false);

        let should_queue_shadow = shadows_enabled_for_frame
            && mesh
                .model
                .as_ref()
                .map(|model| model.index_count > 0 || model.vertex_count > 0)
                .unwrap_or(false)
            && !mesh.is_decal_instance;

        self.frame_graph.node_mut(FrameGraphPass::Main).submit_mesh(
            mesh.clone(),
            Some(queue),
            None,
        );
        if should_queue_shadow {
            self.frame_graph.node_mut(FrameGraphPass::Main).submit_mesh(
                mesh,
                Some(FrameGraphQueue::ShadowCaster),
                None,
            );
        }
        Ok(())
    }

    /// Queue a decal mesh to be rendered with decal-specific settings
    pub fn queue_decal_mesh(
        &mut self,
        mesh: std::sync::Arc<rendering::mesh_system::MeshClass>,
    ) -> RendererResult<()> {
        if let Some(model) = &mesh.model {
            self.register_mesh(model.clone())?;
        }
        self.frame_graph.node_mut(FrameGraphPass::Main).submit_mesh(
            mesh,
            Some(FrameGraphQueue::Decal),
            None,
        );
        Ok(())
    }

    /// Set camera for rendering
    pub fn set_camera(&mut self, camera: rendering::camera_system::CameraClass) {
        self.camera = Some(camera);
    }

    /// Enable or disable lighting
    pub fn enable_lighting(&mut self, enable: bool) {
        self.enable_lighting = enable;
    }

    pub fn set_light_environment(&mut self, environment: Option<LightEnvironmentClass>) {
        self.light_environment = environment;
    }

    /// Add a decal mesh to the render list
    /// Equivalent to C++ Add_To_Render_List for decals
    pub fn add_decal_mesh_to_render_list(
        &mut self,
        decal_mesh: std::sync::Arc<rendering::mesh_system::MeshClass>,
    ) -> RendererResult<()> {
        self.queue_decal_mesh(decal_mesh)
    }

    /// Retrieve the current frame mesh rendering statistics
    pub fn mesh_stats(&self) -> &rendering::mesh_system::MeshRenderStats {
        self.mesh_render_manager.get_stats()
    }

    /// Drain any attachments generated during the previous frame.
    pub fn take_pending_attachments(&mut self) -> Vec<AttachmentRecord> {
        std::mem::take(&mut self.pending_attachments)
    }

    /// Snapshot the most recently prepared shadow-caster submissions.
    pub fn shadow_caster_submissions(&self) -> &[ShadowCasterSubmission] {
        &self.pending_shadow_caster_submissions
    }

    /// Drain the most recently prepared shadow-caster submissions.
    pub fn take_pending_shadow_caster_submissions(&mut self) -> Vec<ShadowCasterSubmission> {
        std::mem::take(&mut self.pending_shadow_caster_submissions)
    }

    fn build_shadow_caster_submissions(
        meshes: &[std::sync::Arc<rendering::mesh_system::MeshClass>],
    ) -> Vec<ShadowCasterSubmission> {
        meshes
            .iter()
            .filter_map(|mesh| Self::shadow_submission_for_mesh(mesh.as_ref()))
            .collect()
    }

    fn shadow_submission_for_mesh(
        mesh: &rendering::mesh_system::MeshClass,
    ) -> Option<ShadowCasterSubmission> {
        if mesh.is_hidden || mesh.is_animation_hidden || mesh.is_decal_instance {
            return None;
        }

        if let Some(model) = &mesh.model {
            if model.index_count > 0 {
                return Some(ShadowCasterSubmission::indexed_triangles(model.index_count));
            }
            if model.vertex_count > 0 {
                return Some(ShadowCasterSubmission::triangles(model.vertex_count));
            }
            if !model.triangles.is_empty() {
                return Some(ShadowCasterSubmission::triangles(
                    (model.triangles.len() as u32).saturating_mul(3),
                ));
            }
        }

        let fallback_vertices = mesh.get_num_polys().saturating_mul(3);
        if fallback_vertices > 0 {
            Some(ShadowCasterSubmission::triangles(fallback_vertices))
        } else {
            None
        }
    }

    fn build_render_info(&self) -> RendererResult<RendererRenderInfoClass> {
        let camera = self.camera.as_ref().ok_or_else(|| {
            ww3d_core::errors::W3DError::NotInitialized("Renderer camera not set".to_string())
        })?;
        let mut render_info = RendererRenderInfoClass::new(Arc::new(camera.clone()));
        render_info.viewport = *camera.get_viewport();
        if let Some(environment) = self.light_environment.clone() {
            render_info.set_lighting_environment(environment);
        }
        if !self.enable_lighting {
            render_info
                .override_flags
                .insert(RenderInfoOverrideFlags::ADDITIONAL_PASSES_ONLY);
        }
        Ok(render_info)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_renderer_creation() {
        // Note: This test requires a mock or real GPU device
        // For now, just test the struct exists
        // let renderer = Renderer::new(gpu_device);
        // assert!(renderer.gpu_device().is_some());
    }

    #[test]
    fn shadow_submission_prefers_indexed_geometry() {
        let mut mesh = rendering::mesh_system::MeshClass::new();
        let mut model = rendering::mesh_system::MeshModelClass::new("indexed");
        model.index_count = 36;
        model.vertex_count = 24;
        mesh.model = Some(Arc::new(model));

        let submission = Renderer::shadow_submission_for_mesh(&mesh).expect("submission");
        assert!(matches!(
            submission.primitive,
            crate::rendering::shadow_system::shadow_map::ShadowCasterPrimitive::Indexed {
                index_count: 36,
                ..
            }
        ));
    }

    #[test]
    fn shadow_submission_falls_back_to_vertex_geometry() {
        let mut mesh = rendering::mesh_system::MeshClass::new();
        let mut model = rendering::mesh_system::MeshModelClass::new("vertex");
        model.index_count = 0;
        model.vertex_count = 18;
        mesh.model = Some(Arc::new(model));

        let submission = Renderer::shadow_submission_for_mesh(&mesh).expect("submission");
        assert!(matches!(
            submission.primitive,
            crate::rendering::shadow_system::shadow_map::ShadowCasterPrimitive::NonIndexed {
                vertex_count: 18,
                ..
            }
        ));
    }

    #[test]
    fn shadow_submission_skips_hidden_or_decal_meshes() {
        let mut hidden = rendering::mesh_system::MeshClass::new();
        hidden.is_hidden = true;
        hidden.model = Some(Arc::new(rendering::mesh_system::MeshModelClass::new(
            "hidden",
        )));
        assert!(Renderer::shadow_submission_for_mesh(&hidden).is_none());

        let mut decal = rendering::mesh_system::MeshClass::new();
        let mut model = rendering::mesh_system::MeshModelClass::new("decal");
        model.index_count = 6;
        decal.model = Some(Arc::new(model));
        decal.is_decal_instance = true;
        assert!(Renderer::shadow_submission_for_mesh(&decal).is_none());
    }
}
