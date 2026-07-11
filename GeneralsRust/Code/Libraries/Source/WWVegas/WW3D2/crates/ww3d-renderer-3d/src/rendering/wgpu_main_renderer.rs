use crate::animation_synchronization::AnimationFrameInput;
use crate::core::error::{Error as RendererError, RendererResult};
use crate::render_object_system::StaticSortRenderObject;
use crate::rendering::camera_system::CameraClass;
use crate::rendering::mesh_system::{self, MeshClass};
use crate::rendering::shadow_system::shadow_map::ShadowCasterSubmission;
use crate::rendering::wgpu_renderer::wgpu_wrapper::{self, WgpuWrapper};
use crate::Renderer;
use glam::Vec4;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::{Device, Queue, Surface, SurfaceConfiguration, TextureFormat};
use ww3d_assets::AssetManager;
use ww3d_core::errors::{W3DError, W3DResult};
use ww3d_core::ww3d::{FrameStats, RendererBackend, WW3D};
use ww3d_engine::{self, EngineError};
use ww3d_gpu::device::GpuDevice;

/// Configuration for the main renderer. Mirrors the intent of the original DX8 renderer while
/// exposing knobs relevant to the WGPU backend.
#[derive(Debug, Clone)]
pub struct WgpuMainRendererConfig {
    /// Target frames per second used for timing heuristics.
    pub target_fps: u32,
    /// Whether to enable V-Sync when a swapchain is available.
    pub vsync: bool,
    /// Whether to enable MSAA.
    pub anti_aliasing: bool,
    /// Backbuffer clear colour used when no explicit colour is provided by callers.
    pub clear_color: Vec4,
}

impl Default for WgpuMainRendererConfig {
    fn default() -> Self {
        Self {
            target_fps: 60,
            vsync: true,
            anti_aliasing: false,
            clear_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// Frame statistics captured for diagnostic and profiling purposes.
#[derive(Debug, Clone, Default)]
pub struct MainRendererStats {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub meshes_rendered: u32,
    pub triangles_rendered: u32,
    pub material_passes: u32,
    pub texture_switches: u32,
    pub shader_switches: u32,
    pub vertex_color_passes: u32,
}

/// Primary high level renderer coordinating frame lifetime and delegating the actual draw work to
/// the [`WgpuWrapper`] compatibility layer.
pub struct WgpuMainRenderer {
    backend: Option<Arc<Mutex<WgpuWrapper>>>,
    renderer: Arc<Mutex<Renderer>>,
    config: WgpuMainRendererConfig,
    stats: MainRendererStats,
    frame_start: Instant,
    last_fps_update: Instant,
    frame_accumulator: Duration,
    frame_counter: u32,
    frame_stats_bridge: Arc<Mutex<FrameStats>>,
    ready_flag: Arc<AtomicBool>,
    registered_with_ww3d: bool,
    pending_frame: Option<ww3d_engine::RenderFrame>,
    pre_scene_callbacks:
        Mutex<Vec<Box<dyn FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send>>>,
    post_frame_callbacks:
        Mutex<Vec<Box<dyn FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send>>>,
    legacy_frame_clock: LegacyFrameClock,
    shadow_caster_submissions: Vec<ShadowCasterSubmission>,
    shadow_caster_count_hint: u32,
}

#[derive(Debug)]
struct LegacyFrameClock {
    last_instant: Instant,
    total_time: Duration,
    frame_counter: u64,
    initialized: bool,
}

impl LegacyFrameClock {
    fn new() -> Self {
        Self {
            last_instant: Instant::now(),
            total_time: Duration::ZERO,
            frame_counter: 0,
            initialized: false,
        }
    }

    fn advance(&mut self) -> AnimationFrameInput {
        let now = Instant::now();
        let delta = if self.initialized {
            now.duration_since(self.last_instant).as_secs_f32()
        } else {
            1.0 / 60.0
        };
        self.initialized = true;
        self.last_instant = now;
        if delta > 0.0 && delta.is_finite() {
            self.total_time += Duration::from_secs_f32(delta);
        }
        self.frame_counter = self.frame_counter.wrapping_add(1);
        AnimationFrameInput::new(
            delta.max(0.0),
            Some(self.total_time.as_secs_f32()),
            Some(self.frame_counter),
        )
    }
}

impl WgpuMainRenderer {
    fn renderable_submission_count(submissions: &[ShadowCasterSubmission]) -> u32 {
        submissions
            .iter()
            .filter(|submission| submission.is_renderable())
            .count() as u32
    }

    fn sync_shadow_submissions(&mut self, submissions: Vec<ShadowCasterSubmission>) {
        let renderable_count = Self::renderable_submission_count(&submissions);
        self.shadow_caster_count_hint = renderable_count;
        self.shadow_caster_submissions = submissions;
    }

    /// Prepare the underlying renderer for a new frame.
    fn prepare_renderer_frame(renderer: &Arc<Mutex<Renderer>>) -> RendererResult<()> {
        let mut renderer_guard = renderer
            .lock()
            .map_err(|_| RendererError::InvalidOperation("renderer mutex poisoned".into()))?;
        renderer_guard.begin_frame()
    }

    /// Construct the renderer from an existing [`WgpuWrapper`] backend.
    pub fn from_backend(backend: Arc<Mutex<WgpuWrapper>>, config: WgpuMainRendererConfig) -> Self {
        let msaa_samples = if config.anti_aliasing { 4 } else { 1 };
        let (device, queue, surface_config, surface_handle) = {
            let mut backend_guard = backend.lock().expect("WGPU backend poisoned");
            backend_guard.set_msaa_samples(msaa_samples);
            (
                backend_guard.device(),
                backend_guard.queue(),
                backend_guard.surface_config().clone(),
                backend_guard.surface(),
            )
        };

        let gpu_device = Arc::new(GpuDevice::from_shared(device, queue));
        let renderer = Arc::new(Mutex::new(Renderer::new(gpu_device)));
        {
            if let Ok(mut guard) = renderer.lock() {
                guard.mesh_render_manager.set_render_formats(
                    surface_config.format,
                    Some(TextureFormat::Depth24PlusStencil8),
                );
                let msaa_samples = if config.anti_aliasing { 4 } else { 1 };
                if let Err(err) = guard.synchronize_swapchain(
                    surface_handle,
                    &surface_config,
                    Some(TextureFormat::Depth24PlusStencil8),
                    msaa_samples,
                    false,
                ) {
                    eprintln!("Failed to configure render targets during renderer init: {err:?}");
                }
                guard.set_camera(CameraClass::new());
            }
        }

        let frame_stats_bridge = Arc::new(Mutex::new(FrameStats::default()));
        let ready_flag = Arc::new(AtomicBool::new(true));
        let mut registered_with_ww3d = WW3D::register_renderer(WgpuCoreBridge::new(
            frame_stats_bridge.clone(),
            ready_flag.clone(),
            Arc::clone(&renderer),
        ));
        if !registered_with_ww3d {
            WW3D::unregister_renderer();
            registered_with_ww3d = WW3D::register_renderer(WgpuCoreBridge::new(
                frame_stats_bridge.clone(),
                ready_flag.clone(),
                Arc::clone(&renderer),
            ));
        }

        Self {
            backend: Some(backend),
            renderer,
            config,
            stats: MainRendererStats::default(),
            frame_start: Instant::now(),
            last_fps_update: Instant::now(),
            frame_accumulator: Duration::ZERO,
            frame_counter: 0,
            frame_stats_bridge,
            ready_flag,
            registered_with_ww3d,
            pending_frame: None,
            pre_scene_callbacks: Mutex::new(Vec::new()),
            post_frame_callbacks: Mutex::new(Vec::new()),
            legacy_frame_clock: LegacyFrameClock::new(),
            shadow_caster_submissions: Vec::new(),
            shadow_caster_count_hint: 0,
        }
    }

    /// Construct the renderer from raw WGPU parts.
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface: Option<Arc<Surface<'static>>>,
        surface_config: SurfaceConfiguration,
    ) -> RendererResult<Self> {
        let backend = Arc::new(Mutex::new(wgpu_wrapper::WgpuWrapper::from_parts(
            device,
            queue,
            surface,
            surface_config,
            None,
            None,
        )?));

        Ok(Self::from_backend(
            backend,
            WgpuMainRendererConfig::default(),
        ))
    }

    /// Construct the renderer using the global WW3D engine lifecycle.
    pub fn from_engine(config: WgpuMainRendererConfig) -> RendererResult<Self> {
        let gpu_device =
            ww3d_engine::gpu_device().map_err(|err| RendererError::RenderError(err.to_string()))?;
        let renderer = Arc::new(Mutex::new(Renderer::new(gpu_device)));

        if let Ok(mut guard) = renderer.lock() {
            let color_format = ww3d_engine::color_format()
                .map_err(|err| RendererError::RenderError(err.to_string()))?;
            let depth_format = ww3d_engine::depth_format()
                .map_err(|err| RendererError::RenderError(err.to_string()))?;
            guard
                .mesh_render_manager
                .set_render_formats(color_format, depth_format);
            guard.set_camera(CameraClass::new());
        }

        let frame_stats_bridge = Arc::new(Mutex::new(FrameStats::default()));
        let ready_flag = Arc::new(AtomicBool::new(true));
        let mut registered_with_ww3d = WW3D::register_renderer(WgpuCoreBridge::new(
            frame_stats_bridge.clone(),
            ready_flag.clone(),
            Arc::clone(&renderer),
        ));
        if !registered_with_ww3d {
            WW3D::unregister_renderer();
            registered_with_ww3d = WW3D::register_renderer(WgpuCoreBridge::new(
                frame_stats_bridge.clone(),
                ready_flag.clone(),
                Arc::clone(&renderer),
            ));
        }

        Ok(Self {
            backend: None,
            renderer,
            config,
            stats: MainRendererStats::default(),
            frame_start: Instant::now(),
            last_fps_update: Instant::now(),
            frame_accumulator: Duration::ZERO,
            frame_counter: 0,
            frame_stats_bridge,
            ready_flag,
            registered_with_ww3d,
            pending_frame: None,
            pre_scene_callbacks: Mutex::new(Vec::new()),
            post_frame_callbacks: Mutex::new(Vec::new()),
            legacy_frame_clock: LegacyFrameClock::new(),
            shadow_caster_submissions: Vec::new(),
            shadow_caster_count_hint: 0,
        })
    }

    /// Access the underlying legacy backend, when available.
    pub fn backend(&self) -> Option<Arc<Mutex<WgpuWrapper>>> {
        self.backend.as_ref().map(Arc::clone)
    }

    /// Access the renderer configuration.
    pub fn config(&self) -> &WgpuMainRendererConfig {
        &self.config
    }

    /// Update the renderer configuration.
    pub fn set_config(&mut self, config: WgpuMainRendererConfig) {
        self.config = config;
    }

    /// Retrieve the most recent frame statistics.
    pub fn stats(&self) -> &MainRendererStats {
        &self.stats
    }

    /// Expose the renderer handle for scene integration.
    pub fn renderer_handle(&self) -> Arc<Mutex<Renderer>> {
        Arc::clone(&self.renderer)
    }

    /// Allow external systems to install an asset manager.
    pub fn set_asset_manager(&self, asset_manager: Arc<Mutex<AssetManager>>) -> RendererResult<()> {
        let mut renderer = self
            .renderer
            .lock()
            .map_err(|_| RendererError::InvalidOperation("renderer mutex poisoned".into()))?;
        renderer.set_asset_manager(asset_manager)?;
        Ok(())
    }

    /// Begin a new frame.
    pub fn begin_frame(&mut self) -> RendererResult<()> {
        self.frame_start = Instant::now();
        self.shadow_caster_submissions.clear();
        self.shadow_caster_count_hint = 0;

        if self.pending_frame.is_some() {
            return Err(RendererError::InvalidOperation(
                "frame already active".into(),
            ));
        }

        match ww3d_engine::begin_render() {
            Ok(frame) => {
                if let Err(err) = Self::prepare_renderer_frame(&self.renderer) {
                    if let Err(end_err) = ww3d_engine::end_render(frame) {
                        eprintln!(
                            "Failed to unwind render frame after renderer init error: {end_err:?}"
                        );
                    }
                    return Err(err);
                }

                self.pending_frame = Some(frame);
                self.ready_flag.store(true, Ordering::Release);
                Ok(())
            }
            Err(EngineError::NotInitialised) => {
                let backend = self.backend.clone().ok_or_else(|| {
                    RendererError::NotInitialized(
                        "WW3D engine not initialised and no legacy backend configured".into(),
                    )
                })?;

                {
                    let mut backend = backend.lock().map_err(|_| {
                        RendererError::InvalidOperation("backend mutex poisoned".into())
                    })?;
                    backend.begin_scene()?;
                    let clear = self.config.clear_color;
                    backend.clear(true, true, clear.truncate(), clear.w, 1.0, 0);
                }

                if let Err(err) = Self::prepare_renderer_frame(&self.renderer) {
                    let mut backend = backend.lock().map_err(|_| {
                        RendererError::InvalidOperation("backend mutex poisoned".into())
                    })?;
                    if let Err(end_err) = backend.end_scene(false) {
                        eprintln!(
                            "Failed to abort legacy backend frame after renderer init error: {end_err:?}"
                        );
                    }
                    return Err(err);
                }

                self.ready_flag.store(true, Ordering::Release);
                Ok(())
            }
            Err(EngineError::FrameInProgress) => Err(RendererError::InvalidOperation(
                "engine frame already active".into(),
            )),
            Err(err) => Err(RendererError::RenderError(err.to_string())),
        }
    }

    /// Present the frame.
    pub fn end_frame(&mut self) -> RendererResult<()> {
        let clear = self.config.clear_color;
        let clear_color = wgpu::Color {
            r: clear.x as f64,
            g: clear.y as f64,
            b: clear.z as f64,
            a: clear.w as f64,
        };

        let frame_mesh_stats = if let Some(mut frame) = self.pending_frame.take() {
            let frame_timing = frame.timing;
            WW3D::sync(frame_timing.total_time.as_millis() as u32);
            let frame_work_result: RendererResult<_> = (|| {
                let had_pre_scene_callbacks = self
                    .pre_scene_callbacks
                    .lock()
                    .map(|callbacks| !callbacks.is_empty())
                    .unwrap_or(false);
                if had_pre_scene_callbacks {
                    self.run_pre_scene_callbacks(&mut frame)?;
                }

                let (stats, shadow_submissions) = {
                    let mut renderer_guard = self.renderer.lock().map_err(|_| {
                        RendererError::InvalidOperation("renderer mutex poisoned".into())
                    })?;

                    renderer_guard
                        .render_frame(
                            &mut frame,
                            if had_pre_scene_callbacks {
                                None
                            } else {
                                Some(clear_color)
                            },
                            None,
                        )?;
                    (
                        renderer_guard.mesh_stats().clone(),
                        renderer_guard.take_pending_shadow_caster_submissions(),
                    )
                };
                self.sync_shadow_submissions(shadow_submissions);

                self.run_post_frame_callbacks(&mut frame)?;
                Ok(stats)
            })();

            // Always attempt to end the engine frame even when rendering/callback work fails.
            // This keeps WW3D frame state coherent and avoids persistent
            // "engine frame already active" failures on subsequent frames.
            let end_result = ww3d_engine::end_render(frame)
                .map_err(|err| RendererError::RenderError(err.to_string()));

            match (frame_work_result, end_result) {
                (Ok(stats), Ok(())) => stats,
                (Err(work_err), Ok(())) => return Err(work_err),
                (Ok(_), Err(end_err)) => return Err(end_err),
                (Err(work_err), Err(end_err)) => {
                    return Err(RendererError::RenderError(format!(
                        "frame work failed: {work_err:?}; additionally failed to end engine frame: {end_err:?}"
                    )))
                }
            }
        } else if let Some(backend) = self.backend.clone() {
            let mut backend = backend
                .lock()
                .map_err(|_| RendererError::InvalidOperation("backend mutex poisoned".into()))?;
            let surface_handle = backend.surface();
            let surface_config = backend.surface_config().clone();

            let mut renderer_guard = self
                .renderer
                .lock()
                .map_err(|_| RendererError::InvalidOperation("renderer mutex poisoned".into()))?;

            let msaa_samples = if self.config.anti_aliasing { 4 } else { 1 };
            backend.set_msaa_samples(msaa_samples);
            renderer_guard
                .synchronize_swapchain(
                    surface_handle,
                    &surface_config,
                    Some(TextureFormat::Depth24PlusStencil8),
                    msaa_samples,
                    false,
                )?;

            let animation_input = self.legacy_frame_clock.advance();
            let sync_ms = (animation_input.total_seconds.unwrap_or(0.0).max(0.0) * 1000.0)
                .clamp(0.0, u32::MAX as f32) as u32;
            WW3D::sync(sync_ms);

            let (stats, shadow_submissions) = backend.with_render_targets(|targets| {
                renderer_guard.render_with_targets(
                    targets,
                    Some(clear_color),
                    None,
                    Some(animation_input),
                )?;
                Ok((
                    renderer_guard.mesh_stats().clone(),
                    renderer_guard.take_pending_shadow_caster_submissions(),
                ))
            })?;
            drop(renderer_guard);
            self.sync_shadow_submissions(shadow_submissions);

            backend.end_scene(true)?;
            if let Ok(mut callbacks) = self.pre_scene_callbacks.lock() {
                if !callbacks.is_empty() {
                    log::warn!(
                        "pre-scene callbacks ignored when running in legacy backend mode ({} callbacks dropped)",
                        callbacks.len()
                    );
                    callbacks.clear();
                }
            }
            if let Ok(mut callbacks) = self.post_frame_callbacks.lock() {
                if !callbacks.is_empty() {
                    log::warn!(
                        "post-frame callbacks ignored when running in legacy backend mode ({} callbacks dropped)",
                        callbacks.len()
                    );
                    callbacks.clear();
                }
            }

            stats
        } else {
            return Err(RendererError::NotInitialized(
                "no active engine frame and legacy backend unavailable".into(),
            ));
        };

        self.stats.draw_calls = frame_mesh_stats.draw_calls;
        self.stats.meshes_rendered = frame_mesh_stats.meshes_rendered;
        self.stats.triangles_rendered = frame_mesh_stats.triangles_rendered;
        self.stats.material_passes = frame_mesh_stats.material_passes;
        self.stats.texture_switches = frame_mesh_stats.texture_switches;
        self.stats.shader_switches = frame_mesh_stats.shader_switches;
        self.stats.vertex_color_passes = frame_mesh_stats.vertex_color_passes;

        if let Ok(mut bridge) = self.frame_stats_bridge.lock() {
            *bridge = FrameStats::from(&self.stats);
        }

        let elapsed = self.frame_start.elapsed();
        self.frame_accumulator += elapsed;
        self.frame_counter += 1;

        if self.last_fps_update.elapsed().as_secs_f32() >= 0.5 {
            if self.frame_accumulator.is_zero() {
                self.stats.fps = 0.0;
                self.stats.frame_time_ms = 0.0;
            } else {
                let average = self.frame_accumulator / self.frame_counter;
                self.stats.frame_time_ms = average.as_secs_f32() * 1000.0;
                self.stats.fps = if average.is_zero() {
                    0.0
                } else {
                    1.0 / average.as_secs_f32()
                };
            }

            self.frame_accumulator = Duration::ZERO;
            self.frame_counter = 0;
            self.last_fps_update = Instant::now();
        }

        Ok(())
    }

    pub fn enqueue_post_frame_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        if let Ok(mut callbacks) = self.post_frame_callbacks.lock() {
            callbacks.push(Box::new(callback));
        }
    }

    pub fn enqueue_pre_scene_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        if let Ok(mut callbacks) = self.pre_scene_callbacks.lock() {
            callbacks.push(Box::new(callback));
        }
    }

    fn run_pre_scene_callbacks(
        &mut self,
        frame: &mut ww3d_engine::RenderFrame,
    ) -> RendererResult<()> {
        let mut callbacks = self.pre_scene_callbacks.lock().map_err(|_| {
            RendererError::InvalidOperation("pre-scene callback mutex poisoned".into())
        })?;
        let callbacks = std::mem::take(&mut *callbacks);
        for callback in callbacks {
            callback(frame)?;
        }
        Ok(())
    }

    fn run_post_frame_callbacks(
        &mut self,
        frame: &mut ww3d_engine::RenderFrame,
    ) -> RendererResult<()> {
        let mut callbacks = self.post_frame_callbacks.lock().map_err(|_| {
            RendererError::InvalidOperation("post-frame callback mutex poisoned".into())
        })?;
        let callbacks = std::mem::take(&mut *callbacks);
        for callback in callbacks {
            callback(frame)?;
        }
        Ok(())
    }

    /// Resize the underlying surface/headless target.
    pub fn resize(&mut self, width: u32, height: u32) -> RendererResult<()> {
        match ww3d_engine::resize(width, height) {
            Ok(()) => return Ok(()),
            Err(EngineError::NotInitialised) => {
                // fall through to the legacy path
            }
            Err(err) => {
                return Err(RendererError::RenderError(err.to_string()));
            }
        }

        let backend = self.backend.clone().ok_or_else(|| {
            RendererError::NotInitialized(
                "WW3D engine not initialised and no legacy backend configured".into(),
            )
        })?;

        let msaa_samples = if self.config.anti_aliasing { 4 } else { 1 };
        let (surface_config, surface_handle) = {
            let mut backend = backend
                .lock()
                .map_err(|_| RendererError::InvalidOperation("backend mutex poisoned".into()))?;
            backend.resize(width, height)?;
            backend.set_msaa_samples(msaa_samples);
            let config = backend.surface_config().clone();
            let surface = backend.surface();
            (config, surface)
        };

        if let Ok(mut renderer) = self.renderer.lock() {
            if let Err(err) = renderer.synchronize_swapchain(
                surface_handle,
                &surface_config,
                Some(TextureFormat::Depth24PlusStencil8),
                msaa_samples,
                false,
            ) {
                eprintln!("Failed to update render targets after resize: {err:?}");
            }
        }
        Ok(())
    }

    /// Access the underlying device.
    pub fn device(&self) -> Arc<Device> {
        if let Some(backend) = &self.backend {
            backend.lock().expect("backend mutex poisoned").device()
        } else {
            ww3d_engine::device().expect("WW3D engine not initialised")
        }
    }

    /// Access the underlying queue.
    pub fn queue(&self) -> Arc<Queue> {
        if let Some(backend) = &self.backend {
            backend.lock().expect("backend mutex poisoned").queue()
        } else {
            ww3d_engine::queue().expect("WW3D engine not initialised")
        }
    }

    /// Access the surface configuration (legacy path only).
    pub fn surface_config(&self) -> SurfaceConfiguration {
        if let Some(backend) = &self.backend {
            backend
                .lock()
                .expect("backend mutex poisoned")
                .surface_config()
                .clone()
        } else {
            panic!("surface configuration unavailable when ww3d_engine drives the renderer")
        }
    }

    /// Publish the current renderer statistics without presenting a frame.
    pub fn snapshot_stats(&mut self) -> FrameStats {
        if let Ok(renderer) = self.renderer.lock() {
            let mesh_stats = renderer.mesh_stats().clone();
            self.stats.draw_calls = mesh_stats.draw_calls;
            self.stats.meshes_rendered = mesh_stats.meshes_rendered;
            self.stats.triangles_rendered = mesh_stats.triangles_rendered;
            self.stats.material_passes = mesh_stats.material_passes;
            self.stats.texture_switches = mesh_stats.texture_switches;
            self.stats.shader_switches = mesh_stats.shader_switches;
            self.stats.vertex_color_passes = mesh_stats.vertex_color_passes;
        }
        if let Ok(mut bridge) = self.frame_stats_bridge.lock() {
            *bridge = FrameStats::from(&self.stats);
        }
        FrameStats::from(&self.stats)
    }

    /// Finish a headless frame without presenting and return the resulting stats.
    pub fn finish_headless_frame(&mut self) -> RendererResult<FrameStats> {
        self.end_frame()?;
        Ok(self.snapshot_stats())
    }

    /// Shadow submissions captured from the active renderer during the previous frame.
    pub fn shadow_caster_submissions(&self) -> &[ShadowCasterSubmission] {
        &self.shadow_caster_submissions
    }

    /// Number of renderable shadow casters captured from the previous frame.
    pub fn shadow_caster_count_hint(&self) -> u32 {
        self.shadow_caster_count_hint
    }
}

impl RendererBackend for WgpuMainRenderer {
    fn begin_frame(&mut self) -> W3DResult<()> {
        WgpuMainRenderer::begin_frame(self).map_err(Into::into)
    }

    fn end_frame(&mut self) -> W3DResult<()> {
        WgpuMainRenderer::end_frame(self).map_err(Into::into)
    }

    fn is_ready(&self) -> bool {
        true
    }

    fn frame_stats(&self) -> FrameStats {
        FrameStats::from(&self.stats)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Drop for WgpuMainRenderer {
    fn drop(&mut self) {
        if self.registered_with_ww3d {
            WW3D::unregister_renderer();
        }
        self.ready_flag.store(false, Ordering::Release);
    }
}

impl From<&MainRendererStats> for FrameStats {
    fn from(stats: &MainRendererStats) -> Self {
        FrameStats {
            fps: stats.fps,
            frame_time_ms: stats.frame_time_ms,
            draw_calls: stats.draw_calls,
            meshes_rendered: stats.meshes_rendered,
            triangles_rendered: stats.triangles_rendered,
            material_passes: stats.material_passes,
            texture_switches: stats.texture_switches,
            shader_switches: stats.shader_switches,
            vertex_color_passes: stats.vertex_color_passes,
        }
    }
}

pub(crate) struct WgpuCoreBridge {
    stats: Arc<Mutex<FrameStats>>,
    ready: Arc<AtomicBool>,
    sorting_enabled: Arc<AtomicBool>,
    static_sort_enabled: Arc<AtomicBool>,
    decals_enabled: Arc<AtomicBool>,
    _renderer: Arc<Mutex<Renderer>>,
}

impl WgpuCoreBridge {
    fn new(
        stats: Arc<Mutex<FrameStats>>,
        ready: Arc<AtomicBool>,
        renderer: Arc<Mutex<Renderer>>,
    ) -> Self {
        Self {
            stats,
            ready,
            sorting_enabled: Arc::new(AtomicBool::new(true)),
            static_sort_enabled: Arc::new(AtomicBool::new(false)),
            decals_enabled: Arc::new(AtomicBool::new(true)),
            _renderer: renderer,
        }
    }

    pub(crate) fn renderer_handle(&self) -> Arc<Mutex<Renderer>> {
        Arc::clone(&self._renderer)
    }
}

impl RendererBackend for WgpuCoreBridge {
    fn begin_frame(&mut self) -> W3DResult<()> {
        self.ready.store(true, Ordering::Release);
        Ok(())
    }

    fn end_frame(&mut self) -> W3DResult<()> {
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn frame_stats(&self) -> FrameStats {
        self.stats
            .lock()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }

    fn set_sorting_enabled(&mut self, enabled: bool) -> W3DResult<()> {
        self.sorting_enabled.store(enabled, Ordering::Release);
        Ok(())
    }

    fn is_sorting_enabled(&self) -> bool {
        self.sorting_enabled.load(Ordering::Acquire)
    }

    fn set_static_sort_lists_enabled(&mut self, enabled: bool) -> W3DResult<()> {
        self.static_sort_enabled.store(enabled, Ordering::Release);
        mesh_system::StaticSortManager::set_static_sort_lists_enabled(enabled);
        Ok(())
    }

    fn are_static_sort_lists_enabled(&self) -> bool {
        self.static_sort_enabled.load(Ordering::Acquire)
    }

    fn set_decals_enabled(&mut self, enabled: bool) -> W3DResult<()> {
        self.decals_enabled.store(enabled, Ordering::Release);
        mesh_system::StaticSortManager::set_decals_enabled(enabled);
        Ok(())
    }

    fn are_decals_enabled(&self) -> bool {
        self.decals_enabled.load(Ordering::Acquire)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_to_static_sort_list(
        &mut self,
        object: Arc<dyn Any + Send + Sync>,
        sort_level: u32,
    ) -> W3DResult<()> {
        match object.downcast::<StaticSortRenderObject>() {
            Ok(handle) => {
                mesh_system::StaticSortManager::add_to_static_sort_list(handle, sort_level);
                Ok(())
            }
            Err(object) => match object.downcast::<MeshClass>() {
                Ok(mesh) => {
                    let handle = StaticSortRenderObject::from_arc(Arc::clone(&mesh));
                    mesh_system::StaticSortManager::add_to_static_sort_list_with_mesh(
                        handle,
                        sort_level,
                        Some(mesh),
                    );
                    Ok(())
                }
                Err(_) => Err(W3DError::UnsupportedType(
                    "static sort object type not handled by WGPU renderer".to_string(),
                )),
            },
        }
    }

    fn flush_static_sort_lists(&mut self) -> W3DResult<()> {
        let _guard = mesh_system::StaticSortManager::begin_flush();
        mesh_system::StaticSortManager::flush_static_sort_list();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn frame_stats_conversion_preserves_fields() {
        let mut source = MainRendererStats::default();
        source.fps = 120.0;
        source.frame_time_ms = 8.3;
        source.draw_calls = 42;
        source.meshes_rendered = 21;
        source.triangles_rendered = 1337;
        source.material_passes = 84;
        source.texture_switches = 12;
        source.shader_switches = 7;
        source.vertex_color_passes = 3;

        let frame_stats = FrameStats::from(&source);

        assert_eq!(frame_stats.fps, source.fps);
        assert_eq!(frame_stats.frame_time_ms, source.frame_time_ms);
        assert_eq!(frame_stats.draw_calls, source.draw_calls);
        assert_eq!(frame_stats.meshes_rendered, source.meshes_rendered);
        assert_eq!(frame_stats.triangles_rendered, source.triangles_rendered);
        assert_eq!(frame_stats.material_passes, source.material_passes);
        assert_eq!(frame_stats.texture_switches, source.texture_switches);
        assert_eq!(frame_stats.shader_switches, source.shader_switches);
        assert_eq!(frame_stats.vertex_color_passes, source.vertex_color_passes);
    }

    #[test]
    fn renderable_submission_count_ignores_non_renderable_entries() {
        let submissions = vec![
            ShadowCasterSubmission::triangles(9),
            ShadowCasterSubmission::indexed_triangles(12),
            ShadowCasterSubmission {
                primitive:
                    crate::rendering::shadow_system::shadow_map::ShadowCasterPrimitive::Indexed {
                        index_count: 0,
                        first_index: 0,
                        base_vertex: 0,
                    },
                first_instance: 0,
                instance_count: 1,
            },
            ShadowCasterSubmission {
                primitive:
                    crate::rendering::shadow_system::shadow_map::ShadowCasterPrimitive::NonIndexed {
                        vertex_count: 12,
                        first_vertex: 0,
                    },
                first_instance: 0,
                instance_count: 0,
            },
        ];

        assert_eq!(
            WgpuMainRenderer::renderable_submission_count(&submissions),
            2
        );
    }
}
