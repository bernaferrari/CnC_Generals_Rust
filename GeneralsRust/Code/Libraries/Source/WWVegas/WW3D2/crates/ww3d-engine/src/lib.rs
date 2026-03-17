//! WW3D Engine lifecycle management and subsystem integration.
//!
//! This crate provides the complete WW3D engine with all integrated subsystems:
//! - Asset loading and management (ww3d-assets)
//! - Scene graph and rendering (ww3d-scene)
//! - Animation system (ww3d-animation)
//! - 3D rendering pipeline (ww3d-renderer-3d)
//! - Effects and particles (ww3d-effects)
//! - Collision detection (ww3d-collision)
//!
//! The engine orchestrates all subsystems and provides a unified game loop
//! with proper frame timing, update phases, and rendering.

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use log::info;
use ww3d_core::ensure_class_registry_initialized;
use ww3d_gpu::{GpuDevice, GpuError};

// Re-export core types
pub use glam;
pub use ww3d_gpu::wgpu;

/// Shared result type for engine operations.
pub type EngineResult<T> = Result<T, EngineError>;

/// High-level configuration used when initialising the renderer.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Desired render width in pixels.
    pub width: u32,
    /// Desired render height in pixels.
    pub height: u32,
    /// Preferred backend selection (Vulkan/Metal/DirectX/WebGPU).
    pub backends: wgpu::Backends,
    /// GPU power preference (discrete vs integrated).
    pub power_preference: wgpu::PowerPreference,
    /// Requested presentation mode. If `None`, the best available mode is chosen.
    pub present_mode: Option<wgpu::PresentMode>,
    /// Desired surface format. If `None`, a SRGB format is selected when possible.
    pub color_format: Option<wgpu::TextureFormat>,
    /// External texture usage flags for the swapchain.
    pub surface_usage: wgpu::TextureUsages,
    /// Requested depth buffer support.
    pub enable_depth: bool,
    /// Depth-stencil format when depth is enabled.
    pub depth_format: wgpu::TextureFormat,
    /// Multisample count for the back buffer.
    pub sample_count: u32,
    /// Features required when requesting the device.
    pub features: wgpu::Features,
    /// Explicit limits to request from the device.
    pub limits: wgpu::Limits,
    /// Shader compiler to use when running on DirectX 12.
    pub dx12_shader_compiler: wgpu::Dx12Compiler,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            backends: wgpu::Backends::all(),
            power_preference: wgpu::PowerPreference::HighPerformance,
            present_mode: Some(wgpu::PresentMode::Fifo),
            color_format: None,
            surface_usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            enable_depth: true,
            depth_format: wgpu::TextureFormat::Depth32Float,
            sample_count: 1,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits {
                // WW3D mesh shaders bind groups 0..7 (camera/model/uv-texture-color).
                max_bind_groups: 8,
                ..wgpu::Limits::default()
            },
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
        }
    }
}

impl EngineConfig {
    fn instance_descriptor(&self) -> wgpu::InstanceDescriptor {
        let mut backend_options = wgpu::BackendOptions::default();
        backend_options.dx12.shader_compiler = self.dx12_shader_compiler.clone();

        wgpu::InstanceDescriptor {
            backends: self.backends,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: Default::default(),
            backend_options,
        }
    }
}

/// Errors raised by engine lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("WW3D engine is already initialised")]
    AlreadyInitialised,
    #[error("WW3D engine has not been initialised")]
    NotInitialised,
    #[error("No compatible GPU adapter was found")]
    NoAdapter,
    #[error("Failed to create WGPU surface: {0}")]
    SurfaceCreation(String),
    #[error(transparent)]
    Surface(#[from] wgpu::SurfaceError),
    #[error(transparent)]
    Gpu(#[from] GpuError),
    #[error("Screenshot failed: {0}")]
    Screenshot(String),
    #[error("A frame is already in progress")]
    FrameInProgress,
    #[error("No frame is currently in progress")]
    NoFrameInProgress,
    #[error("Asset loading failed: {0}")]
    AssetLoad(String),
    #[error("Rendering failed: {0}")]
    Rendering(String),
    #[error("Animation failed: {0}")]
    Animation(String),
    #[error("Collision failed: {0}")]
    Collision(String),
}

/// Runtime settings that mirror toggles in the legacy C++ path.
#[derive(Debug, Clone)]
struct EngineSettings {
    swap_interval: i32,
    texture_reduction: u32,
    movie_capture: MovieCaptureState,
    pending_screenshots: VecDeque<ScreenshotRequest>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            swap_interval: 1,
            texture_reduction: 0,
            movie_capture: MovieCaptureState::default(),
            pending_screenshots: VecDeque::new(),
        }
    }
}

/// Movie capture control state.
#[derive(Debug, Clone)]
struct MovieCaptureState {
    enabled: bool,
    single_frame_pending: bool,
    _frame_rate: f32,
    output_dir: PathBuf,
    base_name: String,
    frame_counter: u64,
}

impl Default for MovieCaptureState {
    fn default() -> Self {
        Self {
            enabled: false,
            single_frame_pending: false,
            _frame_rate: 30.0,
            output_dir: PathBuf::from("captures"),
            base_name: "ww3d_frame".to_string(),
            frame_counter: 0,
        }
    }
}

impl MovieCaptureState {
    fn next_capture_path(&mut self) -> PathBuf {
        let path = self
            .output_dir
            .join(format!("{}_{}.png", self.base_name, self.frame_counter));
        self.frame_counter = self.frame_counter.wrapping_add(1);
        path
    }

    fn configure_output<P: AsRef<Path>>(&mut self, directory: P, base_name: impl Into<String>) {
        self.output_dir = directory.as_ref().to_path_buf();
        self.base_name = base_name.into();
        self.frame_counter = 0;
    }
}

#[derive(Debug, Clone)]
struct ScreenshotRequest {
    path: PathBuf,
}

impl ScreenshotRequest {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

struct ScreenshotReadback {
    path: PathBuf,
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    row_size: usize,
    padded_row_size: usize,
    completion: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

impl std::fmt::Debug for ScreenshotReadback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenshotReadback")
            .field("path", &self.path)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &self.format)
            .finish()
    }
}

/// Internal representation of the active rendering surface.
#[derive(Debug)]
enum SurfaceMode {
    Windowed(WindowSurfaceState),
    Headless(HeadlessTarget),
}

/// Swapchain-backed rendering surface.
#[derive(Debug)]
struct WindowSurfaceState {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth: Option<DepthTarget>,
}

impl WindowSurfaceState {
    fn new(
        surface: wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        config: &EngineConfig,
    ) -> Self {
        let capabilities = surface.get_capabilities(adapter);

        let format = select_surface_format(&capabilities.formats, config.color_format);
        let present_mode = select_present_mode(&capabilities.present_modes, config.present_mode);
        let alpha_mode = select_alpha_mode(&capabilities.alpha_modes);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: config.surface_usage,
            format,
            width: config.width.max(1),
            height: config.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        info!(
            "WW3D surface configured: size={}x{} format={:?} present_mode={:?} alpha_mode={:?}",
            surface_config.width,
            surface_config.height,
            surface_config.format,
            surface_config.present_mode,
            surface_config.alpha_mode
        );

        let depth = config.enable_depth.then(|| {
            DepthTarget::new(
                device,
                config.width,
                config.height,
                config.depth_format,
                config.sample_count,
            )
        });

        surface.configure(device, &surface_config);

        Self {
            surface,
            config: surface_config,
            depth,
        }
    }

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if self.config.width == width && self.config.height == height {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);

        if let Some(depth) = &mut self.depth {
            depth.resize(device, width, height);
        }
    }

    fn color_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    fn depth_view(&self) -> Option<Arc<wgpu::TextureView>> {
        self.depth.as_ref().map(|d| d.view())
    }
}

/// Off-screen render target used when no surface is supplied.
#[derive(Debug)]
struct HeadlessTarget {
    color: wgpu::Texture,
    color_view: Arc<wgpu::TextureView>,
    depth: Option<DepthTarget>,
    format: wgpu::TextureFormat,
    sample_count: u32,
}

impl HeadlessTarget {
    fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        sample_count: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("WW3D Headless Color Target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let color_view = Arc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));

        let depth =
            depth_format.map(|fmt| DepthTarget::new(device, width, height, fmt, sample_count));

        Self {
            color: texture,
            color_view,
            depth,
            format,
            sample_count,
        }
    }

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        let new_color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("WW3D Headless Color Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.color = new_color;
        self.color_view = Arc::new(
            self.color
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );

        if let Some(depth) = &mut self.depth {
            depth.resize(device, width, height);
        }
    }

    fn color_view(&self) -> Arc<wgpu::TextureView> {
        Arc::clone(&self.color_view)
    }

    fn depth_view(&self) -> Option<Arc<wgpu::TextureView>> {
        self.depth.as_ref().map(|d| d.view())
    }

    fn texture(&self) -> &wgpu::Texture {
        &self.color
    }

    fn size(&self) -> (u32, u32) {
        let extent = self.color.size();
        (extent.width, extent.height)
    }
}

/// Depth buffer wrapper.
#[derive(Debug)]
struct DepthTarget {
    _texture: wgpu::Texture,
    view: Arc<wgpu::TextureView>,
    format: wgpu::TextureFormat,
    sample_count: u32,
}

impl DepthTarget {
    fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("WW3D Depth Target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = Arc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));

        Self {
            _texture: texture,
            view,
            format,
            sample_count,
        }
    }

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height, self.format, self.sample_count);
    }

    fn view(&self) -> Arc<wgpu::TextureView> {
        Arc::clone(&self.view)
    }
}

/// Frame timing information for game logic updates
/// Matches C++ WW3D::Sync() timing system from ww3d.cpp:1097-1101
#[derive(Debug, Clone, Copy)]
pub struct FrameTiming {
    /// Current frame number (matches WW3D::FrameCount)
    pub frame_number: u64,
    /// Time elapsed since last frame (delta time)
    pub delta_time: Duration,
    /// Total elapsed time since engine start
    pub total_time: Duration,
    /// Current frames per second
    pub fps: f32,
    /// Time when this frame started
    pub frame_start: Instant,
    /// Current sync time in milliseconds (matches WW3D::SyncTime)
    /// Reference: ww3d.h:291
    pub sync_time: u32,
    /// Previous sync time in milliseconds (matches WW3D::PreviousSyncTime)
    /// Reference: ww3d.h:297
    pub previous_sync_time: u32,
}

impl FrameTiming {
    /// Get delta time as seconds (f32)
    pub fn delta_seconds(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }

    /// Get total time as seconds (f32)
    pub fn total_seconds(&self) -> f32 {
        self.total_time.as_secs_f32()
    }

    /// Get frame time in milliseconds (matches WW3D::Get_Frame_Time())
    /// Reference: ww3d.h:136
    pub fn frame_time_ms(&self) -> u32 {
        self.sync_time.wrapping_sub(self.previous_sync_time)
    }
}

/// Input event types for game logic
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Keyboard key pressed
    KeyPressed { key: String },
    /// Keyboard key released
    KeyReleased { key: String },
    /// Mouse button pressed
    MousePressed { button: u32, x: f32, y: f32 },
    /// Mouse button released
    MouseReleased { button: u32, x: f32, y: f32 },
    /// Mouse moved
    MouseMoved { x: f32, y: f32 },
    /// Mouse scrolled
    MouseScrolled { delta: f32 },
}

/// Callback trait for handling input events
pub trait InputHandler: Send + Sync {
    fn handle_input(&mut self, event: &InputEvent);
}

/// Trait for updateable subsystems
pub trait Subsystem: Send + Sync {
    /// Update the subsystem with frame timing
    fn update(&mut self, timing: &FrameTiming);

    /// Get a debug name for this subsystem
    fn name(&self) -> &str {
        "Unknown Subsystem"
    }
}

/// Container for all game subsystems
pub struct EngineSubsystems {
    /// Registered subsystems
    subsystems: Vec<Box<dyn Subsystem>>,
    /// Input event queue
    input_events: VecDeque<InputEvent>,
    /// Optional input handler callback
    input_handler: Option<Box<dyn InputHandler>>,
}

impl std::fmt::Debug for EngineSubsystems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineSubsystems")
            .field("subsystem_count", &self.subsystems.len())
            .field("has_input_handler", &self.input_handler.is_some())
            .finish()
    }
}

impl Default for EngineSubsystems {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineSubsystems {
    /// Create new subsystems container
    pub fn new() -> Self {
        Self {
            subsystems: Vec::new(),
            input_events: VecDeque::new(),
            input_handler: None,
        }
    }

    /// Register a subsystem
    pub fn register_subsystem(&mut self, subsystem: Box<dyn Subsystem>) {
        self.subsystems.push(subsystem);
    }

    /// Set an input handler for processing input events
    pub fn set_input_handler(&mut self, handler: Box<dyn InputHandler>) {
        self.input_handler = Some(handler);
    }

    /// Queue an input event for processing
    pub fn queue_input(&mut self, event: InputEvent) {
        self.input_events.push_back(event);
    }

    /// Process all queued input events
    pub fn process_input(&mut self) {
        if let Some(handler) = &mut self.input_handler {
            while let Some(event) = self.input_events.pop_front() {
                handler.handle_input(&event);
            }
        } else {
            // Clear events if no handler is set
            self.input_events.clear();
        }
    }

    /// Update all registered subsystems with frame timing
    pub fn update(&mut self, timing: &FrameTiming) {
        // Update all registered subsystems
        for subsystem in &mut self.subsystems {
            subsystem.update(timing);
        }

        // Process input events
        self.process_input();
    }

    /// Get the number of registered subsystems
    pub fn subsystem_count(&self) -> usize {
        self.subsystems.len()
    }
}

/// Per-frame render context returned by `Begin_Render`.
#[derive(Debug)]
pub struct RenderFrame {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    encoder: wgpu::CommandEncoder,
    color_view: Arc<wgpu::TextureView>,
    depth_view: Option<Arc<wgpu::TextureView>>,
    surface_texture: Option<wgpu::SurfaceTexture>,
    _start_time: Instant,
    frame_index: u64,
    color_format: wgpu::TextureFormat,
    /// Frame timing information
    pub timing: FrameTiming,
}

impl RenderFrame {
    /// Borrow the command encoder to record render passes.
    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        &mut self.encoder
    }

    /// Fetch the render target view for this frame.
    pub fn color_view(&self) -> &wgpu::TextureView {
        self.color_view.as_ref()
    }

    /// Clone the render target view handle for situations that need owned access.
    pub fn color_view_arc(&self) -> Arc<wgpu::TextureView> {
        Arc::clone(&self.color_view)
    }

    /// Fetch the depth view when depth is enabled.
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.depth_view.as_deref()
    }

    /// Clone the depth target view when available.
    pub fn depth_view_arc(&self) -> Option<Arc<wgpu::TextureView>> {
        self.depth_view.as_ref().map(Arc::clone)
    }

    /// Provide read-only access to the device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Provide read-only access to the queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Frame sequence number, matching the DX8 stats counter.
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Back buffer color format.
    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }
}

/// FPS calculation helper
#[derive(Debug, Clone)]
struct FpsCounter {
    frame_times: VecDeque<Instant>,
    current_fps: f32,
    update_interval: Duration,
    last_update: Instant,
}

impl FpsCounter {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            frame_times: VecDeque::with_capacity(60),
            current_fps: 0.0,
            update_interval: Duration::from_millis(500),
            last_update: now,
        }
    }

    fn record_frame(&mut self, frame_time: Instant) {
        self.frame_times.push_back(frame_time);

        // Keep only last second of frames
        let cutoff = frame_time - Duration::from_secs(1);
        while let Some(&oldest) = self.frame_times.front() {
            if oldest < cutoff {
                self.frame_times.pop_front();
            } else {
                break;
            }
        }

        // Update FPS periodically
        if frame_time.duration_since(self.last_update) >= self.update_interval {
            if self.frame_times.len() > 1 {
                let elapsed = frame_time.duration_since(self.frame_times[0]).as_secs_f32();
                if elapsed > 0.0 {
                    self.current_fps = (self.frame_times.len() as f32 - 1.0) / elapsed;
                }
            }
            self.last_update = frame_time;
        }
    }

    fn fps(&self) -> f32 {
        self.current_fps
    }
}

/// In-memory state of the running engine.
/// Matches C++ WW3D class from ww3d.h and ww3d.cpp
#[derive(Debug)]
pub struct Engine {
    _instance: Arc<wgpu::Instance>,
    _adapter: wgpu::Adapter,
    adapter_info: wgpu::AdapterInfo,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    gpu_device: Arc<GpuDevice>,
    surface: SurfaceMode,
    settings: EngineSettings,
    frame_active: bool,
    frame_index: u64,
    last_frame_start: Option<Instant>,
    color_format: wgpu::TextureFormat,
    _depth_format: Option<wgpu::TextureFormat>,
    /// All integrated subsystems
    pub subsystems: EngineSubsystems,
    /// Engine start time for total elapsed time tracking
    start_time: Instant,
    /// FPS calculation state
    fps_counter: FpsCounter,
    /// Current sync time in milliseconds (matches WW3D::SyncTime)
    /// Reference: ww3d.cpp:87, ww3d.h:291
    sync_time: u32,
    /// Previous sync time in milliseconds (matches WW3D::PreviousSyncTime)
    /// Reference: ww3d.cpp:88, ww3d.h:297
    previous_sync_time: u32,
    pending_screenshot_readbacks: Vec<ScreenshotReadback>,
}

impl Engine {
    /// Create a windowed engine instance using the supplied surface target.
    /// Reference: ww3d.cpp:185-234 (WW3D::Init)
    ///
    /// This initializes the WW3D engine with a window surface for rendering.
    ///
    /// The C++ Init() function performs:
    /// - DirectX 8 wrapper initialization
    /// - Debug resource allocation
    /// - High-resolution timer setup (timeBeginPeriod)
    /// - Dazzle system initialization from INI file
    /// - Default static sort lists creation
    /// - Animation-triggered sound system initialization
    ///
    /// The Rust version modernizes this by:
    /// - Using wgpu instead of DirectX 8
    /// - Async device initialization
    /// - Thread-safe resource management
    /// - Cross-platform compatibility
    pub async fn new_windowed<W>(window: W, config: EngineConfig) -> EngineResult<Self>
    where
        W: Into<wgpu::SurfaceTarget<'static>> + Send + Sync + 'static,
    {
        ensure_class_registry_initialized();
        let instance = Arc::new(wgpu::Instance::new(&config.instance_descriptor()));
        let surface_target = window.into();
        let surface = instance
            .create_surface(surface_target)
            .map_err(|err| EngineError::SurfaceCreation(format!("{err:?}")))?;

        let adapter = request_adapter(&instance, Some(&surface), config.power_preference).await?;
        let adapter_info = adapter.get_info();

        let gpu_device = Arc::new(
            GpuDevice::create_device(&adapter, config.features, config.limits.clone())
                .await
                .map_err(EngineError::Gpu)?,
        );

        let device = gpu_device.device_arc();
        let queue = gpu_device.queue_arc();

        let surface_state = WindowSurfaceState::new(surface, &adapter, &device, &config);
        let color_format = surface_state.color_format();
        let depth_format = config.enable_depth.then_some(config.depth_format);

        let now = Instant::now();

        Ok(Self {
            _instance: instance,
            _adapter: adapter,
            adapter_info,
            device,
            queue,
            gpu_device,
            surface: SurfaceMode::Windowed(surface_state),
            settings: EngineSettings::default(),
            frame_active: false,
            frame_index: 0,
            last_frame_start: None,
            color_format,
            _depth_format: depth_format,
            subsystems: EngineSubsystems::new(),
            start_time: now,
            fps_counter: FpsCounter::new(),
            // Initialize sync time to 0 (matches C++ ww3d.cpp:87-88)
            sync_time: 0,
            previous_sync_time: 0,
            pending_screenshot_readbacks: Vec::new(),
        })
    }

    /// Create a headless engine instance that renders into an off-screen texture.
    ///
    /// This creates an engine without a window surface, useful for:
    /// - Server-side rendering
    /// - Automated testing
    /// - Screenshot/video generation
    /// - Thumbnail generation
    ///
    /// Note: The C++ version does not support headless mode. This is a Rust
    /// enhancement that provides additional flexibility.
    pub async fn new_headless(config: EngineConfig) -> EngineResult<Self> {
        ensure_class_registry_initialized();
        let instance = Arc::new(wgpu::Instance::new(&config.instance_descriptor()));
        let adapter = request_adapter(&instance, None, config.power_preference).await?;
        let adapter_info = adapter.get_info();

        let gpu_device = Arc::new(
            GpuDevice::create_device(&adapter, config.features, config.limits.clone())
                .await
                .map_err(EngineError::Gpu)?,
        );

        let device = gpu_device.device_arc();
        let queue = gpu_device.queue_arc();

        let format = config
            .color_format
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let depth_format = config.enable_depth.then_some(config.depth_format);

        let target = HeadlessTarget::new(
            &device,
            config.width,
            config.height,
            format,
            depth_format,
            config.sample_count,
        );

        let now = Instant::now();

        Ok(Self {
            _instance: instance,
            _adapter: adapter,
            adapter_info,
            device,
            queue,
            gpu_device,
            surface: SurfaceMode::Headless(target),
            settings: EngineSettings::default(),
            frame_active: false,
            frame_index: 0,
            last_frame_start: None,
            color_format: format,
            _depth_format: depth_format,
            subsystems: EngineSubsystems::new(),
            start_time: now,
            fps_counter: FpsCounter::new(),
            // Initialize sync time to 0 (matches C++ ww3d.cpp:87-88)
            sync_time: 0,
            previous_sync_time: 0,
            pending_screenshot_readbacks: Vec::new(),
        })
    }

    /// Update all subsystems (should be called before begin_render each frame)
    pub fn update(&mut self) -> EngineResult<()> {
        let now = Instant::now();
        let delta_time = self
            .last_frame_start
            .map(|last| now.duration_since(last))
            .unwrap_or(Duration::from_secs_f32(1.0 / 60.0));

        let total_time = now.duration_since(self.start_time);

        self.fps_counter.record_frame(now);

        let timing = FrameTiming {
            frame_number: self.frame_index,
            delta_time,
            total_time,
            fps: self.fps_counter.fps(),
            frame_start: now,
            sync_time: self.sync_time,
            previous_sync_time: self.previous_sync_time,
        };

        // Update all subsystems
        self.subsystems.update(&timing);

        Ok(())
    }

    /// Start a new frame (matches WW3D::Begin_Render())
    /// Reference: ww3d.cpp:708-779, ww3d.h:115
    ///
    /// This function marks the start of rendering for a new frame. It must be paired
    /// with a call to end_render(). Between these two calls, you can render scenes
    /// and objects.
    ///
    /// The C++ version performs:
    /// - Device cooperative level testing (D3D specific)
    /// - Memory allocation statistics tracking
    /// - Texture loader updates
    /// - Dynamic buffer resets
    /// - Movie capture frame grabbing
    /// - Viewport setup for clearing
    /// - Scene begin
    pub fn begin_render(&mut self) -> EngineResult<RenderFrame> {
        if self.frame_active {
            return Err(EngineError::FrameInProgress);
        }

        self.frame_index = self.frame_index.wrapping_add(1);
        let start_time = Instant::now();

        // Calculate timing information
        let delta_time = self
            .last_frame_start
            .map(|last| start_time.duration_since(last))
            .unwrap_or(Duration::from_secs_f32(1.0 / 60.0));

        let total_time = start_time.duration_since(self.start_time);

        self.fps_counter.record_frame(start_time);

        let timing = FrameTiming {
            frame_number: self.frame_index,
            delta_time,
            total_time,
            fps: self.fps_counter.fps(),
            frame_start: start_time,
            sync_time: self.sync_time,
            previous_sync_time: self.previous_sync_time,
        };

        self.last_frame_start = Some(start_time);

        let encoder = self
            .gpu_device
            .create_command_encoder(Some("WW3D Frame Encoder"));

        match &mut self.surface {
            SurfaceMode::Windowed(surface_state) => {
                let frame = surface_state.surface.get_current_texture()?;
                let view = Arc::new(
                    frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                );

                let depth_view = surface_state.depth_view();

                self.frame_active = true;
                Ok(RenderFrame {
                    device: Arc::clone(&self.device),
                    queue: Arc::clone(&self.queue),
                    encoder,
                    color_view: view,
                    depth_view,
                    surface_texture: Some(frame),
                    _start_time: start_time,
                    frame_index: self.frame_index,
                    color_format: self.color_format,
                    timing,
                })
            }
            SurfaceMode::Headless(target) => {
                let color_view = target.color_view();
                let depth_view = target.depth_view();

                self.frame_active = true;
                Ok(RenderFrame {
                    device: Arc::clone(&self.device),
                    queue: Arc::clone(&self.queue),
                    encoder,
                    color_view,
                    depth_view,
                    surface_texture: None,
                    _start_time: start_time,
                    frame_index: self.frame_index,
                    color_format: self.color_format,
                    timing,
                })
            }
        }
    }

    /// Complete the current frame and present/capture it (matches WW3D::End_Render())
    /// Reference: ww3d.cpp:999-1041, ww3d.h:122
    ///
    /// This function marks the completion of a frame. It submits all rendering commands,
    /// processes screenshots, and presents the frame to the screen (if windowed).
    ///
    /// The C++ version performs:
    /// - Sorting renderer flush
    /// - D3D End_Scene
    /// - Frame count increment
    /// - Debug statistics end
    /// - Render state cache invalidation
    pub fn end_render(&mut self, frame: RenderFrame) -> EngineResult<()> {
        if !self.frame_active {
            return Err(EngineError::NoFrameInProgress);
        }
        // The frame token is consumed by this call. Always drop active-frame state even if
        // submission/present/capture paths error, otherwise one failed frame poisons all future
        // begin_render calls with FrameInProgress.
        self.frame_active = false;

        let RenderFrame {
            device: _,
            queue: _,
            encoder,
            color_view: _,
            depth_view: _,
            mut surface_texture,
            _start_time: _,
            frame_index: _,
            color_format: _,
            timing: _,
        } = frame;

        let command_buffer = encoder.finish();
        self.queue.submit(std::iter::once(command_buffer));

        self.process_pending_screenshots(surface_texture.as_ref())?;

        if let Some(texture) = surface_texture.take() {
            self.gpu_device.present_surface_texture(texture);
        } else if self.surface_requires_capture() {
            self.capture_headless_frame()?;
        }

        Ok(())
    }

    /// Get current frame timing information
    pub fn timing(&self) -> FrameTiming {
        let now = Instant::now();
        let delta_time = self
            .last_frame_start
            .map(|last| now.duration_since(last))
            .unwrap_or(Duration::from_secs_f32(1.0 / 60.0));
        let total_time = now.duration_since(self.start_time);

        FrameTiming {
            frame_number: self.frame_index,
            delta_time,
            total_time,
            fps: self.fps_counter.fps(),
            frame_start: now,
            sync_time: self.sync_time,
            previous_sync_time: self.previous_sync_time,
        }
    }

    /// Set the sync time for this frame (matches WW3D::Sync())
    /// Reference: ww3d.cpp:1097-1101, ww3d.h:134
    ///
    /// This function is used to synchronize the engine's internal time with the game's
    /// logical time. The application should call this at the start of every frame with
    /// the current game time in milliseconds.
    ///
    /// # Arguments
    /// * `sync_time` - Current game time in milliseconds
    pub fn sync(&mut self, sync_time: u32) {
        // Matches C++ ww3d.cpp:1097-1101:
        // void WW3D::Sync(unsigned int sync_time)
        // {
        //     PreviousSyncTime = SyncTime;
        //     SyncTime = sync_time;
        // }
        self.previous_sync_time = self.sync_time;
        self.sync_time = sync_time;
    }

    /// Get the current sync time in milliseconds (matches WW3D::Get_Sync_Time())
    /// Reference: ww3d.h:135
    pub fn get_sync_time(&self) -> u32 {
        self.sync_time
    }

    /// Get the frame time (delta) in milliseconds (matches WW3D::Get_Frame_Time())
    /// Reference: ww3d.h:136
    pub fn get_frame_time(&self) -> u32 {
        self.sync_time.wrapping_sub(self.previous_sync_time)
    }

    /// Get the current frame count (matches WW3D::Get_Frame_Count())
    /// Reference: ww3d.h:137
    pub fn get_frame_count(&self) -> u64 {
        self.frame_index
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        self.fps_counter.fps()
    }

    /// Get total elapsed time since engine start
    pub fn elapsed_time(&self) -> Duration {
        Instant::now().duration_since(self.start_time)
    }

    /// Queue an input event for processing
    pub fn queue_input(&mut self, event: InputEvent) {
        self.subsystems.queue_input(event);
    }

    /// Set an input handler
    pub fn set_input_handler(&mut self, handler: Box<dyn InputHandler>) {
        self.subsystems.set_input_handler(handler);
    }

    /// Register a subsystem
    pub fn register_subsystem(&mut self, subsystem: Box<dyn Subsystem>) {
        self.subsystems.register_subsystem(subsystem);
    }

    /// Get a reference to the subsystems container
    pub fn subsystems(&self) -> &EngineSubsystems {
        &self.subsystems
    }

    /// Get a mutable reference to the subsystems container
    pub fn subsystems_mut(&mut self) -> &mut EngineSubsystems {
        &mut self.subsystems
    }

    /// Resize the active surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        match &mut self.surface {
            SurfaceMode::Windowed(surface) => {
                surface.resize(&self.device, width, height);
                self.color_format = surface.color_format();
            }
            SurfaceMode::Headless(target) => {
                target.resize(&self.device, width, height);
            }
        }
    }

    /// Configure the swap interval (VSync) equivalent.
    pub fn set_swap_interval(&mut self, interval: i32) {
        self.settings.swap_interval = interval.max(0);
    }

    /// Retrieve the swap interval.
    pub fn swap_interval(&self) -> i32 {
        self.settings.swap_interval
    }

    /// Apply a texture reduction factor (mip bias) as the DX8 renderer exposed.
    pub fn set_texture_reduction(&mut self, reduction: u32) {
        self.settings.texture_reduction = reduction;
    }

    /// Current texture reduction factor.
    pub fn texture_reduction(&self) -> u32 {
        self.settings.texture_reduction
    }

    /// Enable or disable movie capture.
    pub fn set_movie_capture_enabled(&mut self, enabled: bool) {
        if enabled && !self.settings.movie_capture.enabled {
            self.settings.movie_capture.frame_counter = 0;
        }
        self.settings.movie_capture.enabled = enabled;
    }

    /// Queue a single-frame movie capture (matches `Start_Single_Frame_Movie_Capture`).
    pub fn request_single_frame_capture(&mut self) {
        self.settings.movie_capture.single_frame_pending = true;
    }

    /// Configure movie capture output directory and base filename.
    pub fn set_movie_capture_output(&mut self, directory: PathBuf, base_name: String) {
        self.settings
            .movie_capture
            .configure_output(directory, base_name);
    }

    fn surface_requires_capture(&self) -> bool {
        matches!(self.surface, SurfaceMode::Headless(_))
            && (self.settings.movie_capture.enabled
                || self.settings.movie_capture.single_frame_pending)
    }

    fn capture_headless_frame(&mut self) -> EngineResult<()> {
        // Headless capture will blit the colour target into a CPU-visible buffer.
        // The actual read-back pipeline will be implemented when the movie/screenshot
        // infrastructure is ready. For now we just clear the single-frame flag so
        // successive frames are not queued endlessly.
        self.settings.movie_capture.single_frame_pending = false;
        Ok(())
    }

    /// Query adapter properties for feature parity reporting.
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    /// Expose the underlying device for systems that still expect direct access.
    pub fn device(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    /// Expose the underlying queue.
    pub fn queue(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Expose the higher-level GPU device wrapper used by renderer modules.
    pub fn gpu_device(&self) -> Arc<GpuDevice> {
        Arc::clone(&self.gpu_device)
    }

    /// Back buffer color format advertised to renderers.
    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }

    /// Depth-stencil format currently configured, if any.
    pub fn depth_format(&self) -> Option<wgpu::TextureFormat> {
        self._depth_format
    }

    fn queue_screenshot(&mut self, path: PathBuf) {
        if path.to_string_lossy().contains("generals_internal_frame") {
            eprintln!("DEBUG_SCREENSHOT: queued path={}", path.display());
        }
        self.settings
            .pending_screenshots
            .push_back(ScreenshotRequest::new(path));
    }

    /// Current logical surface size in pixels.
    pub fn surface_size(&self) -> (u32, u32) {
        match &self.surface {
            SurfaceMode::Windowed(surface) => (surface.config.width, surface.config.height),
            SurfaceMode::Headless(target) => target.size(),
        }
    }

    fn process_pending_screenshots(
        &mut self,
        surface_texture: Option<&wgpu::SurfaceTexture>,
    ) -> EngineResult<()> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.flush_pending_screenshot_readbacks();

        let mut requests = Vec::new();
        while let Some(request) = self.settings.pending_screenshots.pop_front() {
            requests.push(request);
        }

        if self.settings.movie_capture.enabled || self.settings.movie_capture.single_frame_pending {
            let path = self.settings.movie_capture.next_capture_path();
            requests.push(ScreenshotRequest::new(path));
            self.settings.movie_capture.single_frame_pending = false;
        }

        if requests.is_empty() {
            return Ok(());
        }

        match &self.surface {
            SurfaceMode::Windowed(_) => {
                let Some(surface_texture) = surface_texture else {
                    for request in &requests {
                        if request.path.to_string_lossy().contains("generals_internal_frame") {
                            eprintln!(
                                "DEBUG_SCREENSHOT: deferred_no_surface path={}",
                                request.path.display()
                            );
                        }
                    }
                    for request in requests.into_iter().rev() {
                        self.settings.pending_screenshots.push_front(request);
                    }
                    return Ok(());
                };

                let (width, height, format) = {
                    let SurfaceMode::Windowed(surface_state) = &self.surface else {
                        unreachable!()
                    };
                    (
                        surface_state.config.width,
                        surface_state.config.height,
                        surface_state.config.format,
                    )
                };
                let texture_ref = &surface_texture.texture;

                for request in requests {
                    if request.path.to_string_lossy().contains("generals_internal_frame") {
                        eprintln!(
                            "DEBUG_SCREENSHOT: capturing path={} size={}x{} format={:?}",
                            request.path.display(),
                            width,
                            height,
                            format
                        );
                    }
                    if let Err(err) = self.queue_texture_capture_readback(
                        texture_ref,
                        width,
                        height,
                        format,
                        &request.path,
                    ) {
                        eprintln!(
                            "WW3D screenshot queue failed for {}: {}",
                            request.path.display(),
                            err
                        );
                    }
                }
            }
            SurfaceMode::Headless(_) => {
                let (width, height, format, texture) = {
                    let SurfaceMode::Headless(target) = &self.surface else {
                        unreachable!()
                    };
                    let (width, height) = target.size();
                    (
                        width,
                        height,
                        self.color_format,
                        target.texture().clone(),
                    )
                };

                for request in requests {
                    if let Err(err) = self.queue_texture_capture_readback(
                        &texture,
                        width,
                        height,
                        format,
                        &request.path,
                    ) {
                        eprintln!(
                            "WW3D screenshot queue failed for {}: {}",
                            request.path.display(),
                            err
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn queue_texture_capture_readback(
        &mut self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        path: &Path,
    ) -> EngineResult<()> {
        let bytes_per_pixel = texture_bytes_per_pixel(format).ok_or_else(|| {
            EngineError::Screenshot(format!(
                "Unsupported texture format {:?} for screenshot",
                format
            ))
        })?;

        let row_size = bytes_per_pixel * width as usize;
        let padded_row_size = align_to(row_size, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
        let buffer_size = padded_row_size * height as usize;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("WW3D Screenshot Buffer"),
            size: buffer_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .gpu_device
            .create_command_encoder(Some("WW3D Screenshot Encoder"));

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_size as u32),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.pending_screenshot_readbacks.push(ScreenshotReadback {
            path: path.to_path_buf(),
            buffer,
            width,
            height,
            format,
            row_size,
            padded_row_size,
            completion: receiver,
        });

        Ok(())
    }

    fn flush_pending_screenshot_readbacks(&mut self) {
        let mut remaining = Vec::new();
        for pending in std::mem::take(&mut self.pending_screenshot_readbacks) {
            match pending.completion.try_recv() {
                Ok(Ok(())) => {
                    if let Err(err) = self.finish_screenshot_readback(pending) {
                        eprintln!("WW3D screenshot finalize failed: {}", err);
                    }
                }
                Ok(Err(err)) => {
                    eprintln!("WW3D screenshot GPU map failed: {}", err);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => remaining.push(pending),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    eprintln!("WW3D screenshot completion channel disconnected");
                }
            }
        }
        self.pending_screenshot_readbacks = remaining;
    }

    fn finish_screenshot_readback(&self, pending: ScreenshotReadback) -> EngineResult<()> {
        let buffer_slice = pending.buffer.slice(..);
        let data = buffer_slice.get_mapped_range();
        let mapped_bytes = data.to_vec();

        drop(data);
        pending.buffer.unmap();

        let path = pending.path;
        let width = pending.width;
        let height = pending.height;
        let format = pending.format;
        let row_size = pending.row_size;
        let padded_row_size = pending.padded_row_size;
        std::thread::spawn(move || {
            let mut image_data = vec![0u8; width as usize * height as usize * 4];
            for (row_index, dest_chunk) in image_data.chunks_exact_mut(width as usize * 4).enumerate()
            {
                let src_offset = row_index * padded_row_size;
                let src_slice = &mapped_bytes[src_offset..src_offset + row_size];
                if let Err(err) = convert_row_to_rgba(format, src_slice, dest_chunk) {
                    eprintln!(
                        "WW3D screenshot row conversion failed for {}: {}",
                        path.display(),
                        err
                    );
                    return;
                }
            }

            if let Err(err) = write_screenshot_png(path.as_path(), width, height, &image_data) {
                eprintln!(
                    "WW3D screenshot encode/write failed for {}: {}",
                    path.display(),
                    err
                );
            }
        });
        Ok(())
    }

    fn capture_texture_to_file(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        path: &Path,
    ) -> EngineResult<()> {
        let bytes_per_pixel = texture_bytes_per_pixel(format).ok_or_else(|| {
            EngineError::Screenshot(format!(
                "Unsupported texture format {:?} for screenshot",
                format
            ))
        })?;

        let row_size = bytes_per_pixel * width as usize;
        let padded_row_size = align_to(row_size, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
        let buffer_size = padded_row_size * height as usize;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("WW3D Screenshot Buffer"),
            size: buffer_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .gpu_device
            .create_command_encoder(Some("WW3D Screenshot Encoder"));

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_size as u32),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        receiver
            .recv()
            .map_err(|_| EngineError::Screenshot("Screenshot readback channel closed".into()))?
            .map_err(|err| EngineError::Screenshot(format!("GPU buffer map failed: {err}")))?;

        let data = buffer_slice.get_mapped_range();
        let mut image_data = vec![0u8; width as usize * height as usize * 4];

        for (row_index, dest_chunk) in image_data.chunks_exact_mut(width as usize * 4).enumerate() {
            let src_offset = row_index * padded_row_size;
            let src_slice = &data[src_offset..src_offset + row_size];
            convert_row_to_rgba(format, src_slice, dest_chunk)?;
        }

        drop(data);
        buffer.unmap();

        write_screenshot_png(path, width, height, &image_data)
    }

}

fn write_screenshot_png(path: &Path, width: u32, height: u32, image_data: &[u8]) -> EngineResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                EngineError::Screenshot(format!(
                    "Failed to create screenshot directory {:?}: {}",
                    parent, err
                ))
            })?;
        }
    }

    let temp_path = unique_temp_path(path);

    let file = File::create(&temp_path).map_err(|err| {
        EngineError::Screenshot(format!(
            "Failed to create screenshot file {:?}: {}",
            temp_path, err
        ))
    })?;

    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.set_filter(png::FilterType::NoFilter);

    let mut writer = encoder
        .write_header()
        .map_err(|err| EngineError::Screenshot(format!("PNG header error: {err}")))?
        .into_stream_writer()
        .map_err(|err| EngineError::Screenshot(format!("PNG stream writer error: {err}")))?;

    writer
        .write_all(image_data)
        .map_err(|err| EngineError::Screenshot(format!("PNG write error: {err}")))?;
    writer
        .finish()
        .map_err(|err| EngineError::Screenshot(format!("PNG finalise error: {err}")))?;

    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp_path, path).map_err(|err| {
        EngineError::Screenshot(format!(
            "Failed to promote screenshot {:?} -> {:?}: {}",
            temp_path, path, err
        ))
    })?;

    let luma = compute_image_luma_u8(image_data);
    let meta_path = screenshot_meta_path(path);
    let meta_tmp_path = unique_temp_path(&meta_path);
    if fs::write(&meta_tmp_path, format!("luma={luma:.3}\n")).is_ok() {
        let _ = fs::rename(&meta_tmp_path, &meta_path);
    }

    if path.to_string_lossy().contains("generals_internal_frame") {
        eprintln!("DEBUG_SCREENSHOT: wrote path={}", path.display());
    }

    Ok(())
}

fn unique_temp_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let base_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ww3d-screenshot");
    let temp_name = format!(".{base_name}.{pid}.{stamp}.tmp");
    path.with_file_name(temp_name)
}

fn compute_image_luma_u8(image_data: &[u8]) -> f32 {
    if image_data.len() < 4 {
        return 0.0;
    }
    let mut accum = 0.0f32;
    let mut pixels = 0usize;
    for rgba in image_data.chunks_exact(4) {
        let luma = 0.2126 * rgba[0] as f32 + 0.7152 * rgba[1] as f32 + 0.0722 * rgba[2] as f32;
        accum += luma;
        pixels += 1;
    }
    if pixels == 0 {
        0.0
    } else {
        accum / pixels as f32
    }
}

fn screenshot_meta_path(path: &Path) -> PathBuf {
    let is_capture_path = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("capture"))
        .unwrap_or(false);
    if is_capture_path {
        return path.with_extension("").with_extension("png.meta");
    }
    path.with_extension("png.meta")
}

fn align_to(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        value
    } else {
        ((value + alignment - 1) / alignment) * alignment
    }
}

fn texture_bytes_per_pixel(format: wgpu::TextureFormat) -> Option<usize> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb => Some(4),
        _ => None,
    }
}

fn convert_row_to_rgba(
    format: wgpu::TextureFormat,
    src: &[u8],
    dest: &mut [u8],
) -> EngineResult<()> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            dest.copy_from_slice(src);
            Ok(())
        }
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            for (src_px, dest_px) in src.chunks_exact(4).zip(dest.chunks_exact_mut(4)) {
                dest_px[0] = src_px[2];
                dest_px[1] = src_px[1];
                dest_px[2] = src_px[0];
                dest_px[3] = src_px[3];
            }
            Ok(())
        }
        other => Err(EngineError::Screenshot(format!(
            "Unsupported texture format {:?} for screenshot",
            other
        ))),
    }
}

async fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'static>>,
    preference: wgpu::PowerPreference,
) -> EngineResult<wgpu::Adapter> {
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: preference,
            compatible_surface: surface,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|_| EngineError::NoAdapter)
}

fn select_surface_format(
    supported: &[wgpu::TextureFormat],
    desired: Option<wgpu::TextureFormat>,
) -> wgpu::TextureFormat {
    if let Some(format) = desired {
        if supported.contains(&format) {
            return format;
        }
    }

    supported
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .unwrap_or_else(|| supported[0])
}

fn select_present_mode(
    supported: &[wgpu::PresentMode],
    desired: Option<wgpu::PresentMode>,
) -> wgpu::PresentMode {
    if let Some(mode) = desired {
        if supported.contains(&mode) {
            return mode;
        }
    }

    supported
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::PresentMode::Mailbox)
        .or_else(|| {
            supported
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::PresentMode::Fifo)
        })
        .unwrap_or(supported[0])
}

fn select_alpha_mode(supported: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    supported
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
        .unwrap_or(supported[0])
}

/// Global engine singleton mirroring the original static WW3D API.
static ENGINE: OnceCell<Mutex<Option<Engine>>> = OnceCell::new();

fn global_engine() -> &'static Mutex<Option<Engine>> {
    ENGINE.get_or_init(|| Mutex::new(None))
}

/// Initialise the engine with a window surface (blocking variant).
/// Reference: ww3d.cpp:185-234 (WW3D::Init)
///
/// This is a blocking wrapper around the async init_with_window function.
/// Use this when you're not in an async context.
pub fn init_with_window_blocking<W>(window: W, config: EngineConfig) -> EngineResult<()>
where
    W: Into<wgpu::SurfaceTarget<'static>> + Send + Sync + 'static,
{
    pollster::block_on(init_with_window(window, config))
}

/// Initialise the engine with a window surface (matches WW3D::Init).
/// Reference: ww3d.cpp:185-234, ww3d.h:72
///
/// This function initializes the global WW3D engine instance with a window
/// for rendering. It must be called before any other WW3D functions.
///
/// # Arguments
/// * `window` - Window surface target (platform-specific)
/// * `config` - Engine configuration settings
///
/// # Errors
/// Returns `EngineError::AlreadyInitialised` if the engine is already initialized.
pub async fn init_with_window<W>(window: W, config: EngineConfig) -> EngineResult<()>
where
    W: Into<wgpu::SurfaceTarget<'static>> + Send + Sync + 'static,
{
    let mut slot = global_engine().lock();
    if slot.is_some() {
        return Err(EngineError::AlreadyInitialised);
    }

    let engine = Engine::new_windowed(window, config).await?;
    *slot = Some(engine);
    Ok(())
}

/// Initialise the engine without a surface (headless).
pub fn init_headless_blocking(config: EngineConfig) -> EngineResult<()> {
    pollster::block_on(init_headless(config))
}

/// Initialise the engine without a surface (headless).
pub async fn init_headless(config: EngineConfig) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    if slot.is_some() {
        return Err(EngineError::AlreadyInitialised);
    }

    let engine = Engine::new_headless(config).await?;
    *slot = Some(engine);
    Ok(())
}

/// Shut the engine down, releasing GPU resources (matches WW3D::Shutdown).
/// Reference: ww3d.cpp:249-300, ww3d.h:73
///
/// This function cleanly shuts down the WW3D engine and releases all GPU resources.
/// After calling this, you can call init again if needed.
///
/// The C++ version performs:
/// - Movie capture stop (if active)
/// - Timer resolution restoration
/// - Predictive LOD optimizer memory freeing
/// - Dazzle system deinitialization
/// - Debug resource release
/// - Asset manager cleanup
/// - DX8 texture manager shutdown
/// - DX8 wrapper shutdown
/// - Static sort lists deletion
/// - Animation sound manager shutdown
pub fn shutdown() -> EngineResult<()> {
    let mut slot = global_engine().lock();
    if slot.take().is_none() {
        return Err(EngineError::NotInitialised);
    }
    Ok(())
}

/// Begin rendering a frame using the global engine singleton (matches WW3D::Begin_Render).
/// Reference: ww3d.cpp:708-779, ww3d.h:115
///
/// This function must be called at the start of each frame, before any rendering.
/// It returns a RenderFrame which must be passed to end_render().
///
/// # Example
/// ```no_run
/// # use ww3d_engine::*;
/// # pollster::block_on(async {
/// let mut frame = begin_render()?;
/// // ... record rendering commands ...
/// end_render(frame)?;
/// # Ok::<(), EngineError>(())
/// # });
/// ```
pub fn begin_render() -> EngineResult<RenderFrame> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => engine.begin_render(),
        None => Err(EngineError::NotInitialised),
    }
}

/// Finish rendering the current frame (matches WW3D::End_Render).
/// Reference: ww3d.cpp:999-1041, ww3d.h:122
///
/// This function must be called after begin_render() to complete the frame.
/// It submits all rendering commands and presents the result.
///
/// # Arguments
/// * `frame` - The RenderFrame returned by begin_render()
pub fn end_render(frame: RenderFrame) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => engine.end_render(frame),
        None => Err(EngineError::NotInitialised),
    }
}

/// Adjust the global swap interval (VSync) setting.
pub fn set_swap_interval(interval: i32) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.set_swap_interval(interval);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Retrieve the global swap interval.
pub fn swap_interval() -> EngineResult<i32> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.swap_interval()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Global texture reduction setter.
pub fn set_texture_reduction(reduction: u32) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.set_texture_reduction(reduction);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Global texture reduction getter.
pub fn texture_reduction() -> EngineResult<u32> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.texture_reduction()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Enable or disable movie capture globally.
pub fn set_movie_capture_enabled(enabled: bool) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.set_movie_capture_enabled(enabled);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Queue a one-off movie capture frame using the global engine.
pub fn request_single_frame_capture() -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.request_single_frame_capture();
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Configure the directory and base filename used for movie capture frames.
pub fn set_movie_capture_output<P, S>(directory: P, base_name: S) -> EngineResult<()>
where
    P: AsRef<Path>,
    S: Into<String>,
{
    let dir_buf = directory.as_ref().to_path_buf();
    let name_string = base_name.into();
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.set_movie_capture_output(dir_buf, name_string);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Schedule a screenshot that will be captured on the next completed frame.
pub fn make_screenshot<P: AsRef<Path>>(path: P) -> EngineResult<()> {
    let Some(mut slot) = global_engine().try_lock() else {
        return Err(EngineError::Rendering(
            "WW3D screenshot scheduling skipped because engine is busy".to_string(),
        ));
    };
    match slot.as_mut() {
        Some(engine) => {
            engine.queue_screenshot(path.as_ref().to_path_buf());
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Notify the engine about a window resize event.
pub fn resize(width: u32, height: u32) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.resize(width, height);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Clone the global wgpu device handle.
pub fn device() -> EngineResult<Arc<wgpu::Device>> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.device()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Clone the global wgpu queue handle.
pub fn queue() -> EngineResult<Arc<wgpu::Queue>> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.queue()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Clone the higher-level GPU device wrapper.
pub fn gpu_device() -> EngineResult<Arc<GpuDevice>> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.gpu_device()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Fetch the current back buffer format used by the engine.
pub fn color_format() -> EngineResult<wgpu::TextureFormat> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.color_format()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Query the global depth format if depth is enabled.
pub fn depth_format() -> EngineResult<Option<wgpu::TextureFormat>> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.depth_format()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Report the current render surface size.
pub fn surface_size() -> EngineResult<(u32, u32)> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.surface_size()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Query adapter information for diagnostics/UI.
pub fn adapter_info() -> EngineResult<wgpu::AdapterInfo> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.adapter_info().clone()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Enumerate adapters matching the provided backend mask.
pub fn enumerate_adapters(backends: wgpu::Backends) -> Vec<wgpu::AdapterInfo> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
    });

    instance
        .enumerate_adapters(backends)
        .into_iter()
        .map(|adapter| adapter.get_info())
        .collect()
}

/// Update all engine subsystems (call before begin_render each frame)
pub fn update() -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => engine.update(),
        None => Err(EngineError::NotInitialised),
    }
}

/// Get current frame timing information
pub fn timing() -> EngineResult<FrameTiming> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.timing()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Get current FPS
pub fn fps() -> EngineResult<f32> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.fps()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Queue an input event for processing
pub fn queue_input(event: InputEvent) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.queue_input(event);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Set an input handler
pub fn set_input_handler(handler: Box<dyn InputHandler>) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.set_input_handler(handler);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Register a subsystem with the global engine
pub fn register_subsystem(subsystem: Box<dyn Subsystem>) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.register_subsystem(subsystem);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Access subsystems with a closure
pub fn with_subsystems<F, R>(f: F) -> EngineResult<R>
where
    F: FnOnce(&EngineSubsystems) -> R,
{
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(f(engine.subsystems())),
        None => Err(EngineError::NotInitialised),
    }
}

/// Access subsystems mutably with a closure
pub fn with_subsystems_mut<F, R>(f: F) -> EngineResult<R>
where
    F: FnOnce(&mut EngineSubsystems) -> R,
{
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => Ok(f(engine.subsystems_mut())),
        None => Err(EngineError::NotInitialised),
    }
}

/// Set the sync time for the current frame (matches WW3D::Sync())
/// Reference: ww3d.cpp:1097-1101, ww3d.h:134
///
/// This function synchronizes the engine's internal time with the game's logical time.
/// The application should call this at the start of every frame with the current game
/// time in milliseconds.
///
/// # Arguments
/// * `sync_time` - Current game time in milliseconds
pub fn sync(sync_time: u32) -> EngineResult<()> {
    let mut slot = global_engine().lock();
    match slot.as_mut() {
        Some(engine) => {
            engine.sync(sync_time);
            Ok(())
        }
        None => Err(EngineError::NotInitialised),
    }
}

/// Get the current sync time in milliseconds (matches WW3D::Get_Sync_Time())
/// Reference: ww3d.h:135
pub fn get_sync_time() -> EngineResult<u32> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.get_sync_time()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Get the frame time (delta) in milliseconds (matches WW3D::Get_Frame_Time())
/// Reference: ww3d.h:136
pub fn get_frame_time() -> EngineResult<u32> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.get_frame_time()),
        None => Err(EngineError::NotInitialised),
    }
}

/// Get the current frame count (matches WW3D::Get_Frame_Count())
/// Reference: ww3d.h:137
pub fn get_frame_count() -> EngineResult<u64> {
    let slot = global_engine().lock();
    match slot.as_ref() {
        Some(engine) => Ok(engine.get_frame_count()),
        None => Err(EngineError::NotInitialised),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_are_sensible() {
        let cfg = EngineConfig::default();
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.height, 720);
        assert!(cfg
            .surface_usage
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
        assert_eq!(cfg.sample_count, 1);
    }
}
