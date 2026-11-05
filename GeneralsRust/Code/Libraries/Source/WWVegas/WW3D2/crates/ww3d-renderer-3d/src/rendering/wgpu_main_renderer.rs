use crate::Renderer;
use crate::core::error::{Error as RendererError, RendererResult};
use crate::render_object_system::StaticSortRenderObject;
use crate::rendering::camera_system::CameraClass;
use crate::rendering::mesh_system::{self, MeshClass};
use crate::rendering::shadow_system::shadow_map::ShadowCasterSubmission;
use glam::Vec4;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Attempt an optional loading-screen frame. The returned boolean records
/// whether a frame was actually finished, rather than just attempted.
pub fn present_optional_frame<T>(
    renderer: &mut T,
    begin: impl FnOnce(&mut T) -> RendererResult<()>,
    finish: impl FnOnce(&mut T) -> RendererResult<()>,
) -> RendererResult<bool> {
    match begin(renderer) {
        Ok(()) => {}
        Err(RendererError::RenderError(message))
            if matches!(
                message.as_str(),
                "surface occluded"
                    | "surface timeout"
                    | "Failed to acquire surface texture: surface occluded"
                    | "Failed to acquire surface texture: surface timeout"
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    }
    finish(renderer)?;
    Ok(true)
}
use wgpu::{Device, Queue};
use ww3d_assets::AssetManager;
use ww3d_core::errors::{W3DError, W3DResult};
use ww3d_core::ww3d::{FrameStats, RendererBackend, Ww3dContext};
use crate::process_state::Renderer3dRuntime;
use ww3d_engine::{self, EngineError, SharedEngine};
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

/// THREAD: game thread (this renderer's frame loop) vs. the host frame pump
/// (`Renderer::with_active_mut` / Code/Main `forward_render.rs`), which queue
/// meshes into the same renderer through this shared handle and therefore meet
/// here.
type SharedRendererHandle = Arc<Mutex<Renderer>>;

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

/// Primary high level renderer coordinating frame lifetime over the host-owned
/// [`ww3d_engine::Engine`] frame (one encoder, one submit per frame).
pub struct WgpuMainRenderer {
    renderer: SharedRendererHandle,
    config: WgpuMainRendererConfig,
    stats: MainRendererStats,
    frame_start: Instant,
    last_fps_update: Instant,
    frame_accumulator: Duration,
    frame_counter: u32,
    frame_stats_bridge: Arc<Mutex<FrameStats>>,
    ready_flag: Arc<AtomicBool>,
    registered_with_ww3d: bool,
    /// Owned WW3D façade (C++ W3DDisplay owns WW3D session state).
    pub ww3d: Arc<Ww3dContext>,
    /// Owned renderer-3d session bag (config / texture / id counters).
    pub renderer_3d: Arc<Renderer3dRuntime>,
    /// Host-owned WW3D engine (explicit; no process singleton).
    pub engine: SharedEngine,
    pending_frame: Option<ww3d_engine::RenderFrame>,
    last_surface_presented: bool,
    /// Work queued for the next frame, recorded in [`FRAME_SCHEDULE`] order.
    schedule: FrameSchedule,
    shadow_caster_submissions: Vec<ShadowCasterSubmission>,
    shadow_caster_count_hint: u32,
}

pub use crate::rendering::frame_schedule::{FRAME_SCHEDULE, FramePhase, ScopedNativePass};
use crate::rendering::frame_schedule::{FrameSchedule, FrameWork};

type ScopedNativePassHandler<'a> =
    dyn FnMut(ScopedNativePass, &mut ww3d_engine::RenderFrame) -> RendererResult<()> + 'a;

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
    fn prepare_renderer_frame(renderer: &SharedRendererHandle) -> RendererResult<()> {
        let mut renderer_guard = renderer
            .lock()
            .map_err(|_| RendererError::InvalidOperation("renderer mutex poisoned".into()))?;
        renderer_guard.begin_frame()
    }

    /// Construct the renderer from an explicit host-owned engine handle.
    pub fn from_engine(engine: SharedEngine, config: WgpuMainRendererConfig) -> RendererResult<Self> {
        let (gpu_device, color_format, depth_format) = {
            let eng = engine.lock();
            (eng.gpu_device(), eng.color_format(), eng.depth_format())
        };
        let renderer = Arc::new(Mutex::new(Renderer::new(gpu_device)));

        if let Ok(mut guard) = renderer.lock() {
            guard
                .mesh_render_manager
                .set_render_formats(color_format, depth_format);
            guard.set_camera(CameraClass::new());
        }

        let frame_stats_bridge = Arc::new(Mutex::new(FrameStats::default()));
        let ready_flag = Arc::new(AtomicBool::new(true));
        let ww3d = Arc::new(Ww3dContext::new());
        let renderer_3d = Arc::new(Renderer3dRuntime::new());
        if let Ok(mut guard) = renderer.lock() {
            guard.set_ww3d(Arc::clone(&ww3d));
            guard.set_renderer_3d(Arc::clone(&renderer_3d));
        }
        let mut registered_with_ww3d = ww3d.register_renderer(WgpuCoreBridge::new(
            frame_stats_bridge.clone(),
            ready_flag.clone(),
            Arc::clone(&renderer),
            Arc::clone(&renderer_3d),
        ));
        if !registered_with_ww3d {
            ww3d.unregister_renderer();
            registered_with_ww3d = ww3d.register_renderer(WgpuCoreBridge::new(
                frame_stats_bridge.clone(),
                ready_flag.clone(),
                Arc::clone(&renderer),
                Arc::clone(&renderer_3d),
            ));
        }

        Ok(Self {
            renderer,
            engine,
            config,
            stats: MainRendererStats::default(),
            frame_start: Instant::now(),
            last_fps_update: Instant::now(),
            frame_accumulator: Duration::ZERO,
            frame_counter: 0,
            frame_stats_bridge,
            ready_flag,
            registered_with_ww3d,
            ww3d,
            renderer_3d,
            pending_frame: None,
            last_surface_presented: false,
            schedule: FrameSchedule::default(),
            shadow_caster_submissions: Vec::new(),
            shadow_caster_count_hint: 0,
        })
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
    pub fn renderer_handle(&self) -> SharedRendererHandle {
        Arc::clone(&self.renderer)
    }

    /// Install the host's archive-backed pass-texture resolver on the scene
    /// renderer (C++ `WW3DAssetManager::Get_Texture` parity for W3D pass
    /// textures that carry only a name).
    pub fn set_pass_texture_provider(
        &self,
        provider: crate::rendering::mesh_system::MeshPassTextureProvider,
    ) -> RendererResult<()> {
        let mut renderer = self
            .renderer
            .lock()
            .map_err(|_| RendererError::InvalidOperation("renderer mutex poisoned".into()))?;
        renderer.set_pass_texture_provider(provider);
        Ok(())
    }

    /// Clear the last result before a new application render attempt.
    pub fn reset_surface_presented(&mut self) {
        self.last_surface_presented = false;
    }

    /// True only when this renderer completed a frame with a window surface.
    pub fn last_surface_presented(&self) -> bool {
        self.last_surface_presented
    }

    /// Begin a new frame.
    ///
    /// FRAME CLOCK: the logic frame clock (`ww3d_engine` `last_frame_start`) is
    /// owned by `engine.lock().update()`, which opens every game frame. This
    /// call used to re-stamp it via `engine.lock().begin_render()`, excluding
    /// frame N's WW3D-update + logic-batch from frame N+1's dt and holding the
    /// sim at ~16-17 fps instead of 30 (C++ anchor: GameEngine::execute,
    /// GameEngine.cpp:774-884 — one clock per iteration). begin_render() now
    /// only reads the clock; `self.frame_start` below is this renderer's own
    /// stats clock (frame_accumulator in end_frame), not the logic dt chain.
    pub fn begin_frame(&mut self) -> RendererResult<()> {
        if self.pending_frame.is_some() {
            return Err(RendererError::InvalidOperation(
                "frame already active".into(),
            ));
        }

        self.last_surface_presented = false;
        self.frame_start = Instant::now();
        self.shadow_caster_submissions.clear();
        self.shadow_caster_count_hint = 0;

        let engine = self.engine.clone();
        let result = match engine.lock().begin_render() {
            Ok(frame) => match Self::prepare_renderer_frame(&self.renderer) {
                Ok(()) => {
                    self.pending_frame = Some(frame);
                    self.ready_flag.store(true, Ordering::Release);
                    Ok(())
                }
                Err(err) => {
                    if let Err(end_err) = engine.lock().end_render(frame) {
                        log::error!(
                            "Failed to unwind render frame after renderer init error: {end_err:?}"
                        );
                    }
                    Err(err)
                }
            },
            Err(EngineError::FrameInProgress) => Err(RendererError::InvalidOperation(
                "engine frame already active".into(),
            )),
            Err(err) => Err(RendererError::RenderError(err.to_string())),
        };
        if result.is_err() {
            self.discard_queued_frame_callbacks();
        }
        result
    }

    /// Drop work queued for a frame that could not be started.
    pub fn discard_queued_frame_callbacks(&mut self) {
        if self.pending_frame.is_some() {
            return;
        }
        self.schedule.clear();
    }

    /// Whether this renderer currently owns an acquired engine frame.
    pub fn has_active_frame(&self) -> bool {
        self.pending_frame.is_some()
    }

    /// Present the frame.
    pub fn end_frame(&mut self) -> RendererResult<()> {
        self.end_frame_with_post_render(None)
    }

    /// Runs a caller-borrowed scene completion before submitting this frame.
    /// The callback cannot escape into the next frame or require shared ownership.
    pub fn end_frame_with_post_render(
        &mut self,
        post_render: Option<&mut dyn FnMut(&mut ww3d_engine::RenderFrame) -> RendererResult<()>>,
    ) -> RendererResult<()> {
        self.end_frame_with_scene_policy(false, post_render)
    }

    /// Reusing a viewport capture skips scene work, while the caller still
    /// composites that capture and paints this frame's interface before submit.
    pub fn end_frame_with_scene_policy(
        &mut self,
        skip_scene: bool,
        mut post_render: Option<
            &mut dyn FnMut(&mut ww3d_engine::RenderFrame) -> RendererResult<()>,
        >,
    ) -> RendererResult<()> {
        self.end_frame_with_scoped_native_passes(skip_scene, post_render, None)
    }

    /// Execute native pass markers through one caller-borrowed receiver.
    /// This receiver never escapes into a subsequent frame.
    pub fn end_frame_with_scoped_native_passes(
        &mut self,
        skip_scene: bool,
        mut post_render: Option<
            &mut dyn FnMut(&mut ww3d_engine::RenderFrame) -> RendererResult<()>,
        >,
        mut scoped_native: Option<&mut ScopedNativePassHandler<'_>>,
    ) -> RendererResult<()> {
        let clear = self.config.clear_color;
        let clear_color = wgpu::Color {
            r: clear.x as f64,
            g: clear.y as f64,
            b: clear.z as f64,
            a: clear.w as f64,
        };

        let frame_mesh_stats = if let Some(mut frame) = self.pending_frame.take() {
            let frame_timing = frame.timing;
            self.ww3d.sync(frame_timing.total_time.as_millis() as u32);
            let mut schedule = std::mem::take(&mut self.schedule);
            let mut frame_work_result: RendererResult<_> = Ok(Default::default());
            let mut callback_result: RendererResult<()> = Ok(());
            if !skip_scene {
                // Work before the mesh flush is the frame's foundation: its
                // first error abandons the rest of that work and the meshes.
                // Later work only logs its failures so the frame still submits.
                let mut recorded_before_opaque = false;
                let mut opaque_reached = false;
                let mut foundation_failed = false;
                let mut opaque_failed = false;
                for phase in FRAME_SCHEDULE {
                    match phase {
                        FramePhase::Opaque => {
                            opaque_reached = true;
                            if foundation_failed {
                                continue;
                            }
                            let clear = (!recorded_before_opaque).then_some(clear_color);
                            frame_work_result = self.record_mesh_flush(&mut frame, clear);
                            opaque_failed = frame_work_result.is_err();
                        }
                        FramePhase::Sorted => {
                            if foundation_failed || opaque_failed {
                                continue;
                            }
                            if let Err(err) = self.record_sorted_flush(&mut frame) {
                                frame_work_result = Err(err);
                            }
                        }
                        _ if !opaque_reached => {
                            let work = schedule.take(phase);
                            if foundation_failed {
                                continue;
                            }
                            recorded_before_opaque |= !work.is_empty();
                            for item in work {
                                if let Err(err) =
                                    Self::run_frame_work(item, &mut frame, &mut scoped_native)
                                {
                                    frame_work_result = Err(err);
                                    foundation_failed = true;
                                    break;
                                }
                            }
                        }
                        _ => {
                            for item in schedule.take(phase) {
                                if let Err(err) =
                                    Self::run_frame_work(item, &mut frame, &mut scoped_native)
                                {
                                    log::error!("{phase:?} frame work failed: {err:?}");
                                }
                            }
                        }
                    }
                }
            }
            schedule.clear();
            if let Some(callback) = post_render.as_mut() {
                callback_result = callback(&mut frame);
            }
            let has_window_surface = frame.has_gpu_color_target();
            let end_result = self.engine.lock().end_render(frame)
                .map_err(|err| RendererError::RenderError(err.to_string()));
            let frame_work_result = match (frame_work_result, callback_result) {
                (Ok(stats), Ok(())) => Ok(stats),
                (Err(err), _) | (_, Err(err)) => Err(err),
            };

            match (frame_work_result, end_result) {
                (Ok(stats), Ok(())) => {
                    self.last_surface_presented = has_window_surface;
                    stats
                }
                (Err(work_err), Ok(())) => return Err(work_err),
                (Ok(_), Err(end_err)) => return Err(end_err),
                (Err(work_err), Err(end_err)) => {
                    return Err(RendererError::RenderError(format!(
                        "frame work failed: {work_err:?}; additionally failed to end engine frame: {end_err:?}"
                    )));
                }
            }
        } else {
            return Err(RendererError::NotInitialized(
                "end_frame called without an active engine frame".into(),
            ));
        };

        self.stats.draw_calls = frame_mesh_stats.draw_calls;
        self.stats.meshes_rendered = frame_mesh_stats.meshes_rendered;
        self.stats.triangles_rendered = frame_mesh_stats.triangles_rendered;
        self.stats.material_passes = frame_mesh_stats.material_passes;
        self.stats.texture_switches = frame_mesh_stats.texture_switches;
        self.stats.shader_switches = frame_mesh_stats.shader_switches;
        self.stats.vertex_color_passes = frame_mesh_stats.vertex_color_passes;

        *self.frame_stats_bridge.lock().unwrap_or_else(|p| p.into_inner()) = FrameStats::from(&self.stats);

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

    /// Queue work for `phase` of this renderer's next frame. Native callbacks
    /// retain Send; browser GPU handles follow wgpu's same-thread contract.
    pub fn enqueue_frame_work<F>(&mut self, phase: FramePhase, work: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()>
            + wgpu::WasmNotSend
            + 'static,
    {
        self.schedule.push(phase, FrameWork::Owned(Box::new(work)));
    }

    /// Queue a native pass recorded by the frame's borrowed receiver at `phase`.
    pub fn enqueue_scoped_native_pass(&mut self, phase: FramePhase, pass: ScopedNativePass) {
        self.schedule.push(phase, FrameWork::ScopedNative(pass));
    }

    /// Work items queued for `phase` of the next frame.
    pub fn queued_frame_work(&self, phase: FramePhase) -> usize {
        self.schedule.queued(phase)
    }

    /// Whether any work is queued for the next frame.
    pub fn has_queued_frame_work(&self) -> bool {
        !self.schedule.is_empty()
    }

    fn run_frame_work(
        work: FrameWork,
        frame: &mut ww3d_engine::RenderFrame,
        scoped_native: &mut Option<&mut ScopedNativePassHandler<'_>>,
    ) -> RendererResult<()> {
        match work {
            FrameWork::Owned(work) => work(frame),
            FrameWork::ScopedNative(pass) => match scoped_native.as_deref_mut() {
                Some(receiver) => receiver(pass, frame),
                None => Err(RendererError::InvalidOperation(
                    "native scene pass has no receiving frame owner".into(),
                )),
            },
        }
    }

    /// `FramePhase::Opaque`: the W3D mesh flush.
    fn record_mesh_flush(
        &mut self,
        frame: &mut ww3d_engine::RenderFrame,
        clear: Option<wgpu::Color>,
    ) -> RendererResult<crate::rendering::mesh_system::MeshRenderStats> {
        let (stats, shadow_submissions) = {
            let mut renderer = self.renderer.lock().map_err(|_| {
                RendererError::InvalidOperation("renderer mutex poisoned".into())
            })?;
            renderer.render_frame(frame, clear, None)?;
            (
                renderer.mesh_stats().clone(),
                renderer.take_pending_shadow_caster_submissions(),
            )
        };
        self.sync_shadow_submissions(shadow_submissions);
        Ok(stats)
    }

    /// `FramePhase::Sorted`: the sorting-renderer flush.
    fn record_sorted_flush(&mut self, frame: &mut ww3d_engine::RenderFrame) -> RendererResult<()> {
        let mut renderer = self.renderer.lock().map_err(|_| {
            RendererError::InvalidOperation("renderer mutex poisoned".into())
        })?;
        renderer.render_sorted_frame(frame)
    }

    /// Resize the underlying surface/headless target.
    pub fn resize(&mut self, width: u32, height: u32) -> RendererResult<()> {
        self.engine.lock().resize(width, height);
        Ok(())
    }

    /// Access the underlying device.
    pub fn device(&self) -> Arc<Device> {
        self.engine.lock().device()
    }

    /// Access the underlying queue.
    pub fn queue(&self) -> Arc<Queue> {
        self.engine.lock().queue()
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
        *self.frame_stats_bridge.lock().unwrap_or_else(|p| p.into_inner()) = FrameStats::from(&self.stats);
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

impl Drop for WgpuMainRenderer {
    fn drop(&mut self) {
        if self.registered_with_ww3d {
            self.ww3d.unregister_renderer();
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
    sorting_enabled: bool,
    static_sort_enabled: bool,
    decals_enabled: bool,
    /// THREAD: same shared renderer handle as [`WgpuMainRenderer::renderer`] —
    /// the host frame pump reaches the renderer through the registered backend
    /// via this handle, so game thread and host meet here.
    _renderer: SharedRendererHandle,
    renderer_3d: Arc<Renderer3dRuntime>,
}

impl WgpuCoreBridge {
    fn new(
        stats: Arc<Mutex<FrameStats>>,
        ready: Arc<AtomicBool>,
        renderer: SharedRendererHandle,
        renderer_3d: Arc<Renderer3dRuntime>,
    ) -> Self {
        Self {
            stats,
            ready,
            sorting_enabled: true,
            static_sort_enabled: true,
            decals_enabled: true,
            _renderer: renderer,
            renderer_3d,
        }
    }

    pub(crate) fn renderer_handle(&self) -> SharedRendererHandle {
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
        self.stats.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn set_sorting_enabled(&mut self, enabled: bool) -> W3DResult<()> {
        self.sorting_enabled = enabled;
        mesh_system::StaticSortManager::set_sorting_enabled(&self.renderer_3d, enabled);
        Ok(())
    }

    fn is_sorting_enabled(&self) -> bool {
        self.sorting_enabled
    }

    fn set_static_sort_lists_enabled(&mut self, enabled: bool) -> W3DResult<()> {
        self.static_sort_enabled = enabled;
        mesh_system::StaticSortManager::set_static_sort_lists_enabled(&self.renderer_3d, enabled);
        Ok(())
    }

    fn are_static_sort_lists_enabled(&self) -> bool {
        self.static_sort_enabled
    }

    fn set_decals_enabled(&mut self, enabled: bool) -> W3DResult<()> {
        self.decals_enabled = enabled;
        mesh_system::StaticSortManager::set_decals_enabled(&self.renderer_3d, enabled);
        Ok(())
    }

    fn are_decals_enabled(&self) -> bool {
        self.decals_enabled
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
                mesh_system::StaticSortManager::add_to_static_sort_list(&self.renderer_3d, handle, sort_level);
                Ok(())
            }
            Err(object) => match object.downcast::<MeshClass>() {
                Ok(mesh) => {
                    let handle = StaticSortRenderObject::from_arc(Arc::clone(&mesh));
                    mesh_system::StaticSortManager::add_to_static_sort_list_with_mesh(
                        &self.renderer_3d,
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
        let _guard = mesh_system::StaticSortManager::begin_flush(Arc::clone(&self.renderer_3d));
        mesh_system::StaticSortManager::flush_static_sort_list(&self.renderer_3d);
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

#[cfg(test)]
#[path = "wgpu_main_renderer_callback_tests.rs"]
mod callback_tests;

#[cfg(test)]
#[path = "wgpu_scoped_native_pass_tests.rs"]
mod scoped_native_pass_tests;
